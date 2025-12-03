use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillError {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct SkillLoadOutcome {
    pub skills: Vec<SkillMetadata>,
    pub errors: Vec<SkillError>,
}
