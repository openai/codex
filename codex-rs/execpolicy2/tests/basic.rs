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
    let policy = PolicyParser::new("test.codexpolicy", policy_src)
        .parse()
        .expect("parse policy");
    let cmd = tokens(&["git", "status"]);
    let evaluation = policy.check(&cmd);
    expect![[r#"
        match {
          decision: allow,
          matchedRules: [
            prefixRuleMatch { matchedPrefix: ["git", "status"], decision: allow },
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
    let parser = PolicyParser::new("test.codexpolicy", policy_src);
    let policy = parser.parse().expect("parse policy");

    let bash_rules = policy.rules().get_vec("bash").expect("bash rules");
    let sh_rules = policy.rules().get_vec("sh").expect("sh rules");
    expect![[r#"[prefix_rule(pattern = [bash, [-c, -l]], decision = allow)]"#]]
        .assert_eq(&rules_to_string(bash_rules));
    expect![[r#"[prefix_rule(pattern = [sh, [-c, -l]], decision = allow)]"#]]
        .assert_eq(&rules_to_string(sh_rules));

    let bash_eval = policy.check(&tokens(&["bash", "-c", "echo", "hi"]));
    expect![[r#"
        match {
          decision: allow,
          matchedRules: [
            prefixRuleMatch { matchedPrefix: ["bash", "-c"], decision: allow },
          ]
        }"#]]
    .assert_eq(&bash_eval.to_string());

    let sh_eval = policy.check(&tokens(&["sh", "-l", "echo", "hi"]));
    expect![[r#"
        match {
          decision: allow,
          matchedRules: [
            prefixRuleMatch { matchedPrefix: ["sh", "-l"], decision: allow },
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
    let parser = PolicyParser::new("test.codexpolicy", policy_src);
    let policy = parser.parse().expect("parse policy");

    let rules = policy.rules().get_vec("npm").expect("npm rules");
    expect![[r#"[prefix_rule(pattern = [npm, [i, install], [--legacy-peer-deps, --no-save]], decision = allow)]"#]]
        .assert_eq(&rules_to_string(rules));

    let npm_i = policy.check(&tokens(&["npm", "i", "--legacy-peer-deps"]));
    expect![[r#"
        match {
          decision: allow,
          matchedRules: [
            prefixRuleMatch { matchedPrefix: ["npm", "i", "--legacy-peer-deps"], decision: allow },
          ]
        }"#]]
    .assert_eq(&npm_i.to_string());

    let npm_install = policy.check(&tokens(&["npm", "install", "--no-save", "leftpad"]));
    expect![[r#"
        match {
          decision: allow,
          matchedRules: [
            prefixRuleMatch { matchedPrefix: ["npm", "install", "--no-save"], decision: allow },
          ]
        }"#]]
    .assert_eq(&npm_install.to_string());
}

#[test]
fn match_and_not_match_examples_are_enforced() {
    let policy_src = r#"
prefix_rule(
    pattern = ["git", "status"],
    match = [["git", "status"], "git status"],
    not_match = [
        ["git", "--config", "color.status=always", "status"],
        "git --config color.status=always status",
    ],
)
    "#;
    let parser = PolicyParser::new("test.codexpolicy", policy_src);
    let policy = parser.parse().expect("parse policy");
    let match_eval = policy.check(&tokens(&["git", "status"]));
    expect![[r#"
        match {
          decision: allow,
          matchedRules: [
            prefixRuleMatch { matchedPrefix: ["git", "status"], decision: allow },
          ]
        }"#]]
    .assert_eq(&match_eval.to_string());

    let no_match_eval = policy.check(&tokens(&[
        "git",
        "--config",
        "color.status=always",
        "status",
    ]));
    expect!["noMatch"].assert_eq(&no_match_eval.to_string());
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
    let parser = PolicyParser::new("test.codexpolicy", policy_src);
    let policy = parser.parse().expect("parse policy");

    let status = policy.check(&tokens(&["git", "status"]));
    expect![[r#"
        match {
          decision: prompt,
          matchedRules: [
            prefixRuleMatch { matchedPrefix: ["git", "status"], decision: allow },
            prefixRuleMatch { matchedPrefix: ["git"], decision: prompt },
          ]
        }"#]]
    .assert_eq(&status.to_string());

    let commit = policy.check(&tokens(&["git", "commit", "-m", "hi"]));
    expect![[r#"
        match {
          decision: forbidden,
          matchedRules: [
            prefixRuleMatch { matchedPrefix: ["git"], decision: prompt },
            prefixRuleMatch { matchedPrefix: ["git", "commit"], decision: forbidden },
          ]
        }"#]]
    .assert_eq(&commit.to_string());
}
