use std::collections::HashSet;

#[derive(Clone, Debug)]
pub(crate) struct ToolPolicy {
    pub(crate) allowed_tools: Option<HashSet<String>>,
    pub(crate) denied_tools: HashSet<String>,
    pub(crate) allow_mcp_tools: bool,
    pub(crate) shell_policy: ShellPolicy,
    pub(crate) extra_tools: HashSet<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ShellPolicy {
    #[default]
    Unrestricted,
    ReadOnly,
}

impl Default for ToolPolicy {
    fn default() -> Self {
        Self {
            allowed_tools: None,
            denied_tools: HashSet::new(),
            allow_mcp_tools: true,
            shell_policy: ShellPolicy::Unrestricted,
            extra_tools: HashSet::new(),
        }
    }
}

impl ToolPolicy {
    pub(crate) fn allows_tool(&self, name: &str) -> bool {
        if self.denied_tools.contains(name) {
            return false;
        }
        match &self.allowed_tools {
            Some(allowed) => allowed.contains(name),
            None => true,
        }
    }
}
