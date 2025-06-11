// Validate the inputs passed to the composite action.
// The script currently ensures that the provided configuration file exists and
// matches the expected schema.

import type { Config } from "./config";

import { existsSync } from "fs";
import * as path from "path";
import { fail } from "./fail";

export function performAdditionalValidation(config: Config, workspace: string) {
  // Additional validation: ensure referenced prompt files exist and are Markdown.
  for (const [label, details] of Object.entries(config.labels)) {
    // Determine which prompt key is present (the schema guarantees exactly one).
    const promptPathStr =
      (details as any).prompt ?? (details as any).promptPath;

    if (promptPathStr) {
      const promptPath = path.isAbsolute(promptPathStr)
        ? promptPathStr
        : path.join(workspace, promptPathStr);

      if (!existsSync(promptPath)) {
        fail(`Prompt file for label '${label}' not found: ${promptPath}`);
      }
      if (!promptPath.endsWith(".md")) {
        fail(
          `Prompt file for label '${label}' must be a .md file (got ${promptPathStr}).`,
        );
      }
    }
  }
}
