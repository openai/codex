//! Project-level documentation discovery.
//!
//! Project-level documentation is primarily stored in files named `AGENTS.md`.
//! Additional fallback filenames can be configured via `project_doc_fallback_filenames`.
//! We include the concatenation of all files found along the path from the
//! project root to the current working directory as follows:
//!
//! 1.  Determine the project root by walking upwards from the current working
//!     directory until a configured `project_root_markers` entry is found.
//!     When `project_root_markers` is unset, the default marker list is used
//!     (`.git`). If no marker is found, only the current working directory is
//!     considered. An empty marker list disables parent traversal.
//! 2.  Collect every `AGENTS.md` found from the project root down to the
//!     current working directory (inclusive) and concatenate their contents in
//!     that order.
//! 3.  We do **not** walk past the project root.

use crate::config::Config;
use crate::config_loader::ConfigLayerStackOrdering;
use crate::config_loader::default_project_root_markers;
use crate::config_loader::merge_toml_values;
use crate::config_loader::project_root_markers_from_config;
use codex_app_server_protocol::ConfigLayerSource;
use codex_exec_server::Environment;
use codex_exec_server::ExecutorFileSystem;
use codex_features::Feature;
use codex_utils_absolute_path::AbsolutePathBuf;
use dunce::canonicalize as normalize_path;
use std::io;
use toml::Value as TomlValue;
use tracing::error;

pub(crate) const HIERARCHICAL_AGENTS_MESSAGE: &str =
    include_str!("../hierarchical_agents_message.md");

/// Default filename scanned for project-level docs.
pub const DEFAULT_PROJECT_DOC_FILENAME: &str = "AGENTS.md";
/// Preferred local override for project-level docs.
pub const LOCAL_PROJECT_DOC_FILENAME: &str = "AGENTS.override.md";

/// When both `Config::instructions` and the project doc are present, they will
/// be concatenated with the following separator.
const PROJECT_DOC_SEPARATOR: &str = "\n\n--- project-doc ---\n\n";

fn render_js_repl_instructions(config: &Config) -> Option<String> {
    if !config.features.enabled(Feature::JsRepl) {
        return None;
    }

    let mut section = String::from("## JavaScript REPL (Node)\n");
    section.push_str(
        "- Use `js_repl` for Node-backed JavaScript with top-level await in a persistent kernel.\n",
    );
    section.push_str("- `js_repl` is a freeform/custom tool. Direct `js_repl` calls must send raw JavaScript tool input (optionally with first-line `// codex-js-repl: timeout_ms=15000`). Do not wrap code in JSON (for example `{\"code\":\"...\"}`), quotes, or markdown code fences.\n");
    section.push_str(
        "- Helpers: `codex.cwd`, `codex.homeDir`, `codex.tmpDir`, `codex.tool(name, args?)`, and `codex.emitImage(imageLike)`.\n",
    );
    section.push_str("- `codex.tool` executes a normal tool call and resolves to the raw tool output object. Use it for shell and non-shell tools alike. Nested tool outputs stay inside JavaScript unless you emit them explicitly.\n");
    section.push_str("- `codex.emitImage(...)` adds one image to the outer `js_repl` function output each time you call it, so you can call it multiple times to emit multiple images. It accepts a data URL, a single `input_image` item, an object like `{ bytes, mimeType }`, or a raw tool response object with exactly one image and no text. It rejects mixed text-and-image content.\n");
    section.push_str("- `codex.tool(...)` and `codex.emitImage(...)` keep stable helper identities across cells. Saved references and persisted objects can reuse them in later cells, but async callbacks that fire after a cell finishes still fail because no exec is active.\n");
    section.push_str("- Request full-resolution image processing with `detail: \"original\"` only when the `view_image` tool schema includes a `detail` argument. The same availability applies to `codex.emitImage(...)`: if `view_image.detail` is present, you may also pass `detail: \"original\"` there. Use this when high-fidelity image perception or precise localization is needed, especially for CUA agents.\n");
    section.push_str("- Example of sharing an in-memory Playwright screenshot: `await codex.emitImage({ bytes: await page.screenshot({ type: \"jpeg\", quality: 85 }), mimeType: \"image/jpeg\", detail: \"original\" })`.\n");
    section.push_str("- Example of sharing a local image tool result: `await codex.emitImage(codex.tool(\"view_image\", { path: \"/absolute/path\", detail: \"original\" }))`.\n");
    section.push_str("- When encoding an image to send with `codex.emitImage(...)` or `view_image`, prefer JPEG at about 85 quality when lossy compression is acceptable; use PNG when transparency or lossless detail matters. Smaller uploads are faster and less likely to hit size limits.\n");
    section.push_str("- Top-level bindings persist across cells. If a cell throws, prior bindings remain available and bindings that finished initializing before the throw often remain usable in later cells. For code you plan to reuse across cells, prefer declaring or assigning it in direct top-level statements before operations that might throw. If you hit `SyntaxError: Identifier 'x' has already been declared`, first reuse the existing binding, reassign a previously declared `let`, or pick a new descriptive name. Use `{ ... }` only for a short temporary block when you specifically need local scratch names; do not wrap an entire cell in block scope if you want those names reusable later. Reset the kernel with `js_repl_reset` only when you need a clean state.\n");
    section.push_str("- Top-level static import declarations (for example `import x from \"./file.js\"`) are currently unsupported in `js_repl`; use dynamic imports with `await import(\"pkg\")`, `await import(\"./file.js\")`, or `await import(\"/abs/path/file.mjs\")` instead. Imported local files must be ESM `.js`/`.mjs` files and run in the same REPL VM context. Bare package imports always resolve from REPL-global search roots (`CODEX_JS_REPL_NODE_MODULE_DIRS`, then cwd), not relative to the imported file location. Local files may statically import only other local relative/absolute/`file://` `.js`/`.mjs` files; package and builtin imports from local files must stay dynamic. `import.meta.resolve()` returns importable strings such as `file://...`, bare package names, and `node:...` specifiers. Local file modules reload between execs, while top-level bindings persist until `js_repl_reset`.\n");

    if config.features.enabled(Feature::JsReplToolsOnly) {
        section.push_str("- Do not call tools directly; use `js_repl` + `codex.tool(...)` for all tool calls, including shell commands.\n");
        section
            .push_str("- MCP tools (if any) can also be called by name via `codex.tool(...)`.\n");
    }

    section.push_str("- Avoid direct access to `process.stdout` / `process.stderr` / `process.stdin`; it can corrupt the JSON line protocol. Use `console.log`, `codex.tool(...)`, and `codex.emitImage(...)`.");

    Some(section)
}

/// Combines `Config::instructions` and `AGENTS.md` (if present) into a single
/// string of instructions.
pub(crate) async fn get_user_instructions(
    config: &Config,
    environment: Option<&Environment>,
) -> Option<String> {
    let fs = environment?.get_filesystem();
    get_user_instructions_with_fs(config, fs.as_ref()).await
}

pub(crate) async fn get_user_instructions_with_fs(
    config: &Config,
    fs: &dyn ExecutorFileSystem,
) -> Option<String> {
    let project_docs = read_project_docs_with_fs(config, fs).await;

    let mut output = String::new();

    if let Some(instructions) = config.user_instructions.clone() {
        output.push_str(&instructions);
    }

    match project_docs {
        Ok(Some(docs)) => {
            if !output.is_empty() {
                output.push_str(PROJECT_DOC_SEPARATOR);
            }
            output.push_str(&docs);
        }
        Ok(None) => {}
        Err(e) => {
            error!("error trying to find project doc: {e:#}");
        }
    };

    if let Some(js_repl_section) = render_js_repl_instructions(config) {
        if !output.is_empty() {
            output.push_str("\n\n");
        }
        output.push_str(&js_repl_section);
    }

    if config.features.enabled(Feature::ChildAgentsMd) {
        if !output.is_empty() {
            output.push_str("\n\n");
        }
        output.push_str(HIERARCHICAL_AGENTS_MESSAGE);
    }

    if !output.is_empty() {
        Some(output)
    } else {
        None
    }
}

/// Attempt to locate and load the project documentation.
///
/// On success returns `Ok(Some(contents))` where `contents` is the
/// concatenation of all discovered docs. If no documentation file is found the
/// function returns `Ok(None)`. Unexpected I/O failures bubble up as `Err` so
/// callers can decide how to handle them.
pub async fn read_project_docs(
    config: &Config,
    environment: &Environment,
) -> io::Result<Option<String>> {
    let fs = environment.get_filesystem();
    read_project_docs_with_fs(config, fs.as_ref()).await
}

async fn read_project_docs_with_fs(
    config: &Config,
    fs: &dyn ExecutorFileSystem,
) -> io::Result<Option<String>> {
    let max_total = config.project_doc_max_bytes;

    if max_total == 0 {
        return Ok(None);
    }

    let paths = discover_project_doc_paths(config, fs).await?;
    if paths.is_empty() {
        return Ok(None);
    }

    let mut remaining: u64 = max_total as u64;
    let mut parts: Vec<String> = Vec::new();

    for p in paths {
        if remaining == 0 {
            break;
        }

        match fs.get_metadata(&p).await {
            Ok(metadata) if !metadata.is_file => continue,
            Ok(_) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        }

        let mut data = match fs.read_file(&p).await {
            Ok(data) => data,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        };
        let size = data.len() as u64;
        if size > remaining {
            data.truncate(remaining as usize);
        }

        if size > remaining {
            tracing::warn!(
                "Project doc `{}` exceeds remaining budget ({} bytes) - truncating.",
                p.display(),
                remaining,
            );
        }

        let text = String::from_utf8_lossy(&data).to_string();
        if !text.trim().is_empty() {
            parts.push(text);
            remaining = remaining.saturating_sub(data.len() as u64);
        }
    }

    if parts.is_empty() {
        Ok(None)
    } else {
        Ok(Some(parts.join("\n\n")))
    }
}

/// Discover the list of AGENTS.md files using the same search rules as
/// `read_project_docs`, but return the file paths instead of concatenated
/// contents. The list is ordered from project root to the current working
/// directory (inclusive). Symlinks are allowed. When `project_doc_max_bytes`
/// is zero, returns an empty list.
pub async fn discover_project_doc_paths(
    config: &Config,
    fs: &dyn ExecutorFileSystem,
) -> io::Result<Vec<AbsolutePathBuf>> {
    if config.project_doc_max_bytes == 0 {
        return Ok(Vec::new());
    }

    let mut dir = config.cwd.clone();
    if let Ok(canon) = normalize_path(&dir) {
        dir = AbsolutePathBuf::try_from(canon)?;
    }

    let mut merged = TomlValue::Table(toml::map::Map::new());
    for layer in config.config_layer_stack.get_layers(
        ConfigLayerStackOrdering::LowestPrecedenceFirst,
        /*include_disabled*/ false,
    ) {
        if matches!(layer.name, ConfigLayerSource::Project { .. }) {
            continue;
        }
        merge_toml_values(&mut merged, &layer.config);
    }
    let project_root_markers = match project_root_markers_from_config(&merged) {
        Ok(Some(markers)) => markers,
        Ok(None) => default_project_root_markers(),
        Err(err) => {
            tracing::warn!("invalid project_root_markers: {err}");
            default_project_root_markers()
        }
    };
    let mut project_root = None;
    if !project_root_markers.is_empty() {
        for ancestor in dir.ancestors() {
            for marker in &project_root_markers {
                let marker_path = ancestor.join(marker);
                let marker_exists = match fs.get_metadata(&marker_path).await {
                    Ok(_) => true,
                    Err(err) if err.kind() == io::ErrorKind::NotFound => false,
                    Err(err) => return Err(err),
                };
                if marker_exists {
                    project_root = Some(ancestor.clone());
                    break;
                }
            }
            if project_root.is_some() {
                break;
            }
        }
    }

    let search_dirs: Vec<AbsolutePathBuf> = if let Some(root) = project_root {
        let mut dirs = Vec::new();
        let mut cursor = dir.clone();
        loop {
            dirs.push(cursor.clone());
            if cursor == root {
                break;
            }
            let Some(parent) = cursor.parent() else {
                break;
            };
            cursor = parent;
        }
        dirs.reverse();
        dirs
    } else {
        vec![dir]
    };

    let mut found: Vec<AbsolutePathBuf> = Vec::new();
    let candidate_filenames = candidate_filenames(config);
    for d in search_dirs {
        for name in &candidate_filenames {
            let candidate = d.join(name);
            match fs.get_metadata(&candidate).await {
                Ok(md) if md.is_file => {
                    found.push(candidate);
                    break;
                }
                Ok(_) => {}
                Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                Err(err) => return Err(err),
            }
        }
    }

    Ok(found)
}
fn candidate_filenames<'a>(config: &'a Config) -> Vec<&'a str> {
    let mut names: Vec<&'a str> =
        Vec::with_capacity(2 + config.project_doc_fallback_filenames.len());
    names.push(LOCAL_PROJECT_DOC_FILENAME);
    names.push(DEFAULT_PROJECT_DOC_FILENAME);
    for candidate in &config.project_doc_fallback_filenames {
        let candidate = candidate.as_str();
        if candidate.is_empty() {
            continue;
        }
        if !names.contains(&candidate) {
            names.push(candidate);
        }
    }
    names
}

#[cfg(test)]
#[path = "project_doc_tests.rs"]
mod tests;
