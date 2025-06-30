/**
 * Vim Input Handler Feature Completion Checklist
 *
 * Core Milestones:
 * 1. Basic Mode Functionality
 *    - [x] **Mode Switching Implementation**
 *         - [x] Start in Insert mode by default.
 *         - [x] Switch from Insert mode to Normal mode using ESC.
 *         - [x] In Normal mode, use "i" to enter Insert mode.
 *         - [x] In Normal mode, support additional Insert mode entry commands:
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
 *    - [x] **Advanced Commands (Future Milestone)**
 *         - Support additional motions like "w" and "b" for word navigation.
 *         - Implement deletion commands (e.g., "dw", "dd").
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

function findPrevWordJump(prompt: string, cursorOffset: number) {
  const regex = /[\s,.;!?]+/g;
  let lastMatch = 0;
  let currentMatch: RegExpExecArray | null;

  const stringToCursorOffset = prompt
    .slice(0, cursorOffset)
    .replace(/[\s,.;!?]+$/, "");

  while ((currentMatch = regex.exec(stringToCursorOffset)) !== null) {
    lastMatch = currentMatch.index;
  }

  if (lastMatch != 0) {
    lastMatch += 1;
  }
  return lastMatch;
}

function findNextWordJump(prompt: string, cursorOffset: number) {
  const regex = /[\s,.;!?]+/g;
  let currentMatch: RegExpExecArray | null;

  while ((currentMatch = regex.exec(prompt)) !== null) {
    if (currentMatch.index > cursorOffset) {
      return currentMatch.index + 1;
    }
  }

  return prompt.length;
}

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
  const [pendingOperator, setPendingOperator] = useState<string | null>(null);
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

  const { cursorOffset } = state;
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

  // Helper function to render the input with appropriate cursor visualization
  const renderCursorValue = (
    text: string,
    cursorOffset: number,
    mode: VimMode
  ): string => {
    if (text.length === 0) {
      // If no text, show a cursor placeholder.
      return mode === VimMode.Insert
        ? chalk.inverse("│")
        : chalk.inverse(" ");
    }
    if (cursorOffset < text.length) {
      return mode === VimMode.Insert
        ? text.slice(0, cursorOffset) + chalk.inverse("│") + text.slice(cursorOffset)
        : text.slice(0, cursorOffset) +
            chalk.inverse(text.charAt(cursorOffset)) +
            text.slice(cursorOffset + 1);
    } else {
      // Cursor is at the end of the text.
      return text + (mode === VimMode.Insert ? chalk.inverse("│") : chalk.inverse(" "));
    }
  };

  const renderedValue = showCursor && focus
    ? renderCursorValue(displayValue, cursorOffset, mode)
    : (displayValue || (placeholder ? chalk.grey(placeholder) : ""));
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
        setPendingOperator(null);
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

    if (pendingOperator) {
      if (pendingOperator === "d") {
        if (input === "d") {
          nextValue = "";
          nextCursorOffset = 0;
          onChange(nextValue);
        } else if (input === "w") {
          const end = findNextWordJump(originalValue, cursorOffset);
          nextValue = originalValue.slice(0, cursorOffset) + originalValue.slice(end);
          onChange(nextValue);
        }
      }
      setPendingOperator(null);
    } else if (input === "d") {
      setPendingOperator("d");
      return;
    } else if (input === "i") {
      switchMode(VimMode.Insert);
      return;
    } else if (input === "a") {
      nextCursorOffset = Math.min(cursorOffset + 1, originalValue.length);
      switchMode(VimMode.Insert);
    } else if (input === "A") {
      nextCursorOffset = originalValue.length;
      switchMode(VimMode.Insert);
    } else if (input === "I") {
      nextCursorOffset = 0;
      switchMode(VimMode.Insert);
    } else if (input === "h") {
      nextCursorOffset = Math.max(cursorOffset - 1, 0);
    } else if (input === "l") {
      nextCursorOffset = Math.min(cursorOffset + 1, originalValue.length);
    } else if (input === "w") {
      nextCursorOffset = findNextWordJump(originalValue, cursorOffset);
    } else if (input === "b") {
      nextCursorOffset = findPrevWordJump(originalValue, cursorOffset);
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
    output: renderedValue + modeIndicator,
  };
}
