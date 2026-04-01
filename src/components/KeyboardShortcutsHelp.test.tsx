import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import KeyboardShortcutsHelp from "./KeyboardShortcutsHelp";

describe("KeyboardShortcutsHelp", () => {
  it("renders all shortcuts", () => {
    render(<KeyboardShortcutsHelp onClose={() => {}} />);
    expect(screen.getByText("Keyboard Shortcuts")).toBeTruthy();
    expect(screen.getByText("Create new profile")).toBeTruthy();
    expect(screen.getByText("Focus search")).toBeTruthy();
    expect(screen.getByText("Open settings")).toBeTruthy();
    expect(screen.getByText("Close dialogs")).toBeTruthy();
  });

  it("calls onClose when overlay is clicked", () => {
    const onClose = vi.fn();
    const { container } = render(<KeyboardShortcutsHelp onClose={onClose} />);
    fireEvent.click(container.querySelector(".modal-overlay")!);
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("does not close when inner panel is clicked", () => {
    const onClose = vi.fn();
    render(<KeyboardShortcutsHelp onClose={onClose} />);
    fireEvent.click(screen.getByRole("dialog"));
    expect(onClose).not.toHaveBeenCalled();
  });

  it("calls onClose when X button is clicked", () => {
    const onClose = vi.fn();
    render(<KeyboardShortcutsHelp onClose={onClose} />);
    fireEvent.click(screen.getByLabelText("Close shortcuts"));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("uses Ctrl as modifier on non-Mac platforms", () => {
    render(<KeyboardShortcutsHelp onClose={() => {}} />);
    expect(screen.getByText("Ctrl+N")).toBeTruthy();
    expect(screen.getByText("Ctrl+D")).toBeTruthy();
    expect(screen.getByText("Ctrl+F")).toBeTruthy();
    expect(screen.getByText("Ctrl+,")).toBeTruthy();
  });
});
