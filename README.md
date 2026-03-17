<div align="center">

<img src="src/assets/logo.png" alt="GitSwitch logo" width="80" height="80" />

# GitSwitch

**Manage multiple Git identities from a beautiful desktop app. No more wrong-email commits.**

[![Latest Release](https://img.shields.io/github/v/release/jalal-haidar/gitswitch?style=flat-square&logo=github)](https://github.com/jalal-haidar/gitswitch/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/jalal-haidar/gitswitch/total?style=flat-square)](https://github.com/jalal-haidar/gitswitch/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/jalal-haidar/gitswitch/ci.yml?branch=main&style=flat-square&label=CI)](https://github.com/jalal-haidar/gitswitch/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
[![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri%202-purple?style=flat-square&logo=tauri)](https://tauri.app)

[**Download**](https://github.com/jalal-haidar/gitswitch/releases/latest) · [**Report a Bug**](https://github.com/jalal-haidar/gitswitch/issues/new?template=bug_report.yml) · [**Request a Feature**](https://github.com/jalal-haidar/gitswitch/issues/new?template=feature_request.yml) · [**Changelog**](CHANGELOG.md)

</div>

---

## What is GitSwitch?

GitSwitch is a lightweight desktop app built with **Tauri + React** that lets you create named Git profiles (name, email, SSH key, GPG key) and switch between them with a single click — globally or per-directory automatically.

---

## ✨ Features

| Feature | Status |
|---|---|
| Create & manage multiple Git profiles | ✅ |
| One-click global identity switch | ✅ |
| Per-directory auto-switch rules | ✅ |
| Detect existing identities from git history | ✅ |
| In-app auto-updater | ✅ |
| Store SSH/GPG paths in OS keyring | ✅ |
| System tray integration | 🔜 |
| macOS / Linux support | 🔜 |

---

## 📦 Installation

### Windows (recommended)

Download the latest installer from the [Releases page](https://github.com/jalal-haidar/gitswitch/releases/latest):

| Installer | Use when |
|---|---|
| `gitswitch_x.x.x_x64-setup.exe` | Typical end-user install |
| `gitswitch_x.x.x_x64_en-US.msi` | Enterprise / silent deployment |

> **Note:** Installers are currently unsigned. Windows SmartScreen may show a warning — click **More info → Run anyway**. A code-signing certificate is on the roadmap.

#### Silent MSI install

```powershell
Start-Process msiexec -Wait -ArgumentList '/i', '"<path>\gitswitch_x.x.x_x64_en-US.msi"', '/qn'
```

#### Uninstall

**Settings → Apps → Apps & features → GitSwitch → Uninstall**, or:

```powershell
Start-Process msiexec -Wait -ArgumentList '/x', '"<path>\gitswitch_x.x.x_x64_en-US.msi"', '/qn'
```

---

## 🛠️ Tech Stack

| Layer | Technology |
|---|---|
| Desktop runtime | [Tauri 2](https://tauri.app) |
| Backend | Rust |
| Frontend | React 19 + TypeScript + Vite |
| State | Zustand |
| Styling | Vanilla CSS (glassmorphism) |

---

## 🚀 Building from Source

### Prerequisites

- [Node.js](https://nodejs.org) ≥ 18
- [Rust](https://rustup.rs) (stable)

### Run locally

```bash
git clone https://github.com/jalal-haidar/gitswitch.git
cd gitswitch
npm install
npm run tauri dev
```

### Build release binary

```bash
npm run tauri build
```

Installers are output to `src-tauri/target/release/bundle/`.

---

## 🧪 Running Tests

```bash
# Frontend
npm run test:unit -- --run

# Rust
cd src-tauri
cargo test --workspace
```

---

## 📂 Project Structure

```
src/                        React frontend
  components/               UI components (Dashboard, ProfileCard, etc.)
  stores/                   Zustand state
  styles/                   Global CSS
src-tauri/
  src/                      Rust backend
    commands/               Tauri commands (profiles, rules, detect)
    config/                 Persistence layer
    models.rs               Shared data models
  tauri.conf.json           App + updater configuration
.github/
  workflows/
    ci.yml                  Lint + test on every push
    release.yml             Build & publish GitHub releases
docs/                       Architecture and setup docs
```

---

## 🔄 Auto-Updates

GitSwitch ships with Tauri's built-in updater. When a new release is published, the app notifies users and installs the update automatically.

- Updater endpoint: `https://github.com/jalal-haidar/gitswitch/releases/latest/download/latest.json`
- All update artifacts are signed with a Minisign key.
- See [docs/UPDATER_SETUP.md](docs/UPDATER_SETUP.md) for details.

---

## 🤝 Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for setup instructions, commit style, and the PR process.

1. Fork the repo
2. Create a feature branch: `git checkout -b feat/my-feature`
3. Commit your changes following [Conventional Commits](https://www.conventionalcommits.org/)
4. Open a Pull Request

---

## 📄 License

[MIT](LICENSE) © 2026 [Jalal Haidar](https://github.com/jalal-haidar)
