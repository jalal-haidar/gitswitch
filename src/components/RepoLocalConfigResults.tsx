import React from "react";
import type { RepoLocalConfig, ScannedRepo } from "../stores/useProfileStore";

export interface RepoLocalConfigResultItem {
  repo: Pick<ScannedRepo, "name" | "path">;
  config: RepoLocalConfig;
}

interface RepoLocalConfigResultsProps {
  items: RepoLocalConfigResultItem[];
  selectedCount: number;
  error?: string | null;
  onClear: () => void;
  onDismiss: (repoPath: string) => void;
}

const CONFIG_FIELDS: Array<{ key: keyof RepoLocalConfig; label: string }> = [
  { key: "userName", label: "user.name" },
  { key: "userEmail", label: "user.email" },
  { key: "userSigningkey", label: "user.signingkey" },
  { key: "commitGpgsign", label: "commit.gpgsign" },
  { key: "coreSshCommand", label: "core.sshCommand" },
];

const RepoLocalConfigResults: React.FC<RepoLocalConfigResultsProps> = ({
  items,
  selectedCount,
  error,
  onClear,
  onDismiss,
}) => {
  const summary =
    items.length === selectedCount
      ? `Showing local config for ${selectedCount} selected repo${selectedCount === 1 ? "" : "s"}.`
      : `Showing ${items.length} of ${selectedCount} selected repo${selectedCount === 1 ? "" : "s"}.`;

  return (
    <section
      className="repo-config-results"
      aria-labelledby="repo-config-results-heading"
    >
      <div className="repo-config-results-header">
        <div>
          <h3 id="repo-config-results-heading">Selected Local Git Config</h3>
          <p className="muted repo-config-results-meta">{summary}</p>
        </div>
        <button
          className="btn btn-ghost btn-sm"
          type="button"
          onClick={onClear}
        >
          Clear Results
        </button>
      </div>

      {error && (
        <div className="repo-config-results-error" role="alert">
          {error}
        </div>
      )}

      <div className="repo-config-results-grid">
        {items.map(({ repo, config }) => (
          <article key={repo.path} className="glass-panel repo-config-card">
            <div className="repo-config-card-header">
              <div className="repo-config-card-copy">
                <strong className="repo-config-card-title">{repo.name}</strong>
                <span className="muted repo-config-card-path" title={repo.path}>
                  {repo.path}
                </span>
              </div>
              <button
                className="btn-icon"
                type="button"
                onClick={() => onDismiss(repo.path)}
                aria-label={`Remove ${repo.name} from local config results`}
              >
                ✕
              </button>
            </div>

            <dl className="repo-config-list">
              {CONFIG_FIELDS.map((field) => (
                <div key={field.key} className="repo-config-item">
                  <dt>{field.label}</dt>
                  <dd>{config[field.key] ?? <em>not set</em>}</dd>
                </div>
              ))}
            </dl>
          </article>
        ))}
      </div>
    </section>
  );
};

export default RepoLocalConfigResults;
