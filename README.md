# GitSwitch 🚀

**GitSwitch** is a cross-platform desktop application designed to make managing multiple Git identities effortless. No more accidental commits with the wrong email or wrestling with SSH configs.

[![Release](https://img.shields.io/github/v/release/jalal-haidar/gitswitch)](https://github.com/jalal-haidar/gitswitch/releases/latest) [![Downloads](https://img.shields.io/github/downloads/jalal-haidar/gitswitch/total)](https://github.com/jalal-haidar/gitswitch/releases)

## ✨ Features

- **Profile Management:** Create and manage multiple Git profiles (Name, Email, SSH keys).
- **One-Click Switch:** Change your global Git identity with a single click.
- **Tauri-Powered:** Lightweight, fast, and secure desktop experience.
- **Directory Rules (WIP):** Automatically switch profiles based on the directory you are working in.

## Recent updates (Mar 16, 2026)

- **Error normalization & UX fixes:** Backend errors are now parsed and normalized so the UI shows friendly messages (no raw JSON blobs).
- **Active profile persisted:** The app now stores and exposes an `active_profile_id` so the UI accurately reflects the globally active Git identity.
- **Inline profile editor:** Profiles can be created and edited inline via the new `ProfileEditor` component.
- **Directory rules CRUD implemented:** Full create/read/update/delete support for directory rules is available (backend commands + frontend UI). Advanced automation (auto-switch watcher, glob generation) is intentionally deferred to a follow-up phase.

## 🛠️ Tech Stack

- **Backend:** Rust + Tauri 2.0
- **Frontend:** React + TypeScript + Vite
- **State Management:** Zustand
- **Styling:** CSS Modules / Vanilla CSS

## 📂 Project Structure

- `src-tauri/`: Rust backend, handling system-level Git configuration and persistence.
- `src/`: React frontend, providing a premium UI for identity management.
- `docs/`: Technical documentation and architecture overviews.

## 🚀 Getting Started

1.  **Install Dependencies:**
    ```bash
    npm install
    ```
2.  **Run in Development:**
    ```bash
    npm run tauri dev
    ```
3.  **Build for Production:**
    ```bash
    npm run tauri build
    ```

## 🏗️ Implementation Status

- [x] Backend Persistence (JSON Storage)
- [x] Rust Models & Commands
- [x] Frontend Store (Zustand)
- [/] UI Components (Current Focus)
- [ ] SSH Key Management
- [ ] System Tray Integration

## 📦 Releases

Binaries, installers, checksums, and a portable ZIP for the latest release are published on the GitHub Releases page.

- **Latest release:** v0.1.0
- Download (Windows EXE): https://github.com/jalal-haidar/gitswitch/releases/download/v0.1.0/gitswitch_0.1.0_x64-setup.exe
- Download (Windows MSI): https://github.com/jalal-haidar/gitswitch/releases/download/v0.1.0/gitswitch_0.1.0_x64_en-US.msi
- Portable ZIP (contains installers + checksums): https://github.com/jalal-haidar/gitswitch/releases/download/v0.1.0/gitswitch_0.1.0.zip
- SHA256 checksums:
  - https://github.com/jalal-haidar/gitswitch/releases/download/v0.1.0/gitswitch_0.1.0_x64-setup.exe.sha256
  - https://github.com/jalal-haidar/gitswitch/releases/download/v0.1.0/gitswitch_0.1.0_x64_en-US.msi.sha256

Verification

- PowerShell (Windows):
  ```powershell
  Get-FileHash -Algorithm SHA256 <path-to-file>
  ```
- macOS / Linux:
  ```bash
  shasum -a 256 <file>
  ```

Signing

- These installers may be unsigned. Signing verifies publisher identity and reduces SmartScreen/AV warnings.
- To provide signed installers, obtain a code-signing certificate and sign in CI or locally (e.g., `signtool.exe` on Windows). If you want, I can help add signing to the CI workflow once you have a certificate.

Visit the releases page for archives and older artifacts:

https://github.com/jalal-haidar/gitswitch/releases

## 🪟 Windows Installation

Choose one of the installers from the releases page and follow these steps.

- GUI Installer (recommended): double-click the `gitswitch_0.1.0_x64-setup.exe` and follow the installer prompts.
- MSI Installer (silent install): open an elevated PowerShell and run:
  ```powershell
  Start-Process msiexec -Wait -ArgumentList '/i','"<path-to>\\gitswitch_0.1.0_x64_en-US.msi"','/qn'
  ```
- Verify checksum (PowerShell):
  ```powershell
  Get-FileHash -Algorithm SHA256 <path-to-file>
  ```
- Verify signature (if signed):
  ```powershell
  signtool verify /pa <path-to-file>
  ```

Notes:

- Installing may require administrator privileges.
- If installers are unsigned, Windows SmartScreen may warn; signed installers reduce such warnings.

## ❌ Uninstalling GitSwitch (Windows)

Remove GitSwitch using the GUI or silently for automated workflows.

- GUI (recommended): open **Settings → Apps → Apps & features**, find "GitSwitch", and choose **Uninstall**. You can also use Control Panel → Programs and Features.
- MSI silent uninstall (admin):
  ```powershell
  Start-Process msiexec -Wait -ArgumentList '/x','"<path-to>\\gitswitch_0.1.0_x64_en-US.msi"','/qn'
  ```
- EXE installer uninstall: run the uninstaller from the Start Menu or the installation directory (for example `C:\Program Files\GitSwitch\uninstall.exe`) if present.

To fully remove user data, delete the application's data directory (check app settings or docs for the exact path). Be careful: this will remove saved profiles and settings.
