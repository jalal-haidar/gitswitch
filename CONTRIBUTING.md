# Contributing to GitSwitch

Thank you for your interest in contributing! This document covers everything you need to get started.

---

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Project Structure](#project-structure)
- [Making Changes](#making-changes)
- [Commit Style](#commit-style)
- [Submitting a Pull Request](#submitting-a-pull-request)
- [Reporting Bugs](#reporting-bugs)
- [Requesting Features](#requesting-features)

---

## Code of Conduct

Be respectful and constructive. We welcome contributors of all skill levels.

---

## Getting Started

1. **Fork** the repository on GitHub.
2. **Clone** your fork locally:
   ```bash
   git clone https://github.com/<your-username>/gitswitch.git
   cd gitswitch
   ```
3. Add the upstream remote:
   ```bash
   git remote add upstream https://github.com/jalal-haidar/gitswitch.git
   ```

---

## Development Setup

### Prerequisites

| Tool | Version |
|---|---|
| Node.js | ≥ 18 |
| Rust | stable (via [rustup](https://rustup.rs)) |
| Tauri CLI | v2 (`npm run tauri`) |

### Install & run

```bash
npm install
npm run tauri dev
```

### Run tests

```bash
# Frontend unit tests
npm run test:unit -- --run

# Rust tests
cd src-tauri
cargo test --workspace
```

---

## Project Structure

```
src/                  React + TypeScript frontend
src-tauri/src/        Rust backend (Tauri commands, models, config)
src-tauri/src/commands/  Individual Tauri command modules
.github/workflows/    CI and release automation
docs/                 Architecture and setup docs
```

---

## Making Changes

- Create a **feature branch** off `main`:
  ```bash
  git checkout -b feat/my-feature
  ```
- Keep changes **focused** — one feature or fix per PR.
- Add or update **tests** for any logic you change.
- Run the full test suite before opening a PR.

---

## Commit Style

We follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add SSH key picker in profile editor
fix: prevent crash when git config is missing
chore: bump dependencies
docs: update contributing guide
```

---

## Submitting a Pull Request

1. Push your branch and open a PR against `main`.
2. Fill in the PR template.
3. Link any related issues using `Closes #<number>`.
4. Ensure all CI checks pass.
5. A maintainer will review and merge or request changes.

---

## Reporting Bugs

Use the [Bug Report template](.github/ISSUE_TEMPLATE/bug_report.md) and include:
- GitSwitch version
- OS and version
- Steps to reproduce
- Expected vs actual behaviour
- Logs or screenshots if available

---

## Requesting Features

Use the [Feature Request template](.github/ISSUE_TEMPLATE/feature_request.md) and describe:
- The problem you're solving
- Your proposed solution
- Any alternatives you've considered
