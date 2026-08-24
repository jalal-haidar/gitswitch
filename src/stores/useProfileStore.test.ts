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
    useProfileStore.setState({
      lastRepoActivity: null,
      globalActiveProfileId: null,
      watcherLifecycle: null,
    });
  });

  it("uses repoPath for apply_profile_to_repo", async () => {
    const event = {
      profileId: "profile-1",
      profileLabel: "Work",
      repositoryPath: "C:\\repo",
      source: "manual" as const,
      occurredAtEpochMs: 123,
    };
    invokeMock.mockResolvedValue(event);

    useProfileStore.setState({ globalActiveProfileId: "global-profile" });
    const result = await useProfileStore
      .getState()
      .applyProfileToRepo("profile-1", "C:\\repo");

    expect(invokeMock).toHaveBeenCalledWith("apply_profile_to_repo", {
      id: "profile-1",
      repoPath: "C:\\repo",
    });
    expect(result).toEqual(event);
    expect(useProfileStore.getState().lastRepoActivity).toEqual(event);
    expect(useProfileStore.getState().globalActiveProfileId).toBe(
      "global-profile",
    );
  });

  it("tracks watcher lifecycle independently from profile state", () => {
    useProfileStore.setState({ globalActiveProfileId: "global-profile" });

    useProfileStore.getState().setWatcherLifecycle({
      state: "restarting",
      message: "watcher failed",
      retryInMs: 2_000,
    });
    expect(useProfileStore.getState().watcherLifecycle).toEqual({
      state: "restarting",
      message: "watcher failed",
      retryInMs: 2_000,
    });
    expect(useProfileStore.getState().globalActiveProfileId).toBe(
      "global-profile",
    );

    useProfileStore.getState().setWatcherLifecycle({
      state: "recovered",
      message: "watcher recovered",
    });
    expect(useProfileStore.getState().watcherLifecycle?.state).toBe(
      "recovered",
    );
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

  it("uses durable global snapshot commands and refreshes state after restore", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_profiles") return [];
      if (command === "get_global_active_profile_id") return null;
      if (command === "has_global_snapshot") return false;
      return undefined;
    });

    await useProfileStore.getState().restoreGlobalSnapshot();

    expect(invokeMock).toHaveBeenCalledWith("restore_global_snapshot");
    expect(invokeMock).toHaveBeenCalledWith("has_global_snapshot");
    expect(useProfileStore.getState().hasGlobalSnapshot).toBe(false);
  });

  it("invokes explicit global and repository snapshot discard commands", async () => {
    invokeMock.mockResolvedValue(undefined);

    await useProfileStore.getState().discardGlobalSnapshot();
    await useProfileStore.getState().discardRepoSnapshot("C:\\repo");

    expect(invokeMock).toHaveBeenCalledWith("discard_global_snapshot");
    expect(invokeMock).toHaveBeenCalledWith("discard_repo_snapshot", {
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
