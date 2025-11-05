/**
 * Keyboard shortcuts hook
 * Implements best practices for keyboard navigation
 */

import { useEffect, useCallback, useRef } from 'react';

export interface ShortcutConfig {
  key: string;
  ctrl?: boolean;
  shift?: boolean;
  alt?: boolean;
  meta?: boolean;
  description: string;
  action: () => void | Promise<void>;
}

export interface UseKeyboardShortcutsOptions {
  shortcuts: ShortcutConfig[];
  enabled?: boolean;
  ignoreWhenInputFocused?: boolean;
}

/**
 * Check if the event target is an input element
 */
function isInputElement(target: EventTarget | null): boolean {
  if (!target || !(target instanceof HTMLElement)) {
    return false;
  }

  const tagName = target.tagName.toLowerCase();
  const isEditable = target.isContentEditable;

  return (
    tagName === 'input' ||
    tagName === 'textarea' ||
    tagName === 'select' ||
    isEditable
  );
}

/**
 * Check if the keyboard event matches the shortcut config
 */
function matchesShortcut(event: KeyboardEvent, config: ShortcutConfig): boolean {
  const keyMatch = event.key.toLowerCase() === config.key.toLowerCase() ||
                   event.code.toLowerCase() === config.key.toLowerCase();

  const ctrlMatch = config.ctrl ? (event.ctrlKey || event.metaKey) : !event.ctrlKey && !event.metaKey;
  const shiftMatch = config.shift ? event.shiftKey : !event.shiftKey;
  const altMatch = config.alt ? event.altKey : !event.altKey;
  const metaMatch = config.meta ? event.metaKey : true; // meta is optional

  return keyMatch && ctrlMatch && shiftMatch && altMatch;
}

/**
 * Format shortcut for display
 */
export function formatShortcut(config: ShortcutConfig): string {
  const parts: string[] = [];

  if (config.meta || config.ctrl) {
    parts.push(navigator.platform.includes('Mac') ? 'Cmd' : 'Ctrl');
  }

  if (config.shift) {
    parts.push('Shift');
  }

  if (config.alt) {
    parts.push('Alt');
  }

  parts.push(config.key.toUpperCase());

  return parts.join('+');
}

/**
 * useKeyboardShortcuts hook
 * 
 * Provides keyboard shortcut management with best practices:
 * - Excludes shortcuts when input elements are focused
 * - Prevents default browser behavior
 * - Supports Ctrl/Cmd, Shift, Alt modifiers
 * - ARIA-compliant
 */
export function useKeyboardShortcuts(options: UseKeyboardShortcutsOptions): void {
  const { shortcuts, enabled = true, ignoreWhenInputFocused = true } = options;
  const shortcutsRef = useRef(shortcuts);

  // Update shortcuts ref when shortcuts change
  useEffect(() => {
    shortcutsRef.current = shortcuts;
  }, [shortcuts]);

  const handleKeyDown = useCallback((event: KeyboardEvent) => {
    // Skip if disabled
    if (!enabled) {
      return;
    }

    // Skip if input is focused (unless explicitly allowed)
    if (ignoreWhenInputFocused && isInputElement(event.target)) {
      return;
    }

    // Find matching shortcut
    const matchingShortcut = shortcutsRef.current.find((config) =>
      matchesShortcut(event, config)
    );

    if (matchingShortcut) {
      event.preventDefault();
      event.stopPropagation();

      try {
        const result = matchingShortcut.action();
        if (result instanceof Promise) {
          result.catch((error) => {
            console.error('Shortcut action failed:', error);
          });
        }
      } catch (error) {
        console.error('Shortcut action failed:', error);
      }
    }
  }, [enabled, ignoreWhenInputFocused]);

  useEffect(() => {
    document.addEventListener('keydown', handleKeyDown);

    return () => {
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [handleKeyDown]);
}

/**
 * Default Codex GUI shortcuts
 */
export const DEFAULT_SHORTCUTS: ShortcutConfig[] = [
  {
    key: 'Enter',
    ctrl: true,
    description: 'Run / Execute',
    action: () => {}, // Placeholder, will be overridden
  },
  {
    key: 'S',
    ctrl: true,
    description: 'Commit',
    action: () => {},
  },
  {
    key: 'S',
    ctrl: true,
    shift: true,
    description: 'Push',
    action: () => {},
  },
  {
    key: 'D',
    ctrl: true,
    description: 'Diff',
    action: () => {},
  },
  {
    key: 'Z',
    ctrl: true,
    description: 'Revert',
    action: () => {},
  },
  {
    key: '/',
    description: 'Help / Show shortcuts',
    action: () => {},
  },
  {
    key: 'K',
    ctrl: true,
    description: 'Command palette',
    action: () => {},
  },
];

