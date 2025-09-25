use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use chrono::DateTime;
use chrono::Utc;
use codex_protocol::mcp_protocol::ConversationId;
use codex_protocol::protocol::InputItem;
use futures::future::join_all;
use serde::Deserialize;
use serde::Serialize;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::warn;

#[derive(Debug, Clone)]
pub struct HookRegistry {
    inner: Arc<HookData>,
}

#[derive(Debug, Clone)]
pub struct HookSummary {
    pub event: String,
    pub commands: Vec<String>,
}

#[derive(Debug, Default, Clone)]
struct HookData {
    events: HashMap<HookEventKind, Vec<HookAction>>,
}

impl PartialEq for HookRegistry {
    fn eq(&self, other: &Self) -> bool {
        if self.inner.events.len() != other.inner.events.len() {
            return false;
        }
        self.inner.events.iter().all(|(kind, actions)| {
            let Some(other_actions) = other.inner.events.get(kind) else {
                return false;
            };
            actions
                .iter()
                .map(|action| action.command.as_str())
                .eq(other_actions.iter().map(|action| action.command.as_str()))
        })
    }
}

impl Eq for HookRegistry {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum HookEventKind {
    UserPromptSubmit,
}

impl HookEventKind {
    fn from_str(value: &str) -> Option<Self> {
        match value {
            "UserPromptSubmit" => Some(Self::UserPromptSubmit),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::UserPromptSubmit => "UserPromptSubmit",
        }
    }
}

#[derive(Debug, Clone)]
struct HookAction {
    kind: HookActionKind,
    command: String,
    state: Arc<HookActionState>,
}

#[derive(Debug, Default)]
struct HookActionState {
    last_error: Mutex<Option<String>>,
}

impl HookActionState {
    fn lock(&self) -> MutexGuard<'_, Option<String>> {
        self.last_error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HookActionKind {
    Command,
}

impl HookRegistry {
    pub fn load_from_path(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read hooks file {}", path.display()))?;
        let file: HookFile = serde_json::from_str(&contents)
            .with_context(|| format!("failed to parse hooks file {}", path.display()))?;

        let mut events: HashMap<HookEventKind, Vec<HookAction>> = HashMap::new();

        for (event_name, rules) in file.hooks.into_iter() {
            let Some(kind) = HookEventKind::from_str(&event_name) else {
                warn!("unsupported hook event type: {event_name}");
                continue;
            };

            let mut actions: Vec<HookAction> = Vec::new();
            for rule in rules {
                for action_cfg in rule.hooks {
                    match action_cfg.r#type {
                        HookActionKindConfig::Command => {
                            if action_cfg.command.trim().is_empty() {
                                continue;
                            }
                            actions
                                .push(HookAction::new(HookActionKind::Command, action_cfg.command));
                        }
                    }
                }
            }

            if !actions.is_empty() {
                events.entry(kind).or_default().extend(actions);
            }
        }

        Ok(Self {
            inner: Arc::new(HookData { events }),
        })
    }

    pub async fn trigger_user_prompt_submit(&self, payload: UserPromptSubmitPayload) -> Result<()> {
        self.dispatch(HookEventKind::UserPromptSubmit, payload)
            .await
    }

    pub fn summaries(&self) -> Vec<HookSummary> {
        let mut summaries: Vec<HookSummary> = self
            .inner
            .events
            .iter()
            .map(|(event, actions)| {
                let event = (*event).as_str().to_string();
                let commands = actions.iter().map(HookAction::display_summary).collect();
                HookSummary { event, commands }
            })
            .collect();
        summaries.sort_by(|a, b| a.event.cmp(&b.event));
        summaries
    }

    async fn dispatch(&self, event: HookEventKind, payload: impl Serialize) -> Result<()> {
        let Some(actions) = self.inner.events.get(&event) else {
            return Ok(());
        };
        if actions.is_empty() {
            return Ok(());
        }

        let payload_str = serde_json::to_string(&payload)?;
        let futures = actions
            .iter()
            .map(|action| action.execute(payload_str.clone()));

        let results = join_all(futures).await;
        let mut failures: Vec<String> = Vec::new();
        for result in results {
            if let Err(err) = result {
                let msg = format!("{err:#}");
                warn!("hook execution failed: {msg}");
                failures.push(msg);
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(anyhow!(
                "{count} hook command(s) failed: {details}",
                count = failures.len(),
                details = failures.join(", "),
            ))
        }
    }
}

#[derive(Debug, Deserialize)]
struct HookFile {
    #[serde(default)]
    hooks: HashMap<String, Vec<HookRule>>,
}

#[derive(Debug, Deserialize)]
struct HookRule {
    #[serde(default)]
    _matcher: Option<String>,
    #[serde(default)]
    hooks: Vec<HookActionConfig>,
}

#[derive(Debug, Deserialize)]
struct HookActionConfig {
    #[serde(rename = "type")]
    r#type: HookActionKindConfig,
    command: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum HookActionKindConfig {
    Command,
}

impl HookAction {
    fn new(kind: HookActionKind, command: String) -> Self {
        Self {
            kind,
            command,
            state: Arc::new(HookActionState::default()),
        }
    }

    fn display_summary(&self) -> String {
        let command = self.command_display();
        let last_error = self.state.lock();
        if let Some(error) = last_error.as_ref() {
            format!("{command} (last failure: {error})")
        } else {
            command
        }
    }

    async fn execute(&self, payload: String) -> Result<()> {
        match self.kind {
            HookActionKind::Command => {
                let result = execute_command(&self.command, payload).await;
                match result {
                    Ok(()) => {
                        *self.state.lock() = None;
                        Ok(())
                    }
                    Err(err) => {
                        *self.state.lock() = Some(format!("{err:#}"));
                        Err(err)
                    }
                }
            }
        }
    }

    fn command_display(&self) -> String {
        expand_command_for_display(&self.command)
    }
}

async fn execute_command(command: &str, payload: String) -> Result<()> {
    let mut cmd = build_shell_command(command);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    let mut child = cmd
        .spawn()
        .with_context(|| format!("failed to spawn hook command `{command}`"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(payload.as_bytes())
            .await
            .context("failed to write hook payload to stdin")?;
        // Ensure EOF so commands that read stdin complete.
        stdin
            .shutdown()
            .await
            .context("failed to close hook stdin")?;
    }

    let status = child
        .wait()
        .await
        .context("failed to wait for hook command")?;

    if !status.success() {
        return Err(anyhow!(
            "hook command `{command}` exited with status {status}"
        ));
    }

    Ok(())
}

fn expand_command_for_display(command: &str) -> String {
    const PLACEHOLDER: &str = "${CLAUDE_HOOKS_PATH:-$PWD/apps/hooks}";
    let mut expanded = command.to_string();

    if expanded.contains(PLACEHOLDER)
        && let Some(path) = resolve_hooks_placeholder_path()
    {
        let display = path.display().to_string();
        expanded = expanded.replace(PLACEHOLDER, &display);
    }

    if expanded.contains("$PWD")
        && let Ok(cwd) = env::current_dir()
    {
        let display = cwd.display().to_string();
        expanded = expanded.replace("$PWD", &display);
    }

    expanded
}

fn resolve_hooks_placeholder_path() -> Option<PathBuf> {
    if let Ok(value) = env::var("CLAUDE_HOOKS_PATH") {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            Some(path)
        } else if let Ok(cwd) = env::current_dir() {
            Some(cwd.join(path))
        } else {
            Some(path)
        }
    } else if let Ok(cwd) = env::current_dir() {
        Some(cwd.join("apps/hooks"))
    } else {
        None
    }
}

fn build_shell_command(command: &str) -> Command {
    #[cfg(target_os = "windows")]
    {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(command);
        cmd
    }

    #[cfg(not(target_os = "windows"))]
    {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command);
        cmd
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HookInputItem {
    Text { text: String },
    Image { image_url: String },
    LocalImage { path: PathBuf },
}

impl From<&InputItem> for HookInputItem {
    fn from(value: &InputItem) -> Self {
        match value {
            InputItem::Text { text } => Self::Text { text: text.clone() },
            InputItem::Image { image_url } => Self::Image {
                image_url: image_url.clone(),
            },
            InputItem::LocalImage { path } => Self::LocalImage { path: path.clone() },
            _ => Self::Text {
                text: format!("{value:?}"),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UserPromptSubmitPayload {
    pub event: &'static str,
    pub timestamp: DateTime<Utc>,
    pub conversation_id: String,
    pub cwd: PathBuf,
    pub model: String,
    pub items: Vec<HookInputItem>,
    pub prompt_text: Option<String>,
}

impl UserPromptSubmitPayload {
    pub fn new(
        conversation_id: ConversationId,
        cwd: PathBuf,
        model: String,
        items: Vec<InputItem>,
        timestamp: DateTime<Utc>,
    ) -> Self {
        let hook_items: Vec<HookInputItem> = items.iter().map(HookInputItem::from).collect();
        let prompt_text = collect_prompt_text(&items);

        Self {
            event: "UserPromptSubmit",
            timestamp,
            conversation_id: conversation_id.to_string(),
            cwd,
            model,
            items: hook_items,
            prompt_text,
        }
    }
}

fn collect_prompt_text(items: &[InputItem]) -> Option<String> {
    let mut segments: Vec<&str> = Vec::new();
    for item in items {
        if let InputItem::Text { text } = item
            && !text.trim().is_empty()
        {
            segments.push(text);
        }
    }
    if segments.is_empty() {
        None
    } else {
        Some(segments.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::protocol::InputItem;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn trigger_noops_when_event_missing() {
        let registry = HookRegistry {
            inner: Arc::new(HookData::default()),
        };
        let payload = UserPromptSubmitPayload::new(
            ConversationId::default(),
            PathBuf::from("/tmp"),
            "model".to_string(),
            Vec::new(),
            Utc::now(),
        );
        assert!(registry.trigger_user_prompt_submit(payload).await.is_ok());
    }

    #[test]
    fn collect_prompt_text_filters_blank_segments() {
        let items = vec![
            InputItem::Text {
                text: "First".to_string(),
            },
            InputItem::Text {
                text: "".to_string(),
            },
            InputItem::Text {
                text: "Second".to_string(),
            },
        ];
        let collected = collect_prompt_text(&items);
        assert_eq!(collected, Some("First\nSecond".to_string()));
    }

    #[test]
    fn load_registry_filters_unknown_events() {
        let hooks = r#"{
            "hooks": {
                "Unknown": [
                    {
                        "hooks": [
                            { "type": "command", "command": "echo nope" }
                        ]
                    }
                ],
                "UserPromptSubmit": [
                    {
                        "hooks": [
                            { "type": "command", "command": "echo ok" }
                        ]
                    }
                ]
            }
        }"#;
        let mut tmp = NamedTempFile::new().expect("tmp file");
        std::io::Write::write_all(tmp.as_file_mut(), hooks.as_bytes()).expect("write file");

        let registry = HookRegistry::load_from_path(tmp.path()).expect("load hooks");
        assert!(
            registry
                .inner
                .events
                .contains_key(&HookEventKind::UserPromptSubmit)
        );
    }
}
