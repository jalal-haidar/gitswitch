use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Writes via a temp file in the same directory, then renames over the target.
pub fn write_atomic(path: &Path, contents: &str) -> io::Result<()> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = path.with_file_name(format!("{}.gitswitch-tmp", name));
    fs::write(&tmp, contents)?;
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

/// Copies `path` to `<path>.gitswitch.bak` once, if the file exists and no backup is there yet.
pub fn backup_once(path: &Path) -> io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let mut bak = path.as_os_str().to_owned();
    bak.push(".gitswitch.bak");
    let bak = PathBuf::from(bak);
    if !bak.exists() {
        fs::copy(path, &bak)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_atomic_replaces_contents_and_backs_up_once() {
        let dir = std::env::temp_dir().join(format!("gs-fsutil-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.txt");
        write_atomic(&f, "one").unwrap();
        write_atomic(&f, "two").unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap(), "two");
        backup_once(&f).unwrap();
        write_atomic(&f, "three").unwrap();
        backup_once(&f).unwrap();
        let bak = dir.join("a.txt.gitswitch.bak");
        assert_eq!(fs::read_to_string(bak).unwrap(), "two");
        fs::remove_dir_all(&dir).ok();
    }
}
