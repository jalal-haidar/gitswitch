import React from 'react';
import { useProfileStore, GitProfile } from '../stores/useProfileStore';
import { invoke } from '@tauri-apps/api/core';

export const DetectedProfilesList: React.FC = () => {
  const { detectedProfiles, detectLoading, detectError, detectIdentities, addProfile, switchProfileGlobally } = useProfileStore();

  const handleImport = async (p: GitProfile) => {
    const existing = useProfileStore.getState().findExistingProfile(p.name, p.email);
    if (existing) {
      // already exists - nothing to do
      return existing;
    }
    const created = await addProfile({ label: p.label || 'Imported', name: p.name, email: p.email, color: p.color || '#6A5ACD', isDefault: false, sshKeyPath: p.sshKeyPath, gpgKeyId: p.gpgKeyId });
    return created;
  };

  const handleApply = async (p: GitProfile) => {
    try {
      await invoke('apply_identity', { name: p.name, email: p.email, gpg_key: p.gpgKeyId ?? null });
    } catch (e) {
      // propagate or set error state — kept minimal for now
      console.error('apply_identity failed', e);
    }
  };

  return (
    <div className="detected-list">
      <div className="section-header">
        <h3>Detected Identities</h3>
        <button className="btn" onClick={() => detectIdentities() } disabled={detectLoading}>
          {detectLoading ? 'Scanning…' : 'Refresh'}
        </button>
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
                <div className="muted">{p.name} {p.email ? `<${p.email}>` : ''}</div>
                {p.sshKeyPath && <div className="muted">SSH: {p.sshKeyPath}</div>}
              </div>
              <div className="detected-actions">
                <button className="btn btn-secondary" onClick={() => handleImport(p)}>Import</button>
                <button className="btn btn-primary" onClick={() => handleApply(p)}>Apply</button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

export default DetectedProfilesList;
