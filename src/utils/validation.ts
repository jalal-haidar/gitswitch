/**
 * Frontend input validation utilities — mirrors the backend's validation rules
 * to provide instant client-side feedback before a round-trip to Rust.
 */

/** Maximum lengths matching backend sanitize_string limits */
export const LIMITS = {
  LABEL: 100,
  NAME: 200,
  EMAIL: 254,
  COLOR: 32,
  SSH_KEY_PATH: 1024,
  GPG_KEY_ID: 128,
  HOST: 253,
} as const;

/**
 * Validate an email address using the same rules as the backend's `is_plausible_email`.
 * Returns `true` if the email is structurally valid.
 */
export function isPlausibleEmail(email: string): boolean {
  const trimmed = email.trim();
  if (trimmed.length < 5 || trimmed.length > LIMITS.EMAIL) return false;

  const parts = trimmed.split("@");
  if (parts.length !== 2) return false;

  const [local, domain] = parts;

  // Local part checks
  if (
    !local ||
    local.startsWith(".") ||
    local.endsWith(".") ||
    local.includes("..")
  )
    return false;

  // Domain checks
  if (
    !domain ||
    !domain.includes(".") ||
    domain.startsWith(".") ||
    domain.endsWith(".") ||
    domain.includes("..")
  )
    return false;

  // Character validation
  const localOk = /^[a-zA-Z0-9._+\-]+$/.test(local);
  const domainOk = /^[a-zA-Z0-9.\-]+$/.test(domain);
  return localOk && domainOk;
}

/**
 * Sanitize a string by removing control characters and truncating.
 * Mirrors the backend's `sanitize_string`.
 */
export function sanitizeString(s: string, maxLen: number): string {
  // Remove control characters (U+0000–U+001F, U+007F–U+009F)
  // eslint-disable-next-line no-control-regex
  const cleaned = s.replace(/[\x00-\x1F\x7F-\x9F]/g, "");
  return cleaned.slice(0, maxLen).trim();
}

/**
 * Validate an SSH host parameter — must only contain safe characters.
 * Mirrors the backend's host validation in `test_ssh_connection`.
 */
export function isValidSshHost(host: string): boolean {
  const trimmed = host.trim();
  if (trimmed.length === 0 || trimmed.length > LIMITS.HOST) return false;
  return /^[a-zA-Z0-9.\-_:@]+$/.test(trimmed);
}

/**
 * Validate a GPG key ID — should be hex digits (short or long key ID).
 */
export function isPlausibleGpgKeyId(id: string): boolean {
  const trimmed = id.trim();
  if (trimmed.length === 0) return true; // Optional field
  if (trimmed.length > LIMITS.GPG_KEY_ID) return false;
  return /^[a-fA-F0-9]+$/.test(trimmed);
}

/**
 * Validate a color hex string.
 */
export function isValidHexColor(color: string): boolean {
  return /^#[0-9a-fA-F]{6}$/.test(color.trim());
}

export interface ValidationResult {
  valid: boolean;
  errors: Record<string, string>;
}

/**
 * Validate a complete profile form before submission.
 * Returns a ValidationResult with per-field error messages.
 */
export function validateProfileForm(profile: {
  label: string;
  name: string;
  email: string;
  color: string;
  sshKeyPath?: string;
  gpgKeyId?: string;
}): ValidationResult {
  const errors: Record<string, string> = {};

  if (!sanitizeString(profile.label, LIMITS.LABEL)) {
    errors.label = "Label is required";
  }
  if (!sanitizeString(profile.name, LIMITS.NAME)) {
    errors.name = "Name is required";
  }
  if (!profile.email.trim()) {
    errors.email = "Email is required";
  } else if (!isPlausibleEmail(profile.email)) {
    errors.email = "Enter a valid email address";
  }
  if (profile.color && !isValidHexColor(profile.color)) {
    errors.color = "Must be a valid hex color (e.g. #7C3AED)";
  }
  if (profile.sshKeyPath && profile.sshKeyPath.trim().length > LIMITS.SSH_KEY_PATH) {
    errors.sshKeyPath = `SSH key path is too long (max ${LIMITS.SSH_KEY_PATH} characters)`;
  }
  if (profile.gpgKeyId && profile.gpgKeyId.trim() && !isPlausibleGpgKeyId(profile.gpgKeyId)) {
    errors.gpgKeyId = "GPG key ID should be a hexadecimal string";
  }

  return {
    valid: Object.keys(errors).length === 0,
    errors,
  };
}
