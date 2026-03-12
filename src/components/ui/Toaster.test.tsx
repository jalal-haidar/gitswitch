import React from "react";
import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { ToastProvider, useToast } from "./useToast";
import Toaster from "./Toaster";

function Trigger({ onAction }: { onAction: () => void }) {
  const { show } = useToast();
  return (
    <button
      onClick={() =>
        show({
          message: "Test toast",
          kind: "error",
          duration: null,
          actions: [
            {
              label: "Retry",
              onClick: onAction,
            },
          ],
        })
      }
    >
      trigger
    </button>
  );
}

describe("Toaster", () => {
  it("renders toast and calls action", async () => {
    const action = vi.fn();
    render(
      <ToastProvider>
        <Trigger onAction={action} />
        <Toaster />
      </ToastProvider>,
    );

    const trigger = screen.getByText("trigger");
    fireEvent.click(trigger);

    expect(await screen.findByText("Test toast")).toBeInTheDocument();

    const actionBtn = screen.getByText("Retry");
    fireEvent.click(actionBtn);

    expect(action).toHaveBeenCalled();
  });
});
