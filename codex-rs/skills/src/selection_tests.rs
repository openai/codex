use super::*;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::collections::HashSet;

#[derive(Default)]
struct TestLookup {
    skills: Vec<SkillMetadata>,
    disabled_paths: HashSet<PathUri>,
    skill_discovery_path_by_path: HashMap<PathUri, PathUri>,
}

impl ExplicitSkillLookup for TestLookup {
    fn skills(&self) -> &[SkillMetadata] {
        &self.skills
    }

    fn disabled_paths(&self) -> &HashSet<PathUri> {
        &self.disabled_paths
    }

    fn skill_discovery_path_for_path(&self, path: &PathUri) -> Option<&PathUri> {
        self.skill_discovery_path_by_path.get(path)
    }
}

fn make_skill(name: &str, path: &str) -> SkillMetadata {
    SkillMetadata {
        name: name.to_string(),
        description: format!("{name} skill"),
        short_description: None,
        interface: None,
        dependencies: None,
        policy: None,
        path_to_skills_md: test_path_buf(path).abs().into(),
        scope: codex_protocol::protocol::SkillScope::User,
        plugin_id: None,
        remote_plugin_id: None,
    }
}

fn linked_skill_mention(name: &str, unix_path: &str) -> String {
    format!("[${name}]({})", test_path_buf(unix_path).display())
}

fn collect_mentions(
    inputs: &[UserInput],
    skills: &[SkillMetadata],
    disabled_paths: &HashSet<PathUri>,
    connector_slug_counts: &HashMap<String, usize>,
) -> Vec<SkillMetadata> {
    let loaded_skills = TestLookup {
        skills: skills.to_vec(),
        disabled_paths: disabled_paths.clone(),
        ..Default::default()
    };
    collect_explicit_skill_mentions(inputs, &loaded_skills, connector_slug_counts)
}

fn skill_outcome_with_discovery_path(skill: SkillMetadata, discovery_path: &str) -> TestLookup {
    TestLookup {
        skill_discovery_path_by_path: HashMap::from([(
            skill.path_to_skills_md.clone(),
            test_path_buf(discovery_path).abs().into(),
        )]),
        skills: vec![skill],
        ..Default::default()
    }
}

#[test]
fn collect_explicit_skill_mentions_text_respects_skill_order() {
    let alpha = make_skill("alpha-skill", "/tmp/alpha");
    let beta = make_skill("beta-skill", "/tmp/beta");
    let skills = vec![beta.clone(), alpha.clone()];
    let inputs = vec![UserInput::Text {
        text: "first $alpha-skill then $beta-skill".to_string(),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    // Text scanning should not change the previous selection ordering semantics.
    assert_eq!(selected, vec![beta, alpha]);
}

#[test]
fn collect_explicit_skill_mentions_prioritizes_structured_inputs() {
    let alpha = make_skill("alpha-skill", "/tmp/alpha");
    let beta = make_skill("beta-skill", "/tmp/beta");
    let skills = vec![alpha.clone(), beta.clone()];
    let inputs = vec![
        UserInput::Text {
            text: "please run $alpha-skill".to_string(),
            text_elements: Vec::new(),
        },
        UserInput::Skill {
            name: "beta-skill".to_string(),
            path: test_path_buf("/tmp/beta"),
        },
    ];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, vec![beta, alpha]);
}

#[test]
fn collect_explicit_skill_mentions_accepts_structured_discovery_path() {
    let skill = make_skill("linked-skill", "/tmp/shared/linked-skill/SKILL.md");
    let loaded_skills = skill_outcome_with_discovery_path(
        skill.clone(),
        "/tmp/project/.agents/skills/linked-skill/SKILL.md",
    );
    let inputs = vec![UserInput::Skill {
        name: "linked-skill".to_string(),
        path: test_path_buf("/tmp/project/.agents/skills/linked-skill/SKILL.md"),
    }];

    let selected = collect_explicit_skill_mentions(&inputs, &loaded_skills, &HashMap::new());

    assert_eq!(selected, vec![skill]);
}

#[test]
fn collect_explicit_skill_mentions_accepts_linked_discovery_path() {
    let skill = make_skill("linked-skill", "/tmp/shared/linked-skill/SKILL.md");
    let loaded_skills = skill_outcome_with_discovery_path(
        skill.clone(),
        "/tmp/project/.agents/skills/linked-skill/SKILL.md",
    );
    let inputs = vec![UserInput::Text {
        text: linked_skill_mention(
            "linked-skill",
            "/tmp/project/.agents/skills/linked-skill/SKILL.md",
        ),
        text_elements: Vec::new(),
    }];

    let selected = collect_explicit_skill_mentions(&inputs, &loaded_skills, &HashMap::new());

    assert_eq!(selected, vec![skill]);
}

#[test]
fn collect_explicit_skill_mentions_rejects_disabled_discovery_path() {
    let skill = make_skill("linked-skill", "/tmp/shared/linked-skill/SKILL.md");
    let mut loaded_skills = skill_outcome_with_discovery_path(
        skill.clone(),
        "/tmp/project/.agents/skills/linked-skill/SKILL.md",
    );
    loaded_skills.disabled_paths.insert(skill.path_to_skills_md);
    let inputs = vec![UserInput::Skill {
        name: "linked-skill".to_string(),
        path: test_path_buf("/tmp/project/.agents/skills/linked-skill/SKILL.md"),
    }];

    let selected = collect_explicit_skill_mentions(&inputs, &loaded_skills, &HashMap::new());

    assert_eq!(selected, Vec::new());
}

#[test]
fn collect_explicit_skill_mentions_skips_invalid_structured_and_blocks_plain_fallback() {
    let alpha = make_skill("alpha-skill", "/tmp/alpha");
    let skills = vec![alpha];
    let inputs = vec![
        UserInput::Text {
            text: "please run $alpha-skill".to_string(),
            text_elements: Vec::new(),
        },
        UserInput::Skill {
            name: "alpha-skill".to_string(),
            path: test_path_buf("/tmp/missing"),
        },
    ];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, Vec::new());
}

#[test]
fn collect_explicit_skill_mentions_skips_disabled_structured_and_blocks_plain_fallback() {
    let alpha = make_skill("alpha-skill", "/tmp/alpha");
    let skills = vec![alpha];
    let inputs = vec![
        UserInput::Text {
            text: "please run $alpha-skill".to_string(),
            text_elements: Vec::new(),
        },
        UserInput::Skill {
            name: "alpha-skill".to_string(),
            path: test_path_buf("/tmp/alpha"),
        },
    ];
    let disabled = HashSet::from([test_path_buf("/tmp/alpha").abs().into()]);
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &disabled, &connector_counts);

    assert_eq!(selected, Vec::new());
}

#[test]
fn collect_explicit_skill_mentions_dedupes_by_path() {
    let alpha = make_skill("alpha-skill", "/tmp/alpha");
    let skills = vec![alpha.clone()];
    let mention = linked_skill_mention("alpha-skill", "/tmp/alpha");
    let inputs = vec![UserInput::Text {
        text: format!("use {mention} and {mention}"),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, vec![alpha]);
}

#[test]
fn collect_explicit_skill_mentions_skips_ambiguous_name() {
    let alpha = make_skill("demo-skill", "/tmp/alpha");
    let beta = make_skill("demo-skill", "/tmp/beta");
    let skills = vec![alpha, beta];
    let inputs = vec![UserInput::Text {
        text: "use $demo-skill and again $demo-skill".to_string(),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, Vec::new());
}

#[test]
fn collect_explicit_skill_mentions_prefers_linked_path_over_name() {
    let alpha = make_skill("demo-skill", "/tmp/alpha");
    let beta = make_skill("demo-skill", "/tmp/beta");
    let skills = vec![alpha, beta.clone()];
    let inputs = vec![UserInput::Text {
        text: format!(
            "use $demo-skill and {}",
            linked_skill_mention("demo-skill", "/tmp/beta")
        ),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, vec![beta]);
}

#[test]
fn collect_explicit_skill_mentions_skips_plain_name_when_connector_matches() {
    let alpha = make_skill("alpha-skill", "/tmp/alpha");
    let skills = vec![alpha];
    let inputs = vec![UserInput::Text {
        text: "use $alpha-skill".to_string(),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::from([("alpha-skill".to_string(), 1)]);

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, Vec::new());
}

#[test]
fn collect_explicit_skill_mentions_allows_explicit_path_with_connector_conflict() {
    let alpha = make_skill("alpha-skill", "/tmp/alpha");
    let skills = vec![alpha.clone()];
    let inputs = vec![UserInput::Text {
        text: format!("use {}", linked_skill_mention("alpha-skill", "/tmp/alpha")),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::from([("alpha-skill".to_string(), 1)]);

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, vec![alpha]);
}

#[test]
fn collect_explicit_skill_mentions_skips_when_linked_path_disabled() {
    let alpha = make_skill("demo-skill", "/tmp/alpha");
    let beta = make_skill("demo-skill", "/tmp/beta");
    let skills = vec![alpha, beta];
    let inputs = vec![UserInput::Text {
        text: format!("use {}", linked_skill_mention("demo-skill", "/tmp/alpha")),
        text_elements: Vec::new(),
    }];
    let disabled = HashSet::from([test_path_buf("/tmp/alpha").abs().into()]);
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &disabled, &connector_counts);

    assert_eq!(selected, Vec::new());
}

#[test]
fn collect_explicit_skill_mentions_prefers_resource_path() {
    let alpha = make_skill("demo-skill", "/tmp/alpha");
    let beta = make_skill("demo-skill", "/tmp/beta");
    let skills = vec![alpha, beta.clone()];
    let inputs = vec![UserInput::Text {
        text: format!("use {}", linked_skill_mention("demo-skill", "/tmp/beta")),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, vec![beta]);
}

#[test]
fn collect_explicit_skill_mentions_skips_missing_path_with_no_fallback() {
    let alpha = make_skill("demo-skill", "/tmp/alpha");
    let beta = make_skill("demo-skill", "/tmp/beta");
    let skills = vec![alpha, beta];
    let inputs = vec![UserInput::Text {
        text: format!("use {}", linked_skill_mention("demo-skill", "/tmp/missing")),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, Vec::new());
}

#[test]
fn collect_explicit_skill_mentions_skips_missing_path_without_fallback() {
    let alpha = make_skill("demo-skill", "/tmp/alpha");
    let skills = vec![alpha];
    let inputs = vec![UserInput::Text {
        text: format!("use {}", linked_skill_mention("demo-skill", "/tmp/missing")),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, Vec::new());
}

/// Linked native paths use URI identity across Windows case and separator spellings.
#[test]
fn collect_explicit_skill_mentions_matches_windows_path_identity() {
    let skill = SkillMetadata {
        path_to_skills_md: PathUri::parse("file:///C:/Skills/Demo/SKILL.md").unwrap(),
        ..make_skill("demo-skill", "/tmp/demo/SKILL.md")
    };
    let loaded_skills = TestLookup {
        skill_discovery_path_by_path: HashMap::from([(
            skill.path_to_skills_md.clone(),
            PathUri::parse("file:///C:/Project/.agents/skills/Demo/SKILL.md").unwrap(),
        )]),
        skills: vec![skill.clone()],
        ..Default::default()
    };

    for path in [
        r"c:\skills\demo\skill.md",
        "C:/SKILLS/DEMO/skill.md",
        "skill://c:/project/.agents/skills/demo/skill.md",
    ] {
        let inputs = vec![UserInput::Text {
            text: format!("use [$demo-skill]({path})"),
            text_elements: Vec::new(),
        }];

        let selected = collect_explicit_skill_mentions(&inputs, &loaded_skills, &HashMap::new());

        assert_eq!(selected, vec![skill.clone()]);
    }
}

/// A differently cased disabled Windows identity still blocks a linked skill selection.
#[test]
fn collect_explicit_skill_mentions_rejects_disabled_windows_path_identity() {
    let skill = SkillMetadata {
        path_to_skills_md: PathUri::parse("file:///C:/Skills/Demo/SKILL.md").unwrap(),
        ..make_skill("demo-skill", "/tmp/demo/SKILL.md")
    };
    let loaded_skills = TestLookup {
        skills: vec![skill],
        disabled_paths: HashSet::from([PathUri::parse("file:///C:/skills/demo/skill.md").unwrap()]),
        ..Default::default()
    };
    let inputs = vec![UserInput::Text {
        text: r"use [$demo-skill](C:\Skills\Demo\SKILL.md)".to_string(),
        text_elements: Vec::new(),
    }];

    let selected = collect_explicit_skill_mentions(&inputs, &loaded_skills, &HashMap::new());

    assert_eq!(selected, Vec::new());
}

/// Native mention text retains literal spaces, percent escapes, and fragments in skill paths.
#[test]
fn collect_explicit_skill_mentions_preserves_native_uri_characters() {
    let percent = make_skill("demo-skill", "/tmp/demo skill%23/SKILL.md");
    let fragment = make_skill("demo-skill", "/tmp/demo skill#/SKILL.md");
    let loaded_skills = TestLookup {
        skills: vec![percent.clone(), fragment.clone()],
        ..Default::default()
    };

    for (path, skill) in [
        ("/tmp/demo skill%23/SKILL.md", percent),
        ("/tmp/demo skill#/SKILL.md", fragment),
    ] {
        let inputs = vec![UserInput::Text {
            text: linked_skill_mention("demo-skill", path),
            text_elements: Vec::new(),
        }];

        let selected = collect_explicit_skill_mentions(&inputs, &loaded_skills, &HashMap::new());

        assert_eq!(selected, vec![skill]);
    }
}
