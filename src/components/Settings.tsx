import React, { useEffect, useState } from "react";
import {
  X,
  Shield,
  RefreshCw,
  Download,
  Upload,
  Power,
  Sun,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { friendlyErrorMessage } from "../utils/error";
import {
  save as saveDialog,
  open as openDialog,
} from "@tauri-apps/plugin-dialog";
import { check } from "@tauri-apps/plugin-updater";

export const Settings: React.FC<{ onClose: () => void }> = ({ onClose }) => {
  const [storeSensitive, setStoreSensitive] = useState(false);
  const [startWithSystem, setStartWithSystem] = useState(false);
  const [theme, setThemeState] = useState<"system" | "light" | "dark">(
    "system",
  );
  const [loading, setLoading] = useState(false);
  const [autostartLoading, setAutostartLoading] = useState(false);
  const [updateLoading, setUpdateLoading] = useState(false);
  const [updateMessage, setUpdateMessage] = useState<string>("");
  const [exportMsg, setExportMsg] = useState("");
  const [importMsg, setImportMsg] = useState("");
  const [exportLoading, setExportLoading] = useState(false);
  const [importLoading, setImportLoading] = useState(false);

  useEffect(() => {
    let mounted = true;
    invoke<boolean>("get_store_sensitive_in_keyring")
      .then((v: boolean) => {
        if (mounted) setStoreSensitive(v);
      })
      .catch(() => {})
      .finally(() => {
        /* no-op */
      });
    invoke<boolean>("get_start_with_system")
      .then((v: boolean) => {
        if (mounted) setStartWithSystem(v);
      })
      .catch(() => {});
    invoke<string>("get_theme")
      .then((v: string) => {
        const valid = ["system", "light", "dark"] as const;
        if (mounted && valid.includes(v as (typeof valid)[number]))
          setThemeState(v as "system" | "light" | "dark");
      })
      .catch(() => {});
    return () => {
      mounted = false;
    };
  }, []);

  const toggle = async (enabled: boolean) => {
    setLoading(true);
    try {
      await invoke<boolean>("set_store_sensitive_in_keyring", { enabled });
      setStoreSensitive(enabled);
    } catch {
      // ignore
    } finally {
      setLoading(false);
    }
  };

  const toggleAutostart = async (enabled: boolean) => {
    setAutostartLoading(true);
    try {
      await invoke<boolean>("set_start_with_system", { enabled });
      setStartWithSystem(enabled);
    } catch (e) {
      console.error("Failed to toggle autostart:", e);
    } finally {
      setAutostartLoading(false);
    }
  };

  const handleThemeChange = async (next: "system" | "light" | "dark") => {
    try {
      await invoke("set_theme", { theme: next });
      setThemeState(next);
      document.documentElement.setAttribute("data-theme", next);
    } catch (e) {
      console.error("Failed to save theme:", e);
    }
  };

  const handleExport = async () => {
    setExportLoading(true);
    setExportMsg("");
    try {
      const path = await saveDialog({
        defaultPath: "gitswitch-profiles.json",
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!path) return;
      await invoke("export_profiles", { path });
      setExportMsg("Profiles exported successfully.");
    } catch (e) {
      setExportMsg(`Export failed: ${friendlyErrorMessage(e)}`);
    } finally {
      setExportLoading(false);
    }
  };

  const handleImport = async () => {
    setImportLoading(true);
    setImportMsg("");
    try {
      const path = await openDialog({
        multiple: false,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!path) return;
      const result = await invoke<{ added: number; skipped: number }>(
        "import_profiles",
        { path },
      );
      setImportMsg(
        `Imported ${result.added} profile(s)${result.skipped ? `, skipped ${result.skipped} duplicate(s)` : "."}`,
      );
    } catch (e) {
      setImportMsg(`Import failed: ${friendlyErrorMessage(e)}`);
    } finally {
      setImportLoading(false);
    }
  };

  const handleCheckForUpdates = async () => {
    if (import.meta.env.DEV) {
      setUpdateMessage("Update checks are not available in development mode.");
      return;
    }

    setUpdateLoading(true);
    setUpdateMessage("Checking for updates...");
    try {
      const update = await check();
      if (!update) {
        setUpdateMessage("You're on the latest version.");
        return;
      }

      setUpdateMessage(`Update found (v${update.version}). Downloading...`);
      await update.downloadAndInstall();
      setUpdateMessage("Update installed. Please restart the app to apply it.");
    } catch (e) {
      setUpdateMessage(`Update check failed: ${friendlyErrorMessage(e)}`);
    } finally {
      setUpdateLoading(false);
    }
  };

  return (
    <div
      className="modal-overlay"
      role="dialog"
      aria-modal="true"
      aria-labelledby="settings-title"
      onClick={onClose}
    >
      <div
        className="modal-panel glass-panel"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="settings-header">
          <h3 id="settings-title">Settings</h3>
          <button
            className="btn-icon"
            onClick={onClose}
            aria-label="Close settings"
          >
            <X size={18} />
          </button>
        </div>

        <div className="settings-section">
          <div className="settings-section-title">
            <Sun size={14} />
            Theme
          </div>
          <div
            className="theme-selector"
            role="group"
            aria-label="Theme selection"
          >
            {(["system", "light", "dark"] as const).map((t) => (
              <button
                key={t}
                type="button"
                className={`btn ${theme === t ? "btn-primary" : "btn-secondary"} theme-btn`}
                onClick={() => handleThemeChange(t)}
                aria-pressed={theme === t}
              >
                {t.charAt(0).toUpperCase() + t.slice(1)}
              </button>
            ))}
          </div>
          <p className="muted settings-hint">
            System follows your OS dark/light preference.
          </p>
        </div>

        <div className="settings-divider" />

        <div className="settings-section">
          <div className="settings-section-title">
            <Power size={14} />
            Startup
          </div>
          <label className="checkbox-row" htmlFor="start-with-system">
            <input
              id="start-with-system"
              type="checkbox"
              checked={startWithSystem}
              disabled={autostartLoading}
              onChange={(e) => toggleAutostart(e.currentTarget.checked)}
            />
            <span>Launch at system startup</span>
          </label>
          <p className="muted settings-hint">
            App will start automatically when you log in to your computer.
          </p>
        </div>

        <div className="settings-divider" />

        <div className="settings-section">
          <div className="settings-section-title">
            <Shield size={14} />
            Security
          </div>
          <label className="checkbox-row" htmlFor="store-sensitive">
            <input
              id="store-sensitive"
              type="checkbox"
              checked={storeSensitive}
              disabled={loading}
              onChange={(e) => toggle(e.currentTarget.checked)}
            />
            <span>Store SSH/GPG paths in OS keyring</span>
          </label>
          <p className="muted settings-hint">
            Moves sensitive paths out of profiles.json into the OS credential
            store. Toggling migrates all existing profiles immediately.
          </p>
        </div>

        <div className="settings-divider" />

        <div className="settings-section">
          <div className="settings-section-title">
            <RefreshCw size={14} />
            Updates
          </div>
          <button
            className="btn btn-secondary settings-update-btn"
            type="button"
            disabled={updateLoading}
            onClick={handleCheckForUpdates}
          >
            {updateLoading ? "Checking…" : "Check for updates"}
          </button>
          {updateMessage && (
            <p className="muted settings-hint">{updateMessage}</p>
          )}
        </div>

        <div className="settings-divider" />

        <div className="settings-section">
          <div className="settings-section-title">
            <Download size={14} />
            Profiles Backup
          </div>
          <div className="settings-row">
            <button
              className="btn btn-secondary"
              type="button"
              disabled={exportLoading}
              onClick={handleExport}
            >
              <Download size={14} />{" "}
              {exportLoading ? "Exporting…" : "Export profiles"}
            </button>
            <button
              className="btn btn-secondary"
              type="button"
              disabled={importLoading}
              onClick={handleImport}
            >
              <Upload size={14} />{" "}
              {importLoading ? "Importing…" : "Import profiles"}
            </button>
          </div>
          {exportMsg && <p className="muted settings-hint">{exportMsg}</p>}
          {importMsg && <p className="muted settings-hint">{importMsg}</p>}
        </div>
      </div>
    </div>
  );
};

export default Settings;
