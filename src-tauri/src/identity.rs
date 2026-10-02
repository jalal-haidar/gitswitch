//! Directory rules: per-profile identity files plus a managed block of
//! `includeIf "gitdir:..."` entries in the global gitconfig.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::errors::BackendError;
use crate::fsutil;
use crate::managed_block;
use crate::models::{AppConfig, GitProfile};

pub const RULES_BLOCK_ID: &str = "directory-rules";
const IDENTITY_EXT: &str = "gitconfig";

/// Normalizes a user-supplied directory into the form used in `gitdir:` patterns:
/// `~` expanded, forward slashes, no Windows verbatim prefix, trailing `/`.
pub fn normalize_gitdir(path: &str, home: Option<&Path>) -> String {
    let mut p = path.trim().replace('\\', "/");
    if let Some(home) = home {
        let h = home.to_string_lossy().replace('\\', "/");
        if p == "~" {
            p = h;
        } else if let Some(rest) = p.strip_prefix("~/") {
            p = format!("{}/{}", h.trim_end_matches('/'), rest);
        }
    }
    if let Some(rest) = p.strip_prefix("//?/") {
        p = rest.to_string();
    }
    if !p.ends_with('/') {
        p.push('/');
    }
    p
}

/// Quotes a gitconfig value, escaping backslashes and double quotes.
fn quote(v: &str) -> String {
    format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""))
}

pub fn ssh_command_for_key(key_path: &str) -> String {
    format!(
        "ssh -i \"{}\" -o IdentitiesOnly=yes",
        key_path.replace('\\', "/")
    )
}

/// Contents of `<identities>/<profileId>.gitconfig`.
pub fn render_identity_file(profile: &GitProfile) -> String {
    let mut out = String::from("# Managed by GitSwitch. Changes are overwritten.\n[user]\n");
    out.push_str(&format!("\tname = {}\n", quote(&profile.name)));
    out.push_str(&format!("\temail = {}\n", quote(&profile.email)));

    let gpg = profile.gpg_key_id.as_deref().filter(|k| !k.is_empty());
    if let Some(key) = gpg {
        out.push_str(&format!("\tsigningkey = {}\n", quote(key)));
    }
    out.push_str(&format!("[commit]\n\tgpgsign = {}\n", gpg.is_some()));

    if let Some(key) = profile.ssh_key_path.as_deref().filter(|k| !k.is_empty()) {
        out.push_str(&format!(
            "[core]\n\tsshCommand = {}\n",
            quote(&ssh_command_for_key(key))
        ));
    }
    out
}

fn gitdir_keyword() -> &'static str {
    // Windows paths are case-insensitive.
    if cfg!(windows) {
        "gitdir/i"
    } else {
        "gitdir"
    }
}

/// Body of the managed block: one `includeIf` per `(gitdir, identity file path)`.
pub fn render_include_block(entries: &[(String, String)]) -> String {
    entries
        .iter()
        .map(|(dir, file)| {
            format!(
                "[includeIf \"{}:{}\"]\n\tpath = {}\n",
                gitdir_keyword(),
                dir.replace('"', "\\\""),
                quote(&file.replace('\\', "/"))
            )
        })
        .collect::<Vec<_>>()
        .join("")
}

/// Brings identity files and the gitconfig block in line with `config`.
/// Rules whose profile no longer exists are skipped.
pub fn apply_rules(
    config: &AppConfig,
    identities_dir: &Path,
    gitconfig: &Path,
) -> Result<(), BackendError> {
    let active = config.settings.auto_switch;

    let mut entries: Vec<(String, String)> = Vec::new();
    let mut wanted: BTreeSet<String> = BTreeSet::new();

    if active {
        for rule in &config.directory_rules {
            let Some(profile) = config.profiles.iter().find(|p| p.id == rule.profile_id) else {
                continue;
            };
            fs::create_dir_all(identities_dir)?;
            let file = identities_dir.join(format!("{}.{}", profile.id, IDENTITY_EXT));
            fsutil::write_atomic(&file, &render_identity_file(profile))?;
            wanted.insert(file.file_name().unwrap().to_string_lossy().into_owned());
            entries.push((rule.path.clone(), file.to_string_lossy().into_owned()));
        }
    }

    remove_stale_identity_files(identities_dir, &wanted)?;

    let current = match fs::read_to_string(gitconfig) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.into()),
    };
    let updated = if entries.is_empty() {
        managed_block::remove_block(&current, RULES_BLOCK_ID)
    } else {
        managed_block::upsert_block(&current, RULES_BLOCK_ID, &render_include_block(&entries))
    };

    if updated != current {
        fsutil::backup_once(gitconfig)?;
        fsutil::write_atomic(gitconfig, &updated)?;
    }
    Ok(())
}

fn remove_stale_identity_files(dir: &Path, wanted: &BTreeSet<String>) -> Result<(), BackendError> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(&format!(".{}", IDENTITY_EXT)) && !wanted.contains(&name) {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AppSettings, DirectoryRule};
    use std::path::PathBuf;
    use std::process::Command;

    fn profile(id: &str, name: &str, email: &str) -> GitProfile {
        GitProfile {
            id: id.into(),
            label: id.into(),
            name: name.into(),
            email: email.into(),
            color: "#000".into(),
            ..Default::default()
        }
    }

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("gs-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn normalize_handles_tilde_backslashes_and_trailing_slash() {
        let home = Path::new("C:\\Users\\me");
        assert_eq!(normalize_gitdir("~/work", Some(home)), "C:/Users/me/work/");
        assert_eq!(normalize_gitdir("D:\\code\\x\\", None), "D:/code/x/");
        assert_eq!(normalize_gitdir("\\\\?\\C:\\a", None), "C:/a/");
        assert_eq!(normalize_gitdir("/srv/a/", None), "/srv/a/");
    }

    #[test]
    fn identity_file_contents() {
        let mut p = profile("p1", "Ada \"A\" Lovelace", "ada@x.dev");
        p.gpg_key_id = Some("ABCD1234".into());
        p.ssh_key_path = Some("C:\\Users\\me\\.ssh\\work".into());
        let s = render_identity_file(&p);
        assert!(s.contains("name = \"Ada \\\"A\\\" Lovelace\""));
        assert!(s.contains("signingkey = \"ABCD1234\""));
        assert!(s.contains("gpgsign = true"));
        assert!(s.contains("sshCommand = \"ssh -i \\\"C:/Users/me/.ssh/work\\\" -o IdentitiesOnly=yes\""));

        let plain = render_identity_file(&profile("p2", "B", "b@x.dev"));
        assert!(plain.contains("gpgsign = false"));
        assert!(!plain.contains("sshCommand") && !plain.contains("signingkey"));
    }

    #[test]
    fn include_block_rendering() {
        let s = render_include_block(&[("C:/w/".into(), "C:\\cfg\\p1.gitconfig".into())]);
        assert!(s.contains(":C:/w/\"]"));
        assert!(s.contains("path = \"C:/cfg/p1.gitconfig\""));
    }

    fn config_with(rules: Vec<DirectoryRule>, profiles: Vec<GitProfile>) -> AppConfig {
        AppConfig {
            profiles,
            directory_rules: rules,
            settings: AppSettings::default(),
        }
    }

    /// Runs real git against the generated config: repos under the rule's directory
    /// must pick up the profile's identity, repos elsewhere must not.
    #[test]
    fn git_applies_identity_only_inside_ruled_directory() {
        let root = temp("rules");
        let work = fs::canonicalize({
            fs::create_dir_all(root.join("work")).unwrap();
            root.join("work")
        })
        .unwrap();
        let other = root.join("other");
        fs::create_dir_all(&other).unwrap();
        let gitconfig = root.join("gitconfig");
        fs::write(&gitconfig, "[user]\n\tname = Global\n\temail = global@x.dev\n").unwrap();

        let cfg = config_with(
            vec![DirectoryRule {
                path: normalize_gitdir(&work.to_string_lossy(), None),
                profile_id: "w".into(),
            }],
            vec![profile("w", "Work Me", "me@work.dev")],
        );
        apply_rules(&cfg, &root.join("identities"), &gitconfig).unwrap();

        let git = |dir: &Path, args: &[&str]| -> String {
            let out = Command::new("git")
                .args(args)
                .current_dir(dir)
                .env("GIT_CONFIG_GLOBAL", &gitconfig)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .output()
                .unwrap();
            assert!(out.status.success(), "{:?}", out);
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };

        let repo_in = work.join("proj");
        let repo_out = other.join("proj");
        for r in [&repo_in, &repo_out] {
            fs::create_dir_all(r).unwrap();
            git(r, &["init", "-q"]);
        }
        assert_eq!(git(&repo_in, &["config", "user.email"]), "me@work.dev");
        assert_eq!(git(&repo_in, &["config", "user.name"]), "Work Me");
        assert_eq!(git(&repo_out, &["config", "user.email"]), "global@x.dev");

        // Removing the rule restores the original file and deletes the identity file.
        apply_rules(&config_with(vec![], cfg.profiles.clone()), &root.join("identities"), &gitconfig)
            .unwrap();
        assert_eq!(
            fs::read_to_string(&gitconfig).unwrap(),
            "[user]\n\tname = Global\n\temail = global@x.dev\n"
        );
        assert!(!root.join("identities").join("w.gitconfig").exists());
        assert_eq!(git(&repo_in, &["config", "user.email"]), "global@x.dev");

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn auto_switch_off_clears_block_and_dangling_rules_are_skipped() {
        let root = temp("off");
        let gitconfig = root.join("gitconfig");
        let rules = vec![
            DirectoryRule { path: "/a/".into(), profile_id: "p".into() },
            DirectoryRule { path: "/b/".into(), profile_id: "missing".into() },
        ];
        let mut cfg = config_with(rules, vec![profile("p", "N", "n@x.dev")]);
        apply_rules(&cfg, &root.join("ids"), &gitconfig).unwrap();
        let text = fs::read_to_string(&gitconfig).unwrap();
        assert!(text.contains("/a/") && !text.contains("/b/"));

        cfg.settings.auto_switch = false;
        apply_rules(&cfg, &root.join("ids"), &gitconfig).unwrap();
        assert!(!managed_block::has_block(
            &fs::read_to_string(&gitconfig).unwrap(),
            RULES_BLOCK_ID
        ));
        fs::remove_dir_all(&root).ok();
    }
}
