export type TurnOptions = {
  /** JSON schema describing the expected agent output. */
  outputSchema?: unknown;
  /** Paths to images to include with the turn input. */
  images?: string[];
};
