import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: mocks.invoke,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: mocks.listen,
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

import { useProfileStore } from "../stores/useProfileStore";
import { ToastProvider } from "./ui/useToast";
import DirectoryRulesSection from "./DirectoryRules";

describe("DirectoryRules watcher lifecycle", () => {
  beforeEach(() => {
    mocks.invoke.mockImplementation((command: string) => {
      if (command === "get_auto_switch_enabled") {
        return new Promise<boolean>(() => undefined);
      }
      return Promise.resolve(undefined);
    });
    mocks.listen.mockResolvedValue(() => undefined);
    useProfileStore.setState({
      profiles: [
        {
          id: "work",
          label: "Work",
          name: "Ada",
          email: "ada@example.com",
          color: "#123456",
          isDefault: true,
        },
      ],
      directoryRules: [
        {
          id: "rule-1",
          path: "C:\\work",
          profileId: "work",
        },
      ],
      autoSwitchEnabled: true,
      watcherLifecycle: {
        state: "restarting",
        message: "watcher unavailable",
        retryInMs: 2_000,
      },
      rulesLoading: false,
    });
  });

  afterEach(() => {
    cleanup();
    mocks.invoke.mockReset();
    mocks.listen.mockReset();
  });

  it("clears degraded UI when the watcher recovers", async () => {
    render(
      <ToastProvider>
        <DirectoryRulesSection />
      </ToastProvider>,
    );

    expect(screen.getByRole("alert")).toHaveTextContent(
      "Watcher restarting automatically",
    );
    expect(screen.getByRole("alert")).toHaveTextContent("Retrying in 2s");

    act(() => {
      useProfileStore.getState().setWatcherLifecycle({
        state: "recovered",
        message: "watcher recovered",
      });
    });

    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
    expect(screen.getByText(/Auto-apply recovered/)).toBeInTheDocument();
  });

  it("states that terminal navigation does not trigger rules", () => {
    render(
      <ToastProvider>
        <DirectoryRulesSection />
      </ToastProvider>,
    );

    expect(screen.getByRole("note")).toHaveTextContent(
      "Terminal navigation and cd alone do not trigger them",
    );
  });
});
