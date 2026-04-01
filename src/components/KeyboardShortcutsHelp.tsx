import React from "react";
import { X, Command } from "lucide-react";

interface KeyboardShortcutsHelpProps {
  onClose: () => void;
}

const KeyboardShortcuts: React.FC<KeyboardShortcutsHelpProps> = ({
  onClose,
}) => {
  const isMac = navigator.platform.toUpperCase().indexOf("MAC") >= 0;
  const modKey = isMac ? "⌘" : "Ctrl";

  const shortcuts = [
    { key: `${modKey}+N`, action: "Create new profile" },
    { key: `${modKey}+F`, action: "Focus search" },
    { key: `${modKey}+,`, action: "Open settings" },
    { key: "Esc", action: "Close dialogs" },
  ];

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="glass-panel keyboard-shortcuts-modal"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-labelledby="shortcuts-title"
      >
        <div className="keyboard-shortcuts-header">
          <h2 id="shortcuts-title">
            <Command size={20} /> Keyboard Shortcuts
          </h2>
          <button
            className="btn-icon"
            onClick={onClose}
            aria-label="Close shortcuts"
          >
            <X size={20} />
          </button>
        </div>
        <div className="keyboard-shortcuts-list">
          {shortcuts.map((shortcut) => (
            <div key={shortcut.key} className="shortcut-item">
              <kbd className="shortcut-key">{shortcut.key}</kbd>
              <span className="shortcut-action">{shortcut.action}</span>
            </div>
          ))}
        </div>
        <div className="keyboard-shortcuts-footer">
          <span className="shortcut-hint">
            Press <kbd>?</kbd> anytime to show this dialog
          </span>
        </div>
      </div>
    </div>
  );
};

export default KeyboardShortcuts;
