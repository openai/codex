import type {
  CodeElement,
  ColElement,
  BoxElement,
  BadgeElement,
  SpacerElement,
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

const indented: UiTree = { type: "text", props: { preserveWhitespace: true }, children: ["  ready"] };

const summary: UiTree = { type: "text", props: { maxLines: 2 }, children: ["build results"] };

const spacer: SpacerElement = { type: "spacer" };
const emptySpacer: SpacerElement = { ...spacer, children: [] };
// @ts-expect-error A spacer has no contents.
const invalidSpacer: SpacerElement = { ...spacer, children: ["x"] };

const badge: BadgeElement = { type: "badge", children: ["queued"] };
const passing: BadgeElement = { ...badge, props: { color: "success" }, children: ["passing"] };
// @ts-expect-error Badges accept semantic colors.
const invalidBadge: BadgeElement = { ...badge, props: { color: "red" } };

const stacked: BoxElement = { type: "box", props: { gap: 1 }, children: [summary, indented] };
const horizontal: BoxElement = { ...stacked, props: { gap: 0, direction: "row", justify: "end" } };
const responsive = (compact: boolean): BoxElement => ({
  ...stacked,
  props: { gap: 1, direction: compact ? "col" : "row" },
});
const responsiveAlignment = (compact: boolean): BoxElement["props"] =>
  compact ? { gap: 1 } : { gap: 1, direction: "row", justify: "end" };
// @ts-expect-error Omitted direction defaults to a column.
const implicitColumn: BoxElement["props"] = { gap: 1, justify: "end" };
const invalidResponsive = (compact: boolean): BoxElement["props"] =>
  // @ts-expect-error The column branch cannot carry row-only props.
  ({ gap: 1, direction: compact ? "col" : "row", wrap: "wrap" });
// @ts-expect-error Boxes require a gap in either direction.
const missingBoxGap: BoxElement = { ...stacked, props: { direction: "col" } };

const command: CodeElement = {
  type: "code",
  children: ["cargo ", null, { type: "bold", children: [" test"] }],
};
// @ts-expect-error Code requires a child list.
const invalidCode: CodeElement = { type: "code", children: "cargo test" };

// @ts-expect-error The snapshot emits the canonical ID even when config used an alias.
const alias: StatuslineItemId = "project-root";
// @ts-expect-error Row props require a gap in terminal columns.
const missingGap: RowElement = { type: "row", props: {}, children: [] };
const twoLines: ColElement = {
  type: "col",
  props: { gap: 0 },
  children: [command, { type: "text", children: ["75%"] }],
};
// @ts-expect-error Column props require a gap in terminal rows.
const missingColGap: ColElement = { type: "col", props: {}, children: [] };
// @ts-expect-error Color literals begin with a hash or match one of the named tokens.
const invalidColor: Color = "ff00bb";

const groupedPositions: RowElement["props"]["justify"][] = ["center", "end"];
