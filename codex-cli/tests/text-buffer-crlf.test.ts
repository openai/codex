import { describe, expect, it } from "vitest";
import TextBuffer from "../src/text-buffer.js";

describe("TextBuffer – newline normalisation", () => {
  it("insertStr should split on \r and \r\n sequences", () => {
    const buf = new TextBuffer("");

    // Windows‑style CRLF
    buf.insertStr("ab\r\ncd\r\nef");

    expect(buf.getLines()).toEqual(["ab", "cd", "ef"]);
    expect(buf.getCursor()).toEqual([2, 2]); // after 'f'
  });
});
