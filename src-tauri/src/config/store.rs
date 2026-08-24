use anyhow::{anyhow, Context, Result};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use tauri::{AppHandle, Manager};

use keyring::Entry;
use once_cell::sync::Lazy;

use crate::errors::BackendError;
use crate::models::AppConfig;

/// Serializes the complete read-modify-write transaction. This prevents two
/// commands from loading the same config and silently overwriting each other.
static CONFIG_TXN_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

const CONFIG_FILE_NAME: &str = "profiles.json";
const KEYRING_SERVICE: &str = "gitswitch";

pub(crate) trait CredentialStore {
    fn get(&self, account: &str) -> Result<Option<String>>;
    fn set(&self, account: &str, value: &str) -> Result<()>;
    fn delete(&self, account: &str) -> Result<()>;
}

struct OsCredentialStore;

impl CredentialStore for OsCredentialStore {
    fn get(&self, account: &str) -> Result<Option<String>> {
        match Entry::new(KEYRING_SERVICE, account).get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn set(&self, account: &str, value: &str) -> Result<()> {
        Entry::new(KEYRING_SERVICE, account)
            .set_password(value)
            .map_err(Into::into)
    }

    fn delete(&self, account: &str) -> Result<()> {
        match Entry::new(KEYRING_SERVICE, account).delete_password() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

fn config_lock() -> MutexGuard<'static, ()> {
    CONFIG_TXN_LOCK.lock().unwrap_or_else(|poisoned| {
        eprintln!("[store] config transaction mutex poisoned, recovering");
        poisoned.into_inner()
    })
}

fn credential_account(profile_id: &str, field: &str) -> String {
    format!("{profile_id}:{field}")
}

fn profile_accounts(profile_id: &str) -> [String; 2] {
    [
        credential_account(profile_id, "ssh_key_path"),
        credential_account(profile_id, "gpg_key_id"),
    ]
}

fn get_config_path(app_handle: &AppHandle) -> Result<PathBuf> {
    let app_dir = app_handle
        .path()
        .app_config_dir()
        .context("Failed to get app config dir")?;

    if !app_dir.exists() {
        fs::create_dir_all(&app_dir).context("Failed to create app config directory")?;
    }

    Ok(app_dir.join(CONFIG_FILE_NAME))
}

pub fn load_config(app_handle: &AppHandle) -> Result<AppConfig> {
    let _guard = config_lock();
    let config_path = get_config_path(app_handle)?;
    load_config_at(&config_path, &OsCredentialStore)
}

/// Runs a complete configuration update under a process-wide lock. Callers
/// must put every config mutation inside the closure to avoid lost updates.
pub fn update_config<T, F>(app_handle: &AppHandle, mutation: F) -> std::result::Result<T, String>
where
    F: FnOnce(&mut AppConfig) -> std::result::Result<T, String>,
{
    let _guard = config_lock();
    let config_path = get_config_path(app_handle).map_err(|error| error.to_string())?;
    let credentials = OsCredentialStore;
    let mut config = load_config_at(&config_path, &credentials).map_err(|e| e.to_string())?;
    let result = mutation(&mut config)?;
    persist_config_at(&config_path, &config, &credentials).map_err(|e| e.to_string())?;
    Ok(result)
}

pub(crate) fn load_config_at(path: &Path, credentials: &dyn CredentialStore) -> Result<AppConfig> {
    let backup_path = path.with_extension("json.bak");
    if !path.exists() && backup_path.exists() {
        fs::rename(&backup_path, path)
            .with_context(|| format!("Failed to recover config backup at {backup_path:?}"))?;
    }

    if !path.exists() {
        let config = AppConfig {
            keyring_entries: Some(HashSet::new()),
            ..AppConfig::default()
        };
        let contents =
            serde_json::to_vec_pretty(&config).context("Failed to serialize AppConfig")?;
        atomic_replace(path, &contents)?;
        return Ok(config);
    }

    let contents =
        fs::read(path).with_context(|| format!("Failed to read config file at {path:?}"))?;
    let mut config: AppConfig = serde_json::from_slice(&contents)
        .with_context(|| format!("Failed to parse config file at {path:?}"))?;

    let accounts = match &config.keyring_entries {
        Some(entries) => entries.clone(),
        None if config.settings.store_sensitive_in_keyring => {
            // Legacy configs had no manifest. Probe only the two deterministic
            // accounts for each profile and persist the discovered manifest on
            // the next successful update.
            let mut discovered = HashSet::new();
            for profile in &config.profiles {
                for account in profile_accounts(&profile.id) {
                    match read_credential(credentials, &account)? {
                        Some(value) if !value.is_empty() => {
                            config
                                .keyring_baseline
                                .entries
                                .insert(account.clone(), value);
                            discovered.insert(account);
                        }
                        _ => {}
                    }
                }
            }
            discovered
        }
        None => HashSet::new(),
    };

    for account in &accounts {
        let value = if let Some(value) = config.keyring_baseline.entries.get(account) {
            value.clone()
        } else {
            read_credential(credentials, account)?
                .ok_or_else(|| anyhow::Error::new(BackendError::secure_storage_missing(account)))?
        };
        config
            .keyring_baseline
            .entries
            .insert(account.clone(), value.clone());
        hydrate_profile(&mut config, account, value);
    }
    config.keyring_entries = Some(accounts);

    Ok(config)
}

fn read_credential(credentials: &dyn CredentialStore, account: &str) -> Result<Option<String>> {
    credentials.get(account).map_err(|error| {
        anyhow::Error::new(BackendError::secure_storage(
            "read",
            account,
            error.to_string(),
        ))
    })
}

fn hydrate_profile(config: &mut AppConfig, account: &str, value: String) {
    for profile in &mut config.profiles {
        if account == credential_account(&profile.id, "ssh_key_path") {
            profile.ssh_key_path = Some(value);
            return;
        }
        if account == credential_account(&profile.id, "gpg_key_id") {
            profile.gpg_key_id = Some(value);
            return;
        }
    }
}

fn desired_credentials(config: &AppConfig) -> HashMap<String, String> {
    if !config.settings.store_sensitive_in_keyring {
        return HashMap::new();
    }

    let mut desired = HashMap::new();
    for profile in &config.profiles {
        if let Some(value) = profile
            .ssh_key_path
            .as_ref()
            .filter(|value| !value.is_empty())
        {
            desired.insert(
                credential_account(&profile.id, "ssh_key_path"),
                value.clone(),
            );
        }
        if let Some(value) = profile
            .gpg_key_id
            .as_ref()
            .filter(|value| !value.is_empty())
        {
            desired.insert(credential_account(&profile.id, "gpg_key_id"), value.clone());
        }
    }
    desired
}

pub(crate) fn persist_config_at(
    path: &Path,
    config: &AppConfig,
    credentials: &dyn CredentialStore,
) -> Result<()> {
    let old_contents = if path.exists() {
        Some(fs::read(path).with_context(|| format!("Failed to read config file at {path:?}"))?)
    } else {
        None
    };
    let baseline = &config.keyring_baseline.entries;
    let desired = desired_credentials(config);

    // Serialize before touching secure storage so even an unexpected
    // serialization failure leaves both persistence layers untouched.
    let mut config_for_save = config.clone();
    if config_for_save.settings.store_sensitive_in_keyring {
        for profile in &mut config_for_save.profiles {
            profile.ssh_key_path = None;
            profile.gpg_key_id = None;
        }
    }
    config_for_save.keyring_entries = Some(desired.keys().cloned().collect());
    config_for_save.keyring_baseline.entries.clear();
    let contents =
        serde_json::to_vec_pretty(&config_for_save).context("Failed to serialize AppConfig")?;

    let mut upserts: Vec<_> = desired
        .iter()
        .filter(|(account, value)| baseline.get(*account) != Some(*value))
        .map(|(account, value)| (account.clone(), value.clone()))
        .collect();
    upserts.sort_by(|a, b| a.0.cmp(&b.0));

    let mut deletes: Vec<_> = baseline
        .keys()
        .filter(|account| !desired.contains_key(*account))
        .cloned()
        .collect();
    deletes.sort();

    let mut applied_upserts = Vec::new();
    for (account, value) in &upserts {
        if let Err(error) = credentials.set(account, value) {
            let rollback = rollback_upserts(credentials, baseline, &applied_upserts);
            return Err(secure_storage_failure("write", account, error, rollback));
        }
        applied_upserts.push(account.clone());
    }

    if let Err(error) = atomic_replace(path, &contents) {
        let rollback = rollback_upserts(credentials, baseline, &applied_upserts);
        return Err(with_rollback_context(error, rollback));
    }

    let mut applied_deletes = Vec::new();
    for account in &deletes {
        if let Err(error) = credentials.delete(account) {
            let mut rollback = rollback_deletes(credentials, baseline, &applied_deletes);
            if let Err(restore_error) = restore_config(path, old_contents.as_deref()) {
                rollback.push(format!("config restore failed: {restore_error}"));
            }
            rollback.extend(rollback_upserts(credentials, baseline, &applied_upserts));
            return Err(secure_storage_failure("delete", account, error, rollback));
        }
        applied_deletes.push(account.clone());
    }

    Ok(())
}

fn rollback_upserts(
    credentials: &dyn CredentialStore,
    baseline: &HashMap<String, String>,
    accounts: &[String],
) -> Vec<String> {
    let mut failures = Vec::new();
    for account in accounts.iter().rev() {
        let result = match baseline.get(account) {
            Some(value) => credentials.set(account, value),
            None => credentials.delete(account),
        };
        if let Err(error) = result {
            failures.push(format!(
                "credential rollback failed for '{account}': {error}"
            ));
        }
    }
    failures
}

fn rollback_deletes(
    credentials: &dyn CredentialStore,
    baseline: &HashMap<String, String>,
    accounts: &[String],
) -> Vec<String> {
    let mut failures = Vec::new();
    for account in accounts.iter().rev() {
        if let Some(value) = baseline.get(account) {
            if let Err(error) = credentials.set(account, value) {
                failures.push(format!(
                    "credential restore failed for '{account}': {error}"
                ));
            }
        }
    }
    failures
}

fn secure_storage_failure(
    operation: &str,
    account: &str,
    error: anyhow::Error,
    rollback: Vec<String>,
) -> anyhow::Error {
    let mut details = error.to_string();
    if !rollback.is_empty() {
        details.push_str("; ");
        details.push_str(&rollback.join("; "));
    }
    anyhow::Error::new(BackendError::secure_storage(operation, account, details))
}

fn with_rollback_context(error: anyhow::Error, rollback: Vec<String>) -> anyhow::Error {
    if rollback.is_empty() {
        error
    } else {
        error.context(rollback.join("; "))
    }
}

fn restore_config(path: &Path, contents: Option<&[u8]>) -> Result<()> {
    match contents {
        Some(contents) => atomic_replace(path, contents),
        None if path.exists() => fs::remove_file(path)
            .with_context(|| format!("Failed to remove newly created config at {path:?}")),
        None => Ok(()),
    }
}

pub(crate) fn atomic_replace(config_path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config directory at {parent:?}"))?;
    }
    let tmp_path = config_path.with_extension("json.tmp");
    let backup_path = config_path.with_extension("json.bak");

    let mut tmp_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&tmp_path)
        .with_context(|| format!("Failed to create temp config file at {:?}", tmp_path))?;

    tmp_file
        .write_all(contents)
        .context("Failed to write to temp config file")?;

    // Ensure contents are flushed to disk
    tmp_file
        .sync_all()
        .context("Failed to sync temp config file to disk")?;

    if backup_path.exists() {
        fs::remove_file(&backup_path)
            .with_context(|| format!("Failed to remove stale config backup at {backup_path:?}"))?;
    }
    if config_path.exists() {
        fs::rename(config_path, &backup_path)
            .with_context(|| format!("Failed to stage existing config at {backup_path:?}"))?;
    }

    if let Err(error) = fs::rename(&tmp_path, config_path) {
        if backup_path.exists() {
            let _ = fs::rename(&backup_path, config_path);
        }
        return Err(anyhow!(
            "Failed to install temp config at {config_path:?}: {error}"
        ));
    }

    if backup_path.exists() {
        // The new config is already safely installed. A stale backup is
        // harmless and can be cleaned on the next write; it must not turn a
        // committed replacement into a reported failure.
        if let Err(error) = fs::remove_file(&backup_path) {
            eprintln!("[store] failed to remove config backup at {backup_path:?}: {error}");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AppSettings, GitProfile};
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[derive(Default)]
    struct MockCredentialStore {
        entries: Mutex<HashMap<String, String>>,
        fail_reads: Mutex<HashSet<String>>,
        fail_writes: Mutex<HashSet<String>>,
        fail_deletes: Mutex<HashSet<String>>,
        write_calls: Mutex<Vec<String>>,
        delete_calls: Mutex<Vec<String>>,
    }

    impl CredentialStore for MockCredentialStore {
        fn get(&self, account: &str) -> Result<Option<String>> {
            if self.fail_reads.lock().unwrap().contains(account) {
                return Err(anyhow!("mock read failure"));
            }
            Ok(self.entries.lock().unwrap().get(account).cloned())
        }

        fn set(&self, account: &str, value: &str) -> Result<()> {
            self.write_calls.lock().unwrap().push(account.to_string());
            if self.fail_writes.lock().unwrap().contains(account) {
                return Err(anyhow!("mock write failure"));
            }
            self.entries
                .lock()
                .unwrap()
                .insert(account.to_string(), value.to_string());
            Ok(())
        }

        fn delete(&self, account: &str) -> Result<()> {
            self.delete_calls.lock().unwrap().push(account.to_string());
            if self.fail_deletes.lock().unwrap().contains(account) {
                return Err(anyhow!("mock delete failure"));
            }
            self.entries.lock().unwrap().remove(account);
            Ok(())
        }
    }

    struct TestConfigPath {
        dir: PathBuf,
        path: PathBuf,
    }

    impl TestConfigPath {
        fn new() -> Self {
            let suffix = TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "gitswitch-store-test-{}-{suffix}",
                std::process::id()
            ));
            fs::create_dir_all(&dir).unwrap();
            let path = dir.join(CONFIG_FILE_NAME);
            Self { dir, path }
        }

        fn write(&self, config: &AppConfig) -> Vec<u8> {
            let bytes = serde_json::to_vec_pretty(config).unwrap();
            fs::write(&self.path, &bytes).unwrap();
            bytes
        }

        fn json(&self) -> serde_json::Value {
            serde_json::from_slice(&fs::read(&self.path).unwrap()).unwrap()
        }
    }

    impl Drop for TestConfigPath {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    fn test_profile() -> GitProfile {
        GitProfile {
            id: "profile-1".to_string(),
            label: "Work".to_string(),
            name: "Alice".to_string(),
            email: "alice@example.com".to_string(),
            color: "#123456".to_string(),
            ssh_key_path: Some("C:/Users/Alice/.ssh/id_ed25519".to_string()),
            gpg_key_id: Some("ABC123".to_string()),
            is_default: true,
            remote_url: None,
            remote_service: None,
        }
    }

    fn plaintext_config() -> AppConfig {
        AppConfig {
            profiles: vec![test_profile()],
            settings: AppSettings {
                store_sensitive_in_keyring: false,
                ..AppSettings::default()
            },
            keyring_entries: Some(HashSet::new()),
            ..AppConfig::default()
        }
    }

    fn secured_config(credentials: &MockCredentialStore) -> AppConfig {
        let mut config = plaintext_config();
        config.settings.store_sensitive_in_keyring = true;
        let desired = desired_credentials(&config);
        *credentials.entries.lock().unwrap() = desired.clone();
        config.keyring_entries = Some(desired.keys().cloned().collect());
        for profile in &mut config.profiles {
            profile.ssh_key_path = None;
            profile.gpg_key_id = None;
        }
        config
    }

    #[test]
    fn enabling_secure_storage_redacts_json_only_after_credentials_are_written() {
        let files = TestConfigPath::new();
        files.write(&plaintext_config());
        let credentials = MockCredentialStore::default();

        let mut config = load_config_at(&files.path, &credentials).unwrap();
        config.settings.store_sensitive_in_keyring = true;
        persist_config_at(&files.path, &config, &credentials).unwrap();

        let json = files.json();
        assert!(json["profiles"][0]["sshKeyPath"].is_null());
        assert!(json["profiles"][0]["gpgKeyId"].is_null());
        assert_eq!(json["keyringEntries"].as_array().unwrap().len(), 2);
        let entries = credentials.entries.lock().unwrap();
        assert_eq!(
            entries.get("profile-1:ssh_key_path").map(String::as_str),
            Some("C:/Users/Alice/.ssh/id_ed25519")
        );
        assert_eq!(
            entries.get("profile-1:gpg_key_id").map(String::as_str),
            Some("ABC123")
        );
    }

    #[test]
    fn disabling_secure_storage_restores_plaintext_then_deletes_credentials() {
        let files = TestConfigPath::new();
        let credentials = MockCredentialStore::default();
        files.write(&secured_config(&credentials));

        let mut config = load_config_at(&files.path, &credentials).unwrap();
        config.settings.store_sensitive_in_keyring = false;
        persist_config_at(&files.path, &config, &credentials).unwrap();

        let json = files.json();
        assert_eq!(
            json["profiles"][0]["sshKeyPath"],
            "C:/Users/Alice/.ssh/id_ed25519"
        );
        assert_eq!(json["profiles"][0]["gpgKeyId"], "ABC123");
        assert_eq!(json["keyringEntries"].as_array().unwrap().len(), 0);
        assert!(credentials.entries.lock().unwrap().is_empty());
    }

    #[test]
    fn write_failure_rolls_back_credentials_and_keeps_original_json() {
        let files = TestConfigPath::new();
        let original = files.write(&plaintext_config());
        let credentials = MockCredentialStore::default();
        credentials
            .fail_writes
            .lock()
            .unwrap()
            .insert("profile-1:ssh_key_path".to_string());

        let mut config = load_config_at(&files.path, &credentials).unwrap();
        config.settings.store_sensitive_in_keyring = true;
        let error = persist_config_at(&files.path, &config, &credentials)
            .unwrap_err()
            .to_string();

        assert!(error.contains("SecureStorageError"));
        assert!(!error.contains("ABC123"));
        assert!(!error.contains("id_ed25519"));
        assert_eq!(fs::read(&files.path).unwrap(), original);
        assert!(credentials.entries.lock().unwrap().is_empty());
    }

    #[test]
    fn read_failure_is_actionable_and_does_not_modify_config() {
        let files = TestConfigPath::new();
        let credentials = MockCredentialStore::default();
        let original = files.write(&secured_config(&credentials));
        credentials
            .fail_reads
            .lock()
            .unwrap()
            .insert("profile-1:gpg_key_id".to_string());

        let error = load_config_at(&files.path, &credentials)
            .unwrap_err()
            .to_string();
        assert!(error.contains("SecureStorageError"));
        assert!(error.contains("previous profile settings were not changed"));
        assert_eq!(fs::read(&files.path).unwrap(), original);
    }

    #[test]
    fn expected_manifest_entry_missing_is_an_error() {
        let files = TestConfigPath::new();
        let credentials = MockCredentialStore::default();
        let config = secured_config(&credentials);
        files.write(&config);
        credentials.entries.lock().unwrap().clear();

        let error = load_config_at(&files.path, &credentials)
            .unwrap_err()
            .to_string();
        assert!(error.contains("expected profile value is missing"));
        assert!(error.contains("SecureStorageError"));
    }

    #[test]
    fn profile_deletion_removes_both_manifested_credentials() {
        let files = TestConfigPath::new();
        let credentials = MockCredentialStore::default();
        files.write(&secured_config(&credentials));

        let mut config = load_config_at(&files.path, &credentials).unwrap();
        config.profiles.clear();
        persist_config_at(&files.path, &config, &credentials).unwrap();

        assert!(credentials.entries.lock().unwrap().is_empty());
        let calls = credentials.delete_calls.lock().unwrap();
        assert!(calls.contains(&"profile-1:ssh_key_path".to_string()));
        assert!(calls.contains(&"profile-1:gpg_key_id".to_string()));
    }

    #[test]
    fn delete_failure_restores_json_and_already_deleted_credentials() {
        let files = TestConfigPath::new();
        let credentials = MockCredentialStore::default();
        let original = files.write(&secured_config(&credentials));
        credentials
            .fail_deletes
            .lock()
            .unwrap()
            .insert("profile-1:ssh_key_path".to_string());

        let mut config = load_config_at(&files.path, &credentials).unwrap();
        config.settings.store_sensitive_in_keyring = false;
        let error = persist_config_at(&files.path, &config, &credentials)
            .unwrap_err()
            .to_string();

        assert!(error.contains("SecureStorageError"));
        assert_eq!(fs::read(&files.path).unwrap(), original);
        let entries = credentials.entries.lock().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries.get("profile-1:gpg_key_id").map(String::as_str),
            Some("ABC123")
        );
    }

    #[test]
    fn unchanged_save_does_not_rewrite_credentials() {
        let files = TestConfigPath::new();
        let credentials = MockCredentialStore::default();
        files.write(&secured_config(&credentials));

        let config = load_config_at(&files.path, &credentials).unwrap();
        persist_config_at(&files.path, &config, &credentials).unwrap();

        assert!(credentials.write_calls.lock().unwrap().is_empty());
        assert!(credentials.delete_calls.lock().unwrap().is_empty());
    }

    #[test]
    fn legacy_config_discovers_and_migrates_deterministic_accounts() {
        let files = TestConfigPath::new();
        let credentials = MockCredentialStore::default();
        let mut config = plaintext_config();
        config.settings.store_sensitive_in_keyring = true;
        config.keyring_entries = None;
        for profile in &mut config.profiles {
            profile.ssh_key_path = None;
            profile.gpg_key_id = None;
        }
        credentials.entries.lock().unwrap().insert(
            "profile-1:ssh_key_path".to_string(),
            "C:/legacy/key".to_string(),
        );
        files.write(&config);

        let loaded = load_config_at(&files.path, &credentials).unwrap();
        assert_eq!(
            loaded.profiles[0].ssh_key_path.as_deref(),
            Some("C:/legacy/key")
        );
        assert!(loaded
            .keyring_entries
            .as_ref()
            .unwrap()
            .contains("profile-1:ssh_key_path"));
    }
}
