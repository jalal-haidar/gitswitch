import { invoke } from "@tauri-apps/api/core";

export interface GeneratedKey {
  privateKeyPath: string;
  publicKey: string;
}

export interface SshKeyInfo {
  path: string;
  name: string;
}

export const generateSshKey = (label: string, email: string) =>
  invoke<GeneratedKey>("generate_ssh_key", { label, email });

export const listSshKeys = () => invoke<SshKeyInfo[]>("list_ssh_keys");

export const readPublicKey = (keyPath: string) =>
  invoke<string>("read_public_key", { keyPath });
