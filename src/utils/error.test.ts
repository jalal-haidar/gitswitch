import { describe, it, expect } from "vitest";
import { normalizeBackendError, toUserMessage } from "./error";

describe("normalizeBackendError", () => {
  it("parses JSON error string from backend", () => {
    const err = JSON.stringify({
      kind: "GitNotFound",
      message: "git not installed",
      hint: "Install git",
    });
    const normalized = normalizeBackendError(err);
    expect(normalized.title).toBe("GitNotFound");
    expect(normalized.message).toBe("git not installed");
    expect(normalized.hint).toBe("Install git");
  });

  it("accepts a structured object (Tauri serializes BackendError directly)", () => {
    const normalized = normalizeBackendError({
      kind: "InvalidInput",
      message: "Directory does not exist",
      hint: null,
    });
    expect(normalized.title).toBe("InvalidInput");
    expect(normalized.message).toBe("Directory does not exist");
    expect(normalized.hint).toBeUndefined();
  });

  it("prefers details over a generic wrapper message", () => {
    const normalized = normalizeBackendError({
      kind: "GitFailed",
      message: "Git command failed",
      details: "fatal: not a git repository",
    });
    expect(normalized.message).toBe("fatal: not a git repository");
  });

  it("extracts JSON wrapped in text", () => {
    const normalized = normalizeBackendError(
      `Error: ${JSON.stringify({ kind: "IoError", message: "disk full" })}`,
    );
    expect(normalized.kind).toBe("IoError");
    expect(normalized.message).toBe("disk full");
  });

  it("returns fallback for plain string", () => {
    const normalized = normalizeBackendError("something went wrong");
    expect(normalized.title).toBe("Error");
    expect(normalized.message).toBe("something went wrong");
  });

  it("never yields [object Object]", () => {
    expect(toUserMessage({ foo: 1 })).toBe("An error occurred");
    expect(toUserMessage(new Error("boom"))).toBe("boom");
  });
});
