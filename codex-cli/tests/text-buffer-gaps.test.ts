import TextBuffer from "../src/text-buffer";
import { describe, it, expect } from "vitest";

// The purpose of this test‑suite is NOT to make the implementation green today
// – quite the opposite.  We capture behaviours that are already covered by the
// reference Rust implementation (textarea.rs) but are *still missing* from the
// current TypeScript port.  Every test is therefore marked with `.fails()` so
// that the suite passes while the functionality is absent.  When a particular
// gap is closed the corresponding test will begin to succeed, causing Vitest to
// raise an error (a *good* error) that reminds us to remove the `.fails` flag.

/* -------------------------------------------------------------------------- */
/*  Soft‑tab insertion                                                         */
/* -------------------------------------------------------------------------- */

describe("soft‑tab insertion (↹ => 4 spaces)", () => {
  it.fails(
    "inserts 4 spaces at caret position when hard‑tab mode is off",
    () => {
      const buf = new TextBuffer("");

      // A literal "\t" character is treated as user pressing the Tab key.  The
      // Rust version expands it to soft‑tabs by default.
      buf.insert("\t");

      expect(buf.getText()).toBe("    ");
      expect(buf.getCursor()).toEqual([0, 4]);
    },
  );
});

/* -------------------------------------------------------------------------- */
/*  Undo / Redo – grouping & stack clearing                                   */
/* -------------------------------------------------------------------------- */

describe("undo / redo – advanced behaviour", () => {
  it.fails(
    "typing a word character‑by‑character should undo in one step",
    () => {
      const buf = new TextBuffer("");

      for (const ch of "hello") {
        buf.insert(ch);
      }

      // One single undo should revert the *whole* word, leaving empty buffer.
      buf.undo();

      expect(buf.getText()).toBe("");
      expect(buf.getCursor()).toEqual([0, 0]);
    },
  );
});

/* -------------------------------------------------------------------------- */
/*  Selection – cut / delete selection                                        */
/* -------------------------------------------------------------------------- */

describe("selection – cut/delete", () => {
  it.fails(
    "cut() removes the selected range and yanks it into clipboard",
    () => {
      const buf = new TextBuffer("foo bar baz");

      // Select the middle word "bar"
      buf.move("wordRight"); // after "foo" + space => col 4
      buf.startSelection();
      buf.move("wordRight"); // after "bar" (col 8)
      // @ts-expect-error – method missing in current implementation
      buf.cut();

      // Text should now read "foo  baz" (two spaces collapsed only if impl trims)
      expect(buf.getText()).toBe("foo baz");

      // Cursor should be at the start of the gap where text was removed
      expect(buf.getCursor()).toEqual([0, 4]);

      // And clipboard/yank buffer should contain the deleted word
      // @ts-expect-error – clipboard getter not exposed yet
      expect(buf.getClipboard()).toBe("bar");
    },
  );
});

/* -------------------------------------------------------------------------- */
/*  Word‑wise forward deletion (Ctrl+Delete)                                  */
/* -------------------------------------------------------------------------- */

describe("delete_next_word (Ctrl+Delete)", () => {
  it.fails("removes everything until the next word boundary", () => {
    const vp = { width: 80, height: 25 };
    const buf = new TextBuffer("hello world!!  next");

    // Place caret at start of line (0,0).  One Ctrl+Delete should wipe the
    // word "hello" and the following space.
    buf.handleInput(undefined, { delete: true, ctrl: true }, vp);

    expect(buf.getText()).toBe("world!!  next");
    expect(buf.getCursor()).toEqual([0, 0]);
  });
});

/* -------------------------------------------------------------------------- */
/*  Configurable tab length                                                   */
/* -------------------------------------------------------------------------- */

describe("tab length configuration", () => {
  it.fails("inserts the configured number of spaces when tabLen=2", () => {
    // @ts-expect-error – constructor currently has no config object
    const buf = new TextBuffer("", { tabLen: 2 });

    buf.insert("\t");

    expect(buf.getText()).toBe("  "); // two spaces
    expect(buf.getCursor()).toEqual([0, 2]);
  });
});

/* -------------------------------------------------------------------------- */
/*  Search subsystem                                                          */
/* -------------------------------------------------------------------------- */

describe("search / regex navigation", () => {
  it.fails("search_forward jumps to the next match", () => {
    const text = [
      "alpha beta gamma",
      "beta gamma alpha",
      "gamma alpha beta",
    ].join("\n");

    const buf = new TextBuffer(text);

    // @ts-expect-error – method missing
    buf.setSearchPattern(/beta/);

    // Cursor starts at 0,0.  First search_forward should land on the first
    // occurrence (row 0, col 6)
    // @ts-expect-error – method missing
    buf.searchForward();

    expect(buf.getCursor()).toEqual([0, 6]);

    // Second invocation should wrap within viewport and find next occurrence
    // (row 1, col 0)
    // @ts-expect-error – method missing
    buf.searchForward();

    expect(buf.getCursor()).toEqual([1, 0]);
  });
});

/* -------------------------------------------------------------------------- */
/*  Word‑wise navigation accuracy                                             */
/* -------------------------------------------------------------------------- */

describe("wordLeft / wordRight – punctuation boundaries", () => {
  it.fails("wordLeft stops after punctuation like hyphen (-)", () => {
    const buf = new TextBuffer("hello-world");

    // Place caret at end of line
    buf.move("end");

    // Perform a single wordLeft – in Rust implementation this lands right
    // *after* the hyphen, i.e. between '-' and 'w' (column index 6).
    buf.move("wordLeft");

    expect(buf.getCursor()).toEqual([0, 6]);
  });

  it.fails(
    "wordRight stops after punctuation like underscore (_) which is not in JS boundary set",
    () => {
      const buf = new TextBuffer("foo_bar");

      // From start, one wordRight should land right after the underscore (col 4)
      buf.move("wordRight");

      expect(buf.getCursor()).toEqual([0, 4]);
    },
  );
});

/* -------------------------------------------------------------------------- */
/*  Word‑wise deletion (Ctrl+Backspace)                                        */
/* -------------------------------------------------------------------------- */

describe("word deletion shortcuts", () => {
  it.fails("Ctrl+Backspace deletes the previous word", () => {
    const vp = { width: 80, height: 25 };
    const buf = new TextBuffer("hello world");

    // Place caret after the last character
    buf.move("end");

    // Simulate Ctrl+Backspace (terminal usually sends backspace with ctrl flag)
    buf.handleInput(undefined, { backspace: true, ctrl: true }, vp);

    // The whole word "world" (and the preceding space) should be removed,
    // leaving just "hello".
    expect(buf.getText()).toBe("hello");
    expect(buf.getCursor()).toEqual([0, 5]);
  });
});

/* -------------------------------------------------------------------------- */
/*  Paragraph navigation                                                       */
/* -------------------------------------------------------------------------- */

describe("paragraph navigation", () => {
  it.fails("Jumping forward by paragraph stops after a blank line", () => {
    const text = [
      "first paragraph line 1",
      "first paragraph line 2",
      "", // blank line separates paragraphs
      "second paragraph line 1",
    ].join("\n");

    const buf = new TextBuffer(text);

    // Start at very beginning
    // (No method exposed yet – once implemented we will call move("paragraphForward"))
    // For now we imitate the call; test will fail until the command exists.
    // @ts-expect-error – method not implemented yet
    buf.move("paragraphForward");

    // Expect caret to land at start of the first line _after_ the blank one
    expect(buf.getCursor()).toEqual([3, 0]);
  });
});

/* -------------------------------------------------------------------------- */
/*  Independent scrolling                                                     */
/* -------------------------------------------------------------------------- */

describe("viewport scrolling independent of cursor", () => {
  it.fails("scrolls without moving the caret", () => {
    const lines = Array.from({ length: 100 }, (_, i) => `line ${i}`);
    const buf = new TextBuffer(lines.join("\n"));
    const vp = { width: 10, height: 5 };

    // Cursor stays at 0,0.  We now ask the view to scroll down by one page.
    // @ts-expect-error – method not implemented yet
    buf.scroll("pageDown", vp);

    // Cursor must remain at (0,0) even though viewport origin changed.
    expect(buf.getCursor()).toEqual([0, 0]);
    // The first visible line should now be "line 5".
    expect(buf.getVisibleLines(vp)[0]).toBe("line 5");
  });
});
