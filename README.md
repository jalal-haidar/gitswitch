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
