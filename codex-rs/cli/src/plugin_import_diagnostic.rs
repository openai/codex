use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use codex_core_plugins::PluginInstallRequest;
use codex_core_plugins::PluginsManager;
use codex_core_plugins::loader::diagnose_plugin_apps;
use codex_core_plugins::loader::diagnose_plugin_mcp_servers;
use codex_core_plugins::loader::load_plugin_hooks;
use codex_core_plugins::loader::load_plugin_skills;
use codex_core_plugins::manifest::diagnose_plugin_manifest;
use codex_core_plugins::marketplace::MarketplacePluginAuthPolicy;
use codex_core_plugins::marketplace::MarketplacePluginInstallPolicy;
use codex_core_plugins::marketplace::MarketplacePluginSource;
use codex_core_plugins::marketplace::ResolvedMarketplacePlugin;
use codex_core_plugins::marketplace::diagnose_marketplace;
use codex_core_plugins::marketplace::find_marketplace_manifest_path;
use codex_core_plugins::marketplace::load_marketplace;
use codex_core_plugins::store::PluginStore;
use codex_protocol::protocol::Product;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

const REPORT_SCHEMA_VERSION: u32 = 1;
const MAX_DETAIL_CHARS: usize = 4_096;
const MAX_SKILL_SCAN_DEPTH: usize = 6;
const MAX_SKILL_SCAN_DIRS: usize = 2_000;
const MAX_SKILL_SCAN_ISSUES: usize = 1_000;

#[derive(Debug, Parser)]
#[command(
    bin_name = "codex debug marketplace-import-diagnostic",
    about = "Generate a redacted per-plugin marketplace import diagnostic"
)]
pub(crate) struct MarketplaceImportDiagnosticCommand {
    /// Marketplace repository root containing .claude-plugin/marketplace.json or
    /// .agents/plugins/marketplace.json.
    #[arg(value_name = "MARKETPLACE_ROOT")]
    marketplace_root: PathBuf,

    /// New JSONL report path. Each record is flushed immediately so partial reports survive failures.
    #[arg(long, value_name = "JSONL_PATH")]
    output: PathBuf,

    /// Exercise the real plugin installer in an isolated temporary Codex home.
    #[arg(long, default_value_t = false)]
    attempt_install: bool,
}

#[derive(Default)]
struct Summary {
    raw_entries: usize,
    detected_entries: usize,
    detection_failures: usize,
    eligible_entries: usize,
    installed_entries: usize,
    install_failures: usize,
    install_skips: usize,
    entries_with_capability_issues: usize,
    entries_without_capabilities: usize,
}

pub(crate) async fn run(command: MarketplaceImportDiagnosticCommand) -> Result<()> {
    if let Some(parent) = command.output.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create report directory {}", parent.display()))?;
    }
    let diagnostic_home = tempfile::Builder::new()
        .prefix("codex-marketplace-import-diagnostic-")
        .tempdir()
        .context("failed to create isolated diagnostic CODEX_HOME")?;
    let requested_root = if command.marketplace_root.is_absolute() {
        command.marketplace_root.clone()
    } else {
        std::env::current_dir()
            .context("failed to resolve current directory")?
            .join(&command.marketplace_root)
    };
    let file = create_report_file(&command.output)?;
    let mut writer = BufWriter::new(file);
    let mut redactor = Redactor::new(&requested_root, diagnostic_home.path());
    write_record(
        &mut writer,
        &json!({
            "schemaVersion": REPORT_SCHEMA_VERSION,
            "event": "run_started",
            "unixTimeSeconds": SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            "codexVersion": env!("CARGO_PKG_VERSION"),
            "buildCommit": option_env!("CODEX_BUILD_COMMIT")
                .or(option_env!("GIT_COMMIT"))
                .unwrap_or("unknown"),
            "platform": { "os": std::env::consts::OS, "arch": std::env::consts::ARCH },
            "marketplaceRoot": "<marketplace-root>",
            "attemptInstall": command.attempt_install,
            "isolatedCodexHome": true,
            "scope": "checked_out_marketplace_enumeration_installation_and_capabilities",
            "notInspected": ["claude_settings_selection", "marketplace_clone_or_auth"],
            "privacy": {
                "pluginContentsIncluded": false,
                "environmentValuesIncluded": false,
                "configContentsIncluded": false,
                "rawCommandArgumentsIncluded": false,
                "sanitizedErrorMessagesIncluded": true,
                "identifiersIncluded": [
                    "marketplace",
                    "plugin",
                    "skill",
                    "mcp_server",
                    "app_connector",
                    "relative_capability_path"
                ]
            }
        }),
    )?;

    let marketplace_root = match std::fs::canonicalize(&command.marketplace_root) {
        Ok(path) => path,
        Err(err) => {
            return finish_preflight_failure(
                &mut writer,
                &command.output,
                "marketplace_root_unavailable",
                &redactor.detail(&err.to_string()),
            );
        }
    };
    let marketplace_root = match AbsolutePathBuf::try_from(marketplace_root) {
        Ok(path) => path,
        Err(err) => {
            return finish_preflight_failure(
                &mut writer,
                &command.output,
                "marketplace_root_not_absolute",
                &redactor.detail(&err.to_string()),
            );
        }
    };
    redactor.add_replacement(marketplace_root.as_path(), "<marketplace-root>");
    let Some(marketplace_path) = find_marketplace_manifest_path(marketplace_root.as_path()) else {
        return finish_preflight_failure(
            &mut writer,
            &command.output,
            "marketplace_manifest_not_found",
            "marketplace root does not contain .claude-plugin/marketplace.json or .agents/plugins/marketplace.json",
        );
    };

    let production_load = match load_marketplace(&marketplace_path) {
        Ok(marketplace) => json!({
            "status": "ok",
            "detectedPluginCount": marketplace.plugins.len()
        }),
        Err(err) => json!({
            "status": "error",
            "code": "production_marketplace_load_failed",
            "detail": redactor.detail(&err.to_string())
        }),
    };
    let diagnostics = match diagnose_marketplace(&marketplace_path) {
        Ok(diagnostics) => diagnostics,
        Err(err) => {
            write_record(
                &mut writer,
                &json!({
                    "schemaVersion": REPORT_SCHEMA_VERSION,
                    "event": "marketplace_diagnostic",
                    "marketplaceManifest": relative_path(&marketplace_path, &marketplace_root, &redactor),
                    "productionLoader": production_load,
                    "status": "error",
                    "code": "diagnostic_marketplace_parse_failed",
                    "detail": redactor.detail(&err.to_string())
                }),
            )?;
            write_record(
                &mut writer,
                &json!({
                    "schemaVersion": REPORT_SCHEMA_VERSION,
                    "event": "run_completed",
                    "status": "report_complete_marketplace_unreadable",
                    "summary": summary_json(&Summary::default())
                }),
            )?;
            println!(
                "Marketplace import diagnostic written to {}",
                command.output.display()
            );
            return Ok(());
        }
    };

    let duplicate_names = duplicate_name_counts(&diagnostics.plugins);
    write_record(
        &mut writer,
        &json!({
            "schemaVersion": REPORT_SCHEMA_VERSION,
            "event": "marketplace_diagnostic",
            "status": "ok",
            "marketplaceManifest": relative_path(&marketplace_path, &marketplace_root, &redactor),
            "marketplaceName": diagnostics.name,
            "rawPluginEntryCount": diagnostics.plugins.len(),
            "duplicatePluginNames": duplicate_names,
            "productionLoader": production_load
        }),
    )?;

    let manager = PluginsManager::new(diagnostic_home.path().to_path_buf());
    let store = PluginStore::new(diagnostic_home.path().to_path_buf());
    let mut summary = Summary {
        raw_entries: diagnostics.plugins.len(),
        ..Default::default()
    };
    let mut seen_plugin_names = HashSet::new();

    for entry in diagnostics.plugins {
        let entry_index = entry.index;
        let source_kind = entry.source_kind;
        let entry_name = entry
            .name
            .clone()
            .unwrap_or_else(|| format!("<entry-{entry_index}>"));
        write_record(
            &mut writer,
            &json!({
                "schemaVersion": REPORT_SCHEMA_VERSION,
                "event": "plugin_started",
                "entryIndex": entry_index,
                "pluginName": entry_name,
                "sourceKind": source_kind
            }),
        )?;
        let resolved = match entry.result {
            Ok(resolved) => resolved,
            Err(err) => {
                summary.detection_failures += 1;
                write_record(
                    &mut writer,
                    &json!({
                        "schemaVersion": REPORT_SCHEMA_VERSION,
                        "event": "plugin_diagnostic",
                        "entryIndex": entry_index,
                        "pluginName": entry_name,
                        "sourceKind": source_kind,
                        "outcome": "not_detected",
                        "detection": {
                            "status": "error",
                            "code": err.code,
                            "detail": redactor.detail(&err.message)
                        },
                        "eligibility": null,
                        "install": null,
                        "capabilities": null
                    }),
                )?;
                continue;
            }
        };

        summary.detected_entries += 1;
        let is_duplicate = !seen_plugin_names.insert(resolved.plugin_id.plugin_name.clone());
        let eligible = plugin_is_eligible(&resolved);
        if eligible && !is_duplicate {
            summary.eligible_entries += 1;
        }

        let (install, installed_root, outcome) = if is_duplicate {
            summary.install_skips += 1;
            (
                json!({
                    "status": "skipped",
                    "code": "duplicate_plugin_name",
                    "detail": "plugin lookup resolves the first marketplace entry with this name"
                }),
                None,
                "detected_not_selectable",
            )
        } else if !eligible {
            summary.install_skips += 1;
            (
                json!({
                    "status": "skipped",
                    "code": eligibility_failure_code(&resolved),
                    "detail": eligibility_failure_detail(&resolved)
                }),
                None,
                "detected_not_eligible",
            )
        } else if !command.attempt_install {
            summary.install_skips += 1;
            (
                json!({
                    "status": "skipped",
                    "code": "install_not_requested",
                    "detail": "the diagnostic was run without --attempt-install"
                }),
                None,
                "detected_only",
            )
        } else {
            write_plugin_stage(
                &mut writer,
                entry_index,
                &resolved.plugin_id.plugin_name,
                "install",
            )?;
            match manager
                .install_plugin(PluginInstallRequest {
                    plugin_name: resolved.plugin_id.plugin_name.clone(),
                    marketplace_path: marketplace_path.clone(),
                })
                .await
            {
                Ok(installed) => {
                    summary.installed_entries += 1;
                    (
                        json!({
                            "status": "ok",
                            "code": "installed",
                            "version": installed.plugin_version
                        }),
                        Some(installed.installed_path),
                        "installed",
                    )
                }
                Err(err) => {
                    summary.install_failures += 1;
                    (
                        install_failure_diagnostic(&resolved, &err.to_string(), &redactor),
                        None,
                        "install_failed",
                    )
                }
            }
        };

        let source_root = installed_root.or_else(|| match &resolved.source {
            MarketplacePluginSource::Local { path } => Some(path.clone()),
            MarketplacePluginSource::Git { .. } => None,
        });
        let capabilities = match source_root {
            Some(root) => {
                write_plugin_stage(
                    &mut writer,
                    entry_index,
                    &resolved.plugin_id.plugin_name,
                    "capabilities",
                )?;
                let result = diagnose_capabilities(&root, &resolved, &store, &redactor).await;
                if result.issue_count > 0 {
                    summary.entries_with_capability_issues += 1;
                }
                if !result.has_capabilities {
                    summary.entries_without_capabilities += 1;
                }
                result.value
            }
            None => json!({
                "status": "unavailable",
                "code": "source_not_materialized",
                "detail": "remote plugin capabilities cannot be inspected because installation did not succeed"
            }),
        };

        write_record(
            &mut writer,
            &json!({
                "schemaVersion": REPORT_SCHEMA_VERSION,
                "event": "plugin_diagnostic",
                "entryIndex": entry_index,
                "pluginName": resolved.plugin_id.plugin_name,
                "pluginId": resolved.plugin_id.as_key(),
                "sourceKind": source_kind,
                "outcome": outcome,
                "detection": {
                    "status": "ok",
                    "code": if is_duplicate { "detected_duplicate_name" } else { "detected" }
                },
                "eligibility": {
                    "status": if eligible && !is_duplicate { "ok" } else { "skipped" },
                    "installationPolicy": installation_policy_name(resolved.policy.installation),
                    "authenticationPolicy": authentication_policy_name(resolved.policy.authentication),
                    "productPolicy": product_policy_names(resolved.policy.products.as_deref()),
                    "code": if is_duplicate {
                        Some("duplicate_plugin_name")
                    } else if eligible {
                        None
                    } else {
                        Some(eligibility_failure_code(&resolved))
                    }
                },
                "install": install,
                "capabilities": capabilities
            }),
        )?;
    }

    write_record(
        &mut writer,
        &json!({
            "schemaVersion": REPORT_SCHEMA_VERSION,
            "event": "run_completed",
            "status": "report_complete",
            "summary": summary_json(&summary)
        }),
    )?;
    println!(
        "Marketplace import diagnostic written to {} ({} entries, {} installed, {} install failures)",
        command.output.display(),
        summary.raw_entries,
        summary.installed_entries,
        summary.install_failures
    );
    Ok(())
}

struct CapabilityResult {
    value: Value,
    issue_count: usize,
    has_capabilities: bool,
}

async fn diagnose_capabilities(
    plugin_root: &AbsolutePathBuf,
    resolved: &ResolvedMarketplacePlugin,
    store: &PluginStore,
    redactor: &Redactor,
) -> CapabilityResult {
    let manifest_diagnostic = diagnose_plugin_manifest(plugin_root.as_path());
    let manifest_path = manifest_diagnostic
        .manifest_path
        .as_deref()
        .map(|path| relative_path_from_path(path, plugin_root, redactor));
    let manifest_issues = manifest_diagnostic
        .issues
        .iter()
        .map(|issue| redactor.detail(issue))
        .collect::<Vec<_>>();
    let unsupported_capability_fields = manifest_diagnostic.unsupported_capability_fields;
    let unsupported_capability_issue_count = unsupported_capability_fields.len();
    let Some(manifest) = manifest_diagnostic.manifest else {
        return CapabilityResult {
            issue_count: manifest_issues.len().max(1),
            has_capabilities: false,
            value: json!({
                "status": "error",
                "code": "manifest_unavailable",
                "manifest": {
                    "path": manifest_path,
                    "issues": manifest_issues,
                    "unsupportedCapabilityFields": unsupported_capability_fields
                },
                "skills": null,
                "mcp": null,
                "hooks": null,
                "apps": null
            }),
        };
    };

    let skill_config_rules = Default::default();
    let skills = load_plugin_skills(
        plugin_root,
        &resolved.plugin_id,
        &manifest.paths,
        Some(Product::Codex),
        &skill_config_rules,
    )
    .await;
    let skill_roots = capability_skill_roots(plugin_root, &manifest.paths.skills);
    let skill_root_probes = skill_roots
        .iter()
        .map(|path| probe_skill_root(path, plugin_root, redactor))
        .collect::<Vec<_>>();
    let skill_root_issue_count = skill_root_probes
        .iter()
        .filter(|probe| probe["status"] != "ok")
        .count();
    let skill_errors = skills
        .errors
        .iter()
        .map(|error| {
            json!({
                "path": relative_path(&error.path, plugin_root, redactor),
                "detail": redactor.detail(&error.message)
            })
        })
        .collect::<Vec<_>>();
    let skill_names = skills
        .skills
        .iter()
        .map(|skill| {
            json!({
                "name": skill.name,
                "path": relative_path(&skill.path_to_skills_md, plugin_root, redactor)
            })
        })
        .collect::<Vec<_>>();

    let mcp = diagnose_plugin_mcp_servers(plugin_root.as_path(), &manifest.paths).await;
    let mcp_issues = mcp
        .issues
        .iter()
        .map(|issue| redactor.detail(issue))
        .collect::<Vec<_>>();
    let app = diagnose_plugin_apps(plugin_root.as_path(), &manifest.paths).await;
    let app_issues = app
        .issues
        .iter()
        .map(|issue| redactor.detail(issue))
        .collect::<Vec<_>>();
    let (hook_sources, hook_warnings) = load_plugin_hooks(
        plugin_root,
        &resolved.plugin_id,
        &store.plugin_data_root(&resolved.plugin_id),
        &manifest.paths,
    );
    let hook_warnings = hook_warnings
        .iter()
        .map(|warning| redactor.detail(warning))
        .collect::<Vec<_>>();

    let manifest_name_matches = manifest.name == resolved.plugin_id.plugin_name;
    let issue_count = manifest_issues.len()
        + skill_errors.len()
        + mcp_issues.len()
        + app_issues.len()
        + hook_warnings.len()
        + skill_root_issue_count
        + unsupported_capability_issue_count
        + usize::from(!manifest_name_matches);
    let has_capabilities = !skill_names.is_empty()
        || !mcp.server_names.is_empty()
        || !hook_sources.is_empty()
        || !app.connector_ids.is_empty();
    let status = if issue_count > 0 { "warning" } else { "ok" };

    CapabilityResult {
        issue_count,
        has_capabilities,
        value: json!({
            "status": status,
            "code": if has_capabilities { "capabilities_inspected" } else { "no_supported_capabilities_detected" },
            "manifest": {
                "path": manifest_path,
                "name": manifest.name,
                "nameMatchesMarketplace": manifest_name_matches,
                "versionPresent": manifest.version.is_some(),
                "issues": manifest_issues,
                "unsupportedCapabilityFields": unsupported_capability_fields
            },
            "skills": {
                "configuredRoots": skill_root_probes,
                "detected": skill_names,
                "errors": skill_errors,
                "reasonWhenEmpty": if skills.skills.is_empty() {
                    Some(if skill_roots.is_empty() {
                        "no default skills directory or manifest skills path was configured"
                    } else if skills.had_errors || skill_root_issue_count > 0 {
                        "skill roots were configured but one or more skills failed to load or scan"
                    } else {
                        "skill roots were inspected but no Codex-compatible skills were detected"
                    })
                } else {
                    None
                }
            },
            "mcp": {
                "configPaths": mcp.config_paths.iter()
                    .map(|path| relative_path(path, plugin_root, redactor))
                    .collect::<Vec<_>>(),
                "serverNames": mcp.server_names,
                "issues": mcp_issues,
                "reasonWhenEmpty": if mcp.config_paths.is_empty() {
                    Some("no default .mcp.json or manifest mcpServers path was configured")
                } else if mcp.server_names.is_empty() {
                    Some("MCP config paths were inspected but no valid servers were detected")
                } else {
                    None
                }
            },
            "hooks": {
                "sourcePaths": hook_sources.iter()
                    .map(|source| source.source_relative_path.clone())
                    .collect::<Vec<_>>(),
                "sourceCount": hook_sources.len(),
                "issues": hook_warnings,
                "reasonWhenEmpty": if hook_sources.is_empty() {
                    Some("no valid manifest hooks or default hooks/hooks.json declarations were detected")
                } else {
                    None
                }
            },
            "apps": {
                "configPaths": app.config_paths.iter()
                    .map(|path| relative_path(path, plugin_root, redactor))
                    .collect::<Vec<_>>(),
                "connectorIds": app.connector_ids.iter()
                    .map(|connector| connector.0.clone())
                    .collect::<Vec<_>>(),
                "issues": app_issues,
                "reasonWhenEmpty": if app.config_paths.is_empty() {
                    Some("no default .app.json or manifest apps path was configured")
                } else if app.connector_ids.is_empty() {
                    Some("app config paths were inspected but no valid connector IDs were detected")
                } else {
                    None
                }
            }
        }),
    }
}

fn probe_skill_root(
    path: &AbsolutePathBuf,
    plugin_root: &AbsolutePathBuf,
    redactor: &Redactor,
) -> Value {
    let display_path = relative_path(path, plugin_root, redactor);
    let metadata = match std::fs::metadata(path.as_path()) {
        Ok(metadata) => metadata,
        Err(err) => {
            return json!({
                "path": display_path,
                "status": "error",
                "code": "metadata_failed",
                "directoriesScanned": 0,
                "issues": [{
                    "code": "metadata_failed",
                    "path": display_path,
                    "detail": redactor.detail(&err.to_string())
                }]
            });
        }
    };
    if !metadata.is_dir() {
        return json!({
            "path": display_path,
            "status": "error",
            "code": "not_a_directory",
            "directoriesScanned": 0,
            "issues": [{
                "code": "not_a_directory",
                "path": display_path
            }]
        });
    }

    let root =
        std::fs::canonicalize(path.as_path()).unwrap_or_else(|_| path.as_path().to_path_buf());
    let mut queue = VecDeque::from([(root.clone(), 0)]);
    let mut visited = HashSet::from([root]);
    let mut issues = Vec::new();
    let mut issue_count = 0;
    let mut truncated_by_directory_limit = false;

    while let Some((directory, depth)) = queue.pop_front() {
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(err) => {
                push_skill_scan_issue(
                    &mut issues,
                    &mut issue_count,
                    json!({
                        "code": "read_directory_failed",
                        "path": relative_path_from_path(&directory, plugin_root, redactor),
                        "detail": redactor.detail(&err.to_string())
                    }),
                );
                continue;
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    push_skill_scan_issue(
                        &mut issues,
                        &mut issue_count,
                        json!({
                            "code": "read_directory_entry_failed",
                            "path": relative_path_from_path(&directory, plugin_root, redactor),
                            "detail": redactor.detail(&err.to_string())
                        }),
                    );
                    continue;
                }
            };
            let entry_path = entry.path();
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let metadata = match std::fs::symlink_metadata(&entry_path) {
                Ok(metadata) => metadata,
                Err(err) => {
                    push_skill_scan_issue(
                        &mut issues,
                        &mut issue_count,
                        json!({
                            "code": "entry_metadata_failed",
                            "path": relative_path_from_path(&entry_path, plugin_root, redactor),
                            "detail": redactor.detail(&err.to_string())
                        }),
                    );
                    continue;
                }
            };

            if metadata.file_type().is_symlink() {
                match std::fs::read_dir(&entry_path) {
                    Ok(_) => enqueue_skill_scan_directory(
                        &entry_path,
                        depth + 1,
                        &mut queue,
                        &mut visited,
                        &mut truncated_by_directory_limit,
                    ),
                    Err(err)
                        if matches!(
                            err.kind(),
                            std::io::ErrorKind::NotADirectory | std::io::ErrorKind::NotFound
                        ) => {}
                    Err(err) => push_skill_scan_issue(
                        &mut issues,
                        &mut issue_count,
                        json!({
                            "code": "read_symlink_directory_failed",
                            "path": relative_path_from_path(&entry_path, plugin_root, redactor),
                            "detail": redactor.detail(&err.to_string())
                        }),
                    ),
                }
            } else if metadata.is_dir() {
                enqueue_skill_scan_directory(
                    &entry_path,
                    depth + 1,
                    &mut queue,
                    &mut visited,
                    &mut truncated_by_directory_limit,
                );
            }
        }
    }

    if truncated_by_directory_limit {
        push_skill_scan_issue(
            &mut issues,
            &mut issue_count,
            json!({
                "code": "directory_limit_reached",
                "detail": format!("skill scan stopped adding directories after {MAX_SKILL_SCAN_DIRS} unique directories")
            }),
        );
    }

    json!({
        "path": display_path,
        "status": if issue_count == 0 { "ok" } else { "warning" },
        "code": if issue_count == 0 { "skill_tree_scanned" } else { "skill_tree_scan_issues" },
        "directoriesScanned": visited.len(),
        "issues": issues,
        "omittedIssueCount": issue_count.saturating_sub(MAX_SKILL_SCAN_ISSUES)
    })
}

fn enqueue_skill_scan_directory(
    path: &Path,
    depth: usize,
    queue: &mut VecDeque<(PathBuf, usize)>,
    visited: &mut HashSet<PathBuf>,
    truncated_by_directory_limit: &mut bool,
) {
    if depth > MAX_SKILL_SCAN_DEPTH {
        return;
    }
    if visited.len() >= MAX_SKILL_SCAN_DIRS {
        *truncated_by_directory_limit = true;
        return;
    }
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if visited.insert(path.clone()) {
        queue.push_back((path, depth));
    }
}

fn push_skill_scan_issue(issues: &mut Vec<Value>, issue_count: &mut usize, issue: Value) {
    *issue_count += 1;
    if issues.len() < MAX_SKILL_SCAN_ISSUES {
        issues.push(issue);
    }
}

fn capability_skill_roots(
    plugin_root: &AbsolutePathBuf,
    manifest_skill_root: &Option<AbsolutePathBuf>,
) -> Vec<AbsolutePathBuf> {
    let mut roots = Vec::new();
    let default_root = plugin_root.join("skills");
    if default_root.as_path().is_dir() {
        roots.push(default_root);
    }
    if let Some(root) = manifest_skill_root {
        roots.push(root.clone());
    }
    roots.sort_unstable();
    roots.dedup();
    roots
}

fn plugin_is_eligible(plugin: &ResolvedMarketplacePlugin) -> bool {
    plugin.policy.installation != MarketplacePluginInstallPolicy::NotAvailable
        && match plugin.policy.products.as_deref() {
            None => true,
            Some([]) => false,
            Some(products) => products.contains(&Product::Codex),
        }
}

fn install_failure_diagnostic(
    plugin: &ResolvedMarketplacePlugin,
    install_error: &str,
    redactor: &Redactor,
) -> Value {
    if let MarketplacePluginSource::Local { path } = &plugin.source {
        match std::fs::metadata(path.as_path()) {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return json!({
                    "status": "error",
                    "code": "source_path_not_found",
                    "detail": "marketplace local source path does not exist"
                });
            }
            Err(err) => {
                return json!({
                    "status": "error",
                    "code": "source_path_metadata_failed",
                    "detail": redactor.detail(&err.to_string())
                });
            }
            Ok(metadata) if !metadata.is_dir() => {
                return json!({
                    "status": "error",
                    "code": "source_path_not_directory",
                    "detail": "marketplace local source path is not a directory"
                });
            }
            Ok(_) => {}
        }
    }

    json!({
        "status": "error",
        "code": "install_failed",
        "detail": redactor.detail(install_error)
    })
}

fn eligibility_failure_code(plugin: &ResolvedMarketplacePlugin) -> &'static str {
    if plugin.policy.installation == MarketplacePluginInstallPolicy::NotAvailable {
        "installation_policy_not_available"
    } else {
        "product_policy_excludes_codex"
    }
}

fn eligibility_failure_detail(plugin: &ResolvedMarketplacePlugin) -> &'static str {
    if plugin.policy.installation == MarketplacePluginInstallPolicy::NotAvailable {
        "marketplace policy marks the plugin as not available for installation"
    } else {
        "marketplace product policy does not include Codex"
    }
}

fn installation_policy_name(policy: MarketplacePluginInstallPolicy) -> &'static str {
    match policy {
        MarketplacePluginInstallPolicy::NotAvailable => "not_available",
        MarketplacePluginInstallPolicy::Available => "available",
        MarketplacePluginInstallPolicy::InstalledByDefault => "installed_by_default",
    }
}

fn authentication_policy_name(policy: MarketplacePluginAuthPolicy) -> &'static str {
    match policy {
        MarketplacePluginAuthPolicy::OnInstall => "on_install",
        MarketplacePluginAuthPolicy::OnUse => "on_use",
    }
}

fn product_policy_names(products: Option<&[Product]>) -> Value {
    match products {
        None => Value::Null,
        Some(products) => json!(
            products
                .iter()
                .map(|product| product.to_app_platform())
                .collect::<Vec<_>>()
        ),
    }
}

fn duplicate_name_counts(
    plugins: &[codex_core_plugins::marketplace::MarketplacePluginDiagnostic],
) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for plugin in plugins {
        if let Some(name) = &plugin.name {
            *counts.entry(name.clone()).or_insert(0) += 1;
        }
    }
    counts.retain(|_, count| *count > 1);
    counts
}

fn summary_json(summary: &Summary) -> Value {
    json!({
        "rawEntries": summary.raw_entries,
        "detectedEntries": summary.detected_entries,
        "detectionFailures": summary.detection_failures,
        "eligibleEntries": summary.eligible_entries,
        "installedEntries": summary.installed_entries,
        "installFailures": summary.install_failures,
        "installSkips": summary.install_skips,
        "entriesWithCapabilityIssues": summary.entries_with_capability_issues,
        "entriesWithoutCapabilities": summary.entries_without_capabilities
    })
}

fn create_report_file(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .with_context(|| format!("failed to create new report {}", path.display()))
}

fn finish_preflight_failure(
    writer: &mut BufWriter<File>,
    output: &Path,
    code: &str,
    detail: &str,
) -> Result<()> {
    write_record(
        writer,
        &json!({
            "schemaVersion": REPORT_SCHEMA_VERSION,
            "event": "marketplace_diagnostic",
            "status": "error",
            "code": code,
            "detail": detail
        }),
    )?;
    write_record(
        writer,
        &json!({
            "schemaVersion": REPORT_SCHEMA_VERSION,
            "event": "run_completed",
            "status": "report_complete_preflight_failed",
            "summary": summary_json(&Summary::default())
        }),
    )?;
    println!(
        "Marketplace import diagnostic written to {} (preflight failed: {code})",
        output.display()
    );
    Ok(())
}

fn write_plugin_stage(
    writer: &mut BufWriter<File>,
    entry_index: usize,
    plugin_name: &str,
    stage: &str,
) -> Result<()> {
    write_record(
        writer,
        &json!({
            "schemaVersion": REPORT_SCHEMA_VERSION,
            "event": "plugin_stage_started",
            "entryIndex": entry_index,
            "pluginName": plugin_name,
            "stage": stage
        }),
    )
}

fn write_record(writer: &mut BufWriter<File>, value: &Value) -> Result<()> {
    serde_json::to_writer(&mut *writer, value).context("failed to serialize diagnostic record")?;
    writer
        .write_all(b"\n")
        .context("failed to write diagnostic record")?;
    writer.flush().context("failed to flush diagnostic record")
}

fn relative_path(path: &AbsolutePathBuf, root: &AbsolutePathBuf, redactor: &Redactor) -> String {
    relative_path_from_path(path.as_path(), root, redactor)
}

fn relative_path_from_path(path: &Path, root: &AbsolutePathBuf, redactor: &Redactor) -> String {
    path.strip_prefix(root.as_path())
        .map(|relative| {
            if relative.as_os_str().is_empty() {
                ".".to_string()
            } else {
                format!("./{}", relative.to_string_lossy().replace('\\', "/"))
            }
        })
        .unwrap_or_else(|_| redactor.detail(&path.display().to_string()))
}

struct Redactor {
    replacements: Vec<(String, &'static str)>,
}

impl Redactor {
    fn new(marketplace_root: &Path, diagnostic_home: &Path) -> Self {
        let mut replacements = vec![
            (marketplace_root.display().to_string(), "<marketplace-root>"),
            (diagnostic_home.display().to_string(), "<diagnostic-home>"),
        ];
        for value in [std::env::var_os("HOME"), std::env::var_os("USERPROFILE")]
            .into_iter()
            .flatten()
        {
            let value = PathBuf::from(value);
            if value.is_absolute() {
                replacements.push((value.display().to_string(), "<home>"));
            }
        }
        if let Ok(cwd) = std::env::current_dir() {
            replacements.push((cwd.display().to_string(), "<cwd>"));
        }
        let mut redactor = Self { replacements };
        redactor.normalize_replacements();
        redactor
    }

    fn add_replacement(&mut self, path: &Path, replacement: &'static str) {
        let value = path.display().to_string();
        self.replacements.retain(|(existing, _)| existing != &value);
        self.replacements.push((value, replacement));
        self.normalize_replacements();
    }

    fn normalize_replacements(&mut self) {
        self.replacements
            .sort_by_key(|entry| std::cmp::Reverse(entry.0.len()));
        self.replacements.dedup_by(|left, right| left.0 == right.0);
    }

    fn detail(&self, detail: &str) -> String {
        let mut redacted = detail.to_string();
        for (value, replacement) in &self.replacements {
            if !value.is_empty() {
                redacted = redacted.replace(value, replacement);
            }
        }
        redacted = redact_git_command_arguments(&redacted);
        redacted = redact_urls(&redacted);
        redacted = redact_scp_git_remotes(redacted);
        redacted = redact_absolute_paths(redacted);
        redacted = redact_parser_values(redacted);
        redacted = redact_known_secrets(redacted);
        truncate_chars(redacted, MAX_DETAIL_CHARS)
    }
}

fn redact_git_command_arguments(detail: &str) -> String {
    if let Some(output_start) = detail.find("\nstdout:") {
        let header = &detail[..output_start];
        if let Some(status_start) = header.rfind(" failed with status ") {
            return format!(
                "git <redacted arguments>{}; git output omitted; failure class: {}",
                &header[status_start..],
                git_failure_class(detail)
            );
        }
    }
    if let Some(command_start) = detail.find("failed to run git ")
        && let Some((_, suffix)) = detail[command_start..].rsplit_once(": ")
    {
        return format!("failed to run git <redacted arguments>: {suffix}");
    }
    detail.to_string()
}

fn git_failure_class(detail: &str) -> &'static str {
    let detail = detail.to_ascii_lowercase();
    if detail.contains("authentication")
        || detail.contains("permission denied")
        || detail.contains("publickey")
    {
        "authentication_or_authorization"
    } else if detail.contains("repository not found") || detail.contains("not found") {
        "repository_not_found"
    } else if detail.contains("could not resolve host")
        || detail.contains("name or service not known")
    {
        "dns_resolution"
    } else if detail.contains("timed out") || detail.contains("timeout") {
        "timeout"
    } else if detail.contains("connection refused") {
        "connection_refused"
    } else if detail.contains("host key verification failed") {
        "host_key_verification"
    } else {
        "git_command_failed"
    }
}

fn redact_scp_git_remotes(input: String) -> String {
    static SCP_REMOTE: OnceLock<regex_lite::Regex> = OnceLock::new();
    let regex = SCP_REMOTE
        .get_or_init(|| compile_redaction_regex(r"\b[A-Za-z0-9._-]+@[A-Za-z0-9.-]+:[^\s]+"));
    regex
        .replace_all(&input, "<git-remote>:<redacted>")
        .to_string()
}

fn redact_absolute_paths(input: String) -> String {
    static UNIX_PATH: OnceLock<regex_lite::Regex> = OnceLock::new();
    static WINDOWS_PATH: OnceLock<regex_lite::Regex> = OnceLock::new();
    static UNC_PATH: OnceLock<regex_lite::Regex> = OnceLock::new();
    let unix_path =
        UNIX_PATH.get_or_init(|| compile_redaction_regex(r#"(^|[\s=(\[\"'`])\/[^\s,;:)\]\"'`]+"#));
    let windows_path = WINDOWS_PATH
        .get_or_init(|| compile_redaction_regex(r#"(?i)\b[A-Z]:[\\/][^\s,;:)\]\"'`]+"#));
    let unc_path = UNC_PATH.get_or_init(|| compile_redaction_regex(r#"\\\\[^\s,;:)\]\"'`]+"#));
    let redacted = unix_path.replace_all(&input, "$1<absolute-path>");
    let redacted = windows_path
        .replace_all(&redacted, "<absolute-path>")
        .to_string();
    unc_path
        .replace_all(&redacted, "<absolute-path>")
        .to_string()
}

fn redact_known_secrets(input: String) -> String {
    static OPENAI_KEY: OnceLock<regex_lite::Regex> = OnceLock::new();
    static AWS_ACCESS_KEY: OnceLock<regex_lite::Regex> = OnceLock::new();
    static BEARER_TOKEN: OnceLock<regex_lite::Regex> = OnceLock::new();
    static SECRET_ASSIGNMENT: OnceLock<regex_lite::Regex> = OnceLock::new();

    let openai_key = OPENAI_KEY.get_or_init(|| compile_redaction_regex(r"sk-[A-Za-z0-9]{20,}"));
    let aws_access_key =
        AWS_ACCESS_KEY.get_or_init(|| compile_redaction_regex(r"\bAKIA[0-9A-Z]{16}\b"));
    let bearer_token = BEARER_TOKEN
        .get_or_init(|| compile_redaction_regex(r"(?i)\bBearer\s+[A-Za-z0-9._\-]{16,}\b"));
    let secret_assignment = SECRET_ASSIGNMENT.get_or_init(|| {
        compile_redaction_regex(
            r#"(?i)\b(api[_-]?key|access[_-]?token|client[_-]?secret|token|secret|password|credential)\b(\s*[:=]\s*)(["']?)[^\s"']{8,}"#,
        )
    });

    let redacted = openai_key.replace_all(&input, "[REDACTED_SECRET]");
    let redacted = aws_access_key.replace_all(&redacted, "[REDACTED_SECRET]");
    let redacted = bearer_token.replace_all(&redacted, "Bearer [REDACTED_SECRET]");
    secret_assignment
        .replace_all(&redacted, "$1$2$3[REDACTED_SECRET]")
        .to_string()
}

fn redact_parser_values(input: String) -> String {
    static PARSER_VALUE: OnceLock<regex_lite::Regex> = OnceLock::new();
    let regex = PARSER_VALUE.get_or_init(|| {
        compile_redaction_regex(
            r#"(?i)\b(string|integer|float|boolean|variant|value|token)(\s+)(`[^`\r\n]*`|"[^"\r\n]*"|'[^'\r\n]*')"#,
        )
    });
    regex
        .replace_all(&input, "$1$2<redacted-value>")
        .to_string()
}

fn compile_redaction_regex(pattern: &str) -> regex_lite::Regex {
    match regex_lite::Regex::new(pattern) {
        Ok(regex) => regex,
        Err(err) => panic!("invalid built-in diagnostic redaction regex: {err}"),
    }
}

fn redact_urls(detail: &str) -> String {
    detail
        .split_inclusive(char::is_whitespace)
        .map(redact_url_token)
        .collect()
}

fn redact_url_token(token: &str) -> String {
    let Some(scheme_end) = token.find("://") else {
        return token.to_string();
    };
    let mut suffix_start = token.len();
    while suffix_start > scheme_end + 3
        && matches!(
            token.as_bytes()[suffix_start - 1],
            b' ' | b'\t' | b'\n' | b'\r' | b'.' | b',' | b';' | b':' | b')' | b']'
        )
    {
        suffix_start -= 1;
    }
    let (_, suffix) = token.split_at(suffix_start);
    format!("<redacted-url>{suffix}")
}

fn truncate_chars(value: String, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value;
    }
    let mut truncated = value.chars().take(limit).collect::<String>();
    truncated.push_str("...[truncated]");
    truncated
}

#[cfg(test)]
#[path = "plugin_import_diagnostic_tests.rs"]
mod tests;
