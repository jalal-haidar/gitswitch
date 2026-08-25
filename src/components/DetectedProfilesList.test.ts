import { describe, expect, it } from "vitest";
import {
  approvedHelpUrl,
  GIT_INSTALL_HELP_URL,
} from "./DetectedProfilesList";

describe("approvedHelpUrl", () => {
  it("allows only the scoped Git installer URL", () => {
    expect(approvedHelpUrl(`Install Git from ${GIT_INSTALL_HELP_URL}`)).toBe(
      GIT_INSTALL_HELP_URL,
    );
    expect(approvedHelpUrl("https://example.com/help")).toBeNull();
    expect(approvedHelpUrl("Unlock the credential store")).toBeNull();
  });
});
