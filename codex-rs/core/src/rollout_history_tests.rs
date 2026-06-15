use super::is_user_turn_boundary;
use codex_protocol::AgentPath;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::InterAgentCommunication;

fn message(role: &str, content: ContentItem) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: role.to_string(),
        content: vec![content],
        phase: None,
    }
}

#[test]
fn classifies_user_directed_turn_boundaries() {
    let user_message = message(
        "user",
        ContentItem::InputText {
            text: "hello".to_string(),
        },
    );
    let agent_message = ResponseItem::AgentMessage {
        author: "/root".to_string(),
        recipient: "/root/worker".to_string(),
        content: Vec::new(),
    };
    let instruction = InterAgentCommunication::new(
        AgentPath::root(),
        AgentPath::root().join("worker").expect("valid agent path"),
        Vec::new(),
        "continue".to_string(),
        /*trigger_turn*/ true,
    );
    let assistant_instruction = message(
        "assistant",
        ContentItem::OutputText {
            text: serde_json::to_string(&instruction).expect("instruction should serialize"),
        },
    );

    assert!(is_user_turn_boundary(&user_message));
    assert!(is_user_turn_boundary(&agent_message));
    assert!(is_user_turn_boundary(&assistant_instruction));
}

#[test]
fn excludes_context_and_non_input_items() {
    let context_message = message(
        "user",
        ContentItem::InputText {
            text: "<environment_context><cwd>/tmp</cwd></environment_context>".to_string(),
        },
    );
    let assistant_message = message(
        "assistant",
        ContentItem::OutputText {
            text: "done".to_string(),
        },
    );

    assert!(!is_user_turn_boundary(&context_message));
    assert!(!is_user_turn_boundary(&assistant_message));
    assert!(!is_user_turn_boundary(&ResponseItem::Other));
}
