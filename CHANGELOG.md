# Changelog

All notable changes to this project will be documented in this file.

## [0.1.0] - 2026-03-11

- Initial release: Windows installers (MSI, EXE)
- Desktop app using Tauri with production frontend build

## [0.1.1] - 2026-03-16

- Fix: directory-rule auto-switch now writes local repository config (`.git/config`) so rules override global config only for matched repos.
- Fix: improved Detect handling for unset git config values to avoid misleading "Git command failed" errors.
- UX: added explicit hint in Directory Rules explaining local config behavior.
