## Overview
`core::tools::handlers::list_dir` enumerates directory contents with depth limits and pagination. It gives the model a structured view of nearby files while enforcing absolute-path requirements and sane output sizes.

## Detailed Behavior
- Accepts `ToolPayload::Function` with JSON arguments parsed into `ListDirArgs`, validating that `offset`, `limit`, and `depth` are positive integers and that `dir_path` is absolute.
- `list_dir_slice` builds a breadth-first traversal queue (using `VecDeque`) to collect entries up to the requested depth, recording each entry’s relative path, display name, type, and indentation depth. It reads directories asynchronously with `tokio::fs::read_dir`.
- Results are sorted lexicographically by display name, truncated to the requested limit (default 25), and formatted with indentation (two spaces per depth level) plus type annotations (directory/file/link). If more entries exist beyond the limit, a trailing marker is appended.
- The handler prepends the absolute path to the target directory, joins the formatted entries with newlines, and returns them as `ToolOutput::Function { success: Some(true) }`.
- Errors (invalid JSON, non-absolute paths, failed metadata) produce `FunctionCallError::RespondToModel` messages so the model can adjust the request.

## Broader Context
- Like `read_file`, this tool is currently exposed only when the experimental feature list includes `list_dir`. It pairs with `grep_files` and `read_file` to support navigation within large repositories.
- Indentation formatting is purely textual; UI clients may render the output directly or post-process it into tree views.
- Context can't yet be determined for symlink traversal policies; current behavior treats symlinks as distinct entry types but does not follow them.

## Technical Debt
- None recorded; future enhancements (e.g., filtering by glob) would expand argument parsing.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.rs.spec.md
  - ../spec.rs.spec.md
  - ../../mod.spec.md
