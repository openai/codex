mod environment;
mod metadata;

pub(crate) use environment::load_environment_skills_from_discovery;

pub(super) const MAX_NAME_LEN: usize = 64;
pub(super) const MAX_QUALIFIED_NAME_LEN: usize = 128;
pub(super) const MAX_DESCRIPTION_LEN: usize = 1024;
pub(super) const MAX_DEPENDENCY_TYPE_LEN: usize = MAX_NAME_LEN;
pub(super) const MAX_DEPENDENCY_TRANSPORT_LEN: usize = MAX_NAME_LEN;
pub(super) const MAX_DEPENDENCY_VALUE_LEN: usize = MAX_DESCRIPTION_LEN;
pub(super) const MAX_DEPENDENCY_DESCRIPTION_LEN: usize = MAX_DESCRIPTION_LEN;
pub(super) const MAX_DEPENDENCY_COMMAND_LEN: usize = MAX_DESCRIPTION_LEN;
pub(super) const MAX_DEPENDENCY_URL_LEN: usize = MAX_DESCRIPTION_LEN;
