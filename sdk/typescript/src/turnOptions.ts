import type { RequestUserInputEvent, RequestUserInputResponse } from "./events";

export type TurnOptions = {
  /** JSON schema describing the expected agent output. */
  outputSchema?: unknown;
  /** AbortSignal to cancel the turn. */
  signal?: AbortSignal;
  /**
   * Optional handler for request_user_input prompts received during `run()`.
   * If omitted and the turn asks for input, `run()` throws.
   */
  onRequestUserInput?: (
    request: Omit<RequestUserInputEvent, "type">,
  ) => Promise<RequestUserInputResponse>;
};
