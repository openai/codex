GIF guidelines:
- Use imagegen only for simple GIFs with 2–16 frames. For more complex animations or more than 16 frames, use other tools instead.
- Set `gif` to request GIF output. Use `{}` for the defaults (9 frames, 200 ms per frame), or set `frame_count` and `frame_duration_ms`.
- To edit a previously generated GIF, first inspect its frames, then create a spritesheet with equal-sized cells in a regular grid. Pass that spritesheet through `referenced_image_paths` and set `gif` to generate the new GIF. Do not pass the GIF itself as an edit input.
