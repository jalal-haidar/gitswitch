use serde::Serialize;
use std::fmt;

#[derive(Debug, Serialize)]
pub enum BackendErrorKind {
    GitNotFound,
    PermissionDenied,
    GitFailed,
    GitTransactionFailed,
    IoError,
    SecureStorageError,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendError {
    pub kind: BackendErrorKind,
    pub message: String,
    pub hint: Option<String>,
    pub details: Option<String>,
    pub operation_failure: Option<String>,
    pub rollback_failure: Option<String>,
}

impl BackendError {
    pub fn new(kind: BackendErrorKind, message: impl Into<String>) -> Self {
        BackendError {
            kind,
            message: message.into(),
            hint: None,
            details: None,
            operation_failure: None,
            rollback_failure: None,
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

    pub fn git_transaction(
        operation: &str,
        operation_failure: impl Into<String>,
        rollback_failure: Option<String>,
    ) -> Self {
        let operation_failure = summarize_failure(&operation_failure.into());
        let rollback_failure = rollback_failure.map(|failure| summarize_failure(&failure));
        let rollback_incomplete = rollback_failure.is_some();
        BackendError {
            kind: BackendErrorKind::GitTransactionFailed,
            message: format!("Git configuration {operation} failed"),
            hint: Some(if rollback_incomplete {
                "Automatic rollback was incomplete. Review the rollback failure before retrying."
                    .to_string()
            } else {
                "The original Git configuration was restored. Fix the reported problem and try again."
                    .to_string()
            }),
            details: Some(operation_failure.clone()),
            operation_failure: Some(operation_failure),
            rollback_failure,
        }
    }

    pub fn io_error(msg: impl Into<String>) -> Self {
        BackendError::new(BackendErrorKind::IoError, msg)
    }

    pub fn secure_storage(operation: &str, account: &str, details: impl Into<String>) -> Self {
        BackendError::new(
            BackendErrorKind::SecureStorageError,
            format!("OS secure storage {operation} failed"),
        )
        .with_hint("Your previous profile settings were not changed. Unlock or repair the OS credential store, then try again.")
        .with_details(format!("Credential account '{account}': {}", details.into()))
    }

    pub fn secure_storage_missing(account: &str) -> Self {
        BackendError::new(
            BackendErrorKind::SecureStorageError,
            "An expected profile value is missing from OS secure storage",
        )
        .with_hint("The profile was not modified. Re-enter the missing SSH/GPG value or disable secure storage after access is restored.")
        .with_details(format!("Missing credential account '{account}'"))
    }
}

pub(crate) fn summarize_failure(error: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(error) else {
        return error.to_string();
    };
    ["operationFailure", "details", "message"]
        .into_iter()
        .find_map(|field| value.get(field).and_then(serde_json::Value::as_str))
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| error.to_string())
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
    fn secure_storage_error_is_structured_and_never_needs_a_secret() {
        let e = BackendError::secure_storage(
            "write",
            "profile-1:ssh_key_path",
            "credential store locked",
        );
        let s = e.to_string();
        assert!(s.contains("SecureStorageError"));
        assert!(s.contains("previous profile settings were not changed"));
        assert!(s.contains("profile-1:ssh_key_path"));
        assert!(!s.contains("secret-value"));
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
        let parsed: serde_json::Value =
            serde_json::from_str(&s).expect("Display output should be valid JSON");
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

    #[test]
    fn git_transaction_reports_operation_and_rollback_separately() {
        let error = BackendError::git_transaction(
            "apply",
            "commit.gpgsign write failed",
            Some("user.name rollback failed".to_string()),
        );
        let value: serde_json::Value = serde_json::from_str(&error.to_string()).unwrap();
        assert_eq!(value["operationFailure"], "commit.gpgsign write failed");
        assert_eq!(value["rollbackFailure"], "user.name rollback failed");
    }

    #[test]
    fn git_transaction_flattens_nested_backend_errors() {
        let nested = BackendError::git_failed("fatal: could not lock config").to_string();
        let error = BackendError::git_transaction("apply", nested, None);
        assert_eq!(
            error.operation_failure.as_deref(),
            Some("fatal: could not lock config")
        );
    }
}
