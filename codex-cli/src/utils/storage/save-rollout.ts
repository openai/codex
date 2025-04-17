/* eslint-disable no-console */

import type { ResponseItem } from "openai/resources/responses/responses";

import { loadConfig, SESSIONS_DIR } from "../config";
import fs from "fs/promises";
import path from "path";

// Use the platform-specific sessions directory from config

async function saveRolloutToHomeSessions(
  items: Array<ResponseItem>,
): Promise<void> {
  await fs.mkdir(SESSIONS_DIR, { recursive: true });

  const sessionId = crypto.randomUUID();
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
    console.error(`Failed to save rollout to ${filePath}: `, error);
  }
}

let debounceTimer: NodeJS.Timeout | null = null;
let pendingItems: Array<ResponseItem> | null = null;

export function saveRollout(items: Array<ResponseItem>): void {
  pendingItems = items;
  if (debounceTimer) {
    clearTimeout(debounceTimer);
  }
  debounceTimer = setTimeout(() => {
    if (pendingItems) {
      saveRolloutToHomeSessions(pendingItems).catch(() => {});
      pendingItems = null;
    }
    debounceTimer = null;
  }, 2000);
}
