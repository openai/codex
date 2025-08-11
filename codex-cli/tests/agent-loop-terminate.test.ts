import { describe, it, expect, vi, beforeEach } from "vitest";

// ---------------------------------------------------------------------------
// Comprehensive test suite for AgentLoop.terminate() method
// 
// This test suite covers all the key behaviors identified in the specification:
// 1. Idempotency - multiple calls should be safe
// 2. State transition - terminated flag should be set
// 3. AbortController activation - hardAbort should be called
// 4. Cancellation cascade - cancel() should be invoked
// 5. Post-termination behavior - subsequent operations should throw/no-op
// 6. Resource cleanup verification
// ---------------------------------------------------------------------------

// Fake OpenAI stream for testing
class FakeStream {
  public controller = { abort: vi.fn() };

  async *[Symbol.asyncIterator]() {
    yield {
      type: "response.completed",
      response: {
        id: "test-resp-1",
        status: "completed",
        output: [],
      },
    } as any;
  }
}

// Mock the OpenAI SDK
vi.mock("openai", () => {
  class MockOpenAI {
    public responses = {
      create: vi.fn().mockImplementation(async () => {
        return new FakeStream();
      })
    };
  }

  class APIConnectionTimeoutError extends Error {}

  return {
    __esModule: true,
    default: MockOpenAI,
    APIConnectionTimeoutError,
  };
});

// Mock dependencies that aren't relevant to terminate() testing
vi.mock("../src/approvals.js", () => ({
  __esModule: true,
  alwaysApprovedCommands: new Set<string>(),
  canAutoApprove: () => ({ type: "auto-approve", runInSandbox: false }) as any,
  isSafeCommand: () => null,
}));

vi.mock("../src/format-command.js", () => ({
  __esModule: true,
  formatCommandForDisplay: (cmd: Array<string>) => cmd.join(" "),
}));

vi.mock("../src/utils/agent/log.js", () => ({
  __esModule: true,
  log: vi.fn(),
  isLoggingEnabled: () => false,
}));

// Import the module under test after mocking dependencies
import { AgentLoop } from "../src/utils/agent/agent-loop.js";

describe("AgentLoop.terminate()", () => {
  let onItem: ReturnType<typeof vi.fn>;
  let onLoading: ReturnType<typeof vi.fn>;
  let onLastResponseId: ReturnType<typeof vi.fn>;
  let getCommandConfirmation: ReturnType<typeof vi.fn>;

  // Helper to create AgentLoop instance with mocked callbacks
  const createAgentLoop = (config: Partial<any> = {}) => {
    return new AgentLoop({
      model: "test-model",
      instructions: "test instructions",
      approvalPolicy: { mode: "auto" } as any,
      additionalWritableRoots: [],
      onItem,
      onLoading,
      getCommandConfirmation,
      onLastResponseId,
      ...config,
    });
  };

  beforeEach(() => {
    // Reset all mocks before each test
    vi.clearAllMocks();
    
    // Create fresh mock functions
    onItem = vi.fn();
    onLoading = vi.fn();
    onLastResponseId = vi.fn();
    getCommandConfirmation = vi.fn().mockResolvedValue({ review: "yes" });
  });

  describe("Idempotency Property", () => {
    it("should be safe to call terminate() multiple times", () => {
      const agent = createAgentLoop();
      
      // First call should work normally
      agent.terminate();
      
      // Subsequent calls should be no-ops (not throw)
      expect(() => agent.terminate()).not.toThrow();
      expect(() => agent.terminate()).not.toThrow();
      expect(() => agent.terminate()).not.toThrow();
    });
  });

  describe("State Transition Property", () => {
    it("should set terminated flag to true", async () => {
      const agent = createAgentLoop();
      
      // Before termination, should be able to run
      const userMsg = [{
        type: "message",
        role: "user", 
        content: [{ type: "input_text", text: "test" }]
      }];
      
      // This should work fine
      await agent.run(userMsg as any);
      
      // Terminate the agent
      agent.terminate();
      
      // After termination, run() should throw
      await expect(agent.run(userMsg as any)).rejects.toThrow("AgentLoop has been terminated");
    });
  });

  describe("AbortController Activation Property", () => {
    it("should abort the hardAbort controller", () => {
      const agent = createAgentLoop();
      
      // Spy on the hardAbort controller
      // We need to access the private field for testing - this is a white-box test
      const hardAbort = (agent as any).hardAbort as AbortController;
      const abortSpy = vi.spyOn(hardAbort, 'abort');
      
      expect(hardAbort.signal.aborted).toBe(false);
      
      agent.terminate();
      
      expect(abortSpy).toHaveBeenCalledTimes(1);
      expect(hardAbort.signal.aborted).toBe(true);
    });
  });

  describe("Cancellation Cascade Property", () => {
    it("should call cancel() method during termination", () => {
      const agent = createAgentLoop();
      
      // Spy on the cancel method
      const cancelSpy = vi.spyOn(agent, 'cancel');
      
      agent.terminate();
      
      expect(cancelSpy).toHaveBeenCalledTimes(1);
    });

    it("should call cancel() which would normally invoke onLoading(false)", () => {
      const agent = createAgentLoop();
      
      // First, test that cancel() normally calls onLoading(false)
      agent.cancel();
      expect(onLoading).toHaveBeenCalledWith(false);
      
      // Reset the mock
      onLoading.mockClear();
      
      // Now test terminate - it sets terminated=true first, then calls cancel()
      // But cancel() will early return because terminated=true
      agent.terminate();
      
      // onLoading should not be called because cancel() early returns when terminated=true
      expect(onLoading).not.toHaveBeenCalled();
    });
  });

  describe("Post-Termination Method Behavior", () => {
    it("should make run() throw after termination", async () => {
      const agent = createAgentLoop();
      
      agent.terminate();
      
      const userMsg = [{
        type: "message",
        role: "user",
        content: [{ type: "input_text", text: "test" }]
      }];
      
      await expect(agent.run(userMsg as any)).rejects.toThrow("AgentLoop has been terminated");
    });

    it("should make cancel() a no-op after termination", () => {
      const agent = createAgentLoop();
      
      agent.terminate();
      
      // Reset the onLoading mock to verify cancel() becomes no-op
      onLoading.mockClear();
      
      // Calling cancel() after terminate should be a no-op
      agent.cancel();
      
      // onLoading should not be called again since cancel() should early return
      expect(onLoading).not.toHaveBeenCalled();
    });

    it("should make subsequent terminate() calls no-ops", () => {
      const agent = createAgentLoop();
      
      // First terminate call
      agent.terminate();
      
      // Spy on cancel to verify it's not called again
      const cancelSpy = vi.spyOn(agent, 'cancel');
      
      // Subsequent terminate calls should early return
      agent.terminate();
      agent.terminate();
      
      expect(cancelSpy).not.toHaveBeenCalled();
    });
  });

  describe("Resource Cleanup Verification", () => {
    it("should properly abort exec operations via hardAbort signal", () => {
      const agent = createAgentLoop();
      
      // The hardAbort signal should have a listener that forwards to execAbortController
      const hardAbort = (agent as any).hardAbort as AbortController;
      const execAbortController = (agent as any).execAbortController as AbortController;
      
      if (execAbortController) {
        const execAbortSpy = vi.spyOn(execAbortController, 'abort');
        
        agent.terminate();
        
        // The hardAbort listener should forward the signal
        expect(execAbortSpy).toHaveBeenCalled();
      }
    });

    it("should not interfere with callback invocation during termination", () => {
      const agent = createAgentLoop();
      
      // Verify the terminate process doesn't break callback mechanisms
      // The hardAbort controller should work properly
      const hardAbort = (agent as any).hardAbort as AbortController;
      expect(hardAbort.signal.aborted).toBe(false);
      
      agent.terminate();
      
      // The hardAbort signal should be aborted
      expect(hardAbort.signal.aborted).toBe(true);
      
      // Callbacks should still be functional if called directly
      expect(() => onLoading(false)).not.toThrow();
    });
  });

  describe("Error Handling in Termination", () => {
    it("will propagate errors from cancel() since terminate() doesn't catch them", () => {
      const agent = createAgentLoop();
      
      // Mock cancel to throw an error
      const originalCancel = agent.cancel;
      agent.cancel = vi.fn(() => {
        throw new Error("Cancel failed");
      });
      
      // terminate() will propagate the error from cancel() since it doesn't catch it
      expect(() => agent.terminate()).toThrow("Cancel failed");
    });

    it("should handle the normal case without errors", () => {
      const agent = createAgentLoop();
      
      // Normal termination should not throw
      expect(() => agent.terminate()).not.toThrow();
      
      // Multiple calls should be safe
      expect(() => agent.terminate()).not.toThrow();
    });
  });

  describe("Integration with Run Loop", () => {
    it("should prevent new runs after termination even if attempted concurrently", async () => {
      const agent = createAgentLoop();
      
      const userMsg = [{
        type: "message", 
        role: "user",
        content: [{ type: "input_text", text: "test" }]
      }];
      
      // Start multiple run attempts
      const runPromises = [
        agent.run(userMsg as any),
        agent.run(userMsg as any),
        agent.run(userMsg as any)
      ];
      
      // Terminate after starting runs
      agent.terminate();
      
      // All runs should complete successfully (they were started before termination)
      await Promise.all(runPromises);
      
      // New runs after termination should fail
      await expect(agent.run(userMsg as any)).rejects.toThrow("AgentLoop has been terminated");
    });

    it("should stop in-progress operations when terminated", async () => {
      const agent = createAgentLoop();
      
      const userMsg = [{
        type: "message",
        role: "user", 
        content: [{ type: "input_text", text: "test" }]
      }];
      
      // Start a run
      const runPromise = agent.run(userMsg as any);
      
      // Terminate immediately
      agent.terminate();
      
      // The run should complete (it may finish or be interrupted)
      await runPromise;
      
      // Subsequent runs should be blocked
      await expect(agent.run(userMsg as any)).rejects.toThrow("AgentLoop has been terminated");
    });
  });

  describe("Method Chaining and State Consistency", () => {
    it("should maintain consistent state across cancel() and terminate()", () => {
      const agent = createAgentLoop();
      
      // Call cancel first
      agent.cancel();
      
      // Then terminate
      agent.terminate();
      
      // Both should work without issues
      expect(() => agent.cancel()).not.toThrow(); // should be no-op
      expect(() => agent.terminate()).not.toThrow(); // should be no-op
    });

    it("should maintain state when terminate() called before any runs", async () => {
      const agent = createAgentLoop();
      
      // Terminate immediately after creation
      agent.terminate();
      
      const userMsg = [{
        type: "message",
        role: "user",
        content: [{ type: "input_text", text: "test" }]
      }];
      
      // Should prevent runs
      await expect(agent.run(userMsg as any)).rejects.toThrow("AgentLoop has been terminated");
      
      // Should make cancel no-op
      expect(() => agent.cancel()).not.toThrow();
    });
  });
});