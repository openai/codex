use super::ChannelParser;
use super::message_phase;
use codex_protocol::models::MessagePhase;
use pretty_assertions::assert_eq;

#[test]
fn maps_bem_channels_to_realtime_phases() {
    for (channel, expected) in [
        ("analysis", MessagePhase::Commentary),
        ("commentary", MessagePhase::Commentary),
        ("final", MessagePhase::FinalAnswer),
    ] {
        assert_eq!(
            message_phase(&format!(
                "<|start|>assistant<|channel|>{channel}<|message|>text<|end|>"
            )),
            Some(expected)
        );
    }
}

#[test]
fn buffers_streamed_text_until_the_bem_channel_is_complete() {
    let mut parser = ChannelParser::default();

    assert_eq!(parser.push("<|start|>assistant<|channel|>com"), None);
    assert_eq!(
        parser.push("mentary<|message|>progress"),
        Some("<|start|>assistant<|channel|>commentary<|message|>progress".to_string())
    );
    assert_eq!(parser.phase(), Some(MessagePhase::Commentary));
    assert_eq!(parser.push("<|end|>"), Some("<|end|>".to_string()));
}

#[test]
fn preserves_unrecognized_output_when_the_stream_finishes() {
    let mut parser = ChannelParser::default();

    assert_eq!(parser.push("plain output"), None);
    assert_eq!(parser.finish(), "plain output");
    assert_eq!(parser.phase(), None);
}
