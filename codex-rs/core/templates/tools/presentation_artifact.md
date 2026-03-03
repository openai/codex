Create and edit PowerPoint presentation artifacts inside the current thread.

- This is a stateful built-in tool. `artifact_id` values are returned by earlier calls and persist only for the current thread.
- Resume and fork do not restore live artifact state. Export files if you need a durable handoff.
- Relative paths resolve from the current working directory.
- Position and size values are in slide points.

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
- `create_layout`
- `add_layout_placeholder`
- `set_slide_layout`
- `update_placeholder_text`
- `set_theme`
- `set_notes`
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
- `update_table_cell`
- `merge_table_cells`
- `add_chart`
- `update_text`
- `replace_text`
- `insert_text_after`
- `set_hyperlink`
- `update_shape_style`
- `bring_to_front`
- `send_to_back`
- `delete_element`
- `delete_artifact`

Example create:
`{"action":"create","args":{"name":"Quarterly Update"}}`

Example edit:
`{"artifact_id":"presentation_x","action":"add_text_shape","args":{"slide_index":0,"text":"Revenue up 24%","position":{"left":48,"top":72,"width":260,"height":80}}}`

Example export:
`{"artifact_id":"presentation_x","action":"export_pptx","args":{"path":"artifacts/q2-update.pptx"}}`

Example layout flow:
`{"artifact_id":"presentation_x","action":"create_layout","args":{"name":"Title Slide"}}`

`{"artifact_id":"presentation_x","action":"add_layout_placeholder","args":{"layout_id":"layout_1","name":"title","placeholder_type":"title","text":"Click to add title","position":{"left":48,"top":48,"width":624,"height":72}}}`

`{"artifact_id":"presentation_x","action":"set_slide_layout","args":{"slide_index":0,"layout_id":"layout_1"}}`

`{"artifact_id":"presentation_x","action":"list_layout_placeholders","args":{"layout_id":"layout_1"}}`

`{"artifact_id":"presentation_x","action":"list_slide_placeholders","args":{"slide_index":0}}`

Example inspect:
`{"artifact_id":"presentation_x","action":"inspect","args":{"kind":"deck,slide,textbox,shape,table,chart,image,notes,layoutList","max_chars":12000}}`

Example resolve:
`{"artifact_id":"presentation_x","action":"resolve","args":{"id":"sh/element_3"}}`

Deck summaries, slide listings, `inspect`, and `resolve` now include active-slide metadata. Use `set_active_slide` to change it explicitly.

Text-bearing elements also support literal `replace_text` and `insert_text_after` helpers for in-place edits without resending the full string.

Text boxes and shapes support whole-element hyperlinks via `set_hyperlink`. Supported `link_type` values are `url`, `slide`, `first_slide`, `last_slide`, `next_slide`, `previous_slide`, `end_show`, `email`, and `file`. Use `clear: true` to remove an existing hyperlink.

Notes visibility is honored on export: `set_notes_visibility` controls whether speaker notes are emitted into exported PPTX output.

Image placeholders can be prompt-only. `add_image` accepts `prompt` without `path`/`data_url`, and unresolved placeholders export as a visible placeholder box instead of failing.

Remote images are supported. `add_image` and `replace_image` accept `uri` in addition to local `path` and `data_url`.

Image edits can target inspect/resolve anchors like `im/element_3`, and `update_shape_style` now accepts image `fit`, `crop`, and `lock_aspect_ratio` updates.

`update_shape_style.position` accepts partial updates, so you can move or resize an element without resending the full rect.

Connectors are supported via `add_connector`, with straight/elbow/curved types plus dash styles and arrow heads.

Example preview:
`{"artifact_id":"presentation_x","action":"export_preview","args":{"slide_index":0,"path":"artifacts/q2-update-slide1.png"}}`

`export_preview` also accepts `format`, `scale`, and `quality` for rendered previews. `format` currently supports `png` and `jpeg`.

Example JPEG preview:
`{"artifact_id":"presentation_x","action":"export_preview","args":{"slide_index":0,"path":"artifacts/q2-update-slide1.jpg","format":"jpeg","scale":0.75,"quality":85}}`
