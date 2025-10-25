export type TurnOptions = {
  /** JSON schema describing the expected agent output. */
  outputSchema?: unknown;
  /** Abort signal that cancels an in-flight turn. */
  signal?: AbortSignal;
};
