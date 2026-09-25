//! Executor-context policy operations that never consult controller paths or environment.

use super::FileSystemAccessMode;
use super::FileSystemPath;
use super::FileSystemSandboxEntry;
use super::FileSystemSandboxKind;
use super::FileSystemSandboxPolicy;
use super::FileSystemSandboxPolicyContext;
use super::FileSystemSpecialPath;
use super::InvalidDenyReadGlobBehavior;
use super::PROJECT_ROOTS_GLOB_PATTERN_PREFIX;
use super::PROTECTED_METADATA_PATH_NAMES;
use super::ReadDenyMatcher;
use super::deny_read_validator::DenyReadValidator;
use super::deny_read_validator::has_context_read_denials;
use super::file_system_root;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;
use std::collections::HashSet;

const INVALID_PATH: &str = "managed filesystem denial cannot be materialized safely";
const SHADOWED: &str = "managed filesystem denial is shadowed by a selected profile grant";

impl ReadDenyMatcher {
    /// Builds a strict matcher using only the supplied execution-host paths.
    pub fn try_new_with_context(
        policy: &FileSystemSandboxPolicy,
        context: &FileSystemSandboxPolicyContext<'_>,
    ) -> Result<Option<Self>, String> {
        if !has_context_read_denials(policy, context) {
            return Ok(None);
        }
        validate_context_paths(policy, context)?;
        let prepared =
            policy.prepare_deny_read_matcher(context, InvalidDenyReadGlobBehavior::ReturnError)?;
        Ok(Some(Self {
            native_cwd: None,
            user_home_dir: None,
            temporary_directories: Vec::new(),
            prepared,
        }))
    }
}

impl FileSystemSandboxPolicy {
    /// Grants root and root-metadata write for an approved command unless explicitly denied.
    /// If a retained denial cannot be resolved or denies the root, keep the original policy.
    pub fn for_approved_command(&self, context: &FileSystemSandboxPolicyContext<'_>) -> Self {
        if self.kind != FileSystemSandboxKind::Restricted {
            return self.clone();
        }
        let mut approved = Self::restricted(vec![FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            FileSystemAccessMode::Write,
        )]);
        approved.preserve_deny_read_restrictions_from(self);
        if context.workspace_roots.is_empty()
            && approved.entries.iter().any(|entry| {
                matches!(
                    &entry.path,
                    FileSystemPath::Special {
                        value: FileSystemSpecialPath::ProjectRoots { .. }
                    }
                ) || matches!(
                    &entry.path,
                    FileSystemPath::GlobPattern { pattern }
                        if pattern.starts_with(PROJECT_ROOTS_GLOB_PATTERN_PREFIX)
                )
            })
        {
            return self.clone();
        }
        let Some(mut approved) =
            approved.try_materialize_project_roots_with_path_uris(context.workspace_roots)
        else {
            return self.clone();
        };
        let Some(root) = file_system_root(context) else {
            return self.clone();
        };
        let Ok(matcher) = ReadDenyMatcher::try_new_with_context(&approved, context) else {
            return self.clone();
        };
        if !approved.resolve_access(&root, context).can_write()
            || matcher
                .as_ref()
                .is_some_and(|matcher| matcher.is_read_denied_uri(&root, context))
        {
            return self.clone();
        }
        for name in PROTECTED_METADATA_PATH_NAMES {
            let Ok(path) = root.join_descendant(name) else {
                return self.clone();
            };
            if !matcher
                .as_ref()
                .is_some_and(|matcher| matcher.is_read_denied_uri(&path, context))
            {
                approved.entries.push(FileSystemSandboxEntry::new(
                    path.into(),
                    FileSystemAccessMode::Write,
                ));
            }
        }
        approved
    }

    /// Checks exact-root read access using paths owned by the execution host.
    /// Deny glob enforcement is checked separately with [`ReadDenyMatcher`].
    pub fn can_read_path(
        &self,
        path: &PathUri,
        context: &FileSystemSandboxPolicyContext<'_>,
    ) -> bool {
        self.resolve_access(path, context).can_read()
    }

    /// Preserves workspace-root symbols and adds their executor-owned concrete paths.
    /// Legacy home-relative workspace denials clear all grants when their target is unknown.
    pub fn with_materialized_project_roots_for_path_uris(mut self, roots: &[PathUri]) -> Self {
        let Some(materialized) = self
            .clone()
            .try_materialize_project_roots_with_path_uris(roots)
        else {
            return Self::restricted(Vec::new());
        };
        for entry in materialized.entries {
            if !self.entries.contains(&entry) {
                self.entries.push(entry);
            }
        }
        self
    }

    /// Builds workspace-write defaults without inspecting the execution host's filesystem.
    pub fn workspace_write_with_path_uris(
        roots: &[PathUri],
        exclude_tmpdir_env_var: bool,
        exclude_slash_tmp: bool,
    ) -> Self {
        Self::workspace_write(&[], exclude_tmpdir_env_var, exclude_slash_tmp)
            .with_materialized_project_roots_for_path_uris(roots)
    }

    /// Returns explicitly denied roots using only the selected executor's paths.
    /// A filesystem-root denial is omitted so it does not obscure narrower readable paths.
    /// Fails rather than omit a denied temporary directory the executor did not report.
    pub fn get_unreadable_roots_with_context(
        &self,
        context: &FileSystemSandboxPolicyContext<'_>,
    ) -> Result<Vec<PathUri>, String> {
        if !matches!(self.kind, FileSystemSandboxKind::Restricted) {
            return Ok(Vec::new());
        }
        if context.temporary_directories.is_none()
            && self.entries.iter().any(|entry| {
                entry.access == FileSystemAccessMode::Deny
                    && matches!(
                        &entry.path,
                        FileSystemPath::Special {
                            value: FileSystemSpecialPath::Tmpdir
                        }
                    )
            })
        {
            return Err("executor did not report its denied temporary directories".into());
        }
        let filesystem_root = file_system_root(context);
        let mut seen = HashSet::new();
        Ok(self
            .resolved_entries(context)
            .into_iter()
            .filter(|(path, access)| {
                *access == FileSystemAccessMode::Deny
                    && !filesystem_root
                        .as_ref()
                        .is_some_and(|root| path.starts_with(root) && root.starts_with(path))
                    && seen.insert(path.clone())
            })
            .map(|(path, _)| path)
            .collect())
    }

    /// Resolves deny glob text using the execution host's convention and home.
    pub fn get_unreadable_globs_with_context(
        &self,
        context: &FileSystemSandboxPolicyContext<'_>,
    ) -> Result<Vec<String>, String> {
        self.deny_read_globs(context)
    }

    /// Checks the existing Core rules for required entries and readable concrete paths.
    ///
    /// Sandbox-mode selection and native glob expansion remain caller-owned.
    pub fn validate_managed_deny_read(
        &self,
        required: &Self,
        context: &FileSystemSandboxPolicyContext<'_>,
    ) -> Result<(), String> {
        DenyReadValidator::new(required, context)?
            .validate(self, context)
            .map_err(|_| SHADOWED.to_string())
    }
}

fn validate_context_paths(
    policy: &FileSystemSandboxPolicy,
    context: &FileSystemSandboxPolicyContext<'_>,
) -> Result<(), String> {
    let convention = context.cwd.infer_path_convention().ok_or(INVALID_PATH)?;
    let valid = |path: &PathUri| {
        path.infer_path_convention() == Some(convention) && path.lexical_depth().is_some()
    };
    if !valid(context.cwd) || context.workspace_roots.iter().any(|path| !valid(path)) {
        return Err(INVALID_PATH.into());
    }
    for entry in &policy.entries {
        match &entry.path {
            FileSystemPath::Path { path } if !valid(path) => return Err(INVALID_PATH.into()),
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Tmpdir,
            } => {
                let directories = context.temporary_directories.ok_or(
                    "executor temporary-directory metadata is unavailable for managed filesystem denials",
                )?;
                if directories.iter().any(|path| !valid(path)) {
                    return Err(
                        "executor temporary-directory metadata cannot be materialized safely"
                            .into(),
                    );
                }
            }
            FileSystemPath::Special {
                value:
                    FileSystemSpecialPath::ProjectRoots {
                        subpath: Some(subpath),
                    },
            } if context
                .workspace_roots
                .iter()
                .any(|root| root.join(subpath).is_err()) =>
            {
                return Err(INVALID_PATH.into());
            }
            FileSystemPath::GlobPattern { pattern }
                if (pattern.starts_with("~/")
                    || convention == PathConvention::Windows && pattern.starts_with("~\\"))
                    && !context.user_home_dir.is_some_and(valid) =>
            {
                return Err(INVALID_PATH.into());
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "target_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "target_approved_materialization_tests.rs"]
mod approved_materialization_tests;
