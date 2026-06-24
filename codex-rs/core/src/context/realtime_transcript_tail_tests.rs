use super::*;
use pretty_assertions::assert_eq;

#[test]
fn renders_tail_as_hidden_realtime_delegation_context() {
    assert_eq!(
        RealtimeTranscriptTail::new("user: ship <this> & that").render(),
        "<realtime_delegation>\n  <input>The user just ended their realtime session. Here is the remaining handoff/transcript tail. You probably do not have to do anything; acknowledge the handoff unless the transcript itself asks for something.</input>\n  <transcript_delta>user: ship &lt;this&gt; &amp; that</transcript_delta>\n</realtime_delegation>"
    );
}
