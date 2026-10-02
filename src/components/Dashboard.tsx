import React, { useEffect, useState } from "react";
import { Plus, Users, RefreshCw } from "lucide-react";
import { GitProfile, useProfileStore } from "../stores/useProfileStore";
import { useErrorToast } from "./ui/useErrorToast";
import { ProfileCard } from "./ProfileCard";
import DetectedProfilesList from "./DetectedProfilesList";
import DirectoryRules from "./DirectoryRules";
import ProfileForm from "./ProfileForm";

export const Dashboard: React.FC = () => {
  const { profiles, loading, fetchProfiles, detectIdentities, detectLoading } =
    useProfileStore();
  const showError = useErrorToast();
  // undefined = form closed, null = creating, profile = editing
  const [formTarget, setFormTarget] = useState<GitProfile | null | undefined>(
    undefined,
  );

  useEffect(() => {
    fetchProfiles().catch((e) => showError(e, "Could not load profiles"));
  }, [fetchProfiles, showError]);

  const handleDetectClick = () => {
    detectIdentities().catch((e) => showError(e));
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
              onClick={() => setFormTarget(null)}
            >
              <Plus size={18} /> New Profile
            </button>
          </div>
        </div>

        {formTarget !== undefined && (
          <ProfileForm
            key={formTarget?.id ?? "new"}
            profile={formTarget ?? undefined}
            onClose={() => setFormTarget(undefined)}
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
              <ProfileCard
                key={profile.id}
                profile={profile}
                isActive={profile.isDefault}
                onEdit={() => setFormTarget(profile)}
              />
            ))}
          </div>
        )}

        <section style={{ marginTop: 24 }}>
          <DirectoryRules />
        </section>

        <section style={{ marginTop: 24 }}>
          <DetectedProfilesList />
        </section>
      </section>
    </div>
  );
};
