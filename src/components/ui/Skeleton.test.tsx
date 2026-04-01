import { render } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { ProfileCardSkeleton, RuleCardSkeleton } from "./Skeleton";

describe("Skeleton components", () => {
  it("ProfileCardSkeleton renders with skeleton class", () => {
    const { container } = render(<ProfileCardSkeleton />);
    const el = container.querySelector(".profile-card.skeleton");
    expect(el).toBeTruthy();
    expect(container.querySelector(".skeleton-avatar")).toBeTruthy();
    expect(container.querySelector(".skeleton-text-title")).toBeTruthy();
  });

  it("RuleCardSkeleton renders with skeleton class", () => {
    const { container } = render(<RuleCardSkeleton />);
    const el = container.querySelector(".rule-item.skeleton");
    expect(el).toBeTruthy();
    expect(container.querySelectorAll(".skeleton-button-small").length).toBe(2);
  });
});
