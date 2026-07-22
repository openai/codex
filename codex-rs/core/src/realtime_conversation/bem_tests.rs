use super::ChannelParser;
use super::message_phase;
use codex_protocol::models::MessagePhase;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::sync::Arc;

#[test]
fn maps_bem_channels_to_realtime_phases() {
    let channel_prefixes = BTreeMap::new();
    for (channel, expected) in [
        ("ANALYSIS", MessagePhase::Commentary),
        ("COMMENTARY", MessagePhase::Commentary),
        ("FINAL", MessagePhase::FinalAnswer),
    ] {
        assert_eq!(
            message_phase(&format!("[{channel}]text"), &channel_prefixes),
            Some(expected)
        );
    }
}

#[test]
fn client_prefixes_override_only_their_configured_channels() {
    let channel_prefixes = BTreeMap::from([
        (
            "analysis".to_string(),
            vec!["[THINKING]".to_string(), "[THOUGHT]".to_string()],
        ),
        (
            "commentary".to_string(),
            vec!["[PROGRESS]".to_string(), "[UPDATE]".to_string()],
        ),
    ]);

    for (text, expected) in [
        ("[THINKING]reasoning", MessagePhase::Commentary),
        ("[THOUGHT]reasoning", MessagePhase::Commentary),
        ("[PROGRESS]working", MessagePhase::Commentary),
        ("[UPDATE]working", MessagePhase::Commentary),
        ("[FINAL]finished", MessagePhase::FinalAnswer),
    ] {
        assert_eq!(message_phase(text, &channel_prefixes), Some(expected));
    }

    assert_eq!(
        message_phase("[ANALYSIS]old analysis prefix", &channel_prefixes),
        None
    );
    assert_eq!(
        message_phase("[COMMENTARY]old commentary prefix", &channel_prefixes),
        None
    );
}

#[test]
fn empty_client_prefixes_do_not_match_every_message() {
    let channel_prefixes = BTreeMap::from([("analysis".to_string(), vec![String::new()])]);

    assert_eq!(message_phase("plain output", &channel_prefixes), None);
    assert_eq!(
        message_phase("[FINAL]finished", &channel_prefixes),
        Some(MessagePhase::FinalAnswer)
    );
}

#[test]
fn buffers_streamed_text_until_the_bem_channel_is_complete() {
    let mut parser = ChannelParser::default();

    assert_eq!(parser.push("[COM"), None);
    assert_eq!(
        parser.push("MENTARY]progress"),
        Some("[COMMENTARY]progress".to_string())
    );
    assert_eq!(parser.phase(), Some(MessagePhase::Commentary));
    assert_eq!(parser.push(" update"), Some(" update".to_string()));
}

#[test]
fn buffers_a_client_prefix_until_the_streamed_header_is_complete() {
    let channel_prefixes = BTreeMap::from([(
        "commentary".to_string(),
        vec!["[PROGRESS]".to_string(), "[UPDATE]".to_string()],
    )]);
    let mut parser = ChannelParser::new(Arc::new(channel_prefixes));

    assert_eq!(parser.push("[PRO"), None);
    assert_eq!(
        parser.push("GRESS]working"),
        Some("[PROGRESS]working".to_string())
    );
    assert_eq!(parser.phase(), Some(MessagePhase::Commentary));
}

#[test]
fn preserves_unrecognized_output_when_the_stream_finishes() {
    let mut parser = ChannelParser::default();

    assert_eq!(parser.push("plain output"), None);
    assert_eq!(parser.finish(), "plain output");
    assert_eq!(parser.phase(), None);
}
