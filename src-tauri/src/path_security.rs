use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

fn raw_user_home() -> Result<PathBuf, String> {
    env::var("USERPROFILE")
        .or_else(|_| env::var("HOME"))
        .map(PathBuf::from)
        .map_err(|_| "Cannot determine the current user's home directory".to_string())
}

fn expand_home(raw: &str, home: &Path) -> Result<PathBuf, String> {
    if raw == "~" {
        return Ok(home.to_path_buf());
    }
    if let Some(rest) = raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\")) {
        return Ok(home.join(rest));
    }
    if raw.starts_with('~') {
        return Err("Only '~/' home-relative paths are supported".to_string());
    }
    Ok(PathBuf::from(raw))
}

#[cfg(windows)]
fn normalize_canonical_path(path: PathBuf) -> PathBuf {
    let value = path.to_string_lossy();
    if let Some(rest) = value.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = value.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path
    }
}

#[cfg(not(windows))]
fn normalize_canonical_path(path: PathBuf) -> PathBuf {
    path
}

pub(crate) fn canonicalize_existing(path: &Path, label: &str) -> Result<PathBuf, String> {
    fs::canonicalize(path)
        .map(normalize_canonical_path)
        .map_err(|error| format!("Could not canonicalize {label} {}: {error}", path.display()))
}

pub(crate) fn canonical_user_home() -> Result<PathBuf, String> {
    let home = raw_user_home()?;
    let canonical = canonicalize_existing(&home, "home directory")?;
    if !canonical.is_dir() {
        return Err(format!(
            "Home directory is not a directory: {}",
            canonical.display()
        ));
    }
    Ok(canonical)
}

#[cfg(windows)]
fn component_eq(left: Component<'_>, right: Component<'_>) -> bool {
    left.as_os_str()
        .to_string_lossy()
        .eq_ignore_ascii_case(&right.as_os_str().to_string_lossy())
}

#[cfg(not(windows))]
fn component_eq(left: Component<'_>, right: Component<'_>) -> bool {
    left == right
}

pub(crate) fn is_within(candidate: &Path, root: &Path) -> bool {
    let mut candidate_components = candidate.components();
    for root_component in root.components() {
        let Some(candidate_component) = candidate_components.next() else {
            return false;
        };
        if !component_eq(candidate_component, root_component) {
            return false;
        }
    }
    true
}

#[cfg(windows)]
fn is_unc_or_device_path(path: &Path) -> bool {
    let value = path.to_string_lossy().replace('/', "\\");
    value.starts_with(r"\\")
}

#[cfg(not(windows))]
fn is_unc_or_device_path(_path: &Path) -> bool {
    false
}

fn canonicalize_home_bound(
    raw: &str,
    canonical_home: &Path,
    label: &str,
) -> Result<PathBuf, String> {
    let candidate = expand_home(raw, canonical_home)?;
    if !candidate.is_absolute() {
        return Err(format!(
            "{label} must be an absolute path or start with '~/'."
        ));
    }
    if is_unc_or_device_path(&candidate) && !is_unc_or_device_path(canonical_home) {
        return Err(format!("{label} must be inside your home directory"));
    }
    let canonical = canonicalize_existing(&candidate, label)?;
    if !is_within(&canonical, canonical_home) {
        return Err(format!(
            "{label} must resolve inside your home directory ({})",
            canonical_home.display()
        ));
    }
    Ok(canonical)
}

fn canonical_ssh_key_with_home(raw: &str, canonical_home: &Path) -> Result<PathBuf, String> {
    let canonical = canonicalize_home_bound(raw, canonical_home, "SSH key path")?;
    if !canonical.is_file() {
        return Err(format!(
            "SSH key path is not a regular file: {}",
            canonical.display()
        ));
    }
    Ok(canonical)
}

pub(crate) fn canonical_ssh_key(raw: &str) -> Result<PathBuf, String> {
    let home = canonical_user_home()?;
    canonical_ssh_key_with_home(raw, &home)
}

fn canonical_export_target_with_home(raw: &str, canonical_home: &Path) -> Result<PathBuf, String> {
    if raw.trim().is_empty() {
        return Err("Export path is required".to_string());
    }
    let raw_path = Path::new(raw);
    if !raw_path.is_absolute() {
        return Err("Export path must be absolute".to_string());
    }
    if is_unc_or_device_path(raw_path) && !is_unc_or_device_path(canonical_home) {
        return Err("Export path must be inside your home directory".to_string());
    }

    let parent = raw_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| "Export path must have a parent directory".to_string())?;
    let canonical_parent = canonicalize_existing(parent, "export parent directory")?;
    if !canonical_parent.is_dir() {
        return Err(format!(
            "Export parent is not a directory: {}",
            canonical_parent.display()
        ));
    }
    if !is_within(&canonical_parent, canonical_home) {
        return Err(format!(
            "Export path must resolve inside your home directory ({})",
            canonical_home.display()
        ));
    }

    match fs::symlink_metadata(raw_path) {
        Ok(_) => {
            let canonical_target = canonicalize_existing(raw_path, "export target")?;
            if !is_within(&canonical_target, canonical_home) {
                return Err(format!(
                    "Export target must resolve inside your home directory ({})",
                    canonical_home.display()
                ));
            }
            if !canonical_target.is_file() {
                return Err(format!(
                    "Export target is not a regular file: {}",
                    canonical_target.display()
                ));
            }
            Ok(canonical_target)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let file_name = raw_path
                .file_name()
                .filter(|name| !name.is_empty())
                .ok_or_else(|| "Export path must include a file name".to_string())?;
            Ok(canonical_parent.join(file_name))
        }
        Err(error) => Err(format!(
            "Could not inspect export target {}: {error}",
            raw_path.display()
        )),
    }
}

pub(crate) fn canonical_export_target(raw: &str) -> Result<PathBuf, String> {
    let home = canonical_user_home()?;
    canonical_export_target_with_home(raw, &home)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use uuid::Uuid;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = env::temp_dir().join(format!("gitswitch-path-test-{}", Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn rejects_traversal_and_partial_prefixes() {
        let root = TestDirectory::new();
        let home = root.0.join("home");
        let outside = root.0.join("home-other");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let key = outside.join("id_ed25519");
        File::create(&key).unwrap();
        let canonical_home = canonicalize_existing(&home, "home").unwrap();

        assert!(canonical_ssh_key_with_home(
            home.join("..")
                .join("home-other")
                .join("id_ed25519")
                .to_str()
                .unwrap(),
            &canonical_home,
        )
        .is_err());
        assert!(!is_within(
            &canonicalize_existing(&outside, "outside").unwrap(),
            &canonical_home,
        ));
    }

    #[test]
    fn accepts_existing_key_and_canonicalizes_export_parent() {
        let root = TestDirectory::new();
        let home = root.0.join("home");
        let ssh = home.join(".ssh");
        let exports = home.join("exports");
        fs::create_dir_all(&ssh).unwrap();
        fs::create_dir_all(&exports).unwrap();
        let key = ssh.join("id_ed25519");
        File::create(&key).unwrap();
        let canonical_home = canonicalize_existing(&home, "home").unwrap();

        assert_eq!(
            canonical_ssh_key_with_home(key.to_str().unwrap(), &canonical_home).unwrap(),
            canonicalize_existing(&key, "key").unwrap()
        );
        assert_eq!(
            canonical_export_target_with_home(
                exports.join("profiles.json").to_str().unwrap(),
                &canonical_home,
            )
            .unwrap(),
            canonicalize_existing(&exports, "exports")
                .unwrap()
                .join("profiles.json")
        );
    }

    #[test]
    fn rejects_missing_export_parent_and_outside_export() {
        let root = TestDirectory::new();
        let home = root.0.join("home");
        let outside = root.0.join("outside");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let canonical_home = canonicalize_existing(&home, "home").unwrap();

        assert!(canonical_export_target_with_home(
            home.join("missing").join("profiles.json").to_str().unwrap(),
            &canonical_home,
        )
        .is_err());
        assert!(canonical_export_target_with_home(
            outside.join("profiles.json").to_str().unwrap(),
            &canonical_home,
        )
        .is_err());
    }

    #[cfg(unix)]
    #[test]
    fn resolves_symlinks_before_home_boundary_check() {
        use std::os::unix::fs::symlink;

        let root = TestDirectory::new();
        let home = root.0.join("home");
        let outside = root.0.join("outside");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&outside).unwrap();
        File::create(outside.join("id_ed25519")).unwrap();
        symlink(&outside, home.join("linked-ssh")).unwrap();
        let canonical_home = canonicalize_existing(&home, "home").unwrap();

        assert!(canonical_ssh_key_with_home(
            home.join("linked-ssh").join("id_ed25519").to_str().unwrap(),
            &canonical_home,
        )
        .is_err());
    }

    #[cfg(windows)]
    #[test]
    fn resolves_junctions_and_handles_windows_case_and_unc_boundaries() {
        let root = TestDirectory::new();
        let home = root.0.join("Home");
        let outside = root.0.join("Outside");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&outside).unwrap();
        File::create(outside.join("id_ed25519")).unwrap();
        junction::create(&outside, home.join("linked-ssh")).unwrap();
        let canonical_home = canonicalize_existing(&home, "home").unwrap();

        assert!(canonical_ssh_key_with_home(
            home.join("linked-ssh").join("id_ed25519").to_str().unwrap(),
            &canonical_home,
        )
        .is_err());
        assert!(is_within(
            &PathBuf::from(r"C:\Users\ALICE\.ssh\id_ed25519"),
            &PathBuf::from(r"c:/users/alice"),
        ));
        assert!(!is_within(
            &PathBuf::from(r"\\server\other\id_ed25519"),
            &PathBuf::from(r"\\server\home\alice"),
        ));
    }
}
