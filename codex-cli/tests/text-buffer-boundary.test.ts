import TextBuffer from "../src/text-buffer";
import { describe, it, expect } from "vitest";

describe("TextBuffer – boundary tests", () => {
  describe("extremely long lines", () => {
    it("should handle very long single line", () => {
      const longLine = "a".repeat(10000);
      const buf = new TextBuffer(longLine);
      
      // Test cursor movement in long line
      buf.move("end");
      expect(buf.getCursor()).toEqual([0, 10000]);
      
      // Test insert in middle of long line
      buf.move("left");
      buf.insert("b");
      expect(buf.getText().length).toBe(10001);
      
      // Test delete in long line
      buf.del();
      expect(buf.getText().length).toBe(10000);
    });
  });

  describe("large number of lines", () => {
    it("should handle buffer with many lines", () => {
      const manyLines = Array(1000).fill("test").join("\n");
      const buf = new TextBuffer(manyLines);
      
      // Test cursor movement through many lines
      buf.move("end");
      expect(buf.getCursor()[0]).toBe(999);
      
      // Test insert at end of many lines
      buf.insert("x");
      expect(buf.getLines().length).toBe(1000);
      
      // Test newline in middle of many lines
      buf.move("up");
      buf.move("end");
      buf.insert("\n");
      expect(buf.getLines().length).toBe(1001);
    });
  });
}); 