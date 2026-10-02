import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { ToastProvider } from "./ui/useToast";
import { Toaster } from "./ui/Toaster";
import { ProfileForm } from "./ProfileForm";
import { useProfileStore } from "../stores/useProfileStore";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

const existing = {
  id: "p1",
  label: "Work",
  name: "W",
  email: "w@x.dev",
  color: "#000000",
  isDefault: true,
  sshKeyPath: "C:/Users/me/.ssh/work",
  sshHostAlias: "github-work",
};

function renderForm(props: Partial<React.ComponentProps<typeof ProfileForm>> = {}) {
  const onClose = vi.fn();
  render(
    <ToastProvider>
      <ProfileForm onClose={onClose} {...props} />
      <Toaster />
    </ToastProvider>,
  );
  return { onClose };
}

describe("ProfileForm", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation(async (cmd: string) =>
      cmd === "list_ssh_keys" || cmd === "get_profiles" ? [] : undefined,
    );
    useProfileStore.setState({ profiles: [] });
  });

  it("requires label, name and email, then creates the profile", async () => {
    const { onClose } = renderForm();
    const save = screen.getByText("Save") as HTMLButtonElement;
    expect(save.disabled).toBe(true);

    fireEvent.change(screen.getByPlaceholderText("Work"), { target: { value: "Home" } });
    fireEvent.change(screen.getByPlaceholderText("Jane Doe"), { target: { value: "J" } });
    fireEvent.change(screen.getByPlaceholderText("jane@work.dev"), {
      target: { value: "j@home.dev" },
    });
    expect(save.disabled).toBe(false);
    fireEvent.click(save);

    await waitFor(() => expect(onClose).toHaveBeenCalled());
    const call = invoke.mock.calls.find((c) => c[0] === "add_profile")!;
    expect(call[1].profile).toMatchObject({ id: "", label: "Home", email: "j@home.dev" });
  });

  it("disables key generation until the profile exists", () => {
    renderForm();
    expect((screen.getByText("Generate key") as HTMLButtonElement).disabled).toBe(true);
  });

  it("saves, then writes the ssh alias when editing", async () => {
    renderForm({ profile: existing });
    fireEvent.click(screen.getByText("Write SSH config"));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("apply_ssh_alias", { profileId: "p1" }),
    );
    const order = invoke.mock.calls.map((c) => c[0]);
    expect(order.indexOf("update_profile")).toBeLessThan(order.indexOf("apply_ssh_alias"));
  });

  it("shows backend ssh errors as readable toasts", async () => {
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_ssh_keys") return [];
      if (cmd === "test_ssh_connection")
        throw { kind: "SshFailed", message: "`ssh` was not found on PATH" };
      return undefined;
    });
    renderForm({ profile: existing });
    fireEvent.click(screen.getByText("Test connection"));
    expect(await screen.findByText(/ssh` was not found on PATH/)).toBeTruthy();
  });
});
