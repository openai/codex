import type {
  Color,
  RowElement,
  StatuslineItemId,
  StatuslineState,
  UiTree,
} from "../../../../skills/src/assets/samples/create-statusline/references/statusline.js";

// Typecheck a provider as an author would: the snapshot arrives over JSON, and
// each returned element is sent as resource text.
export function render(state: StatuslineState): UiTree {
  const children: UiTree[] = [];
  const text = (value: string): UiTree => ({ type: "text", children: [value] });
  const { account, config, thread } = state;
  if (config.statusline_items.includes("model")) {
    children.push({
      type: "text",
      props: { color: "status-model", weight: "bold" },
      children: [thread.model.display_name],
    });
  }

  const workspace = account.workspace?.headline;
  if (workspace) {
    children.push({ type: "italic", children: [workspace] });
  }
  const window = account.rate_limits[0]?.primary_limit;
  if (window && window.resets_at !== null) {
    children.push(
      text(`${window.used_percent}% until ${new Date(window.resets_at * 1_000).toISOString()}`),
    );
  }
  if (thread.usage.total) {
    children.push(text(String(thread.usage.total.output_tokens)));
  }
  if (config.statusline_items.includes("current-dir")) {
    children.push(text(thread.workspace.cwd));
  }
  if (children.length === 0) {
    return null;
  }
  return { type: "row", props: { gap: 1, wrap: "wrap", justify: "between" }, children };
}

// @ts-expect-error The snapshot emits the canonical ID even when config used an alias.
const alias: StatuslineItemId = "project-root";
// @ts-expect-error Row props require a gap in terminal columns.
const missingGap: RowElement = { type: "row", props: {}, children: [] };
// @ts-expect-error Color literals begin with a hash or match one of the named tokens.
const invalidColor: Color = "ff00bb";

const groupedPositions: RowElement["props"]["justify"][] = ["center", "end"];
