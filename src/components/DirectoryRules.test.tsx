import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { ToastProvider } from "./ui/useToast";
import { Toaster } from "./ui/Toaster";
import { DirectoryRules } from "./DirectoryRules";
import { useProfileStore } from "../stores/useProfileStore";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

const profile = {
  id: "p1",
  label: "Work",
  name: "W",
  email: "w@x.dev",
  color: "#000",
  isDefault: true,
};

function renderRules() {
  return render(
    <ToastProvider>
      <DirectoryRules />
      <Toaster />
    </ToastProvider>,
  );
}

describe("DirectoryRules", () => {
  beforeEach(() => {
    invoke.mockReset();
    useProfileStore.setState({ profiles: [profile], directoryRules: [] });
  });

  it("adds a rule via the backend and refreshes the list", async () => {
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_directory_rules")
        return invoke.mock.calls.some((c) => c[0] === "add_directory_rule")
          ? [{ path: "C:/work/", profileId: "p1" }]
          : [];
      return undefined;
    });
    renderRules();

    fireEvent.change(screen.getByLabelText("Folder path"), {
      target: { value: "C:/work" },
    });
    fireEvent.click(screen.getByText("Add rule"));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("add_directory_rule", {
        path: "C:/work",
        profileId: "p1",
      }),
    );
    expect(await screen.findByText("C:/work/")).toBeTruthy();
    expect(screen.getByText("→ Work")).toBeTruthy();
  });

  it("shows a readable toast when the backend rejects a rule", async () => {
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === "add_directory_rule")
        throw { kind: "InvalidInput", message: "Directory does not exist: /nope" };
      return [];
    });
    renderRules();

    fireEvent.change(screen.getByLabelText("Folder path"), {
      target: { value: "/nope" },
    });
    fireEvent.click(screen.getByText("Add rule"));

    expect(
      await screen.findByText(/Directory does not exist: \/nope/),
    ).toBeTruthy();
  });

  it("removes a rule", async () => {
    useProfileStore.setState({
      directoryRules: [{ path: "C:/work/", profileId: "p1" }],
    });
    invoke.mockResolvedValue([]);
    renderRules();

    fireEvent.click(screen.getByLabelText("Remove rule for C:/work/"));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("remove_directory_rule", {
        path: "C:/work/",
      }),
    );
  });
});
