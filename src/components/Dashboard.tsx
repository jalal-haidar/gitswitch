import React, { useEffect, useRef, useState, useMemo } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getVersion } from "@tauri-apps/api/app";
import {
  Plus,
  Users,
  RefreshCw,
  Settings as SettingsIcon,
  Search,
  ScanSearch,
} from "lucide-react";
import { open as openFolderPicker } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import Settings from "./Settings";

import {
  GitProfile,
  ScannedRepo,
  useProfileStore,
} from "../stores/useProfileStore";
import { useToast } from "./ui/useToast";
import { normalizeBackendError } from "../utils/error";
import { ProfileCard } from "./ProfileCard";
import DetectedProfilesList from "./DetectedProfilesList";
import ProfileEditor, {
  ProfileEditorValue,
  toEditorValue,
} from "./ProfileEditor";
import DirectoryRulesSection from "./DirectoryRules";

export const Dashboard: React.FC = () => {
  const {
    profiles,
    activeProfileId,
    loading,
    fetchProfiles,
    fetchDirectoryRules,
    addProfile,
    updateProfile,
    detectIdentities,
    detectLoading,
    scanRepos,
  } = useProfileStore();
  const [showCreate, setShowCreate] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const searchInputRef = useRef<HTMLInputElement>(null);
  const toast = useToast();

  // Repo scanner state
  const [scannedRepos, setScannedRepos] = useState<ScannedRepo[]>([]);
  const [scanLoading, setScanLoading] = useState(false);
  const [applyTargets, setApplyTargets] = useState<Record<string, string>>({});
  const [applyingPath, setApplyingPath] = useState<string | null>(null);
  const [appVersion, setAppVersion] = useState<string>("");

  useEffect(() => {
    getVersion()
      .then(setAppVersion)
      .catch(() => setAppVersion("0.2.3"));
  }, []);

  useEffect(() => {
    fetchProfiles();
    fetchDirectoryRules();
  }, [fetchDirectoryRules, fetchProfiles]);

  // Update window title to reflect active profile
  useEffect(() => {
    const activeProfile = profiles.find((p) => p.id === activeProfileId);
    const titleSuffix = activeProfile ? ` — ${activeProfile.label}` : "";
    getCurrentWindow()
      .setTitle(`GitSwitch${titleSuffix}`)
      .catch(() => {
        /* not in Tauri context */
      });
  }, [activeProfileId, profiles]);

  // Keep a ref so the auto-switch listener always has the latest toast without re-subscribing
  const toastRef = useRef(toast);
  useEffect(() => {
    toastRef.current = toast;
  }, [toast]);

  // Refresh profiles when a tray switch happens ("profiles-changed" from Rust)
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    const setup = async () => {
      const { listen } = await import("@tauri-apps/api/event");
      unlisten = await listen("profiles-changed", () => {
        void useProfileStore.getState().fetchProfiles();
      });
    };
    setup();
    return () => {
      unlisten?.();
    };
  }, []);

  // Warn when OS keyring write fails — credential stored as plain text fallback
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    const setup = async () => {
      const { listen } = await import("@tauri-apps/api/event");
      unlisten = await listen<string>("keyring-warning", (event) => {
        toastRef.current.show({
          message: `⚠ Keyring unavailable — ${event.payload}`,
          kind: "error",
          duration: 8000,
        });
      });
    };
    setup();
    return () => {
      unlisten?.();
    };
  }, []);

  // Alert when the auto-switch file watcher dies unexpectedly
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    const setup = async () => {
      const { listen } = await import("@tauri-apps/api/event");
      unlisten = await listen<string>("auto-switch-error", (event) => {
        toastRef.current.show({
          message: `Auto-switch stopped: ${event.payload}. Restart the app to re-enable it.`,
          kind: "error",
          duration: 10000,
        });
      });
    };
    setup();
    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    const setup = async () => {
      const { listen } = await import("@tauri-apps/api/event");
      unlisten = await listen<{ profile_id: string; path: string }>(
        "auto-switch-triggered",
        (event) => {
          const state = useProfileStore.getState();
          const profile = state.profiles.find(
            (p) => p.id === event.payload.profile_id,
          );
          const label = profile?.label ?? event.payload.profile_id;
          // Trim path to last 2 segments for readability
          const segments = event.payload.path
            .replace(/\\/g, "/")
            .split("/")
            .filter(Boolean);
          const shortPath = segments.slice(-2).join("/");
          toastRef.current.show({
            message: `Auto-switched to \"${label}\" (…/${shortPath})`,
            kind: "success",
            duration: 4500,
          });
          void state.fetchProfiles();
        },
      );
    };
    setup();
    return () => {
      unlisten?.();
    };
  }, []);

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const isMac = navigator.platform.toUpperCase().indexOf("MAC") >= 0;
      const cmdOrCtrl = isMac ? e.metaKey : e.ctrlKey;

      // Escape - Close editor or settings
      if (e.key === "Escape") {
        if (showSettings) {
          setShowSettings(false);
          e.preventDefault();
        } else if (showCreate) {
          setShowCreate(false);
          e.preventDefault();
        } else if (editingId) {
          setEditingId(null);
          e.preventDefault();
        }
        return;
      }

      // Cmd/Ctrl+N - New profile (unless in input field or editing)
      if (cmdOrCtrl && e.key === "n") {
        const target = e.target as HTMLElement;
        const isInInput =
          target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.isContentEditable;
        if (!isInInput && !showCreate && !editingId) {
          setShowCreate(true);
          e.preventDefault();
        }
        return;
      }

      // Cmd/Ctrl+, - Open settings
      if (cmdOrCtrl && e.key === ",") {
        setShowSettings(true);
        e.preventDefault();
        return;
      }

      // Cmd/Ctrl+F - Focus search box
      if (cmdOrCtrl && e.key === "f") {
        if (profiles.length > 0 && searchInputRef.current) {
          searchInputRef.current.focus();
          e.preventDefault();
        }
        return;
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [showSettings, showCreate, editingId, profiles.length]);

  const duplicateExists = (value: ProfileEditorValue) => {
    const nextName = value.name.trim().toLowerCase();
    const nextEmail = value.email.trim().toLowerCase();
    return profiles.some(
      (profile) =>
        profile.id !== value.id &&
        profile.name.trim().toLowerCase() === nextName &&
        profile.email.trim().toLowerCase() === nextEmail,
    );
  };

  const filteredProfiles = useMemo(() => {
    if (!searchQuery.trim()) return profiles;
    const query = searchQuery.trim().toLowerCase();
    return profiles.filter(
      (p) =>
        p.label.toLowerCase().includes(query) ||
        p.name.toLowerCase().includes(query) ||
        p.email.toLowerCase().includes(query),
    );
  }, [profiles, searchQuery]);

  const handleCreate = async (value: ProfileEditorValue) => {
    if (duplicateExists(value)) return;
    try {
      await addProfile({
        label: value.label,
        name: value.name,
        email: value.email,
        color: value.color,
        isDefault: value.isDefault,
        sshKeyPath: value.sshKeyPath,
        gpgKeyId: value.gpgKeyId,
      });
      setShowCreate(false);
      toast.show({ message: `Created ${value.label}`, kind: "success" });
    } catch (e: any) {
      const info = normalizeBackendError(e?.toString?.() ?? e);
      toast.show({ message: info.message, kind: "error" });
    }
  };

  const handleUpdate = async (value: ProfileEditorValue) => {
    if (!value.id || duplicateExists(value)) return;
    try {
      await updateProfile({
        id: value.id,
        label: value.label,
        name: value.name,
        email: value.email,
        color: value.color,
        isDefault: value.isDefault,
        sshKeyPath: value.sshKeyPath,
        gpgKeyId: value.gpgKeyId,
      });
      setEditingId(null);
      toast.show({ message: `Updated ${value.label}`, kind: "success" });
    } catch (e: any) {
      const info = normalizeBackendError(e?.toString?.() ?? e);
      toast.show({ message: info.message, kind: "error" });
    }
  };

  const handleDetectClick = async () => {
    try {
      await detectIdentities();
    } catch (e: any) {
      const info = normalizeBackendError(e?.toString?.() ?? e);
      toast.show({
        message: info.message,
        kind: "error",
        duration: info.hint ? 8000 : 6000,
      });
    }
  };

  const handleScanRepos = async () => {
    const root = await openFolderPicker({
      multiple: false,
      directory: true,
      title: "Select root folder to scan for git repos",
    });
    if (!root) return;
    setScanLoading(true);
    try {
      const results = await scanRepos(root as string);
      setScannedRepos(results);
      // Pre-select matched profile (or first profile) for each row
      const targets: Record<string, string> = {};
      for (const r of results) {
        targets[r.path] = r.matchedProfileId ?? profiles[0]?.id ?? "";
      }
      setApplyTargets(targets);
      if (results.length === 0) {
        toast.show({
          message: "No git repos found in that folder.",
          kind: "success",
        });
      }
    } catch (e: any) {
      toast.show({ message: `Scan failed: ${e}`, kind: "error" });
    } finally {
      setScanLoading(false);
    }
  };

  const handleApplyToRepo = async (repoPath: string) => {
    const profileId = applyTargets[repoPath];
    if (!profileId) return;
    setApplyingPath(repoPath);
    try {
      await invoke("apply_profile_to_repo", { profileId, repoPath });
      const label =
        profiles.find((p) => p.id === profileId)?.label ?? profileId;
      const repoName =
        repoPath.replace(/\\/g, "/").split("/").filter(Boolean).pop() ??
        repoPath;
      toast.show({
        message: `Applied "${label}" to ${repoName}`,
        kind: "success",
      });
    } catch (e: any) {
      toast.show({ message: `Apply failed: ${e}`, kind: "error" });
    } finally {
      setApplyingPath(null);
    }
  };

  return (
    <div className="dashboard">
      <header className="dashboard-header">
        <h1 className="text-gradient">GitSwitch</h1>
        <p>Manage your Git identities with ease.</p>
      </header>

      <section>
        <div className="section-header">
          <h2>
            Your Profiles
            {profiles.length > 0 && (
              <span className="profile-count-badge">{profiles.length}</span>
            )}
          </h2>
          <div className="section-actions">
            <button
              className="btn btn-ghost"
              title="Settings"
              onClick={() => setShowSettings(true)}
            >
              <SettingsIcon size={16} />
            </button>
            <button
              className="btn btn-ghost detect-btn"
              onClick={handleDetectClick}
              title="Detect identities"
              disabled={detectLoading}
            >
              <RefreshCw size={16} /> {detectLoading ? "Scanning…" : "Detect"}
            </button>
            <button
              className="btn btn-primary"
              onClick={() => {
                setEditingId(null);
                setShowCreate((current) => !current);
              }}
            >
              <Plus size={18} /> {showCreate ? "Close" : "New Profile"}
            </button>
          </div>
        </div>

        {showCreate && (
          <ProfileEditor
            key="create"
            submitLabel="Create Profile"
            busy={loading}
            isDuplicate={duplicateExists}
            onCancel={() => setShowCreate(false)}
            onSubmit={handleCreate}
          />
        )}

        {profiles.length > 0 && (
          <div className="profile-search">
            <Search size={16} className="profile-search-icon" />
            <input
              ref={searchInputRef}
              type="text"
              placeholder="Search profiles by label, name, or email…"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="profile-search-input"
            />
          </div>
        )}

        {loading ? (
          <div className="empty-state">Loading your profiles...</div>
        ) : profiles.length === 0 ? (
          <div className="welcome-panel glass-panel">
            <div className="welcome-icon">
              <Users size={40} />
            </div>
            <h3>Welcome to GitSwitch</h3>
            <p className="welcome-tagline">
              Switch Git identities instantly — for work, personal, open-source,
              or any project.
            </p>
            <ol className="welcome-steps">
              <li>
                <span className="step-num">1</span>
                <div>
                  <strong>Create a profile</strong>
                  <span>
                    Click <strong>New Profile</strong> above. Set your name,
                    email, and optionally an SSH key or GPG key.
                  </span>
                </div>
              </li>
              <li>
                <span className="step-num">2</span>
                <div>
                  <strong>Switch to it</strong>
                  <span>
                    Hit <strong>Switch to Profile</strong> — GitSwitch writes
                    your identity to global Git config instantly.
                  </span>
                </div>
              </li>
              <li>
                <span className="step-num">3</span>
                <div>
                  <strong>Auto-switch by folder (optional)</strong>
                  <span>
                    Go to <strong>Directory Rules</strong> to activate profiles
                    automatically when you enter a project folder.
                  </span>
                </div>
              </li>
              <li>
                <span className="step-num">4</span>
                <div>
                  <strong>Set up SSH (optional)</strong>
                  <span>
                    Generate a key with <code>ssh-keygen -t ed25519</code>, add
                    the public key to GitHub, then set the private key path in
                    your profile.
                  </span>
                </div>
              </li>
            </ol>
            <button
              className="btn btn-primary welcome-cta"
              onClick={() => setShowCreate(true)}
            >
              <Plus size={16} /> Create your first profile
            </button>
          </div>
        ) : filteredProfiles.length === 0 ? (
          <div className="glass-panel empty-state">
            <Users size={48} />
            <p>No profiles match your search.</p>
          </div>
        ) : (
          <div className="profile-list">
            {filteredProfiles.map((profile) => (
              <React.Fragment key={profile.id}>
                <ProfileCard
                  profile={profile}
                  isActive={activeProfileId === profile.id}
                  onEdit={(selected: GitProfile) => {
                    setShowCreate(false);
                    setEditingId(selected.id);
                  }}
                />
                {editingId === profile.id && (
                  <ProfileEditor
                    key={editingId}
                    initialValue={toEditorValue(profile)}
                    submitLabel="Save Changes"
                    busy={loading}
                    isDuplicate={duplicateExists}
                    onCancel={() => setEditingId(null)}
                    onSubmit={handleUpdate}
                  />
                )}
              </React.Fragment>
            ))}
          </div>
        )}

        <section className="detected-section">
          <DetectedProfilesList />
        </section>

        {/* ── Repo Scanner ─────────────────────────────────────────────── */}
        <section className="scan-section" aria-labelledby="scan-heading">
          <div className="section-header">
            <h2 id="scan-heading">Repo Scanner</h2>
            <div className="section-actions">
              {scannedRepos.length > 0 && (
                <button
                  className="btn btn-ghost"
                  type="button"
                  onClick={() => setScannedRepos([])}
                >
                  Clear
                </button>
              )}
              <button
                className="btn btn-primary"
                type="button"
                onClick={handleScanRepos}
                disabled={scanLoading}
              >
                <ScanSearch size={16} />
                {scanLoading ? "Scanning…" : "Scan Folder…"}
              </button>
            </div>
          </div>

          {scannedRepos.length === 0 && !scanLoading && (
            <p className="muted scan-hint">
              Pick a root folder (e.g. C:\projects) to discover all git repos
              inside it and bulk-apply identities in one place.
            </p>
          )}

          {scannedRepos.length > 0 && (
            <div className="glass-panel scan-results">
              <div className="scan-count muted">
                {scannedRepos.length} repo{scannedRepos.length !== 1 ? "s" : ""}{" "}
                found
              </div>
              <div
                className="scan-table"
                role="table"
                aria-label="Scanned repositories"
              >
                <div className="scan-header" role="row" aria-hidden="true">
                  <span>Repository</span>
                  <span>Detected Identity</span>
                  <span>Apply Profile</span>
                </div>
                {scannedRepos.map((repo) => {
                  const matchedProfile = profiles.find(
                    (p) => p.id === repo.matchedProfileId,
                  );
                  const targetProfileId = applyTargets[repo.path] ?? "";
                  const isApplying = applyingPath === repo.path;
                  return (
                    <div key={repo.path} className="scan-row" role="row">
                      <div className="scan-cell scan-repo-info">
                        <strong className="scan-repo-name">{repo.name}</strong>
                        <span
                          className="muted scan-repo-path"
                          title={repo.path}
                        >
                          {repo.path}
                        </span>
                        {repo.remoteService && (
                          <span
                            className={`detail-item remote-badge remote-badge--${repo.remoteService}`}
                          >
                            {repo.remoteService.charAt(0).toUpperCase() +
                              repo.remoteService.slice(1)}
                          </span>
                        )}
                      </div>

                      <div className="scan-cell scan-identity">
                        {repo.userName || repo.userEmail ? (
                          <>
                            <span>{repo.userName ?? "–"}</span>
                            <span className="muted">
                              {repo.userEmail ?? ""}
                            </span>
                            {matchedProfile ? (
                              <span
                                className="detail-item scan-match-badge"
                                style={{
                                  background: `${matchedProfile.color}22`,
                                  color: matchedProfile.color,
                                  borderColor: `${matchedProfile.color}44`,
                                }}
                              >
                                {matchedProfile.label}
                              </span>
                            ) : (
                              <span className="muted scan-no-match">
                                No match
                              </span>
                            )}
                          </>
                        ) : (
                          <span className="muted">Not set</span>
                        )}
                      </div>

                      <div className="scan-cell scan-apply">
                        <select
                          value={targetProfileId}
                          onChange={(e) =>
                            setApplyTargets((prev) => ({
                              ...prev,
                              [repo.path]: e.target.value,
                            }))
                          }
                          aria-label={`Select profile for ${repo.name}`}
                        >
                          <option value="">– select –</option>
                          {profiles.map((p) => (
                            <option key={p.id} value={p.id}>
                              {p.label}
                            </option>
                          ))}
                        </select>
                        <button
                          className="btn btn-primary btn-sm"
                          type="button"
                          disabled={!targetProfileId || isApplying}
                          onClick={() => handleApplyToRepo(repo.path)}
                        >
                          {isApplying ? "Applying…" : "Apply"}
                        </button>
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          )}
        </section>

        <DirectoryRulesSection />
      </section>

      <footer className="app-footer">
        <span className="muted">GitSwitch v{appVersion}</span>
      </footer>

      {showSettings && <Settings onClose={() => setShowSettings(false)} />}
    </div>
  );
};
