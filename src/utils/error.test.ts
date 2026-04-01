import { describe, it, expect } from "vitest";
import { normalizeBackendError, friendlyErrorMessage } from "./error";

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

  it("returns fallback for plain string", () => {
    const normalized = normalizeBackendError("something went wrong");
    expect(normalized.title).toBe("Error");
    expect(normalized.message).toBe("something went wrong");
  });
});

describe("friendlyErrorMessage", () => {
  it("extracts message from JSON error string", () => {
    const err = JSON.stringify({
      kind: "GitFailed",
      message: "permission denied",
    });
    expect(friendlyErrorMessage(err)).toBe("permission denied");
  });

  it("returns plain string as-is", () => {
    expect(friendlyErrorMessage("oops")).toBe("oops");
  });

  it("handles Error objects", () => {
    expect(friendlyErrorMessage(new Error("boom"))).toBe("Error: boom");
  });

  it("handles null/undefined", () => {
    expect(friendlyErrorMessage(null)).toBe("An error occurred");
    expect(friendlyErrorMessage(undefined)).toBe("An error occurred");
  });

  it("extracts message from wrapped JSON like 'Error: {...}'", () => {
    const err = `Error: ${JSON.stringify({ kind: "IoError", message: "disk full" })}`;
    expect(friendlyErrorMessage(err)).toBe("disk full");
  });
});
