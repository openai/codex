use codex_protocol::config_types::ForcedLoginMethod;

/// Authentication restrictions supplied by locally managed requirements.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManagedAuthPolicy {
    pub allowed_login_methods: Option<Vec<ForcedLoginMethod>>,
    pub allowed_chatgpt_workspaces: Option<Vec<String>>,
}

impl ManagedAuthPolicy {
    pub fn allows_login_method(
        &self,
        method: ForcedLoginMethod,
        forced_login_method: Option<ForcedLoginMethod>,
        forced_workspaces: Option<&[String]>,
    ) -> bool {
        forced_login_method.is_none_or(|forced| forced == method)
            && self
                .allowed_login_methods
                .as_ref()
                .is_none_or(|allowed| allowed.contains(&method))
            && (method != ForcedLoginMethod::Chatgpt
                || self
                    .effective_chatgpt_workspaces(forced_workspaces)
                    .is_none_or(|workspaces| !workspaces.is_empty()))
    }

    pub fn allowed_login_methods(
        &self,
        forced_login_method: Option<ForcedLoginMethod>,
        forced_workspaces: Option<&[String]>,
    ) -> Vec<ForcedLoginMethod> {
        [ForcedLoginMethod::Api, ForcedLoginMethod::Chatgpt]
            .into_iter()
            .filter(|method| {
                self.allows_login_method(*method, forced_login_method, forced_workspaces)
            })
            .collect()
    }

    pub fn effective_chatgpt_workspaces(
        &self,
        forced_workspaces: Option<&[String]>,
    ) -> Option<Vec<String>> {
        match (
            forced_workspaces,
            self.allowed_chatgpt_workspaces.as_deref(),
        ) {
            (Some(forced), Some(allowed)) => Some(
                forced
                    .iter()
                    .filter(|workspace| allowed.contains(workspace))
                    .cloned()
                    .collect(),
            ),
            (Some(forced), None) => Some(forced.to_vec()),
            (None, Some(allowed)) => Some(allowed.to_vec()),
            (None, None) => None,
        }
    }
}
