import React from "react";

export const ProfileCardSkeleton: React.FC = () => {
  return (
    <div className="glass-panel profile-card skeleton">
      <div className="profile-header">
        <div className="skeleton-avatar" />
        <div className="skeleton-text skeleton-text-title" />
      </div>
      <div className="profile-details">
        <div className="skeleton-text skeleton-text-short" />
        <div className="skeleton-text skeleton-text-medium" />
      </div>
      <div className="profile-actions">
        <div className="skeleton-button" />
        <div className="skeleton-button" />
      </div>
    </div>
  );
};

export const RuleCardSkeleton: React.FC = () => {
  return (
    <div className="glass-panel rule-item skeleton">
      <div className="skeleton-text skeleton-text-medium" />
      <div className="skeleton-text skeleton-text-short" />
      <div className="rule-actions">
        <div className="skeleton-button skeleton-button-small" />
        <div className="skeleton-button skeleton-button-small" />
      </div>
    </div>
  );
};
