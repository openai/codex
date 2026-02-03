export type CodexConfigValue = string | number | boolean | CodexConfigValue[] | CodexConfigObject;

export type CodexConfigObject = { [key: string]: CodexConfigValue };

export type CodexOptions = {
  codexPathOverride?: string;
  baseUrl?: string;
  apiKey?: string;
  /**
   * Additional `--config key=value` overrides to pass to the Codex CLI.
   *
   * Provide a JSON object and the SDK will flatten it into dotted paths and
   * serialize values as TOML literals so they are compatible with the CLI's
   * `--config` parsing.
   */
  config?: CodexConfigObject;
  /**
   * Environment variables passed to the Codex CLI process. When provided, the SDK
   * will not inherit variables from `process.env`.
   */
  env?: Record<string, string>;
};
