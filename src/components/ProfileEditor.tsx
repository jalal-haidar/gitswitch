import React, { useMemo, useState } from "react";
import { open as openFilePicker } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import type { GitProfile } from "../stores/useProfileStore";

const HelpTooltip: React.FC<{ text: string }> = ({ text }) => (
  <span className="help-tooltip" aria-label={text} tabIndex={0}>
    ?
    <span className="help-tooltip-bubble" role="tooltip">
      {text}
    </span>
  </span>
);

interface SshTestResult {
  success: boolean;
  username: string | null;
  message: string;
}

export interface ProfileEditorValue {
  id?: string;
  label: string;
  name: string;
  email: string;
  color: string;
  sshKeyPath?: string;
  gpgKeyId?: string;
  isDefault: boolean;
}

interface ProfileEditorProps {
  initialValue?: ProfileEditorValue;
  submitLabel: string;
  busy?: boolean;
  isDuplicate?: (value: ProfileEditorValue) => boolean;
  onSubmit: (value: ProfileEditorValue) => Promise<void> | void;
  onCancel: () => void;
}

const emptyProfile: ProfileEditorValue = {
  label: "",
  name: "",
  email: "",
  color: "#7C3AED",
  sshKeyPath: "",
  gpgKeyId: "",
  isDefault: false,
};

export const toEditorValue = (profile: GitProfile): ProfileEditorValue => ({
  id: profile.id,
  label: profile.label,
  name: profile.name,
  email: profile.email,
  color: profile.color,
  sshKeyPath: profile.sshKeyPath ?? "",
  gpgKeyId: profile.gpgKeyId ?? "",
  isDefault: profile.isDefault,
});

export const ProfileEditor: React.FC<ProfileEditorProps> = ({
  initialValue,
  submitLabel,
  busy = false,
  isDuplicate,
  onSubmit,
  onCancel,
}) => {
  const [value, setValue] = useState<ProfileEditorValue>(
    initialValue ?? emptyProfile,
  );
  const [touched, setTouched] = useState(false);
  const [sshTestStatus, setSshTestStatus] = useState<SshTestResult | null>(
    null,
  );
  const [sshTesting, setSshTesting] = useState(false);

  const testSshConnection = async () => {
    if (!value.sshKeyPath?.trim()) return;
    setSshTesting(true);
    setSshTestStatus(null);
    try {
      const result = await invoke<SshTestResult>("test_ssh_connection", {
        keyPath: value.sshKeyPath.trim(),
        host: null,
      });
      setSshTestStatus(result);
    } catch (e: any) {
      setSshTestStatus({ success: false, username: null, message: String(e) });
    } finally {
      setSshTesting(false);
    }
  };

  const emailValid = useMemo(
    () => /.+@.+\..+/.test(value.email.trim()),
    [value.email],
  );
  const duplicateExists = useMemo(
    () => isDuplicate?.(value) ?? false,
    [isDuplicate, value],
  );
  const canSubmit =
    value.label.trim() !== "" &&
    value.name.trim() !== "" &&
    value.email.trim() !== "" &&
    emailValid &&
    !duplicateExists;

  const setField = <K extends keyof ProfileEditorValue>(
    key: K,
    nextValue: ProfileEditorValue[K],
  ) => {
    setTouched(true);
    setValue((current) => ({ ...current, [key]: nextValue }));
  };

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    setTouched(true);
    if (!canSubmit) return;
    await onSubmit({
      ...value,
      label: value.label.trim(),
      name: value.name.trim(),
      email: value.email.trim(),
      sshKeyPath: value.sshKeyPath?.trim() || undefined,
      gpgKeyId: value.gpgKeyId?.trim() || undefined,
    });
  };

  return (
    <form className="glass-panel profile-editor" onSubmit={handleSubmit}>
      <div className="profile-editor-grid">
        <label className="field-group" htmlFor="profile-label">
          <span>Label</span>
          <input
            id="profile-label"
            aria-label="Profile label"
            aria-invalid={touched && !value.label.trim()}
            value={value.label}
            onChange={(event) => setField("label", event.target.value)}
            placeholder="Work"
            autoFocus
          />
          {touched && !value.label.trim() && (
            <span className="field-error" role="alert">
              Label is required
            </span>
          )}
        </label>
        <label className="field-group" htmlFor="profile-name">
          <span>Name</span>
          <input
            id="profile-name"
            aria-label="Full name"
            aria-invalid={touched && !value.name.trim()}
            value={value.name}
            onChange={(event) => setField("name", event.target.value)}
            placeholder="Jane Doe"
          />
          {touched && !value.name.trim() && (
            <span className="field-error" role="alert">
              Name is required
            </span>
          )}
        </label>
        <label className="field-group" htmlFor="profile-email">
          <span>Email</span>
          <input
            id="profile-email"
            aria-label="Email address"
            aria-required="true"
            aria-invalid={touched && (!value.email.trim() || !emailValid)}
            value={value.email}
            onChange={(event) => setField("email", event.target.value)}
            placeholder="jane@example.com"
            type="email"
          />
          {touched && !value.email.trim() && (
            <span className="field-error" role="alert">
              Email is required
            </span>
          )}
          {touched && value.email.trim() && !emailValid && (
            <span className="field-error" role="alert">
              Enter a valid email address
            </span>
          )}
          {touched && duplicateExists && (
            <span className="field-error" role="alert">
              A profile with this name and email already exists
            </span>
          )}
        </label>
        <label className="field-group" htmlFor="profile-color">
          <span>Color</span>
          <div className="color-field-row">
            <input
              id="profile-color"
              className="color-input"
              aria-label="Profile color"
              value={value.color}
              onChange={(event) => setField("color", event.target.value)}
              type="color"
            />
            <input
              aria-label="Profile color hex"
              value={value.color}
              onChange={(event) => setField("color", event.target.value)}
              placeholder="#7C3AED"
            />
          </div>
        </label>
        <label className="field-group field-group--wide" htmlFor="profile-ssh">
          <span>
            SSH Key Path{" "}
            <HelpTooltip text="Path to your SSH private key (e.g. C:\Users\you\.ssh\id_ed25519). Must be inside your home directory. Generate one with: ssh-keygen -t ed25519 -C your@email.com — then add the .pub file to GitHub → Settings → SSH keys." />
          </span>
          <div className="file-picker-row">
            <input
              id="profile-ssh"
              aria-label="SSH key path"
              value={value.sshKeyPath ?? ""}
              onChange={(event) => setField("sshKeyPath", event.target.value)}
              placeholder="C:\\Users\\you\\.ssh\\id_ed25519"
            />
            <button
              type="button"
              className="btn btn-secondary btn-browse"
              title="Browse for SSH key file"
              onClick={async () => {
                const selected = await openFilePicker({
                  multiple: false,
                  title: "Select SSH Key",
                });
                if (selected) {
                  setField("sshKeyPath", selected as string);
                  setSshTestStatus(null);
                }
              }}
            >
              Browse
            </button>
            {value.sshKeyPath?.trim() && (
              <button
                type="button"
                className="btn btn-secondary btn-browse"
                title="Test SSH connection to GitHub"
                onClick={testSshConnection}
                disabled={sshTesting}
              >
                {sshTesting ? "Testing…" : "Test"}
              </button>
            )}
          </div>
          {sshTestStatus && (
            <div
              className={`ssh-test-status ${sshTestStatus.success ? "ssh-test-ok" : "ssh-test-fail"}`}
              role="status"
            >
              {sshTestStatus.success ? "✓" : "✗"} {sshTestStatus.message}
            </div>
          )}
        </label>
        <label className="field-group" htmlFor="profile-gpg">
          <span>
            GPG Key ID{" "}
            <HelpTooltip text="Short or long ID of your GPG signing key (e.g. F88469E368AE85F0). Find it with: gpg --list-secret-keys --keyid-format SHORT. Used to sign commits — enable Vigilant Mode on GitHub to show a Verified badge on all your commits." />
          </span>
          <input
            id="profile-gpg"
            aria-label="GPG key id"
            value={value.gpgKeyId ?? ""}
            onChange={(event) => setField("gpgKeyId", event.target.value)}
            placeholder="ABC123"
          />
        </label>
      </div>

      <label className="checkbox-row" htmlFor="profile-default">
        <input
          id="profile-default"
          checked={value.isDefault}
          onChange={(event) => setField("isDefault", event.target.checked)}
          type="checkbox"
        />
        <span>Make this the default profile</span>
      </label>

      {touched && value.email.trim() !== "" && !emailValid && (
        <div className="form-error" role="alert" aria-live="assertive">
          Enter a valid email address.
        </div>
      )}
      {duplicateExists && (
        <div className="form-error" role="alert" aria-live="assertive">
          A profile with the same name and email already exists.
        </div>
      )}

      <div className="profile-editor-actions">
        <button className="btn btn-secondary" onClick={onCancel} type="button">
          Cancel
        </button>
        <button
          className="btn btn-primary"
          disabled={!canSubmit || busy}
          type="submit"
        >
          {busy ? "Saving…" : submitLabel}
        </button>
      </div>
    </form>
  );
};

export default ProfileEditor;
