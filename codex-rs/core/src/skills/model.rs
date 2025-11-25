use std::collections::HashMap;
use std::path::PathBuf;

/// Origin of a skill, or the location it was loaded from.
/// - Public: fetched from the global, public location.
/// - Private: fetched from the current session's private location.
/// - Byos: Bring Your Own Skills; untracked, local folder attached by user using prompt.
///
/// TODO: In v0, all entries from `~/.codex/.skills` are treated as `Public`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillScope {
    Public,
    Private,
    Byos,
}

/// In-memory representation of a single skill loaded from disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    pub scope: SkillScope,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub path: PathBuf,
    pub content: String,
    pub license: Option<String>,
    pub version: Option<String>,
}

impl Skill {
    /// Deterministic identifier combining scope, name, and normalized path.
    /// Format: `<scope>:<name>:<path>` where path uses forward slashes.
    pub fn id(&self) -> String {
        let scope = match self.scope {
            SkillScope::Public => "public",
            SkillScope::Private => "private",
            SkillScope::Byos => "byos",
        };
        let path_str = self.path.to_string_lossy().replace('\\', "/");
        format!("{scope}:{}:{path_str}", self.name)
    }
}

/// In-memory representation of a collection of skills available to the session.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillCatalog {
    pub skills: HashMap<String, Skill>,
}

#[cfg(test)]
mod tests {
    use super::Skill;
    use super::SkillCatalog;
    use super::SkillScope;
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[test]
    fn constructs_skill_and_catalog() {
        let skill = Skill {
            scope: SkillScope::Public,
            name: "example-skill".to_string(),
            description: "Demo skill for testing".to_string(),
            tags: vec!["demo".to_string(), "testing".to_string()],
            path: PathBuf::from("/tmp/skills/example-skill"),
            content: "# Example Skill\n\nBody.".to_string(),
            license: Some("Apache-2.0".to_string()),
            version: Some("abc123".to_string()),
        };

        let id = skill.id();
        let mut skills = HashMap::new();
        skills.insert(id.clone(), skill);

        let catalog = SkillCatalog { skills };

        assert_eq!(catalog.skills.len(), 1);
        let retrieved = catalog.skills.get(&id).unwrap();
        assert_eq!(retrieved.name, "example-skill");
        assert_eq!(retrieved.scope, SkillScope::Public);
        assert_eq!(retrieved.version.as_deref(), Some("abc123"));
    }

    #[test]
    fn id_composition_is_stable_and_path_sensitive() {
        struct TestCase {
            name: &'static str,
            scope: SkillScope,
            skill_name: &'static str,
            path: PathBuf,
            expected: &'static str,
        }

        let cases = vec![
            TestCase {
                name: "public simple",
                scope: SkillScope::Public,
                skill_name: "data-viz",
                path: PathBuf::from("/data-viz"),
                expected: "public:data-viz:/data-viz",
            },
            TestCase {
                name: "private simple",
                scope: SkillScope::Private,
                skill_name: "lint",
                path: PathBuf::from(".skills/lint"),
                expected: "private:lint:.skills/lint",
            },
            TestCase {
                name: "byos simple",
                scope: SkillScope::Byos,
                skill_name: "custom-skill",
                path: PathBuf::from("/my-skills/custom-skill"),
                expected: "byos:custom-skill:/my-skills/custom-skill",
            },
            TestCase {
                name: "public same name different path",
                scope: SkillScope::Public,
                skill_name: "data-viz",
                path: PathBuf::from("/tmp/skills/community/data-viz"),
                expected: "public:data-viz:/tmp/skills/community/data-viz",
            },
            TestCase {
                name: "public name and path folder don't match",
                scope: SkillScope::Public,
                skill_name: "data-viz-1",
                path: PathBuf::from("/tmp/skills/community/data-viz-2"),
                expected: "public:data-viz-1:/tmp/skills/community/data-viz-2",
            },
            TestCase {
                name: "byos windows separators with raw string as input",
                scope: SkillScope::Byos,
                skill_name: "custom-skill",
                path: PathBuf::from(r"C:\skills\custom-skill"),
                expected: "byos:custom-skill:C:/skills/custom-skill",
            },
            TestCase {
                name: "byos windows separators with escaped string as input",
                scope: SkillScope::Byos,
                skill_name: "custom-skill",
                path: PathBuf::from("C:\\skills\\custom-skill"),
                expected: "byos:custom-skill:C:/skills/custom-skill",
            },
        ];

        for case in cases {
            let skill = Skill {
                scope: case.scope,
                name: case.skill_name.to_string(),
                description: "d".to_string(),
                tags: vec![],
                path: case.path,
                content: "".to_string(),
                license: None,
                version: None,
            };
            assert_eq!(skill.id(), case.expected, "case {}", case.name);
        }
    }
}
