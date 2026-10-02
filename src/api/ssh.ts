import { invoke } from "@tauri-apps/api/core";
import type { GitProfile } from "../stores/useProfileStore";

export interface SshKeyInfo {
  path: string;
  name: string;
}

export interface GeneratedKey {
  privateKeyPath: string;
  publicKey: string;
  profile: GitProfile;
}

export interface SshTestResult {
  success: boolean;
  message: string;
}

export const generateSshKey = (profileId: string, passphrase?: string) =>
  invoke<GeneratedKey>("generate_ssh_key", { profileId, passphrase });

export const listSshKeys = () => invoke<SshKeyInfo[]>("list_ssh_keys");

export const readPublicKey = (keyPath: string) =>
  invoke<string>("read_public_key", { keyPath });

export const applySshAlias = (profileId: string) =>
  invoke<void>("apply_ssh_alias", { profileId });

export const testSshConnection = (host: string) =>
  invoke<SshTestResult>("test_ssh_connection", { host });
