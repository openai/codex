use codex_config::HooksFile;

/// Parsed plugin metadata parameterized by its resource locator representation.
///
/// Host loading uses absolute paths, while resolved packages replace them with
/// authority-bound locators before exposing the manifest to consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginManifest<Resource> {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub keywords: Vec<String>,
    pub paths: PluginManifestPaths<Resource>,
    pub interface: Option<PluginManifestInterface<Resource>>,
}

/// Component resources declared by a plugin manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginManifestPaths<Resource> {
    pub skills: Vec<Resource>,
    pub mcp_servers: Option<PluginManifestMcpServers<Resource>>,
    pub apps: Vec<Resource>,
    pub hooks: Option<PluginManifestHooks<Resource>>,
}

/// MCP server declarations embedded in or referenced by a plugin manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginManifestMcpServers<Resource> {
    Paths(Vec<Resource>),
    Object(String),
}

/// Hook declarations embedded in or referenced by a plugin manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginManifestHooks<Resource> {
    Paths(Vec<Resource>),
    Inline(Vec<HooksFile>),
}

/// Optional model- and UI-facing plugin metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginManifestInterface<Resource> {
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
    pub composer_icon: Option<Resource>,
    pub logo: Option<Resource>,
    pub screenshots: Vec<Resource>,
}

impl<Resource> Default for PluginManifestInterface<Resource> {
    fn default() -> Self {
        Self {
            display_name: None,
            short_description: None,
            long_description: None,
            developer_name: None,
            category: None,
            capabilities: Vec::new(),
            website_url: None,
            privacy_policy_url: None,
            terms_of_service_url: None,
            default_prompt: None,
            brand_color: None,
            composer_icon: None,
            logo: None,
            screenshots: Vec::new(),
        }
    }
}

impl<Resource> PluginManifest<Resource> {
    /// Returns the model- and UI-facing package name, falling back to the manifest name.
    pub fn display_name(&self) -> &str {
        self.interface
            .as_ref()
            .and_then(|interface| interface.display_name.as_deref())
            .map(str::trim)
            .filter(|display_name| !display_name.is_empty())
            .unwrap_or(&self.name)
    }

    pub(crate) fn try_map_resources<Mapped, Error>(
        self,
        mut map: impl FnMut(Resource) -> Result<Mapped, Error>,
    ) -> Result<PluginManifest<Mapped>, Error> {
        let PluginManifest {
            name,
            version,
            description,
            keywords,
            paths,
            interface,
        } = self;
        let PluginManifestPaths {
            skills,
            mcp_servers,
            apps,
            hooks,
        } = paths;
        let hooks = match hooks {
            Some(PluginManifestHooks::Paths(paths)) => Some(PluginManifestHooks::Paths(
                paths
                    .into_iter()
                    .map(&mut map)
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            Some(PluginManifestHooks::Inline(hooks)) => Some(PluginManifestHooks::Inline(hooks)),
            None => None,
        };
        let mcp_servers = match mcp_servers {
            Some(PluginManifestMcpServers::Paths(paths)) => Some(PluginManifestMcpServers::Paths(
                paths
                    .into_iter()
                    .map(&mut map)
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            Some(PluginManifestMcpServers::Object(servers)) => {
                Some(PluginManifestMcpServers::Object(servers))
            }
            None => None,
        };
        let interface = match interface {
            Some(interface) => {
                let PluginManifestInterface {
                    display_name,
                    short_description,
                    long_description,
                    developer_name,
                    category,
                    capabilities,
                    website_url,
                    privacy_policy_url,
                    terms_of_service_url,
                    default_prompt,
                    brand_color,
                    composer_icon,
                    logo,
                    screenshots,
                } = interface;
                Some(PluginManifestInterface {
                    display_name,
                    short_description,
                    long_description,
                    developer_name,
                    category,
                    capabilities,
                    website_url,
                    privacy_policy_url,
                    terms_of_service_url,
                    default_prompt,
                    brand_color,
                    composer_icon: composer_icon.map(&mut map).transpose()?,
                    logo: logo.map(&mut map).transpose()?,
                    screenshots: screenshots
                        .into_iter()
                        .map(&mut map)
                        .collect::<Result<Vec<_>, _>>()?,
                })
            }
            None => None,
        };

        Ok(PluginManifest {
            name,
            version,
            description,
            keywords,
            paths: PluginManifestPaths {
                skills: skills
                    .into_iter()
                    .map(&mut map)
                    .collect::<Result<Vec<_>, _>>()?,
                mcp_servers,
                apps: apps
                    .into_iter()
                    .map(&mut map)
                    .collect::<Result<Vec<_>, _>>()?,
                hooks,
            },
            interface,
        })
    }
}
