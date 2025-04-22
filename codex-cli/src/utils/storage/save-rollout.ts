import type { ResponseItem } from "openai/resources/responses/responses";

import { loadConfig, SESSIONS_DIR } from "../config.js";
import { log } from "../logger/log.js";
import fs from "fs/promises";
import path from "path";

// Use the platform-specific sessions directory from config

async function saveRolloutAsync(
  sessionId: string,
  items: Array<ResponseItem>,
): Promise<void> {
  await fs.mkdir(SESSIONS_DIR, { recursive: true });

  const timestamp = new Date().toISOString();
  const ts = timestamp.replace(/[:.]/g, "-").slice(0, 10);
  const filename = `rollout-${ts}-${sessionId}.json`;
  const filePath = path.join(SESSIONS_DIR, filename);
  const config = loadConfig();

  try {
    await fs.writeFile(
      filePath,
      JSON.stringify(
        {
          session: {
            timestamp,
            id: sessionId,
            instructions: config.instructions,
          },
          items,
        },
        null,
        2,
      ),
      "utf8",
    );
  } catch (error) {
    log(`error: failed to save rollout to ${filePath}: ${error}`);
  }
}

export function saveRollout(
  sessionId: string,
  items: Array<ResponseItem>,
): void {
  // Best-effort. We also do not log here in case of failure as that should be taken care of
  // by `saveRolloutAsync` already.
  saveRolloutAsync(sessionId, items).catch(() => {});
}
