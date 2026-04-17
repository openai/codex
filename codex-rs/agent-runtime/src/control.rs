use crate::registry::AgentMetadata;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::user_input::UserInput;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpawnAgentForkMode {
    FullHistory,
    LastNTurns(usize),
}

#[derive(Clone, Debug, Default)]
pub struct SpawnAgentOptions {
    pub fork_parent_spawn_call_id: Option<String>,
    pub fork_mode: Option<SpawnAgentForkMode>,
}

#[derive(Clone, Debug)]
pub struct LiveAgent {
    pub thread_id: ThreadId,
    pub metadata: AgentMetadata,
    pub status: AgentStatus,
}

pub fn keep_forked_rollout_item(item: &RolloutItem) -> bool {
    match item {
        RolloutItem::ResponseItem(ResponseItem::Message { role, phase, .. }) => match role.as_str()
        {
            "system" | "developer" | "user" => true,
            "assistant" => *phase == Some(MessagePhase::FinalAnswer),
            _ => false,
        },
        RolloutItem::ResponseItem(
            ResponseItem::Reasoning { .. }
            | ResponseItem::LocalShellCall { .. }
            | ResponseItem::FunctionCall { .. }
            | ResponseItem::ToolSearchCall { .. }
            | ResponseItem::FunctionCallOutput { .. }
            | ResponseItem::CustomToolCall { .. }
            | ResponseItem::CustomToolCallOutput { .. }
            | ResponseItem::ToolSearchOutput { .. }
            | ResponseItem::WebSearchCall { .. }
            | ResponseItem::ImageGenerationCall { .. }
            | ResponseItem::GhostSnapshot { .. }
            | ResponseItem::Compaction { .. }
            | ResponseItem::Other,
        ) => false,
        RolloutItem::Compacted(_)
        | RolloutItem::EventMsg(_)
        | RolloutItem::SessionMeta(_)
        | RolloutItem::TurnContext(_) => true,
    }
}

pub fn thread_spawn_parent_thread_id(session_source: &SessionSource) -> Option<ThreadId> {
    match session_source {
        SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id, ..
        }) => Some(*parent_thread_id),
        _ => None,
    }
}

pub fn thread_spawn_depth(session_source: &SessionSource) -> Option<i32> {
    match session_source {
        SessionSource::SubAgent(SubAgentSource::ThreadSpawn { depth, .. }) => Some(*depth),
        _ => None,
    }
}

pub fn agent_matches_prefix(agent_path: Option<&AgentPath>, prefix: &AgentPath) -> bool {
    if prefix.is_root() {
        return true;
    }

    agent_path.is_some_and(|agent_path| {
        agent_path == prefix
            || agent_path
                .as_str()
                .strip_prefix(prefix.as_str())
                .is_some_and(|suffix| suffix.starts_with('/'))
    })
}

pub fn render_input_preview(initial_operation: &Op) -> String {
    match initial_operation {
        Op::UserInput { items, .. } => items
            .iter()
            .map(|item| match item {
                UserInput::Text { text, .. } => text.clone(),
                UserInput::Image { .. } => "[image]".to_string(),
                UserInput::LocalImage { path } => format!("[local_image:{}]", path.display()),
                UserInput::Skill { name, path } => format!("[skill:${name}]({})", path.display()),
                UserInput::Mention { name, path } => format!("[mention:${name}]({path})"),
                _ => "[input]".to_string(),
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Op::InterAgentCommunication { communication } => communication.content.clone(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::models::ContentItem;
    use codex_protocol::protocol::InterAgentCommunication;
    use pretty_assertions::assert_eq;

    fn agent_path(path: &str) -> AgentPath {
        AgentPath::try_from(path).expect("valid agent path")
    }

    #[test]
    fn render_input_preview_summarizes_user_input_items() {
        let op = Op::UserInput {
            items: vec![
                UserInput::Text {
                    text: "hello".to_string(),
                    text_elements: Vec::new(),
                },
                UserInput::Image {
                    image_url: "data:image/png;base64,abc".to_string(),
                },
                UserInput::Mention {
                    name: "doc".to_string(),
                    path: "app://doc".to_string(),
                },
            ],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
        };

        assert_eq!(
            render_input_preview(&op),
            "hello\n[image]\n[mention:$doc](app://doc)"
        );
    }

    #[test]
    fn render_input_preview_uses_inter_agent_message_content() {
        let communication = InterAgentCommunication::new(
            AgentPath::root(),
            agent_path("/root/worker"),
            Vec::new(),
            "wake up".to_string(),
            /*trigger_turn*/ true,
        );
        let op = Op::InterAgentCommunication { communication };

        assert_eq!(render_input_preview(&op), "wake up");
    }

    #[test]
    fn agent_matches_prefix_accepts_root_exact_and_descendants() {
        let worker = agent_path("/root/worker");
        let worker_child = agent_path("/root/worker/child");
        let other = agent_path("/root/other");

        assert!(agent_matches_prefix(Some(&worker), &AgentPath::root()));
        assert!(agent_matches_prefix(Some(&worker), &worker));
        assert!(agent_matches_prefix(Some(&worker_child), &worker));
        assert!(!agent_matches_prefix(Some(&other), &worker));
        assert!(!agent_matches_prefix(/*agent_path*/ None, &worker));
    }

    #[test]
    fn thread_spawn_parent_and_depth_only_match_thread_spawn_sources() {
        let parent_thread_id = ThreadId::new();
        let session_source = SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id,
            depth: 2,
            agent_path: None,
            agent_nickname: None,
            agent_role: None,
        });

        assert_eq!(
            thread_spawn_parent_thread_id(&session_source),
            Some(parent_thread_id)
        );
        assert_eq!(thread_spawn_depth(&session_source), Some(2));
        assert_eq!(thread_spawn_parent_thread_id(&SessionSource::Cli), None);
        assert_eq!(
            thread_spawn_depth(&SessionSource::SubAgent(SubAgentSource::Review)),
            None
        );
    }

    #[test]
    fn forked_rollout_filter_keeps_only_contextual_items_and_final_assistant_messages() {
        let final_assistant_message = RolloutItem::ResponseItem(ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "done".to_string(),
            }],
            end_turn: None,
            phase: Some(MessagePhase::FinalAnswer),
        });
        let in_progress_assistant_message = RolloutItem::ResponseItem(ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "thinking".to_string(),
            }],
            end_turn: None,
            phase: None,
        });

        assert!(keep_forked_rollout_item(&final_assistant_message));
        assert!(!keep_forked_rollout_item(&in_progress_assistant_message));
        assert!(!keep_forked_rollout_item(&RolloutItem::ResponseItem(
            ResponseItem::Other
        )));
    }
}
