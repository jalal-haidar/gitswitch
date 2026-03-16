import React, { useEffect } from "react";
import ReactDOM from "react-dom";
import { useToast } from "./useToast";
import { X, CheckCircle, Info } from "lucide-react";

const ToastItem: React.FC<{
  id: string;
  message: string;
  kind: string;
  duration: number | null;
  actions?: { label: string; onClick: () => void }[];
  onClose: (id: string) => void;
}> = ({ id, message, kind, duration, actions, onClose }) => {
  useEffect(() => {
    if (duration === null) return; // sticky
    const t = setTimeout(() => onClose(id), duration);
    return () => clearTimeout(t);
  }, [id, duration, onClose]);
  return (
    <div className={`toast ${kind} show`} role="status" aria-live="polite">
      <div className="toast-icon">
        {kind === "success" ? (
          <CheckCircle size={18} />
        ) : kind === "error" ? (
          <X size={18} />
        ) : (
          <Info size={18} />
        )}
      </div>
      <div className="toast-body">
        {message}
        {actions && actions.length > 0 && (
          <div className="toast-actions">
            {actions.map((a, idx) => (
              <button
                key={idx}
                className="btn btn-secondary"
                onClick={() => {
                  try {
                    a.onClick();
                  } catch (err) {
                    // swallow
                    console.error("toast action failed", err);
                  }
                  onClose(id);
                }}
              >
                {a.label}
              </button>
            ))}
          </div>
        )}
      </div>
      <button
        className="toast-close"
        onClick={() => onClose(id)}
        aria-label="Close notification"
      >
        <X size={14} />
      </button>
    </div>
  );
};

export const Toaster: React.FC = () => {
  const { toasts, dismiss } = useToast();

  if (typeof document === "undefined") return null;

  return ReactDOM.createPortal(
    <div className="toaster">
      {toasts.map((t) => (
        <ToastItem
          key={t.id}
          id={t.id}
          message={t.message}
          kind={t.kind}
          duration={t.duration}
          actions={t.actions as any}
          onClose={dismiss}
        />
      ))}
    </div>,
    document.body,
  );
};

export default Toaster;
