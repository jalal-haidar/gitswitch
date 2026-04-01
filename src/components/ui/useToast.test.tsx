import React from "react";
import { renderHook, act } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { ToastProvider, useToast } from "./useToast";

function wrapper({ children }: { children: React.ReactNode }) {
  return <ToastProvider>{children}</ToastProvider>;
}

describe("useToast", () => {
  it("throws when used outside ToastProvider", () => {
    expect(() => renderHook(() => useToast())).toThrow(
      "useToastContext must be used within ToastProvider",
    );
  });

  it("starts with no toasts", () => {
    const { result } = renderHook(() => useToast(), { wrapper });
    expect(result.current.toasts).toHaveLength(0);
  });

  it("adds a toast on show()", () => {
    const { result } = renderHook(() => useToast(), { wrapper });
    act(() => {
      result.current.show({ message: "Hello" });
    });
    expect(result.current.toasts).toHaveLength(1);
    expect(result.current.toasts[0].message).toBe("Hello");
    expect(result.current.toasts[0].kind).toBe("info");
  });

  it("respects kind and duration", () => {
    const { result } = renderHook(() => useToast(), { wrapper });
    act(() => {
      result.current.show({ message: "err", kind: "error", duration: null });
    });
    const toast = result.current.toasts[0];
    expect(toast.kind).toBe("error");
    expect(toast.duration).toBeNull();
  });

  it("dismiss removes a toast by id", () => {
    const { result } = renderHook(() => useToast(), { wrapper });
    let id: string;
    act(() => {
      id = result.current.show({ message: "remove me" });
    });
    expect(result.current.toasts).toHaveLength(1);
    act(() => {
      result.current.dismiss(id!);
    });
    expect(result.current.toasts).toHaveLength(0);
  });

  it("caps toasts at 6", () => {
    const { result } = renderHook(() => useToast(), { wrapper });
    act(() => {
      for (let i = 0; i < 10; i++) {
        result.current.show({ message: `toast ${i}` });
      }
    });
    expect(result.current.toasts.length).toBeLessThanOrEqual(6);
  });

  it("returns a unique id from show()", () => {
    const { result } = renderHook(() => useToast(), { wrapper });
    const ids: string[] = [];
    act(() => {
      ids.push(result.current.show({ message: "a" }));
      ids.push(result.current.show({ message: "b" }));
    });
    expect(ids[0]).not.toBe(ids[1]);
  });
});
