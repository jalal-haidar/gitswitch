import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { ErrorBoundary } from "./ErrorBoundary";

// A component that throws on demand
function Thrower({ shouldThrow }: { shouldThrow: boolean }) {
  if (shouldThrow) throw new Error("Test explosion");
  return <div>All good</div>;
}

describe("ErrorBoundary", () => {
  beforeEach(() => {
    // Suppress React error boundary console noise
    vi.spyOn(console, "error").mockImplementation(() => {});
  });

  it("renders children when no error", () => {
    render(
      <ErrorBoundary>
        <Thrower shouldThrow={false} />
      </ErrorBoundary>,
    );
    expect(screen.getByText("All good")).toBeTruthy();
  });

  it("shows fallback UI when a child throws", () => {
    render(
      <ErrorBoundary>
        <Thrower shouldThrow={true} />
      </ErrorBoundary>,
    );
    expect(screen.getByText("Something went wrong")).toBeTruthy();
    expect(screen.getByText("Test explosion")).toBeTruthy();
  });

  it("shows generic message when error has no message", () => {
    function EmptyThrow() {
      throw new Error("");
    }
    render(
      <ErrorBoundary>
        <EmptyThrow />
      </ErrorBoundary>,
    );
    expect(screen.getByText("An unexpected error occurred.")).toBeTruthy();
  });

  it("recovers after clicking Try Again when underlying issue is fixed", () => {
    // First render throws, then we fix it and click Try Again
    let shouldThrow = true;
    function ConditionalThrower() {
      if (shouldThrow) throw new Error("Boom");
      return <div>Recovered</div>;
    }

    render(
      <ErrorBoundary>
        <ConditionalThrower />
      </ErrorBoundary>,
    );
    expect(screen.getByText("Something went wrong")).toBeTruthy();

    // "Fix" the underlying issue
    shouldThrow = false;

    fireEvent.click(screen.getByRole("button", { name: /try again/i }));
    expect(screen.getByText("Recovered")).toBeTruthy();
  });

  it("logs error via componentDidCatch", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    render(
      <ErrorBoundary>
        <Thrower shouldThrow={true} />
      </ErrorBoundary>,
    );
    expect(spy).toHaveBeenCalled();
    const loggedArgs = spy.mock.calls.find(
      (args) => args[0] === "[ErrorBoundary]",
    );
    expect(loggedArgs).toBeTruthy();
  });
});
