import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, it, expect, vi } from "vitest";
import React from "react";
import { toEditorValue } from "./ProfileEditor";
import { ProfileEditor } from "./ProfileEditor";
import type { GitProfile } from "../stores/useProfileStore";

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: mocks.invoke,
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

afterEach(() => {
  cleanup();
  mocks.invoke.mockReset();
});

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

  it("tests only GitHub with the key path and surfaces strict host guidance", async () => {
    mocks.invoke.mockResolvedValue({
      success: false,
      username: null,
      message:
        "GitHub is not trusted in your OpenSSH known_hosts file. Verify GitHub's published fingerprint, connect once with OpenSSH in a terminal, then retry.",
    });
    render(
      React.createElement(ProfileEditor, {
        initialValue: toEditorValue(fullProfile),
        submitLabel: "Save",
        onSubmit: () => undefined,
        onCancel: () => undefined,
      }),
    );

    fireEvent.click(screen.getByRole("button", { name: "Test" }));

    await waitFor(() =>
      expect(mocks.invoke).toHaveBeenCalledWith("test_ssh_connection", {
        keyPath: "/home/jane/.ssh/id_ed25519",
      }),
    );
    expect(await screen.findByRole("status")).toHaveTextContent(
      "not trusted in your OpenSSH known_hosts",
    );
  });
});
