# GitSwitch 🚀

**GitSwitch** is a cross-platform desktop application designed to make managing multiple Git identities effortless. No more accidental commits with the wrong email or wrestling with SSH configs.

## ✨ Features

- **Profile Management:** Create and manage multiple Git profiles (Name, Email, SSH keys).
- **One-Click Switch:** Change your global Git identity with a single click.
- **Tauri-Powered:** Lightweight, fast, and secure desktop experience.
- **Directory Rules (WIP):** Automatically switch profiles based on the directory you are working in.

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
