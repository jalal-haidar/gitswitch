# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/).

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
