use async_trait::async_trait;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time;
use tracing::warn;

use crate::protocol::StopHookEvent;
use crate::protocol::StopHookEventDecision;
use crate::protocol::StopHookEventStage;
use crate::protocol::StopHookEventStatus;

const DOT_CODEX_DIR: &str = ".codex";
const HOOKS_DIR: &str = "hooks";
const HOOKS_FILE: &str = "hooks.json";
const DEFAULT_HOOK_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StopHooksToml {
    #[serde(default)]
    pub include_project_hooks: Option<bool>,
    #[serde(default)]
    pub sources: BTreeMap<String, StopHookSourceToml>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StopHookSourceToml {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub order: Option<i64>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Option<Vec<String>>,
    #[serde(default)]
    pub timeout: Option<u64>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub file: Option<AbsolutePathBuf>,
    #[serde(default, rename = "type")]
    pub hook_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopHooksConfig {
    pub include_project_hooks: bool,
    pub sources: Vec<StopHookSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopHookSource {
    Command(StopHookCommand),
    HooksFile(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopHookCommand {
    pub command: String,
    pub args: Vec<String>,
    pub timeout: Option<u64>,
    pub timeout_ms: Option<u64>,
}

impl StopHooksConfig {
    pub fn from_toml(value: Option<StopHooksToml>) -> Self {
        let mut config = StopHooksConfig {
            include_project_hooks: true,
            sources: Vec::new(),
        };
        let Some(value) = value else {
            return config;
        };

        config.include_project_hooks = value.include_project_hooks.unwrap_or(true);

        let mut sources = Vec::new();
        for (name, source) in value.sources {
            if source.enabled == Some(false) {
                continue;
            }

            let order = source.order.unwrap_or(0);
            if let Some(file) = source.file {
                if source.command.is_some() {
                    warn!("Stop hook source `{name}` specifies both file and command; skipping",);
                    continue;
                }
                sources.push((order, name, StopHookSource::HooksFile(file.into())));
                continue;
            }

            let Some(command) = source.command else {
                warn!("Stop hook source `{name}` missing command or file; skipping");
                continue;
            };
            if let Some(hook_type) = source.hook_type.as_deref()
                && !hook_type.eq_ignore_ascii_case("command")
            {
                warn!("Stop hook source `{name}` has unsupported type `{hook_type}`; skipping",);
                continue;
            }
            let args = source.args.unwrap_or_default();
            let entry = StopHookCommand {
                command,
                args,
                timeout: source.timeout,
                timeout_ms: source.timeout_ms,
            };
            sources.push((order, name, StopHookSource::Command(entry)));
        }

        sources.sort_by(|(left_order, left_name, _), (right_order, right_name, _)| {
            left_order
                .cmp(right_order)
                .then_with(|| left_name.cmp(right_name))
        });

        config.sources = sources.into_iter().map(|(_, _, source)| source).collect();
        config
    }
}

impl Default for StopHooksConfig {
    fn default() -> Self {
        Self {
            include_project_hooks: true,
            sources: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StopHookContext {
    pub cwd: PathBuf,
    pub conversation_id: String,
    pub turn_id: String,
    pub rollout_path: Option<PathBuf>,
    pub input_messages: Vec<String>,
    pub last_agent_message: Option<String>,
    pub stop_hooks: StopHooksConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopHookDecision {
    Allow,
    Block {
        reason: String,
        system_message: Option<String>,
    },
}

#[derive(Debug, Error)]
pub enum StopHookError {
    #[error("stop hook I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("stop hook JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[async_trait]
pub trait StopHookEventSink: Send + Sync {
    async fn emit(&self, event: StopHookEvent);
}

#[derive(Debug, Clone)]
struct StopHookRunOutcome {
    status: StopHookEventStatus,
    output: Option<HookOutput>,
}

#[derive(Debug, Deserialize, Clone)]
struct HooksFile {
    #[serde(default)]
    hooks: Option<HookSection>,
    #[serde(default, rename = "Stop")]
    stop_upper: Option<Vec<StopHookItem>>,
    #[serde(default, rename = "stop")]
    stop_lower: Option<Vec<StopHookItem>>,
}

#[derive(Debug, Deserialize, Clone)]
struct HookSection {
    #[serde(default, rename = "Stop")]
    stop_upper: Option<Vec<StopHookItem>>,
    #[serde(default, rename = "stop")]
    stop_lower: Option<Vec<StopHookItem>>,
}

#[derive(Debug, Clone)]
enum StopHookItem {
    Entry(HookEntry),
    Group(HookGroup),
}

impl<'de> Deserialize<'de> for StopHookItem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        if value.get("hooks").is_some() {
            let group: HookGroup =
                serde_json::from_value(value).map_err(serde::de::Error::custom)?;
            Ok(StopHookItem::Group(group))
        } else {
            let entry: HookEntry =
                serde_json::from_value(value).map_err(serde::de::Error::custom)?;
            Ok(StopHookItem::Entry(entry))
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
struct HookGroup {
    #[serde(default)]
    hooks: Vec<HookEntry>,
    #[serde(default, rename = "matcher")]
    _matcher: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct HookEntry {
    #[serde(default, rename = "type")]
    hook_type: Option<String>,
    command: Option<String>,
    #[serde(default)]
    args: Option<Vec<String>>,
    #[serde(default)]
    timeout: Option<u64>,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
struct HookOutput {
    decision: Option<String>,
    reason: Option<String>,
    #[serde(rename = "systemMessage")]
    system_message: Option<String>,
}

#[derive(Debug, Serialize)]
struct HookInput {
    hook_event_name: &'static str,
    cwd: String,
    conversation_id: String,
    turn_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    rollout_path: Option<String>,
    input_messages: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_agent_message: Option<String>,
}

pub async fn evaluate_stop_hooks(
    ctx: StopHookContext,
    reporter: Option<&dyn StopHookEventSink>,
) -> Result<StopHookDecision, StopHookError> {
    let hook_entries = collect_hook_entries(&ctx).await;
    if hook_entries.is_empty() {
        return Ok(StopHookDecision::Allow);
    }

    let total = u32::try_from(hook_entries.len()).unwrap_or(u32::MAX);
    emit_stop_hook_event(
        reporter,
        StopHookEvent {
            stage: StopHookEventStage::Started,
            hook_display: None,
            index: None,
            total: Some(total),
            status: None,
            decision: None,
            reason: None,
            system_message: None,
            error: None,
            duration_ms: None,
        },
    )
    .await;

    let input = HookInput {
        hook_event_name: "stop",
        cwd: ctx.cwd.display().to_string(),
        conversation_id: ctx.conversation_id.clone(),
        turn_id: ctx.turn_id.clone(),
        rollout_path: ctx
            .rollout_path
            .as_ref()
            .map(|path| path.display().to_string()),
        input_messages: ctx.input_messages.clone(),
        last_agent_message: ctx.last_agent_message.clone(),
    };
    let payload = serde_json::to_vec(&input)?;

    let mut system_messages: Vec<String> = Vec::new();
    for (idx, hook) in hook_entries.into_iter().enumerate() {
        let index = u32::try_from(idx + 1).unwrap_or(u32::MAX);
        let hook_display = hook.display();
        emit_stop_hook_event(
            reporter,
            StopHookEvent {
                stage: StopHookEventStage::HookStarted,
                hook_display: Some(hook_display.clone()),
                index: Some(index),
                total: Some(total),
                status: None,
                decision: None,
                reason: None,
                system_message: None,
                error: None,
                duration_ms: None,
            },
        )
        .await;

        let started_at = Instant::now();
        let outcome = match run_hook(&ctx, &hook, &payload).await {
            Ok(outcome) => outcome,
            Err(err) => {
                warn!("Stop hook execution failed for {}: {err}", hook_display);
                emit_stop_hook_event(
                    reporter,
                    StopHookEvent {
                        stage: StopHookEventStage::HookFinished,
                        hook_display: Some(hook_display),
                        index: Some(index),
                        total: Some(total),
                        status: Some(StopHookEventStatus::Error),
                        decision: None,
                        reason: None,
                        system_message: None,
                        error: Some(err.to_string()),
                        duration_ms: Some(elapsed_ms(started_at)),
                    },
                )
                .await;
                continue;
            }
        };

        let mut status = outcome.status;
        let mut decision = None;
        let mut error = None;
        let mut block_reason = None;
        if let Some(result) = outcome.output {
            if let Some(message) = result.system_message
                && !message.trim().is_empty()
            {
                system_messages.push(message);
            }
            if let Some(decision_raw) = result.decision {
                match decision_raw.to_ascii_lowercase().as_str() {
                    "block" => {
                        let reason = result.reason.unwrap_or_default();
                        if reason.trim().is_empty() {
                            warn!(
                                "Stop hook returned block without reason; ignoring: {}",
                                hook_display
                            );
                            status = StopHookEventStatus::InvalidOutput;
                            error = Some("block without reason".to_string());
                        } else {
                            decision = Some(StopHookEventDecision::Block);
                            block_reason = Some(reason);
                        }
                    }
                    "approve" => {
                        decision = Some(StopHookEventDecision::Allow);
                    }
                    _ => {
                        warn!("Stop hook returned unknown decision: {decision_raw}");
                        status = StopHookEventStatus::InvalidOutput;
                        error = Some(format!("unknown decision: {decision_raw}"));
                    }
                }
            }
        }

        emit_stop_hook_event(
            reporter,
            StopHookEvent {
                stage: StopHookEventStage::HookFinished,
                hook_display: Some(hook_display.clone()),
                index: Some(index),
                total: Some(total),
                status: Some(status),
                decision,
                reason: block_reason.clone(),
                system_message: None,
                error,
                duration_ms: Some(elapsed_ms(started_at)),
            },
        )
        .await;

        if let Some(reason) = block_reason {
            let system_message = combine_system_messages(system_messages);
            emit_stop_hook_event(
                reporter,
                StopHookEvent {
                    stage: StopHookEventStage::Completed,
                    hook_display: None,
                    index: None,
                    total: Some(total),
                    status: None,
                    decision: Some(StopHookEventDecision::Block),
                    reason: Some(reason.clone()),
                    system_message: system_message.clone(),
                    error: None,
                    duration_ms: None,
                },
            )
            .await;
            return Ok(StopHookDecision::Block {
                reason,
                system_message,
            });
        }
    }

    emit_stop_hook_event(
        reporter,
        StopHookEvent {
            stage: StopHookEventStage::Completed,
            hook_display: None,
            index: None,
            total: Some(total),
            status: None,
            decision: Some(StopHookEventDecision::Allow),
            reason: None,
            system_message: None,
            error: None,
            duration_ms: None,
        },
    )
    .await;

    Ok(StopHookDecision::Allow)
}

async fn emit_stop_hook_event(reporter: Option<&dyn StopHookEventSink>, event: StopHookEvent) {
    if let Some(reporter) = reporter {
        reporter.emit(event).await;
    }
}

fn elapsed_ms(started_at: Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
}

async fn collect_hook_entries(ctx: &StopHookContext) -> Vec<HookEntry> {
    let mut entries = Vec::new();
    let mut seen_files: HashSet<PathBuf> = HashSet::new();

    for source in &ctx.stop_hooks.sources {
        match source {
            StopHookSource::Command(command) => {
                entries.push(hook_entry_from_command(command));
            }
            StopHookSource::HooksFile(path) => {
                if !seen_files.insert(path.clone()) {
                    continue;
                }
                match load_hook_entries(path).await {
                    Ok(mut file_entries) => entries.append(&mut file_entries),
                    Err(err) => {
                        warn!("Stop hook file {} failed to load: {err}", path.display());
                    }
                }
            }
        }
    }

    if ctx.stop_hooks.include_project_hooks
        && let Some(path) = find_hooks_file(&ctx.cwd)
        && seen_files.insert(path.clone())
    {
        match load_hook_entries(&path).await {
            Ok(mut file_entries) => entries.append(&mut file_entries),
            Err(err) => {
                warn!("Stop hook file {} failed to load: {err}", path.display());
            }
        }
    }

    entries
}

async fn load_hook_entries(path: &Path) -> Result<Vec<HookEntry>, StopHookError> {
    let contents = tokio::fs::read_to_string(path).await?;
    let hooks_file: HooksFile = serde_json::from_str(&contents)?;
    Ok(extract_stop_hooks(&hooks_file))
}

fn hook_entry_from_command(command: &StopHookCommand) -> HookEntry {
    HookEntry {
        hook_type: Some("command".to_string()),
        command: Some(command.command.clone()),
        args: if command.args.is_empty() {
            None
        } else {
            Some(command.args.clone())
        },
        timeout: command.timeout,
        timeout_ms: command.timeout_ms,
    }
}

fn extract_stop_hooks(file: &HooksFile) -> Vec<HookEntry> {
    let mut hooks = Vec::new();
    if let Some(section) = &file.hooks {
        if let Some(entries) = section.stop_upper.clone() {
            hooks.extend(flatten_items(entries));
        }
        if let Some(entries) = section.stop_lower.clone() {
            hooks.extend(flatten_items(entries));
        }
    }
    if let Some(entries) = file.stop_upper.clone() {
        hooks.extend(flatten_items(entries));
    }
    if let Some(entries) = file.stop_lower.clone() {
        hooks.extend(flatten_items(entries));
    }
    hooks
        .into_iter()
        .filter(|hook| match hook.hook_type.as_deref() {
            Some(t) => t.eq_ignore_ascii_case("command"),
            None => true,
        })
        .filter(|hook| hook.command.is_some())
        .collect()
}

fn flatten_items(items: Vec<StopHookItem>) -> Vec<HookEntry> {
    let mut out = Vec::new();
    for item in items {
        match item {
            StopHookItem::Entry(entry) => out.push(entry),
            StopHookItem::Group(group) => out.extend(group.hooks),
        }
    }
    out
}

fn find_hooks_file(cwd: &Path) -> Option<PathBuf> {
    for ancestor in cwd.ancestors() {
        let candidate = ancestor
            .join(DOT_CODEX_DIR)
            .join(HOOKS_DIR)
            .join(HOOKS_FILE);
        if candidate.exists() {
            return Some(candidate);
        }
        let fallback = ancestor.join(DOT_CODEX_DIR).join(HOOKS_FILE);
        if fallback.exists() {
            return Some(fallback);
        }
    }
    None
}

async fn run_hook(
    ctx: &StopHookContext,
    hook: &HookEntry,
    payload: &[u8],
) -> Result<StopHookRunOutcome, StopHookError> {
    let Some(command) = hook.command.as_ref() else {
        return Ok(StopHookRunOutcome {
            status: StopHookEventStatus::InvalidOutput,
            output: None,
        });
    };

    let timeout = hook
        .timeout_ms
        .map(Duration::from_millis)
        .or_else(|| hook.timeout.map(Duration::from_secs));
    let timeout = timeout.unwrap_or_else(|| Duration::from_secs(DEFAULT_HOOK_TIMEOUT_SECS));

    let mut cmd = Command::new(command);
    if let Some(args) = hook.args.as_ref() {
        cmd.args(args);
    }
    let mut env = BTreeMap::new();
    env.insert("CODEX_HOOK_EVENT".to_string(), "stop".to_string());
    env.insert("CODEX_CWD".to_string(), ctx.cwd.display().to_string());
    env.insert(
        "CODEX_CONVERSATION_ID".to_string(),
        ctx.conversation_id.clone(),
    );
    env.insert("CODEX_TURN_ID".to_string(), ctx.turn_id.clone());
    if let Some(path) = ctx.rollout_path.as_ref() {
        env.insert("CODEX_ROLLOUT_PATH".to_string(), path.display().to_string());
    }
    cmd.envs(env);
    cmd.current_dir(&ctx.cwd);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(payload).await?;
    }

    let output = match time::timeout(timeout, child.wait_with_output()).await {
        Ok(result) => result?,
        Err(_) => {
            warn!("Stop hook timed out: {}", hook.display());
            return Ok(StopHookRunOutcome {
                status: StopHookEventStatus::Timeout,
                output: None,
            });
        }
    };

    if !output.status.success() {
        warn!(
            "Stop hook exited with status {:?}: {}",
            output.status.code(),
            hook.display()
        );
        return Ok(StopHookRunOutcome {
            status: StopHookEventStatus::ExitFailure,
            output: None,
        });
    }

    if output.stdout.is_empty() {
        return Ok(StopHookRunOutcome {
            status: StopHookEventStatus::NoOutput,
            output: None,
        });
    }

    let parsed: HookOutput = match serde_json::from_slice(&output.stdout) {
        Ok(parsed) => parsed,
        Err(err) => {
            warn!("Stop hook output invalid JSON: {err}");
            return Ok(StopHookRunOutcome {
                status: StopHookEventStatus::InvalidOutput,
                output: None,
            });
        }
    };
    Ok(StopHookRunOutcome {
        status: StopHookEventStatus::Ok,
        output: Some(parsed),
    })
}

fn combine_system_messages(messages: Vec<String>) -> Option<String> {
    if messages.is_empty() {
        None
    } else {
        Some(messages.join("\n"))
    }
}

impl HookEntry {
    fn display(&self) -> String {
        match (&self.command, &self.args) {
            (Some(cmd), Some(args)) if !args.is_empty() => {
                format!("{cmd} {}", args.join(" "))
            }
            (Some(cmd), _) => cmd.clone(),
            _ => "<unknown hook>".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn parses_stop_hooks_from_groups_and_top_level() {
        let payload = json!({
            "hooks": {
                "Stop": [
                    { "type": "command", "command": "one" },
                    { "hooks": [ { "command": "two" }, { "command": "three" } ] }
                ]
            },
            "Stop": [
                { "command": "four" }
            ]
        });
        let hooks_file: HooksFile = serde_json::from_value(payload).expect("deserialize hooks");
        let hooks = extract_stop_hooks(&hooks_file);
        let commands: Vec<String> = hooks.into_iter().filter_map(|hook| hook.command).collect();
        assert_eq!(commands, vec!["one", "two", "three", "four"]);
    }

    #[test]
    fn stop_hooks_config_orders_and_filters_sources() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let file_path = temp.path().join("hooks.json");
        let file = AbsolutePathBuf::from_absolute_path(&file_path)?;

        let mut sources = BTreeMap::new();
        sources.insert(
            "disabled".to_string(),
            StopHookSourceToml {
                enabled: Some(false),
                order: Some(1),
                command: Some("skip".to_string()),
                args: None,
                timeout: None,
                timeout_ms: None,
                file: None,
                hook_type: None,
            },
        );
        sources.insert(
            "file".to_string(),
            StopHookSourceToml {
                enabled: None,
                order: Some(0),
                command: None,
                args: None,
                timeout: None,
                timeout_ms: None,
                file: Some(file.clone()),
                hook_type: None,
            },
        );
        sources.insert(
            "later".to_string(),
            StopHookSourceToml {
                enabled: None,
                order: Some(2),
                command: Some("later".to_string()),
                args: Some(vec!["--ok".to_string()]),
                timeout: None,
                timeout_ms: Some(500),
                file: None,
                hook_type: None,
            },
        );
        sources.insert(
            "early".to_string(),
            StopHookSourceToml {
                enabled: None,
                order: Some(1),
                command: Some("early".to_string()),
                args: None,
                timeout: Some(3),
                timeout_ms: None,
                file: None,
                hook_type: Some("command".to_string()),
            },
        );

        let toml = StopHooksToml {
            include_project_hooks: Some(false),
            sources,
        };

        let config = StopHooksConfig::from_toml(Some(toml));
        assert_eq!(config.include_project_hooks, false);
        assert_eq!(
            config.sources,
            vec![
                StopHookSource::HooksFile(file.into()),
                StopHookSource::Command(StopHookCommand {
                    command: "early".to_string(),
                    args: Vec::new(),
                    timeout: Some(3),
                    timeout_ms: None,
                }),
                StopHookSource::Command(StopHookCommand {
                    command: "later".to_string(),
                    args: vec!["--ok".to_string()],
                    timeout: None,
                    timeout_ms: Some(500),
                }),
            ]
        );

        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stop_hook_blocks_with_reason() {
        use std::os::unix::fs::PermissionsExt;

        let temp = TempDir::new().expect("temp dir");
        let cwd = temp.path();
        let hooks_dir = cwd.join(DOT_CODEX_DIR).join(HOOKS_DIR);
        fs::create_dir_all(&hooks_dir).expect("create hooks dir");

        let script_path = cwd.join("stop_hook.sh");
        fs::write(
            &script_path,
            r#"#!/bin/sh
echo '{"decision":"block","reason":"repeat","systemMessage":"loop"}'
"#,
        )
        .expect("write hook script");
        let mut perms = fs::metadata(&script_path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).expect("chmod");

        let hooks_json = json!({
            "hooks": {
                "Stop": [
                    { "type": "command", "command": script_path.to_string_lossy() }
                ]
            }
        });
        let hooks_path = hooks_dir.join(HOOKS_FILE);
        fs::write(&hooks_path, serde_json::to_string(&hooks_json).unwrap())
            .expect("write hooks.json");

        let ctx = StopHookContext {
            cwd: cwd.to_path_buf(),
            conversation_id: "conv".to_string(),
            turn_id: "turn".to_string(),
            rollout_path: None,
            input_messages: vec!["hi".to_string()],
            last_agent_message: Some("done".to_string()),
            stop_hooks: StopHooksConfig::default(),
        };

        let decision = evaluate_stop_hooks(ctx, None).await.expect("hook run");
        assert_eq!(
            decision,
            StopHookDecision::Block {
                reason: "repeat".to_string(),
                system_message: Some("loop".to_string()),
            }
        );
    }
}
