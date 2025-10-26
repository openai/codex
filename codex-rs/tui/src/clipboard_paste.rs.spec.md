## Overview
`clipboard_paste` captures pasted images and normalizes pasted paths for the TUI. It bridges platform clipboard access, temporary file handling, and path parsing helpers used by the composer.

## Detailed Behavior
- `paste_image_as_png` (non-Android):
  - Uses `arboard` to fetch clipboard contents, preferring file lists but falling back to raw image data.
  - Converts the image to PNG bytes using `image` crate and returns the byte buffer plus `PastedImageInfo` (dimensions, format label).
  - Emits detailed `PasteImageError` variants for clipboard, encoding, or IO failures.
- `paste_image_to_temp_png` writes the PNG bytes to a unique temp file (persisted after write) and returns the path with image metadata.
- Android builds return `PasteImageError::ClipboardUnavailable`.
- `normalize_pasted_path` trims and normalizes text representing file paths:
  - Supports `file://` URLs, Windows drive/UNC paths, and shell-escaped single paths (via `shlex`).
- `pasted_image_format` infers an `EncodedImageFormat` based on file extension.

## Broader Context
- `ChatComposer` uses these helpers to attach images from the clipboard and to interpret pasted file paths when building prompts.

## Technical Debt
- Detection heuristics for Windows paths and shell escaping are intentionally simple; future refinements may use more robust typed-path parsing if needed.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Consider leveraging a cross-platform path parsing library to improve URL/UNC handling and minimize edge cases.
related_specs:
  - ./bottom_pane/chat_composer.rs.spec.md
