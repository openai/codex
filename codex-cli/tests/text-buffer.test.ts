import TextBuffer from "../src/text-buffer";
import { describe, it, expect } from "vitest";

describe("TextBuffer – basic editing parity with Rust suite", () => {
  /* ------------------------------------------------------------------ */
  /*  insert_char                                                        */
  /* ------------------------------------------------------------------ */
  it("insert_char / printable (single line)", () => {
    // (col, char, expectedLine)
    const cases: Array<[number, string, string]> = [
      [0, "x", "xab"],
      [1, "x", "axb"],
      [2, "x", "abx"],
      [1, "あ", "aあb"],
    ];

    for (const [col, ch, want] of cases) {
      const buf = new TextBuffer("ab");
      buf.move("end"); // go to col 2
      while (buf.getCursor()[1] > col) {
        buf.move("left");
      }
      buf.insert(ch);
      expect(buf.getText()).toBe(want);
      expect(buf.getCursor()).toEqual([0, col + 1]);
    }
  });

  /* ------------------------------------------------------------------ */
  /*  insert_char – newline support                                      */
  /* ------------------------------------------------------------------ */
  it("insert_char with a newline should split the line", () => {
    const buf = new TextBuffer("ab");
    // jump to end of first (and only) line
    buf.move("end");
    // Insert a raw \n character – the Rust implementation splits the line
    buf.insert("\n");

    // We expect the text to be split into two separate lines
    expect(buf.getLines()).toEqual(["ab", ""]);
    expect(buf.getCursor()).toEqual([1, 0]);
  });

  /* ------------------------------------------------------------------ */
  /*  insert_str helpers                                                 */
  /* ------------------------------------------------------------------ */
  it("insert_str should insert multi‑line strings", () => {
    const initial = ["ab", "cd", "ef"].join("\n");
    const buf = new TextBuffer(initial);

    // place cursor at (row:0, col:0)
    // No move needed – cursor starts at 0,0

    buf.insertStr("x\ny");

    const wantLines = ["x", "yab", "cd", "ef"];
    expect(buf.getLines()).toEqual(wantLines);
    expect(buf.getCursor()).toEqual([1, 1]);
  });

  /* ------------------------------------------------------------------ */
  /*  Undo / Redo                                                        */
  /* ------------------------------------------------------------------ */
  it("undo / redo history should revert edits", () => {
    const buf = new TextBuffer("hello");
    buf.move("end");
    buf.insert("!"); // text becomes "hello!"

    expect(buf.undo()).toBe(true);
    expect(buf.getText()).toBe("hello");

    expect(buf.redo()).toBe(true);
    expect(buf.getText()).toBe("hello!");
  });

  /* ------------------------------------------------------------------ */
  /*  Selection model                                                    */
  /* ------------------------------------------------------------------ */
  it("copy & paste should operate on current selection", () => {
    const buf = new TextBuffer("hello world");
    buf.startSelection();
    // Select the word "hello"
    buf.move("right"); // h
    buf.move("right"); // e
    buf.move("right"); // l
    buf.move("right"); // l
    buf.move("right"); // o
    buf.endSelection();
    buf.copy();

    // Move to end and paste
    buf.move("end");
    // add one space before pasting copied word
    buf.insert(" ");
    buf.paste();

    expect(buf.getText()).toBe("hello world hello");
  });

  /* ------------------------------------------------------------------ */
  /*  Backspace behaviour                                                */
  /* ------------------------------------------------------------------ */

  describe("backspace", () => {
    it("deletes the character to the *left* of the caret within a line", () => {
      const buf = new TextBuffer("abc");

      // Move caret after the second character ( index 2 => after 'b' )
      buf.move("right"); // -> a|bc (col 1)
      buf.move("right"); // -> ab|c (col 2)

      buf.backspace();

      expect(buf.getLines()).toEqual(["ac"]);
      expect(buf.getCursor()).toEqual([0, 1]);
    });

    it("merges with the previous line when invoked at column 0", () => {
      const buf = new TextBuffer(["ab", "cd"].join("\n"));

      // Place caret at the beginning of second line
      buf.move("down"); // row = 1, col = 0

      buf.backspace();

      expect(buf.getLines()).toEqual(["abcd"]);
      expect(buf.getCursor()).toEqual([0, 2]); // after 'b'
    });

    it("is a no-op at the very beginning of the buffer", () => {
      const buf = new TextBuffer("ab");
      buf.backspace(); // caret starts at (0,0)

      expect(buf.getLines()).toEqual(["ab"]);
      expect(buf.getCursor()).toEqual([0, 0]);
    });
  });

  describe("cursor initialization", () => {
    it("initializes cursor to (0,0) by default", () => {
      const buf = new TextBuffer("hello\nworld");
      expect(buf.getCursor()).toEqual([0, 0]);
    });

    it("sets cursor to valid position within line", () => {
      const buf = new TextBuffer("hello", 2);
      expect(buf.getCursor()).toEqual([0, 2]); // cursor at 'l'
    });

    it("sets cursor to end of line", () => {
      const buf = new TextBuffer("hello", 5);
      expect(buf.getCursor()).toEqual([0, 5]); // cursor after 'o'
    });

    it("sets cursor across multiple lines", () => {
      const buf = new TextBuffer("hello\nworld", 7);
      expect(buf.getCursor()).toEqual([1, 1]); // cursor at 'o' in 'world'
    });

    it("defaults to position 0 for invalid index", () => {
      const buf = new TextBuffer("hello", 999);
      expect(buf.getCursor()).toEqual([0, 0]);
    });
  });

  /* ------------------------------------------------------------------ */
  /*  Vertical cursor movement – we should preserve the preferred column  */
  /* ------------------------------------------------------------------ */

  describe("up / down navigation keeps the preferred column", () => {
    it("restores horizontal position when moving across shorter lines", () => {
      // Three lines: long / short / long
      const lines = ["abcdef", "x", "abcdefg"].join("\n");
      const buf = new TextBuffer(lines);

      // Place caret after the 5th char in first line (col = 5)
      buf.move("end"); // col 6 (after 'f')
      buf.move("left"); // col 5 (between 'e' and 'f')

      // Move down twice – through a short line and back to a long one
      buf.move("down"); // should land on (1, 1) due to clamp
      buf.move("down"); // desired: (2, 5)

      expect(buf.getCursor()).toEqual([2, 5]);
    });
  });

  /* ------------------------------------------------------------------ */
  /*  Left / Right arrow navigation across Unicode surrogate pairs       */
  /* ------------------------------------------------------------------ */

  describe("left / right navigation", () => {
    it("should treat multi‑code‑unit emoji as a single character", () => {
      // '🐶' is a surrogate‑pair (length 2) but one user‑perceived char.
      const buf = new TextBuffer("🐶a");

      // Move caret once to the right – logically past the emoji.
      buf.move("right");

      // Insert another printable character
      buf.insert("x");

      // We expect the emoji to stay intact and the text to be 🐶xa
      expect(buf.getLines()).toEqual(["🐶xa"]);
      // Cursor should be after the inserted char (two visible columns along)
      expect(buf.getCursor()).toEqual([0, 2]);
    });
  });

  /* ------------------------------------------------------------------ */
  /*  HandleInput – raw DEL bytes should map to backspace                */
  /* ------------------------------------------------------------------ */

  it("handleInput should treat \x7f input as backspace", () => {
    const buf = new TextBuffer("");
    const vp = { width: 80, height: 25 };

    // Type "hello" via printable input path
    for (const ch of "hello") {
      buf.handleInput(ch, {}, vp);
    }

    // Two DEL bytes – terminal's backspace
    buf.handleInput("\x7f", {}, vp);
    buf.handleInput("\x7f", {}, vp);

    expect(buf.getText()).toBe("hel");
    expect(buf.getCursor()).toEqual([0, 3]);
  });

  /* ------------------------------------------------------------------ */
  /*  HandleInput – `key.delete` should ALSO behave as backspace          */
  /* ------------------------------------------------------------------ */

  it("handleInput should treat key.delete as backspace", () => {
    const buf = new TextBuffer("");
    const vp = { width: 80, height: 25 };

    for (const ch of "hello") {
      buf.handleInput(ch, {}, vp);
    }

    // Simulate the Delete (Mac backspace) key three times
    buf.handleInput(undefined, { delete: true }, vp);
    buf.handleInput(undefined, { delete: true }, vp);
    buf.handleInput(undefined, { delete: true }, vp);

    expect(buf.getText()).toBe("he");
    expect(buf.getCursor()).toEqual([0, 2]);
  });

  /* ------------------------------------------------------------------ */
  /*  Cursor positioning semantics                                       */
  /* ------------------------------------------------------------------ */

  describe("cursor movement & backspace semantics", () => {
    it("typing should leave cursor after the last inserted character", () => {
      const vp = { width: 80, height: 25 };
      const buf = new TextBuffer("");

      buf.handleInput("h", {}, vp);
      expect(buf.getCursor()).toEqual([0, 1]);

      for (const ch of "ello") {
        buf.handleInput(ch, {}, vp);
      }
      expect(buf.getCursor()).toEqual([0, 5]); // after 'o'
    });

    it("arrow‑left moves the caret to *between* characters (highlight next)", () => {
      const vp = { width: 80, height: 25 };
      const buf = new TextBuffer("");
      for (const ch of "bar") {
        buf.handleInput(ch, {}, vp);
      } // cursor at col 3

      buf.move("left"); // col 2 (right before 'r')
      buf.move("left"); // col 1 (right before 'a')

      expect(buf.getCursor()).toEqual([0, 1]);
      // Character to the RIGHT of caret should be 'a'
      const charRight = [...buf.getLines()[0]!][buf.getCursor()[1]];
      expect(charRight).toBe("a");

      // Backspace should delete the char to the *left* (i.e. 'b'), leaving "ar"
      buf.backspace();
      expect(buf.getLines()[0]).toBe("ar");
      expect(buf.getCursor()).toEqual([0, 0]);
    });
  });
});
