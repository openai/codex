use std::collections::HashMap;

use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct McpServerConfig {
    pub command: String,

    #[serde(default)]
    pub args: Vec<String>,

    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
}
