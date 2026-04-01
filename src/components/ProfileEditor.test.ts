import { describe, it, expect } from "vitest";
import { toEditorValue } from "./ProfileEditor";
import type { GitProfile } from "../stores/useProfileStore";

describe("toEditorValue", () => {
  const fullProfile: GitProfile = {
    id: "abc-123",
    label: "Work",
    name: "Jane Doe",
    email: "jane@work.com",
    color: "#7C3AED",
    sshKeyPath: "/home/jane/.ssh/id_ed25519",
    gpgKeyId: "F88469E3",
    isDefault: true,
    remoteUrl: "https://github.com/jane/repo.git",
    remoteService: "github",
  };

  it("maps all fields", () => {
    const result = toEditorValue(fullProfile);
    expect(result.id).toBe("abc-123");
    expect(result.label).toBe("Work");
    expect(result.name).toBe("Jane Doe");
    expect(result.email).toBe("jane@work.com");
    expect(result.color).toBe("#7C3AED");
    expect(result.sshKeyPath).toBe("/home/jane/.ssh/id_ed25519");
    expect(result.gpgKeyId).toBe("F88469E3");
    expect(result.isDefault).toBe(true);
  });

  it("converts undefined optional fields to empty string", () => {
    const minimal: GitProfile = {
      id: "min-1",
      label: "Minimal",
      name: "Bob",
      email: "bob@test.com",
      color: "#000000",
      isDefault: false,
    };
    const result = toEditorValue(minimal);
    expect(result.sshKeyPath).toBe("");
    expect(result.gpgKeyId).toBe("");
  });

  it("does not include remoteUrl or remoteService", () => {
    const result = toEditorValue(fullProfile);
    expect(result).not.toHaveProperty("remoteUrl");
    expect(result).not.toHaveProperty("remoteService");
  });
});
