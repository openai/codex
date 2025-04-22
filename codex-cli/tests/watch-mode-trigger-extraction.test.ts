// This file tests the trigger pattern matching and context extraction functions
// used in the useWatchMode hook

import { describe, it, expect } from "vitest";

// These functions are private in the hook, so we need to recreate them for testing
const TRIGGER_PATTERN =
  /\/\/\s*(.*),?\s*AI[!?]|#\s*(.*),?\s*AI[!?]|\/\*\s*(.*),?\s*AI[!?]\s*\*\//;

function findAllTriggers(content: string): Array<RegExpMatchArray> {
  const matches: Array<RegExpMatchArray> = [];
  const regex = new RegExp(TRIGGER_PATTERN, "g");

  let match;
  while ((match = regex.exec(content)) != null) {
    matches.push(match);
  }

  return matches;
}

function extractContextAroundTrigger(
  content: string,
  triggerMatch: RegExpMatchArray,
): { context: string; instruction: string } {
  // Default context size (number of lines before and after the trigger)
  const contextSize = 20;

  // Get the lines of the file
  const lines = content.split("\n");

  // Find the line number of the trigger
  const triggerPos =
    content.substring(0, triggerMatch.index).split("\n").length - 1;

  // Calculate start and end lines for context
  const startLine = Math.max(0, triggerPos - contextSize);
  const endLine = Math.min(lines.length - 1, triggerPos + contextSize);

  // Extract the context lines
  const contextLines = lines.slice(startLine, endLine + 1);

  // Join the context lines back together
  const context = contextLines.join("\n");

  // Extract the instruction from the capture groups
  // The regex has 3 capture groups for different comment styles:
  // Group 1: // instruction AI!
  // Group 2: # instruction AI!
  // Group 3: /* instruction AI! */
  const instruction =
    triggerMatch[1] ||
    triggerMatch[2] ||
    triggerMatch[3] ||
    "fix or improve this code";

  return { context, instruction };
}

describe("Watch mode trigger pattern matching", () => {
  it("should detect double-slash (JS-style) AI triggers", () => {
    const content = `
    function testFunction() {
      // This is a normal comment
      // Fix this bug, AI!
      return 1 + 1;
    }
    `;

    const matches = findAllTriggers(content);

    expect(matches.length).toBe(1);
    expect(matches[0]![0]).toContain("Fix this bug, AI!");
    expect(matches[0]![1]).toBe("Fix this bug, ");
  });

  it("should detect hash (Python/Ruby-style) AI triggers", () => {
    const content = `
    def test_function():
      # This is a normal comment
      # What does this function do, AI?
      return 1 + 1
    `;

    const matches = findAllTriggers(content);

    expect(matches.length).toBe(1);
    expect(matches[0]![0]).toContain("# What does this function do, AI?");
    expect(matches[0]![2]).toBe("What does this function do, ");
  });

  it("should detect block comment (CSS/C-style) AI triggers", () => {
    const content = `
    function testFunction() {
      /* This is a normal block comment */
      /* Refactor this code, AI! */
      return 1 + 1;
    }
    `;

    const matches = findAllTriggers(content);

    expect(matches.length).toBe(1);
    expect(matches[0]![0]).toContain("/* Refactor this code, AI! */");
    expect(matches[0]![3]).toBe("Refactor this code, ");
  });

  it("should detect multiple AI triggers in a single file", () => {
    const content = `
    function testFunction() {
      // Fix this bug, AI!
      return 1 + 1;
    }
    
    function anotherFunction() {
      # What does this function do, AI?
      return 2 + 2;
    }
    
    /* Refactor this code, AI! */
    function thirdFunction() {
      return 3 + 3;
    }
    `;

    const matches = findAllTriggers(content);

    expect(matches.length).toBe(3);
    expect(matches[0]![1]).toBe("Fix this bug, ");
    expect(matches[1]![2]).toBe("What does this function do, ");
    expect(matches[2]![3]).toBe("Refactor this code, ");
  });

  it("should handle AI! pattern with question mark", () => {
    const content = `
    function testFunction() {
      // What's going on here, AI?
      return 1 + 1;
    }
    `;

    const matches = findAllTriggers(content);

    expect(matches.length).toBe(1);
    expect(matches[0]![1]).toBe("What's going on here, ");
  });

  it("should handle AI! pattern with exclamation mark", () => {
    const content = `
    function testFunction() {
      // Fix this, AI!
      return 1 + 1;
    }
    `;

    const matches = findAllTriggers(content);

    expect(matches.length).toBe(1);
    expect(matches[0]![1]).toBe("Fix this, ");
  });

  it("should ignore non-AI comments", () => {
    const content = `
    function testFunction() {
      // This is a normal comment
      // AI is an interesting topic
      // This uses an AI model
      return 1 + 1;
    }
    `;

    const matches = findAllTriggers(content);

    expect(matches.length).toBe(0);
  });
});

describe("Context extraction around AI triggers", () => {
  it("should extract the correct context around a trigger in the middle of the file", () => {
    const content = `// File header
import { useState } from 'react';

// Component definition
function Counter() {
  // State initialization
  const [count, setCount] = useState(0);
  
  // Fix this increment function, AI!
  const increment = () => {
    setCount(count);  // Bug: doesn't increment
  };
  
  // Decrement function
  const decrement = () => {
    setCount(count - 1);
  };
  
  // Render
  return (
    <div>
      <button onClick={decrement}>-</button>
      <span>{count}</span>
      <button onClick={increment}>+</button>
    </div>
  );
}

export default Counter;`;

    const matches = findAllTriggers(content);
    expect(matches.length).toBe(1);

    const { context, instruction } = extractContextAroundTrigger(
      content,
      matches[0]!,
    );

    // Should include appropriate context around the trigger (the entire file in this case)
    expect(context).toBe(content);

    // Should extract the instruction correctly
    expect(instruction).toBe("Fix this increment function, ");
  });

  it("should extract a limited context when file is very large", () => {
    // Create a large file with 100 lines
    const fileLines = Array.from({ length: 100 }, (_, i) => `// Line ${i + 1}`);

    // Insert the trigger at line 50
    fileLines[49] = "// Optimize this code, AI!";

    const content = fileLines.join("\n");
    const matches = findAllTriggers(content);

    const { context, instruction } = extractContextAroundTrigger(
      content,
      matches[0]!,
    );

    // Should only include the default number of lines around the trigger (15 before, 15 after)
    const contextLines = context.split("\n");
    expect(contextLines.length).toBeLessThan(50); // Less than the full 100 lines
    expect(contextLines.length).toBeGreaterThanOrEqual(31); // At least the trigger line + 15 before + 15 after

    // Should include the trigger line
    expect(context).toContain("// Optimize this code, AI!");

    // Should extract the instruction correctly
    expect(instruction).toBe("Optimize this code, ");
  });

  it("should handle triggers at the beginning of the file", () => {
    const content = `// Explain this code, AI!
function complexFunction() {
  return [1, 2, 3].map(x => x * 2).reduce((a, b) => a + b, 0);
}`;

    const matches = findAllTriggers(content);

    const { context, instruction } = extractContextAroundTrigger(
      content,
      matches[0]!,
    );

    // Should include the entire short file
    expect(context).toBe(content);

    // Should extract the instruction correctly
    expect(instruction).toBe("Explain this code, ");
  });

  it("should handle triggers at the end of the file", () => {
    const content = `function complexFunction() {
  return [1, 2, 3].map(x => x * 2).reduce((a, b) => a + b, 0);
}
// Explain this code, AI!`;

    const matches = findAllTriggers(content);

    const { context, instruction } = extractContextAroundTrigger(
      content,
      matches[0]!,
    );

    // Should include the entire short file
    expect(context).toBe(content);

    // Should extract the instruction correctly
    expect(instruction).toBe("Explain this code, ");
  });
});

