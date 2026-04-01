import { describe, it, expect } from "vitest";
import {
  isPlausibleEmail,
  sanitizeString,
  isValidSshHost,
  isPlausibleGpgKeyId,
  isValidHexColor,
  validateProfileForm,
} from "./validation";

describe("isPlausibleEmail", () => {
  it("accepts valid emails", () => {
    expect(isPlausibleEmail("user@example.com")).toBe(true);
    expect(isPlausibleEmail("jane.doe+work@company.co.uk")).toBe(true);
    expect(isPlausibleEmail("a@b.cd")).toBe(true);
  });

  it("rejects emails that are too short", () => {
    expect(isPlausibleEmail("a@b")).toBe(false);
    expect(isPlausibleEmail("@.c")).toBe(false);
  });

  it("rejects emails without proper domain", () => {
    expect(isPlausibleEmail("user@")).toBe(false);
    expect(isPlausibleEmail("user@domain")).toBe(false);
    expect(isPlausibleEmail("user@.com")).toBe(false);
    expect(isPlausibleEmail("user@com.")).toBe(false);
  });

  it("rejects emails with consecutive dots", () => {
    expect(isPlausibleEmail("user..name@domain.com")).toBe(false);
    expect(isPlausibleEmail("user@domain..com")).toBe(false);
  });

  it("rejects emails with leading/trailing dots in local part", () => {
    expect(isPlausibleEmail(".user@domain.com")).toBe(false);
    expect(isPlausibleEmail("user.@domain.com")).toBe(false);
  });

  it("rejects emails with multiple @ signs", () => {
    expect(isPlausibleEmail("user@@domain.com")).toBe(false);
    expect(isPlausibleEmail("user@domain@com")).toBe(false);
  });

  it("rejects emails with invalid characters", () => {
    expect(isPlausibleEmail("user name@domain.com")).toBe(false);
    expect(isPlausibleEmail("user<>@domain.com")).toBe(false);
  });
});

describe("sanitizeString", () => {
  it("removes control characters", () => {
    expect(sanitizeString("hello\x00world", 100)).toBe("helloworld");
    expect(sanitizeString("tab\there", 100)).toBe("tabhere");
    expect(sanitizeString("new\nline", 100)).toBe("newline");
  });

  it("truncates to max length", () => {
    expect(sanitizeString("abcdefghij", 5)).toBe("abcde");
  });

  it("trims whitespace", () => {
    expect(sanitizeString("  hello  ", 100)).toBe("hello");
  });

  it("handles empty strings", () => {
    expect(sanitizeString("", 100)).toBe("");
  });
});

describe("isValidSshHost", () => {
  it("accepts valid hosts", () => {
    expect(isValidSshHost("github.com")).toBe(true);
    expect(isValidSshHost("git@github.com")).toBe(true);
    expect(isValidSshHost("my-server.local")).toBe(true);
    expect(isValidSshHost("192.168.1.1")).toBe(true);
  });

  it("rejects empty hosts", () => {
    expect(isValidSshHost("")).toBe(false);
    expect(isValidSshHost("   ")).toBe(false);
  });

  it("rejects hosts with dangerous characters", () => {
    expect(isValidSshHost("host; rm -rf /")).toBe(false);
    expect(isValidSshHost("$(evil)")).toBe(false);
    expect(isValidSshHost("host`cmd`")).toBe(false);
  });
});

describe("isPlausibleGpgKeyId", () => {
  it("accepts valid hex key IDs", () => {
    expect(isPlausibleGpgKeyId("F88469E368AE85F0")).toBe(true);
    expect(isPlausibleGpgKeyId("ABC123")).toBe(true);
    expect(isPlausibleGpgKeyId("abcdef0123456789")).toBe(true);
  });

  it("accepts empty (optional field)", () => {
    expect(isPlausibleGpgKeyId("")).toBe(true);
  });

  it("rejects non-hex characters", () => {
    expect(isPlausibleGpgKeyId("GHIJKL")).toBe(false);
    expect(isPlausibleGpgKeyId("not-a-key")).toBe(false);
  });
});

describe("isValidHexColor", () => {
  it("accepts valid hex colors", () => {
    expect(isValidHexColor("#7C3AED")).toBe(true);
    expect(isValidHexColor("#000000")).toBe(true);
    expect(isValidHexColor("#ffffff")).toBe(true);
  });

  it("rejects invalid colors", () => {
    expect(isValidHexColor("7C3AED")).toBe(false);
    expect(isValidHexColor("#FFF")).toBe(false);
    expect(isValidHexColor("#GGGGGG")).toBe(false);
    expect(isValidHexColor("")).toBe(false);
  });
});

describe("validateProfileForm", () => {
  const validProfile = {
    label: "Work",
    name: "Jane Doe",
    email: "jane@example.com",
    color: "#7C3AED",
  };

  it("validates a correct profile", () => {
    const result = validateProfileForm(validProfile);
    expect(result.valid).toBe(true);
    expect(Object.keys(result.errors)).toHaveLength(0);
  });

  it("flags missing required fields", () => {
    const result = validateProfileForm({
      ...validProfile,
      label: "",
      name: "",
      email: "",
    });
    expect(result.valid).toBe(false);
    expect(result.errors.label).toBeDefined();
    expect(result.errors.name).toBeDefined();
    expect(result.errors.email).toBeDefined();
  });

  it("flags invalid email", () => {
    const result = validateProfileForm({
      ...validProfile,
      email: "notanemail",
    });
    expect(result.valid).toBe(false);
    expect(result.errors.email).toBeDefined();
  });

  it("flags invalid GPG key", () => {
    const result = validateProfileForm({
      ...validProfile,
      gpgKeyId: "not-hex",
    });
    expect(result.valid).toBe(false);
    expect(result.errors.gpgKeyId).toBeDefined();
  });
});
