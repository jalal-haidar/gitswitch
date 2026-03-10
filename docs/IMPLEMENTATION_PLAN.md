# GitSwitch — Project Orchestration & Implementation Plan

> A cross-platform desktop app that makes managing multiple Git identities effortless.

---

## 1. Problem Statement

Developers working with multiple Git accounts (personal, work, open-source organizations) face constant friction:

- **Identity Confusion** — Accidentally committing with the wrong `user.name` / `user.email`
- **SSH Key Juggling** — Manually editing `~/.ssh/config` every time context changes
- **Credential Popup Hell** — Repeatedly seeing "Select an account" dialogs (as shown in the screenshot)
- **No Single Source of Truth** — Scattered `.gitconfig` files, conditional includes, and credential helpers with no visibility
- **Time Wasted** — What should take zero thought requires manual file editing and debugging

### Who This Is For

Any developer who uses **2+ Git identities** on the same machine (e.g., personal GitHub + work GitHub/GitLab/Bitbucket + org accounts).

---

## 2. Competitive Landscape

| Tool | Type | Strengths | Gaps |
|------|------|-----------|------|
| **GitKraken** | Desktop Git Client | Profiles, SSH key gen | Heavy, paid for enterprise, it's a full Git client — not a focused identity manager |
| **GitHub Desktop** | Desktop Git Client | Multi-account since 2023 | GitHub-only, no SSH/GPG key management, no directory rules |
| **GitShift** | VS Code Extension | Workspace name/email switching | VS Code only, no SSH keys, no system tray, limited automation |
| **Git Account Switcher** | macOS App | Keychain integration, AES encryption | macOS only, no cross-platform |
| **Manual SSH Config** | Config files | Full control | Error-prone, no UI, steep learning curve |

### Our Differentiator

**GitSwitch** is a **dedicated, cross-platform, lightweight** identity management tool — not a full Git client. It focuses exclusively on making multi-account Git setups painless with:
- Visual profile management (name, email, SSH keys, GPG keys, signing preferences)
- Automatic profile switching based on directory rules
- System tray for instant switching
- Eventually, a VS Code extension for workspace-level control

---

## 3. Tech Stack Decision

### Desktop App: **Tauri 2.0 + React + TypeScript**

| Aspect | Choice | Rationale |
|--------|--------|-----------|
| **Framework** | **Tauri 2.0** | ~5MB binary (vs 100MB+ Electron), ~30MB RAM, sub-second startup. Uses OS native webview + Rust backend. Security-by-design. |
| **Frontend** | **React 19 + TypeScript** | Most popular UI library, strong ecosystem, excellent typing. Familiar to target audience (developers). |
| **Styling** | **CSS Modules + CSS Variables** | Lightweight, no build dependency. Clean theming with CSS custom properties for dark/light modes. |
| **State Management** | **Zustand** | Minimal, fast, TypeScript-first. Perfect for a focused app. |
| **Build Tool** | **Vite** | Bundled with Tauri's React template. Instant HMR. |
| **Icons** | **Lucide React** | Clean, consistent icon set. MIT licensed. |
| **Notifications** | **Tauri Notification Plugin** | Native OS notifications for profile switch confirmations. |

### Why Tauri over Electron?

> [!IMPORTANT]  
> For a **developer utility** that runs in the background and should feel native, Tauri is the clear winner:

- **Binary size**: ~5MB vs ~100MB+ (developers don't want a bloated identity manager)
- **Memory**: ~30MB idle vs ~150-300MB (it runs in system tray all day)
- **Security**: Rust backend with explicit permission model (we're handling SSH keys!)
- **Native feel**: Uses OS webview, so it looks and feels like a system utility
- **Future-proof**: Tauri 2.0 supports mobile (iOS/Android) if we ever want to go there

### VS Code Extension (Future Phase)

| Aspect | Choice |
|--------|--------|
| **Language** | TypeScript |
| **Framework** | VS Code Extension API |
| **UI** | VS Code Webview API (React) |

---

## 4. Feature Roadmap

### Phase 1: Core Identity Manager (MVP)
- **Profile CRUD** — Create, read, update, delete Git profiles (name, email, avatar/color)  
- **Active Profile Display** — Show which identity is currently active globally
- **One-Click Switch** — Switch `user.name` and `user.email` globally with a single click
- **Profile Validation** — Verify email format, warn on duplicates
- **Settings Persistence** — Store profiles in a local JSON/TOML config file

### Phase 2: SSH & Credential Management
- **SSH Key Association** — Link SSH keys to specific profiles
- **SSH Config Auto-Generation** — Automatically manage `~/.ssh/config` Host entries
- **SSH Key Generation** — Generate new SSH key pairs from within the app
- **Git Credential Helper Integration** — Configure credential helpers per profile
- **GPG Key Association** — Link GPG signing keys to profiles

### Phase 3: Automation & System Integration
- **Directory Rules** — Define rules like "all repos under `~/work/` use Work profile"
- **Git Conditional Includes** — Auto-generate `[includeIf]` blocks in `.gitconfig`
- **System Tray** — Quick-switch profiles from the system tray icon
- **Profile Status in Tray** — Show active profile name/color in tray tooltip
- **Native Notifications** — Notify when profile auto-switches based on directory rules

### Phase 4: VS Code Extension
- **Workspace Profile Picker** — Status bar item showing active Git identity
- **Per-Workspace Override** — Set a specific profile for the open workspace
- **Profile Sync** — Read profiles from the desktop app's config (shared config file)
- **Quick Switch Command** — `Ctrl+Shift+P` → "GitSwitch: Select Profile"

---

## 5. Architecture Overview

```
┌─────────────────────────────────────────────┐
│                 GitSwitch App               │
├──────────────────┬──────────────────────────┤
│   React Frontend │     Tauri Rust Backend   │
│                  │                          │
│  ┌────────────┐  │  ┌────────────────────┐  │
│  │ Profile UI │◄─┼─►│ Git Config Manager │  │
│  │ Dashboard  │  │  │ (read/write .git-  │  │
│  │ Settings   │  │  │  config files)     │  │
│  └────────────┘  │  ├────────────────────┤  │
│                  │  │ SSH Key Manager    │  │
│  ┌────────────┐  │  │ (generate, link,   │  │
│  │ System Tray│◄─┼─►│  manage keys)      │  │
│  │ Integration│  │  ├────────────────────┤  │
│  └────────────┘  │  │ Directory Watcher  │  │
│                  │  │ (auto-switch based │  │
│                  │  │  on CWD rules)     │  │
│                  │  ├────────────────────┤  │
│                  │  │ Config Store       │  │
│                  │  │ (profiles.json)    │  │
│                  │  └────────────────────┘  │
├──────────────────┴──────────────────────────┤
│            OS Native Layer                  │
│  - SSH Agent  - Git CLI  - System Tray      │
│  - Notifications  - File System Watcher     │
└─────────────────────────────────────────────┘
```

### Data Flow

1. **User creates a profile** → React sends command via Tauri IPC → Rust writes to `profiles.json`
2. **User switches profile** → Rust updates `~/.gitconfig` `[user]` section → Sends notification
3. **Directory rule triggers** → Rust file watcher detects CWD change → auto-switches → notifies frontend
4. **System tray click** → Native menu shows profiles → Rust handles switch → updates frontend state

### Config File Location

```
# Windows
%APPDATA%/com.gitswitch.app/

# macOS
~/Library/Application Support/com.gitswitch.app/

# Linux
~/.config/com.gitswitch.app/
```

**profiles.json** (example):
```json
{
  "profiles": [
    {
      "id": "uuid-1",
      "label": "Personal",
      "name": "John Doe",
      "email": "john@personal.dev",
      "color": "#7C3AED",
      "sshKeyPath": "~/.ssh/id_ed25519_personal",
      "gpgKeyId": null,
      "isDefault": true
    },
    {
      "id": "uuid-2",
      "label": "Work",
      "name": "John Doe",
      "email": "john.doe@company.com",
      "color": "#0EA5E9",
      "sshKeyPath": "~/.ssh/id_ed25519_work",
      "gpgKeyId": "ABC123",
      "isDefault": false
    }
  ],
  "directoryRules": [
    {
      "path": "~/work/**",
      "profileId": "uuid-2"
    }
  ],
  "settings": {
    "autoSwitch": true,
    "showNotifications": true,
    "startWithSystem": false,
    "theme": "system"
  }
}
```

---

## 6. Project Structure

```
gitswitch/
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── main.rs         # Tauri app entry point
│   │   ├── commands/       # IPC command handlers
│   │   │   ├── profiles.rs # Profile CRUD commands
│   │   │   ├── git.rs      # Git config operations
│   │   │   ├── ssh.rs      # SSH key management
│   │   │   └── settings.rs # App settings
│   │   ├── config/         # Config file management
│   │   │   ├── store.rs    # Read/write profiles.json
│   │   │   └── git.rs      # Git config file parser
│   │   └── utils/          # Helpers
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                    # React frontend
│   ├── components/
│   │   ├── ProfileCard.tsx
│   │   ├── ProfileForm.tsx
│   │   ├── ProfileList.tsx
│   │   ├── DirectoryRules.tsx
│   │   ├── SSHKeyManager.tsx
│   │   ├── Settings.tsx
│   │   └── SystemTray.tsx
│   ├── stores/
│   │   └── useProfileStore.ts
│   ├── hooks/
│   │   └── useTauriCommand.ts
│   ├── styles/
│   │   ├── index.css       # Global styles + design tokens
│   │   └── themes.css      # Dark/light theme variables
│   ├── App.tsx
│   └── main.tsx
├── docs/                   # Documentation
│   ├── README.md
│   ├── USER_GUIDE.md
│   ├── CONTRIBUTING.md
│   ├── ARCHITECTURE.md
│   └── assets/             # Screenshots, diagrams
├── .github/
│   ├── workflows/          # CI/CD (build + release)
│   └── ISSUE_TEMPLATE/
├── package.json
├── tsconfig.json
├── vite.config.ts
└── LICENSE
```

---

## 7. UI/UX Design Direction

### Design Principles
- **Developer-first** — Dark mode default, keyboard shortcuts, minimal clicks
- **Glanceable** — Active profile always visible, color-coded profiles
- **Non-intrusive** — Lives in system tray, pops up only when needed
- **Premium** — Glassmorphism cards, smooth transitions, modern typography (Inter font)

### Key Screens

1. **Dashboard** — Active profile prominently displayed, quick-switch buttons for other profiles
2. **Profile Editor** — Form to edit name, email, SSH key, GPG key, color
3. **Directory Rules** — Visual rule builder (drag-and-drop directories)
4. **SSH Keys** — List of keys, generation wizard, association to profiles
5. **Settings** — Theme, startup behavior, notification preferences

### Color Palette (Dark Mode Default)
- Background: `#0F1117` (deep dark)
- Surface: `#1A1D27` (card backgrounds)
- Border: `#2A2D37` (subtle borders)
- Accent: `#7C3AED` → `#A78BFA` (purple gradient, primary actions)
- Success: `#10B981` (active profile indicator)
- Text: `#F1F5F9` (primary), `#94A3B8` (secondary)

---

## 8. Documentation Plan

| Document | Purpose | When |
|----------|---------|------|
| **README.md** | Project overview, install, quick start | Phase 1 |
| **USER_GUIDE.md** | Full feature walkthrough with screenshots | After Phase 2 |
| **CONTRIBUTING.md** | How to contribute, code style, PR process | Phase 1 |
| **ARCHITECTURE.md** | Technical deep-dive, data flow, config format | Phase 1 |
| **CHANGELOG.md** | Version history | Ongoing |
| **LICENSE** | MIT License | Phase 1 |

---

## 9. Development Milestones

| Milestone | Deliverable | Estimated Effort |
|-----------|-------------|-----------------|
| **M1** | Project setup, profile CRUD, global switch | 2-3 days |
| **M2** | SSH key management, credential helpers | 2-3 days |
| **M3** | Directory rules, conditional includes, system tray | 3-4 days |
| **M4** | Polish: themes, animations, notifications | 1-2 days |
| **M5** | Documentation + CI/CD | 1-2 days |
| **M6** | VS Code extension (future) | 3-5 days |

---

## 10. Verification Plan

### Automated Testing
```bash
# Rust backend unit tests
cd src-tauri && cargo test

# Frontend component testing (Vitest + React Testing Library)
npm run test
```

### Manual Verification (Phase 1 MVP)
1. **Create a profile** → Verify data appears in `profiles.json`
2. **Switch profiles** → Run `git config --global user.name` and `git config --global user.email` to confirm values changed
3. **Edit a profile** → Verify changes persist after app restart
4. **Delete a profile** → Verify removal from config and UI
5. **App launch** → Verify correct active profile is displayed on startup

### Browser-Based UI Testing
- Launch the Tauri dev server (`npm run tauri dev`)
- Visually inspect all screens for correct styling, responsiveness, and interactions
- Test dark/light theme switching

---

## User Review Required

> [!IMPORTANT]
> Please review the following decisions before I proceed:

1. **App Name**: "GitSwitch" — does this resonate, or do you have a preferred name?
2. **Tech Stack**: Tauri 2.0 (Rust) + React + TypeScript — are you comfortable with Rust for the backend, or would you prefer Electron (pure JS)?
3. **Phase 1 Scope**: Starting with profile CRUD + one-click global switch (no SSH keys yet). Is this the right MVP?
4. **Should I start building Phase 1 now?**
