use serde::Deserialize;
use serde::Serialize;

use crate::codex::TurnContext;
use crate::model_visible_context::ContextualUserContextRole;
use crate::model_visible_context::ModelVisibleContextFragment;
use crate::model_visible_context::TurnContextDiffFragment;
use crate::model_visible_context::TurnContextDiffParams;
use codex_protocol::protocol::TurnContextItem;

use crate::model_visible_context::AGENTS_MD_CLOSE_TAG_PREFIX;
use crate::model_visible_context::AGENTS_MD_OPEN_TAG_PREFIX;
use crate::model_visible_context::PLUGINS_FRAGMENT_MARKERS;
use crate::model_visible_context::SKILL_FRAGMENT_MARKERS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename = "user_instructions", rename_all = "snake_case")]
pub(crate) struct AgentsMdInstructions {
    pub directory: String,
    pub text: String,
}

impl ModelVisibleContextFragment for AgentsMdInstructions {
    type Role = ContextualUserContextRole;

    fn render_text(&self) -> String {
        format!(
            "{prefix}{directory}>\n{contents}\n{suffix}{directory}>",
            prefix = AGENTS_MD_OPEN_TAG_PREFIX,
            directory = self.directory,
            contents = self.text,
            suffix = AGENTS_MD_CLOSE_TAG_PREFIX,
        )
    }
}

impl TurnContextDiffFragment for AgentsMdInstructions {
    fn from_turn_context(
        turn_context: &TurnContext,
        _params: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        let text = turn_context.user_instructions.as_ref()?.clone();
        Some(Self {
            directory: turn_context.cwd.to_string_lossy().into_owned(),
            text,
        })
    }

    fn diff_from_turn_context_item(
        previous: &TurnContextItem,
        turn_context: &TurnContext,
        params: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        let current = Self::from_turn_context(turn_context, params)?;
        let previous_directory = previous.cwd.to_string_lossy().into_owned();
        if previous.user_instructions.as_deref() == Some(current.text.as_str())
            && previous_directory == current.directory
        {
            return None;
        }

        Some(current)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename = "skill_instructions", rename_all = "snake_case")]
pub(crate) struct SkillInstructions {
    pub name: String,
    pub path: String,
    pub contents: String,
}

impl ModelVisibleContextFragment for SkillInstructions {
    type Role = ContextualUserContextRole;

    fn render_text(&self) -> String {
        SKILL_FRAGMENT_MARKERS.wrap_body(format!(
            "<name>{}</name>\n<path>{}</path>\n{}",
            self.name, self.path, self.contents
        ))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename = "plugin_instructions", rename_all = "snake_case")]
pub(crate) struct PluginInstructions {
    pub text: String,
}

impl ModelVisibleContextFragment for PluginInstructions {
    type Role = ContextualUserContextRole;

    fn render_text(&self) -> String {
        PLUGINS_FRAGMENT_MARKERS.wrap_body(self.text.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseItem;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_user_instructions() {
        let user_instructions = AgentsMdInstructions {
            directory: "test_directory".to_string(),
            text: "test_text".to_string(),
        };
        let response_item = user_instructions.into_message();

        let ResponseItem::Message { role, content, .. } = response_item else {
            panic!("expected ResponseItem::Message");
        };

        assert_eq!(role, "user");

        let [ContentItem::InputText { text }] = content.as_slice() else {
            panic!("expected one InputText content item");
        };

        assert_eq!(
            text,
            "<AGENTS.md INSTRUCTIONS FOR test_directory>\ntest_text\n</AGENTS.md INSTRUCTIONS FOR test_directory>",
        );
    }

    #[test]
    fn test_is_user_instructions() {
        assert!(crate::model_visible_context::is_contextual_user_fragment(
            &ContentItem::InputText {
                text: "<AGENTS.md INSTRUCTIONS FOR test_directory>\ntest_text\n</AGENTS.md INSTRUCTIONS FOR test_directory>"
                    .to_string(),
            }
        ));
    }

    #[test]
    fn test_skill_instructions() {
        let skill_instructions = SkillInstructions {
            name: "demo-skill".to_string(),
            path: "skills/demo/SKILL.md".to_string(),
            contents: "body".to_string(),
        };
        let response_item = skill_instructions.into_message();

        let ResponseItem::Message { role, content, .. } = response_item else {
            panic!("expected ResponseItem::Message");
        };

        assert_eq!(role, "user");

        let [ContentItem::InputText { text }] = content.as_slice() else {
            panic!("expected one InputText content item");
        };

        assert_eq!(
            text,
            "<skill>\n<name>demo-skill</name>\n<path>skills/demo/SKILL.md</path>\nbody\n</skill>",
        );
    }

    #[test]
    fn test_is_skill_instructions() {
        assert!(SKILL_FRAGMENT_MARKERS.matches_text(
            "<skill>\n<name>demo-skill</name>\n<path>skills/demo/SKILL.md</path>\nbody\n</skill>"
        ));
        assert!(!SKILL_FRAGMENT_MARKERS.matches_text("regular text"));
    }

    #[test]
    fn test_plugin_instructions() {
        let plugin_instructions = PluginInstructions {
            text: "## Plugins\n- `sample`".to_string(),
        };
        let response_item = plugin_instructions.into_message();

        let ResponseItem::Message { role, content, .. } = response_item else {
            panic!("expected ResponseItem::Message");
        };

        assert_eq!(role, "user");

        let [ContentItem::InputText { text }] = content.as_slice() else {
            panic!("expected one InputText content item");
        };

        assert_eq!(text, "<plugins>\n## Plugins\n- `sample`\n</plugins>");
    }
}
