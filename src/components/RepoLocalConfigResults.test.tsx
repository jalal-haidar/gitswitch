import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import RepoLocalConfigResults from "./RepoLocalConfigResults";

describe("RepoLocalConfigResults", () => {
  it("renders repo config values and fallback text", () => {
    render(
      <RepoLocalConfigResults
        items={[
          {
            repo: {
              name: "workspace-a",
              path: "C:\\repos\\workspace-a",
            },
            config: {
              userName: "Ada",
              userEmail: "ada@example.com",
              coreSshCommand: 'ssh -i "C:/keys/id_a" -o IdentitiesOnly=yes',
            },
          },
        ]}
        selectedCount={1}
        onClear={vi.fn()}
        onDismiss={vi.fn()}
      />,
    );

    expect(screen.getByText("Selected Local Git Config")).toBeInTheDocument();
    expect(screen.getByText("workspace-a")).toBeInTheDocument();
    expect(screen.getByText("ada@example.com")).toBeInTheDocument();
    expect(screen.getAllByText("not set").length).toBeGreaterThan(0);
  });

  it("calls dismiss and clear handlers", () => {
    const onDismiss = vi.fn();
    const onClear = vi.fn();

    render(
      <RepoLocalConfigResults
        items={[
          {
            repo: {
              name: "workspace-a",
              path: "C:\\repos\\workspace-a",
            },
            config: {},
          },
        ]}
        selectedCount={2}
        error="Failed to load one repository"
        onClear={onClear}
        onDismiss={onDismiss}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Clear Results" }));
    fireEvent.click(
      screen.getByRole("button", {
        name: "Remove workspace-a from local config results",
      }),
    );

    expect(screen.getByRole("alert")).toHaveTextContent(
      "Failed to load one repository",
    );
    expect(onClear).toHaveBeenCalledOnce();
    expect(onDismiss).toHaveBeenCalledWith("C:\\repos\\workspace-a");
  });
});
