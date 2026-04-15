use std::path::Path;
use std::path::PathBuf;

use codex_protocol::protocol::HookEventName;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HooksFile {
    #[serde(default)]
    pub hooks: HookEventsToml,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HookEventsToml {
    #[serde(rename = "PreToolUse", default)]
    pub pre_tool_use: Vec<MatcherGroup>,
    #[serde(rename = "PostToolUse", default)]
    pub post_tool_use: Vec<MatcherGroup>,
    #[serde(rename = "SessionStart", default)]
    pub session_start: Vec<MatcherGroup>,
    #[serde(rename = "UserPromptSubmit", default)]
    pub user_prompt_submit: Vec<MatcherGroup>,
    #[serde(rename = "Stop", default)]
    pub stop: Vec<MatcherGroup>,
}

impl HookEventsToml {
    pub fn is_empty(&self) -> bool {
        self.pre_tool_use.is_empty()
            && self.post_tool_use.is_empty()
            && self.session_start.is_empty()
            && self.user_prompt_submit.is_empty()
            && self.stop.is_empty()
    }

    pub fn handler_count(&self) -> usize {
        [
            &self.pre_tool_use,
            &self.post_tool_use,
            &self.session_start,
            &self.user_prompt_submit,
            &self.stop,
        ]
        .into_iter()
        .flatten()
        .map(|group| group.hooks.len())
        .sum()
    }

    pub fn into_matcher_groups(self) -> [(HookEventName, Vec<MatcherGroup>); 5] {
        [
            (HookEventName::PreToolUse, self.pre_tool_use),
            (HookEventName::PostToolUse, self.post_tool_use),
            (HookEventName::SessionStart, self.session_start),
            (HookEventName::UserPromptSubmit, self.user_prompt_submit),
            (HookEventName::Stop, self.stop),
        ]
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MatcherGroup {
    #[serde(default)]
    pub matcher: Option<String>,
    #[serde(default)]
    pub hooks: Vec<HookHandlerConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type")]
pub enum HookHandlerConfig {
    #[serde(rename = "command")]
    Command {
        command: String,
        #[serde(default, rename = "timeout", alias = "timeoutSec")]
        timeout_sec: Option<u64>,
        #[serde(default)]
        r#async: bool,
        #[serde(default, rename = "statusMessage")]
        status_message: Option<String>,
    },
    #[serde(rename = "prompt")]
    Prompt {},
    #[serde(rename = "agent")]
    Agent {},
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedHooksRequirementsToml {
    pub managed_dir: Option<PathBuf>,
    pub windows_managed_dir: Option<PathBuf>,
    #[serde(flatten)]
    pub hooks: HookEventsToml,
}

impl ManagedHooksRequirementsToml {
    pub fn is_empty(&self) -> bool {
        self.managed_dir.is_none() && self.windows_managed_dir.is_none() && self.hooks.is_empty()
    }

    pub fn handler_count(&self) -> usize {
        self.hooks.handler_count()
    }

    pub fn managed_dir_for_current_platform(&self) -> Option<&Path> {
        #[cfg(windows)]
        {
            self.windows_managed_dir.as_deref()
        }

        #[cfg(not(windows))]
        {
            self.managed_dir.as_deref()
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::HookEventsToml;
    use super::HooksFile;
    use super::ManagedHooksRequirementsToml;

    #[test]
    fn hooks_file_deserializes_existing_json_shape() {
        let parsed: HooksFile = serde_json::from_str(
            r#"{
              "hooks": {
                "PreToolUse": [
                  {
                    "matcher": "^Bash$",
                    "hooks": [
                      {
                        "type": "command",
                        "command": "python3 /tmp/pre.py",
                        "timeoutSec": 10,
                        "statusMessage": "checking"
                      }
                    ]
                  }
                ]
              }
            }"#,
        )
        .expect("hooks.json should deserialize");

        assert_eq!(parsed.hooks.pre_tool_use.len(), 1);
        assert_eq!(parsed.hooks.pre_tool_use[0].hooks.len(), 1);
    }

    #[test]
    fn hook_events_deserialize_from_toml_arrays_of_tables() {
        let parsed: HookEventsToml = toml::from_str(
            r#"
                [[PreToolUse]]
                matcher = "^Bash$"

                [[PreToolUse.hooks]]
                type = "command"
                command = "python3 /tmp/pre.py"
                timeoutSec = 10
                statusMessage = "checking"
            "#,
        )
        .expect("hook events TOML should deserialize");

        assert_eq!(parsed.pre_tool_use.len(), 1);
        assert_eq!(parsed.pre_tool_use[0].hooks.len(), 1);
    }

    #[test]
    fn managed_hooks_requirements_flatten_hook_events() {
        let parsed: ManagedHooksRequirementsToml = toml::from_str(
            r#"
                managed_dir = "/enterprise/place"

                [[PreToolUse]]
                matcher = "^Bash$"

                [[PreToolUse.hooks]]
                type = "command"
                command = "python3 /enterprise/place/pre.py"
            "#,
        )
        .expect("requirements hooks TOML should deserialize");

        assert_eq!(
            parsed.managed_dir.as_deref(),
            Some(std::path::Path::new("/enterprise/place"))
        );
        assert_eq!(parsed.hooks.pre_tool_use.len(), 1);
    }
}
