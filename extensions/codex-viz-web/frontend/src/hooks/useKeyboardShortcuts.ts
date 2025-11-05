import { useEffect, useCallback } from 'react'

export type ShortcutAction =
  | 'toggle-commits'
  | 'toggle-heatmap'
  | 'toggle-branches'
  | 'toggle-all'
  | 'toggle-stats'
  | 'reset-camera'
  | 'increase-speed'
  | 'decrease-speed'
  | 'toggle-realtime'
  | 'toggle-help'
  | 'focus-search'
  | 'take-screenshot'

export interface Shortcut {
  key: string
  ctrl?: boolean
  alt?: boolean
  shift?: boolean
  description: string
  action: ShortcutAction
}

export const shortcuts: Shortcut[] = [
  { key: '1', description: 'Show Commits View', action: 'toggle-commits' },
  { key: '2', description: 'Show Heatmap View', action: 'toggle-heatmap' },
  { key: '3', description: 'Show Branches View', action: 'toggle-branches' },
  { key: '4', description: 'Show All Views', action: 'toggle-all' },
  { key: 'g', description: 'Toggle Performance Stats', action: 'toggle-stats' },
  { key: 'r', description: 'Reset Camera Position', action: 'reset-camera' },
  { key: '+', description: 'Increase Animation Speed', action: 'increase-speed' },
  { key: '-', description: 'Decrease Animation Speed', action: 'decrease-speed' },
  { key: 'l', description: 'Toggle Realtime Monitor', action: 'toggle-realtime' },
  { key: '?', shift: true, description: 'Show Keyboard Shortcuts', action: 'toggle-help' },
  { key: '/', ctrl: true, description: 'Focus Search (Future)', action: 'focus-search' },
  { key: 's', ctrl: true, description: 'Take Screenshot (Future)', action: 'take-screenshot' },
]

export function useKeyboardShortcuts(
  handlers: Partial<Record<ShortcutAction, () => void>>
) {
  const handleKeyDown = useCallback(
    (event: KeyboardEvent) => {
      // Check if we're in an input field
      const target = event.target as HTMLElement
      if (
        target.tagName === 'INPUT' ||
        target.tagName === 'TEXTAREA' ||
        target.isContentEditable
      ) {
        return
      }

      // Find matching shortcut
      const shortcut = shortcuts.find((s) => {
        const keyMatch = s.key.toLowerCase() === event.key.toLowerCase()
        const ctrlMatch = s.ctrl ? event.ctrlKey || event.metaKey : !event.ctrlKey && !event.metaKey
        const altMatch = s.alt ? event.altKey : !event.altKey
        const shiftMatch = s.shift ? event.shiftKey : !event.shiftKey

        return keyMatch && ctrlMatch && altMatch && shiftMatch
      })

      if (shortcut && handlers[shortcut.action]) {
        event.preventDefault()
        handlers[shortcut.action]!()
      }
    },
    [handlers]
  )

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [handleKeyDown])
}

export function getShortcutDisplay(shortcut: Shortcut): string {
  const parts: string[] = []
  if (shortcut.ctrl) parts.push('Ctrl')
  if (shortcut.alt) parts.push('Alt')
  if (shortcut.shift) parts.push('Shift')
  parts.push(shortcut.key.toUpperCase())
  return parts.join('+')
}

