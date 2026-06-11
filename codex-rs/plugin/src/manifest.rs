use codex_config::HooksFile;
use codex_utils_absolute_path::AbsolutePathBuf;

/// Parsed metadata and component locators from a plugin manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginManifest {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub keywords: Vec<String>,
    pub paths: PluginManifestPaths,
    pub interface: Option<PluginManifestInterface>,
}

/// Component paths declared by a plugin manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginManifestPaths {
    pub skills: Option<AbsolutePathBuf>,
    pub mcp_servers: Option<AbsolutePathBuf>,
    pub apps: Option<AbsolutePathBuf>,
    pub hooks: Option<PluginManifestHooks>,
}

/// Hook declarations embedded in or referenced by a plugin manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginManifestHooks {
    Paths(Vec<AbsolutePathBuf>),
    Inline(Vec<HooksFile>),
}

/// Optional model- and UI-facing plugin metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginManifestInterface {
    pub display_name: Option<String>,
    pub short_description: Option<String>,
    pub long_description: Option<String>,
    pub developer_name: Option<String>,
    pub category: Option<String>,
    pub capabilities: Vec<String>,
    pub website_url: Option<String>,
    pub privacy_policy_url: Option<String>,
    pub terms_of_service_url: Option<String>,
    pub default_prompt: Option<Vec<String>>,
    pub brand_color: Option<String>,
    pub composer_icon: Option<AbsolutePathBuf>,
    pub logo: Option<AbsolutePathBuf>,
    pub screenshots: Vec<AbsolutePathBuf>,
}
