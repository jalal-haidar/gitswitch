use std::process::Command;

/// Suppress the CMD console window flicker on Windows when spawning child processes.
/// No-op on non-Windows platforms.
#[cfg(windows)]
pub fn no_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
}

#[cfg(not(windows))]
pub fn no_window(_cmd: &mut Command) {}
