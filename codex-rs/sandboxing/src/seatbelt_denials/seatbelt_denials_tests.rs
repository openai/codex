use super::*;
use pretty_assertions::assert_eq;

fn logged_denial(pid: i32, name: String) -> LoggedDenial {
    let chars = name.chars().count();
    LoggedDenial {
        pid,
        denial: SandboxDenial {
            name,
            capability: String::new(),
        },
        chars,
    }
}

#[test]
fn collected_logs_are_capped_at_one_thousand_characters() {
    let mut logged_denials = VecDeque::new();
    let mut collected_chars = 0;
    let old_denial = logged_denial(/*pid*/ 1, "a".repeat(600));
    let recent_denial = logged_denial(/*pid*/ 2, "é".repeat(500));
    let tracked_pids = HashSet::from([1, 2]);

    push_logged_denial(
        &mut logged_denials,
        &mut collected_chars,
        old_denial,
        Some(MAX_COLLECTED_LOG_CHARS),
        &tracked_pids,
    );
    push_logged_denial(
        &mut logged_denials,
        &mut collected_chars,
        recent_denial.clone(),
        Some(MAX_COLLECTED_LOG_CHARS),
        &tracked_pids,
    );

    assert_eq!(logged_denials, VecDeque::from([recent_denial]));
    assert_eq!(collected_chars, 500);
}

#[test]
fn tracked_denials_outlive_unrelated_noise_after_descendant_discovery() {
    let mut logged_denials = VecDeque::new();
    let mut collected_chars = 0;
    let root_denial = logged_denial(/*pid*/ 1, "a".repeat(400));
    let descendant_denial = logged_denial(/*pid*/ 2, "b".repeat(400));
    let unrelated_denial = logged_denial(/*pid*/ 3, "c".repeat(300));
    let mut tracked_pids = HashSet::from([1]);

    push_logged_denial(
        &mut logged_denials,
        &mut collected_chars,
        root_denial.clone(),
        Some(MAX_COLLECTED_LOG_CHARS),
        &tracked_pids,
    );
    push_logged_denial(
        &mut logged_denials,
        &mut collected_chars,
        descendant_denial.clone(),
        Some(MAX_COLLECTED_LOG_CHARS),
        &tracked_pids,
    );
    tracked_pids.insert(2);
    push_logged_denial(
        &mut logged_denials,
        &mut collected_chars,
        unrelated_denial,
        Some(MAX_COLLECTED_LOG_CHARS),
        &tracked_pids,
    );

    assert_eq!(
        logged_denials,
        VecDeque::from([root_denial, descendant_denial])
    );
    assert_eq!(collected_chars, 800);
}

#[test]
fn parses_denial_from_large_log_record() {
    let line = serde_json::to_vec(&serde_json::json!({
        "eventMessage": "Sandbox: touch(1234) deny(1) file-write-create /private/tmp/nope",
        "metadata": "x".repeat(2_000),
    }))
    .expect("valid log record");

    assert_eq!(
        parse_log_line(&line),
        Some(LoggedDenial {
            pid: 1234,
            denial: SandboxDenial {
                name: "touch".to_string(),
                capability: "file-write-create /private/tmp/nope".to_string(),
            },
            chars: "touch".chars().count() + "file-write-create /private/tmp/nope".chars().count(),
        })
    );
}
