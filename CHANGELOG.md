# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/).

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
