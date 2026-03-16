import React from "react";
import { User, Mail, Trash2, CheckCircle2, Pencil } from "lucide-react";
import { GitProfile, useProfileStore } from "../stores/useProfileStore";

interface ProfileCardProps {
  profile: GitProfile;
  isActive: boolean;
  onEdit: (profile: GitProfile) => void;
}

export const ProfileCard: React.FC<ProfileCardProps> = ({
  profile,
  isActive,
  onEdit,
}) => {
  const { switchProfileGlobally, deleteProfile, loading } = useProfileStore();

  return (
    <div
      className={`glass-panel profile-card ${isActive ? "active" : ""}`}
      role="group"
      aria-label={`Profile ${profile.label}`}
    >
      <div className="profile-header">
        <div className="profile-avatar">
          <User size={20} />
        </div>
        <div className="profile-info">
          <h3>{profile.label}</h3>
          <div className="profile-details">
            <span className="detail-item">
              <User size={12} /> {profile.name}
            </span>
            <span className="detail-item">
              <Mail size={12} /> {profile.email}
            </span>
          </div>
        </div>
        {isActive && (
          <div className="active-badge" role="status" aria-live="polite">
            <CheckCircle2 size={16} /> Active
          </div>
        )}
      </div>

      <div className="profile-actions">
        {!isActive && (
          <button
            className="btn btn-primary"
            onClick={() => switchProfileGlobally(profile.id)}
            disabled={loading}
          >
            Switch to Profile
          </button>
        )}
        <button
          className="btn btn-secondary"
          onClick={() => onEdit(profile)}
          type="button"
          aria-label={`Edit ${profile.label}`}
        >
          <Pencil size={16} /> Edit
        </button>
        <button
          className="btn-icon delete-btn"
          onClick={() => deleteProfile(profile.id)}
          title="Delete Profile"
          aria-label={`Delete ${profile.label}`}
          disabled={loading}
        >
          <Trash2 size={16} />
        </button>
      </div>
    </div>
  );
};
