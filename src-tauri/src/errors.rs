use serde::Serialize;
use std::fmt;

#[derive(Debug, Serialize)]
pub enum BackendErrorKind {
    GitNotFound,
    PermissionDenied,
    GitFailed,
    IoError,
    NotFound,
    InvalidInput,
    SshFailed,
    Unknown,
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

    pub fn not_found(msg: impl Into<String>) -> Self {
        BackendError::new(BackendErrorKind::NotFound, msg)
    }

    pub fn invalid_input(msg: impl Into<String>) -> Self {
        BackendError::new(BackendErrorKind::InvalidInput, msg)
    }

    pub fn ssh_failed(msg: impl Into<String>) -> Self {
        BackendError::new(BackendErrorKind::SshFailed, "SSH command failed").with_details(msg)
    }
}

impl From<std::io::Error> for BackendError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::PermissionDenied => BackendError::permission_denied(e.to_string()),
            std::io::ErrorKind::NotFound => BackendError::not_found(e.to_string()),
            _ => BackendError::io_error(e.to_string()),
        }
    }
}

impl From<anyhow::Error> for BackendError {
    fn from(e: anyhow::Error) -> Self {
        // Preserve a permission-denied cause from the underlying io::Error, if any.
        let denied = e.chain().any(|c| {
            c.downcast_ref::<std::io::Error>()
                .is_some_and(|io| io.kind() == std::io::ErrorKind::PermissionDenied)
        });
        let msg = format!("{:#}", e);
        if denied {
            BackendError::permission_denied(msg)
        } else {
            BackendError::io_error(msg)
        }
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
        // should include kind and hint
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
    fn io_error_conversion_maps_kinds() {
        let denied: BackendError =
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "nope").into();
        assert!(matches!(denied.kind, BackendErrorKind::PermissionDenied));
        let missing: BackendError =
            std::io::Error::new(std::io::ErrorKind::NotFound, "gone").into();
        assert!(matches!(missing.kind, BackendErrorKind::NotFound));
    }

    #[test]
    fn anyhow_conversion_keeps_permission_cause() {
        let io = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "nope");
        let e: BackendError = anyhow::Error::new(io).context("writing config").into();
        assert!(matches!(e.kind, BackendErrorKind::PermissionDenied));
        assert!(e.hint.is_some());
    }
}
