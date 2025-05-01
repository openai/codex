import TextBuffer from "../src/text-buffer.js";
import { describe, test, expect } from "vitest";

describe("TextBuffer – word‑wise navigation & deletion", () => {
  test("wordRight moves to end‑of‑line when no further boundary", () => {
    const tb = new TextBuffer("hello");

    // Move the caret inside the word (index 3)
    tb.move("right");
    tb.move("right");
    tb.move("right");

    tb.move("wordRight");

    const [, col] = tb.getCursor();
    expect(col).toBe(5); // end of the word / line
  });

  test("Ctrl+Backspace on raw byte deletes previous word", () => {
    const tb = new TextBuffer("hello world");
    const vp = { height: 10, width: 80 } as const;

    // Place caret at end
    tb.move("end");

    // Simulate terminal sending DEL (0x7f) byte with ctrl modifier – Ink
    // usually does *not* set `key.backspace` in this path.
    tb.handleInput("\x7f", { ctrl: true }, vp);

    expect(tb.getText()).toBe("hello ");
  });

  test("Option/Alt+Backspace deletes previous word", () => {
    const tb = new TextBuffer("foo bar baz");
    const vp = { height: 10, width: 80 } as const;

    // caret at end
    tb.move("end");

    // Simulate Option+Backspace (alt): Ink sets key.backspace = true, key.alt = true (no raw byte)
    tb.handleInput(undefined, { backspace: true, alt: true }, vp);

    expect(tb.getText()).toBe("foo bar ");
  });

  test("Option/Alt+Delete deletes previous word (matches shells)", () => {
    const tb = new TextBuffer("foo bar baz");
    const vp = { height: 10, width: 80 } as const;

    // Place caret at end so we can test backward deletion.
    tb.move("end");

    // Simulate Option+Delete (parsed as alt-modified Delete on some terminals)
    tb.handleInput(undefined, { delete: true, alt: true }, vp);

    expect(tb.getText()).toBe("foo bar ");
  });

  test("wordLeft eventually reaches column 0", () => {
    const tb = new TextBuffer("hello world");

    // Move to end of line first
    tb.move("end");

    // two wordLefts should land at start of line
    tb.move("wordLeft");
    tb.move("wordLeft");

    const [, col] = tb.getCursor();
    expect(col).toBe(0);
  });

  test("wordRight jumps over a delimiter into the next word", () => {
    const tb = new TextBuffer("hello world");

    tb.move("wordRight"); // from start – should land after "hello" (between space & w)
    let [, col] = tb.getCursor();
    expect(col).toBe(5);

    // Next wordRight should move to end of line (after "world")
    tb.move("wordRight");
    [, col] = tb.getCursor();
    expect(col).toBe(11);
  });

  test("deleteWordLeft after trailing space only deletes the last word, not the whole line", () => {
    const tb = new TextBuffer("I want you to refactor my view ");
    tb.move("end"); // Place caret after the space
    tb.deleteWordLeft();
    expect(tb.getText()).toBe("I want you to refactor my ");
    const [, col] = tb.getCursor();
    expect(col).toBe("I want you to refactor my ".length);
  });

  test("deleteWordLeft removes the previous word and positions the caret correctly", () => {
    const tb = new TextBuffer("hello world");

    // Place caret at end of line
    tb.move("end");

    // Act
    tb.deleteWordLeft();

    expect(tb.getText()).toBe("hello ");
    const [, col] = tb.getCursor();
    expect(col).toBe(6); // after the space
  });

  test("deleteWordRight removes the following word", () => {
    const tb = new TextBuffer("hello world");

    // Move caret to start of "world"
    tb.move("wordRight"); // caret after "hello"
    tb.move("right"); // skip the space, now at index 6 (start of world)

    // Act
    tb.deleteWordRight();

    expect(tb.getText()).toBe("hello ");
    const [, col] = tb.getCursor();
    expect(col).toBe(6);
  });

  test("Shift+Option/Alt+Delete deletes next word", () => {
    const tb = new TextBuffer("foo bar baz");
    const vp = { height: 10, width: 80 } as const;

    // Move caret between first and second word (after space)
    tb.move("wordRight"); // after foo
    tb.move("right"); // skip space -> start of bar

    // Shift+Option+Delete should now remove "bar "
    tb.handleInput(undefined, { delete: true, alt: true, shift: true }, vp);

    expect(tb.getText()).toBe("foo baz");
  });
});
