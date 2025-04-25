import type { ResponseItem } from "openai/resources/responses/responses.mjs";

/**
 * Represents a grouped sequence of response items (e.g., function call batches).
 */
export type GroupedResponseItem = {
  label: string;
  items: Array<ResponseItem>;
};
