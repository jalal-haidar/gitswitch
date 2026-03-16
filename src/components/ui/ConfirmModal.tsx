import React from "react";

interface ConfirmModalProps {
  open: boolean;
  title?: string;
  description?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  busy?: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}

export const ConfirmModal: React.FC<ConfirmModalProps> = ({
  open,
  title = "Confirm",
  description,
  confirmLabel = "Confirm",
  cancelLabel = "Cancel",
  busy = false,
  onCancel,
  onConfirm,
}) => {
  if (!open) return null;

  return (
    <div className="modal-overlay" role="dialog" aria-modal="true">
      <div className="modal-panel glass-panel">
        <h3>{title}</h3>
        {description && <p className="muted">{description}</p>}
        <div className="profile-editor-actions" style={{ marginTop: "1rem" }}>
          <button
            className="btn btn-secondary"
            onClick={onCancel}
            type="button"
          >
            {cancelLabel}
          </button>
          <button
            className="btn btn-primary"
            onClick={onConfirm}
            disabled={busy}
            type="button"
          >
            {busy ? "Processing…" : confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
};

export default ConfirmModal;
