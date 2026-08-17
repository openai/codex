use pretty_assertions::assert_eq;

use super::*;

fn query(text: &str) -> ShadowQuery {
    ShadowQuery {
        text: text.to_string(),
        truncated: false,
    }
}

fn begin(context: &ShadowTaskContext, turn_id: &str, text: &str) -> TaskContextSnapshot {
    context.begin_turn(turn_id, &query(text), &[])
}

#[test]
fn prior_requests_survive_continuations_and_keep_short_substantive_input() {
    let context = ShadowTaskContext::default();
    begin(&context, "1", "Fix lint errors.");
    assert_eq!(
        query("continue\nFix lint errors."),
        begin(&context, "2", "continue").query
    );
    begin(&context, "3", "OK!!!");
    begin(&context, "4", "deploy");
    begin(&context, "5", "修复测试");
    assert_eq!(
        query("continue\n修复测试\ndeploy"),
        begin(&context, "6", "continue").query
    );
    assert_eq!(
        query("deploy\n修复测试"),
        begin(&context, "7", "deploy").query
    );
    assert!(is_substantive(
        "go",
        &[UserInput::Mention {
            name: "go".to_string(),
            path: "skill://go".to_string(),
        }]
    ));
}

#[test]
fn current_turn_observations_are_pending_and_late_observations_are_ignored() {
    let context = ShadowTaskContext::default();
    begin(&context, "1", "continue");
    context.record("1", "a".to_string());
    assert_eq!(
        Vec::<String>::new(),
        begin(&context, "1", "continue").recent_skills
    );
    assert_eq!(vec!["a"], begin(&context, "2", "continue").recent_skills);
    context.record("1", "late".to_string());
    context.record("2", "a".to_string());
    context.record("2", "b".to_string());
    assert_eq!(vec!["a"], begin(&context, "2", "continue").recent_skills);
    assert_eq!(
        TaskContextSnapshot {
            query: query("continue"),
            recent_skills: vec!["b".to_string(), "a".to_string()],
        },
        begin(&context, "3", "continue")
    );
    assert_eq!(
        TaskContextSnapshot {
            query: query("continue"),
            recent_skills: Vec::new(),
        },
        begin(&ShadowTaskContext::default(), "3", "continue")
    );
}

#[test]
fn history_and_augmented_query_are_bounded_at_utf8_boundaries() {
    let context = ShadowTaskContext::default();
    let first = "界".repeat(MAX_REQUEST_BYTES);
    let initial = begin(&context, "1", &first);
    assert_eq!(
        ShadowQuery {
            text: take_bytes_at_char_boundary(&first, MAX_QUERY_BYTES).to_string(),
            truncated: true,
        },
        initial.query
    );
    for index in 0..=MAX_RECENT_SKILLS {
        context.record("1", format!("skill-{index}"));
    }
    begin(&context, "2", &"é".repeat(MAX_REQUEST_BYTES));
    context.record("2", "skill-1".to_string());
    let expected = format!(
        "continue\n{}\n{}",
        "é".repeat(MAX_REQUEST_BYTES / 2),
        take_bytes_at_char_boundary(&first, MAX_REQUEST_BYTES)
    );
    assert_eq!(
        TaskContextSnapshot {
            query: ShadowQuery {
                text: take_bytes_at_char_boundary(&expected, MAX_QUERY_BYTES).to_string(),
                truncated: true,
            },
            recent_skills: std::iter::once("skill-1".to_string())
                .chain(
                    (2..=MAX_RECENT_SKILLS)
                        .rev()
                        .map(|index| format!("skill-{index}"))
                )
                .collect(),
        },
        begin(&context, "3", "continue")
    );
}
