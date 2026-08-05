mod discovery;
mod environment;
mod host;
mod metadata;
mod namespace;

pub(crate) use environment::load_environment_skills_from_discovery;
pub(crate) use environment::load_environment_skills_from_root;
pub(crate) use host::HostSkillRoot;
pub(crate) use host::load_host_skill_root;

pub(super) const SKILLS_FILENAME: &str = "SKILL.md";
pub(super) const SKILLS_METADATA_DIR: &str = "agents";
pub(super) const SKILLS_METADATA_FILENAME: &str = "openai.yaml";
pub(super) const MAX_NAME_LEN: usize = 64;
pub(super) const MAX_QUALIFIED_NAME_LEN: usize = 128;
pub(super) const MAX_DESCRIPTION_LEN: usize = 1024;
pub(super) const MAX_DEPENDENCY_TYPE_LEN: usize = MAX_NAME_LEN;
pub(super) const MAX_DEPENDENCY_TRANSPORT_LEN: usize = MAX_NAME_LEN;
pub(super) const MAX_DEPENDENCY_VALUE_LEN: usize = MAX_DESCRIPTION_LEN;
pub(super) const MAX_DEPENDENCY_DESCRIPTION_LEN: usize = MAX_DESCRIPTION_LEN;
pub(super) const MAX_DEPENDENCY_COMMAND_LEN: usize = MAX_DESCRIPTION_LEN;
pub(super) const MAX_DEPENDENCY_URL_LEN: usize = MAX_DESCRIPTION_LEN;
pub(super) const MAX_SCAN_DEPTH: usize = 6;
pub(super) const MAX_SKILLS_DIRS_PER_ROOT: usize = 2000;
