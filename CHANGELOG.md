# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

- Directory rules: folders can map to a profile via `includeIf "gitdir:"` in the global gitconfig
- SSH: generate ed25519 keys, attach existing keys, write `~/.ssh/config` host aliases, test connections
- Profile create/edit form (replaces the demo "New Profile" button)
- All backend commands return structured `BackendError`s; the UI shows them as readable toasts
- Fix: applying a detected identity ignored its GPG key (argument name mismatch)
- Fix: `npm install` failed on React 19 (`@testing-library/react` upgraded to v16)
- CI: type-check, install Tauri's Linux libraries for Rust tests

## [0.1.0] - 2026-03-11

- Initial release: Windows installers (MSI, EXE)
- Desktop app using Tauri with production frontend build
