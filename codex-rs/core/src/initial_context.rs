#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InitialContextInclusions {
    pub(crate) model_update: bool,
    pub(crate) permissions: bool,
    pub(crate) developer_instructions: bool,
    pub(crate) separate_developer_instructions: bool,
    pub(crate) memory: bool,
    pub(crate) collaboration: bool,
    pub(crate) realtime: bool,
    pub(crate) personality: bool,
    pub(crate) apps: bool,
    pub(crate) skills: bool,
    pub(crate) plugins: bool,
    pub(crate) commit: bool,
    pub(crate) user_instructions: bool,
    pub(crate) environment_context: bool,
}

impl InitialContextInclusions {
    pub(crate) const fn full() -> Self {
        Self {
            model_update: true,
            permissions: true,
            developer_instructions: true,
            separate_developer_instructions: false,
            memory: true,
            collaboration: true,
            realtime: true,
            personality: true,
            apps: true,
            skills: true,
            plugins: true,
            commit: true,
            user_instructions: true,
            environment_context: true,
        }
    }

    pub(crate) const fn none() -> Self {
        Self {
            model_update: false,
            permissions: false,
            developer_instructions: false,
            separate_developer_instructions: false,
            memory: false,
            collaboration: false,
            realtime: false,
            personality: false,
            apps: false,
            skills: false,
            plugins: false,
            commit: false,
            user_instructions: false,
            environment_context: false,
        }
    }
}
