import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { friendlyErrorMessage } from "../utils/error";

export interface GitProfile {
  id: string;
  label: string;
  name: string;
  email: string;
  color: string;
  sshKeyPath?: string;
  gpgKeyId?: string;
  isDefault: boolean;
  /** Populated only by detect/scan — never persisted to profiles.json */
  remoteUrl?: string;
  remoteService?: string;
}

export interface DirectoryRule {
  id: string;
  path: string;
  profileId: string;
  /** Epoch-ms timestamp of the last auto-switch that fired for this rule */
  lastTriggeredAt?: number;
}

export interface ScannedRepo {
  path: string;
  name: string;
  userName?: string;
  userEmail?: string;
  remoteUrl?: string;
  remoteService?: string;
  appliedProfileId?: string;
  /** Repo-local core.sshCommand, if configured */
  sshCommand?: string;
}

export type RepoApplySource = "manual" | "auto";

export interface RepoApplyEvent {
  profileId: string;
  profileLabel: string;
  repositoryPath: string;
  source: RepoApplySource;
  occurredAtEpochMs: number;
}

/** Current values actually written in a repo's .git/config — proof of a switch. */
export interface RepoLocalConfig {
  userName?: string;
  userEmail?: string;
  userSigningkey?: string;
  commitGpgsign?: string;
  coreSshCommand?: string;
  appliedProfileId?: string;
}

interface ProfileState {
  profiles: GitProfile[];
  directoryRules: DirectoryRule[];
  autoSwitchEnabled: boolean;
  autoSwitchLoading: boolean;
  lastRepoActivity: RepoApplyEvent | null;
  globalActiveProfileId: string | null;
  hasGlobalSnapshot: boolean;
  loading: boolean;
  rulesLoading: boolean;
  error: string | null;
  rulesError: string | null;
  detectedProfiles: GitProfile[];
  detectLoading: boolean;
  detectError: string | null;
  fetchProfiles: () => Promise<void>;
  addProfile: (profile: Omit<GitProfile, "id">) => Promise<GitProfile>;
  findExistingProfile: (
    name?: string,
    email?: string,
  ) => GitProfile | undefined;
  updateProfile: (profile: GitProfile) => Promise<void>;
  deleteProfile: (id: string) => Promise<void>;
  switchProfileGlobally: (id: string) => Promise<void>;
  restoreGlobalSnapshot: () => Promise<void>;
  discardGlobalSnapshot: () => Promise<void>;
  detectIdentities: (directory?: string) => Promise<void>;
  fetchAutoSwitchSetting: () => Promise<void>;
  setAutoSwitchEnabled: (enabled: boolean) => Promise<void>;
  fetchLastRepoActivity: () => Promise<void>;
  setLastRepoActivity: (event: RepoApplyEvent) => void;
  fetchDirectoryRules: () => Promise<void>;
  addDirectoryRule: (rule: Omit<DirectoryRule, "id">) => Promise<DirectoryRule>;
  updateDirectoryRule: (rule: DirectoryRule) => Promise<void>;
  deleteDirectoryRule: (id: string) => Promise<void>;
  applyProfileToRepo: (
    profileId: string,
    repoPath: string,
  ) => Promise<RepoApplyEvent>;
  getTheme: () => Promise<string>;
  setTheme: (theme: string) => Promise<void>;
  scanRepos: (root: string, maxDepth?: number) => Promise<ScannedRepo[]>;
  restoreRepoSnapshot: (repoPath: string) => Promise<void>;
  discardRepoSnapshot: (repoPath: string) => Promise<void>;
  getRepoLocalConfig: (repoPath: string) => Promise<RepoLocalConfig>;
}

export const useProfileStore = create<ProfileState>((set, get) => ({
  profiles: [],
  directoryRules: [],
  autoSwitchEnabled: true,
  autoSwitchLoading: false,
  lastRepoActivity: null,
  globalActiveProfileId: null,
  hasGlobalSnapshot: false,
  loading: false,
  rulesLoading: false,
  error: null,
  rulesError: null,
  detectedProfiles: [],
  detectLoading: false,
  detectError: null,

  fetchAutoSwitchSetting: async () => {
    set({ autoSwitchLoading: true, rulesError: null });
    try {
      const autoSwitchEnabled = await invoke<boolean>(
        "get_auto_switch_enabled",
      );
      set({ autoSwitchEnabled, autoSwitchLoading: false });
    } catch (e) {
      set({ rulesError: friendlyErrorMessage(e), autoSwitchLoading: false });
      throw e;
    }
  },

  setAutoSwitchEnabled: async (enabled) => {
    set({ autoSwitchLoading: true, rulesError: null });
    try {
      const autoSwitchEnabled = await invoke<boolean>(
        "set_auto_switch_enabled",
        {
          enabled,
        },
      );
      set({ autoSwitchEnabled, autoSwitchLoading: false });
    } catch (e) {
      set({ rulesError: friendlyErrorMessage(e), autoSwitchLoading: false });
      throw e;
    }
  },

  fetchLastRepoActivity: async () => {
    try {
      const lastRepoActivity = await invoke<RepoApplyEvent | null>(
        "get_last_repo_activity",
      );
      set({ lastRepoActivity });
    } catch {
      // Activity status should not break the rest of the UI.
    }
  },

  setLastRepoActivity: (lastRepoActivity) => set({ lastRepoActivity }),

  fetchProfiles: async () => {
    set({ loading: true, error: null });
    try {
      const [profiles, globalActiveProfileId, hasGlobalSnapshot] =
        await Promise.all([
        invoke<GitProfile[]>("get_profiles"),
        invoke<string | null>("get_global_active_profile_id"),
        invoke<boolean>("has_global_snapshot"),
      ]);
      set({
        profiles,
        globalActiveProfileId,
        hasGlobalSnapshot,
        loading: false,
      });
    } catch (e) {
      set({ error: friendlyErrorMessage(e), loading: false });
    }
  },

  addProfile: async (profileDraft) => {
    set({ loading: true, error: null });
    try {
      const created = await invoke<GitProfile>("add_profile", {
        profile: { id: "", ...profileDraft },
      });
      await get().fetchProfiles();
      set({ loading: false });
      return created;
    } catch (e) {
      set({ error: friendlyErrorMessage(e), loading: false });
      throw e;
    }
  },

  findExistingProfile: (name?: string, email?: string) => {
    const ps = get().profiles;
    if (!name && !email) return undefined;
    return ps.find((p) => {
      const matchesName = name
        ? p.name.trim().toLowerCase() === name.trim().toLowerCase()
        : true;
      const matchesEmail = email
        ? p.email.trim().toLowerCase() === email.trim().toLowerCase()
        : true;
      return matchesName && matchesEmail;
    });
  },

  updateProfile: async (profile) => {
    set({ loading: true, error: null });
    try {
      await invoke("update_profile", { profile });
      await get().fetchProfiles();
      set({ loading: false });
    } catch (e) {
      set({ error: friendlyErrorMessage(e), loading: false });
      throw e;
    }
  },

  deleteProfile: async (id) => {
    set({ loading: true, error: null });
    try {
      await invoke("delete_profile", { id });
      await get().fetchProfiles();
      set({ loading: false });
    } catch (e) {
      set({ error: friendlyErrorMessage(e), loading: false });
      throw e;
    }
  },

  switchProfileGlobally: async (id) => {
    set({ loading: true, error: null });
    try {
      await invoke("switch_profile_globally", { id });
      await get().fetchProfiles();
      set({ loading: false });
    } catch (e) {
      set({ error: friendlyErrorMessage(e), loading: false });
      throw e;
    }
  },

  restoreGlobalSnapshot: async () => {
    set({ loading: true, error: null });
    try {
      await invoke("restore_global_snapshot");
      await get().fetchProfiles();
      set({ loading: false });
    } catch (e) {
      set({ error: friendlyErrorMessage(e), loading: false });
      throw e;
    }
  },

  discardGlobalSnapshot: async () => {
    try {
      await invoke("discard_global_snapshot");
      set({ hasGlobalSnapshot: false });
    } catch (e) {
      set({ error: friendlyErrorMessage(e) });
      throw e;
    }
  },

  detectIdentities: async (directory?: string) => {
    set({ detectLoading: true, detectError: null });
    try {
      const detected = await invoke<GitProfile[]>("detect_identities", {
        directory,
      });
      set({ detectedProfiles: detected, detectLoading: false });
    } catch (e) {
      set({ detectError: friendlyErrorMessage(e), detectLoading: false });
      // rethrow so callers (components) can display toasts or handle actions
      throw e;
    }
  },

  fetchDirectoryRules: async () => {
    set({ rulesLoading: true, rulesError: null });
    try {
      const directoryRules = await invoke<DirectoryRule[]>(
        "get_directory_rules",
      );
      set({ directoryRules, rulesLoading: false });
    } catch (e) {
      set({ rulesError: friendlyErrorMessage(e), rulesLoading: false });
      throw e;
    }
  },

  addDirectoryRule: async (ruleDraft) => {
    set({ rulesLoading: true, rulesError: null });
    try {
      const created = await invoke<DirectoryRule>("add_directory_rule", {
        rule: { id: "", ...ruleDraft },
      });
      await get().fetchDirectoryRules();
      set({ rulesLoading: false });
      return created;
    } catch (e) {
      set({ rulesError: friendlyErrorMessage(e), rulesLoading: false });
      throw e;
    }
  },

  updateDirectoryRule: async (rule) => {
    set({ rulesLoading: true, rulesError: null });
    try {
      await invoke("update_directory_rule", { rule });
      await get().fetchDirectoryRules();
      set({ rulesLoading: false });
    } catch (e) {
      set({ rulesError: friendlyErrorMessage(e), rulesLoading: false });
      throw e;
    }
  },

  deleteDirectoryRule: async (id) => {
    set({ rulesLoading: true, rulesError: null });
    try {
      await invoke("delete_directory_rule", { id });
      await get().fetchDirectoryRules();
      set({ rulesLoading: false });
    } catch (e) {
      set({ rulesError: friendlyErrorMessage(e), rulesLoading: false });
      throw e;
    }
  },

  applyProfileToRepo: async (profileId, repoPath) => {
    const event = await invoke<RepoApplyEvent>("apply_profile_to_repo", {
      id: profileId,
      repoPath,
    });
    set({ lastRepoActivity: event });
    return event;
  },

  getTheme: async () => {
    return invoke<string>("get_theme");
  },

  setTheme: async (theme) => {
    await invoke("set_theme", { theme });
    document.documentElement.setAttribute("data-theme", theme);
  },

  scanRepos: async (root, maxDepth?) => {
    return invoke<ScannedRepo[]>("scan_repos", { root, maxDepth });
  },
  restoreRepoSnapshot: async (repoPath) => {
    try {
      await invoke("restore_repo_snapshot", { repoPath });
    } catch (e) {
      set({ error: friendlyErrorMessage(e) });
      throw e;
    }
  },
  discardRepoSnapshot: async (repoPath) => {
    try {
      await invoke("discard_repo_snapshot", { repoPath });
    } catch (e) {
      set({ error: friendlyErrorMessage(e) });
      throw e;
    }
  },
  getRepoLocalConfig: async (repoPath) => {
    return invoke<RepoLocalConfig>("get_repo_local_config", {
      repoPath,
    });
  },
}));
