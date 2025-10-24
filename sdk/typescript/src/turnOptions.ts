export type TurnOptions = {
  /** JSON schema describing the expected agent output. */
  outputSchema?: unknown;
  /** Codex profile to apply for this turn. */
  profile?: string;
};
