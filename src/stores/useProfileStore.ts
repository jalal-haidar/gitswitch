import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

export interface GitProfile {
  id: string;
  label: string;
  name: string;
  email: string;
  color: string;
  sshKeyPath?: string;
  gpgKeyId?: string;
  isDefault: boolean;
}

interface ProfileState {
  profiles: GitProfile[];
  loading: boolean;
  error: string | null;
  detectedProfiles: GitProfile[];
  detectLoading: boolean;
  detectError: string | null;
  fetchProfiles: () => Promise<void>;
  addProfile: (profile: Omit<GitProfile, 'id'>) => Promise<void>;
  updateProfile: (profile: GitProfile) => Promise<void>;
  deleteProfile: (id: string) => Promise<void>;
  switchProfileGlobally: (id: string) => Promise<void>;
  detectIdentities: (directory?: string) => Promise<void>;
}

export const useProfileStore = create<ProfileState>((set, get) => ({
  profiles: [],
  loading: false,
  error: null,
  detectedProfiles: [],
  detectLoading: false,
  detectError: null,

  fetchProfiles: async () => {
    set({ loading: true, error: null });
    try {
      const profiles = await invoke<GitProfile[]>('get_profiles');
      set({ profiles, loading: false });
    } catch (e: any) {
      set({ error: e.toString(), loading: false });
    }
  },

  addProfile: async (profileDraft) => {
    set({ loading: true, error: null });
    try {
      await invoke('add_profile', { profile: { id: '', ...profileDraft } });
      await get().fetchProfiles();
    } catch (e: any) {
      set({ error: e.toString(), loading: false });
    }
  },

  updateProfile: async (profile) => {
    set({ loading: true, error: null });
    try {
      await invoke('update_profile', { profile });
      await get().fetchProfiles();
    } catch (e: any) {
      set({ error: e.toString(), loading: false });
    }
  },

  deleteProfile: async (id) => {
    set({ loading: true, error: null });
    try {
      await invoke('delete_profile', { id });
      await get().fetchProfiles();
    } catch (e: any) {
      set({ error: e.toString(), loading: false });
    }
  },

  switchProfileGlobally: async (id) => {
    set({ error: null });
    try {
      await invoke('switch_profile_globally', { id });
    } catch (e: any) {
      set({ error: e.toString() });
    }
  }

  ,detectIdentities: async (directory?: string) => {
    set({ detectLoading: true, detectError: null });
    try {
      const detected = await invoke<GitProfile[]>('detect_identities', { directory });
      set({ detectedProfiles: detected, detectLoading: false });
    } catch (e: any) {
      set({ detectError: e.toString(), detectLoading: false });
    }
  }
}));
