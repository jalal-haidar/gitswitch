import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { Layout } from "./Layout";

describe("Layout", () => {
  it("renders children inside container", () => {
    render(
      <Layout>
        <span>Test content</span>
      </Layout>,
    );
    expect(screen.getByText("Test content")).toBeTruthy();
  });

  it("has background glow elements", () => {
    const { container } = render(
      <Layout>
        <div />
      </Layout>,
    );
    expect(container.querySelector(".bg-glow-1")).toBeTruthy();
    expect(container.querySelector(".bg-glow-2")).toBeTruthy();
  });

  it("applies fade-in animation class", () => {
    const { container } = render(
      <Layout>
        <div />
      </Layout>,
    );
    expect(container.querySelector(".animate-fade-in")).toBeTruthy();
  });
});
