# codex-mermaid

Standalone, bounded Mermaid text renderer.
It uses the existing `unicode-width` dependency; no Mermaid runtime or new external
production dependency is needed.

## Supported subsets

| Family | Supported syntax |
| --- | --- |
| `flowchart`, `graph` | TD/TB, BT, LR, RL; rectangle `[label]`, decision `{label}`, and stadium `([label])` nodes; directed and labeled `-->` edges; chains, branches, merges, loops |
| `sequenceDiagram` | Implicit participants, `participant`/`actor`, aliases, `->`, `->>`, `-->`, `-->>`, `-x`, `--x`, self-messages, `Note over A[,B]`, nested `loop`/`alt`/`opt`/`critical`/`break`, one labeled `else` per `alt` |
| `stateDiagram-v2`, `stateDiagram` | Flat states, `state "label" as ID`, descriptions, directed transitions with optional labels, initial/final `[*]`, direction declarations |
| `classDiagram` | `class ID`, multiline member bodies, `ID : member`, solid/dashed links, association, inheritance, composition, aggregation, dependency, realization, quoted endpoint cardinalities, relationship labels, direction declarations |
| `erDiagram` | Entities, multiline attribute bodies, `type name [PK, FK, UK] ["comment"]`, all four endpoint cardinalities, identifying/non-identifying relationships, relationship labels, direction declarations |

Identifiers are ASCII letters followed by letters, digits, or underscores. Text
supports ordinary Unicode and CJK. Full-line `%%` comments and semicolon-separated
statements are supported; semicolons inside labels are not. Member/attribute bodies
need a separate statement for each opening brace, member, and closing brace (for
example `class Order {` followed by member lines and a final `}`). Class member text
is retained in one compartment, including visibility, signatures, and return types.
ER attribute types and names use the identifier grammar above.

Flowchart node and edge labels may be enclosed in double quotes. Literal ampersands
are supported in these flowchart labels; other diagram families retain their stricter
label subset. HTML and entity escapes remain unsupported.

These are explicit subsets, not complete Mermaid compatibility. Compound states,
flowchart subgraphs, other shapes, sequence activation and parallel fragments,
styling, front matter, directives, HTML, escapes, combining/zero-width characters,
and ligatures with non-additive widths return errors. Callers should retain source
on any error; the library never returns a partial diagram.

## Layout and notation

Graph nodes appear in declaration/first-reference order, in the requested direction.
Each edge gets its own lane and endpoint positions. Crossings use `╪` and never
join routes. Decisions use `◇` inside a box; stadiums use rounded box corners. Horizontal layouts
reserve a text gutter for every endpoint, which can make connected graphs wide;
the caller receives `TooWide` if the complete output does not fit.

Class links use arrows into the referenced class, `◁`/`△` for inheritance/realization,
`◆` for composition, and `◇` for aggregation. Endpoint cardinalities appear in
parentheses next to the appropriate class or entity. ER cardinalities are written
as `1`, `0..1`, `1..many`, or `0..many`. Solid ER links are identifying; dashed links
are non-identifying. Class members and ER keys/comments appear verbatim as readable
text, rather than renderer metadata.

Sequence messages retain chronological order. `->>`/`-->>` use `▶`/`◀` arrowheads,
`->`/`-->` have no arrowhead, and `-x`/`--x` use a cross endpoint. Solid and dashed
strokes remain distinct. Lifelines crossed by a message use `┼`; this is not a recipient.
Control fragments have nested frames and explicit branch labels. Notes span their
named participants. States use separate `● initial` and `◎ final` nodes.

## Limits and evaluation

All inputs are limited to 16 KiB. Graphs allow 16 nodes, 24 edges, and 16 members per
node. Sequences allow 8 participants, 64 events (including fragment boundaries),
and 4 fragment levels. Source labels and identifiers are limited to 40 display
cells/ASCII bytes respectively. Rendered canvases are capped at 65,536 cells,
independent of the caller's maximum width. The library performs no I/O.

From `codex-rs`, preview a file, optionally specifying the available width:

```sh
cargo run -p codex-mermaid --example render -- 180 < diagram.mmd
```

Run `just test -p codex-mermaid --lib`. Coverage includes complex snapshots for every
family, relationship endpoints, all ER cardinalities, width/error bounds,
truncated input, and reconstruction of every edge in all 512 directed three-node
graphs in each of the four layout directions.


`render_spans` returns the same layout as lines of semantic `Node`, `Edge`, and
`Text` spans. Callers apply their own theme; the crate never emits ANSI escapes.
