use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::io;
use std::sync::Arc;

use codex_exec_server::EnvironmentAccess;
use codex_exec_server::EnvironmentAccessExt;
use codex_exec_server::EnvironmentAccessKey;
use codex_exec_server::FileSystemEnvironmentAccessor;
use codex_exec_server::LOCAL_FS;
use codex_exec_server::ReadFileOptions;
use codex_protocol::protocol::SkillScope;
use codex_skills::SkillError;
use codex_skills::SkillMetadata;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use futures::StreamExt;

use crate::loader::MAX_CONCURRENT_SKILL_LOADS;
use crate::loader::canonicalize_for_skill_identity;

#[derive(Debug, Clone, Default)]
pub struct SkillLoadOutcome {
    pub skills: Vec<SkillMetadata>,
    pub errors: Vec<SkillError>,
    pub disabled_paths: HashSet<PathUri>,
    pub(crate) skill_roots: Vec<AbsolutePathBuf>,
    pub(crate) skill_root_by_path: Arc<HashMap<PathUri, AbsolutePathBuf>>,
    pub(crate) skill_discovery_path_by_path: Arc<HashMap<PathUri, PathUri>>,
    pub(crate) agent_plugin_skill_paths: HashSet<PathUri>,
    pub(crate) file_systems_by_skill_path: SkillFileSystemsByPath,
    pub(crate) implicit_skills_by_scripts_dir: Arc<HashMap<PathUri, SkillMetadata>>,
    pub(crate) implicit_skills_by_doc_path: Arc<HashMap<PathUri, SkillMetadata>>,
}

impl SkillLoadOutcome {
    /// Builds an already-composed outcome while retaining the filesystem that supplied each skill.
    pub(crate) fn from_parts(
        skills: Vec<SkillMetadata>,
        errors: Vec<SkillError>,
        skill_roots: Vec<AbsolutePathBuf>,
        skill_root_by_path: HashMap<PathUri, AbsolutePathBuf>,
        skill_discovery_path_by_path: HashMap<PathUri, PathUri>,
        agent_plugin_skill_paths: HashSet<PathUri>,
        file_systems_by_skill_path: HashMap<PathUri, EnvironmentAccessKey>,
    ) -> Self {
        Self {
            skills,
            errors,
            skill_roots,
            skill_root_by_path: Arc::new(skill_root_by_path),
            skill_discovery_path_by_path: Arc::new(skill_discovery_path_by_path),
            agent_plugin_skill_paths,
            file_systems_by_skill_path: SkillFileSystemsByPath::new(file_systems_by_skill_path),
            ..Self::default()
        }
    }

    pub fn is_skill_enabled(&self, skill: &SkillMetadata) -> bool {
        !self.disabled_paths.contains(&skill.path_to_skills_md)
    }

    pub(crate) fn skills_with_enabled(&self) -> impl Iterator<Item = (&SkillMetadata, bool)> {
        self.skills
            .iter()
            .map(|skill| (skill, self.is_skill_enabled(skill)))
    }

    pub(crate) async fn with_disabled_paths(
        mut self,
        disabled_paths: HashSet<PathUri>,
        accessors: &[&dyn EnvironmentAccess],
    ) -> Self {
        self.disabled_paths = disabled_paths;
        let mut by_scripts_dir = HashMap::new();
        let mut by_doc_path = HashMap::new();
        let mut scans = Vec::new();
        for skill in self
            .skills
            .iter()
            .filter(|skill| self.is_skill_enabled(skill))
        {
            let Some(source) = self.file_system_for_skill(skill) else {
                continue;
            };
            let Some(access) = accessors
                .iter()
                .copied()
                .find(|access| source == &access.cache_key())
            else {
                continue;
            };
            // Only native paths participate in implicit command detection.
            let Ok(path) = skill.path_to_skills_md.to_abs_path() else {
                continue;
            };
            scans.push(async move {
                let skill_doc_path = canonicalize_for_skill_identity(access, &path).await;
                let scripts_dir = match path.parent() {
                    Some(skill_dir) => Some(
                        canonicalize_for_skill_identity(access, &skill_dir.join("scripts")).await,
                    ),
                    None => None,
                };
                (skill, skill_doc_path, scripts_dir)
            });
        }
        let mut scans = futures::stream::iter(scans).buffered(MAX_CONCURRENT_SKILL_LOADS);
        while let Some((skill, doc_path, scripts_dir)) = scans.next().await {
            by_doc_path.insert(PathUri::from_abs_path(&doc_path), skill.clone());
            if let Some(scripts_dir) = scripts_dir {
                by_scripts_dir.insert(PathUri::from_abs_path(&scripts_dir), skill.clone());
            }
        }
        drop(scans);
        self.implicit_skills_by_scripts_dir = Arc::new(by_scripts_dir);
        self.implicit_skills_by_doc_path = Arc::new(by_doc_path);
        self
    }

    pub(crate) fn is_agent_plugin_skill(&self, skill: &SkillMetadata) -> bool {
        self.agent_plugin_skill_paths
            .contains(&skill.path_to_skills_md)
    }

    /// Returns the discovery root that supplied a loaded skill path.
    pub(crate) fn skill_root_for_path(&self, path: &PathUri) -> Option<&AbsolutePathBuf> {
        self.skill_root_by_path.get(path)
    }

    /// Returns the logical path used to discover a canonical skill path.
    pub(crate) fn skill_discovery_path_for_path(&self, path: &PathUri) -> Option<&PathUri> {
        self.skill_discovery_path_by_path.get(path)
    }

    /// Returns loaded skill roots in discovery order.
    pub(crate) fn skill_roots_in_discovery_order(&self) -> impl Iterator<Item = &AbsolutePathBuf> {
        self.skill_roots.iter()
    }

    pub(crate) fn file_system_for_skill(
        &self,
        skill: &SkillMetadata,
    ) -> Option<&EnvironmentAccessKey> {
        self.file_systems_by_skill_path
            .get(&skill.path_to_skills_md)
    }

    /// Reads through current access to the original filesystem; cached metadata grants no access.
    pub(crate) async fn read_skill_text(
        &self,
        skill: &SkillMetadata,
        accessors: &[&dyn EnvironmentAccess],
    ) -> io::Result<String> {
        let host = FileSystemEnvironmentAccessor::unrestricted(&LOCAL_FS);
        let access: &dyn EnvironmentAccess = match skill.scope {
            SkillScope::Repo => self
                .file_system_for_skill(skill)
                .and_then(|source| {
                    accessors
                        .iter()
                        .copied()
                        .find(|access| source.same_file_system(&access.cache_key()))
                })
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "skill discovery filesystem is unavailable for this callback",
                    )
                })?,
            SkillScope::User | SkillScope::System | SkillScope::Admin => &host,
        };
        access
            .read_file_text(&skill.path_to_skills_md, ReadFileOptions::default())
            .await
    }
}

impl codex_skills::ImplicitSkillLookup for SkillLoadOutcome {
    fn implicit_skill_for_scripts_dir(&self, path: &AbsolutePathBuf) -> Option<&SkillMetadata> {
        self.implicit_skills_by_scripts_dir
            .get(&PathUri::from_abs_path(path))
    }

    fn implicit_skill_for_doc_path(&self, path: &AbsolutePathBuf) -> Option<&SkillMetadata> {
        self.implicit_skills_by_doc_path
            .get(&PathUri::from_abs_path(path))
    }
}

impl codex_skills::ExplicitSkillLookup for SkillLoadOutcome {
    fn skills(&self) -> &[SkillMetadata] {
        &self.skills
    }

    fn disabled_paths(&self) -> &HashSet<PathUri> {
        &self.disabled_paths
    }

    fn skill_discovery_path_for_path(&self, path: &PathUri) -> Option<&PathUri> {
        SkillLoadOutcome::skill_discovery_path_for_path(self, path)
    }

    fn is_skill_enabled(&self, skill: &SkillMetadata) -> bool {
        SkillLoadOutcome::is_skill_enabled(self, skill)
    }
}

#[derive(Clone, Default)]
pub(crate) struct SkillFileSystemsByPath {
    values: Arc<HashMap<PathUri, EnvironmentAccessKey>>,
}

impl SkillFileSystemsByPath {
    pub(crate) fn new(values: HashMap<PathUri, EnvironmentAccessKey>) -> Self {
        Self {
            values: Arc::new(values),
        }
    }

    fn get(&self, path: &PathUri) -> Option<&EnvironmentAccessKey> {
        self.values.get(path)
    }
}

impl fmt::Debug for SkillFileSystemsByPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SkillFileSystemsByPath")
            .field("len", &self.values.len())
            .finish()
    }
}
