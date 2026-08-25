# Security Policy

## Supported Versions

| Version        | Supported |
| -------------- | --------- |
| Latest release | ✅        |
| Older releases | ❌        |

We only support the latest released version. Please upgrade before reporting a security issue.

## Reporting a Vulnerability

**Do not open a public GitHub issue for security vulnerabilities.**

Please report security issues by emailing the maintainer directly or using [GitHub's private vulnerability reporting](https://github.com/jalal-haidar/gitswitch/security/advisories/new).

Include:

- A description of the vulnerability
- Steps to reproduce
- Potential impact
- Any suggested mitigations

You can expect an acknowledgement within **72 hours** and a resolution or status update within **14 days**.

## Desktop Threat Model

GitSwitch trusts its locally bundled frontend, native Rust code, the current OS user session, and signed application updates. Data crossing into native commands is not trusted merely because it came from the UI. This includes imported profile JSON, repository contents, file and directory paths, SSH output, and all Tauri command arguments.

Native path boundaries are enforced after resolving traversal, symlinks, and Windows junctions. SSH keys and profile exports must resolve beneath the current user's canonical home directory; repositories may live elsewhere but are canonicalized before Git or snapshot operations. SSH connection tests use the user's existing OpenSSH `known_hosts` file and never accept a new or changed host key automatically.

The production webview loads only bundled content under a restrictive Content Security Policy. Its Tauri capability exposes only the app, event, window, dialog, updater, and narrowly scoped URL-opening commands used by the main window.

## Accepted Residual Risks

- A process already running as the same OS user may replace a validated path between validation and use, modify Git/OpenSSH configuration, or replace executables found through `PATH`.
- Users remain responsible for verifying SSH host fingerprints before updating `known_hosts`.
- The CSP retains inline style support for dynamic React presentation, but does not permit inline scripts or remote application content.
- CSP and Tauri capabilities do not protect against a compromised OS, system webview, native dependency, or signed application build.
