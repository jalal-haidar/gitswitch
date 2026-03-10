import React, { useEffect } from 'react';
import { Plus, Users } from 'lucide-react';
import { useProfileStore } from '../stores/useProfileStore';
import { ProfileCard } from './ProfileCard';

export const Dashboard: React.FC = () => {
  const { profiles, loading, fetchProfiles, addProfile } = useProfileStore();

  useEffect(() => {
    fetchProfiles();
  }, [fetchProfiles]);

  const handleAddDemo = () => {
    addProfile({
      label: 'Personal',
      name: 'John Doe',
      email: 'john@doe.com',
      color: '#7C3AED',
      isDefault: true
    });
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
          <button className="btn btn-primary" onClick={handleAddDemo}>
            <Plus size={18} /> New Profile
          </button>
        </div>

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
              />
            ))}
          </div>
        )}
      </section>
    </div>
  );
};
