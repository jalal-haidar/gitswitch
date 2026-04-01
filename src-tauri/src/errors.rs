use serde::Serialize;
use std::fmt;

#[derive(Debug, Serialize)]
pub enum BackendErrorKind {
    GitNotFound,
    PermissionDenied,
    GitFailed,
    IoError,
}

#[derive(Debug, Serialize)]
pub struct BackendError {
    pub kind: BackendErrorKind,
    pub message: String,
    pub hint: Option<String>,
    pub details: Option<String>,
}

impl BackendError {
    pub fn new(kind: BackendErrorKind, message: impl Into<String>) -> Self {
        BackendError {
            kind,
            message: message.into(),
            hint: None,
            details: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    pub fn git_not_found() -> Self {
        BackendError::new(
            BackendErrorKind::GitNotFound,
            "Git executable not found on PATH",
        )
        .with_hint("Install Git from https://git-scm.com/downloads")
    }

    pub fn permission_denied(msg: impl Into<String>) -> Self {
        BackendError::new(BackendErrorKind::PermissionDenied, msg)
            .with_hint("Permission denied — try running the app with elevated permissions or adjust file permissions")
    }

    pub fn git_failed(msg: impl Into<String>) -> Self {
        BackendError::new(BackendErrorKind::GitFailed, "Git command failed").with_details(msg)
    }

    pub fn io_error(msg: impl Into<String>) -> Self {
        BackendError::new(BackendErrorKind::IoError, msg)
    }
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Serialize to JSON so frontend can parse structured error, fallback to message
        match serde_json::to_string(self) {
            Ok(s) => write!(f, "{}", s),
            Err(_) => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for BackendError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_error_git_not_found_serializes() {
        let e = BackendError::git_not_found();
        let s = e.to_string();
        assert!(s.contains("GitNotFound") || s.contains("Git executable not found"));
        assert!(s.contains("git-scm.com") || s.contains("Install Git"));
    }

    #[test]
    fn backend_error_permission_has_hint() {
        let e = BackendError::permission_denied("access denied to file");
        let s = e.to_string();
        assert!(s.contains("PermissionDenied") || s.contains("Permission denied"));
        assert!(s.contains("elevated") || s.contains("permissions"));
    }

    #[test]
    fn git_failed_includes_details() {
        let e = BackendError::git_failed("fatal: not a git repository");
        let s = e.to_string();
        assert!(s.contains("GitFailed"));
        assert!(s.contains("fatal: not a git repository"));
    }

    #[test]
    fn io_error_includes_message() {
        let e = BackendError::io_error("file not found");
        let s = e.to_string();
        assert!(s.contains("IoError"));
        assert!(s.contains("file not found"));
    }

    #[test]
    fn with_hint_sets_hint() {
        let e = BackendError::new(BackendErrorKind::IoError, "something broke")
            .with_hint("try again later");
        assert_eq!(e.hint.as_deref(), Some("try again later"));
    }

    #[test]
    fn with_details_sets_details() {
        let e = BackendError::new(BackendErrorKind::GitFailed, "git error")
            .with_details("stderr output here");
        assert_eq!(e.details.as_deref(), Some("stderr output here"));
    }

    #[test]
    fn display_produces_valid_json() {
        let e = BackendError::git_not_found();
        let s = e.to_string();
        let parsed: serde_json::Value = serde_json::from_str(&s)
            .expect("Display output should be valid JSON");
        assert!(parsed.get("kind").is_some());
        assert!(parsed.get("message").is_some());
        assert!(parsed.get("hint").is_some());
    }

    #[test]
    fn chained_builders() {
        let e = BackendError::new(BackendErrorKind::IoError, "base message")
            .with_hint("some hint")
            .with_details("some details");
        assert_eq!(e.message, "base message");
        assert_eq!(e.hint.as_deref(), Some("some hint"));
        assert_eq!(e.details.as_deref(), Some("some details"));
    }
}
