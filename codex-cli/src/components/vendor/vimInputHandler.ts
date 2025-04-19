/**
 * Vim Input Handler Feature Completion Checklist
 *
 * Core Milestones:
 * 1. Basic Mode Functionality
 *    - [x] **Mode Switching Implementation**
 *         - [x] Start in Insert mode by default.
 *         - [x] Switch from Insert mode to Normal mode using ESC.
 *         - [x] In Normal mode, use "i" to enter Insert mode.
 *         - [ ] In Normal mode, support additional Insert mode entry commands:
 *               "a" (append after cursor), "A" (append at end), "I" (insert at beginning)
 *               and any other keys appropriate for a single-line input.
 *
 * 2. Insert Mode Enhancements
 *    - [x] Ensure text insertion mimics the default input handler.
 *    - [x] Handle special keys (newline, backspace, etc.) appropriately.
 *
 * 3. Normal Mode Navigation & Editing
 *    - [x] **Basic Navigation**
 *         - [x] 'h' to move cursor left.
 *         - [x] 'l' to move cursor right.
 *    - [x] **Editing Operations**
 *         - [x] 'x' to delete the character under the cursor.
 *    - [ ] **Advanced Commands (Future Milestone)**
 *         - Support additional motions like "w" and "b" for word navigation.
 *         - Implement deletion and change commands (e.g., "dw", "dd", etc.).
 *
 * 4. UI/UX and Visual Feedback Improvements
 *    - [x] Display a clear mode indicator (e.g., "INSERT" vs. "NORMAL").
 *    - [ ] Enhance caret highlighting and overall styling based on mode.
 *
 * 5. Extensibility and Customization
 *    - [ ] Architect the code to allow future integration of additional Vim modes (Visual, Replace, etc.).
 *    - [ ] Allow customizable keybindings in future iterations.
 *
 * 6. Testing & Documentation
 *    - [ ] Add unit tests for both Insert and Normal mode behaviors.
 *    - [ ] Document the known limitations and outline future work items.
 */

import { useEffect, useState } from "react";
import { useDefaultInputHandler } from "./defaultInputHandler";
import chalk from "chalk";
import { InputHandler } from "./input-handlers";

enum VimMode {
  Normal = "normal",
  Insert = "insert",
  // Future extension: Visual, Replace, etc.
}

export const useVimInputHandler: InputHandler = ({
  value: originalValue,
  placeholder = "",
  focus = true,
  mask,
  highlightPastedText = false,
  showCursor = true,
  onChange,
  onSubmit,
  cursorState: [state, setState],
}) => {
  const [mode, switchMode] = useState<VimMode>(VimMode.Insert);
  // Sync state with prop changes (as in the default handler)
  useEffect(() => {
    setState((prevState) => {
      if (!focus || !showCursor) return prevState;
      const newValue = originalValue || "";
      if (prevState.cursorOffset > newValue.length) {
        return { ...prevState, cursorOffset: newValue.length };
      }
      return prevState;
    });
  }, [originalValue, focus, showCursor]);

  const { cursorOffset, cursorWidth } = state;
  const cursorActualWidth = highlightPastedText ? cursorWidth : 0;
  const displayValue = mask ? mask.repeat(originalValue.length) : originalValue;

  // Create a default handler instance to delegate to when in Insert mode
  const defaultHandlerObj = useDefaultInputHandler({
    value: originalValue,
    placeholder,
    focus,
    mask,
    highlightPastedText,
    showCursor,
    onChange,
    onSubmit,
    cursorState: [state, setState],
  });

  // Basic rendering similar to default – you can expand this later.
  let renderedValue = showCursor && focus ? "" : displayValue;
  let renderedPlaceholder = placeholder ? chalk.grey(placeholder) : undefined;

  if (showCursor && focus) {
    renderedPlaceholder =
      placeholder.length > 0
        ? chalk.inverse(placeholder[0]) + chalk.grey(placeholder.slice(1))
        : chalk.inverse(" ");
    let i = 0;
    for (const char of displayValue) {
      renderedValue +=
        i >= cursorOffset - cursorActualWidth && i < cursorOffset
          ? chalk.inverse(char)
          : char;
      i++;
    }
    if (cursorOffset === displayValue.length) {
      renderedValue += chalk.inverse(" ");
    }
  }
  // Prepend a mode indicator (e.g. "INSERT" in green, "NORMAL" in blue)
  const modeIndicator =
    mode === VimMode.Insert
      ? chalk.bgGreen.white(" INSERT ")
      : chalk.bgBlue.white(" NORMAL ");

  const handler = (input: string, key: any) => {
    if (mode === VimMode.Insert) {
      // In Insert mode, override escape key handling.
      if (key.escape) {
        switchMode(VimMode.Normal);
        return;
      }
      // Delegate all other keys to the default input handler.
      defaultHandlerObj.handler(input, key);
      return;
    }

    // Normal mode handling.
    let nextCursorOffset = cursorOffset;
    let nextValue = originalValue;
    let nextCursorWidth = 0;

    if (input === "i") {
      switchMode(VimMode.Insert);
      return;
    } else if (input === "h") {
      nextCursorOffset = Math.max(cursorOffset - 1, 0);
    } else if (input === "l") {
      nextCursorOffset = Math.min(cursorOffset + 1, originalValue.length);
    } else if (input === "x") {
      if (cursorOffset < originalValue.length) {
        nextValue =
          originalValue.slice(0, cursorOffset) +
          originalValue.slice(cursorOffset + 1);
        onChange(nextValue);
      }
    }
    setState((prev) => ({
      ...prev,
      cursorOffset: nextCursorOffset,
      cursorWidth: nextCursorWidth,
    }));
  };

  return {
    handler,
    output: (mode === VimMode.Insert ? defaultHandlerObj.output : (renderedValue || renderedPlaceholder)) + modeIndicator,
  };
}
