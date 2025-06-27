import type { ResponseItem } from "openai/resources/responses/responses.mjs";

import { approximateTokensUsed } from "./approximate-tokens-used.js";
import { getApiKey } from "./config.js";
import { type SupportedModelId, openAiModelInfo } from "./model-info.js";
import { createOpenAIClient } from "./openai-client.js";

const MODEL_LIST_TIMEOUT_MS = 2_000; // 2 seconds
export const RECOMMENDED_MODELS: Array<string> = ["o4-mini", "o3"];

/**
 * Background model loader / cache.
 *
 * We start fetching the list of available models from OpenAI once the CLI
 * enters interactive mode.  The request is made exactly once during the
 * lifetime of the process and the results are cached for subsequent calls.
 */
async function fetchModels(provider: string): Promise<Array<string>> {
  // If the user has not configured an API key we cannot retrieve the models.
  if (!getApiKey(provider)) {
    throw new Error("No API key configured for provider: " + provider);
  }

  try {
    const openai = createOpenAIClient({ provider });
    const list = await openai.models.list();
    const models: Array<string> = [];
    for await (const model of list as AsyncIterable<{ id?: string }>) {
      if (model && typeof model.id === "string") {
        let modelStr = model.id;
        // Fix for gemini.
        if (modelStr.startsWith("models/")) {
          modelStr = modelStr.replace("models/", "");
        }
        models.push(modelStr);
      }
    }

    return models.sort();
  } catch (error) {
    return [];
  }
}

/** Returns the list of models available for the provided key / credentials. */
export async function getAvailableModels(
  provider: string,
): Promise<Array<string>> {
  return fetchModels(provider.toLowerCase());
}

/**
 * Verifies that the provided model identifier is present in the set returned by
 * {@link getAvailableModels}.
 */
export async function isModelSupportedForResponses(
  provider: string,
  model: string | undefined | null,
): Promise<boolean> {
  if (
    typeof model !== "string" ||
    model.trim() === "" ||
    RECOMMENDED_MODELS.includes(model)
  ) {
    return true;
  }

  try {
    const models = await Promise.race<Array<string>>([
      getAvailableModels(provider),
      new Promise<Array<string>>((resolve) =>
        setTimeout(() => resolve([]), MODEL_LIST_TIMEOUT_MS),
      ),
    ]);

    // If the timeout fired we get an empty list → treat as supported to avoid
    // false negatives.
    if (models.length === 0) {
      return true;
    }

    return models.includes(model.trim());
  } catch {
    // Network or library failure → don't block start‑up.
    return true;
  }
}

/** Returns the maximum context length (in tokens) for a given model. */
export function maxTokensForModel(model: string): number {
  if (model in openAiModelInfo) {
    return openAiModelInfo[model as SupportedModelId].maxContextLength;
  }

  // fallback to heuristics for models not in the registry
  const lower = model.toLowerCase();
  if (lower.includes("32k")) {
    return 32000;
  }
  if (lower.includes("16k")) {
    return 16000;
  }
  if (lower.includes("8k")) {
    return 8000;
  }
  if (lower.includes("4k")) {
    return 4000;
  }
  return 128000; // Default to 128k for any other model.
}

/** Calculates the percentage of tokens remaining in context for a model. */
export function calculateContextPercentRemaining(
  items: Array<ResponseItem>,
  model: string,
): number {
  const used = approximateTokensUsed(items);
  const max = maxTokensForModel(model);
  const remaining = Math.max(0, max - used);
  return (remaining / max) * 100;
}

/**
 * Type‑guard that narrows a {@link ResponseItem} to one that represents a
 * user‑authored message. The OpenAI SDK represents both input *and* output
 * messages with a discriminated union where:
 *   • `type` is the string literal "message" and
 *   • `role` is one of "user" | "assistant" | "system" | "developer".
 *
 * For the purposes of de‑duplication we only care about *user* messages so we
 * detect those here in a single, reusable helper.
 */
function isUserMessage(
  item: ResponseItem,
): item is ResponseItem & { type: "message"; role: "user"; content: unknown } {
  return item.type === "message" && (item as { role?: string }).role === "user";
}

/**
 * Deduplicate the stream of {@link ResponseItem}s before they are persisted in
 * component state.
 *
 * Historically we used the (optional) {@code id} field returned by the
 * OpenAI streaming API as the primary key: the first occurrence of any given
 * {@code id} “won” and subsequent duplicates were dropped.  In practice this
 * proved brittle because locally‑generated user messages don’t include an
 * {@code id}.  The result was that if a user quickly pressed <Enter> twice the
 * exact same message would appear twice in the transcript.
 *
 * The new rules are therefore:
 *   1.  If a {@link ResponseItem} has an {@code id} keep only the *first*
 *       occurrence of that {@code id} (this retains the previous behaviour for
 *       assistant / tool messages).
 *   2.  Additionally, collapse *consecutive* user messages with identical
 *       content.  Two messages are considered identical when their serialized
 *       {@code content} array matches exactly.  We purposefully restrict this
 *       to **adjacent** duplicates so that legitimately repeated questions at
 *       a later point in the conversation are still shown.
 */
export function uniqueById(items: Array<ResponseItem>): Array<ResponseItem> {
  const seenIds = new Set<string>();
  const deduped: Array<ResponseItem> = [];

  for (const item of items) {
    // ──────────────────────────────────────────────────────────────────
    // Rule #1 – de‑duplicate by id when present
    // ──────────────────────────────────────────────────────────────────
    if (typeof item.id === "string" && item.id.length > 0) {
      if (seenIds.has(item.id)) {
        continue; // skip duplicates
      }
      seenIds.add(item.id);
    }

    // ──────────────────────────────────────────────────────────────────
    // Rule #2 – collapse consecutive identical user messages
    // ──────────────────────────────────────────────────────────────────
    if (isUserMessage(item) && deduped.length > 0) {
      const prev = deduped[deduped.length - 1]!;

      if (
        isUserMessage(prev) &&
        // Note: the `content` field is an array of message parts. Performing
        // a deep compare is over‑kill here; serialising to JSON is sufficient
        // (and fast for the tiny payloads involved).
        JSON.stringify(prev.content) === JSON.stringify(item.content)
      ) {
        continue; // skip duplicate user message
      }
    }

    deduped.push(item);
  }

  return deduped;
}
