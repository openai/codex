# Remove Hardcoded App ID Filters Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the seven-ID connector denylist from connector listings and Codex Apps MCP tools without changing unrelated visibility, discovery, accessibility, or approval behavior.

**Architecture:** Delete both copies of the denylist and remove the filter call sites instead of retaining pass-through policy APIs. Preserve server-provided visibility and existing generic connector/tool processing, with regression coverage at the MCP cache, app-server integration, and plugin-app listing boundaries.

**Tech Stack:** Rust, Tokio, RMCP, Cargo/Just, Bazel lock generation, `pretty_assertions`.

---

### Task 1: Stop filtering Codex Apps MCP tools by connector ID

**Files:**
- Modify: `codex-rs/codex-mcp/src/connection_manager_tests.rs:611`
- Modify: `codex-rs/codex-mcp/src/codex_apps.rs:18,215-240,282-291`
- Modify: `codex-rs/codex-mcp/src/rmcp_client.rs:21,345-408`
- Modify: `codex-rs/utils/plugins/src/mcp_connector.rs:1-27`
- Modify: `codex-rs/utils/plugins/Cargo.toml:16-24`
- Modify: `codex-rs/Cargo.lock`
- Modify: `MODULE.bazel.lock`

- [ ] **Step 1: Change the cache test to require preservation of a formerly denied connector**

Replace the existing denylist cache test with:

```rust
#[test]
fn codex_apps_tools_cache_preserves_formerly_disallowed_connectors() {
    let codex_home = tempdir().expect("tempdir");
    let cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let tools = vec![
        create_test_tool_with_connector(
            CODEX_APPS_MCP_SERVER_NAME,
            "formerly_blocked_tool",
            "connector_2b0a9009c9c64bf9933a3dae3f2b1254",
            Some("Formerly Blocked"),
        ),
        create_test_tool_with_connector(
            CODEX_APPS_MCP_SERVER_NAME,
            "calendar_tool",
            "calendar",
            Some("Calendar"),
        ),
    ];

    write_cached_codex_apps_tools(&cache_context, &tools);
    let cached = read_cached_codex_apps_tools(&cache_context).expect("cache entry exists for user");

    assert_eq!(cached, tools);
}
```

- [ ] **Step 2: Run the regression test and verify RED**

Run from `codex-rs`:

```bash
just test -p codex-mcp codex_apps_tools_cache_preserves_formerly_disallowed_connectors
```

Expected: FAIL because the cache currently removes `formerly_blocked_tool`.

- [ ] **Step 3: Remove MCP denylist filtering and its duplicate policy**

In `codex_apps.rs`, return cached tools without filtering:

```rust
CachedCodexAppsToolsLoad::Hit(cache.tools)
```

Write the supplied tools without filtering:

```rust
let Ok(bytes) = serde_json::to_vec_pretty(&CodexAppsToolsDiskCache {
    schema_version: CODEX_APPS_TOOLS_CACHE_SCHEMA_VERSION,
    tools: tools.to_vec(),
}) else {
    return;
};
```

Delete `filter_disallowed_codex_apps_tools` and its `is_connector_id_allowed` import. In `rmcp_client.rs`, delete the filter import and replace the originator-specific return branch with:

```rust
Ok(tools)
```

Delete the denylist constants and `is_connector_id_allowed` functions from `mcp_connector.rs`, leaving `sanitize_name` and `sanitize_slug`. Remove this manifest dependency because it is no longer used by `codex-utils-plugins`:

```toml
codex-login = { workspace = true }
```

- [ ] **Step 4: Verify GREEN for the MCP cache and utility crate**

Run from `codex-rs`:

```bash
just test -p codex-mcp codex_apps_tools_cache_preserves_formerly_disallowed_connectors
just test -p codex-utils-plugins
```

Expected: both commands PASS.

- [ ] **Step 5: Refresh and validate dependency locks**

Run from the repository root:

```bash
just bazel-lock-update
just bazel-lock-check
```

Expected: the `codex-utils-plugins` Cargo lock entry no longer lists `codex-login`, and the Bazel lock check exits successfully.

- [ ] **Step 6: Commit the MCP policy removal**

```bash
git add codex-rs/codex-mcp/src/connection_manager_tests.rs \
  codex-rs/codex-mcp/src/codex_apps.rs \
  codex-rs/codex-mcp/src/rmcp_client.rs \
  codex-rs/utils/plugins/src/mcp_connector.rs \
  codex-rs/utils/plugins/Cargo.toml \
  codex-rs/Cargo.lock MODULE.bazel.lock
git commit -m "Remove app ID filtering from MCP tools"
```

### Task 2: Stop filtering connector listings by connector ID

**Files:**
- Modify: `codex-rs/app-server/tests/suite/v2/app_list.rs:367-432`
- Modify: `codex-rs/chatgpt/src/connectors.rs:11-24,130-184,268-279`
- Modify: `codex-rs/connectors/src/filter.rs:5-66,102-159`
- Modify: `codex-rs/core/src/connectors.rs:34,110-115,144-166,240-246,352-358`

- [ ] **Step 1: Make the app-server integration test use a formerly denied ID**

In `list_apps_keeps_apps_with_app_only_tools_accessible`, define and use:

```rust
let connector_id = "connector_2b0a9009c9c64bf9933a3dae3f2b1254";
let connectors = vec![AppInfo {
    id: connector_id.to_string(),
    name: "Formerly Blocked".to_string(),
    description: Some("Formerly blocked connector".to_string()),
    logo_url: None,
    logo_url_dark: None,
    distribution_channel: None,
    branding: None,
    app_metadata: None,
    labels: None,
    install_url: None,
    is_accessible: false,
    is_enabled: true,
    plugin_display_names: Vec::new(),
}];
let mut app_only_tool = connector_tool(connector_id, "Formerly Blocked")?;
```

Change the final ID assertion to:

```rust
assert_eq!(data[0].id, connector_id);
```

- [ ] **Step 2: Change the plugin-app test to require the formerly denied app**

Replace the existing denylist test with:

```rust
#[test]
fn connectors_for_plugin_apps_preserves_formerly_disallowed_plugin_apps() {
    let connector_id = "asdk_app_6938a94a61d881918ef32cb999ff937c";
    let connectors = connectors_for_plugin_apps(
        Vec::new(),
        &[AppConnectorId(connector_id.to_string())],
    );
    assert_eq!(
        connectors,
        vec![merged_app(connector_id, /*is_accessible*/ false)]
    );
}
```

- [ ] **Step 3: Run both regression tests and verify RED**

Run from `codex-rs`:

```bash
just test -p codex-chatgpt connectors_for_plugin_apps_preserves_formerly_disallowed_plugin_apps
just test -p codex-app-server list_apps_keeps_apps_with_app_only_tools_accessible
```

Expected: both FAIL because connector listing still removes the formerly denied IDs. The app-server failure should retain the directory app but report it inaccessible or omit it due to the remaining connector filter.

- [ ] **Step 4: Remove the connector-list denylist and originator plumbing**

Reduce `filter_tool_suggest_discoverable_connectors` to generic discovery checks:

```rust
pub fn filter_tool_suggest_discoverable_connectors(
    directory_connectors: Vec<AppInfo>,
    accessible_connectors: &[AppInfo],
    discoverable_connector_ids: &HashSet<String>,
) -> Vec<AppInfo> {
    let accessible_connector_ids: HashSet<&str> = accessible_connectors
        .iter()
        .filter(|connector| connector.is_accessible)
        .map(|connector| connector.id.as_str())
        .collect();

    let mut connectors = directory_connectors
        .into_iter()
        .filter(|connector| !accessible_connector_ids.contains(connector.id.as_str()))
        .filter(|connector| discoverable_connector_ids.contains(connector.id.as_str()))
        .collect::<Vec<_>>();
    connectors.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.id.cmp(&right.id))
    });
    connectors
}
```

Delete both denylist constants, `filter_disallowed_connectors`, the local originator classifier, and the denylist-specific unit tests. Remove the originator argument from the two remaining discovery tests and from the `codex-core` call site.

In `codex-core`, replace the cached connector filtering closure with:

```rust
read_cached_accessible_connectors(&cache_key)
```

and:

```rust
let accessible_connectors = accessible_connectors_from_mcp_tools(mcp_tools);
```

Remove the now-unused `originator` import.

In `codex-chatgpt`, remove the filter and originator imports. Rename `merge_and_filter_plugin_connectors` to `merge_directory_and_plugin_connectors` and return `merge_plugin_connectors(...)` directly. Build `connectors_by_id` directly from the merged connectors, and return `merge_connectors(connectors, accessible_connectors)` directly from `merge_connectors_with_accessible`.

- [ ] **Step 5: Verify GREEN across connector consumers**

Run from `codex-rs`:

```bash
just test -p codex-connectors
just test -p codex-chatgpt
just test -p codex-core
just test -p codex-app-server list_apps_keeps_apps_with_app_only_tools_accessible
```

Expected: all commands PASS, including both formerly denied connector regressions and existing generic visibility/discovery tests.

- [ ] **Step 6: Commit the connector-list policy removal**

```bash
git add codex-rs/app-server/tests/suite/v2/app_list.rs \
  codex-rs/chatgpt/src/connectors.rs \
  codex-rs/connectors/src/filter.rs \
  codex-rs/core/src/connectors.rs
git commit -m "Remove app ID filtering from connector lists"
```

### Task 3: Run final repository validation

**Files:**
- Verify: all files changed in Tasks 1 and 2

- [ ] **Step 1: Prove the policy and IDs are gone from runtime code**

Run from the repository root:

```bash
rg -n 'filter_disallowed_connectors|filter_disallowed_codex_apps_tools|is_connector_id_allowed|DISALLOWED_CONNECTOR_IDS|FIRST_PARTY_CHAT_DISALLOWED_CONNECTOR_IDS' codex-rs
rg -n 'asdk_app_6938a94a61d881918ef32cb999ff937c|connector_2b0a9009c9c64bf9933a3dae3f2b1254|connector_3f8d1a79f27c4c7ba1a897ab13bf37dc|connector_68de829bf7648191acd70a907364c67c|connector_68e004f14af881919eb50893d3d9f523|connector_69272cb413a081919685ec3c88d1744e|connector_0f9c9d4592e54d0a9a12b3f44a1e2010' codex-rs --glob '!**/tests/**' --glob '!**/*_tests.rs'
```

Expected: the first search has no matches. The second search has no production-policy matches; regression-test references are permitted.

- [ ] **Step 2: Run complete changed-crate tests**

Run from `codex-rs`:

```bash
just test -p codex-mcp
just test -p codex-connectors
just test -p codex-chatgpt
just test -p codex-core
just test -p codex-app-server
```

Expected: all commands PASS.

- [ ] **Step 3: Request approval and run the complete workspace test suite**

Because `codex-core` changes, request user approval before running:

```bash
just test
```

Expected: the complete workspace test suite PASSes.

- [ ] **Step 4: Run scoped lint fixes and formatting last**

Run from `codex-rs`:

```bash
just fix -p codex-mcp
just fix -p codex-connectors
just fix -p codex-chatgpt
just fix -p codex-core
just fix -p codex-app-server
just fmt
```

Per repository instructions, do not rerun tests after `fix` or `fmt`.

- [ ] **Step 5: Inspect the final diff**

Run from the repository root:

```bash
git diff --check
git status -sb
git diff --stat origin/main...HEAD
git diff origin/main...HEAD -- codex-rs docs/superpowers
```

Expected: no whitespace errors, only scoped files are changed, and no denylist runtime code remains.

- [ ] **Step 6: Commit any validation-only changes**

If lock generation, lint fixes, or formatting changed tracked files after the task commits, stage the scoped file set explicitly:

```bash
git add MODULE.bazel.lock \
  codex-rs/Cargo.lock \
  codex-rs/app-server/tests/suite/v2/app_list.rs \
  codex-rs/chatgpt/src/connectors.rs \
  codex-rs/codex-mcp/src/codex_apps.rs \
  codex-rs/codex-mcp/src/connection_manager_tests.rs \
  codex-rs/codex-mcp/src/rmcp_client.rs \
  codex-rs/connectors/src/filter.rs \
  codex-rs/core/src/connectors.rs \
  codex-rs/utils/plugins/Cargo.toml \
  codex-rs/utils/plugins/src/mcp_connector.rs
git commit -m "Finalize app ID filter removal"
```

### Task 4: Publish the draft pull request

**Files:**
- Verify: committed branch state only

- [ ] **Step 1: Verify GitHub prerequisites and PR scope**

```bash
gh --version
gh auth status
git status -sb
git log --oneline origin/main..HEAD
git diff --stat origin/main...HEAD
```

Expected: `gh` is installed and authenticated, the worktree is clean, and only the design, plan, tests, implementation, and lockfile changes are present.

- [ ] **Step 2: Push the branch**

```bash
git push -u origin codex/remove-hardcoded-app-id-filters
```

Expected: the branch is created on `origin` with upstream tracking.

- [ ] **Step 3: Open a draft PR**

Create a draft PR targeting the remote default branch with title:

```text
[codex] Remove hardcoded app ID filters
```

The PR body must summarize the duplicate denylist removal, preserved service-driven visibility and approval behavior, regression coverage, dependency cleanup, and every validation command run.

- [ ] **Step 4: Report the published result**

Report the branch, commits, draft PR URL, target branch, tests, lock checks, and any skipped validation with its reason.
