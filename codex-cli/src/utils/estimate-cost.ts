/* eslint-disable no-irregular-whitespace */

import type { ResponseItem } from "openai/resources/responses/responses.mjs";

import { approximateTokensUsed } from "./approximate-tokens-used.js";

/**
 * Approximate per‑token pricing (in USD) for common OpenAI models.
 *
 * The list is intentionally *non‑exhaustive*: OpenAI regularly introduces new
 * variants.  Unknown model names simply result in a `null` cost estimate so
 * that callers can gracefully fall back (e.g. by omitting cost figures from
 * user‑visible summaries).
 */
const priceMap: Array<{ pattern: RegExp; pricePerThousandTokens: number }> = [
  // –––––––––––––– GPT‑4o family ––––––––––––––
  { pattern: /gpt-4o-search-preview/i, pricePerThousandTokens: 0.0025 },
  { pattern: /gpt-4o-mini-search-preview/i, pricePerThousandTokens: 0.00015 },
  { pattern: /gpt-4o-realtime-preview/i, pricePerThousandTokens: 0.005 },
  { pattern: /gpt-4o-audio-preview/i, pricePerThousandTokens: 0.0025 },
  { pattern: /gpt-4o-mini-audio-preview/i, pricePerThousandTokens: 0.00015 },
  { pattern: /gpt-4o-mini-realtime-preview/i, pricePerThousandTokens: 0.0006 },
  { pattern: /gpt-4o-mini/i, pricePerThousandTokens: 0.00015 },
  { pattern: /gpt-4o/i, pricePerThousandTokens: 0.0025 },

  // –––––––––––––– GPT‑4.1 / 4.5 ––––––––––––––
  { pattern: /gpt-4\.1-nano/i, pricePerThousandTokens: 0.0001 },
  { pattern: /gpt-4\.1-mini/i, pricePerThousandTokens: 0.0004 },
  { pattern: /gpt-4\.1/i, pricePerThousandTokens: 0.002 },

  { pattern: /gpt-4\.5-preview/i, pricePerThousandTokens: 0.075 },
  { pattern: /gpt-4\.5/i, pricePerThousandTokens: 0.075 },

  // –––––––––––––– “o‑series” experimental ––––––––––––––
  { pattern: /o4-mini/i, pricePerThousandTokens: 0.0011 },
  { pattern: /o3-mini/i, pricePerThousandTokens: 0.0011 },
  { pattern: /o1-mini/i, pricePerThousandTokens: 0.0011 },
  { pattern: /\bo3\b/i, pricePerThousandTokens: 0.015 },
  { pattern: /o1[- ]?pro/i, pricePerThousandTokens: 0.15 },
  { pattern: /\bo1\b/i, pricePerThousandTokens: 0.015 },

  // –––––––––––––– Misc ––––––––––––––
  { pattern: /computer-use-preview/i, pricePerThousandTokens: 0.003 },

  // GPT‑4 Turbo (Apr 2024)
  { pattern: /gpt-4-turbo/i, pricePerThousandTokens: 0.01 },

  // Legacy GPT‑4 8k / 32k context models
  { pattern: /gpt-4\b/i, pricePerThousandTokens: 0.03 },

  // GPT‑3.5‑Turbo family
  { pattern: /gpt-3\.5-turbo/i, pricePerThousandTokens: 0.0005 },
];

/**
 * Convert the *per‑thousand‑tokens* price entry to a *per‑token* figure.  If
 * the model is unrecognised we return `null` so that callers can fall back.
 */
export function pricePerToken(model: string): number | null {
  const entry = priceMap.find(({ pattern }) => pattern.test(model));
  if (!entry) {
    return null;
  }
  return entry.pricePerThousandTokens / 1000;
}

/**
 * Rough cost estimate (USD) for a series of {@link ResponseItem}s when using
 * the specified model.  Returns `null` when the model is unknown.
 */
export function estimateCostUSD(
  items: Array<ResponseItem>,
  model: string,
): number | null {
  const perToken = pricePerToken(model);
  if (perToken == null) {
    return null;
  }
  const tokens = approximateTokensUsed(items);
  return tokens * perToken;
}
