import React from "react";
import { User, Mail, Trash2, CheckCircle2, Pencil } from "lucide-react";
import { GitProfile, useProfileStore } from "../stores/useProfileStore";
import { useErrorToast } from "./ui/useErrorToast";

interface ProfileCardProps {
  profile: GitProfile;
  isActive: boolean;
  onEdit: () => void;
}

export const ProfileCard: React.FC<ProfileCardProps> = ({
  profile,
  isActive,
  onEdit,
}) => {
  const { switchProfileGlobally, deleteProfile } = useProfileStore();
  const showError = useErrorToast();

  return (
    <div
      className={`glass-panel profile-card ${isActive ? "active" : ""}`}
      style={{ "--profile-color": profile.color } as React.CSSProperties}
    >
      <div className="profile-header">
        <div
          className="profile-avatar"
          style={{ backgroundColor: profile.color }}
        >
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
          <div className="active-badge">
            <CheckCircle2 size={16} /> Active
          </div>
        )}
      </div>

      <div className="profile-actions">
        {!isActive && (
          <button
            className="btn btn-primary"
            onClick={() =>
              switchProfileGlobally(profile.id).catch((e) =>
                showError(e, "Could not switch profile"),
              )
            }
          >
            Switch to Profile
          </button>
        )}
        <button className="btn-icon" onClick={onEdit} title="Edit Profile">
          <Pencil size={16} />
        </button>
        <button
          className="btn-icon delete-btn"
          onClick={() =>
            deleteProfile(profile.id).catch((e) =>
              showError(e, "Could not delete profile"),
            )
          }
          title="Delete Profile"
        >
          <Trash2 size={16} />
        </button>
      </div>
    </div>
  );
};
