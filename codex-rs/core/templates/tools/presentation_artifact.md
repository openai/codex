Create and edit PowerPoint presentation artifacts inside the current thread.

- This is a stateful built-in tool. `artifact_id` values are returned by earlier calls and persist only for the current thread.
- Resume and fork do not restore live artifact state. Export files if you need a durable handoff.
- Relative paths resolve from the current working directory.
- Position and size values are in slide points.
- Every tool call uses a top-level `actions` array of sequential steps. Each call operates on a single top-level `artifact_id` when one is needed. If a call starts with `create` or `import_pptx`, later steps in the same call automatically reuse the returned artifact id.

Supported actions:
- `create`
- `import_pptx`
- `export_pptx`
- `export_preview`
- `get_summary`
- `list_slides`
- `list_layouts`
- `list_layout_placeholders`
- `list_slide_placeholders`
- `inspect`
- `resolve`
- `to_proto`
- `record_patch`
- `apply_patch`
- `undo`
- `redo`
- `create_layout`
- `add_layout_placeholder`
- `set_slide_layout`
- `update_placeholder_text`
- `set_theme`
- `add_style`
- `get_style`
- `describe_styles`
- `set_notes`
- `set_notes_rich_text`
- `append_notes`
- `clear_notes`
- `set_notes_visibility`
- `set_active_slide`
- `add_slide`
- `insert_slide`
- `duplicate_slide`
- `move_slide`
- `delete_slide`
- `set_slide_background`
- `add_text_shape`
- `add_shape`
- `add_connector`
- `add_image`
- `replace_image`
- `add_table`
- `update_table_style`
- `style_table_block`
- `update_table_cell`
- `merge_table_cells`
- `add_chart`
- `update_chart`
- `add_chart_series`
- `update_text`
- `set_rich_text`
- `format_text_range`
- `replace_text`
- `insert_text_after`
- `set_hyperlink`
- `set_comment_author`
- `add_comment_thread`
- `add_comment_reply`
- `toggle_comment_reaction`
- `resolve_comment_thread`
- `reopen_comment_thread`
- `update_shape_style`
- `bring_to_front`
- `send_to_back`
- `delete_element`
- `delete_artifact`

Example create:
`{"actions":[{"action":"create","args":{"name":"Quarterly Update"}}]}`

Example create with custom slide size:
`{"actions":[{"action":"create","args":{"name":"Quarterly Update","slide_size":{"width":960,"height":540}}}]}`

Example edit:
`{"artifact_id":"presentation_x","actions":[{"action":"add_text_shape","args":{"slide_index":0,"text":"Revenue up 24%","position":{"left":48,"top":72,"width":260,"height":80}}}]}`

Example sequential batch:
`{"actions":[{"action":"create","args":{"name":"Quarterly Update"}},{"action":"add_slide","args":{}},{"action":"add_text_shape","args":{"slide_index":0,"text":"Revenue up 24%","position":{"left":48,"top":72,"width":260,"height":80}}}]}`

Table creation also accepts optional `column_widths` and `row_heights` arrays in points when you need explicit table sizing instead of even splits. Tables also support `style_options`, `borders`, and `right_to_left`, with `update_table_style` and `style_table_block` available for incremental styling after creation.

Example export:
`{"artifact_id":"presentation_x","actions":[{"action":"export_pptx","args":{"path":"artifacts/q2-update.pptx"}}]}`

Example layout flow:
`{"artifact_id":"presentation_x","actions":[{"action":"create_layout","args":{"name":"Title Slide"}}]}`

`{"artifact_id":"presentation_x","actions":[{"action":"add_layout_placeholder","args":{"layout_id":"layout_1","name":"title","placeholder_type":"title","text":"Click to add title","position":{"left":48,"top":48,"width":624,"height":72}}}]}`

`{"artifact_id":"presentation_x","actions":[{"action":"set_slide_layout","args":{"slide_index":0,"layout_id":"layout_1"}}]}`

`{"artifact_id":"presentation_x","actions":[{"action":"list_layout_placeholders","args":{"layout_id":"layout_1"}}]}`

`{"artifact_id":"presentation_x","actions":[{"action":"list_slide_placeholders","args":{"slide_index":0}}]}`

Layout references in `create_layout.parent_layout_id`, `add_layout_placeholder.layout_id`, `add_slide`, `insert_slide`, `set_slide_layout`, and `list_layout_placeholders` accept either a layout id or a layout name. Name matching prefers exact id, then exact name, then case-insensitive name.

`insert_slide` accepts `index` or `after_slide_index`. If neither is provided, the new slide is inserted immediately after the active slide, or appended if no active slide is set yet.

Example inspect:
`{"artifact_id":"presentation_x","actions":[{"action":"inspect","args":{"include":"deck,slide,textbox,shape,table,chart,image,notes,layoutList,textRange,comment","exclude":"notes","search":"roadmap","max_chars":12000}}]}`

Example inspect target window:
`{"artifact_id":"presentation_x","actions":[{"action":"inspect","args":{"include":"textbox","target":{"id":"sh/element_3","before_lines":1,"after_lines":1}}}]}`

Example resolve:
`{"artifact_id":"presentation_x","actions":[{"action":"resolve","args":{"id":"sh/element_3"}}]}`

Example proto export:
`{"artifact_id":"presentation_x","actions":[{"action":"to_proto","args":{}}]}`

`to_proto` returns a full JSON snapshot of the current in-memory presentation document, including slide/layout records, anchors, notes, theme state, and typed element payloads.

Rich text is supported on notes, text boxes, shapes with text, and table cells. Use `set_rich_text` to replace a full rich-text payload, `set_notes_rich_text` for speaker notes, and `format_text_range` to annotate a substring by `query` or explicit codepoint range. `inspect`, `resolve`, and `to_proto` surface text-range anchors as `tr/<range_id>`.

Comment threads are supported through `set_comment_author`, `add_comment_thread`, `add_comment_reply`, `toggle_comment_reaction`, `resolve_comment_thread`, and `reopen_comment_thread`. Thread anchors resolve as `th/<thread_id>`, and comment records appear in both `inspect` and `to_proto`.

Charts support richer series metadata plus `update_chart` and `add_chart_series`, including legend, axis, data-label, marker, fill, and per-point override state.

Exported PPTX files embed Codex metadata so rich text, comment threads, and advanced table/chart state round-trip through `export_pptx` and `import_pptx` even when the base OOXML representation is lossy.

Example patch recording:
`{"artifact_id":"presentation_x","actions":[{"action":"record_patch","args":{"operations":[{"action":"add_text_shape","args":{"slide_index":0,"text":"Headline","position":{"left":48,"top":48,"width":320,"height":72}}},{"action":"set_slide_background","args":{"slide_index":0,"fill":"#F7F1E8"}}]}}]}`

Example patch application:
`{"artifact_id":"presentation_x","actions":[{"action":"apply_patch","args":{"patch":{"version":1,"artifactId":"presentation_x","operations":[{"action":"add_text_shape","args":{"slide_index":0,"text":"Headline","position":{"left":48,"top":48,"width":320,"height":72}}},{"action":"set_slide_background","args":{"slide_index":0,"fill":"#F7F1E8"}}]}}}]}`

Patch payloads are single-artifact and currently support existing in-memory editing actions like slide/element/layout/theme/text updates. Lifecycle, import/export, and nested history actions are intentionally excluded.

Example undo/redo:
`{"artifact_id":"presentation_x","actions":[{"action":"undo","args":{}}]}`

`{"artifact_id":"presentation_x","actions":[{"action":"redo","args":{}}]}`

Deck summaries, slide listings, `inspect`, and `resolve` now include active-slide metadata. Use `set_active_slide` to change it explicitly.

Theme snapshots and `to_proto` both expose the deck theme hex color map via `hex_color_map` / `hexColorMap`.

Named text styles are supported through `add_style`, `get_style`, and `describe_styles`. Built-in styles include `title`, `heading1`, `body`, `list`, and `numberedList`.

Example style creation:
`{"artifact_id":"presentation_x","actions":[{"action":"add_style","args":{"name":"callout","font_size":18,"color":"#336699","italic":true,"underline":true}}]}`

Example style lookup:
`{"artifact_id":"presentation_x","actions":[{"action":"get_style","args":{"name":"title"}}]}`

Text styling payloads on `add_text_shape`, `add_shape.text_style`, `update_text.styling`, and `update_table_cell.styling` accept `style` and `underline` in addition to the existing whole-element fields.

Text-bearing elements also support literal `replace_text` and `insert_text_after` helpers for in-place edits without resending the full string.

Example rich text update:
`{"artifact_id":"presentation_x","actions":[{"action":"set_rich_text","args":{"element_id":"element_3","text":[[{"run":"Quarterly ","text_style":{"bold":true}},"update pipeline"]],"text_layout":{"wrap":"square","auto_fit":"shrinkText","vertical_alignment":"middle","insets":{"left":6,"right":6,"top":4,"bottom":4}}}}]}`

Example substring formatting:
`{"artifact_id":"presentation_x","actions":[{"action":"format_text_range","args":{"element_id":"element_3","query":"update","styling":{"italic":true},"link":{"uri":"https://example.com/update","is_external":true}}}]}`

Text boxes and shapes support whole-element hyperlinks via `set_hyperlink`. Supported `link_type` values are `url`, `slide`, `first_slide`, `last_slide`, `next_slide`, `previous_slide`, `end_show`, `email`, and `file`. Use `clear: true` to remove an existing hyperlink.

Notes visibility is honored on export: `set_notes_visibility` controls whether speaker notes are emitted into exported PPTX output.

Example comment thread:
`{"artifact_id":"presentation_x","actions":[{"action":"set_comment_author","args":{"display_name":"Jamie Fox","initials":"JF"}},{"action":"add_comment_thread","args":{"slide_index":0,"element_id":"element_3","query":"Quarterly","text":"Tighten this headline"}},{"action":"add_comment_reply","args":{"thread_id":"thread_1","text":"Applied to the draft."}}]}`

Image placeholders can be prompt-only. `add_image` accepts `prompt` without `path`/`data_url`, and unresolved placeholders export as a visible placeholder box instead of failing.

Layout placeholders with `placeholder_type: "picture"` or `"image"` materialize as placeholder image elements on slides, so they appear in `list_slide_placeholders`, `inspect`, and `resolve` as `image` records rather than generic shapes.

Remote images are supported. `add_image` and `replace_image` accept `uri` in addition to local `path`, raw base64 `blob`, and `data_url`.

Image edits can target inspect/resolve anchors like `im/element_3`, and `update_shape_style` now accepts image `fit`, `crop`, `rotation`, `flip_horizontal`, `flip_vertical`, and `lock_aspect_ratio` updates.

`add_image` and `replace_image` also accept optional `rotation`, `flip_horizontal`, and `flip_vertical` fields for image transforms.

`update_shape_style.position` accepts partial updates, so you can move or resize an element without resending the full rect.

Shape strokes accept an optional `style` field such as `solid`, `dashed`, `dotted`, `dash-dot`, `dash-dot-dot`, `long-dash`, or `long-dash-dot`. This applies to ordinary shapes via `add_shape.stroke` and `update_shape_style.stroke`.

`add_shape` also accepts optional `rotation`, `flip_horizontal`, and `flip_vertical` fields. Those same transform fields can also be provided inside the `position` object for `add_shape`, `add_image`, and `update_shape_style`.

Connectors are supported via `add_connector`, with straight/elbow/curved types plus dash styles and arrow heads.

Example chart update:
`{"artifact_id":"presentation_x","actions":[{"action":"update_chart","args":{"element_id":"element_7","style_index":12,"legend_position":"bottom","y_axis_title":"USD (millions)"}},{"action":"add_chart_series","args":{"element_id":"element_7","name":"Target","values":[11,13],"fill":"#70AD47","marker":{"symbol":"diamond","size":6}}}]}`

Example preview:
`{"artifact_id":"presentation_x","actions":[{"action":"export_preview","args":{"slide_index":0,"path":"artifacts/q2-update-slide1.png"}}]}`

`export_preview` also accepts `format`, `scale`, and `quality` for rendered previews. `format` currently supports `png`, `jpeg`, and `svg`.

Example JPEG preview:
`{"artifact_id":"presentation_x","actions":[{"action":"export_preview","args":{"slide_index":0,"path":"artifacts/q2-update-slide1.jpg","format":"jpeg","scale":0.75,"quality":85}}]}`
