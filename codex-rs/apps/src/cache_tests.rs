use std::sync::Arc;

use pretty_assertions::assert_eq;
use rmcp::model::JsonObject;
use rmcp::model::Meta;
use serde_json::json;
use tempfile::TempDir;

use super::*;

const PRODUCTION_UPSTREAM: &str = "https://chatgpt.com/backend-api/ps/mcp";

fn context(home: &TempDir, account_id: &str) -> ScopedCodexAppsCacheContext {
    scoped_context(
        home,
        account_id,
        PRODUCTION_UPSTREAM,
        /*product_sku*/ None,
    )
}

fn scoped_context(
    home: &TempDir,
    account_id: &str,
    upstream_url: &str,
    product_sku: Option<&str>,
) -> ScopedCodexAppsCacheContext {
    CodexAppsCacheContext::new(
        home.path(),
        CodexAppsCacheIdentity::default().with_account_id(Some(account_id.to_string())),
    )
    .scoped(upstream_url.to_string(), product_sku.map(str::to_string))
}

fn tool(name: &str) -> Tool {
    Tool::new(name.to_string(), "test tool", Arc::new(JsonObject::new()))
}

#[test]
fn cache_is_isolated_by_full_identity() -> Result<()> {
    let home = TempDir::new()?;
    let personal = context(&home, "personal");
    let workspace = CodexAppsCacheContext::new(
        home.path(),
        CodexAppsCacheIdentity::default()
            .with_account_id(Some("personal".to_string()))
            .with_workspace_account(/*is_workspace_account*/ true),
    )
    .scoped(PRODUCTION_UPSTREAM.to_string(), /*product_sku*/ None);

    personal.write_tools(&[tool("personal_tool")])?;
    workspace.write_tools(&[tool("workspace_tool")])?;

    assert_ne!(personal.cache_path(), workspace.cache_path());
    assert_eq!(personal.load_tools()?, Some(vec![tool("personal_tool")]));
    assert_eq!(workspace.load_tools()?, Some(vec![tool("workspace_tool")]));
    Ok(())
}

#[test]
fn cache_is_isolated_by_upstream_and_product_sku() -> Result<()> {
    let home = TempDir::new()?;
    let production = scoped_context(
        &home,
        "same-user",
        PRODUCTION_UPSTREAM,
        /*product_sku*/ None,
    );
    let staging = scoped_context(
        &home,
        "same-user",
        "https://staging.example/api/codex/ps/mcp",
        /*product_sku*/ None,
    );
    let desktop = scoped_context(&home, "same-user", PRODUCTION_UPSTREAM, Some("desktop"));
    let cli = scoped_context(&home, "same-user", PRODUCTION_UPSTREAM, Some("cli"));

    production.write_tools(&[tool("production")])?;
    staging.write_tools(&[tool("staging")])?;
    desktop.write_tools(&[tool("desktop")])?;
    cli.write_tools(&[tool("cli")])?;

    let paths = [
        production.cache_path(),
        staging.cache_path(),
        desktop.cache_path(),
        cli.cache_path(),
    ];
    assert_eq!(
        paths.iter().collect::<std::collections::HashSet<_>>().len(),
        4
    );
    assert_eq!(production.load_tools()?, Some(vec![tool("production")]));
    assert_eq!(staging.load_tools()?, Some(vec![tool("staging")]));
    assert_eq!(desktop.load_tools()?, Some(vec![tool("desktop")]));
    assert_eq!(cli.load_tools()?, Some(vec![tool("cli")]));
    Ok(())
}

#[test]
fn provenance_free_raw_and_v4_caches_are_ignored() -> Result<()> {
    let home = TempDir::new()?;
    let context = context(&home, "legacy-scope");
    let identity_hash = stable_json_hash(&context.key.identity);
    let unscoped_raw_path = home
        .path()
        .join(CACHE_DIR)
        .join(format!("{identity_hash}.json"));
    let unscoped_v4_path = home
        .path()
        .join("cache/codex_apps_tools")
        .join(format!("{identity_hash}.json"));
    std::fs::create_dir_all(
        unscoped_raw_path
            .parent()
            .expect("raw cache path has parent"),
    )?;
    std::fs::write(
        &unscoped_raw_path,
        serde_json::to_vec_pretty(&RawToolsDiskCache {
            schema_version: CACHE_SCHEMA_VERSION,
            tools: vec![tool("unscoped-raw")],
        })?,
    )?;
    std::fs::create_dir_all(unscoped_v4_path.parent().expect("v4 cache path has parent"))?;
    std::fs::write(
        &unscoped_v4_path,
        serde_json::to_vec_pretty(&json!({
            "schema_version": 4,
            "tools": [],
        }))?,
    )?;

    assert_ne!(context.cache_path(), unscoped_raw_path);
    assert_eq!(context.load_tools()?, None);
    Ok(())
}

#[test]
fn corrupt_cache_is_reported_and_never_crosses_identity() -> Result<()> {
    let home = TempDir::new()?;
    let valid = context(&home, "valid");
    let corrupt = context(&home, "corrupt");
    valid.write_tools(&[tool("valid_tool")])?;
    let corrupt_path = corrupt.cache_path();
    std::fs::create_dir_all(corrupt_path.parent().expect("cache path has parent"))?;
    std::fs::write(&corrupt_path, b"{not-json")?;

    assert!(corrupt.load_tools().is_err());
    assert_eq!(valid.load_tools()?, Some(vec![tool("valid_tool")]));
    Ok(())
}

#[test]
fn cache_at_byte_limit_is_accepted() -> Result<()> {
    let home = TempDir::new()?;
    let context = context(&home, "exact-size");
    let path = context.cache_path();
    std::fs::create_dir_all(path.parent().expect("cache path has parent"))?;
    let mut contents = serde_json::to_vec(&RawToolsDiskCache {
        schema_version: CACHE_SCHEMA_VERSION,
        tools: Vec::new(),
    })?;
    contents.resize(MAX_CACHE_BYTES, b' ');
    std::fs::write(path, contents)?;

    assert_eq!(context.load_tools()?, Some(Vec::new()));
    Ok(())
}

#[test]
fn cache_over_byte_limit_is_rejected() -> Result<()> {
    let home = TempDir::new()?;
    let context = context(&home, "oversize");
    let path = context.cache_path();
    std::fs::create_dir_all(path.parent().expect("cache path has parent"))?;
    let mut contents = serde_json::to_vec(&RawToolsDiskCache {
        schema_version: CACHE_SCHEMA_VERSION,
        tools: Vec::new(),
    })?;
    contents.resize(MAX_CACHE_BYTES + 1, b' ');
    std::fs::write(path, contents)?;

    let error = context.load_tools().expect_err("oversized cache must fail");
    assert!(
        error.to_string().contains("exceeds the 8388608-byte limit"),
        "unexpected cache error: {error:#}"
    );
    Ok(())
}

#[test]
fn cache_rejects_raw_tool_inventory_over_limit_on_read_and_write() -> Result<()> {
    let home = TempDir::new()?;
    let context = context(&home, "too-many-tools");
    let tools = vec![tool("repeated"); crate::MAX_CODEX_APPS_TOOLS + 1];

    let write_error = context
        .write_tools(&tools)
        .expect_err("oversized inventory write must fail");
    assert!(
        write_error
            .to_string()
            .contains("exceeded the 4096-tool limit"),
        "unexpected cache write error: {write_error:#}"
    );

    let path = context.cache_path();
    std::fs::create_dir_all(path.parent().expect("cache path has parent"))?;
    std::fs::write(
        path,
        serde_json::to_vec(&RawToolsDiskCache {
            schema_version: CACHE_SCHEMA_VERSION,
            tools,
        })?,
    )?;
    let read_error = context
        .load_tools()
        .expect_err("oversized inventory read must fail");
    assert!(
        read_error
            .to_string()
            .contains("exceeded the 4096-tool limit"),
        "unexpected cache read error: {read_error:#}"
    );
    Ok(())
}

#[test]
fn atomic_write_replaces_a_previous_inventory() -> Result<()> {
    let home = TempDir::new()?;
    let context = context(&home, "replace");
    context.write_tools(&[tool("old")])?;
    context.write_tools(&[tool("new")])?;

    assert_eq!(context.load_tools()?, Some(vec![tool("new")]));
    Ok(())
}

#[test]
fn private_approval_context_is_removed_from_existing_and_new_caches() -> Result<()> {
    let home = TempDir::new()?;
    let context = context(&home, "private-context");
    let mut tool = tool("private");
    let meta = tool.meta.get_or_insert_with(Meta::new);
    meta.insert(
        codex_protocol::mcp::MCP_APPROVAL_CONTEXT_META_KEY.to_string(),
        json!({ "connectedAccountEmail": "spoofed@example.com" }),
    );
    meta.insert(
        META_CODEX_APPS.to_string(),
        json!({
            META_CONNECTED_ACCOUNT_EMAIL: "owner@example.com",
            "retained": true,
        }),
    );

    let path = context.cache_path();
    std::fs::create_dir_all(path.parent().expect("cache path parent"))?;
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&RawToolsDiskCache {
            schema_version: CACHE_SCHEMA_VERSION,
            tools: vec![tool.clone()],
        })?,
    )?;
    let loaded = context.load_tools()?.expect("existing cache");
    assert_private_context_removed(&loaded[0]);

    context.write_tools(&[tool])?;
    let loaded = context.load_tools()?.expect("new cache");
    assert_private_context_removed(&loaded[0]);
    Ok(())
}

fn assert_private_context_removed(tool: &Tool) {
    let meta = tool.meta.as_ref().expect("tool metadata");
    assert!(
        meta.get(codex_protocol::mcp::MCP_APPROVAL_CONTEXT_META_KEY)
            .is_none()
    );
    let source = meta
        .get(META_CODEX_APPS)
        .and_then(JsonValue::as_object)
        .expect("Apps source metadata");
    assert!(source.get(META_CONNECTED_ACCOUNT_EMAIL).is_none());
    assert_eq!(source.get("retained"), Some(&json!(true)));
}
