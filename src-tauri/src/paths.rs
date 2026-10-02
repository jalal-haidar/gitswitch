use std::env;
use std::path::PathBuf;

pub fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Global gitconfig: `GIT_CONFIG_GLOBAL` if set, else `~/.gitconfig`.
pub fn global_gitconfig() -> Option<PathBuf> {
    env::var_os("GIT_CONFIG_GLOBAL")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|h| h.join(".gitconfig")))
}
