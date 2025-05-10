// This file tests the trigger pattern matching and context extraction functions
// used in the useWatchMode hook

import { describe, it, expect } from "vitest";
import {
  findAllTriggers,
  extractContextAroundTrigger,
  getTriggerPattern
} from "../src/utils/watch-mode-utils";

// For testing, we'll use a larger context size
const TEST_CONTEXT_SIZE = 20;

describe("Watch mode trigger pattern matching", () => {
  it("should detect double-slash (JS-style) CODEX triggers", () => {
    const content = `
    function testFunction() {
      // This is a normal comment
      // CODEX: Fix this bug
      return 1 + 1;
    }
    `;

    const matches = findAllTriggers(content);

    expect(matches.length).toBe(1);
    expect(matches[0]![0]).toContain("// CODEX: Fix this bug");
    expect(matches[0]![1]).toBe("Fix this bug");
  });

  it("should detect CODEX triggers with different indentation", () => {
    const content = `
    def test_function():
      # This is a normal comment
      // CODEX: What does this function do
      return 1 + 1
    `;

    const matches = findAllTriggers(content);

    expect(matches.length).toBe(1);
    expect(matches[0]![0]).toContain("// CODEX: What does this function do");
    expect(matches[0]![1]).toBe("What does this function do");
  });


  it("should detect multiple CODEX triggers in a single file", () => {
    const content = `
    function testFunction() {
      // CODEX: Fix this bug
      return 1 + 1;
    }
    
    function anotherFunction() {
      // CODEX: What does this function do
      return 2 + 2;
    }
    
    function thirdFunction() {
      // CODEX: Optimize this algorithm
      return 3 + 3;
    }
    `;

    const matches = findAllTriggers(content);

    expect(matches.length).toBe(3);
    expect(matches[0]![1]).toBe("Fix this bug");
    expect(matches[1]![1]).toBe("What does this function do");
    expect(matches[2]![1]).toBe("Optimize this algorithm");
  });

  it("should handle CODEX pattern with question", () => {
    const content = `
    function testFunction() {
      // CODEX: What's going on here?
      return 1 + 1;
    }
    `;

    const matches = findAllTriggers(content);

    expect(matches.length).toBe(1);
    expect(matches[0]![1]).toBe("What's going on here?");
  });

  it("should handle CODEX pattern with imperative", () => {
    const content = `
    function testFunction() {
      // CODEX: Fix this
      return 1 + 1;
    }
    `;

    const matches = findAllTriggers(content);

    expect(matches.length).toBe(1);
    expect(matches[0]![1]).toBe("Fix this");
  });

  it("should ignore non-CODEX comments", () => {
    const content = `
    function testFunction() {
      // This is a normal comment
      // CODEX is a great tool
      // This uses a CODEX model
      return 1 + 1;
    }
    `;

    const matches = findAllTriggers(content);

    expect(matches.length).toBe(0);
  });

  it("should not detect SQL-style (--) comments with the new pattern", () => {
    const content = `
    SELECT * FROM users
    -- This is a normal comment
    -- CODEX: Optimize this query
    WHERE age > 18;
    `;

    const matches = findAllTriggers(content);

    // Should not match because the pattern only looks for // CODEX:
    expect(matches.length).toBe(0);
  });

  
  it("should handle custom trigger patterns", () => {
    // Create custom patterns for testing
    const customPatternString = '/(?:\\/\\/|#)\\s*AI:(TODO|FIXME)\\s+(.*)/i';
    const match = customPatternString.match(/^\/(.*)\/([gimuy]*)$/);
    const [, pattern, flags] = match!;
    const customPattern = new RegExp(pattern, flags + 'g');
    
    const content = `
    function testFunction() {
      // This is a normal comment
      // AI:TODO Fix this bug
      return 1 + 1;
    }
    
    function anotherFunction() {
      # AI:FIXME Handle null input
      return x * 2;
    }
    `;

    const matches: Array<RegExpMatchArray> = [];
    let matchResult;
    while ((matchResult = customPattern.exec(content)) != null) {
      matches.push(matchResult);
    }

    expect(matches.length).toBe(2);
    expect(matches[0]![0]).toContain("AI:TODO Fix this bug");
    expect(matches[0]![2]).toBe("Fix this bug");
    expect(matches[1]![0]).toContain("AI:FIXME Handle null input");
    expect(matches[1]![2]).toBe("Handle null input");
  });
});

describe("Context extraction around CODEX triggers", () => {
  it("should extract the correct context around a trigger in the middle of the file", () => {
    const content = `// File header
import { useState } from 'react';

// Component definition
function Counter() {
  // State initialization
  const [count, setCount] = useState(0);
  
  // CODEX: Fix this increment function
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
      TEST_CONTEXT_SIZE,
    );

    // Should include appropriate context around the trigger (the entire file in this case)
    // Use includes instead of exact equality to handle whitespace differences
    expect(context).toContain("// CODEX: Fix this increment function");

    // Should extract the instruction correctly
    expect(instruction).toBe("Fix this increment function");
  });

  it("should extract a limited context when file is very large", () => {
    // Create a large file with 100 lines
    const fileLines = Array.from({ length: 100 }, (_, i) => `// Line ${i + 1}`);

    // Insert the trigger at line 50
    fileLines[49] = "// CODEX: Optimize this code";

    const content = fileLines.join("\n");
    const matches = findAllTriggers(content);

    const { context, instruction } = extractContextAroundTrigger(
      content,
      matches[0]!,
      TEST_CONTEXT_SIZE,
    );

    // Should only include the default number of lines around the trigger (20 before, 20 after)
    const contextLines = context.split("\n");
    expect(contextLines.length).toBeLessThan(50); // Less than the full 100 lines
    expect(contextLines.length).toBeGreaterThanOrEqual(41); // At least the trigger line + 20 before + 20 after

    // Should include the trigger line
    expect(context).toContain("// CODEX: Optimize this code");

    // Should extract the instruction correctly
    expect(instruction).toBe("Optimize this code");
  });

  it("should handle triggers at the beginning of the file", () => {
    const content = `// CODEX: Explain this code
function complexFunction() {
  return [1, 2, 3].map(x => x * 2).reduce((a, b) => a + b, 0);
}`;

    const matches = findAllTriggers(content);

    const { context, instruction } = extractContextAroundTrigger(
      content,
      matches[0]!,
      TEST_CONTEXT_SIZE,
    );

    // Should include the entire short file
    expect(context).toBe(content);

    // Should extract the instruction correctly
    expect(instruction).toBe("Explain this code");
  });

  it("should handle triggers at the end of the file", () => {
    const content = `function complexFunction() {
  return [1, 2, 3].map(x => x * 2).reduce((a, b) => a + b, 0);
}
// CODEX: Explain this code`;

    const matches = findAllTriggers(content);

    const { context, instruction } = extractContextAroundTrigger(
      content,
      matches[0]!,
      TEST_CONTEXT_SIZE,
    );

    // Should include the entire short file
    expect(context).toBe(content);

    // Should extract the instruction correctly
    expect(instruction).toBe("Explain this code");
  });
});
