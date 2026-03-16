import React, { useEffect, useState } from "react";
import { Plus, Users, RefreshCw } from "lucide-react";
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

  useEffect(() => {
    fetchProfiles();
    fetchDirectoryRules();
  }, [fetchDirectoryRules, fetchProfiles]);

  const toast = useToast();
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
          <h2>Your Profiles</h2>
          <div className="section-actions">
            <button
              className="btn btn-ghost"
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
            submitLabel="Create Profile"
            busy={loading}
            isDuplicate={duplicateExists}
            onCancel={() => setShowCreate(false)}
            onSubmit={handleCreate}
          />
        )}

        {loading ? (
          <div className="empty-state">Loading your profiles...</div>
        ) : profiles.length === 0 ? (
          <div className="glass-panel empty-state">
            <Users size={48} />
            <p>No profiles found. Create one to get started!</p>
          </div>
        ) : (
          <div className="profile-list">
            {profiles.map((profile) => (
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
    </div>
  );
};
