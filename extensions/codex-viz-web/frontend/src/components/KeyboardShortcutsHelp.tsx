import { useEffect, useRef } from 'react'
import { shortcuts, getShortcutDisplay } from '../hooks/useKeyboardShortcuts'
import './KeyboardShortcutsHelp.css'

interface KeyboardShortcutsHelpProps {
  isOpen: boolean
  onClose: () => void
}

export default function KeyboardShortcutsHelp({ isOpen, onClose }: KeyboardShortcutsHelpProps) {
  const modalRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }

    if (isOpen) {
      window.addEventListener('keydown', handleEscape)
      modalRef.current?.focus()
    }

    return () => window.removeEventListener('keydown', handleEscape)
  }, [isOpen, onClose])

  if (!isOpen) return null

  return (
    <div className="shortcuts-overlay" onClick={onClose}>
      <div
        ref={modalRef}
        className="shortcuts-modal"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-labelledby="shortcuts-title"
        aria-modal="true"
        tabIndex={-1}
      >
        <div className="shortcuts-header">
          <h2 id="shortcuts-title" className="shortcuts-title">
            ⌨️ Keyboard Shortcuts
          </h2>
          <button
            className="shortcuts-close"
            onClick={onClose}
            aria-label="Close shortcuts help"
          >
            ✕
          </button>
        </div>

        <div className="shortcuts-grid">
          {shortcuts.map((shortcut) => (
            <div key={shortcut.action} className="shortcut-item">
              <kbd className="shortcut-key">{getShortcutDisplay(shortcut)}</kbd>
              <span className="shortcut-description">{shortcut.description}</span>
            </div>
          ))}
        </div>

        <div className="shortcuts-footer">
          <p className="shortcuts-hint">Press <kbd>Esc</kbd> to close</p>
        </div>
      </div>
    </div>
  )
}

