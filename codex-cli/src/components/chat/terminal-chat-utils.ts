import type { ResponseItem } from "openai/resources/responses/responses.mjs";

import { approximateTokensUsed } from "../../utils/approximate-tokens-used.js";

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
 * Returns the maximum context length (in tokens) for a given model.
 * These numbers are best‑effort guesses and provide a basis for UI percentages.
 */
export function maxTokensForModel(model: string): number {
  const lower = model.toLowerCase();
  // Heuristics for common context window sizes. Keep the checks loosely
  // ordered from *largest* to *smallest* so that more specific long‑context
  // models are detected before their shorter generic counterparts.

  // Special‑case for 1,047,576‑token demo model (gpt‑4‑long). We match either
  // the literal number or "gpt-4.1" variants we occasionally encounter.
  if (lower.includes("1,047,576") || /gpt-4\.1/i.test(lower)) {
    return 1047576;
  }

  if (lower.includes("128k") || /gpt-4\.5|gpt-4o-mini|gpt-4o\b/i.test(lower)) {
    return 128000;
  }

  // Experimental o‑series advertised at ~200k context
  if (/\bo[134]\b|o[134]-mini|o1[- ]?pro/i.test(lower)) {
    return 200000;
  }

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
  // Default to 128k for newer long‑context models
  return 128000;
}

/**
 * Calculates the percentage of tokens remaining in context for a model.
 */
export function calculateContextPercentRemaining(
  items: Array<ResponseItem>,
  model: string,
  extraContextChars = 0,
): number {
  const tokensFromItems = approximateTokensUsed(items);
  const extraTokens = Math.ceil(extraContextChars / 4);
  const used = tokensFromItems + extraTokens;
  const max = maxTokensForModel(model);
  const remaining = Math.max(0, max - used);
  return (remaining / max) * 100;
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
