import { useState, useEffect } from "react";
import chalk from "chalk";
import { InputHandler, TextInputProps } from "./input-handlers";

enum VimMode {
  Normal = "normal",
  Insert = "insert",
  // Future extension: Visual, Replace, etc.
}

export function useVimInputHandler({
  value: originalValue,
  placeholder = "",
  focus = true,
  mask,
  highlightPastedText = false,
  showCursor = true,
  onChange,
  onSubmit,
}: TextInputProps): InputHandler {
  const [state, setState] = useState({
    cursorOffset: (originalValue || "").length,
    cursorWidth: 0,
    mode: VimMode.Insert, // start in insert mode
  });

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

  function switchMode(newMode: VimMode) {
    setState((prev) => ({ ...prev, mode: newMode }));
  }

  const { cursorOffset, cursorWidth, mode } = state;
  const cursorActualWidth = highlightPastedText ? cursorWidth : 0;
  const displayValue = mask ? mask.repeat(originalValue.length) : originalValue;

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
    let nextCursorOffset = cursorOffset;
    let nextValue = originalValue;
    let nextCursorWidth = 0;

    if (mode === VimMode.Insert) {
      // In insert mode: allow normal text entry
      if (key.escape) {
        // Leave insert mode → switch to normal mode
        switchMode(VimMode.Normal);
        return;
      }
      if (key.return) {
        if (onSubmit) {
          onSubmit(originalValue);
        }
        return;
      }
      // Basic text input insertion similar to default handler:
      nextValue =
        originalValue.slice(0, cursorOffset) +
        input +
        originalValue.slice(cursorOffset);
      nextCursorOffset += input.length;
      if (input.length > 1) {
        nextCursorWidth = input.length;
      }
      onChange(nextValue);
    } else if (mode === VimMode.Normal) {
      // In normal mode: execute commands instead of inserting text
      if (input === "i") {
        // Enter insert mode
        switchMode(VimMode.Insert);
        return;
      } else if (input === "h") {
        // Move cursor left
        nextCursorOffset = Math.max(cursorOffset - 1, 0);
      } else if (input === "l") {
        // Move cursor right
        nextCursorOffset = Math.min(cursorOffset + 1, originalValue.length);
      } else if (input === "x") {
        // Delete character under cursor
        if (cursorOffset < originalValue.length) {
          nextValue =
            originalValue.slice(0, cursorOffset) +
            originalValue.slice(cursorOffset + 1);
          onChange(nextValue);
        }
      }
      // (Future normal mode commands can be added here)
    }
    setState((prev) => ({
      ...prev,
      cursorOffset: nextCursorOffset,
      cursorWidth: nextCursorWidth,
    }));
  };

  return {
    handler,
    output: (renderedValue || renderedPlaceholder) + modeIndicator,
  };
}
