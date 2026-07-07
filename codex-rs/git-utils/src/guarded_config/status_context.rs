use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::sync::Arc;

use super::CapabilityIdentity;
use super::GuardedGitConfig;
use super::command_failure;
use crate::git_command::IsolatedGitReadContext;
use crate::git_command::MAX_INTERNAL_GIT_OUTPUT_BYTES;
use crate::git_config::parse_git_boolean;
use crate::git_config::read_effective_config_with_implicit_booleans_async;
const STATUS_SAFE_CONFIG_PATTERN: &str = r"^(attr\.tree|core\.(filemode|symlinks|ignorecase|precomposeunicode|protecthfs|protectntfs|trustctime|checkstat|longpaths|fscache|splitindex|sparsecheckout|sparsecheckoutcone|autocrlf|eol|safecrlf|checkroundtripencoding|bigfilethreshold|quotepath|abbrev)|index\.(sparse|version))$";
const STATUS_IMPLICIT_BOOLEAN_KEYS: &[&str] = &[
    "core.filemode",
    "core.symlinks",
    "core.ignorecase",
    "core.precomposeunicode",
    "core.protecthfs",
    "core.protectntfs",
    "core.trustctime",
    "core.longpaths",
    "core.fscache",
    "core.splitindex",
    "core.sparsecheckout",
    "core.sparsecheckoutcone",
    "core.autocrlf",
    "core.safecrlf",
    "core.quotepath",
    "index.sparse",
];
const REPOSITORY_FORMAT_CONFIG_PATTERN: &str =
    r"^(core\.repositoryformatversion|extensions\.(objectformat|compatobjectformat))$";
const MAX_STATUS_ATTRIBUTE_SOURCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_STATUS_PROJECTED_CONFIG_BYTES: usize = 1024 * 1024;
const MAX_STATUS_PROJECTED_CONFIG_ENTRIES: usize = 256;

pub(super) struct SealedStatusReadContext {
    owner: Arc<CapabilityIdentity>,
    context: IsolatedGitReadContext,
    has_untracked: bool,
}

impl SealedStatusReadContext {
    pub(super) fn context(
        &self,
        owner: &Arc<CapabilityIdentity>,
    ) -> io::Result<&IsolatedGitReadContext> {
        if !Arc::ptr_eq(&self.owner, owner) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "sealed Status read context belongs to another operation",
            ));
        }
        Ok(&self.context)
    }

    pub(super) fn has_untracked(&self, owner: &Arc<CapabilityIdentity>) -> io::Result<bool> {
        let _ = self.context(owner)?;
        Ok(self.has_untracked)
    }

    #[cfg(test)]
    pub(super) fn config_path(
        &self,
        owner: &Arc<CapabilityIdentity>,
    ) -> io::Result<std::path::PathBuf> {
        Ok(self.context(owner)?.config_path())
    }

    #[cfg(test)]
    pub(super) fn attributes_path(
        &self,
        owner: &Arc<CapabilityIdentity>,
    ) -> io::Result<std::path::PathBuf> {
        Ok(self.context(owner)?.attributes_path())
    }
}

impl GuardedGitConfig<'_> {
    pub(super) async fn build_status_read_context_async(
        &self,
        head_oid: Option<&str>,
        has_untracked: bool,
        configured_core_symlinks: Option<bool>,
    ) -> io::Result<SealedStatusReadContext> {
        if self.sources.git.attribute_source_is_custom() {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "frozen Status is unavailable with GIT_ATTR_SOURCE",
            ));
        }
        let info_attributes = self
            .sources
            .git
            .read_active_info_attributes_bounded_async(MAX_STATUS_ATTRIBUTE_SOURCE_BYTES)
            .await?;
        let core_attributes_file = self.read_status_core_attributes_file_async().await?;
        let operation_identity = self.operation_identity();
        let context = self
            .sources
            .git
            .create_isolated_read_context(head_oid, &operation_identity)?;
        self.write_status_projection_config_async(
            &context,
            core_attributes_file.as_deref(),
            configured_core_symlinks,
        )
        .await?;
        std::fs::write(
            context.attributes_path(),
            status_info_attributes(info_attributes)?,
        )?;
        let context = context.seal_projected_files()?;
        Ok(SealedStatusReadContext {
            owner: Arc::clone(&self.identity),
            context,
            has_untracked,
        })
    }

    async fn write_status_projection_config_async(
        &self,
        context: &IsolatedGitReadContext,
        core_attributes_file: Option<&OsStr>,
        configured_core_symlinks: Option<bool>,
    ) -> io::Result<()> {
        let entries = read_effective_config_with_implicit_booleans_async(
            self.sources.git,
            &self.sources.canonical_root,
            &self.sources.base_config_args,
            STATUS_SAFE_CONFIG_PATTERN,
            "status allowlist",
            STATUS_IMPLICIT_BOOLEAN_KEYS,
        )
        .await?;
        let repository_format = self
            .sources
            .read_direct_common_config_async(REPOSITORY_FORMAT_CONFIG_PATTERN, "repository format")
            .await?;
        if entries.len().saturating_add(repository_format.len())
            > MAX_STATUS_PROJECTED_CONFIG_ENTRIES
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "status config projection exceeds its entry limit",
            ));
        }
        refuse_sparse_status(&entries)?;
        let projected_core_symlinks = configured_status_core_symlinks(&entries)?;
        if projected_core_symlinks != configured_core_symlinks {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "core.symlinks changed during Status preparation",
            ));
        }
        let projected_bytes = entries.iter().chain(repository_format.iter()).try_fold(
            0_usize,
            |total, (key, entry)| {
                total
                    .checked_add(key.len())
                    .and_then(|total| total.checked_add(entry.value.len()))
                    .and_then(|total| total.checked_add(2))
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "status config projection size overflow",
                        )
                    })
            },
        )?;
        if projected_bytes > MAX_STATUS_PROJECTED_CONFIG_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "status config projection exceeds its byte limit",
            ));
        }
        for entry in entries
            .values()
            .filter(|entry| entry.key != "core.symlinks")
        {
            self.write_status_config_value_async(&context.config_path(), &entry.key, &entry.value)
                .await?;
        }
        for entry in repository_format.values() {
            self.write_status_config_value_async(&context.config_path(), &entry.key, &entry.value)
                .await?;
        }
        self.write_status_config_value_async(&context.config_path(), "core.bare", "false")
            .await?;
        if let Some(core_symlinks) = configured_core_symlinks {
            self.write_status_config_value_async(
                &context.config_path(),
                "core.symlinks",
                if core_symlinks { "true" } else { "false" },
            )
            .await?;
        }
        if let Some(core_attributes_file) = core_attributes_file {
            self.write_status_config_value_async(
                &context.config_path(),
                "core.attributesfile",
                core_attributes_file,
            )
            .await?;
        }
        if std::fs::metadata(context.config_path())?.len()
            > MAX_STATUS_PROJECTED_CONFIG_BYTES as u64
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "serialized status config projection exceeds its byte limit",
            ));
        }
        Ok(())
    }

    async fn write_status_config_value_async(
        &self,
        config_path: &Path,
        key: &str,
        value: impl AsRef<OsStr>,
    ) -> io::Result<()> {
        let mut command = self
            .sources
            .git
            .async_config_file_write_command(config_path);
        command.arg(key).arg(value);
        let output = self
            .sources
            .git
            .output_async_bounded(command, MAX_INTERNAL_GIT_OUTPUT_BYTES)
            .await?;
        if output.status.success() {
            Ok(())
        } else {
            Err(command_failure("status config projection write", &output))
        }
    }

    /// Resolve the effective configured-global attribute selector using the
    /// path semantics available on the oldest supported Git. The returned
    /// absolute bytes are written into the owned config so a relative or `~`
    /// spelling cannot be reinterpreted against the synthetic config file.
    async fn read_status_core_attributes_file_async(&self) -> io::Result<Option<OsString>> {
        let mut command = self.pending_status_command(
            crate::FsmonitorOverride::Disabled,
            /*neutralizer*/ None,
        )?;
        command.disable_optional_locks().args([
            "config",
            "--null",
            "--path",
            "--get",
            "core.attributesFile",
        ]);
        let output = command.output().await?;
        if output.status.code() == Some(1) && output.stdout.is_empty() && output.stderr.is_empty() {
            return Ok(None);
        }
        if !output.status.success() {
            return Err(command_failure(
                "status core.attributesFile discovery",
                &output,
            ));
        }
        let configured = parse_status_config_path(&output.stdout)?;
        if configured.is_empty() {
            return Ok(Some(configured));
        }
        let configured_path = std::path::PathBuf::from(&configured);
        if configured_path.is_absolute() {
            return Ok(Some(configured));
        }
        if configured_path
            .components()
            .any(|component| matches!(component, std::path::Component::Prefix(_)))
        {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "frozen Status is unavailable with a drive-relative core.attributesFile",
            ));
        }
        Ok(Some(
            self.sources
                .canonical_root
                .join(configured_path)
                .into_os_string(),
        ))
    }
}

fn configured_status_core_symlinks(
    entries: &BTreeMap<String, crate::git_config::GitConfigEntry>,
) -> io::Result<Option<bool>> {
    let Some(entry) = entries.get("core.symlinks") else {
        return Ok(None);
    };
    parse_git_boolean(entry.value.as_bytes())
        .map(Some)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid Status config value for core.symlinks",
            )
        })
}

fn refuse_sparse_status(
    entries: &BTreeMap<String, crate::git_config::GitConfigEntry>,
) -> io::Result<()> {
    if entries.contains_key("attr.tree") {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "frozen Status is unavailable with attr.tree",
        ));
    }
    for key in [
        "core.sparsecheckout",
        "core.sparsecheckoutcone",
        "index.sparse",
    ] {
        let Some(entry) = entries.get(key) else {
            continue;
        };
        match parse_git_boolean(entry.value.as_bytes()) {
            Some(false) | Some(true) if key == "core.sparsecheckoutcone" => {}
            Some(false) => {}
            Some(true) => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "frozen Status is unavailable for sparse-checkout repositories",
                ));
            }
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid sparse Status config value for {key}"),
                ));
            }
        }
    }
    Ok(())
}

fn status_info_attributes(mut original: Vec<u8>) -> io::Result<Vec<u8>> {
    const RESET: &[u8] = b"* !filter !diff\n";
    if !original.is_empty() && !original.ends_with(b"\n") {
        original.push(b'\n');
    }
    if original.len().saturating_add(RESET.len()) > MAX_STATUS_ATTRIBUTE_SOURCE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "owned Git info attributes exceed their byte limit",
        ));
    }
    original.extend_from_slice(RESET);
    Ok(original)
}

fn parse_status_config_path(output: &[u8]) -> io::Result<OsString> {
    let value = output.strip_suffix(&[0]).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "unterminated Git core.attributesFile path output",
        )
    })?;
    if value.contains(&0) || value.contains(&b'\n') || value.contains(&b'\r') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "ambiguous Git core.attributesFile path output",
        ));
    }
    crate::safe_git::git_path_argument(value)
}

#[cfg(test)]
#[path = "status_context_tests.rs"]
mod tests;
