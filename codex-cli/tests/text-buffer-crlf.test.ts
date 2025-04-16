import TextBuffer from "../src/text-buffer.js";
import { describe, it, expect } from "vitest";

describe("TextBuffer – newline normalisation", () => {
  it("insertStr should split on \r and \r\n sequences", () => {
    const buf = new TextBuffer("");

    // Windows‑style CRLF
    buf.insertStr("ab\r\ncd\r\nef");

    expect(buf.getLines()).toEqual(["ab", "cd", "ef"]);
    expect(buf.getCursor()).toEqual([2, 2]); // after 'f'
  });
});
