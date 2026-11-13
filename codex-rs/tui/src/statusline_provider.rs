//! MCP server identity and inventory for statusline providers.

#[cfg_attr(not(test), expect(dead_code, reason = "staged provider discovery"))]
mod inventory;
#[cfg(test)]
mod test_support;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct McpServerName(String);

impl McpServerName {
    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for McpServerName {
    fn from(name: &str) -> Self {
        Self(name.to_owned())
    }
}
