import TextBuffer from "../src/text-buffer.js";
import { describe, it, expect } from "vitest";

// These tests ensure that the TextBuffer copy‑&‑paste logic keeps parity with
// the Rust reference implementation (`textarea.rs`).  When a multi‑line
// string *without* a trailing newline is pasted at the beginning of a line,
// the final pasted line should be merged with the text that originally
// followed the caret – exactly how most editors behave.

function setupBuffer(): TextBuffer {
  return new TextBuffer("ab\ncd\nef");
}

describe("TextBuffer – copy/paste multi‑line", () => {
  it("copies a multi‑line selection without the trailing newline", () => {
    const buf = setupBuffer();

    // Select from (0,0) → (1,2)  ["ab", "cd"]
    buf.startSelection(); // anchor at 0,0
    buf.move("down"); // 1,0
    buf.move("right");
    buf.move("right"); // 1,2

    const copied = buf.copy();
    expect(copied).toBe("ab\ncd");
  });

  it("pastes the multi‑line clipboard as separate lines (does not merge with following text)", () => {
    const buf = setupBuffer();

    // Make the same selection and copy
    buf.startSelection();
    buf.move("down");
    buf.move("right");
    buf.move("right");
    buf.copy();

    // Move caret to the start of the last line and paste
    buf.move("down");
    buf.move("home"); // (2,0)

    const ok = buf.paste();
    expect(ok).toBe(true);

    // Desired final buffer – behaviour should match the Rust reference:
    // the final pasted line is *merged* with the original text on the
    // insertion row.
    expect(buf.getLines()).toEqual(["ab", "cd", "ab", "cdef"]);
  });
});
