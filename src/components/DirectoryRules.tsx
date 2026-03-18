import React, { useEffect, useMemo, useState } from "react";
import { Pencil, Plus, Trash2, FolderOpen } from "lucide-react";
import { open as openFolderPicker } from "@tauri-apps/plugin-dialog";
import { normalizeBackendError } from "../utils/error";
import { useToast } from "./ui/useToast";
import { DirectoryRule, useProfileStore } from "../stores/useProfileStore";

interface RuleDraft {
  id?: string;
  path: string;
  profileId: string;
}

const emptyRule: RuleDraft = {
  path: "",
  profileId: "",
};

const RuleEditor: React.FC<{
  value: RuleDraft;
  profiles: { id: string; label: string }[];
  duplicate: boolean;
  busy: boolean;
  submitLabel: string;
  onChange: (next: RuleDraft) => void;
  onCancel: () => void;
  onSubmit: () => Promise<void>;
}> = ({
  value,
  profiles,
  duplicate,
  busy,
  submitLabel,
  onChange,
  onCancel,
  onSubmit,
}) => {
  const [touchedPath, setTouchedPath] = useState(false);
  const [touchedProfile, setTouchedProfile] = useState(false);
  const [submitted, setSubmitted] = useState(false);

  const pathOk = value.path.trim() !== "";
  const profileOk = value.profileId.trim() !== "";
  const canSubmit = pathOk && profileOk && !duplicate;

  const showPathError = (touchedPath || submitted) && !pathOk;
  const showProfileError = (touchedProfile || submitted) && !profileOk;

  const handleSubmit = async () => {
    setSubmitted(true);
    if (!canSubmit) return;
    await onSubmit();
  };

  return (
    <div className="glass-panel rule-editor">
      <div className="rule-editor-grid">
        <label className="field-group" htmlFor="rule-path">
          <span>Directory Path</span>
          <div className="file-picker-row">
            <input
              id="rule-path"
              aria-label="Directory path"
              placeholder="C:\\Users\\you\\work"
              value={value.path}
              onChange={(event) => {
                setTouchedPath(true);
                onChange({ ...value, path: event.target.value });
              }}
              onBlur={() => setTouchedPath(true)}
            />
            <button
              type="button"
              className="btn btn-secondary btn-browse"
              title="Browse for directory"
              onClick={async () => {
                const selected = await openFolderPicker({
                  multiple: false,
                  directory: true,
                  title: "Select Directory",
                });
                if (selected) {
                  setTouchedPath(true);
                  onChange({ ...value, path: selected as string });
                }
              }}
            >
              <FolderOpen size={14} />
            </button>
          </div>
        </label>

        <label className="field-group" htmlFor="rule-profile">
          <span>Profile</span>
          <select
            id="rule-profile"
            aria-label="Profile selection"
            value={value.profileId}
            onChange={(event) => {
              setTouchedProfile(true);
              onChange({ ...value, profileId: event.target.value });
            }}
            onBlur={() => setTouchedProfile(true)}
          >
            <option value="">Select profile…</option>
            {profiles.map((profile) => (
              <option key={profile.id} value={profile.id}>
                {profile.label}
              </option>
            ))}
          </select>
        </label>
      </div>

      {showPathError && (
        <div className="form-error" role="alert" aria-live="polite">
          Path is required.
        </div>
      )}
      {showProfileError && (
        <div className="form-error" role="alert" aria-live="polite">
          Select a profile.
        </div>
      )}
      {duplicate && (
        <div className="form-error" role="alert" aria-live="polite">
          A rule with this path and profile already exists.
        </div>
      )}

      <div className="profile-editor-actions">
        <button className="btn btn-secondary" type="button" onClick={onCancel}>
          Cancel
        </button>
        <button
          className="btn btn-primary"
          type="button"
          onClick={handleSubmit}
          disabled={busy}
        >
          {busy ? "Saving…" : submitLabel}
        </button>
      </div>
    </div>
  );
};

export const DirectoryRulesSection: React.FC = () => {
  const {
    profiles,
    directoryRules,
    autoSwitchEnabled,
    autoSwitchLoading,
    lastAutoSwitchEvent,
    rulesLoading,
    fetchAutoSwitchSetting,
    fetchLastAutoSwitchEvent,
    setAutoSwitchEnabled,
    addDirectoryRule,
    updateDirectoryRule,
    deleteDirectoryRule,
  } = useProfileStore();

  const toast = useToast();
  const [showCreate, setShowCreate] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draft, setDraft] = useState<RuleDraft>(emptyRule);

  useEffect(() => {
    fetchAutoSwitchSetting().catch(() => undefined);
  }, [fetchAutoSwitchSetting]);

  useEffect(() => {
    // Fetch once on mount, then update live via the auto-switch-triggered event
    // (replaces the old 3-second setInterval poll)
    fetchLastAutoSwitchEvent().catch(() => undefined);

    let unlisten: (() => void) | undefined;
    const setup = async () => {
      const { listen } = await import("@tauri-apps/api/event");
      unlisten = await listen("auto-switch-triggered", () => {
        fetchLastAutoSwitchEvent().catch(() => undefined);
      });
    };
    setup();
    return () => {
      unlisten?.();
    };
  }, [fetchLastAutoSwitchEvent]);

  const profileOptions = useMemo(
    () => profiles.map((p) => ({ id: p.id, label: p.label })),
    [profiles],
  );

  const duplicate = useMemo(() => {
    const path = draft.path.trim().toLowerCase();
    if (!path || !draft.profileId) return false;
    return directoryRules.some(
      (rule) =>
        rule.id !== draft.id &&
        rule.path.trim().toLowerCase() === path &&
        rule.profileId === draft.profileId,
    );
  }, [directoryRules, draft]);

  const watchedPathCount = useMemo(
    () => directoryRules.filter((rule) => rule.path.trim() !== "").length,
    [directoryRules],
  );

  const lastEventProfileLabel = useMemo(() => {
    if (!lastAutoSwitchEvent) return "Unknown profile";
    return (
      profiles.find((profile) => profile.id === lastAutoSwitchEvent.profileId)
        ?.label ?? "Unknown profile"
    );
  }, [lastAutoSwitchEvent, profiles]);

  const lastEventTime = useMemo(() => {
    if (!lastAutoSwitchEvent?.occurredAtEpochMs) return null;
    return new Date(lastAutoSwitchEvent.occurredAtEpochMs).toLocaleString();
  }, [lastAutoSwitchEvent]);

  const handleToggleAutoSwitch = async (
    event: React.ChangeEvent<HTMLInputElement>,
  ) => {
    const enabled = event.target.checked;
    try {
      await setAutoSwitchEnabled(enabled);
      toast.show({
        message: enabled ? "Auto-switch enabled" : "Auto-switch disabled",
        kind: "success",
      });
    } catch (e: any) {
      const info = normalizeBackendError(e?.toString?.() ?? e);
      toast.show({ message: info.message, kind: "error" });
    }
  };

  const startCreate = () => {
    setEditingId(null);
    setDraft({ ...emptyRule, profileId: profileOptions[0]?.id ?? "" });
    setShowCreate(true);
  };

  const startEdit = (rule: DirectoryRule) => {
    setShowCreate(false);
    setEditingId(rule.id);
    setDraft({ id: rule.id, path: rule.path, profileId: rule.profileId });
  };

  const resetEditor = () => {
    setShowCreate(false);
    setEditingId(null);
    setDraft(emptyRule);
  };

  const handleCreate = async () => {
    try {
      await addDirectoryRule({
        path: draft.path.trim(),
        profileId: draft.profileId,
      });
      toast.show({ message: "Directory rule added", kind: "success" });
      resetEditor();
    } catch (e: any) {
      const info = normalizeBackendError(e?.toString?.() ?? e);
      toast.show({ message: info.message, kind: "error" });
    }
  };

  const handleUpdate = async () => {
    if (!draft.id) return;
    try {
      await updateDirectoryRule({
        id: draft.id,
        path: draft.path.trim(),
        profileId: draft.profileId,
      });
      toast.show({ message: "Directory rule updated", kind: "success" });
      resetEditor();
    } catch (e: any) {
      const info = normalizeBackendError(e?.toString?.() ?? e);
      toast.show({ message: info.message, kind: "error" });
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await deleteDirectoryRule(id);
      toast.show({ message: "Directory rule deleted", kind: "success" });
      if (editingId === id) {
        resetEditor();
      }
    } catch (e: any) {
      const info = normalizeBackendError(e?.toString?.() ?? e);
      toast.show({ message: info.message, kind: "error" });
    }
  };

  return (
    <section className="rules-section" aria-labelledby="rules-heading">
      <div className="section-header">
        <h2 id="rules-heading">Directory Rules</h2>
        <div className="rules-header-actions">
          <label className="toggle-row" htmlFor="auto-switch-toggle">
            <span>Auto-switch</span>
            <input
              id="auto-switch-toggle"
              type="checkbox"
              checked={autoSwitchEnabled}
              onChange={handleToggleAutoSwitch}
              disabled={autoSwitchLoading}
              aria-label="Enable automatic profile switching"
            />
          </label>
          <button
            className="btn btn-primary"
            type="button"
            onClick={startCreate}
          >
            <Plus size={16} /> Add Rule
          </button>
        </div>
      </div>

      <div className="muted rules-status" role="status" aria-live="polite">
        {autoSwitchEnabled
          ? `Auto-switch is on. Watching ${watchedPathCount} path${watchedPathCount === 1 ? "" : "s"}.`
          : "Auto-switch is off."}
      </div>

      <div className="muted rules-status-note" role="note">
        Rules write local git config (.git/config), which overrides global
        config in matched repositories.
      </div>

      {lastAutoSwitchEvent && (
        <div
          className="muted rules-last-event"
          role="status"
          aria-live="polite"
        >
          Last auto-switch: {lastEventProfileLabel} at{" "}
          {lastAutoSwitchEvent.path}
          {lastEventTime ? ` (${lastEventTime})` : ""}
        </div>
      )}

      {showCreate && (
        <RuleEditor
          value={draft}
          profiles={profileOptions}
          duplicate={duplicate}
          busy={rulesLoading}
          submitLabel="Create Rule"
          onChange={setDraft}
          onCancel={resetEditor}
          onSubmit={handleCreate}
        />
      )}

      {directoryRules.length === 0 ? (
        <div
          className="glass-panel empty-state"
          role="status"
          aria-live="polite"
        >
          <p>No directory rules yet.</p>
        </div>
      ) : (
        <div className="rule-list" role="list">
          {directoryRules.map((rule) => {
            const profileLabel =
              profiles.find((profile) => profile.id === rule.profileId)
                ?.label ?? "Unknown profile";

            return (
              <React.Fragment key={rule.id}>
                <div
                  className="glass-panel rule-row"
                  role="listitem"
                  tabIndex={0}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") startEdit(rule);
                  }}
                  aria-label={`Rule for ${rule.path}, profile ${profileLabel}`}
                >
                  <div className="rule-main">
                    <strong>{rule.path}</strong>
                    <div className="muted">Profile: {profileLabel}</div>
                  </div>
                  <div className="rule-actions">
                    <button
                      className="btn btn-secondary"
                      type="button"
                      onClick={() => startEdit(rule)}
                      aria-label={`Edit rule ${rule.path}`}
                    >
                      <Pencil size={14} /> Edit
                    </button>
                    <button
                      className="btn-icon delete-btn"
                      type="button"
                      onClick={() => handleDelete(rule.id)}
                      title="Delete rule"
                      aria-label={`Delete rule ${rule.path}`}
                    >
                      <Trash2 size={16} />
                    </button>
                  </div>
                </div>

                {editingId === rule.id && (
                  <RuleEditor
                    value={draft}
                    profiles={profileOptions}
                    duplicate={duplicate}
                    busy={rulesLoading}
                    submitLabel="Save Rule"
                    onChange={setDraft}
                    onCancel={resetEditor}
                    onSubmit={handleUpdate}
                  />
                )}
              </React.Fragment>
            );
          })}
        </div>
      )}
    </section>
  );
};

export default DirectoryRulesSection;
