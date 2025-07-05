import type { Config, LabelConfig } from "./config";

import { getDefaultConfig } from "./default-label-config";
import { readFileSync, readdirSync, statSync } from "fs";
import * as path from "path";

/**
 * Build an in-memory configuration object by scanning the repository for
 * Markdown templates located in `.github/codex/labels`.
 *
 * Each `*.md` file in that directory represents a label that can trigger the
 * Codex GitHub Action. The filename **without** the extension is interpreted
 * as the label name, e.g. `codex-review.md` ➜ `codex-review`.
 *
 * For every such label we derive the corresponding `doneLabel` by appending
 * the suffix `-completed`.
 */
export function loadConfig(workspace: string): Config {
  const labelsDir = path.join(workspace, ".github", "codex", "labels");

  let entries: string[];
  try {
    entries = readdirSync(labelsDir);
  } catch {
    // If the directory is missing, return the default configuration.
    return getDefaultConfig();
  }

  const labels: Record<string, LabelConfig> = {};

  for (const entry of entries) {
    if (!entry.endsWith(".md")) {
      continue;
    }

    const fullPath = path.join(labelsDir, entry);

    if (!statSync(fullPath).isFile()) {
      continue;
    }

    const labelName = entry.slice(0, -3); // trim ".md"

    labels[labelName] = new FileLabelConfig(fullPath);
  }

  return { labels };
}

class FileLabelConfig implements LabelConfig {
  constructor(private readonly promptPath: string) {}

  getPromptTemplate(): string {
    return readFileSync(this.promptPath, "utf8");
  }
}
