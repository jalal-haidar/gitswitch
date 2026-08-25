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
  Copy,
} from "lucide-react";
import { open as openFolderPicker } from "@tauri-apps/plugin-dialog";
import { normalizeBackendError, friendlyErrorMessage } from "../utils/error";
import { useToast } from "./ui/useToast";
import {
  DirectoryRule,
  AutoSwitchFailureEvent,
  RepoLocalConfig,
  useProfileStore,
} from "../stores/useProfileStore";
import { RuleCardSkeleton } from "./ui/Skeleton";
import ConfirmModal from "./ui/ConfirmModal";

function expectedSshCommand(keyPath: string): string {
  const normalized = keyPath.replace(/\\/g, "/");
  const quoted = /["$`\\\n\r]/.test(normalized)
    ? `'${normalized.split("'").join("'\"'\"'")}'`
    : `"${normalized}"`;
  return `ssh -i ${quoted} -o IdentitiesOnly=yes`;
}

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
                try {
                  const selected = await openFolderPicker({
                    multiple: false,
                    directory: true,
                    title: "Select Directory",
                  });
                  if (selected) {
                    setTouchedPath(true);
                    onChange({ ...value, path: selected as string });
                  }
                } catch {
                  // Dialog plugin failure — silently ignore
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
    watcherLifecycle,
    rulesLoading,
    fetchAutoSwitchSetting,
    setAutoSwitchEnabled,
    addDirectoryRule,
    updateDirectoryRule,
    deleteDirectoryRule,
    getRepoLocalConfig,
    applyProfileToRepo,
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
  const [deleteConfirmId, setDeleteConfirmId] = useState<string | null>(null);
  const [draft, setDraft] = useState<RuleDraft>(emptyRule);

  useEffect(() => {
    fetchAutoSwitchSetting().catch(() => undefined);
  }, [fetchAutoSwitchSetting]);

  useEffect(() => {
    let unlistenFailed: (() => void) | undefined;
    const setup = async () => {
      const { listen } = await import("@tauri-apps/api/event");
      unlistenFailed = await listen<AutoSwitchFailureEvent>(
        "auto-switch-failed",
        (event) => {
          const info = normalizeBackendError(event.payload.message ?? "");
          const repoName = event.payload.repositoryPath
            .replace(/\\/g, "/")
            .split("/")
            .filter(Boolean)
            .pop();
          toast.show({
            message: `Automatic repo apply failed${repoName ? ` for ${repoName}` : ""}: ${info.message}`,
            kind: "error",
            duration: info.hint ? 10000 : 8000,
          });
        },
      );
    };
    setup().catch(() => undefined);
    return () => {
      unlistenFailed?.();
    };
  }, [toast]);

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
        rule.path.trim().toLowerCase() === path,
    );
  }, [directoryRules, draft]);

  const watchedPathCount = useMemo(
    () => directoryRules.filter((rule) => rule.path.trim() !== "").length,
    [directoryRules],
  );

  const handleToggleAutoSwitch = async (
    event: React.ChangeEvent<HTMLInputElement>,
  ) => {
    const enabled = event.target.checked;
    try {
      await setAutoSwitchEnabled(enabled);
      toast.show({
        message: enabled
          ? "Automatic repo apply enabled"
          : "Automatic repo apply disabled",
        kind: "success",
      });
    } catch (e) {
      toast.show({ message: friendlyErrorMessage(e), kind: "error" });
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
    } catch (e) {
      toast.show({ message: friendlyErrorMessage(e), kind: "error" });
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
    } catch (e) {
      toast.show({ message: friendlyErrorMessage(e), kind: "error" });
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await deleteDirectoryRule(id);
      toast.show({ message: "Directory rule deleted", kind: "success" });
      if (editingId === id) {
        resetEditor();
      }
      setDeleteConfirmId(null);
    } catch (e) {
      toast.show({ message: friendlyErrorMessage(e), kind: "error" });
    }
  };

  const ruleToDelete = directoryRules.find((r) => r.id === deleteConfirmId);

  return (
    <section className="rules-section" aria-labelledby="rules-heading">
      <div className="section-header">
        <h2 id="rules-heading">Directory Rules</h2>
        <div className="rules-header-actions">
          <label className="toggle-row" htmlFor="auto-switch-toggle">
            <span>Auto-apply</span>
            <input
              id="auto-switch-toggle"
              type="checkbox"
              checked={autoSwitchEnabled}
              onChange={handleToggleAutoSwitch}
              disabled={autoSwitchLoading}
              aria-label="Enable automatic repo-local profile application"
            />
          </label>
          <button
            className="btn btn-primary"
            type="button"
            title="Create a rule that applies a profile to a repository's local Git config after relevant filesystem changes beneath the selected root."
            onClick={startCreate}
          >
            <Plus size={16} /> Add Rule
          </button>
        </div>
      </div>

      <div className="muted rules-status" role="status" aria-live="polite">
        {autoSwitchEnabled
          ? watcherLifecycle?.state === "recovered"
            ? `Auto-apply recovered. Watching ${watchedPathCount} configured path${watchedPathCount === 1 ? "" : "s"}.`
            : `Auto-apply is on. Watching ${watchedPathCount} configured path${watchedPathCount === 1 ? "" : "s"}.`
          : "Auto-apply is off."}
      </div>

      {(watcherLifecycle?.state === "degraded" ||
        watcherLifecycle?.state === "restarting") && (
        <div className="watcher-health watcher-health--degraded" role="alert">
          <strong>
            {watcherLifecycle.state === "restarting"
              ? "Watcher restarting automatically"
              : "Watcher degraded"}
          </strong>
          <span>
            {watcherLifecycle.message}
            {watcherLifecycle.retryInMs
              ? ` Retrying in ${Math.ceil(watcherLifecycle.retryInMs / 1000)}s.`
              : ""}
          </span>
        </div>
      )}

      {autoSwitchEnabled && watcherLifecycle?.state === "stopped" && (
        <div className="watcher-health" role="status">
          <strong>Watcher stopped</strong>
          <span>{watcherLifecycle.message}</span>
        </div>
      )}

      <div className="muted rules-status-note" role="note">
        Rules enforce repo-local Git config after relevant create, modify,
        rename, or remove activity beneath watched roots. Terminal navigation
        and <code>cd</code> alone do not trigger them.
      </div>

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

      {rulesLoading ? (
        <div className="rule-list" role="list">
          {[...Array(2)].map((_, i) => (
            <RuleCardSkeleton key={i} />
          ))}
        </div>
      ) : directoryRules.length === 0 && !showCreate ? (
        <div
          className="welcome-panel glass-panel rules-empty-guide"
          role="status"
          aria-live="polite"
        >
          <div className="welcome-icon">
            <FolderOpen size={32} />
          </div>
          <h3>Auto-apply by filesystem activity</h3>
          <p className="welcome-tagline">
            GitSwitch can enforce a profile in each repository’s local Git
            config when relevant files change beneath a watched root.
          </p>
          <ol className="welcome-steps">
            <li>
              <span className="step-num">1</span>
              <div>
                <strong>Enable Auto-apply</strong>
                <span>
                  Toggle <strong>Auto-apply</strong> on above. GitSwitch
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
                  <code>C:\work</code>) and which profile to apply when relevant
                  repository activity is detected beneath it.
                </span>
              </div>
            </li>
            <li>
              <span className="step-num">3</span>
              <div>
                <strong>Work normally</strong>
                <span>
                  Create, edit, rename, or remove a source file — GitSwitch
                  applies the matching profile to that repository. Entering the
                  directory or running <code>cd</code> does nothing by itself.
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
                        Last automatically applied:{" "}
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
                        } catch (err) {
                          setTestState((s) => ({
                            ...s,
                            [rule.id]: {
                              loading: false,
                              error: friendlyErrorMessage(err),
                            },
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
                      onClick={() => setDeleteConfirmId(rule.id)}
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

                    const copyToClipboard = (text: string) => {
                      navigator.clipboard
                        .writeText(text)
                        .then(() => {
                          toast.show({
                            message: "Copied to clipboard",
                            kind: "success",
                            duration: 2000,
                          });
                        })
                        .catch(() => {
                          toast.show({
                            message: "Failed to copy",
                            kind: "error",
                          });
                        });
                    };
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
                              {cfg.userName && (
                                <button
                                  className="btn-icon-inline"
                                  type="button"
                                  onClick={() => copyToClipboard(cfg.userName!)}
                                  title="Copy to clipboard"
                                  aria-label="Copy user.name"
                                >
                                  <Copy size={12} />
                                </button>
                              )}
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
                              {cfg.userEmail && (
                                <button
                                  className="btn-icon-inline"
                                  type="button"
                                  onClick={() =>
                                    copyToClipboard(cfg.userEmail!)
                                  }
                                  title="Copy to clipboard"
                                  aria-label="Copy user.email"
                                >
                                  <Copy size={12} />
                                </button>
                              )}
                            </span>
                          </li>
                          {(() => {
                            // Build the exact core.sshCommand string that
                            // switch_profile_for_repo writes so we can do a
                            // value-level comparison, not just presence check.
                            const expectedSshCmd = profile?.sshKeyPath
                              ? expectedSshCommand(profile.sshKeyPath)
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
                                  {cfg.coreSshCommand && (
                                    <button
                                      className="btn-icon-inline"
                                      type="button"
                                      onClick={() =>
                                        copyToClipboard(cfg.coreSshCommand!)
                                      }
                                      title="Copy to clipboard"
                                      aria-label="Copy core.sshCommand"
                                    >
                                      <Copy size={12} />
                                    </button>
                                  )}
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

      <ConfirmModal
        open={deleteConfirmId !== null}
        title="Delete directory rule?"
        description={
          ruleToDelete
            ? `Delete rule for "${ruleToDelete.path}"? This cannot be undone.`
            : "Delete this rule?"
        }
        confirmLabel="Delete"
        cancelLabel="Cancel"
        busy={rulesLoading}
        onCancel={() => setDeleteConfirmId(null)}
        onConfirm={() => {
          if (deleteConfirmId) {
            handleDelete(deleteConfirmId);
          }
        }}
      />
    </section>
  );
};

export default DirectoryRulesSection;
