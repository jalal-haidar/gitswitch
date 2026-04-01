import React from "react";
import {
  User,
  Mail,
  Trash2,
  CheckCircle2,
  Pencil,
  KeyRound,
  Key,
  Copy,
  FolderInput,
} from "lucide-react";
import { GitProfile, useProfileStore } from "../stores/useProfileStore";
import { invoke } from "@tauri-apps/api/core";
import { open as openFolderPicker } from "@tauri-apps/plugin-dialog";
import { useToast } from "./ui/useToast";
import ConfirmModal from "./ui/ConfirmModal";

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
  const { deleteProfile, addProfile, loading, applyProfileToRepo } = useProfileStore();
  const toast = useToast();
  const [confirmOpen, setConfirmOpen] = React.useState(false);
  const [confirmBusy, setConfirmBusy] = React.useState(false);
  const [pendingSnapshot, setPendingSnapshot] = React.useState<any>(null);
  const [deleteConfirmOpen, setDeleteConfirmOpen] = React.useState(false);
  const [applyBusy, setApplyBusy] = React.useState(false);
  const [dupBusy, setDupBusy] = React.useState(false);

  return (
    <div
      className={`glass-panel profile-card ${isActive ? "active" : ""}`}
      role="group"
      aria-label={`Profile ${profile.label}`}
    >
      <div className="profile-header">
        <div
          className="profile-avatar"
          style={{
            background: `${profile.color}26`,
            borderColor: `${profile.color}50`,
            color: profile.color,
          }}
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
            {profile.sshKeyPath && (
              <span
                className="detail-item ssh-badge"
                title={`SSH key: ${profile.sshKeyPath}`}
              >
                <KeyRound size={12} /> SSH
              </span>
            )}
            {profile.gpgKeyId && (
              <span
                className="detail-item gpg-badge"
                title={`GPG key: ${profile.gpgKeyId}`}
              >
                <Key size={12} /> GPG
              </span>
            )}
            {profile.remoteService && (
              <span
                className={`detail-item remote-badge remote-badge--${profile.remoteService}`}
                title={profile.remoteUrl ?? profile.remoteService}
              >
                {profile.remoteService.charAt(0).toUpperCase() +
                  profile.remoteService.slice(1)}
              </span>
            )}
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
          <>
            <button
              className="btn btn-primary"
              onClick={async () => {
                try {
                  setConfirmBusy(true);
                  const snapshot = await invoke<any>(
                    "snapshot_global_git_config",
                  );
                  setPendingSnapshot(snapshot);
                  setConfirmBusy(false);
                  setConfirmOpen(true);
                } catch (e: any) {
                  setConfirmBusy(false);
                  toast.show({
                    message: `Failed to prepare switch: ${e}`,
                    kind: "error",
                  });
                }
              }}
              disabled={loading}
            >
              Switch to Profile
            </button>

            <ConfirmModal
              open={confirmOpen}
              title={`Switch global Git identity?`}
              description={`Switch global Git identity to ${profile.name} <${profile.email}>? This updates your global git config.`}
              confirmLabel="Switch"
              cancelLabel="Cancel"
              busy={confirmBusy}
              onCancel={() => setConfirmOpen(false)}
              onConfirm={async () => {
                setConfirmBusy(true);
                try {
                  await invoke("switch_profile_globally", { id: profile.id });
                  await useProfileStore.getState().fetchProfiles();

                  toast.show({
                    message: `Switched to ${profile.label}`,
                    kind: "success",
                    actions: [
                      {
                        label: "Undo",
                        onClick: async () => {
                          try {
                            if (pendingSnapshot) {
                              await invoke("restore_global_git_config", {
                                snapshot: pendingSnapshot,
                              });
                              await useProfileStore.getState().fetchProfiles();
                              toast.show({
                                message: "Restored previous Git config",
                                kind: "success",
                              });
                            }
                          } catch (err) {
                            toast.show({
                              message: `Restore failed: ${err}`,
                              kind: "error",
                            });
                          }
                        },
                      },
                    ],
                  });
                } catch (e: any) {
                  toast.show({ message: `Switch failed: ${e}`, kind: "error" });
                } finally {
                  setConfirmBusy(false);
                  setConfirmOpen(false);
                  setPendingSnapshot(null);
                }
              }}
            />
          </>
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
          className="btn btn-secondary"
          type="button"
          disabled={dupBusy || loading}
          aria-label={`Duplicate ${profile.label}`}
          title="Duplicate profile"
          onClick={async () => {
            setDupBusy(true);
            try {
              const {
                id: _id,
                isDefault: _def,
                remoteUrl: _ru,
                remoteService: _rs,
                ...rest
              } = profile;
              await addProfile({
                ...rest,
                label: `Copy of ${profile.label}`,
                isDefault: false,
              });
              toast.show({
                message: `Duplicated ${profile.label}`,
                kind: "success",
              });
            } catch (e: any) {
              toast.show({ message: `Duplicate failed: ${e}`, kind: "error" });
            } finally {
              setDupBusy(false);
            }
          }}
        >
          <Copy size={16} /> {dupBusy ? "Copying…" : "Duplicate"}
        </button>
        <button
          className="btn btn-secondary"
          type="button"
          disabled={applyBusy}
          aria-label={`Apply ${profile.label} to a repository`}
          title="Pick a repository folder and apply this profile's name, email, SSH key and GPG key to its local git config — overrides global config for that repo only."
          onClick={async () => {
            setApplyBusy(true);
            try {
              const selected = await openFolderPicker({
                multiple: false,
                directory: true,
                title: "Select repository folder",
              });
              if (!selected) {
                setApplyBusy(false);
                return;
              }
              await applyProfileToRepo(profile.id, selected as string);
              toast.show({
                message: `Applied ${profile.label} to repo`,
                kind: "success",
              });
            } catch (e: any) {
              toast.show({ message: `Apply failed: ${e}`, kind: "error" });
            } finally {
              setApplyBusy(false);
            }
          }}
        >
          <FolderInput size={16} /> {applyBusy ? "Applying…" : "Apply to Repo"}
        </button>
        <button
          className="btn-icon delete-btn"
          onClick={() => setDeleteConfirmOpen(true)}
          title="Delete Profile"
          aria-label={`Delete ${profile.label}`}
          disabled={loading}
        >
          <Trash2 size={16} />
        </button>
      </div>

      <ConfirmModal
        open={deleteConfirmOpen}
        title="Delete profile?"
        description={`Delete "${profile.label}"? This cannot be undone.`}
        confirmLabel="Delete"
        cancelLabel="Cancel"
        busy={loading}
        onCancel={() => setDeleteConfirmOpen(false)}
        onConfirm={async () => {
          await deleteProfile(profile.id);
          setDeleteConfirmOpen(false);
          toast.show({ message: `Deleted ${profile.label}`, kind: "success" });
        }}
      />
    </div>
  );
};
