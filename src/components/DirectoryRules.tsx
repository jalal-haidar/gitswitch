import React, { useEffect, useState } from "react";
import { FolderOpen, FolderPlus, Trash2 } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { useProfileStore } from "../stores/useProfileStore";
import { useErrorToast } from "./ui/useErrorToast";
import { useToast } from "./ui/useToast";

export const DirectoryRules: React.FC = () => {
  const profiles = useProfileStore((s) => s.profiles);
  const rules = useProfileStore((s) => s.directoryRules);
  const fetchDirectoryRules = useProfileStore((s) => s.fetchDirectoryRules);
  const addDirectoryRule = useProfileStore((s) => s.addDirectoryRule);
  const removeDirectoryRule = useProfileStore((s) => s.removeDirectoryRule);
  const previewDirectory = useProfileStore((s) => s.previewDirectory);
  const showError = useErrorToast();
  const { show } = useToast();

  const [path, setPath] = useState("");
  const [profileId, setProfileId] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    fetchDirectoryRules().catch((e) => showError(e, "Could not load rules"));
  }, [fetchDirectoryRules, showError]);

  // Keep a valid selection as profiles load or get deleted.
  useEffect(() => {
    if (!profiles.some((p) => p.id === profileId)) {
      setProfileId(profiles[0]?.id ?? "");
    }
  }, [profiles, profileId]);

  const profileLabel = (id: string) =>
    profiles.find((p) => p.id === id)?.label ?? "Unknown profile";

  const handleBrowse = async () => {
    try {
      const picked = await open({ directory: true, multiple: false });
      if (typeof picked === "string") setPath(picked);
    } catch (e) {
      showError(e, "Could not open folder picker");
    }
  };

  const handleAdd = async () => {
    setBusy(true);
    try {
      await addDirectoryRule(path, profileId);
      setPath("");
      show({ message: "Rule added", kind: "success" });
    } catch (e) {
      showError(e, "Could not add rule");
    } finally {
      setBusy(false);
    }
  };

  const handleRemove = (rulePath: string) =>
    removeDirectoryRule(rulePath).catch((e) =>
      showError(e, "Could not remove rule"),
    );

  const handlePreview = async (rulePath: string) => {
    try {
      const id = await previewDirectory(rulePath);
      show({
        message: id.email
          ? `Commits here use ${id.name ?? "?"} <${id.email}>`
          : "No repository found in this directory yet",
        kind: "info",
        duration: 6000,
      });
    } catch (e) {
      showError(e, "Preview failed");
    }
  };

  const canAdd = !busy && path.trim() !== "" && profileId !== "";

  return (
    <div className="directory-rules">
      <h3>Directory Rules</h3>
      <p className="muted">
        Repositories under a folder automatically use the chosen profile.
      </p>

      {rules.length === 0 ? (
        <div className="empty-state">No directory rules yet.</div>
      ) : (
        <ul className="rule-list">
          {rules.map((r) => (
            <li key={r.path} className="detected-item">
              <div className="detected-main">
                <strong>{r.path}</strong>
                <div className="muted">→ {profileLabel(r.profileId)}</div>
              </div>
              <div className="detected-actions">
                <button
                  className="btn btn-secondary"
                  onClick={() => handlePreview(r.path)}
                >
                  Check
                </button>
                <button
                  className="btn-icon delete-btn"
                  onClick={() => handleRemove(r.path)}
                  title="Remove rule"
                  aria-label={`Remove rule for ${r.path}`}
                >
                  <Trash2 size={16} />
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}

      <div className="rule-form">
        <input
          type="text"
          placeholder="Folder, e.g. ~/work"
          aria-label="Folder path"
          value={path}
          onChange={(e) => setPath(e.target.value)}
        />
        <button
          className="btn btn-ghost"
          onClick={handleBrowse}
          title="Browse…"
          aria-label="Browse for folder"
        >
          <FolderOpen size={16} />
        </button>
        <select
          aria-label="Profile"
          value={profileId}
          onChange={(e) => setProfileId(e.target.value)}
        >
          {profiles.map((p) => (
            <option key={p.id} value={p.id}>
              {p.label}
            </option>
          ))}
        </select>
        <button
          className="btn btn-primary"
          onClick={handleAdd}
          disabled={!canAdd}
        >
          <FolderPlus size={16} /> Add rule
        </button>
      </div>
    </div>
  );
};

export default DirectoryRules;
