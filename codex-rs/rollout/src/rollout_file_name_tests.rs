use codex_protocol::ThreadId;
use pretty_assertions::assert_eq;
use time::macros::datetime;

use super::RolloutFileName;

#[test]
fn canonical_rollout_file_names_round_trip() {
    let timestamp = datetime!(2026-08-11 18:42:07 UTC);
    let thread_id =
        ThreadId::from_string("019ff1a2-b3c4-7d5e-8f60-112233445566").expect("valid thread id");
    let rollout_id =
        ThreadId::from_string("019ff1a2-b3c4-7d5e-8f60-667788990011").expect("valid rollout id");

    for (file_name, expected) in [
        (
            RolloutFileName::new(timestamp, thread_id, thread_id),
            "rollout-2026-08-11T18-42-07-019ff1a2-b3c4-7d5e-8f60-112233445566.jsonl",
        ),
        (
            RolloutFileName::new(timestamp, thread_id, rollout_id),
            "rollout-2026-08-11T18-42-07-019ff1a2-b3c4-7d5e-8f60-112233445566_019ff1a2-b3c4-7d5e-8f60-667788990011.jsonl",
        ),
    ] {
        let rendered = file_name.render().expect("timestamp should format");
        assert_eq!(rendered, expected);
        assert_eq!(RolloutFileName::parse(&rendered), Some(file_name));
    }
}
