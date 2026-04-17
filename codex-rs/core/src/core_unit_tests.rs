mod tools_spec {
    pub(crate) use crate::tools::registry::ToolRegistryBuilder;
    pub(crate) use crate::tools::spec::build_specs_with_discoverable_tools;
    pub(crate) use crate::tools::spec::tool_user_shell_type;
    pub(crate) use codex_mcp::ToolInfo;
    pub(crate) use codex_protocol::dynamic_tools::DynamicToolSpec;

    mod tests {
        use std::collections::HashMap;

        include!("../tests/unit/tools/spec_tests.rs");
    }
}
