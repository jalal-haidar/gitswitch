import React, { useEffect, useRef, useState, useMemo } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Plus,
  Users,
  RefreshCw,
  Settings as SettingsIcon,
  Search,
} from "lucide-react";
import Settings from "./Settings";

import { GitProfile, useProfileStore } from "../stores/useProfileStore";
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
  } = useProfileStore();
  const [showCreate, setShowCreate] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const toast = useToast();

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
          <div className="glass-panel empty-state">
            <Users size={48} />
            <p>No profiles found. Create one to get started!</p>
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

        <DirectoryRulesSection />
      </section>
      {showSettings && <Settings onClose={() => setShowSettings(false)} />}
    </div>
  );
};
