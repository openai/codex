use super::*;
use pretty_assertions::assert_eq;

#[test]
fn oversized_wait_preserves_every_target_and_marks_shortened_answers() {
    let answer = "\"\\\n🦀".repeat(10_000);
    let polls = (0..8).map(|i| json!({
        "schemaVersion": 1,
        "thread": {"id": format!("thread-{i}"), "status": {"type": "idle"}},
        "cursor": format!("cursor-{i}"),
        "changed": true,
        "latestTurn": {"id": format!("turn-{i}"), "status": "failed", "error": {"message": "Execution failed"}},
        "latestAssistantMessage": {"id": format!("answer-{i}"), "text": answer, "phase": "final_answer"}
    })).collect::<Vec<_>>();
    let input =
        json!({"timedOut": false, "wake": {"threadId": "thread-0"}, "polls": polls, "errors": []});
    let text = serialize(input.clone()).unwrap();
    assert!(text.len() <= MAX_RESPONSE_BYTES);
    let output: Value = serde_json::from_str(&text).unwrap();
    let mut expected = input;
    expected["truncated"] = json!(true);
    for (index, poll) in expected["polls"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        poll["truncated"] = json!(true);
        let retained = output["polls"][index]["latestAssistantMessage"]["text"]
            .as_str()
            .unwrap();
        assert!(!retained.is_empty());
        assert!(answer.starts_with(retained));
        poll["latestAssistantMessage"]["text"] = json!(retained);
        poll["latestAssistantMessage"]["truncated"] = json!(true);
    }
    assert_eq!(output, expected);
}

#[test]
fn oversized_list_preserves_entries_and_pagination() {
    let entries = (0..50)
        .map(|i| {
            json!({
                "id": format!("thread-{i}"), "kind": "codex", "title": "Title".repeat(400),
                "summary": "Summary".repeat(100), "status": "idle", "cwd": "/large/path".repeat(100)
            })
        })
        .collect::<Vec<_>>();
    let text = serialize(json!({"threads": entries, "nextCursor": "next-page"})).unwrap();
    assert!(text.len() <= MAX_RESPONSE_BYTES);
    let output: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(output["nextCursor"], "next-page");
    assert_eq!(
        output["threads"]
            .as_array()
            .unwrap()
            .iter()
            .map(|thread| thread["id"].clone())
            .collect::<Vec<_>>(),
        (0..50)
            .map(|i| json!(format!("thread-{i}")))
            .collect::<Vec<_>>()
    );
}
