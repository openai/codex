import React, { useEffect, useState } from "react";
import { Text, useInput } from "ink";
import chalk from "chalk";
import type { Except } from "type-fest";

export type TextInputProps = {
  /**
   * Text to display when `value` is empty.
   */
  readonly placeholder?: string;

  /**
   * Listen to user's input. Useful in case there are multiple input components
   * at the same time and input must be "routed" to a specific component.
   */
  readonly focus?: boolean; // eslint-disable-line react/boolean-prop-naming

  /**
   * Replace all chars and mask the value. Useful for password inputs.
   */
  readonly mask?: string;

  /**
   * Whether to show cursor and allow navigation inside text input with arrow keys.
   */
  readonly showCursor?: boolean; // eslint-disable-line react/boolean-prop-naming

  /**
   * Highlight pasted text
   */
  readonly highlightPastedText?: boolean; // eslint-disable-line react/boolean-prop-naming

  /**
   * Value to display in a text input.
   */
  readonly value: string;

  /**
   * Function to call when value updates.
   */
  readonly onChange: (value: string) => void;

  /**
   * Function to call when `Enter` is pressed, where first argument is a value of the input.
   */
  readonly onSubmit?: (value: string) => void;

  /**
   * Explicitly set the cursor position to the end of the text
   */
  readonly cursorToEnd?: boolean;
};

function findPrevWordJump(prompt: string, cursorOffset: number) {
  const regex = /[\s,.;!?]+/g;
  let lastMatch = 0;
  let currentMatch: RegExpExecArray | null;

  const stringToCursorOffset = prompt
    .slice(0, cursorOffset)
    .replace(/[\s,.;!?]+$/, "");

  // Loop through all matches
  while ((currentMatch = regex.exec(stringToCursorOffset)) !== null) {
    lastMatch = currentMatch.index;
  }

  // Include the last match unless it is the first character
  if (lastMatch != 0) {
    lastMatch += 1;
  }
  return lastMatch;
}

function findNextWordJump(prompt: string, cursorOffset: number) {
  const regex = /[\s,.;!?]+/g;
  let currentMatch: RegExpExecArray | null;

  // Loop through all matches
  while ((currentMatch = regex.exec(prompt)) !== null) {
    if (currentMatch.index > cursorOffset) {
      return currentMatch.index + 1;
    }
  }

  return prompt.length;
}

function TextInput({
  value: originalValue,
  placeholder = "",
  focus = true,
  mask,
  highlightPastedText = false,
  showCursor = true,
  onChange,
  onSubmit,
  cursorToEnd = false,
}: TextInputProps) {
  const [state, setState] = useState({
    cursorOffset: (originalValue || "").length,
    cursorWidth: 0,
  });

  useEffect(() => {
    if (cursorToEnd) {
      setState((prev) => ({
        ...prev,
        cursorOffset: (originalValue || "").length,
      }));
    }
  }, [cursorToEnd, originalValue, focus]);

  const { cursorOffset, cursorWidth } = state;

  useEffect(() => {
    setState((previousState) => {
      if (!focus || !showCursor) {
        return previousState;
      }

      const newValue = originalValue || "";
      // Sets the cursor to the end of the line if the value is empty or the cursor is at the end of the line.
      if (
        previousState.cursorOffset === 0 ||
        previousState.cursorOffset > newValue.length - 1
      ) {
        return {
          cursorOffset: newValue.length,
          cursorWidth: 0,
        };
      }

      return previousState;
    });
  }, [originalValue, focus, showCursor]);

  const cursorActualWidth = highlightPastedText ? cursorWidth : 0;

  const value = mask ? mask.repeat(originalValue.length) : originalValue;
  let renderedValue = value;
  let renderedPlaceholder = placeholder ? chalk.grey(placeholder) : undefined;

  // Fake mouse cursor, because it's too inconvenient to deal with actual cursor and ansi escapes.
  if (showCursor && focus) {
    renderedPlaceholder =
      placeholder.length > 0
        ? chalk.inverse(placeholder[0]) + chalk.grey(placeholder.slice(1))
        : chalk.inverse(" ");

    renderedValue = value.length > 0 ? "" : chalk.inverse(" ");

    let i = 0;

    for (const char of value) {
      renderedValue +=
        i >= cursorOffset - cursorActualWidth && i <= cursorOffset
          ? chalk.inverse(char)
          : char;

      i++;
    }

    if (value.length > 0 && cursorOffset === value.length) {
      renderedValue += chalk.inverse(" ");
    }
  }

  useInput(
    (input, key) => {
      // ────────────────────────────────────────────────────────────────
      // Support Shift+Enter / Ctrl+Enter from terminals that have
      // modifyOtherKeys enabled.  Such terminals encode the key‑combo in a
      // CSI sequence rather than sending a bare "\r"/"\n".  Ink passes the
      // sequence through as raw text (without the initial ESC), so we need to
      // detect and translate it before the generic character handler below
      // treats it as literal input (e.g. "[27;2;13~").  We support both the
      // modern *mode 2* (CSI‑u, ending in "u") and the legacy *mode 1*
      // variant (ending in "~").
      //
      //  - Shift+Enter  → insert newline (same behaviour as Option+Enter)
      //  - Ctrl+Enter   → submit the input (same as plain Enter)
      //
      // References: https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h3-Modify-Other-Keys
      // ────────────────────────────────────────────────────────────────

      function handleEncodedEnterSequence(raw: string): boolean {
        // CSI‑u (modifyOtherKeys=2)  → "[13;<mod>u"
        let m = raw.match(/^\[([0-9]+);([0-9]+)u$/);
        if (m && m[1] === "13") {
          const mod = Number(m[2]);
          const hasCtrl = Math.floor(mod / 4) % 2 === 1;

          if (hasCtrl) {
            if (onSubmit) {
              onSubmit(originalValue);
            }
          } else {
            const newValue =
              originalValue.slice(0, cursorOffset) +
              "\n" +
              originalValue.slice(cursorOffset);

            setState({
              cursorOffset: cursorOffset + 1,
              cursorWidth: 0,
            });
            onChange(newValue);
          }
          return true; // handled
        }

        // CSI‑~ (modifyOtherKeys=1) → "[27;<mod>;13~"
        m = raw.match(/^\[27;([0-9]+);13~$/);
        if (m) {
          const mod = Number(m[1]);
          const hasCtrl = Math.floor(mod / 4) % 2 === 1;

          if (hasCtrl) {
            if (onSubmit) {
              onSubmit(originalValue);
            }
          } else {
            const newValue =
              originalValue.slice(0, cursorOffset) +
              "\n" +
              originalValue.slice(cursorOffset);

            setState({
              cursorOffset: cursorOffset + 1,
              cursorWidth: 0,
            });
            onChange(newValue);
          }
          return true; // handled
        }
        return false; // not an encoded Enter sequence
      }

      if (handleEncodedEnterSequence(input)) {
        return;
      }
      if (
        key.upArrow ||
        key.downArrow ||
        (key.ctrl && input === "c") ||
        key.tab ||
        (key.shift && key.tab)
      ) {
        return;
      }

      let nextCursorOffset = cursorOffset;
      let nextValue = originalValue;
      let nextCursorWidth = 0;

      // TODO: continue improving the cursor management to feel native
      if (key.return) {
        if (key.meta) {
          // This does not work yet. We would like to have this behavior:
          //     Mac terminal: Settings → Profiles → Keyboard → Use Option as Meta key
          //     iTerm2: Open Settings → Profiles → Keys → General → Set Left/Right Option as Esc+
          // And then when Option+ENTER is pressed, we want to insert a newline.
          // However, even with the settings, the input="\n" and only key.shift is True.
          // This is likely an artifact of how ink works.
          nextValue =
            originalValue.slice(0, cursorOffset) +
            "\n" +
            originalValue.slice(cursorOffset, originalValue.length);
          nextCursorOffset++;
        } else {
          // Handle Enter key: support bash-style line continuation with backslash
          // -- count consecutive backslashes immediately before cursor
          // -- only a single trailing backslash at end indicates line continuation
          const isAtEnd = cursorOffset === originalValue.length;
          const trailingMatch = originalValue.match(/\\+$/);
          const trailingCount = trailingMatch ? trailingMatch[0].length : 0;
          if (isAtEnd && trailingCount === 1) {
            nextValue += "\n";
            nextCursorOffset = nextValue.length;
            nextCursorWidth = 0;
          } else if (onSubmit) {
            onSubmit(originalValue);
            return;
          }
        }
      } else if ((key.ctrl && input === "a") || (key.meta && key.leftArrow)) {
        nextCursorOffset = 0;
      } else if ((key.ctrl && input === "e") || (key.meta && key.rightArrow)) {
        // Move cursor to end of line
        nextCursorOffset = originalValue.length;
        // Emacs/readline-style navigation and editing shortcuts
      } else if (key.ctrl && input === "b") {
        // Move cursor backward by one
        if (showCursor) {
          nextCursorOffset = Math.max(cursorOffset - 1, 0);
        }
      } else if (key.ctrl && input === "f") {
        // Move cursor forward by one
        if (showCursor) {
          nextCursorOffset = Math.min(cursorOffset + 1, originalValue.length);
        }
      } else if (key.ctrl && input === "d") {
        // Delete character at cursor (forward delete)
        if (cursorOffset < originalValue.length) {
          nextValue =
            originalValue.slice(0, cursorOffset) +
            originalValue.slice(cursorOffset + 1);
        }
      } else if (key.ctrl && input === "k") {
        // Kill text from cursor to end of line
        nextValue = originalValue.slice(0, cursorOffset);
      } else if (key.ctrl && input === "u") {
        // Kill text from start to cursor
        nextValue = originalValue.slice(cursorOffset);
        nextCursorOffset = 0;
      } else if (key.ctrl && input === "w") {
        // Delete the word before cursor
        {
          const left = originalValue.slice(0, cursorOffset);
          const match = left.match(/\s*\S+$/);
          const cut = match ? match[0].length : cursorOffset;
          nextValue =
            originalValue.slice(0, cursorOffset - cut) +
            originalValue.slice(cursorOffset);
          nextCursorOffset = cursorOffset - cut;
        }
      } else if (key.meta && (key.backspace || key.delete)) {
        const regex = /[\s,.;!?]+/g;
        let lastMatch = 0;
        let currentMatch: RegExpExecArray | null;

        const stringToCursorOffset = originalValue
          .slice(0, cursorOffset)
          .replace(/[\s,.;!?]+$/, "");

        // Loop through all matches
        while ((currentMatch = regex.exec(stringToCursorOffset)) !== null) {
          lastMatch = currentMatch.index;
        }

        // Include the last match unless it is the first character
        if (lastMatch != 0) {
          lastMatch += 1;
        }

        nextValue =
          stringToCursorOffset.slice(0, lastMatch) +
          originalValue.slice(cursorOffset, originalValue.length);
        nextCursorOffset = lastMatch;
      } else if (key.meta && (input === "b" || key.leftArrow)) {
        nextCursorOffset = findPrevWordJump(originalValue, cursorOffset);
      } else if (key.meta && (input === "f" || key.rightArrow)) {
        nextCursorOffset = findNextWordJump(originalValue, cursorOffset);
      } else if (key.leftArrow) {
        if (showCursor) {
          nextCursorOffset--;
        }
      } else if (key.rightArrow) {
        if (showCursor) {
          nextCursorOffset++;
        }
      } else if (key.backspace || key.delete) {
        if (cursorOffset > 0) {
          nextValue =
            originalValue.slice(0, cursorOffset - 1) +
            originalValue.slice(cursorOffset, originalValue.length);

          nextCursorOffset--;
        }
      } else {
        nextValue =
          originalValue.slice(0, cursorOffset) +
          input +
          originalValue.slice(cursorOffset, originalValue.length);

        nextCursorOffset += input.length;

        if (input.length > 1) {
          nextCursorWidth = input.length;
        }
      }

      if (cursorOffset < 0) {
        nextCursorOffset = 0;
      }

      if (cursorOffset > originalValue.length) {
        nextCursorOffset = originalValue.length;
      }

      setState({
        cursorOffset: nextCursorOffset,
        cursorWidth: nextCursorWidth,
      });

      if (nextValue !== originalValue) {
        onChange(nextValue);
      }
    },
    { isActive: focus },
  );

  return (
    <Text>
      {placeholder
        ? value.length > 0
          ? renderedValue
          : renderedPlaceholder
        : renderedValue}
    </Text>
  );
}

export default TextInput;

type UncontrolledProps = {
  readonly initialValue?: string;
} & Except<TextInputProps, "value" | "onChange">;

export function UncontrolledTextInput({
  initialValue = "",
  ...props
}: UncontrolledProps) {
  const [value, setValue] = useState(initialValue);

  return <TextInput {...props} value={value} onChange={setValue} />;
}
