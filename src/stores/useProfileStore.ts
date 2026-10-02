import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { toUserMessage } from "../utils/error";

export interface GitProfile {
  id: string;
  label: string;
  name: string;
  email: string;
  color: string;
  sshKeyPath?: string;
  gpgKeyId?: string;
  isDefault: boolean;
  sshHostAlias?: string;
  sshHostname?: string;
}

export interface DirectoryRule {
  path: string;
  profileId: string;
}

export interface EffectiveIdentity {
  name?: string | null;
  email?: string | null;
}

interface ProfileState {
  directoryRules: DirectoryRule[];
  profiles: GitProfile[];
  loading: boolean;
  error: string | null;
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
  detectIdentities: (directory?: string) => Promise<void>;
  fetchDirectoryRules: () => Promise<void>;
  addDirectoryRule: (path: string, profileId: string) => Promise<void>;
  removeDirectoryRule: (path: string) => Promise<void>;
  previewDirectory: (path: string) => Promise<EffectiveIdentity>;
}

// Mutating actions record a readable message in `error` and rethrow the original
// error so callers can show a toast (see useErrorToast).
export const useProfileStore = create<ProfileState>((set, get) => ({
  profiles: [],
  directoryRules: [],
  loading: false,
  error: null,
  detectedProfiles: [],
  detectLoading: false,
  detectError: null,

  fetchProfiles: async () => {
    set({ loading: true, error: null });
    try {
      const profiles = await invoke<GitProfile[]>("get_profiles");
      set({ profiles, loading: false });
    } catch (e) {
      set({ error: toUserMessage(e), loading: false });
      throw e;
    }
  },

  addProfile: async (profileDraft) => {
    set({ loading: true, error: null });
    try {
      const created = await invoke<GitProfile>("add_profile", {
        profile: { id: "", ...profileDraft },
      });
      await get().fetchProfiles();
      return created;
    } catch (e) {
      set({ error: toUserMessage(e), loading: false });
      throw e;
    }
  },

  findExistingProfile: (name?: string, email?: string) => {
    if (!name && !email) return undefined;
    const norm = (s: string) => s.trim().toLowerCase();
    return get().profiles.find(
      (p) =>
        (!name || norm(p.name) === norm(name)) &&
        (!email || norm(p.email) === norm(email)),
    );
  },

  updateProfile: async (profile) => {
    set({ loading: true, error: null });
    try {
      await invoke("update_profile", { profile });
      await get().fetchProfiles();
    } catch (e) {
      set({ error: toUserMessage(e), loading: false });
      throw e;
    }
  },

  deleteProfile: async (id) => {
    set({ loading: true, error: null });
    try {
      await invoke("delete_profile", { id });
      // the backend also drops this profile's directory rules
      await Promise.all([get().fetchProfiles(), get().fetchDirectoryRules()]);
    } catch (e) {
      set({ error: toUserMessage(e), loading: false });
      throw e;
    }
  },

  switchProfileGlobally: async (id) => {
    set({ error: null });
    try {
      await invoke("switch_profile_globally", { id });
    } catch (e) {
      set({ error: toUserMessage(e) });
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
      set({ detectError: toUserMessage(e), detectLoading: false });
      // rethrow so callers can show a toast with the hint
      throw e;
    }
  },

  fetchDirectoryRules: async () => {
    try {
      const directoryRules = await invoke<DirectoryRule[]>(
        "get_directory_rules",
      );
      set({ directoryRules });
    } catch (e) {
      set({ error: toUserMessage(e) });
      throw e;
    }
  },

  addDirectoryRule: async (path, profileId) => {
    set({ error: null });
    try {
      await invoke("add_directory_rule", { path, profileId });
      await get().fetchDirectoryRules();
    } catch (e) {
      set({ error: toUserMessage(e) });
      throw e;
    }
  },

  removeDirectoryRule: async (path) => {
    set({ error: null });
    try {
      await invoke("remove_directory_rule", { path });
      await get().fetchDirectoryRules();
    } catch (e) {
      set({ error: toUserMessage(e) });
      throw e;
    }
  },

  previewDirectory: (path) => invoke<EffectiveIdentity>("preview_directory", { path }),
}));
