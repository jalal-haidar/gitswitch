import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

import { useProfileStore } from "./useProfileStore";

describe("useProfileStore invoke payloads", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("uses repoPath for apply_profile_to_repo", async () => {
    invokeMock.mockResolvedValue(undefined);

    await useProfileStore
      .getState()
      .applyProfileToRepo("profile-1", "C:\\repo");

    expect(invokeMock).toHaveBeenCalledWith("apply_profile_to_repo", {
      id: "profile-1",
      repoPath: "C:\\repo",
    });
  });

  it("uses maxDepth for scan_repos", async () => {
    invokeMock.mockResolvedValue([]);

    await useProfileStore.getState().scanRepos("C:\\projects", 3);

    expect(invokeMock).toHaveBeenCalledWith("scan_repos", {
      root: "C:\\projects",
      maxDepth: 3,
    });
  });

  it("uses repoPath for restore_repo_snapshot", async () => {
    invokeMock.mockResolvedValue(undefined);

    await useProfileStore.getState().restoreRepoSnapshot("C:\\repo");

    expect(invokeMock).toHaveBeenCalledWith("restore_repo_snapshot", {
      repoPath: "C:\\repo",
    });
  });

  it("uses repoPath for get_repo_local_config", async () => {
    invokeMock.mockResolvedValue({});

    await useProfileStore.getState().getRepoLocalConfig("C:\\repo");

    expect(invokeMock).toHaveBeenCalledWith("get_repo_local_config", {
      repoPath: "C:\\repo",
    });
  });
});
