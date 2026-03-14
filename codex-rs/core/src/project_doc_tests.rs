use super::*;
use crate::config::ConfigBuilder;
use crate::features::Feature;
use crate::skills::loader::SkillRoot;
use crate::skills::loader::load_skills_from_roots;
use codex_protocol::protocol::SkillScope;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper that returns a `Config` pointing at `root` and using `limit` as
/// the maximum number of bytes to embed from AGENTS.md. The caller can
/// optionally specify a custom `instructions` string – when `None` the
/// value is cleared to mimic a scenario where no system instructions have
/// been configured.
async fn make_config(root: &TempDir, limit: usize, instructions: Option<&str>) -> Config {
    let codex_home = TempDir::new().unwrap();
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await
        .expect("defaults for test should always succeed");

    config.cwd = root.path().to_path_buf();
    config.project_doc_max_bytes = limit;

    config.user_instructions = instructions.map(ToOwned::to_owned);
    config
}

async fn make_config_with_fallback(
    root: &TempDir,
    limit: usize,
    instructions: Option<&str>,
    fallbacks: &[&str],
) -> Config {
    let mut config = make_config(root, limit, instructions).await;
    config.project_doc_fallback_filenames = fallbacks
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    config
}

async fn make_config_with_project_root_markers(
    root: &TempDir,
    limit: usize,
    instructions: Option<&str>,
    markers: &[&str],
) -> Config {
    let codex_home = TempDir::new().unwrap();
    let cli_overrides = vec![(
        "project_root_markers".to_string(),
        TomlValue::Array(
            markers
                .iter()
                .map(|marker| TomlValue::String((*marker).to_string()))
                .collect(),
        ),
    )];
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .cli_overrides(cli_overrides)
        .build()
        .await
        .expect("defaults for test should always succeed");

    config.cwd = root.path().to_path_buf();
    config.project_doc_max_bytes = limit;
    config.user_instructions = instructions.map(ToOwned::to_owned);
    config
}

fn load_test_skills(config: &Config) -> crate::skills::SkillLoadOutcome {
    load_skills_from_roots([SkillRoot {
        path: config.codex_home.join("skills"),
        scope: SkillScope::User,
    }])
}

/// AGENTS.md missing – should yield `None`.
#[tokio::test]
async fn no_doc_file_returns_none() {
    let tmp = tempfile::tempdir().expect("tempdir");

    let res = build_project_doc_instructions_text(&make_config(&tmp, 4096, None).await).await;
    assert!(
        res.is_none(),
        "Expected None when AGENTS.md is absent and no system instructions provided"
    );
    assert!(res.is_none(), "Expected None when AGENTS.md is absent");
}

/// Small file within the byte-limit is returned unmodified.
#[tokio::test]
async fn doc_smaller_than_limit_is_returned() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("AGENTS.md"), "hello world").unwrap();

    let res = build_project_doc_instructions_text(&make_config(&tmp, 4096, None).await)
        .await
        .expect("doc expected");

    assert_eq!(
        res, "hello world",
        "The document should be returned verbatim when it is smaller than the limit and there are no existing instructions"
    );
}

/// Oversize file is truncated to `project_doc_max_bytes`.
#[tokio::test]
async fn doc_larger_than_limit_is_truncated() {
    const LIMIT: usize = 1024;
    let tmp = tempfile::tempdir().expect("tempdir");

    let huge = "A".repeat(LIMIT * 2); // 2 KiB
    fs::write(tmp.path().join("AGENTS.md"), &huge).unwrap();

    let res = build_project_doc_instructions_text(&make_config(&tmp, LIMIT, None).await)
        .await
        .expect("doc expected");

    assert_eq!(res.len(), LIMIT, "doc should be truncated to LIMIT bytes");
    assert_eq!(res, huge[..LIMIT]);
}

/// When `cwd` is nested inside a repo, the search should locate AGENTS.md
/// placed at the repository root (identified by `.git`).
#[tokio::test]
async fn finds_doc_in_repo_root() {
    let repo = tempfile::tempdir().expect("tempdir");

    // Simulate a git repository. Note .git can be a file or a directory.
    std::fs::write(
        repo.path().join(".git"),
        "gitdir: /path/to/actual/git/dir\n",
    )
    .unwrap();

    // Put the doc at the repo root.
    fs::write(repo.path().join("AGENTS.md"), "root level doc").unwrap();

    // Now create a nested working directory: repo/workspace/crate_a
    let nested = repo.path().join("workspace/crate_a");
    std::fs::create_dir_all(&nested).unwrap();

    // Build config pointing at the nested dir.
    let mut cfg = make_config(&repo, 4096, None).await;
    cfg.cwd = nested;

    let res = build_project_doc_instructions_text(&cfg)
        .await
        .expect("doc expected");
    assert_eq!(res, "root level doc");
}

/// Explicitly setting the byte-limit to zero disables project docs.
#[tokio::test]
async fn zero_byte_limit_disables_docs() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("AGENTS.md"), "something").unwrap();

    let res = build_project_doc_instructions_text(&make_config(&tmp, 0, None).await).await;
    assert!(
        res.is_none(),
        "With limit 0 the function should return None"
    );
}

#[tokio::test]
async fn js_repl_instructions_are_appended_when_enabled() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut cfg = make_config(&tmp, 4096, None).await;
    cfg.features
        .enable(Feature::JsRepl)
        .expect("test config should allow js_repl");

    let res = render_js_repl_instructions(&cfg).expect("js_repl instructions expected");
    let expected = "## JavaScript REPL (Node)\n- Use `js_repl` for Node-backed JavaScript with top-level await in a persistent kernel.\n- `js_repl` is a freeform/custom tool. Direct `js_repl` calls must send raw JavaScript tool input (optionally with first-line `// codex-js-repl: timeout_ms=15000`). Do not wrap code in JSON (for example `{\"code\":\"...\"}`), quotes, or markdown code fences.\n- Helpers: `codex.cwd`, `codex.homeDir`, `codex.tmpDir`, `codex.tool(name, args?)`, and `codex.emitImage(imageLike)`.\n- `codex.tool` executes a normal tool call and resolves to the raw tool output object. Use it for shell and non-shell tools alike. Nested tool outputs stay inside JavaScript unless you emit them explicitly.\n- `codex.emitImage(...)` adds one image to the outer `js_repl` function output each time you call it, so you can call it multiple times to emit multiple images. It accepts a data URL, a single `input_image` item, an object like `{ bytes, mimeType }`, or a raw tool response object with exactly one image and no text. It rejects mixed text-and-image content.\n- `codex.tool(...)` and `codex.emitImage(...)` keep stable helper identities across cells. Saved references and persisted objects can reuse them in later cells, but async callbacks that fire after a cell finishes still fail because no exec is active.\n- Request full-resolution image processing with `detail: \"original\"` only when the `view_image` tool schema includes a `detail` argument. The same availability applies to `codex.emitImage(...)`: if `view_image.detail` is present, you may also pass `detail: \"original\"` there. Use this when high-fidelity image perception or precise localization is needed, especially for CUA agents.\n- Example of sharing an in-memory Playwright screenshot: `await codex.emitImage({ bytes: await page.screenshot({ type: \"jpeg\", quality: 85 }), mimeType: \"image/jpeg\", detail: \"original\" })`.\n- Example of sharing a local image tool result: `await codex.emitImage(codex.tool(\"view_image\", { path: \"/absolute/path\", detail: \"original\" }))`.\n- When encoding an image to send with `codex.emitImage(...)` or `view_image`, prefer JPEG at about 85 quality when lossy compression is acceptable; use PNG when transparency or lossless detail matters. Smaller uploads are faster and less likely to hit size limits.\n- Top-level bindings persist across cells. If a cell throws, prior bindings remain available and bindings that finished initializing before the throw often remain usable in later cells. For code you plan to reuse across cells, prefer declaring or assigning it in direct top-level statements before operations that might throw. If you hit `SyntaxError: Identifier 'x' has already been declared`, first reuse the existing binding, reassign a previously declared `let`, or pick a new descriptive name. Use `{ ... }` only for a short temporary block when you specifically need local scratch names; do not wrap an entire cell in block scope if you want those names reusable later. Reset the kernel with `js_repl_reset` only when you need a clean state.\n- Top-level static import declarations (for example `import x from \"./file.js\"`) are currently unsupported in `js_repl`; use dynamic imports with `await import(\"pkg\")`, `await import(\"./file.js\")`, or `await import(\"/abs/path/file.mjs\")` instead. Imported local files must be ESM `.js`/`.mjs` files and run in the same REPL VM context. Bare package imports always resolve from REPL-global search roots (`CODEX_JS_REPL_NODE_MODULE_DIRS`, then cwd), not relative to the imported file location. Local files may statically import only other local relative/absolute/`file://` `.js`/`.mjs` files; package and builtin imports from local files must stay dynamic. `import.meta.resolve()` returns importable strings such as `file://...`, bare package names, and `node:...` specifiers. Local file modules reload between execs, while top-level bindings persist until `js_repl_reset`.\n- Avoid direct access to `process.stdout` / `process.stderr` / `process.stdin`; it can corrupt the JSON line protocol. Use `console.log`, `codex.tool(...)`, and `codex.emitImage(...)`.";
    assert_eq!(res, expected);
}

#[tokio::test]
async fn js_repl_tools_only_instructions_are_feature_gated() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut cfg = make_config(&tmp, 4096, None).await;
    let mut features = cfg.features.get().clone();
    features
        .enable(Feature::JsRepl)
        .enable(Feature::JsReplToolsOnly);
    cfg.features
        .set(features)
        .expect("test config should allow js_repl tool restrictions");

    let res = render_js_repl_instructions(&cfg).expect("js_repl instructions expected");
    let expected = "## JavaScript REPL (Node)\n- Use `js_repl` for Node-backed JavaScript with top-level await in a persistent kernel.\n- `js_repl` is a freeform/custom tool. Direct `js_repl` calls must send raw JavaScript tool input (optionally with first-line `// codex-js-repl: timeout_ms=15000`). Do not wrap code in JSON (for example `{\"code\":\"...\"}`), quotes, or markdown code fences.\n- Helpers: `codex.cwd`, `codex.homeDir`, `codex.tmpDir`, `codex.tool(name, args?)`, and `codex.emitImage(imageLike)`.\n- `codex.tool` executes a normal tool call and resolves to the raw tool output object. Use it for shell and non-shell tools alike. Nested tool outputs stay inside JavaScript unless you emit them explicitly.\n- `codex.emitImage(...)` adds one image to the outer `js_repl` function output each time you call it, so you can call it multiple times to emit multiple images. It accepts a data URL, a single `input_image` item, an object like `{ bytes, mimeType }`, or a raw tool response object with exactly one image and no text. It rejects mixed text-and-image content.\n- `codex.tool(...)` and `codex.emitImage(...)` keep stable helper identities across cells. Saved references and persisted objects can reuse them in later cells, but async callbacks that fire after a cell finishes still fail because no exec is active.\n- Request full-resolution image processing with `detail: \"original\"` only when the `view_image` tool schema includes a `detail` argument. The same availability applies to `codex.emitImage(...)`: if `view_image.detail` is present, you may also pass `detail: \"original\"` there. Use this when high-fidelity image perception or precise localization is needed, especially for CUA agents.\n- Example of sharing an in-memory Playwright screenshot: `await codex.emitImage({ bytes: await page.screenshot({ type: \"jpeg\", quality: 85 }), mimeType: \"image/jpeg\", detail: \"original\" })`.\n- Example of sharing a local image tool result: `await codex.emitImage(codex.tool(\"view_image\", { path: \"/absolute/path\", detail: \"original\" }))`.\n- When encoding an image to send with `codex.emitImage(...)` or `view_image`, prefer JPEG at about 85 quality when lossy compression is acceptable; use PNG when transparency or lossless detail matters. Smaller uploads are faster and less likely to hit size limits.\n- Top-level bindings persist across cells. If a cell throws, prior bindings remain available and bindings that finished initializing before the throw often remain usable in later cells. For code you plan to reuse across cells, prefer declaring or assigning it in direct top-level statements before operations that might throw. If you hit `SyntaxError: Identifier 'x' has already been declared`, first reuse the existing binding, reassign a previously declared `let`, or pick a new descriptive name. Use `{ ... }` only for a short temporary block when you specifically need local scratch names; do not wrap an entire cell in block scope if you want those names reusable later. Reset the kernel with `js_repl_reset` only when you need a clean state.\n- Top-level static import declarations (for example `import x from \"./file.js\"`) are currently unsupported in `js_repl`; use dynamic imports with `await import(\"pkg\")`, `await import(\"./file.js\")`, or `await import(\"/abs/path/file.mjs\")` instead. Imported local files must be ESM `.js`/`.mjs` files and run in the same REPL VM context. Bare package imports always resolve from REPL-global search roots (`CODEX_JS_REPL_NODE_MODULE_DIRS`, then cwd), not relative to the imported file location. Local files may statically import only other local relative/absolute/`file://` `.js`/`.mjs` files; package and builtin imports from local files must stay dynamic. `import.meta.resolve()` returns importable strings such as `file://...`, bare package names, and `node:...` specifiers. Local file modules reload between execs, while top-level bindings persist until `js_repl_reset`.\n- Do not call tools directly; use `js_repl` + `codex.tool(...)` for all tool calls, including shell commands.\n- MCP tools (if any) can also be called by name via `codex.tool(...)`.\n- Avoid direct access to `process.stdout` / `process.stderr` / `process.stdin`; it can corrupt the JSON line protocol. Use `console.log`, `codex.tool(...)`, and `codex.emitImage(...)`.";
    assert_eq!(res, expected);
}

#[tokio::test]
async fn js_repl_image_detail_original_does_not_change_instructions() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut cfg = make_config(&tmp, 4096, None).await;
    let mut features = cfg.features.get().clone();
    features
        .enable(Feature::JsRepl)
        .enable(Feature::ImageDetailOriginal);
    cfg.features
        .set(features)
        .expect("test config should allow js_repl image detail settings");

    let res = render_js_repl_instructions(&cfg).expect("js_repl instructions expected");
    let expected = "## JavaScript REPL (Node)\n- Use `js_repl` for Node-backed JavaScript with top-level await in a persistent kernel.\n- `js_repl` is a freeform/custom tool. Direct `js_repl` calls must send raw JavaScript tool input (optionally with first-line `// codex-js-repl: timeout_ms=15000`). Do not wrap code in JSON (for example `{\"code\":\"...\"}`), quotes, or markdown code fences.\n- Helpers: `codex.cwd`, `codex.homeDir`, `codex.tmpDir`, `codex.tool(name, args?)`, and `codex.emitImage(imageLike)`.\n- `codex.tool` executes a normal tool call and resolves to the raw tool output object. Use it for shell and non-shell tools alike. Nested tool outputs stay inside JavaScript unless you emit them explicitly.\n- `codex.emitImage(...)` adds one image to the outer `js_repl` function output each time you call it, so you can call it multiple times to emit multiple images. It accepts a data URL, a single `input_image` item, an object like `{ bytes, mimeType }`, or a raw tool response object with exactly one image and no text. It rejects mixed text-and-image content.\n- `codex.tool(...)` and `codex.emitImage(...)` keep stable helper identities across cells. Saved references and persisted objects can reuse them in later cells, but async callbacks that fire after a cell finishes still fail because no exec is active.\n- Request full-resolution image processing with `detail: \"original\"` only when the `view_image` tool schema includes a `detail` argument. The same availability applies to `codex.emitImage(...)`: if `view_image.detail` is present, you may also pass `detail: \"original\"` there. Use this when high-fidelity image perception or precise localization is needed, especially for CUA agents.\n- Example of sharing an in-memory Playwright screenshot: `await codex.emitImage({ bytes: await page.screenshot({ type: \"jpeg\", quality: 85 }), mimeType: \"image/jpeg\", detail: \"original\" })`.\n- Example of sharing a local image tool result: `await codex.emitImage(codex.tool(\"view_image\", { path: \"/absolute/path\", detail: \"original\" }))`.\n- When encoding an image to send with `codex.emitImage(...)` or `view_image`, prefer JPEG at about 85 quality when lossy compression is acceptable; use PNG when transparency or lossless detail matters. Smaller uploads are faster and less likely to hit size limits.\n- Top-level bindings persist across cells. If a cell throws, prior bindings remain available and bindings that finished initializing before the throw often remain usable in later cells. For code you plan to reuse across cells, prefer declaring or assigning it in direct top-level statements before operations that might throw. If you hit `SyntaxError: Identifier 'x' has already been declared`, first reuse the existing binding, reassign a previously declared `let`, or pick a new descriptive name. Use `{ ... }` only for a short temporary block when you specifically need local scratch names; do not wrap an entire cell in block scope if you want those names reusable later. Reset the kernel with `js_repl_reset` only when you need a clean state.\n- Top-level static import declarations (for example `import x from \"./file.js\"`) are currently unsupported in `js_repl`; use dynamic imports with `await import(\"pkg\")`, `await import(\"./file.js\")`, or `await import(\"/abs/path/file.mjs\")` instead. Imported local files must be ESM `.js`/`.mjs` files and run in the same REPL VM context. Bare package imports always resolve from REPL-global search roots (`CODEX_JS_REPL_NODE_MODULE_DIRS`, then cwd), not relative to the imported file location. Local files may statically import only other local relative/absolute/`file://` `.js`/`.mjs` files; package and builtin imports from local files must stay dynamic. `import.meta.resolve()` returns importable strings such as `file://...`, bare package names, and `node:...` specifiers. Local file modules reload between execs, while top-level bindings persist until `js_repl_reset`.\n- Avoid direct access to `process.stdout` / `process.stderr` / `process.stdin`; it can corrupt the JSON line protocol. Use `console.log`, `codex.tool(...)`, and `codex.emitImage(...)`.";
    assert_eq!(res, expected);
}

/// Project-doc assembly ignores config user_instructions, which are now a
/// separate contextual-user fragment.
#[tokio::test]
async fn project_doc_assembly_ignores_config_user_instructions() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("AGENTS.md"), "proj doc").unwrap();

    const INSTRUCTIONS: &str = "base instructions";

    let res =
        build_project_doc_instructions_text(&make_config(&tmp, 4096, Some(INSTRUCTIONS)).await)
            .await
            .expect("should produce project-doc instructions");

    assert_eq!(res, "proj doc");
}

/// With no project doc present, config user_instructions do not produce
/// project-doc text on their own.
#[tokio::test]
async fn config_user_instructions_do_not_create_project_doc_text() {
    let tmp = tempfile::tempdir().expect("tempdir");

    const INSTRUCTIONS: &str = "some instructions";

    let res =
        build_project_doc_instructions_text(&make_config(&tmp, 4096, Some(INSTRUCTIONS)).await)
            .await;

    assert_eq!(res, None);
}

/// When both the repository root and the working directory contain
/// AGENTS.md files, their contents are concatenated from root to cwd.
#[tokio::test]
async fn concatenates_root_and_cwd_docs() {
    let repo = tempfile::tempdir().expect("tempdir");

    // Simulate a git repository.
    std::fs::write(
        repo.path().join(".git"),
        "gitdir: /path/to/actual/git/dir\n",
    )
    .unwrap();

    // Repo root doc.
    fs::write(repo.path().join("AGENTS.md"), "root doc").unwrap();

    // Nested working directory with its own doc.
    let nested = repo.path().join("workspace/crate_a");
    std::fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("AGENTS.md"), "crate doc").unwrap();

    let mut cfg = make_config(&repo, 4096, None).await;
    cfg.cwd = nested;

    let res = build_project_doc_instructions_text(&cfg)
        .await
        .expect("doc expected");
    assert_eq!(res, "root doc\n\ncrate doc");
}

#[tokio::test]
async fn project_root_markers_are_honored_for_agents_discovery() {
    let root = tempfile::tempdir().expect("tempdir");
    fs::write(root.path().join(".codex-root"), "").unwrap();
    fs::write(root.path().join("AGENTS.md"), "parent doc").unwrap();

    let nested = root.path().join("dir1");
    fs::create_dir_all(nested.join(".git")).unwrap();
    fs::write(nested.join("AGENTS.md"), "child doc").unwrap();

    let mut cfg = make_config_with_project_root_markers(&root, 4096, None, &[".codex-root"]).await;
    cfg.cwd = nested;

    let discovery = discover_project_doc_paths(&cfg).expect("discover paths");
    let expected_parent =
        dunce::canonicalize(root.path().join("AGENTS.md")).expect("canonical parent doc path");
    let expected_child =
        dunce::canonicalize(cfg.cwd.join("AGENTS.md")).expect("canonical child doc path");
    assert_eq!(discovery.len(), 2);
    assert_eq!(discovery[0], expected_parent);
    assert_eq!(discovery[1], expected_child);

    let res = build_project_doc_instructions_text(&cfg)
        .await
        .expect("doc expected");
    assert_eq!(res, "parent doc\n\nchild doc");
}

/// AGENTS.override.md is preferred over AGENTS.md when both are present.
#[tokio::test]
async fn agents_local_md_preferred() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join(DEFAULT_PROJECT_DOC_FILENAME), "versioned").unwrap();
    fs::write(tmp.path().join(LOCAL_PROJECT_DOC_FILENAME), "local").unwrap();

    let cfg = make_config(&tmp, 4096, None).await;

    let res = build_project_doc_instructions_text(&cfg)
        .await
        .expect("local doc expected");

    assert_eq!(res, "local");

    let discovery = discover_project_doc_paths(&cfg).expect("discover paths");
    assert_eq!(discovery.len(), 1);
    assert_eq!(
        discovery[0].file_name().unwrap().to_string_lossy(),
        LOCAL_PROJECT_DOC_FILENAME
    );
}

/// When AGENTS.md is absent but a configured fallback exists, the fallback is used.
#[tokio::test]
async fn uses_configured_fallback_when_agents_missing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("EXAMPLE.md"), "example instructions").unwrap();

    let cfg = make_config_with_fallback(&tmp, 4096, None, &["EXAMPLE.md"]).await;

    let res = build_project_doc_instructions_text(&cfg)
        .await
        .expect("fallback doc expected");

    assert_eq!(res, "example instructions");
}

/// AGENTS.md remains preferred when both AGENTS.md and fallbacks are present.
#[tokio::test]
async fn agents_md_preferred_over_fallbacks() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("AGENTS.md"), "primary").unwrap();
    fs::write(tmp.path().join("EXAMPLE.md"), "secondary").unwrap();

    let cfg = make_config_with_fallback(&tmp, 4096, None, &["EXAMPLE.md", ".example.md"]).await;

    let res = build_project_doc_instructions_text(&cfg)
        .await
        .expect("AGENTS.md should win");

    assert_eq!(res, "primary");

    let discovery = discover_project_doc_paths(&cfg).expect("discover paths");
    assert_eq!(discovery.len(), 1);
    assert!(
        discovery[0]
            .file_name()
            .unwrap()
            .to_string_lossy()
            .eq(DEFAULT_PROJECT_DOC_FILENAME)
    );
}

#[tokio::test]
async fn render_skills_section_includes_available_skills_and_usage_rules() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = make_config(&tmp, 4096, None).await;
    create_skill(
        cfg.codex_home.clone(),
        "pdf-processing",
        "extract from pdfs",
    );

    let skills = load_test_skills(&cfg);
    let skill_list = skills.errors.is_empty().then_some(skills.skills.as_slice());
    let res = crate::skills::render_skills_section(skill_list.expect("skills expected"))
        .expect("instructions expected");
    let expected_path = dunce::canonicalize(
        cfg.codex_home
            .join("skills/pdf-processing/SKILL.md")
            .as_path(),
    )
    .unwrap_or_else(|_| cfg.codex_home.join("skills/pdf-processing/SKILL.md"));
    let expected_path_str = expected_path.to_string_lossy().replace('\\', "/");
    let usage_rules = "- Discovery: The list above is the skills available in this session (name + description + file path). Skill bodies live on disk at the listed paths.\n- Trigger rules: If the user names a skill (with `$SkillName` or plain text) OR the task clearly matches a skill's description shown above, you must use that skill for that turn. Multiple mentions mean use them all. Do not carry skills across turns unless re-mentioned.\n- Missing/blocked: If a named skill isn't in the list or the path can't be read, say so briefly and continue with the best fallback.\n- How to use a skill (progressive disclosure):\n  1) After deciding to use a skill, open its `SKILL.md`. Read only enough to follow the workflow.\n  2) When `SKILL.md` references relative paths (e.g., `scripts/foo.py`), resolve them relative to the skill directory listed above first, and only consider other paths if needed.\n  3) If `SKILL.md` points to extra folders such as `references/`, load only the specific files needed for the request; don't bulk-load everything.\n  4) If `scripts/` exist, prefer running or patching them instead of retyping large code blocks.\n  5) If `assets/` or templates exist, reuse them instead of recreating from scratch.\n- Coordination and sequencing:\n  - If multiple skills apply, choose the minimal set that covers the request and state the order you'll use them.\n  - Announce which skill(s) you're using and why (one short line). If you skip an obvious skill, say why.\n- Context hygiene:\n  - Keep context small: summarize long sections instead of pasting them; only load extra files when needed.\n  - Avoid deep reference-chasing: prefer opening only files directly linked from `SKILL.md` unless you're blocked.\n  - When variants exist (frameworks, providers, domains), pick only the relevant reference file(s) and note that choice.\n- Safety and fallback: If a skill can't be applied cleanly (missing files, unclear instructions), state the issue, pick the next-best approach, and continue.";
    let expected = format!(
        "<skills_instructions>\n## Skills\nA skill is a set of local instructions to follow that is stored in a `SKILL.md` file. Below is the list of skills that can be used. Each entry includes a name, description, and file path so you can open the source for full instructions when using a specific skill.\n### Available skills\n- pdf-processing: extract from pdfs (file: {expected_path_str})\n### How to use skills\n{usage_rules}\n</skills_instructions>"
    );
    assert_eq!(res, expected);
}

#[tokio::test]
async fn render_skills_section_without_project_doc() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = make_config(&tmp, 4096, None).await;
    create_skill(cfg.codex_home.clone(), "linting", "run clippy");

    let skills = load_test_skills(&cfg);
    let skill_list = skills.errors.is_empty().then_some(skills.skills.as_slice());
    let res = crate::skills::render_skills_section(skill_list.expect("skills expected"))
        .expect("instructions expected");
    let expected_path =
        dunce::canonicalize(cfg.codex_home.join("skills/linting/SKILL.md").as_path())
            .unwrap_or_else(|_| cfg.codex_home.join("skills/linting/SKILL.md"));
    let expected_path_str = expected_path.to_string_lossy().replace('\\', "/");
    let usage_rules = "- Discovery: The list above is the skills available in this session (name + description + file path). Skill bodies live on disk at the listed paths.\n- Trigger rules: If the user names a skill (with `$SkillName` or plain text) OR the task clearly matches a skill's description shown above, you must use that skill for that turn. Multiple mentions mean use them all. Do not carry skills across turns unless re-mentioned.\n- Missing/blocked: If a named skill isn't in the list or the path can't be read, say so briefly and continue with the best fallback.\n- How to use a skill (progressive disclosure):\n  1) After deciding to use a skill, open its `SKILL.md`. Read only enough to follow the workflow.\n  2) When `SKILL.md` references relative paths (e.g., `scripts/foo.py`), resolve them relative to the skill directory listed above first, and only consider other paths if needed.\n  3) If `SKILL.md` points to extra folders such as `references/`, load only the specific files needed for the request; don't bulk-load everything.\n  4) If `scripts/` exist, prefer running or patching them instead of retyping large code blocks.\n  5) If `assets/` or templates exist, reuse them instead of recreating from scratch.\n- Coordination and sequencing:\n  - If multiple skills apply, choose the minimal set that covers the request and state the order you'll use them.\n  - Announce which skill(s) you're using and why (one short line). If you skip an obvious skill, say why.\n- Context hygiene:\n  - Keep context small: summarize long sections instead of pasting them; only load extra files when needed.\n  - Avoid deep reference-chasing: prefer opening only files directly linked from `SKILL.md` unless you're blocked.\n  - When variants exist (frameworks, providers, domains), pick only the relevant reference file(s) and note that choice.\n- Safety and fallback: If a skill can't be applied cleanly (missing files, unclear instructions), state the issue, pick the next-best approach, and continue.";
    let expected = format!(
        "<skills_instructions>\n## Skills\nA skill is a set of local instructions to follow that is stored in a `SKILL.md` file. Below is the list of skills that can be used. Each entry includes a name, description, and file path so you can open the source for full instructions when using a specific skill.\n### Available skills\n- linting: run clippy (file: {expected_path_str})\n### How to use skills\n{usage_rules}\n</skills_instructions>"
    );
    assert_eq!(res, expected);
}

#[tokio::test]
async fn apps_feature_does_not_emit_user_instructions_by_itself() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut cfg = make_config(&tmp, 4096, None).await;
    cfg.features
        .enable(Feature::Apps)
        .expect("test config should allow apps");

    let res = build_project_doc_instructions_text(&cfg).await;
    assert_eq!(res, None);
}

#[tokio::test]
async fn apps_feature_does_not_append_to_project_doc_user_instructions() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("AGENTS.md"), "base doc").unwrap();

    let mut cfg = make_config(&tmp, 4096, None).await;
    cfg.features
        .enable(Feature::Apps)
        .expect("test config should allow apps");

    let res = build_project_doc_instructions_text(&cfg)
        .await
        .expect("instructions expected");
    assert_eq!(res, "base doc");
}

fn create_skill(codex_home: PathBuf, name: &str, description: &str) {
    let skill_dir = codex_home.join(format!("skills/{name}"));
    fs::create_dir_all(&skill_dir).unwrap();
    let content = format!("---\nname: {name}\ndescription: {description}\n---\n\n# Body\n");
    fs::write(skill_dir.join("SKILL.md"), content).unwrap();
}
