import { handleDirectCommand, isDirectCommandResult } from "../src/utils/direct-command.js";
import { ReviewDecision } from "../src/utils/agent/review.js";
import { expect, test, vi, describe } from "vitest";

describe("Direct command execution", () => {
  test("isDirectCommandResult correctly identifies DirectCommandResult", () => {
    // Valid direct command result
    const validResult = {
      outputText: "test output",
      metadata: {},
      prefix: "!",
      originalCommand: "!ls",
      addToContext: false
    };
    
    // Invalid results
    const missingPrefix = {
      outputText: "test output",
      metadata: {},
      originalCommand: "!ls",
      addToContext: false
    };
    
    const missingOriginalCommand = {
      outputText: "test output",
      metadata: {},
      prefix: "!",
      addToContext: false
    };
    
    const missingAddToContext = {
      outputText: "test output",
      metadata: {},
      prefix: "!",
      originalCommand: "!ls"
    };
    
    expect(isDirectCommandResult(validResult)).toBe(true);
    expect(isDirectCommandResult(missingPrefix)).toBe(false);
    expect(isDirectCommandResult(missingOriginalCommand)).toBe(false);
    expect(isDirectCommandResult(missingAddToContext)).toBe(false);
  });
  
  test("handleDirectCommand with ! prefix does not add to context", async () => {
    // Mock the exec command handler
    const mockExecCommand = vi.fn().mockResolvedValue({
      outputText: "test output",
      metadata: {}
    });
    
    // Mock the getCommandConfirmation function
    const mockGetCommandConfirmation = vi.fn().mockResolvedValue({
      review: ReviewDecision.YES,
      command: ["ls"]
    });
    
    // Create a mock config
    const mockConfig = {
      model: "test-model",
      instructions: "",
      notify: false,
      directCommands: {
        autoApprove: true,
        addToContext: true
      }
    };
    
    // Replace the imported handleExecCommand with our mock
    vi.mock("../src/utils/agent/handle-exec-command.js", () => ({
      handleExecCommand: mockExecCommand
    }));
    
    // Call the function with ! prefix
    const result = await handleDirectCommand(
      "!ls -la",
      mockConfig,
      mockGetCommandConfirmation
    );
    
    // Verify the result
    expect(result.prefix).toBe("!");
    expect(result.originalCommand).toBe("!ls -la");
    expect(result.addToContext).toBe(false);  // ! prefix should never add to context
  });
  
  test("handleDirectCommand with $ prefix does add to context", async () => {
    // Mock the exec command handler
    const mockExecCommand = vi.fn().mockResolvedValue({
      outputText: "test output",
      metadata: {}
    });
    
    // Mock the getCommandConfirmation function
    const mockGetCommandConfirmation = vi.fn().mockResolvedValue({
      review: ReviewDecision.YES,
      command: ["ls"]
    });
    
    // Create a mock config
    const mockConfig = {
      model: "test-model",
      instructions: "",
      notify: false,
      directCommands: {
        autoApprove: true,
        addToContext: true
      }
    };
    
    // Replace the imported handleExecCommand with our mock
    vi.mock("../src/utils/agent/handle-exec-command.js", () => ({
      handleExecCommand: mockExecCommand
    }));
    
    // Call the function with $ prefix
    const result = await handleDirectCommand(
      "$ls -la",
      mockConfig,
      mockGetCommandConfirmation
    );
    
    // Verify the result
    expect(result.prefix).toBe("$");
    expect(result.originalCommand).toBe("$ls -la");
    expect(result.addToContext).toBe(true);  // $ prefix should add to context when config allows
  });
  
  test("handleDirectCommand respects config.directCommands.addToContext setting", async () => {
    // Mock the exec command handler
    const mockExecCommand = vi.fn().mockResolvedValue({
      outputText: "test output",
      metadata: {}
    });
    
    // Mock the getCommandConfirmation function
    const mockGetCommandConfirmation = vi.fn().mockResolvedValue({
      review: ReviewDecision.YES,
      command: ["ls"]
    });
    
    // Create a mock config with addToContext disabled
    const mockConfig = {
      model: "test-model",
      instructions: "",
      notify: false,
      directCommands: {
        autoApprove: true,
        addToContext: false
      }
    };
    
    // Replace the imported handleExecCommand with our mock
    vi.mock("../src/utils/agent/handle-exec-command.js", () => ({
      handleExecCommand: mockExecCommand
    }));
    
    // Call the function with $ prefix
    const result = await handleDirectCommand(
      "$ls -la",
      mockConfig,
      mockGetCommandConfirmation
    );
    
    // Verify the result
    expect(result.prefix).toBe("$");
    expect(result.originalCommand).toBe("$ls -la");
    expect(result.addToContext).toBe(false);  // Even with $ prefix, config setting disables adding to context
  });
});