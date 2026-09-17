pub(crate) const LIFE_SCIENCES_OAUTH_STATE_SUFFIX: &str = ".onboarding_entrypoint=life_sciences";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginOnboardingEntrypoint {
    LifeSciences,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LoginCallbackResult {
    pub onboarding_entrypoint: Option<LoginOnboardingEntrypoint>,
}
