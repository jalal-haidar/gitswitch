import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { ConfirmModal } from "./ConfirmModal";

describe("ConfirmModal", () => {
  it("renders nothing when closed", () => {
    const { container } = render(
      <ConfirmModal open={false} onCancel={vi.fn()} onConfirm={vi.fn()} />,
    );
    expect(container.innerHTML).toBe("");
  });

  it("renders title and description when open", () => {
    render(
      <ConfirmModal
        open={true}
        title="Delete profile?"
        description="This cannot be undone."
        onCancel={vi.fn()}
        onConfirm={vi.fn()}
      />,
    );
    expect(screen.getByText("Delete profile?")).toBeInTheDocument();
    expect(screen.getByText("This cannot be undone.")).toBeInTheDocument();
  });

  it("calls onConfirm when confirm button clicked", () => {
    const onConfirm = vi.fn();
    render(
      <ConfirmModal open={true} onCancel={vi.fn()} onConfirm={onConfirm} />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Confirm" }));
    expect(onConfirm).toHaveBeenCalledOnce();
  });

  it("calls onCancel when cancel button clicked", () => {
    const onCancel = vi.fn();
    render(
      <ConfirmModal open={true} onCancel={onCancel} onConfirm={vi.fn()} />,
    );
    fireEvent.click(screen.getByText("Cancel"));
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it("calls onCancel on Escape key", () => {
    const onCancel = vi.fn();
    render(
      <ConfirmModal open={true} onCancel={onCancel} onConfirm={vi.fn()} />,
    );
    fireEvent.keyDown(document, { key: "Escape" });
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it("shows custom button labels", () => {
    render(
      <ConfirmModal
        open={true}
        confirmLabel="Yes, delete"
        cancelLabel="No, keep"
        onCancel={vi.fn()}
        onConfirm={vi.fn()}
      />,
    );
    expect(screen.getByText("Yes, delete")).toBeInTheDocument();
    expect(screen.getByText("No, keep")).toBeInTheDocument();
  });

  it("disables confirm button when busy", () => {
    render(
      <ConfirmModal
        open={true}
        busy={true}
        onCancel={vi.fn()}
        onConfirm={vi.fn()}
      />,
    );
    const btn = screen.getByText("Processing…");
    expect(btn).toBeDisabled();
  });
});
