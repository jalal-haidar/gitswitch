import React, { useEffect, useMemo, useRef, useState } from "react";
import {
  Pencil,
  Plus,
  Trash2,
  FolderOpen,
  FlaskConical,
  CheckCircle2,
  XCircle,
  Loader2,
} from "lucide-react";
import { open as openFolderPicker } from "@tauri-apps/plugin-dialog";
import { normalizeBackendError } from "../utils/error";
import { useToast } from "./ui/useToast";
import {
  DirectoryRule,
  RepoLocalConfig,
  useProfileStore,
} from "../stores/useProfileStore";

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

  const pathInputRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    // Auto-focus path input when editor mounts
    pathInputRef.current?.focus();
  }, []);

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
              ref={pathInputRef}
              aria-label="Directory path"
              placeholder="Paste a path or click Browse…"
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
    getRepoLocalConfig,
    applyProfileToRepo,
    fetchProfiles,
  } = useProfileStore();

  // Per-rule test state: ruleId → { loading, result, error }
  const [testState, setTestState] = useState<
    Record<
      string,
      { loading: boolean; result?: RepoLocalConfig; error?: string }
    >
  >({});

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

    let unlistenSuccess: (() => void) | undefined;
    let unlistenFailed: (() => void) | undefined;
    const setup = async () => {
      const { listen } = await import("@tauri-apps/api/event");
      unlistenSuccess = await listen("auto-switch-triggered", () => {
        fetchLastAutoSwitchEvent().catch(() => undefined);
        // Also refresh profiles so the active-profile indicator in the
        // Dashboard reflects the new active_profile_id set by the switch.
        fetchProfiles().catch(() => undefined);
      });
      unlistenFailed = await listen<string>("auto-switch-failed", (event) => {
        const info = normalizeBackendError(event.payload ?? "");
        toast.show({
          message: `Auto-switch failed: ${info.message}`,
          kind: "error",
          duration: info.hint ? 10000 : 8000,
        });
      });
    };
    setup();
    return () => {
      unlistenSuccess?.();
      unlistenFailed?.();
    };
  }, [fetchLastAutoSwitchEvent, toast]);

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
    const existing = directoryRules.find((r) => r.id === draft.id);
    try {
      await updateDirectoryRule({
        id: draft.id,
        path: draft.path.trim(),
        profileId: draft.profileId,
        lastTriggeredAt: existing?.lastTriggeredAt,
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
            title="Create a directory rule: whenever you save a file inside a matched folder, GitSwitch automatically switches to the assigned profile — no manual switching needed."
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

      {directoryRules.length === 0 && !showCreate ? (
        <div
          className="welcome-panel glass-panel rules-empty-guide"
          role="status"
          aria-live="polite"
        >
          <div className="welcome-icon">
            <FolderOpen size={32} />
          </div>
          <h3>Auto-switch by directory</h3>
          <p className="welcome-tagline">
            GitSwitch can switch your Git identity automatically when you work
            in different folders — no manual switching needed.
          </p>
          <ol className="welcome-steps">
            <li>
              <span className="step-num">1</span>
              <div>
                <strong>Enable Auto-switch</strong>
                <span>
                  Toggle <strong>Auto-switch</strong> on above. GitSwitch
                  watches your filesystem in the background.
                </span>
              </div>
            </li>
            <li>
              <span className="step-num">2</span>
              <div>
                <strong>Add a rule</strong>
                <span>
                  Click <strong>+ Add Rule</strong> and pick a folder (e.g.{" "}
                  <code>C:\work</code>) and which profile to activate when a
                  file change is detected inside it.
                </span>
              </div>
            </li>
            <li>
              <span className="step-num">3</span>
              <div>
                <strong>Work normally</strong>
                <span>
                  Save any file in that folder — GitSwitch silently applies the
                  matching profile's identity to that repository's local config.
                </span>
              </div>
            </li>
          </ol>
          <button className="btn btn-primary welcome-cta" onClick={startCreate}>
            <Plus size={16} /> Add your first rule
          </button>
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
                    {rule.lastTriggeredAt ? (
                      <div className="muted rule-last-triggered">
                        Last triggered:{" "}
                        {new Date(rule.lastTriggeredAt).toLocaleString()}
                      </div>
                    ) : null}
                  </div>
                  <div className="rule-actions">
                    <button
                      className="btn btn-secondary"
                      type="button"
                      title="Apply this rule now and verify what's in the repo's local git config"
                      disabled={testState[rule.id]?.loading}
                      onClick={async () => {
                        setTestState((s) => ({
                          ...s,
                          [rule.id]: { loading: true },
                        }));
                        try {
                          // Let user pick a specific repo inside the watched directory
                          const picked = await openFolderPicker({
                            multiple: false,
                            directory: true,
                            title:
                              "Pick a repo inside this rule's directory to test",
                            defaultPath: rule.path,
                          });
                          if (!picked) {
                            setTestState((s) => ({
                              ...s,
                              [rule.id]: { loading: false },
                            }));
                            return;
                          }
                          await applyProfileToRepo(
                            rule.profileId,
                            picked as string,
                          );
                          const cfg = await getRepoLocalConfig(
                            picked as string,
                          );
                          setTestState((s) => ({
                            ...s,
                            [rule.id]: { loading: false, result: cfg },
                          }));
                        } catch (err: any) {
                          const info = normalizeBackendError(
                            err?.toString?.() ?? err,
                          );
                          setTestState((s) => ({
                            ...s,
                            [rule.id]: { loading: false, error: info.message },
                          }));
                        }
                      }}
                      aria-label={`Test rule ${rule.path}`}
                    >
                      {testState[rule.id]?.loading ? (
                        <Loader2 size={14} className="spin" />
                      ) : (
                        <FlaskConical size={14} />
                      )}{" "}
                      Test
                    </button>
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

                {testState[rule.id]?.result &&
                  (() => {
                    const ts = testState[rule.id]!;
                    const profile = profiles.find(
                      (p) => p.id === rule.profileId,
                    );
                    const cfg = ts.result!;
                    const check = (actual?: string, expected?: string) =>
                      actual && expected && actual.trim() === expected.trim();
                    return (
                      <div
                        className="glass-panel rule-proof-panel"
                        role="status"
                      >
                        <div className="rule-proof-header">
                          <strong>Local git config proof</strong>
                          <button
                            className="btn-icon"
                            type="button"
                            onClick={() =>
                              setTestState((s) => {
                                const next = { ...s };
                                delete next[rule.id];
                                return next;
                              })
                            }
                            aria-label="Dismiss proof panel"
                          >
                            ✕
                          </button>
                        </div>
                        <ul className="rule-proof-list">
                          <li>
                            {check(cfg.userName, profile?.name) ? (
                              <CheckCircle2 size={14} className="proof-ok" />
                            ) : (
                              <XCircle size={14} className="proof-fail" />
                            )}
                            <span>
                              <strong>user.name</strong>:{" "}
                              {cfg.userName ?? <em>not set</em>}
                            </span>
                          </li>
                          <li>
                            {check(cfg.userEmail, profile?.email) ? (
                              <CheckCircle2 size={14} className="proof-ok" />
                            ) : (
                              <XCircle size={14} className="proof-fail" />
                            )}
                            <span>
                              <strong>user.email</strong>:{" "}
                              {cfg.userEmail ?? <em>not set</em>}
                            </span>
                          </li>
                          {(() => {
                            // Build the exact core.sshCommand string that
                            // switch_profile_for_repo writes so we can do a
                            // value-level comparison, not just presence check.
                            // Format: ssh -i "<path with forward slashes>" -o IdentitiesOnly=yes
                            const expectedSshCmd = profile?.sshKeyPath
                              ? `ssh -i "${profile.sshKeyPath.replace(/\\/g, "/")}" -o IdentitiesOnly=yes`
                              : undefined;
                            // Profile has no SSH key → repo should have no sshCommand either.
                            const sshOk = expectedSshCmd
                              ? cfg.coreSshCommand === expectedSshCmd
                              : !cfg.coreSshCommand;
                            return (
                              <li>
                                {sshOk ? (
                                  <CheckCircle2
                                    size={14}
                                    className="proof-ok"
                                  />
                                ) : (
                                  <XCircle size={14} className="proof-fail" />
                                )}
                                <span>
                                  <strong>core.sshCommand</strong>:{" "}
                                  {cfg.coreSshCommand ?? <em>not set</em>}
                                  {!sshOk && (
                                    <span className="proof-hint">
                                      {expectedSshCmd
                                        ? ` (expected: ${expectedSshCmd})`
                                        : " (unexpected — profile has no SSH key)"}
                                    </span>
                                  )}
                                </span>
                              </li>
                            );
                          })()}
                        </ul>
                      </div>
                    );
                  })()}
                {testState[rule.id]?.error && (
                  <div
                    className="glass-panel rule-proof-panel rule-proof-error"
                    role="alert"
                  >
                    <XCircle size={14} className="proof-fail" />{" "}
                    {testState[rule.id]!.error}
                    <button
                      className="btn-icon"
                      type="button"
                      onClick={() =>
                        setTestState((s) => {
                          const next = { ...s };
                          delete next[rule.id];
                          return next;
                        })
                      }
                      aria-label="Dismiss error"
                    >
                      ✕
                    </button>
                  </div>
                )}

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
