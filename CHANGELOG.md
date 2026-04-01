# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.2.6] - 2026-04-01

### Added

- Expanded automated coverage across frontend and backend, including new tests for `ErrorBoundary`, keyboard shortcuts, layout/skeleton UI, toast state, profile editor mapping, tray hex-colour parsing, and email validation edge cases.
- Added regression tests for multi-byte UTF-8 and emoji colour inputs to ensure tray colour parsing never panics on malformed or hand-edited profile values.

### Changed

- Standardized frontend typing further by introducing a dedicated `GitConfigSnapshot` type alias and removing the last `invoke<any>` call.
- Memoized `ProfileCard` with `React.memo` and aligned the `KeyboardShortcutsHelp` component name with its filename for cleaner component identity and render behavior.
- Cleaned up remaining Rust lint issues so `cargo clippy` runs without warnings.

### Fixed

- Restored the release version from the accidental `0.3.0` bump to the intended patch release `0.2.6` across app manifests.
- Hardened tray icon colour parsing to reject non-hex and non-ASCII input safely instead of risking byte-slice panics.
- Enabled the repo scan home-directory safety guard on Windows as well as other platforms.
- Fixed global Git config undo so `core.sshCommand` is restored along with name, email, signing key, and `commit.gpgsign`.
- Made profile export writes more durable by flushing data to disk with `sync_all()` before returning success.
- Improved backend error messages by including missing entity IDs in profile and directory-rule lookup failures.

## [0.2.5] - 2026-03-31

### Added

- **Rule proof panel**: clicking "Test Rule" on any directory rule applies the rule's profile to a chosen repo folder and immediately reads back the repo's `.git/config`, showing a per-field ✓/✗ comparison (name, email, signing key, SSH command) as hard proof the switch took effect.
- **`get_repo_local_config` backend command**: reads `user.name`, `user.email`, `user.signingkey`, and `core.sshCommand` directly from a repo's `.git/config` — no inference, ground-truth proof.
- **`auto-switch-failed` event**: the backend now emits a structured failure event (with rule path and reason) whenever an automatic profile switch fails; the Directory Rules panel shows a normalized error toast with optional hint text.
- **Scan cap warning**: when a repo scan returns exactly 200 results (the backend hard cap), an inline warning is shown prompting the user to narrow the root folder.
- **`has_repo_snapshot` / `restore_repo_snapshot` commands**: expose transient snapshot existence and restore to the frontend for the scan panel's Restore button.

### Fixed

- **Apply / Restore invoke key mismatch**: frontend was sending `{ profileId, repoPath }` but the Rust commands expect `{ id, repo_path }` — Apply and Restore always failed with a missing-key error. Fixed across `Dashboard.tsx`, `ProfileCard.tsx`, and `useProfileStore.ts`.
- **`scanRepos` depth ignored**: `maxDepth` was sent camelCase; Rust expected `max_depth` — explicit depth was silently ignored, always defaulting to 5. Fixed.
- **Auto-switch skip guard too broad**: the per-repo switch was skipped whenever the global `active_profile_id` matched, regardless of what was actually in the repo's local `.git/config`. Guard now compares the actual local `user.name`, `user.email`, and `core.sshCommand` against the profile's expected values.
- **Auto-switch path matching case-insensitive on Windows**: `PathBuf::starts_with` is case-sensitive; rule paths with different casing from the notify event path would never fire. A `path_starts_with_ci` helper now lowercases both sides on Windows.
- **UNC path prefix (`\\?\`) not stripped**: `fs::canonicalize` on Windows returns `\\?\`-prefixed extended paths; the previous strip used the wrong byte sequence so the prefix leaked through and broke all watched-path comparisons. Fixed to strip the correct 4-char prefix.
- **Path boundary off-by-one**: string-level `starts_with` caused `C:\work` to match events in `C:\work2`. The boundary check now requires the remainder to be empty or start with a path separator.
- **Wrong git root passed to auto-switch**: when a rule watches a parent directory (e.g. `C:\projects`), the switch was called against the rule root rather than the actual `.git` root of the file that triggered the event. The event path is now walked upward to find the real git root.
- **Snapshot overwritten on rapid saves**: each file-save event overwrote the transient snapshot, losing the pre-burst baseline. Snapshot is now captured only once per repo (skipped if one already exists).
- **Rule path not validated as directory**: `add_directory_rule` and `update_directory_rule` checked `exists()` but not `is_dir()` — a file path passed manually bypassed the folder picker. Both commands now reject non-directory paths.
- **Auto-switch watcher no restart on channel disconnect**: if the event channel dropped, the watcher thread exited permanently. The watcher loop now restarts with exponential backoff (1 s → 2 s → … capped at 30 s).
- **Proof panel SSH check presence-only**: the SSH row showed ✓ for any profile with an SSH key, regardless of whether the _correct_ key was applied to the repo. Now compares the exact `core.sshCommand` string.
- **Active profile indicator stale after auto-switch**: the Dashboard's active-profile highlight was not refreshed when `auto-switch-triggered` fired. The listener now also calls `fetchProfiles()`.
- **`last_triggered_at` save failure silently swallowed**: failures were discarded with `let _`; they now log to stderr for visibility in the Tauri dev console.

### Changed

- **Auto-switch debounce raised from 500 ms to 1500 ms**: IDE "Save All" and `cargo build` produce event bursts longer than 500 ms; the shorter window caused repeated redundant git-config writes.
- **Longest-prefix best-match uses path component depth**: previously used `OsStr::len()` (byte count), which gave wrong priority for Unicode path segments on Windows. Now uses `components().count()`.
- **`auto-switch-failed` toasts normalized**: raw backend error strings are now passed through `normalizeBackendError`, giving the user a clean message and optional hint; toast duration extends to 10 s when a hint is present.

## [0.2.4] - 2026-03-18

### Added

- **Section dividers**: visual separators between the Your Profiles, Repo Scanner, and Directory Rules sections for clearer layout.
- **Repo Scanner search filter**: filter the scan results list in real time by repo name, path, or detected identity.
- **Repo Scanner pagination**: scan results are now paginated (20 repos per page) with Prev / Next controls, preventing UI freeze when scanning large directories.
- **Custom styled checkboxes**: all checkboxes app-wide now use a consistent themed design (accent-colour fill, custom check mark, focus ring) replacing the native OS style.
- **Custom styled select dropdowns**: all `<select>` elements use a unified dark-themed appearance with a custom chevron arrow, hover glow, and focus ring.
- **Directory path input auto-focus**: when "Add your first rule" (or Add Rule) is clicked, the Directory Path input is focused automatically.
- **Improved placeholder text** on the Directory Path input: `"Paste a path or click Browse…"` instead of the previous generic example.

### Changed

- **Detected Identities section** is now hidden by default and only appears after the user clicks the **Detect** button in the header, reducing visual clutter on first load.
- **Settings section order** updated to: Theme → Startup → Security → Updates → Profiles Backup (theme and common preferences promoted to the top).

## [0.2.3] - 2026-03-18

### Added

- **Welcome onboarding screen**: when no profiles exist, the dashboard now shows a structured 4-step guide covering profile creation, switching, directory auto-switch rules, and SSH key setup — with a direct "Create your first profile" CTA button.
- **Help tooltips on SSH and GPG fields**: `?` icons on the SSH Key Path and GPG Key ID fields in the profile editor explain what each field does, how to generate the required key, and where to register it (GitHub Settings), visible on hover or keyboard focus.

### Fixed

- SSH connection test now correctly recognises GitHub's success response (`"You've successfully authenticated"`) — previously it only matched the older wording and always showed a failure indicator even when the key worked.

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
