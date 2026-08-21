import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { Settings } from "./Settings";

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: mocks.invoke,
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
  save: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-updater", () => ({
  check: vi.fn(),
}));

function mockInitialSettings(storeSensitive: boolean) {
  mocks.invoke.mockImplementation((command: string) => {
    switch (command) {
      case "get_store_sensitive_in_keyring":
        return Promise.resolve(storeSensitive);
      case "get_start_with_system":
        return Promise.resolve(false);
      case "get_theme":
        return Promise.resolve("system");
      default:
        return Promise.resolve(undefined);
    }
  });
}

describe("Settings secure storage toggle", () => {
  beforeEach(() => {
    mocks.invoke.mockReset();
  });

  it.each([
    { initial: false, requested: true },
    { initial: true, requested: false },
  ])(
    "keeps the previous value and shows the actionable error when changing $initial to $requested fails",
    async ({ initial }) => {
      mockInitialSettings(initial);
      const error = JSON.stringify({
        kind: "SecureStorageError",
        message: "OS secure storage write failed",
        hint: "Unlock or repair the OS credential store, then try again.",
      });
      mocks.invoke.mockImplementation((command: string) => {
        if (command === "get_store_sensitive_in_keyring") {
          return Promise.resolve(initial);
        }
        if (command === "get_start_with_system") return Promise.resolve(false);
        if (command === "get_theme") return Promise.resolve("system");
        if (command === "set_store_sensitive_in_keyring") {
          return Promise.reject(error);
        }
        return Promise.resolve(undefined);
      });

      render(<Settings onClose={() => undefined} />);
      const checkbox = await screen.findByRole("checkbox", {
        name: /store ssh\/gpg paths in os keyring/i,
      });
      await waitFor(() => expect(checkbox).toHaveProperty("checked", initial));
      fireEvent.click(checkbox);

      const alert = await screen.findByRole("alert");
      expect(alert).toHaveTextContent("OS secure storage write failed");
      expect(alert).toHaveTextContent("Unlock or repair");
      expect(checkbox).toHaveProperty("checked", initial);
    },
  );

  it("updates the checkbox only after a successful migration", async () => {
    mockInitialSettings(false);
    mocks.invoke.mockImplementation((command: string) => {
      if (command === "get_store_sensitive_in_keyring") return Promise.resolve(false);
      if (command === "get_start_with_system") return Promise.resolve(false);
      if (command === "get_theme") return Promise.resolve("system");
      if (command === "set_store_sensitive_in_keyring") return Promise.resolve(true);
      return Promise.resolve(undefined);
    });

    render(<Settings onClose={() => undefined} />);
    const checkbox = await screen.findByRole("checkbox", {
      name: /store ssh\/gpg paths in os keyring/i,
    });
    fireEvent.click(checkbox);

    await waitFor(() => expect(checkbox).toBeChecked());
    expect(screen.getByRole("status")).toHaveTextContent(
      "moved to OS secure storage",
    );
  });
});
