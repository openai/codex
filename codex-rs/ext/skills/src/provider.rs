use std::future::Future;
use std::pin::Pin;

mod host;

use codex_core::config::Config;
use codex_core_skills::SkillsLoadInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_plugins::PluginSkillRoot;

use crate::catalog::SkillAuthority;
use crate::catalog::SkillCatalog;
use crate::catalog::SkillPackageId;
use crate::catalog::SkillProviderResult;
use crate::catalog::SkillReadResult;
use crate::catalog::SkillResourceId;
use crate::catalog::SkillSearchResult;

pub use host::HostSkillProvider;

#[derive(Clone, Debug)]
pub struct SkillListQuery {
    pub turn_id: String,
    pub executor_authorities: Vec<SkillAuthority>,
    pub host: Option<HostSkillListQuery>,
    pub include_host_skills: bool,
    pub include_bundled_skills: bool,
    pub include_remote_skills: bool,
}

#[derive(Clone, Debug)]
pub struct HostSkillConfig {
    codex_home: AbsolutePathBuf,
    input: SkillsLoadInput,
}

impl HostSkillConfig {
    pub fn from_config(config: &Config, effective_skill_roots: Vec<PluginSkillRoot>) -> Self {
        Self {
            codex_home: config.codex_home.clone(),
            input: SkillsLoadInput::new(
                config.cwd.clone(),
                effective_skill_roots,
                config.config_layer_stack.clone(),
                config.bundled_skills_enabled(),
            ),
        }
    }

    pub fn list_query(&self, cwd: AbsolutePathBuf) -> HostSkillListQuery {
        let mut input = self.input.clone();
        input.cwd = cwd;
        HostSkillListQuery {
            codex_home: self.codex_home.clone(),
            input,
        }
    }

    pub fn default_cwd(&self) -> AbsolutePathBuf {
        self.input.cwd.clone()
    }
}

#[derive(Clone, Debug)]
pub struct HostSkillListQuery {
    pub codex_home: AbsolutePathBuf,
    pub input: SkillsLoadInput,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillReadRequest {
    pub authority: SkillAuthority,
    pub package: SkillPackageId,
    pub resource: SkillResourceId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillSearchRequest {
    pub authority: SkillAuthority,
    pub package: SkillPackageId,
    pub query: String,
}

pub type SkillProviderFuture<'a, T> =
    Pin<Box<dyn Future<Output = SkillProviderResult<T>> + Send + 'a>>;

/// Source-specific skill catalog and resource access.
///
/// Implementations must preserve authority boundaries: a resource listed by a
/// provider must be read or searched through the same provider/authority rather
/// than converted into an ambient local path.
pub trait SkillProvider: Send + Sync {
    fn list(&self, query: SkillListQuery) -> SkillProviderFuture<'_, SkillCatalog>;

    fn read(&self, request: SkillReadRequest) -> SkillProviderFuture<'_, SkillReadResult>;

    fn search(&self, request: SkillSearchRequest) -> SkillProviderFuture<'_, SkillSearchResult>;
}
