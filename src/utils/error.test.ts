import { describe, it, expect } from "vitest";
import { normalizeBackendError } from "./error";

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
