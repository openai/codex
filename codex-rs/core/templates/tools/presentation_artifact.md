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
- `add_slide`
- `insert_slide`
- `duplicate_slide`
- `move_slide`
- `delete_slide`
- `set_slide_background`
- `add_text_shape`
- `add_shape`
- `add_image`
- `replace_image`
- `add_table`
- `update_table_cell`
- `merge_table_cells`
- `add_chart`
- `update_text`
- `update_shape_style`
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

Notes visibility is honored on export: `set_notes_visibility` controls whether speaker notes are emitted into exported PPTX output.

Example preview:
`{"artifact_id":"presentation_x","action":"export_preview","args":{"slide_index":0,"path":"artifacts/q2-update-slide1.png"}}`
