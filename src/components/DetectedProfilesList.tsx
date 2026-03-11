import React, { useState } from "react";
import { useProfileStore, GitProfile } from "../stores/useProfileStore";
import { invoke } from "@tauri-apps/api/core";
import { useToast } from "./ui/useToast";
import { normalizeBackendError } from "../utils/error";

export const DetectedProfilesList: React.FC = () => {
  const detectedProfiles = useProfileStore((s) => s.detectedProfiles);
  const detectLoading = useProfileStore((s) => s.detectLoading);
  const detectError = useProfileStore((s) => s.detectError);
  const addProfile = useProfileStore((s) => s.addProfile);
  const findExistingProfile = useProfileStore((s) => s.findExistingProfile);

  const [importingId, setImportingId] = useState<string | null>(null);
  const [applyingId, setApplyingId] = useState<string | null>(null);
  const toast = useToast();

  const handleImport = async (p: GitProfile) => {
    setImportingId(p.id);
    try {
      const existing = findExistingProfile(p.name, p.email);
      if (existing) {
        toast.show({ message: "Already exists", kind: "info" });
        return;
      }
      const created = await addProfile({
        label: p.label || "Imported",
        name: p.name,
        email: p.email,
        color: p.color || "#6A5ACD",
        isDefault: false,
        sshKeyPath: p.sshKeyPath,
        gpgKeyId: p.gpgKeyId,
      });
      toast.show({ message: `Imported ${created.name}`, kind: "success" });
    } catch (e: any) {
      const info = normalizeBackendError(e?.toString?.() ?? e);
      const actions = [] as { label: string; onClick: () => void }[];
      actions.push({ label: "Retry", onClick: () => handleImport(p) });
      if (info.hint && typeof info.hint === "string") {
        actions.push({
          label: "Help",
          onClick: () => window.open(info.hint as string, "_blank"),
        });
      }

      toast.show({
        message: `Import failed: ${info.message}`,
        kind: "error",
        duration: info.hint ? 10000 : 7000,
        actions,
      });
    } finally {
      setImportingId(null);
    }
  };

  const handleApply = async (p: GitProfile) => {
    setApplyingId(p.id);
    try {
      await invoke("apply_identity", {
        name: p.name,
        email: p.email,
        gpg_key: p.gpgKeyId ?? null,
      });
      toast.show({ message: `Applied ${p.name}`, kind: "success" });
    } catch (e: any) {
      console.error("apply_identity failed", e);
      const info = normalizeBackendError(e?.toString?.() ?? e);
      const actions = [] as { label: string; onClick: () => void }[];
      actions.push({ label: "Retry", onClick: () => handleApply(p) });
      if (info.hint && typeof info.hint === "string") {
        actions.push({
          label: "Help",
          onClick: () => window.open(info.hint as string, "_blank"),
        });
      }
      toast.show({
        message: `Apply failed: ${info.message}`,
        kind: "error",
        duration: info.hint ? 10000 : 7000,
        actions,
      });
    } finally {
      setApplyingId(null);
    }
  };

  return (
    <div className="detected-list">
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
        }}
      >
        <h3>Detected Identities</h3>
        {/* Toaster renders portal-mounted toasts globally */}
      </div>

      {detectError && <div className="error">{detectError}</div>}

      {detectedProfiles.length === 0 ? (
        <div className="empty-state">No identities detected.</div>
      ) : (
        <div className="profile-list">
          {detectedProfiles.map((p) => (
            <div key={p.id} className="detected-item">
              <div className="detected-main">
                <strong>{p.label}</strong>
                <div className="muted">
                  {p.name} {p.email ? `<${p.email}>` : ""}
                </div>
                {p.sshKeyPath && (
                  <div className="muted">SSH: {p.sshKeyPath}</div>
                )}
              </div>
              <div className="detected-actions">
                <button
                  className="btn btn-secondary"
                  onClick={() => handleImport(p)}
                  disabled={!!importingId || detectLoading}
                >
                  {importingId === p.id ? "Importing…" : "Import"}
                </button>
                <button
                  className="btn btn-primary"
                  onClick={() => handleApply(p)}
                  disabled={!!applyingId || detectLoading}
                >
                  {applyingId === p.id ? "Applying…" : "Apply"}
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

export default DetectedProfilesList;
