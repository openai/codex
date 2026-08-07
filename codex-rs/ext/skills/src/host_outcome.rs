use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::io;
use std::sync::Arc;

use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::LOCAL_FS;
use codex_skills::SkillError;
use codex_skills::SkillMetadata;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;

#[derive(Debug, Clone, Default)]
pub struct SkillLoadOutcome {
    pub skills: Vec<SkillMetadata>,
    pub errors: Vec<SkillError>,
    pub disabled_paths: HashSet<AbsolutePathBuf>,
    pub(crate) skill_roots: Vec<AbsolutePathBuf>,
    pub(crate) skill_root_by_path: Arc<HashMap<AbsolutePathBuf, AbsolutePathBuf>>,
    pub(crate) skill_discovery_path_by_path: Arc<HashMap<AbsolutePathBuf, AbsolutePathBuf>>,
    pub(crate) agent_plugin_skill_paths: HashSet<AbsolutePathBuf>,
    pub(crate) file_systems_by_skill_path: SkillFileSystemsByPath,
    pub(crate) implicit_skills_by_scripts_dir: Arc<HashMap<AbsolutePathBuf, SkillMetadata>>,
    pub(crate) implicit_skills_by_doc_path: Arc<HashMap<AbsolutePathBuf, SkillMetadata>>,
}

impl SkillLoadOutcome {
    /// Builds an already-composed outcome while retaining the filesystem that supplied each skill.
    pub fn from_parts(
        skills: Vec<SkillMetadata>,
        errors: Vec<SkillError>,
        skill_roots: Vec<AbsolutePathBuf>,
        skill_root_by_path: HashMap<AbsolutePathBuf, AbsolutePathBuf>,
        skill_discovery_path_by_path: HashMap<AbsolutePathBuf, AbsolutePathBuf>,
        agent_plugin_skill_paths: HashSet<AbsolutePathBuf>,
        file_systems_by_skill_path: HashMap<AbsolutePathBuf, Arc<dyn ExecutorFileSystem>>,
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

    pub fn is_skill_allowed_for_implicit_invocation(&self, skill: &SkillMetadata) -> bool {
        self.is_skill_enabled(skill) && skill.allows_implicit_invocation()
    }

    pub fn allowed_skills_for_implicit_invocation(&self) -> Vec<SkillMetadata> {
        self.skills
            .iter()
            .filter(|skill| self.is_skill_allowed_for_implicit_invocation(skill))
            .cloned()
            .collect()
    }

    pub fn skills_with_enabled(&self) -> impl Iterator<Item = (&SkillMetadata, bool)> {
        self.skills
            .iter()
            .map(|skill| (skill, self.is_skill_enabled(skill)))
    }

    pub fn with_disabled_paths(mut self, disabled_paths: HashSet<AbsolutePathBuf>) -> Self {
        self.disabled_paths = disabled_paths;
        let mut by_scripts_dir = HashMap::new();
        let mut by_doc_path = HashMap::new();
        for skill in self
            .skills
            .iter()
            .filter(|skill| self.is_skill_enabled(skill))
        {
            let skill_doc_path = canonicalize_if_exists(&skill.path_to_skills_md);
            by_doc_path.insert(skill_doc_path, skill.clone());

            if let Some(skill_dir) = skill.path_to_skills_md.parent() {
                let scripts_dir = canonicalize_if_exists(&skill_dir.join("scripts"));
                by_scripts_dir.insert(scripts_dir, skill.clone());
            }
        }
        self.implicit_skills_by_scripts_dir = Arc::new(by_scripts_dir);
        self.implicit_skills_by_doc_path = Arc::new(by_doc_path);
        self
    }

    pub fn is_agent_plugin_skill(&self, skill: &SkillMetadata) -> bool {
        self.agent_plugin_skill_paths
            .contains(&skill.path_to_skills_md)
    }

    /// Returns the discovery root that supplied a loaded skill path.
    pub fn skill_root_for_path(&self, path: &AbsolutePathBuf) -> Option<&AbsolutePathBuf> {
        self.skill_root_by_path.get(path)
    }

    /// Returns the logical path used to discover a canonical skill path.
    pub fn skill_discovery_path_for_path(
        &self,
        path: &AbsolutePathBuf,
    ) -> Option<&AbsolutePathBuf> {
        self.skill_discovery_path_by_path.get(path)
    }

    /// Returns loaded skill roots in discovery order.
    pub fn skill_roots_in_discovery_order(&self) -> impl Iterator<Item = &AbsolutePathBuf> {
        self.skill_roots.iter()
    }

    pub(crate) fn file_system_for_skill(
        &self,
        skill: &SkillMetadata,
    ) -> Option<Arc<dyn ExecutorFileSystem>> {
        self.file_systems_by_skill_path
            .get(&skill.path_to_skills_md)
    }

    /// Reads one loaded skill through the filesystem that discovered it.
    pub async fn read_skill_text(&self, skill: &SkillMetadata) -> io::Result<String> {
        let fs = self
            .file_system_for_skill(skill)
            .unwrap_or_else(|| Arc::clone(&LOCAL_FS));
        let path = PathUri::from_abs_path(&skill.path_to_skills_md);
        fs.read_file_text(&path, /*sandbox*/ None).await
    }
}

impl codex_skills::ImplicitSkillLookup for SkillLoadOutcome {
    fn implicit_skill_for_scripts_dir(&self, path: &AbsolutePathBuf) -> Option<&SkillMetadata> {
        self.implicit_skills_by_scripts_dir.get(path)
    }

    fn implicit_skill_for_doc_path(&self, path: &AbsolutePathBuf) -> Option<&SkillMetadata> {
        self.implicit_skills_by_doc_path.get(path)
    }
}

impl codex_skills::ExplicitSkillLookup for SkillLoadOutcome {
    fn skills(&self) -> &[SkillMetadata] {
        &self.skills
    }

    fn disabled_paths(&self) -> &HashSet<AbsolutePathBuf> {
        &self.disabled_paths
    }

    fn skill_discovery_path_for_path(&self, path: &AbsolutePathBuf) -> Option<&AbsolutePathBuf> {
        SkillLoadOutcome::skill_discovery_path_for_path(self, path)
    }

    fn is_skill_enabled(&self, skill: &SkillMetadata) -> bool {
        SkillLoadOutcome::is_skill_enabled(self, skill)
    }
}

#[derive(Clone, Default)]
pub(crate) struct SkillFileSystemsByPath {
    values: Arc<HashMap<AbsolutePathBuf, Arc<dyn ExecutorFileSystem>>>,
}

impl SkillFileSystemsByPath {
    pub(crate) fn new(values: HashMap<AbsolutePathBuf, Arc<dyn ExecutorFileSystem>>) -> Self {
        Self {
            values: Arc::new(values),
        }
    }

    fn get(&self, path: &AbsolutePathBuf) -> Option<Arc<dyn ExecutorFileSystem>> {
        self.values.get(path).map(Arc::clone)
    }
}

impl fmt::Debug for SkillFileSystemsByPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SkillFileSystemsByPath")
            .field("len", &self.values.len())
            .finish()
    }
}

fn canonicalize_if_exists(path: &AbsolutePathBuf) -> AbsolutePathBuf {
    path.canonicalize().unwrap_or_else(|_| path.clone())
}
