import React, { useEffect, useState } from "react";
import { X, Shield, RefreshCw } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { check } from "@tauri-apps/plugin-updater";

export const Settings: React.FC<{ onClose: () => void }> = ({ onClose }) => {
  const [storeSensitive, setStoreSensitive] = useState(false);
  const [loading, setLoading] = useState(false);
  const [updateLoading, setUpdateLoading] = useState(false);
  const [updateMessage, setUpdateMessage] = useState<string>("");

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
      const msg = e instanceof Error ? e.message : String(e);
      setUpdateMessage(`Update check failed: ${msg}`);
    } finally {
      setUpdateLoading(false);
    }
  };

  return (
    <div
      className="modal-overlay"
      role="dialog"
      aria-modal="true"
      onClick={onClose}
    >
      <div
        className="modal-panel glass-panel"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="settings-header">
          <h3>Settings</h3>
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
      </div>
    </div>
  );
};

export default Settings;
