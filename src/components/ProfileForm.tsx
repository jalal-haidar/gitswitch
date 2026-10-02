import React, { useEffect, useState } from "react";
import { GitProfile, useProfileStore } from "../stores/useProfileStore";
import { useErrorToast } from "./ui/useErrorToast";
import { useToast } from "./ui/useToast";
import "../styles/forms.css";
import {
  applySshAlias,
  generateSshKey,
  listSshKeys,
  readPublicKey,
  SshKeyInfo,
  testSshConnection,
} from "../api/ssh";

interface ProfileFormProps {
  /** Existing profile to edit, or undefined to create a new one. */
  profile?: GitProfile;
  onClose: () => void;
}

const emptyDraft: Omit<GitProfile, "id"> = {
  label: "",
  name: "",
  email: "",
  color: "#7C3AED",
  isDefault: false,
};

// Empty inputs are stored as undefined so the backend sees `None`.
const orUndefined = (v: string) => (v.trim() === "" ? undefined : v.trim());

export const ProfileForm: React.FC<ProfileFormProps> = ({
  profile,
  onClose,
}) => {
  const addProfile = useProfileStore((s) => s.addProfile);
  const updateProfile = useProfileStore((s) => s.updateProfile);
  const fetchProfiles = useProfileStore((s) => s.fetchProfiles);
  const showError = useErrorToast();
  const { show } = useToast();

  const [draft, setDraft] = useState<Omit<GitProfile, "id">>(
    profile ? { ...profile } : emptyDraft,
  );
  const [keys, setKeys] = useState<SshKeyInfo[]>([]);
  const [busy, setBusy] = useState<string | null>(null);

  const editing = !!profile;
  const set = <K extends keyof GitProfile>(k: K, v: GitProfile[K]) =>
    setDraft((d) => ({ ...d, [k]: v }));

  useEffect(() => {
    listSshKeys()
      .then(setKeys)
      .catch(() => setKeys([]));
  }, []);

  const run = async (name: string, fn: () => Promise<void>, prefix: string) => {
    setBusy(name);
    try {
      await fn();
    } catch (e) {
      showError(e, prefix);
    } finally {
      setBusy(null);
    }
  };

  const valid = draft.label.trim() && draft.name.trim() && draft.email.trim();

  const persist = async () => {
    if (editing) await updateProfile({ ...draft, id: profile!.id });
    else await addProfile(draft);
  };

  const handleSave = () =>
    run(
      "save",
      async () => {
        await persist();
        onClose();
      },
      "Could not save profile",
    );

  const handleGenerate = () =>
    run(
      "generate",
      async () => {
        const res = await generateSshKey(profile!.id);
        set("sshKeyPath", res.privateKeyPath);
        await fetchProfiles();
        await navigator.clipboard?.writeText(res.publicKey).catch(() => {});
        show({
          message: "Key generated. Public key copied — add it to your Git host.",
          kind: "success",
          duration: 7000,
        });
      },
      "Could not generate key",
    );

  const handleCopyPublic = () =>
    run(
      "copy",
      async () => {
        const pub = await readPublicKey(draft.sshKeyPath!);
        await navigator.clipboard.writeText(pub);
        show({ message: "Public key copied", kind: "success" });
      },
      "Could not read public key",
    );

  const handleApplyAlias = () =>
    run(
      "alias",
      async () => {
        await persist(); // the backend reads the saved profile
        await applySshAlias(profile!.id);
        show({ message: "Wrote host entry to ~/.ssh/config", kind: "success" });
      },
      "Could not write SSH config",
    );

  const handleTest = () =>
    run(
      "test",
      async () => {
        const host = draft.sshHostAlias || draft.sshHostname || "github.com";
        const res = await testSshConnection(host);
        show({
          message: res.success ? `Connected: ${res.message}` : res.message,
          kind: res.success ? "success" : "error",
          duration: 8000,
        });
      },
      "Connection test failed",
    );

  return (
    <div className="glass-panel profile-form">
      <h3>{editing ? "Edit profile" : "New profile"}</h3>

      <div className="form-grid">
        <label>
          Label
          <input
            value={draft.label}
            onChange={(e) => set("label", e.target.value)}
            placeholder="Work"
          />
        </label>
        <label>
          Name
          <input
            value={draft.name}
            onChange={(e) => set("name", e.target.value)}
            placeholder="Jane Doe"
          />
        </label>
        <label>
          Email
          <input
            type="email"
            value={draft.email}
            onChange={(e) => set("email", e.target.value)}
            placeholder="jane@work.dev"
          />
        </label>
        <label>
          GPG key ID
          <input
            value={draft.gpgKeyId ?? ""}
            onChange={(e) => set("gpgKeyId", orUndefined(e.target.value))}
            placeholder="optional"
          />
        </label>
        <label>
          Color
          <input
            type="color"
            value={draft.color}
            onChange={(e) => set("color", e.target.value)}
          />
        </label>
        <label className="inline">
          <input
            type="checkbox"
            checked={draft.isDefault}
            onChange={(e) => set("isDefault", e.target.checked)}
          />
          Default profile
        </label>
      </div>

      <h4>SSH</h4>
      {!editing && (
        <p className="muted">Save the profile first to generate keys.</p>
      )}
      <div className="form-grid">
        <label>
          Key
          <input
            value={draft.sshKeyPath ?? ""}
            onChange={(e) => set("sshKeyPath", orUndefined(e.target.value))}
            placeholder="~/.ssh/id_ed25519"
          />
        </label>
        <label>
          Existing keys
          <select
            value=""
            onChange={(e) => e.target.value && set("sshKeyPath", e.target.value)}
          >
            <option value="">Choose…</option>
            {keys.map((k) => (
              <option key={k.path} value={k.path}>
                {k.name}
              </option>
            ))}
          </select>
        </label>
        <label>
          Host alias
          <input
            value={draft.sshHostAlias ?? ""}
            onChange={(e) => set("sshHostAlias", orUndefined(e.target.value))}
            placeholder="github-work"
          />
        </label>
        <label>
          Host name
          <input
            value={draft.sshHostname ?? ""}
            onChange={(e) => set("sshHostname", orUndefined(e.target.value))}
            placeholder="github.com"
          />
        </label>
      </div>

      <div className="form-actions ssh-actions">
        <button
          className="btn btn-secondary"
          onClick={handleGenerate}
          disabled={!editing || !!busy}
        >
          {busy === "generate" ? "Generating…" : "Generate key"}
        </button>
        <button
          className="btn btn-secondary"
          onClick={handleCopyPublic}
          disabled={!draft.sshKeyPath || !!busy}
        >
          Copy public key
        </button>
        <button
          className="btn btn-secondary"
          onClick={handleApplyAlias}
          disabled={
            !editing || !draft.sshKeyPath || !draft.sshHostAlias || !!busy
          }
          title="Adds a Host entry to ~/.ssh/config"
        >
          Write SSH config
        </button>
        <button
          className="btn btn-secondary"
          onClick={handleTest}
          disabled={!!busy}
        >
          {busy === "test" ? "Testing…" : "Test connection"}
        </button>
      </div>

      <div className="form-actions">
        <button className="btn btn-ghost" onClick={onClose} disabled={!!busy}>
          Cancel
        </button>
        <button
          className="btn btn-primary"
          onClick={handleSave}
          disabled={!valid || !!busy}
        >
          {busy === "save" ? "Saving…" : "Save"}
        </button>
      </div>
    </div>
  );
};

export default ProfileForm;
