import React, { useEffect, useState } from "react";
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
      setUpdateMessage(
        "Update installed. Please restart the app to apply the new version.",
      );
    } catch {
      setUpdateMessage(
        "Unable to check or install updates. Ensure updater endpoint and public key are configured.",
      );
    } finally {
      setUpdateLoading(false);
    }
  };

  return (
    <div className="modal-overlay" role="dialog" aria-modal="true">
      <div className="modal-panel glass-panel">
        <header
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
          }}
        >
          <h3>Settings</h3>
          <button
            className="btn btn-ghost"
            onClick={onClose}
            aria-label="Close"
          >
            Close
          </button>
        </header>

        <div style={{ marginTop: "1rem" }}>
          <label className="checkbox-row" htmlFor="store-sensitive">
            <input
              id="store-sensitive"
              type="checkbox"
              checked={storeSensitive}
              disabled={loading}
              onChange={(e) => toggle(e.currentTarget.checked)}
            />
            <span>Store SSH/GPG paths in OS keyring (recommended)</span>
          </label>

          <div style={{ marginTop: "1rem", display: "grid", gap: "0.5rem" }}>
            <button
              className="btn btn-secondary"
              type="button"
              disabled={updateLoading}
              onClick={handleCheckForUpdates}
            >
              {updateLoading ? "Checking..." : "Check for updates"}
            </button>
            {updateMessage && <p className="muted">{updateMessage}</p>}
          </div>
        </div>
      </div>
    </div>
  );
};

export default Settings;
