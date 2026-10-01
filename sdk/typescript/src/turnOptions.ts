export type CyberAccessProgram = "standard" | "daybreak_blue" | "daybreak_red";

export type TurnOptions = {
  /**
   * Experimental Cyber selection for this turn with the built-in OpenAI provider.
   * Omit to use server defaults.
   */
  cyberAccessProgram?: CyberAccessProgram;
  /** JSON schema describing the expected agent output. */
  outputSchema?: unknown;
  /** AbortSignal to cancel the turn. */
  signal?: AbortSignal;
};
