use codex_execpolicy2::PolicyParser;
use codex_execpolicy2::Rule;
use expect_test::expect;

fn tokens(cmd: &[&str]) -> Vec<String> {
    cmd.iter().map(std::string::ToString::to_string).collect()
}

fn rules_to_string(rules: &[Rule]) -> String {
    format!(
        "[{}]",
        rules
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

#[test]
fn basic_match() {
    let policy_src = r#"
prefix_rule(
    pattern = ["git", "status"],
)
    "#;
    let policy = PolicyParser::new("test.policy", policy_src)
        .parse()
        .expect("parse policy");
    let cmd = tokens(&["git", "status"]);
    let evaluation = policy.evaluate(&cmd);
    expect![[r#"Match {
  decision: allow,
  matched_rules: [
    PrefixRuleMatch { matched_prefix: ["git", "status"], decision: allow },
  ]
}"#]]
    .assert_eq(&evaluation.to_string());
}

#[test]
fn only_first_token_alias_expands_to_multiple_rules() {
    let policy_src = r#"
prefix_rule(
    pattern = [["bash", "sh"], ["-c", "-l"]],
)
    "#;
    let parser = PolicyParser::new("test.policy", policy_src);
    let policy = parser.parse().expect("parse policy");

    let bash_rules = policy.rules().get_vec("bash").expect("bash rules");
    let sh_rules = policy.rules().get_vec("sh").expect("sh rules");
    expect![[r#"[prefix_rule(pattern = [bash, [-c, -l]], decision = allow)]"#]]
        .assert_eq(&rules_to_string(bash_rules));
    expect![[r#"[prefix_rule(pattern = [sh, [-c, -l]], decision = allow)]"#]]
        .assert_eq(&rules_to_string(sh_rules));

    let bash_eval = policy.evaluate(&tokens(&["bash", "-c", "echo", "hi"]));
    expect![[r#"Match {
  decision: allow,
  matched_rules: [
    PrefixRuleMatch { matched_prefix: ["bash", "-c"], decision: allow },
  ]
}"#]]
    .assert_eq(&bash_eval.to_string());

    let sh_eval = policy.evaluate(&tokens(&["sh", "-l", "echo", "hi"]));
    expect![[r#"Match {
  decision: allow,
  matched_rules: [
    PrefixRuleMatch { matched_prefix: ["sh", "-l"], decision: allow },
  ]
}"#]]
    .assert_eq(&sh_eval.to_string());
}

#[test]
fn tail_aliases_are_not_cartesian_expanded() {
    let policy_src = r#"
prefix_rule(
    pattern = ["npm", ["i", "install"], ["--legacy-peer-deps", "--no-save"]],
)
    "#;
    let parser = PolicyParser::new("test.policy", policy_src);
    let policy = parser.parse().expect("parse policy");

    let rules = policy.rules().get_vec("npm").expect("npm rules");
    expect![[r#"[prefix_rule(pattern = [npm, [i, install], [--legacy-peer-deps, --no-save]], decision = allow)]"#]]
        .assert_eq(&rules_to_string(rules));

    let npm_i = policy.evaluate(&tokens(&["npm", "i", "--legacy-peer-deps"]));
    expect![[r#"Match {
  decision: allow,
  matched_rules: [
    PrefixRuleMatch { matched_prefix: ["npm", "i", "--legacy-peer-deps"], decision: allow },
  ]
}"#]]
    .assert_eq(&npm_i.to_string());

    let npm_install = policy.evaluate(&tokens(&["npm", "install", "--no-save", "leftpad"]));
    expect![[r#"Match {
  decision: allow,
  matched_rules: [
    PrefixRuleMatch { matched_prefix: ["npm", "install", "--no-save"], decision: allow },
  ]
}"#]]
    .assert_eq(&npm_install.to_string());
}

#[test]
fn match_and_not_match_examples_are_enforced() {
    let policy_src = r#"
prefix_rule(
    pattern = ["git", "status"],
    match = [["git", "status"]],
    not_match = [["git", "reset", "--hard"]],
)
    "#;
    let parser = PolicyParser::new("test.policy", policy_src);
    let policy = parser.parse().expect("parse policy");
    let match_eval = policy.evaluate(&tokens(&["git", "status"]));
    expect![[r#"Match {
  decision: allow,
  matched_rules: [
    PrefixRuleMatch { matched_prefix: ["git", "status"], decision: allow },
  ]
}"#]]
    .assert_eq(&match_eval.to_string());

    let no_match_eval = policy.evaluate(&tokens(&["git", "reset", "--hard"]));
    expect!["NoMatch"].assert_eq(&no_match_eval.to_string());
}

#[test]
fn strictest_decision_wins_across_matches() {
    let policy_src = r#"
prefix_rule(
    pattern = ["git", "status"],
    decision = "allow",
)
prefix_rule(
    pattern = ["git"],
    decision = "prompt",
)
prefix_rule(
    pattern = ["git", "commit"],
    decision = "forbidden",
)
    "#;
    let parser = PolicyParser::new("test.policy", policy_src);
    let policy = parser.parse().expect("parse policy");

    let status = policy.evaluate(&tokens(&["git", "status"]));
    expect![[r#"Match {
  decision: prompt,
  matched_rules: [
    PrefixRuleMatch { matched_prefix: ["git", "status"], decision: allow },
    PrefixRuleMatch { matched_prefix: ["git"], decision: prompt },
  ]
}"#]]
    .assert_eq(&status.to_string());

    let commit = policy.evaluate(&tokens(&["git", "commit", "-m", "hi"]));
    expect![[r#"Match {
  decision: forbidden,
  matched_rules: [
    PrefixRuleMatch { matched_prefix: ["git"], decision: prompt },
    PrefixRuleMatch { matched_prefix: ["git", "commit"], decision: forbidden },
  ]
}"#]]
    .assert_eq(&commit.to_string());
}
