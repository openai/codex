use codex_execpolicy2::Decision;
use codex_execpolicy2::PolicyParser;
use codex_execpolicy2::RuleMatch;
use codex_execpolicy2::tokenize_command;

#[test]
fn matches_default_git_status() {
    let policy = codex_execpolicy2::load_default_policy().expect("parse");
    let cmd = tokenize_command("git status").expect("tokenize");
    let eval = policy.evaluate(&cmd).expect("match");
    assert_eq!(eval.decision, Decision::Allow);
    assert_eq!(
        eval.matched_rules,
        vec![RuleMatch {
            rule_id: "git_status".to_string(),
            matched_prefix: vec!["git".to_string(), "status".to_string()],
            decision: Decision::Allow,
        }]
    );
}

#[test]
fn pattern_expands_alternatives() {
    let policy_src = r#"
prefix_rule(
    id = "npm_install",
    pattern = ["npm", ["i", "install"]],
)
    "#;
    let parser = PolicyParser::new("test.policy", policy_src);
    let policy = parser.parse().expect("parse policy");

    for cmd in ["npm i", "npm install"] {
        let tokens = tokenize_command(cmd).expect("tokenize");
        let eval = policy.evaluate(&tokens).expect("match");
        let matched_prefix = if cmd.ends_with(" i") {
            vec!["npm".to_string(), "i".to_string()]
        } else {
            vec!["npm".to_string(), "install".to_string()]
        };
        assert_eq!(
            eval.matched_rules,
            vec![RuleMatch {
                rule_id: "npm_install".to_string(),
                matched_prefix,
                decision: Decision::Allow,
            }]
        );
    }

    let no_match = tokenize_command("npmx install").expect("tokenize");
    assert!(policy.evaluate(&no_match).is_none());
}

#[test]
fn match_and_not_match_examples_are_enforced() {
    let policy_src = r#"
prefix_rule(
    id = "git_status",
    pattern = ["git", "status"],
    match = ["git status"],
    not_match = ["git reset --hard"],
)
    "#;
    let parser = PolicyParser::new("test.policy", policy_src);
    let policy = parser.parse().expect("parse policy");
    assert!(
        policy
            .evaluate(&tokenize_command("git status").expect("tokenize"))
            .is_some()
    );
    assert!(
        policy
            .evaluate(&tokenize_command("git reset --hard").expect("tokenize"))
            .is_none()
    );
}

#[test]
fn strictest_decision_wins_across_matches() {
    let policy_src = r#"
prefix_rule(
    id = "allow_git_status",
    pattern = ["git", "status"],
    decision = "allow",
)
prefix_rule(
    id = "prompt_git",
    pattern = ["git"],
    decision = "prompt",
)
prefix_rule(
    id = "forbid_git_commit",
    pattern = ["git", "commit"],
    decision = "forbidden",
)
    "#;
    let parser = PolicyParser::new("test.policy", policy_src);
    let policy = parser.parse().expect("parse policy");

    let status = tokenize_command("git status").expect("tokenize");
    let status_eval = policy.evaluate(&status).expect("match");
    assert_eq!(status_eval.decision, Decision::Prompt);
    assert_eq!(
        status_eval.matched_rules,
        vec![
            RuleMatch {
                rule_id: "allow_git_status".to_string(),
                matched_prefix: vec!["git".to_string(), "status".to_string()],
                decision: Decision::Allow,
            },
            RuleMatch {
                rule_id: "prompt_git".to_string(),
                matched_prefix: vec!["git".to_string()],
                decision: Decision::Prompt,
            }
        ]
    );

    let commit = tokenize_command("git commit -m hi").expect("tokenize");
    let commit_eval = policy.evaluate(&commit).expect("match");
    assert_eq!(commit_eval.decision, Decision::Forbidden);
    assert_eq!(
        commit_eval.matched_rules,
        vec![
            RuleMatch {
                rule_id: "prompt_git".to_string(),
                matched_prefix: vec!["git".to_string()],
                decision: Decision::Prompt,
            },
            RuleMatch {
                rule_id: "forbid_git_commit".to_string(),
                matched_prefix: vec!["git".to_string(), "commit".to_string()],
                decision: Decision::Forbidden,
            }
        ]
    );
}

#[test]
fn unnamed_rule_uses_source_as_name() {
    let policy_src = r#"
prefix_rule(
    id = "unnamed_rule",
    pattern = ["echo"],
)
    "#;
    let parser = PolicyParser::new("test.policy", policy_src);
    let policy = parser.parse().expect("parse policy");
    let eval = policy
        .evaluate(&tokenize_command("echo hi").expect("tokenize"))
        .expect("match");
    assert_eq!(
        eval.matched_rules,
        vec![RuleMatch {
            rule_id: "unnamed_rule".to_string(),
            matched_prefix: vec!["echo".to_string()],
            decision: Decision::Allow,
        }]
    );
}
