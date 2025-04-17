import type { ApplyPatchCommand } from "../../../approvals";
import type { AppConfig } from "../../config";
import type { ResponseInputItem, ResponseItem } from "openai/resources/responses/responses";
import type { ReviewDecision } from "../review";

/**
 * Command confirmation result returned by user when reviewing commands.
 */
export type CommandConfirmation = {
  review: ReviewDecision;
  applyPatch?: ApplyPatchCommand | undefined;
  customDenyMessage?: string;
};

/**
 * Parameters required to initialize an agent loop implementation.
 */
export type AgentLoopParams = {
  model: string;
  config?: AppConfig;
  instructions?: string;
  approvalPolicy: any; // Using 'any' here to match the existing implementation, should be replaced with proper type
  onItem: (item: ResponseItem) => void;
  onLoading: (loading: boolean) => void;

  /** Called when the command is not auto-approved to request explicit user review. */
  getCommandConfirmation: (
    command: Array<string>,
    applyPatch: ApplyPatchCommand | undefined,
  ) => Promise<CommandConfirmation>;
  onLastResponseId: (lastResponseId: string) => void;
};

/**
 * Interface defining the contract for an agent loop implementation that
 * handles communication with an AI Completion Model and manages the lifecycle
 * of requests, responses, and tool calls.
 */
export interface IAgentLoop {
  /**
   * The session identifier for the current agent loop instance.
   */
  readonly sessionId: string;

  /**
   * Cancels the current operation, aborting any in-progress API requests
   * and tool executions. This allows users to interrupt the current task.
   */
  cancel(): void;

  /**
   * Permanently terminates the agent loop. After calling this method, the 
   * instance becomes unusable and any subsequent operations will fail.
   */
  terminate(): void;

  /**
   * Runs the agent with the provided input and handles the response flow,
   * including processing function calls and streaming responses.
   * 
   * @param input The array of input items to send to the model
   * @param previousResponseId Optional ID of the previous response for conversation continuity
   */
  run(
    input: Array<ResponseInputItem>,
    previousResponseId?: string
  ): Promise<void>;
}