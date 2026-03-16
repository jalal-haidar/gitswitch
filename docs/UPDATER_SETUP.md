# Updater Setup (Tauri v2)

This project is configured to use Tauri's updater plugin.

## 1) Generate updater keys (once)

Run locally (do **not** commit private key):

```powershell
npx tauri signer generate -w "$env:USERPROFILE\.tauri\gitswitch.key"
```

This prints a **public key** and writes the encrypted private key file.

- Put the printed public key into `src-tauri/tauri.conf.json` at `plugins.updater.pubkey`.
- Keep the private key file safe.

## 2) Add GitHub secrets

In GitHub repo settings → Secrets and variables → Actions, add:

- `TAURI_SIGNING_PRIVATE_KEY` = contents of `gitswitch.key`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` = password used when generating key

## 3) Release process

Push a version tag:

```powershell
git tag v0.1.2
git push origin v0.1.2
```

Workflow `.github/workflows/release.yml` builds and publishes assets + updater signatures.

## 4) Updater endpoint

The app checks:

- `https://github.com/jalal-haidar/gitswitch/releases/latest/download/latest.json`

Ensure the release workflow uploads updater metadata and signatures (handled by `tauri-apps/tauri-action`).

## 5) Local testing

- Install an older signed version.
- Publish a newer signed tag.
- In app Settings → **Check for updates**.

If checking fails, verify `pubkey`, secrets, and release assets/signatures.
