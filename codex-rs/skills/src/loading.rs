use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::hash::Hash;
use std::hash::Hasher;
use std::pin::Pin;
use std::sync::Arc;

use codex_protocol::protocol::Product;
use codex_utils_absolute_path::AbsolutePathBuf;

use crate::SkillMetadata;

/// A skill document that could not be read, parsed, or validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillError {
    pub path: AbsolutePathBuf,
    pub message: String,
}

/// Filesystem-independent metadata discovered beneath one canonical skill root.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedSkillRoot {
    pub root: AbsolutePathBuf,
    pub skills: Vec<SkillMetadata>,
    pub skill_discovery_path_by_path: Arc<HashMap<AbsolutePathBuf, AbsolutePathBuf>>,
    pub errors: Vec<SkillError>,
    pub is_agent_plugin: bool,
}

/// Skills and errors produced by loading an ordered set of roots.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LoadedSkills {
    pub skills: Vec<SkillMetadata>,
    pub errors: Vec<SkillError>,
}

/// Caches parsed roots without assigning ownership of the cache to a loader.
///
/// Implementations own storage and determine its lifetime; loaders may reuse an existing snapshot
/// or publish a newly loaded one without learning about the caller's broader state.
pub trait SkillRootSnapshotCache<Root>: Send + Sync {
    fn get(&self, root: &Root) -> Option<LoadedSkillRoot>;

    fn insert(&self, root: Root, snapshot: LoadedSkillRoot);
}

/// Shared access to one owner-managed collection of parsed skill roots.
///
/// Equality and hashing use the cache allocation's identity so callers can include the handle in
/// their own cache keys without inspecting or taking ownership of its contents.
pub struct SkillRootSnapshots<Root> {
    cache: Arc<dyn SkillRootSnapshotCache<Root>>,
}

impl<Root> SkillRootSnapshots<Root> {
    pub fn new(cache: Arc<dyn SkillRootSnapshotCache<Root>>) -> Self {
        Self { cache }
    }

    pub fn get(&self, root: &Root) -> Option<LoadedSkillRoot> {
        self.cache.get(root)
    }

    pub fn insert(&self, root: Root, snapshot: LoadedSkillRoot) {
        self.cache.insert(root, snapshot);
    }
}

impl<Root> Clone for SkillRootSnapshots<Root> {
    fn clone(&self) -> Self {
        Self {
            cache: Arc::clone(&self.cache),
        }
    }
}

impl<Root> PartialEq for SkillRootSnapshots<Root> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.cache, &other.cache)
    }
}

impl<Root> Eq for SkillRootSnapshots<Root> {}

impl<Root> Hash for SkillRootSnapshots<Root> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.cache) as *const ()).hash(state);
    }
}

impl<Root> fmt::Debug for SkillRootSnapshots<Root> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SkillRootSnapshots").finish_non_exhaustive()
    }
}

/// Inputs for loading roots while optionally reusing snapshots owned by the caller.
#[derive(Debug, Clone)]
pub struct SkillRootLoadRequest<Root> {
    pub roots: Vec<Root>,
    pub restriction_product: Option<Product>,
    pub snapshots: Option<SkillRootSnapshots<Root>>,
}

/// Object-safe future returned by a shared skill-root loader.
pub type SkillLoadFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Loads ordered skill roots without owning their source inventory or capability state.
///
/// Implementations are responsible for filesystem access, metadata parsing, root precedence,
/// product filtering, and reuse of any snapshots supplied in the request.
pub trait SkillRootLoader<Root>: Send + Sync {
    fn load_roots(&self, request: SkillRootLoadRequest<Root>) -> SkillLoadFuture<'_, LoadedSkills>;
}

#[cfg(test)]
#[path = "loading_tests.rs"]
mod tests;
