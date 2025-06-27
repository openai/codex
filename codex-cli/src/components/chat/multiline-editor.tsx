/* eslint-disable @typescript-eslint/no-explicit-any */

import { useTerminalSize } from "../../hooks/use-terminal-size";
import TextBuffer from "../../text-buffer.js";
import chalk from "chalk";
import { Box, Text, useInput } from "ink";
import { EventEmitter } from "node:events";
import React, { useRef, useState } from "react";

/* --------------------------------------------------------------------------
 * Polyfill missing `ref()` / `unref()` methods on the mock `Stdin` stream
 * provided by `ink-testing-library`.
 *
 * The real `process.stdin` object exposed by Node.js inherits these methods
 * from `Socket`, but the lightweight stub used in tests only extends
 * `EventEmitter`.  Ink calls the two methods when enabling/disabling raw
 * mode, so make them harmless no-ops when they're absent to avoid runtime
 * failures during unit tests.
 * ----------------------------------------------------------------------- */

// Cast through `unknown` ➜ `any` to avoid the `TS2352`/`TS4111` complaints
// when augmenting the prototype with the stubbed `ref`/`unref` methods in the
// test environment.  Using `any` here is acceptable because we purposefully
// monkey‑patch internals of Node's `EventEmitter` solely for the benefit of
// Ink's stdin stub – type‑safety is not a primary concern at this boundary.
//
const proto: any = EventEmitter.prototype;

if (typeof proto["ref"] !== "function") {
  proto["ref"] = function ref() {};
}
if (typeof proto["unref"] !== "function") {
  proto["unref"] = function unref() {};
}

/*
 * The `ink-testing-library` stub emits only a `data` event when its `stdin`
 * mock receives `write()` calls.  Ink, however, listens for `readable` and
 * uses the `read()` method to fetch the buffered chunk.  Bridge the gap by
 * hooking into `EventEmitter.emit` so that every `data` emission also:
 *   1.  Buffers the chunk for a subsequent `read()` call, and
 *   2.  Triggers a `readable` event, matching the contract expected by Ink.
 */

// Preserve original emit to avoid infinite recursion.
// eslint‑disable‑next‑line @typescript-eslint/no‑unsafe‑assignment
const originalEmit = proto["emit"] as (...args: Array<any>) => boolean;

proto["emit"] = function patchedEmit(
  this: any,
  event: string,
  ...args: Array<any>
): boolean {
  if (event === "data") {
    const chunk = args[0] as string;

    if (
      process.env["TEXTBUFFER_DEBUG"] === "1" ||
      process.env["TEXTBUFFER_DEBUG"] === "true"
    ) {
      // eslint-disable-next-line no-console
      console.log("[MultilineTextEditor:stdin] data", JSON.stringify(chunk));
    }
    // Store carriage returns as‑is so that Ink can distinguish between plain
    // <Enter> ("\r") and a bare line‑feed ("\n").  This matters because Ink's
    // `parseKeypress` treats "\r" as key.name === "return", whereas "\n" maps
    // to "enter" – allowing us to differentiate between plain Enter (submit)
    // and Shift+Enter (insert newline) inside `useInput`.

    // Identify the lightweight testing stub: lacks `.read()` but exposes
    // `.setRawMode()` and `isTTY` similar to the real TTY stream.
    if (
      !(this as any)._inkIsStub &&
      typeof (this as any).setRawMode === "function" &&
      typeof (this as any).isTTY === "boolean" &&
      typeof (this as any).read !== "function"
    ) {
      (this as any)._inkIsStub = true;

      // Provide a minimal `read()` shim so Ink can pull queued chunks.
      (this as any).read = function read() {
        const ret = (this as any)._inkBuffered ?? null;
        (this as any)._inkBuffered = null;
        if (
          process.env["TEXTBUFFER_DEBUG"] === "1" ||
          process.env["TEXTBUFFER_DEBUG"] === "true"
        ) {
          // eslint-disable-next-line no-console
          console.log("[MultilineTextEditor:stdin.read]", JSON.stringify(ret));
        }
        return ret;
      };
    }

    if ((this as any)._inkIsStub) {
      // Buffer the payload so that `read()` can synchronously retrieve it.
      if (typeof (this as any)._inkBuffered === "string") {
        (this as any)._inkBuffered += chunk;
      } else {
        (this as any)._inkBuffered = chunk;
      }

      // Notify listeners that data is ready in a way Ink understands.
      if (
        process.env["TEXTBUFFER_DEBUG"] === "1" ||
        process.env["TEXTBUFFER_DEBUG"] === "true"
      ) {
        // eslint-disable-next-line no-console
        console.log(
          "[MultilineTextEditor:stdin] -> readable",
          JSON.stringify(chunk),
        );
      }
      originalEmit.call(this, "readable");
    }
  }

  // Forward the original event.
  return originalEmit.call(this, event, ...args);
};

export interface MultilineTextEditorProps {
  // Initial contents.
  readonly initialText?: string;

  // Visible width.
  readonly width?: number;

  // Visible height.
  readonly height?: number;

  // Called when the user submits (plain <Enter> key).
  readonly onSubmit?: (text: string) => void;

  // Capture keyboard input.
  readonly focus?: boolean;

  // Called when the internal text buffer updates.
  readonly onChange?: (text: string) => void;

  // Optional initial cursor position (character offset)
  readonly initialCursorOffset?: number;
}

// Expose a minimal imperative API so parent components (e.g. TerminalChatInput)
// can query the caret position to implement behaviours like history
// navigation that depend on whether the cursor sits on the first/last line.
export interface MultilineTextEditorHandle {
  /** Current caret row */
  getRow(): number;
  /** Current caret column */
  getCol(): number;
  /** Total number of lines in the buffer */
  getLineCount(): number;
  /** Helper: caret is on the very first row */
  isCursorAtFirstRow(): boolean;
  /** Helper: caret is on the very last row */
  isCursorAtLastRow(): boolean;
  /** Full text contents */
  getText(): string;
  /** Move the cursor to the end of the text */
  moveCursorToEnd(): void;
}

const MultilineTextEditorInner = (
  {
    initialText = "",
    // Width can be provided by the caller.  When omitted we fall back to the
    // current terminal size (minus some padding handled by `useTerminalSize`).
    width,
    height = 10,
    onSubmit,
    focus = true,
    onChange,
    initialCursorOffset,
  }: MultilineTextEditorProps,
  ref: React.Ref<MultilineTextEditorHandle | null>,
): React.ReactElement => {
  // ---------------------------------------------------------------------------
  // Editor State
  // ---------------------------------------------------------------------------

  const buffer = useRef(new TextBuffer(initialText, initialCursorOffset));
  const [version, setVersion] = useState(0);

  // Keep track of the current terminal size so that the editor grows/shrinks
  // with the window.  `useTerminalSize` already subtracts a small horizontal
  // padding so that we don't butt up right against the edge.
  const terminalSize = useTerminalSize();

  // If the caller didn't specify a width we dynamically choose one based on
  // the terminal's current column count.  We still enforce a reasonable
  // minimum so that the UI never becomes unusably small.
  const effectiveWidth = Math.max(20, width ?? terminalSize.columns);

  // ---------------------------------------------------------------------------
  // Keyboard handling.
  // ---------------------------------------------------------------------------

  useInput(
    (input, key) => {
      if (!focus) {
        return;
      }

      if (
        process.env["TEXTBUFFER_DEBUG"] === "1" ||
        process.env["TEXTBUFFER_DEBUG"] === "true"
      ) {
        // eslint-disable-next-line no-console
        console.log("[MultilineTextEditor] event", { input, key });
      }

      // 1a) CSI-u / modifyOtherKeys *mode 2* (Ink strips initial ESC, so we
      //     start with '[') – format: "[<code>;<modifiers>u".
      if (input.startsWith("[") && input.endsWith("u")) {
        const m = input.match(/^\[([0-9]+);([0-9]+)u$/);
        if (m && m[1] === "13") {
          const mod = Number(m[2]);
          // In xterm's encoding: bit-1 (value 2) is Shift. Everything >1 that
          // isn't exactly 1 means some modifier was held. We treat *shift or
          // alt present* (2,3,4,6,8,9) as newline; Ctrl (bit-2 / value 4)
          // triggers submit.  See xterm/DEC modifyOtherKeys docs.

          const hasCtrl = Math.floor(mod / 4) % 2 === 1;
          if (hasCtrl) {
            if (onSubmit) {
              onSubmit(buffer.current.getText());
            }
          } else {
            buffer.current.newline();
          }
          setVersion((v) => v + 1);
          return;
        }
      }

      // 1b) CSI-~ / modifyOtherKeys *mode 1* – format: "[27;<mod>;<code>~".
      //     Terminals such as iTerm2 (default), older xterm versions, or when
      //     modifyOtherKeys=1 is configured, emit this legacy sequence.  We
      //     translate it to the same behaviour as the mode‑2 variant above so
      //     that Shift+Enter (newline) / Ctrl+Enter (submit) work regardless
      //     of the user’s terminal settings.
      if (input.startsWith("[27;") && input.endsWith("~")) {
        const m = input.match(/^\[27;([0-9]+);13~$/);
        if (m) {
          const mod = Number(m[1]);
          const hasCtrl = Math.floor(mod / 4) % 2 === 1;

          if (hasCtrl) {
            if (onSubmit) {
              onSubmit(buffer.current.getText());
            }
          } else {
            buffer.current.newline();
          }
          setVersion((v) => v + 1);
          return;
        }
      }

      // 2) Single‑byte control chars ------------------------------------------------
      if (input === "\n") {
        // Ctrl+J or pasted newline → insert newline.
        buffer.current.newline();
        setVersion((v) => v + 1);
        return;
      }

      if (input === "\r") {
        // Plain Enter – submit (works on all basic terminals).
        if (onSubmit) {
          onSubmit(buffer.current.getText());
        }
        return;
      }

      // Let <Esc> fall through so the parent handler (if any) can act on it.

      // Delegate remaining keys to our pure TextBuffer
      if (
        process.env["TEXTBUFFER_DEBUG"] === "1" ||
        process.env["TEXTBUFFER_DEBUG"] === "true"
      ) {
        // eslint-disable-next-line no-console
        console.log("[MultilineTextEditor] key event", { input, key });
      }

      const modified = buffer.current.handleInput(
        input,
        key as Record<string, boolean>,
        { height, width: effectiveWidth },
      );
      if (modified) {
        setVersion((v) => v + 1);
      }

      const newText = buffer.current.getText();
      if (onChange) {
        onChange(newText);
      }
    },
    { isActive: focus },
  );

  // ---------------------------------------------------------------------------
  // Rendering helpers.
  // ---------------------------------------------------------------------------

  /* ------------------------------------------------------------------------- */
  /*  Imperative handle – expose a read‑only view of caret & buffer geometry    */
  /* ------------------------------------------------------------------------- */

  React.useImperativeHandle(
    ref,
    () => ({
      getRow: () => buffer.current.getCursor()[0],
      getCol: () => buffer.current.getCursor()[1],
      getLineCount: () => buffer.current.getText().split("\n").length,
      isCursorAtFirstRow: () => buffer.current.getCursor()[0] === 0,
      isCursorAtLastRow: () => {
        const [row] = buffer.current.getCursor();
        const lineCount = buffer.current.getText().split("\n").length;
        return row === lineCount - 1;
      },
      getText: () => buffer.current.getText(),
      moveCursorToEnd: () => {
        buffer.current.move("home");
        const lines = buffer.current.getText().split("\n");
        for (let i = 0; i < lines.length - 1; i++) {
          buffer.current.move("down");
        }
        buffer.current.move("end");
        // Force a re-render
        setVersion((v) => v + 1);
      },
    }),
    [],
  );

  // Read everything from the buffer
  const visibleLines = buffer.current.getVisibleLines({
    height,
    width: effectiveWidth,
  });
  const [cursorRow, cursorCol] = buffer.current.getCursor();
  const scrollRow = (buffer.current as any).scrollRow as number;
  const scrollCol = (buffer.current as any).scrollCol as number;

  return (
    <Box flexDirection="column" key={version}>
      {visibleLines.map((lineText, idx) => {
        const absoluteRow = scrollRow + idx;

        // apply horizontal slice
        let display = lineText.slice(scrollCol, scrollCol + effectiveWidth);
        if (display.length < effectiveWidth) {
          display = display.padEnd(effectiveWidth, " ");
        }

        // Highlight the *character under the caret* (i.e. the one immediately
        // to the right of the insertion position) so that the block cursor
        // visually matches the logical caret location.  This makes the
        // highlighted glyph the one that would be replaced by `insert()` and
        // *not* the one that would be removed by `backspace()`.

        if (absoluteRow === cursorRow) {
          const relativeCol = cursorCol - scrollCol;
          const highlightCol = relativeCol;

          if (highlightCol >= 0 && highlightCol < effectiveWidth) {
            const charToHighlight = display[highlightCol] || " ";
            const highlighted = chalk.inverse(charToHighlight);
            display =
              display.slice(0, highlightCol) +
              highlighted +
              display.slice(highlightCol + 1);
          } else if (relativeCol === effectiveWidth) {
            // Caret sits just past the right edge; show a block cursor in the
            // gutter so the user still sees it.
            display = display.slice(0, effectiveWidth - 1) + chalk.inverse(" ");
          }
        }

        return <Text key={idx}>{display}</Text>;
      })}
    </Box>
  );
};

const MultilineTextEditor = React.forwardRef(MultilineTextEditorInner);
export default MultilineTextEditor;
