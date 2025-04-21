import { describe, test, expect, vi, beforeEach } from "vitest";
import {
  normalizeShellCommand,
  processShellToolInput,
  parseClaudeToolCall,
  claudeToolToOpenAIFunction,
  createDefaultClaudeTools,
  createShellCommandInstructions
} from "../src/utils/providers/claude-tools.js";

// Mock console.log to avoid cluttering test output
vi.spyOn(console, 'log').mockImplementation(() => {});

describe("Claude shell command handling", () => {
  test("normalizeShellCommand should return 'not implemented' message", () => {
    // Test with string input
    const result1 = normalizeShellCommand("ls -la");
    expect(result1).toEqual(["echo", "Shell commands are not implemented in claude provider"]);
    
    // Test with array input
    const result2 = normalizeShellCommand(["ls", "-la"]);
    expect(result2).toEqual(["echo", "Shell commands are not implemented in claude provider"]);
    
    // Test with undefined input
    const result3 = normalizeShellCommand(undefined);
    expect(result3).toEqual(["echo", "Shell commands are not implemented in claude provider"]);
  });
  
  test("processShellToolInput should return 'not implemented' message", () => {
    // Test with string input
    const result1 = processShellToolInput("ls -la");
    expect(result1.command).toEqual(["echo", "Shell commands are not implemented in claude provider"]);
    
    // Test with object input
    const result2 = processShellToolInput({ command: ["ls", "-la"] });
    expect(result2.command).toEqual(["echo", "Shell commands are not implemented in claude provider"]);
  });
  
  test("parseClaudeToolCall should handle shell tool specially", () => {
    // Test with shell tool
    const result1 = parseClaudeToolCall({
      id: "tool_123",
      name: "shell",
      input: "ls -la"
    });
    expect(result1.arguments.command).toEqual(["echo", "Shell commands are not implemented in claude provider"]);
    
    // Test with non-shell tool
    const result2 = parseClaudeToolCall({
      id: "tool_456",
      name: "other_tool",
      input: { some: "data" }
    });
    expect(result2.arguments).toEqual({ some: "data" });
  });
  
  test("claudeToolToOpenAIFunction should handle shell tool specially", () => {
    // Test with shell tool
    const result1 = claudeToolToOpenAIFunction({
      id: "tool_123",
      name: "shell",
      input: "ls -la"
    });
    expect(JSON.parse(result1.arguments).command).toEqual(["echo", "Shell commands are not implemented in claude provider"]);
    
    // Test with non-shell tool
    const result2 = claudeToolToOpenAIFunction({
      id: "tool_456",
      name: "other_tool",
      input: { some: "data" }
    });
    expect(JSON.parse(result2.arguments)).toEqual({ some: "data" });
  });
  
  test("createDefaultClaudeTools should return empty array", () => {
    const result = createDefaultClaudeTools();
    expect(result).toEqual([]);
  });
  
  test("createShellCommandInstructions should return notice message", () => {
    const result = createShellCommandInstructions();
    expect(result).toContain("Shell commands are not implemented");
    expect(result).toContain("Any attempt to use the shell tool will result in a message");
  });
});