import type { ResponseItem } from "openai/resources/responses/responses";

import { loadConfig } from "../config";
import { log } from "../logger/log.js";
import fs from "fs/promises";
import os from "os";
import path from "path";

const SESSIONS_ROOT = path.join(os.homedir(), ".codex", "sessions");

async function saveRolloutAsync(
  sessionId: string,
  items: Array<ResponseItem>,
  responseId?: string,
): Promise<void> {
  await fs.mkdir(SESSIONS_ROOT, { recursive: true });

  const timestamp = new Date().toISOString();
  const filename = `${sessionId}.json`;
  const filePath = path.join(SESSIONS_ROOT, filename);
  const config = loadConfig();

  try {
    const sessionData: Record<string, unknown> = {
      timestamp,
      id: sessionId,
      instructions: config.instructions,
    };
    if (responseId) {
      sessionData["responseId"] = responseId;
    }
    await fs.writeFile(
      filePath,
      JSON.stringify(
        {
          session: sessionData,
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
  responseId?: string,
): void {
  // Best-effort. We also do not log here in case of failure as that should be taken care of
  // by `saveRolloutAsync` already.
  saveRolloutAsync(sessionId, items, responseId).catch(() => {});
}
