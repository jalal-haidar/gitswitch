import React from 'react';
import { useProfileStore, GitProfile } from '../stores/useProfileStore';

export const DetectedProfilesList: React.FC = () => {
  const { detectedProfiles, detectLoading, detectError, detectIdentities, addProfile, switchProfileGlobally } = useProfileStore();

  const handleImport = async (p: GitProfile) => {
    await addProfile({ label: p.label || 'Imported', name: p.name, email: p.email, color: p.color || '#6A5ACD', isDefault: false, sshKeyPath: p.sshKeyPath, gpgKeyId: p.gpgKeyId });
  };

  const handleApply = async (p: GitProfile) => {
    // Create a temporary profile then apply using switchProfileGlobally by adding then finding id
    await addProfile({ label: p.label || 'Temp', name: p.name, email: p.email, color: p.color || '#6A5ACD', isDefault: false, sshKeyPath: p.sshKeyPath, gpgKeyId: p.gpgKeyId });
    // refresh and find the newly added profile id
    const state = useProfileStore.getState();
    const matched = state.profiles.find(pr => pr.name === p.name && pr.email === p.email);
    if (matched) await switchProfileGlobally(matched.id);
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
