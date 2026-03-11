import React, {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
} from "react";

export type ToastKind = "info" | "success" | "error";

export interface ToastAction {
  label: string;
  onClick: () => void;
}

export interface ToastItem {
  id: string;
  message: string;
  kind: ToastKind;
  duration: number | null; // ms, null = sticky
  actions?: ToastAction[];
}

interface ToastContextValue {
  toasts: ToastItem[];
  show: (opts: {
    message: string;
    kind?: ToastKind;
    duration?: number | null;
    actions?: ToastAction[];
  }) => string;
  dismiss: (id: string) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

export const useToastContext = () => {
  const ctx = useContext(ToastContext);
  if (!ctx)
    throw new Error("useToastContext must be used within ToastProvider");
  return ctx;
};

export const ToastProvider: React.FC<{ children?: React.ReactNode }> = ({
  children,
}) => {
  const [toasts, setToasts] = useState<ToastItem[]>([]);

  const show = useCallback(
    ({
      message,
      kind = "info",
      duration = 3500,
      actions,
    }: {
      message: string;
      kind?: ToastKind;
      duration?: number | null;
      actions?: ToastAction[];
    }) => {
      const id = Math.random().toString(36).slice(2, 9);
      const item: ToastItem = { id, message, kind, duration, actions };
      setToasts((t) => [item, ...t].slice(0, 6));
      return id;
    },
    [],
  );

  const dismiss = useCallback((id: string) => {
    setToasts((t) => t.filter((x) => x.id !== id));
  }, []);

  const value = useMemo(
    () => ({ toasts, show, dismiss }),
    [toasts, show, dismiss],
  );

  return (
    <ToastContext.Provider value={value}>{children}</ToastContext.Provider>
  );
};

export const useToast = () => useToastContext();
