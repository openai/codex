use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use serde_json::Value;

pub fn parse_persisted_session_source(value: &str) -> Option<SessionSource> {
    let parsed = serde_json::from_str(value)
        .ok()
        .or_else(|| serde_json::from_value(Value::String(value.to_string())).ok());
    match parsed {
        Some(SessionSource::Unknown) => {
            parse_legacy_thread_spawn_source(value).or(Some(SessionSource::Unknown))
        }
        Some(source) => Some(source),
        None => parse_legacy_thread_spawn_source(value),
    }
}

pub fn persisted_session_source_parent_thread_id(value: &str) -> Option<ThreadId> {
    parse_persisted_session_source(value)?.parent_thread_id()
}

fn parse_legacy_thread_spawn_source(value: &str) -> Option<SessionSource> {
    let legacy = value
        .strip_prefix("subagent_thread_spawn_")
        .or_else(|| value.strip_prefix("thread_spawn_"))?;
    let (parent_thread_id, depth) = legacy.rsplit_once("_d")?;
    let parent_thread_id = ThreadId::from_string(parent_thread_id).ok()?;
    let depth = depth.parse::<i32>().ok()?;
    Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
        parent_thread_id,
        depth,
        agent_path: None,
        agent_nickname: None,
        agent_role: None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_current_persisted_subagent_thread_spawn_encoding() {
        let parent_thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000123").expect("thread id");
        let source = SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id,
            depth: 1,
            agent_path: None,
            agent_nickname: None,
            agent_role: None,
        });
        let persisted = serde_json::to_string(&source).expect("serialize session source");

        assert_eq!(parse_persisted_session_source(&persisted), Some(source));
        assert_eq!(
            persisted_session_source_parent_thread_id(&persisted),
            Some(parent_thread_id)
        );
    }

    #[test]
    fn parses_legacy_display_thread_spawn_encoding() {
        let parent_thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000123").expect("thread id");
        let legacy = format!("subagent_thread_spawn_{parent_thread_id}_d1");

        assert_eq!(
            parse_persisted_session_source(&legacy),
            Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth: 1,
                agent_path: None,
                agent_nickname: None,
                agent_role: None,
            }))
        );
        assert_eq!(
            persisted_session_source_parent_thread_id(&legacy),
            Some(parent_thread_id)
        );
    }

    #[test]
    fn parses_builtin_string_variants_written_without_json_quotes() {
        assert_eq!(
            parse_persisted_session_source("cli"),
            Some(SessionSource::Cli)
        );
        assert_eq!(
            parse_persisted_session_source("vscode"),
            Some(SessionSource::VSCode)
        );
    }
}
