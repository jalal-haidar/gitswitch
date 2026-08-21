use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use gitswitch_lib::native_test_support::{
    credential_account, decide_auto_switch, AppConfig, ConfigHarness, CredentialFailure,
    DirectoryRule, GitConfigSnapshot, GitProfile, GitSandbox, TestAutoSwitchDecision,
};

fn profile(id: &str, ssh_key: Option<&Path>) -> GitProfile {
    GitProfile {
        id: id.to_string(),
        label: "Work".to_string(),
        name: "New Name".to_string(),
        email: "new@example.com".to_string(),
        color: "#123456".to_string(),
        ssh_key_path: ssh_key.map(|path| path.to_string_lossy().into_owned()),
        gpg_key_id: Some("NEW-GPG".to_string()),
        is_default: true,
        remote_url: None,
        remote_service: None,
    }
}

fn baseline() -> GitConfigSnapshot {
    GitConfigSnapshot {
        user_name: Some("Original Name".to_string()),
        user_email: Some("original@example.com".to_string()),
        user_signingkey: Some("ORIGINAL-GPG".to_string()),
        commit_gpgsign: Some("false".to_string()),
        core_ssh_command: Some("ssh -i \"original-key\" -o IdentitiesOnly=yes".to_string()),
    }
}

fn seed_global(sandbox: &GitSandbox, values: &GitConfigSnapshot) {
    sandbox
        .set_global("user.name", values.user_name.as_deref().unwrap())
        .unwrap();
    sandbox
        .set_global("user.email", values.user_email.as_deref().unwrap())
        .unwrap();
    sandbox
        .set_global(
            "user.signingkey",
            values.user_signingkey.as_deref().unwrap(),
        )
        .unwrap();
    sandbox
        .set_global("commit.gpgsign", values.commit_gpgsign.as_deref().unwrap())
        .unwrap();
    sandbox
        .set_global(
            "core.sshCommand",
            values.core_ssh_command.as_deref().unwrap(),
        )
        .unwrap();
}

fn seed_local(sandbox: &GitSandbox, repo: &Path, values: &GitConfigSnapshot) {
    sandbox
        .set_local(repo, "user.name", values.user_name.as_deref().unwrap())
        .unwrap();
    sandbox
        .set_local(repo, "user.email", values.user_email.as_deref().unwrap())
        .unwrap();
    sandbox
        .set_local(
            repo,
            "user.signingkey",
            values.user_signingkey.as_deref().unwrap(),
        )
        .unwrap();
    sandbox
        .set_local(
            repo,
            "commit.gpgsign",
            values.commit_gpgsign.as_deref().unwrap(),
        )
        .unwrap();
    sandbox
        .set_local(
            repo,
            "core.sshCommand",
            values.core_ssh_command.as_deref().unwrap(),
        )
        .unwrap();
}

fn expected_profile_values(key: &Path) -> GitConfigSnapshot {
    GitConfigSnapshot {
        user_name: Some("New Name".to_string()),
        user_email: Some("new@example.com".to_string()),
        user_signingkey: Some("NEW-GPG".to_string()),
        commit_gpgsign: Some("true".to_string()),
        core_ssh_command: Some(format!(
            "ssh -i \"{}\" -o IdentitiesOnly=yes",
            key.to_string_lossy().replace('\\', "/")
        )),
    }
}

fn config_with_profile(profile: GitProfile) -> AppConfig {
    let mut config = AppConfig::default();
    config.profiles.push(profile);
    config
}

#[test]
fn global_apply_and_restore_are_isolated_and_exact() {
    let sandbox = GitSandbox::new().unwrap();
    let guard = sandbox.guard_bytes().unwrap();
    let original = baseline();
    let key = sandbox.create_ssh_key("global-key").unwrap();
    seed_global(&sandbox, &original);

    let captured = sandbox.read_global().unwrap();
    assert_eq!(captured, original);
    sandbox
        .apply_global(&profile("global", Some(&key)))
        .unwrap();
    assert_eq!(
        sandbox.read_global().unwrap(),
        expected_profile_values(&key)
    );
    sandbox.restore_global(&captured).unwrap();
    assert_eq!(sandbox.read_global().unwrap(), original);

    let environment = sandbox.isolation_environment();
    assert_eq!(
        environment.get("GIT_CONFIG_NOSYSTEM").map(String::as_str),
        Some("1")
    );
    assert_eq!(
        environment.get("GIT_CONFIG_GLOBAL").map(PathBuf::from),
        Some(sandbox.global_config_path().to_path_buf())
    );
    assert_eq!(sandbox.guard_bytes().unwrap(), guard);
    assert!(!sandbox.calls().is_empty());
}

#[test]
fn repo_local_apply_and_restore_cover_all_five_values() {
    let sandbox = GitSandbox::new().unwrap();
    let repo = sandbox.init_repo("repo").unwrap();
    let key = sandbox.create_ssh_key("repo-key").unwrap();
    let original = baseline();
    seed_local(&sandbox, &repo, &original);

    let captured = sandbox.read_local(&repo).unwrap();
    sandbox
        .apply_local(&repo, &profile("local", Some(&key)))
        .unwrap();
    assert_eq!(
        sandbox.read_local(&repo).unwrap(),
        expected_profile_values(&key)
    );
    sandbox.restore_local(&repo, &captured).unwrap();
    assert_eq!(sandbox.read_local(&repo).unwrap(), original);
}

#[test]
fn restore_removes_values_that_were_initially_unset() {
    let sandbox = GitSandbox::new().unwrap();
    let repo = sandbox.init_repo("unset-repo").unwrap();
    let key = sandbox.create_ssh_key("unset-key").unwrap();
    let original = sandbox.read_local(&repo).unwrap();
    assert_eq!(
        original,
        GitConfigSnapshot {
            user_name: None,
            user_email: None,
            user_signingkey: None,
            commit_gpgsign: None,
            core_ssh_command: None,
        }
    );

    sandbox
        .apply_local(&repo, &profile("unset", Some(&key)))
        .unwrap();
    sandbox.restore_local(&repo, &original).unwrap();
    assert_eq!(sandbox.read_local(&repo).unwrap(), original);
}

#[test]
fn later_git_failure_characterizes_current_partial_global_write() {
    let sandbox = GitSandbox::new().unwrap();
    let key = sandbox.create_ssh_key("failure-key").unwrap();
    let original = baseline();
    seed_global(&sandbox, &original);
    sandbox.fail_next_write("commit.gpgsign");

    let error = sandbox
        .apply_global(&profile("failure", Some(&key)))
        .unwrap_err();
    assert!(error.contains("injected Git failure"));
    assert_eq!(
        sandbox.read_global().unwrap(),
        GitConfigSnapshot {
            user_name: Some("New Name".to_string()),
            user_email: Some("new@example.com".to_string()),
            user_signingkey: Some("NEW-GPG".to_string()),
            commit_gpgsign: original.commit_gpgsign,
            core_ssh_command: original.core_ssh_command,
        }
    );
}

#[test]
fn missing_repo_ssh_key_characterizes_current_partial_write() {
    let sandbox = GitSandbox::new().unwrap();
    let repo = sandbox.init_repo("missing-key-repo").unwrap();
    let original = baseline();
    seed_local(&sandbox, &repo, &original);
    let missing = sandbox.root().join("does-not-exist");

    let error = sandbox
        .apply_local(&repo, &profile("missing", Some(&missing)))
        .unwrap_err();
    assert!(error.contains("SSH key file not found"));
    assert_eq!(
        sandbox.read_local(&repo).unwrap(),
        GitConfigSnapshot {
            user_name: Some("New Name".to_string()),
            user_email: Some("new@example.com".to_string()),
            user_signingkey: Some("NEW-GPG".to_string()),
            commit_gpgsign: Some("true".to_string()),
            core_ssh_command: original.core_ssh_command,
        }
    );
}

#[test]
fn keyring_enable_write_failure_rolls_back_json_and_credentials() {
    let harness = ConfigHarness::new().unwrap();
    let key = harness.root().join("id_ed25519");
    fs::write(&key, b"key").unwrap();
    let profile = profile("profile-1", Some(&key));
    let config = config_with_profile(profile);
    harness.write(&config).unwrap();
    let original = harness.bytes().unwrap();
    harness.fail(
        CredentialFailure::Write,
        &credential_account("profile-1", "ssh_key_path"),
    );

    let mut loaded = harness.load().unwrap();
    loaded.settings.store_sensitive_in_keyring = true;
    let error = harness.persist(&loaded).unwrap_err();
    assert!(error.contains("SecureStorageError"));
    assert_eq!(harness.bytes().unwrap(), original);
    assert!(harness.entries().is_empty());
}

#[test]
fn keyring_disable_delete_failure_restores_json_and_credentials() {
    let harness = ConfigHarness::new().unwrap();
    let key = harness.root().join("id_ed25519");
    fs::write(&key, b"key").unwrap();
    harness
        .write(&config_with_profile(profile("profile-1", Some(&key))))
        .unwrap();
    let mut enabled = harness.load().unwrap();
    enabled.settings.store_sensitive_in_keyring = true;
    harness.persist(&enabled).unwrap();
    let secured = harness.bytes().unwrap();
    let expected_entries = harness.entries();

    harness.fail(
        CredentialFailure::Delete,
        &credential_account("profile-1", "ssh_key_path"),
    );
    let mut disabled = harness.load().unwrap();
    disabled.settings.store_sensitive_in_keyring = false;
    let error = harness.persist(&disabled).unwrap_err();
    assert!(error.contains("SecureStorageError"));
    assert_eq!(harness.bytes().unwrap(), secured);
    assert_eq!(harness.entries(), expected_entries);
}

#[test]
fn auto_switch_decision_is_deterministic_without_a_watcher() {
    let sandbox = GitSandbox::new().unwrap();
    let parent = sandbox.root().join("projects");
    let repo = parent.join("nested");
    fs::create_dir_all(&parent).unwrap();
    sandbox.init_repo("projects/nested").unwrap();
    let event_path = repo.join("src").join("main.rs");
    fs::create_dir_all(event_path.parent().unwrap()).unwrap();
    fs::write(&event_path, b"fn main() {}\n").unwrap();
    let key = sandbox.create_ssh_key("auto-key").unwrap();
    let selected_profile = profile("nested-profile", Some(&key));
    let mut config = config_with_profile(selected_profile.clone());
    let rules = vec![
        DirectoryRule {
            id: "parent-rule".to_string(),
            path: parent.to_string_lossy().into_owned(),
            profile_id: "parent-profile".to_string(),
            last_triggered_at: None,
        },
        DirectoryRule {
            id: "nested-rule".to_string(),
            path: repo.to_string_lossy().into_owned(),
            profile_id: selected_profile.id.clone(),
            last_triggered_at: None,
        },
    ];

    assert!(matches!(
        decide_auto_switch(&sandbox, &config, &rules, std::slice::from_ref(&event_path), None),
        TestAutoSwitchDecision::Apply { ref rule_id, ref profile_id, ref repo }
            if rule_id == "nested-rule"
                && profile_id == "nested-profile"
                && repo.ends_with("nested")
    ));
    assert_eq!(
        decide_auto_switch(
            &sandbox,
            &config,
            &rules,
            std::slice::from_ref(&event_path),
            Some(Duration::from_millis(100)),
        ),
        TestAutoSwitchDecision::Debounced
    );

    sandbox.apply_local(&repo, &selected_profile).unwrap();
    assert!(matches!(
        decide_auto_switch(&sandbox, &config, &rules, std::slice::from_ref(&event_path), None),
        TestAutoSwitchDecision::AlreadyApplied(ref matched) if matched.ends_with("nested")
    ));

    let non_repo_event = parent.join("outside-repo.txt");
    fs::write(&non_repo_event, b"not in a repository\n").unwrap();
    assert!(matches!(
        decide_auto_switch(&sandbox, &config, &rules[..1], &[non_repo_event], None,),
        TestAutoSwitchDecision::MissingRepository(_)
    ));

    config.settings.auto_switch = false;
    assert_eq!(
        decide_auto_switch(
            &sandbox,
            &config,
            &rules,
            std::slice::from_ref(&event_path),
            None
        ),
        TestAutoSwitchDecision::Disabled
    );
    assert_eq!(
        decide_auto_switch(&sandbox, &config, &rules, &[repo.join("ignored.tmp")], None,),
        TestAutoSwitchDecision::NoMatch
    );
}
