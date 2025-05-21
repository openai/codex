import type { ConfirmationResult } from "./hooks/use-confirmation";

import { ReviewDecision } from "./utils/agent/review";
import axios from "axios";

/**
 * Send a permission request to a remote human and wait for their response.
 *
 * Expects the server to reply with JSON shaped like:
 *   { decision: "yes" | "no-continue" | "no-exit" | "explain" | "always", customDenyMessage?: string }
 *
 * @param url The full POST URL of the confirmation server (e.g., http://localhost:8000/ask)
 * @param prompt A message describing the action for which confirmation is needed
 * @param explanation Optional additional explanation
 * @returns ConfirmationResult with decision and optional custom message
 * @throws If the network request fails or the server response is invalid
 */
export async function requestRemotePermission(
  url: string,
  prompt: string,
  explanation?: string,
): Promise<ConfirmationResult> {
  const requestId = crypto.randomUUID();
  const fullMessage = explanation ? `${prompt}\n\n${explanation}` : prompt;
  try {
    // eslint-disable-next-line no-console
    console.log(`[permission-client] Asking for confirmation: ${fullMessage}`);
    const response = await axios.post<ConfirmationResult>(
      url,
      {
        request_id: requestId,
        message: fullMessage,
      },
      {
        timeout: 0,
      },
    );

    const { decision, customDenyMessage } = response.data;

    if (!Object.values(ReviewDecision).includes(decision)) {
      throw new Error(
        `Unexpected decision value from permission server: ${decision}`,
      );
    }

    return { decision, customDenyMessage };
  } catch (err) {
    throw new Error(
      `Failed to get confirmation from permission server: ${(err as Error).message}`,
    );
  }
}
