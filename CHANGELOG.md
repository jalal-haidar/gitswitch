# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.2.2] - 2026-03-18

### Added

- **Multi-repo scanner**: pick a root folder to discover all git repos inside it recursively; displays detected name/email, matched profile badge, and remote service (GitHub/GitLab/Bitbucket) per repo with a one-click Apply button.
- **Duplicate Profile**: copy any profile to a new one pre-filled with the same details via the "Duplicate" button on each profile card.
- **Apply to Repo**: manually apply a profile to a specific repository folder without waiting for an auto-switch event, via the "Apply to Repo" button on each profile card.
- **Light / Dark / System theme**: theme selector in Settings with full CSS variable implementation; persisted to config; applied on startup.
- **Per-rule last-triggered timestamp**: each directory rule card now shows the last time it auto-switched, stamped in Rust after every successful auto-switch.
- **Remote URL service badge**: profile cards and scanned repo rows show GitHub / GitLab / Bitbucket / Other badge derived from `remote.origin.url`.
- **Remote URL detection**: `detect_identities` now also captures `remote.origin.url` and classifies the hosting service.
- **Version display**: current app version shown in the Dashboard footer.

### Changed

- Page title corrected from Tauri template default to **GitSwitch**.
- SSH key paths are now restricted to the user home directory (path traversal hardening).
- Leading `~` in SSH key paths is now expanded to the home directory.
- Export `version = 0` is now rejected on import (previously only `version > 1` was checked).
- Auto-switch watcher thread now emits an `auto-switch-error` event when it dies; the frontend shows a persistent error toast.
- OS keyring write failures now emit a `keyring-warning` event instead of being silently discarded; the frontend shows a warning toast.

### Fixed

- Rule path existence is preserved correctly when updating a directory rule (was being clobbered).

## [0.2.1] - 2026-03-17

### Added

- Export/Import profiles as JSON for easy backup and restore across machines.
- Profile search/filter box with real-time search across label, name, and email fields.
- Run at startup toggle in Settings for automatic app launch at system login.
- Per-profile colored tray icon that dynamically changes to match the active profile's color.
- Keyboard shortcuts:
  - `Esc` — Close editor, cancel profile editing, or close settings modal
  - `Ctrl/Cmd+N` — Open new profile form
  - `Ctrl/Cmd+F` — Focus search box
  - `Ctrl/Cmd+,` — Open settings modal
- Profile count badge in dashboard header.
- Active profile name now shown in window title.

### Changed

- System tray icon now displays a colored dot matching the active profile's color.
- App now hides to system tray when closed (instead of exiting). Use "Quit" from tray menu to exit completely.
- Form editor now uses stable key props to prevent unwanted resets while typing.

## [0.2.0] - 2026-03-17

### Changed

- UI: add spacing between Settings and Detect buttons in the toolbar.

### Fixed

- CI: fix `rust-tests` job by installing required Linux system libraries (`libgtk-3-dev`, `libwebkit2gtk-4.1-dev`, etc.) before running `cargo test`.

## [0.1.1] - 2026-03-16

### Fixed

- Directory-rule auto-switch now writes local repository config (`.git/config`) so rules override global config only for matched repos.
- Improved Detect handling for unset git config values to avoid misleading "Git command failed" errors.

### Changed

- Added explicit hint in Directory Rules explaining local config behavior.

## [0.1.0] - 2026-03-11

### Added

- Initial release: Windows installers (MSI, EXE).
- Desktop app using Tauri 2 with React + TypeScript frontend.
- Profile management: create, edit, delete Git profiles.
- One-click global identity switch.
- Directory rules: per-directory auto-switch.
- In-app auto-updater via Tauri updater plugin.
