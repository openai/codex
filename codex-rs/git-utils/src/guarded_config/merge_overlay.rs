use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io;
use std::io::Write;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;

use super::BoundSubcommand;
use super::CapabilityIdentity;
use super::GuardedGitConfig;
use crate::git_command::IsolatedGitCommonDir;
use crate::git_command::IsolatedGitStorage;
use crate::git_config::GitConfigEntry;
use crate::git_config::GitConfigValue;
use crate::git_config::MergeConfigRecord;
use crate::git_config::parse_git_boolean_symmetric_i32;
const REPOSITORY_FORMAT_CONFIG_PATTERN: &str =
    r"^(core\.repositoryformatversion|extensions\.(objectformat|compatobjectformat))$";
const SANITIZED_CONFIG_PATTERN: &str = r"^(core\.(filemode|symlinks|ignorecase|precomposeunicode|protecthfs|protectntfs|trustctime|checkstat|longpaths|fscache|splitindex|sparsecheckout|sparsecheckoutcone|autocrlf|eol|safecrlf|checkroundtripencoding|bigfilethreshold)|index\.(sparse|version)|merge\.conflictstyle)$";
// Git ignores an attribute line whose content length is at least this value.
const GIT_ATTRIBUTE_LINE_LENGTH_LIMIT: usize = 2048;

/// A complete, fresh merge-config read bound to one authorized operation.
///
/// The fields are private so callers cannot mint a partial driver inventory
/// and then ask the capability to treat its neutralizer as complete.
struct MergeConfigSnapshot {
    owner: Arc<CapabilityIdentity>,
    default: Option<String>,
    namespaces: BTreeSet<String>,
    conditional_invalid: bool,
}

/// The helper-free merge behavior projected into the app-owned attribute
/// layer. A configured selected custom driver is retained as a distinct state
/// and quarantined as Git's built-in binary driver in both probe and final
/// children.
#[derive(Clone, Debug, Eq, PartialEq)]
enum MergeSelection {
    Builtin(BuiltinMergeDriver),
    QuarantinedCustom { name: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BuiltinMergeDriver {
    Text,
    Binary,
    Union,
}

impl BuiltinMergeDriver {
    fn from_effective_attribute(attribute: &str, sanitized_default: &str) -> Self {
        let selected = if attribute == "unspecified" {
            sanitized_default
        } else {
            attribute
        };
        match selected {
            "unset" | "binary" => Self::Binary,
            "union" => Self::Union,
            // A set attribute, the explicit text driver, and an unconfigured
            // name with no user-driver namespace fall back to Git's built-in
            // text merge semantics.
            _ => Self::Text,
        }
    }

    fn attribute_value(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Binary => "binary",
            Self::Union => "union",
        }
    }
}

/// Per-path apply semantics frozen at the strict `check-attr` probe.
///
/// Git's public `check-attr` format conflates special states with the literal
/// values `set`, `unset`, and `unspecified`. This snapshot intentionally uses
/// Git's conventional rendering for those uncommon literal spellings. Every
/// other safely representable value is retained byte-for-byte.
struct MergeAttributeSnapshot {
    owner: Arc<CapabilityIdentity>,
    paths: BTreeMap<String, FrozenPathApplyAttributes>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FrozenPathApplyAttributes {
    merge: MergeSelection,
    safe: SafeApplyAttributes,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ParsedPathApplyAttributes {
    merge: String,
    safe: SafeApplyAttributes,
}

/// A helper-free attribute state that can be projected into an app-owned
/// `info/attributes` file without being reinterpreted as config or a command.
#[derive(Clone, Debug, Eq, PartialEq)]
enum ProjectedAttribute {
    Set,
    Unset,
    Unspecified,
    Value(Vec<u8>),
}

impl ProjectedAttribute {
    fn from_check_attr(value: &[u8]) -> io::Result<Self> {
        // `git check-attr` deliberately renders both special states and the
        // literal values `=set`, `=unset`, and `=unspecified` with the same
        // sentinel text. Match Git's documented convention for this bounded
        // PR while retaining all other raw values.
        match value {
            b"set" => Ok(Self::Set),
            b"unset" => Ok(Self::Unset),
            b"unspecified" => Ok(Self::Unspecified),
            value => {
                validate_projected_attribute_value(value)?;
                Ok(Self::Value(value.to_vec()))
            }
        }
    }

    fn projected_token(&self, name: &str) -> Vec<u8> {
        match self {
            Self::Set => name.as_bytes().to_vec(),
            Self::Unset => format!("-{name}").into_bytes(),
            Self::Unspecified => format!("!{name}").into_bytes(),
            Self::Value(value) => {
                let mut token = Vec::with_capacity(name.len() + 1 + value.len());
                token.extend_from_slice(name.as_bytes());
                token.push(b'=');
                token.extend_from_slice(value);
                token
            }
        }
    }
}

/// Safe in-process attributes consumed by Git 2.54's apply, low-level merge,
/// and checkout conversion paths. `filter` is captured only to make the fixed
/// query complete; projection always replaces it with `!filter`.
#[derive(Clone, Debug, Eq, PartialEq)]
struct SafeApplyAttributes {
    whitespace: ProjectedAttribute,
    conflict_marker_size: ProjectedAttribute,
    text: ProjectedAttribute,
    crlf: ProjectedAttribute,
    eol: ProjectedAttribute,
    ident: ProjectedAttribute,
    working_tree_encoding: ProjectedAttribute,
    _filter: ProjectedAttribute,
}

impl MergeAttributeSnapshot {
    fn from_effective(
        owner: &Arc<CapabilityIdentity>,
        config: &MergeConfigSnapshot,
        attributes: BTreeMap<String, ParsedPathApplyAttributes>,
        selected_custom: &BTreeMap<String, String>,
    ) -> io::Result<Self> {
        config.ensure_owner(owner)?;
        let sanitized_default = config.sanitized_default_driver();
        let paths = attributes
            .into_iter()
            .map(|(path, attributes)| {
                let merge = selected_custom
                    .get(&path)
                    .map(|name| MergeSelection::QuarantinedCustom { name: name.clone() })
                    .unwrap_or_else(|| {
                        MergeSelection::Builtin(BuiltinMergeDriver::from_effective_attribute(
                            &attributes.merge,
                            sanitized_default,
                        ))
                    });
                (
                    path,
                    FrozenPathApplyAttributes {
                        merge,
                        safe: attributes.safe,
                    },
                )
            })
            .collect();
        Ok(Self {
            owner: Arc::clone(owner),
            paths,
        })
    }

    fn ensure_owner(&self, owner: &Arc<CapabilityIdentity>) -> io::Result<()> {
        if !Arc::ptr_eq(&self.owner, owner) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "merge attribute snapshot belongs to another operation",
            ));
        }
        Ok(())
    }
}

impl MergeConfigSnapshot {
    fn from_records(
        owner: &Arc<CapabilityIdentity>,
        records: Vec<MergeConfigRecord>,
    ) -> io::Result<Self> {
        let mut default = None;
        let mut namespaces = BTreeSet::new();
        let mut conditional_invalid = false;
        for record in records {
            if record.key == "merge.default" {
                match record.value {
                    GitConfigValue::Implicit => conditional_invalid = true,
                    GitConfigValue::Explicit(value) => default = Some(value),
                }
                continue;
            }
            let Some(namespace) = crate::merge_driver::merge_driver_subsection(&record.key)? else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "merge config record unexpectedly has no driver subsection",
                ));
            };
            namespaces.insert(namespace.to_string());
            let final_key = record
                .key
                .rsplit_once('.')
                .map(|(_, key)| key)
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "malformed merge config record")
                })?;
            if matches!(&record.value, GitConfigValue::Implicit)
                && matches!(final_key, "driver" | "name" | "recursive")
            {
                conditional_invalid = true;
            }
        }
        Ok(Self {
            owner: Arc::clone(owner),
            default,
            namespaces,
            conditional_invalid,
        })
    }

    fn namespaces(&self) -> &BTreeSet<String> {
        &self.namespaces
    }

    fn default_driver(&self) -> Option<&str> {
        self.default.as_deref()
    }

    fn conditional_invalid(&self) -> bool {
        self.conditional_invalid
    }

    fn ensure_owner(&self, owner: &Arc<CapabilityIdentity>) -> io::Result<()> {
        if !Arc::ptr_eq(&self.owner, owner) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "merge config snapshot belongs to another operation",
            ));
        }
        Ok(())
    }

    fn sanitized_default_driver(&self) -> &str {
        match self.default.as_deref() {
            Some("binary") => "binary",
            Some("union") => "union",
            _ => "text",
        }
    }
}

/// A sealed, helper-free common repository view bound to one operation.
///
/// The real Git directory, index, worktree, and object store remain selected;
/// only common config and attributes are replaced for the final three-way
/// child. Construction accepts a complete merge snapshot so a caller cannot
/// attach an unreviewed or partial view.
pub(super) struct SealedMergeConfigOverride {
    owner: Arc<CapabilityIdentity>,
    common_dir: IsolatedGitCommonDir,
    effective_paths: Vec<String>,
    selected_custom: BTreeMap<String, String>,
    custom_destinations: BTreeSet<String>,
    conditional_invalid: bool,
    object_format: IndexObjectFormat,
    proof: Option<ThreeWayMergePolicyProof>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IndexStageEntry {
    mode: u32,
    object_id: IndexObjectId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum IndexObjectId {
    Sha1([u8; 20]),
    Sha256([u8; 32]),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IndexObjectFormat {
    Sha1,
    Sha256,
}

type IndexStageSnapshot = BTreeMap<String, BTreeMap<u8, IndexStageEntry>>;

struct ThreeWayMergePolicyProof {
    owner: Arc<CapabilityIdentity>,
    revert: bool,
    patch_path: String,
    custom_destinations: BTreeSet<String>,
    conditional_invalid: bool,
    prospective_index: IndexStageSnapshot,
}

impl SealedMergeConfigOverride {
    pub(super) fn common_dir(
        &self,
        owner: &Arc<CapabilityIdentity>,
    ) -> io::Result<&IsolatedGitCommonDir> {
        self.ensure_owner(owner)?;
        Ok(&self.common_dir)
    }

    fn ensure_owner(&self, owner: &Arc<CapabilityIdentity>) -> io::Result<()> {
        if !Arc::ptr_eq(&self.owner, owner) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "sealed Git merge override belongs to another operation",
            ));
        }
        Ok(())
    }

    fn proof_required(&self) -> bool {
        self.conditional_invalid || !self.custom_destinations.is_empty()
    }
}

impl<'git> GuardedGitConfig<'git> {
    /// Install the complete fallback merge policy in one non-bypassable step.
    /// This is the only crate-visible merge-overlay API: callers cannot mark
    /// the policy complete without the fixed fresh config and attribute reads.
    pub(crate) fn install_three_way_merge_policy(
        &mut self,
        primary_records: &[String],
    ) -> io::Result<()> {
        let paths = self.apply_filter_paths()?;
        let snapshot = self.read_merge_config_snapshot()?;
        let input = merge_attribute_input(&paths)?;
        let output = self.query_merge_attributes(&snapshot, input)?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "git merge attribute probe failed with status {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        let parsed_attributes = parse_three_way_attributes(&output.stdout, &paths)?;
        let merge_attributes = parsed_attributes
            .iter()
            .map(|(path, attributes)| (path.clone(), attributes.merge.clone()))
            .collect::<BTreeMap<_, _>>();
        let selected_custom = crate::merge_driver::untrusted_driver_selections(
            snapshot.namespaces(),
            snapshot.default_driver(),
            &merge_attributes,
        );
        let mut custom_destinations = BTreeSet::new();
        for path in primary_records {
            if selected_custom.contains_key(path) && !custom_destinations.insert(path.clone()) {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    format!(
                        "refusing to classify repeated patch records for selected custom merge path {path:?}"
                    ),
                ));
            }
        }
        let attributes = MergeAttributeSnapshot::from_effective(
            &self.identity,
            &snapshot,
            parsed_attributes,
            &selected_custom,
        )?;
        let isolated = self.build_merge_override(
            &snapshot,
            &attributes,
            paths,
            selected_custom,
            custom_destinations,
        )?;
        self.attach_merge_override(isolated)
    }

    /// Read merge-driver policy from the frozen, authorized base invocation.
    /// Attached neutralizers are deliberately excluded so this is a fresh
    /// view of the user's effective policy at fallback time.
    fn read_merge_config_snapshot(&self) -> io::Result<MergeConfigSnapshot> {
        let _ = self.apply_filter_paths()?;
        if self.merge_policy_installed {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "merge policy may be read only before its neutralizer is attached",
            ));
        }
        #[cfg(test)]
        MERGE_CONFIG_READ_COUNT.with(|count| count.set(count.get() + 1));
        MergeConfigSnapshot::from_records(&self.identity, self.sources.read_merge_config()?)
    }

    /// Run the one fixed fresh apply-attribute query while the merge
    /// neutralizer is still absent. Existing apply-filter policy remains
    /// attached, and callers cannot change the framing or attribute names.
    fn query_merge_attributes(
        &self,
        snapshot: &MergeConfigSnapshot,
        input: std::fs::File,
    ) -> io::Result<std::process::Output> {
        let _ = self.apply_filter_paths()?;
        snapshot.ensure_owner(&self.identity)?;
        if self.merge_policy_installed {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "merge attributes must be read before attaching the merge neutralizer",
            ));
        }
        #[cfg(test)]
        MERGE_ATTRIBUTE_READ_COUNT.with(|count| count.set(count.get() + 1));
        let mut command = self.command_with_attached_overlays()?;
        BoundSubcommand::CheckAttr.append_to(&mut command);
        command
            .env("GIT_OPTIONAL_LOCKS", "0")
            .args([
                "--stdin",
                "-z",
                "merge",
                "whitespace",
                "conflict-marker-size",
                "text",
                "crlf",
                "eol",
                "ident",
                "filter",
                "working-tree-encoding",
            ])
            .stdin(Stdio::from(input));
        self.sources.git.output(command)
    }

    fn build_merge_override(
        &self,
        snapshot: &MergeConfigSnapshot,
        attributes: &MergeAttributeSnapshot,
        effective_paths: Vec<String>,
        selected_custom: BTreeMap<String, String>,
        custom_destinations: BTreeSet<String>,
    ) -> io::Result<SealedMergeConfigOverride> {
        snapshot.ensure_owner(&self.identity)?;
        attributes.ensure_owner(&self.identity)?;
        if self.merge_policy_installed {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "an isolated three-way config is already attached",
            ));
        }

        let entries = self
            .sources
            .read_effective(SANITIZED_CONFIG_PATTERN, "three-way allowlist")?;
        let shared_repository = self
            .sources
            .read_shared_repository()?
            .as_ref()
            .map(normalize_shared_repository)
            .transpose()?;
        let repository_format = self
            .sources
            .read_direct_common_config(REPOSITORY_FORMAT_CONFIG_PATTERN, "repository format")?;
        let object_format = index_object_format(&repository_format)?;
        let common_dir = self.sources.git.create_isolated_common_dir()?;
        let config_path = common_dir.config_path();
        self.ensure_owned_config_path(&config_path, "owned isolated Git common config")?;
        for entry in entries.values() {
            self.write_sanitized_config_value(&config_path, &entry.key, &entry.value)?;
        }
        if let Some(shared_repository) = shared_repository {
            self.write_sanitized_config_value(
                &config_path,
                "core.sharedrepository",
                &shared_repository,
            )?;
        }
        for entry in repository_format.values() {
            self.write_sanitized_config_value(&config_path, &entry.key, &entry.value)?;
        }
        self.write_sanitized_config_value(&config_path, "core.bare", "false")?;
        self.write_sanitized_config_value(
            &config_path,
            "merge.default",
            snapshot.sanitized_default_driver(),
        )?;
        if snapshot.conditional_invalid() {
            append_conditional_invalid_merge_config(&config_path)?;
        }
        self.write_projected_merge_attributes(&common_dir, attributes)?;
        #[cfg(test)]
        MERGE_OVERLAY_COUNT.with(|count| count.set(count.get() + 1));
        Ok(SealedMergeConfigOverride {
            owner: Arc::clone(&self.identity),
            common_dir,
            effective_paths,
            selected_custom,
            custom_destinations,
            conditional_invalid: snapshot.conditional_invalid(),
            object_format,
            proof: None,
        })
    }

    fn attach_merge_override(&mut self, isolated: SealedMergeConfigOverride) -> io::Result<()> {
        if self.merge_policy_installed {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "a second isolated three-way config is not permitted",
            ));
        }
        let [apply] = self.filters.as_slice() else {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "merge policy requires exactly one apply filter snapshot",
            ));
        };
        if apply.role() != crate::safe_git::FilterPolicyRole::Apply {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "merge policy requires an apply filter snapshot",
            ));
        }
        isolated.ensure_owner(&self.identity)?;
        self.merge = Some(isolated);
        self.merge_policy_installed = true;
        Ok(())
    }

    fn write_sanitized_config_value(
        &self,
        config_path: &Path,
        key: &str,
        value: &str,
    ) -> io::Result<()> {
        // The config path is absolute and operation-owned, so no repository
        // cwd is needed. This keeps source repository values from being
        // interpreted while serializing the already validated snapshot.
        let mut command = self.sources.git.command();
        command
            .args(["config", "--file"])
            .arg(config_path)
            .args(["--add", key, value]);
        let output = self.sources.git.output(command)?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "failed to write isolated Git config value {key:?} (status {}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(())
    }

    fn write_projected_merge_attributes(
        &self,
        common_dir: &IsolatedGitCommonDir,
        attributes: &MergeAttributeSnapshot,
    ) -> io::Result<()> {
        attributes.ensure_owner(&self.identity)?;
        let attributes_path = common_dir.attributes_path();
        self.ensure_owned_config_path(&attributes_path, "owned isolated Git merge attributes")?;
        std::fs::write(
            attributes_path,
            projected_merge_attributes(&attributes.paths)?,
        )
    }

    pub(crate) fn three_way_requires_merge_policy_proof(&self) -> io::Result<bool> {
        let merge = self.merge.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "isolated three-way config is unavailable",
            )
        })?;
        merge.ensure_owner(&self.identity)?;
        Ok(merge.proof_required())
    }

    pub(crate) fn create_three_way_scratch_storage(&self) -> io::Result<IsolatedGitStorage> {
        if !self.three_way_requires_merge_policy_proof()? {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "three-way scratch storage requested without a merge-policy proof requirement",
            ));
        }
        self.sources.git.create_isolated_git_storage()
    }

    /// Prove that the frozen merge policy cannot reach repository-selected
    /// helper dispatch or a conditionally invalid merge-config read for this
    /// exact patch and prospective index. The caller may first model reverse
    /// staging in `storage`.
    pub(crate) fn prove_three_way_merge_policy_safety(
        &mut self,
        storage: &IsolatedGitStorage,
        revert: bool,
        patch_path: &str,
    ) -> io::Result<()> {
        let (effective_paths, custom_destinations, selected_custom, conditional_invalid) = {
            let merge = self.merge.as_ref().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "isolated three-way config is unavailable",
                )
            })?;
            merge.ensure_owner(&self.identity)?;
            if !merge.proof_required() {
                return Ok(());
            }
            if merge.proof.is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "merge-policy scratch proof may be installed only once",
                ));
            }
            (
                merge.effective_paths.clone(),
                merge.custom_destinations.clone(),
                merge.selected_custom.clone(),
                merge.conditional_invalid,
            )
        };

        // This also binds the scratch child to the successful real-policy
        // gate for the exact patch and orientation.
        let whitespace_mode = self.final_apply_whitespace_mode(revert, patch_path)?;
        let prospective_index =
            self.capture_merge_index_stage_snapshot(&effective_paths, Some(storage))?;
        let output = self.run_scratch_three_way_apply(
            storage,
            revert,
            patch_path,
            whitespace_mode.is_fatal(),
        )?;
        let resulting_index =
            self.capture_merge_index_stage_snapshot(&effective_paths, Some(storage))?;

        let completed = match output.status.code() {
            Some(0) => true,
            Some(1) => newly_unmerged_noncustom_path(
                &prospective_index,
                &resulting_index,
                &custom_destinations,
            ),
            Some(_) | None => false,
        };
        if !completed {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!(
                    "unable to prove helper-free merge-policy reachability (status {}): {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            ));
        }

        if let Some(path) = custom_destinations.iter().find(|path| {
            resulting_index
                .get(*path)
                .is_some_and(|entries| entries.keys().any(|stage| *stage != 0))
        }) {
            let driver = selected_custom
                .get(path)
                .map(String::as_str)
                .unwrap_or("<unknown>");
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!(
                    "refusing an internal Git three-way apply because merge driver {driver:?} is reachable for {path:?}"
                ),
            ));
        }

        let merge = self.merge.as_mut().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "isolated three-way config disappeared during classification",
            )
        })?;
        merge.proof = Some(ThreeWayMergePolicyProof {
            owner: Arc::clone(&self.identity),
            revert,
            patch_path: patch_path.to_string(),
            custom_destinations,
            conditional_invalid,
            prospective_index,
        });
        Ok(())
    }

    pub(super) fn consume_three_way_merge_policy_proof(
        &mut self,
        revert: bool,
        patch_path: &str,
    ) -> io::Result<()> {
        let (proof, effective_paths, custom_destinations, conditional_invalid) = {
            let merge = self.merge.as_mut().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "isolated three-way config is unavailable",
                )
            })?;
            merge.ensure_owner(&self.identity)?;
            if !merge.proof_required() {
                return Ok(());
            }
            // Consume before every subsequent fallible validation. A failed
            // query or mismatch must not leave a reusable proof behind.
            let proof = merge.proof.take().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "the frozen merge policy requires an unused scratch proof",
                )
            })?;
            (
                proof,
                merge.effective_paths.clone(),
                merge.custom_destinations.clone(),
                merge.conditional_invalid,
            )
        };
        if !Arc::ptr_eq(&proof.owner, &self.identity)
            || proof.revert != revert
            || proof.patch_path != patch_path
            || proof.custom_destinations != custom_destinations
            || proof.conditional_invalid != conditional_invalid
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "merge-policy scratch proof does not match the final apply",
            ));
        }
        let current_index =
            self.capture_merge_index_stage_snapshot(&effective_paths, /*storage*/ None)?;
        if current_index != proof.prospective_index {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Git index changed after merge-policy classification",
            ));
        }
        Ok(())
    }

    pub(super) fn ensure_three_way_merge_policy_proof_installed(&self) -> io::Result<()> {
        let merge = self.merge.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "isolated three-way config is unavailable",
            )
        })?;
        merge.ensure_owner(&self.identity)?;
        if merge.proof_required() && merge.proof.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "the frozen merge policy requires a completed scratch proof",
            ));
        }
        Ok(())
    }

    fn run_scratch_three_way_apply(
        &self,
        storage: &IsolatedGitStorage,
        revert: bool,
        patch_path: &str,
        neutralize_fatal_whitespace: bool,
    ) -> io::Result<std::process::Output> {
        let merge = self.merge.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "isolated three-way config is unavailable",
            )
        })?;
        let isolated = merge.common_dir(&self.identity)?;
        let mut command = self
            .sources
            .git
            .command_for_cwd(&self.sources.canonical_root)?;
        super::append_safe_scalar_overrides(&mut command);
        self.apply_policy
            .as_ref()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "scratch apply policy scalars are not frozen",
                )
            })?
            .append_to(&mut command);
        BoundSubcommand::Apply.append_to(&mut command);
        command.args(["--cached", "--3way"]);
        if neutralize_fatal_whitespace {
            command.arg("--whitespace=nowarn");
        }
        if revert {
            command.arg("-R");
        }
        command.arg("--").arg(patch_path);
        self.sources
            .git
            .output_in_isolated_scratch(command, isolated, storage)
    }

    fn capture_merge_index_stage_snapshot(
        &self,
        paths: &[String],
        storage: Option<&IsolatedGitStorage>,
    ) -> io::Result<IndexStageSnapshot> {
        let merge = self.merge.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "isolated three-way config is unavailable",
            )
        })?;
        let isolated = merge.common_dir(&self.identity)?;
        let mut command = self
            .sources
            .git
            .command_for_cwd(&self.sources.canonical_root)?;
        super::append_safe_scalar_overrides(&mut command);
        command.arg("--literal-pathspecs");
        BoundSubcommand::LsFiles.append_to(&mut command);
        command.args(["--stage", "-z", "--"]).args(paths);
        let output = if let Some(storage) = storage {
            self.sources
                .git
                .output_in_isolated_scratch(command, isolated, storage)?
        } else {
            self.sources
                .git
                .output_in_isolated_common_dir(command, isolated)?
        };
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "failed to inspect isolated Git index stages (status {}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        parse_index_stage_snapshot(&output.stdout, paths, merge.object_format)
    }
}

fn newly_unmerged_noncustom_path(
    before: &IndexStageSnapshot,
    after: &IndexStageSnapshot,
    custom_destinations: &BTreeSet<String>,
) -> bool {
    after.iter().any(|(path, entries)| {
        !custom_destinations.contains(path)
            && before
                .get(path)
                .is_none_or(|prior| prior.keys().all(|stage| *stage == 0))
            && entries.keys().any(|stage| *stage != 0)
            && before.get(path) != Some(entries)
    })
}

fn parse_index_stage_snapshot(
    output: &[u8],
    expected_paths: &[String],
    object_format: IndexObjectFormat,
) -> io::Result<IndexStageSnapshot> {
    if output.is_empty() {
        return Ok(IndexStageSnapshot::new());
    }
    let body = output
        .strip_suffix(&[0])
        .ok_or_else(|| invalid_stage_output("unterminated Git index stage output"))?;
    if body.is_empty() {
        return Err(invalid_stage_output("empty Git index stage record"));
    }
    if body.split(|byte| *byte == 0).any(<[u8]>::is_empty) {
        return Err(invalid_stage_output("empty Git index stage record"));
    }
    if !output.ends_with(&[0]) {
        return Err(invalid_stage_output("unterminated Git index stage output"));
    }
    let expected = expected_paths
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if expected.len() != expected_paths.len() || expected.iter().any(|path| path.is_empty()) {
        return Err(invalid_stage_output("duplicate expected Git index path"));
    }
    let mut snapshot = IndexStageSnapshot::new();
    for record in body.split(|byte| *byte == 0) {
        let separator = record
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or_else(|| invalid_stage_output("missing Git index path separator"))?;
        let (header, path) = record.split_at(separator);
        let path = std::str::from_utf8(&path[1..])
            .map_err(|_| invalid_stage_output("non-UTF-8 Git index path"))?;
        if !expected.contains(path) {
            return Err(invalid_stage_output("unexpected Git index path"));
        }
        let fields = header.split(|byte| *byte == b' ').collect::<Vec<_>>();
        let [mode, object_id, stage] = fields.as_slice() else {
            return Err(invalid_stage_output("noncanonical Git index stage header"));
        };
        let mode = std::str::from_utf8(mode)
            .ok()
            .filter(|mode| mode.len() == 6 && mode.bytes().all(|byte| matches!(byte, b'0'..=b'7')))
            .and_then(|mode| u32::from_str_radix(mode, 8).ok())
            .filter(|mode| matches!(*mode, 0o040000 | 0o100644 | 0o100755 | 0o120000 | 0o160000))
            .ok_or_else(|| invalid_stage_output("invalid Git index mode"))?;
        let expected_oid_length = match object_format {
            IndexObjectFormat::Sha1 => 40,
            IndexObjectFormat::Sha256 => 64,
        };
        if object_id.len() != expected_oid_length
            || !object_id
                .iter()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(invalid_stage_output("invalid Git index object ID"));
        }
        let object_id = parse_index_object_id(object_id)?;
        let stage = std::str::from_utf8(stage)
            .ok()
            .filter(|stage| stage.len() == 1)
            .and_then(|stage| stage.parse::<u8>().ok())
            .filter(|stage| *stage <= 3)
            .ok_or_else(|| invalid_stage_output("invalid Git index stage"))?;
        let entries = snapshot.entry(path.to_string()).or_default();
        if entries.contains_key(&stage) {
            return Err(invalid_stage_output("duplicate Git index stage"));
        }
        entries.insert(stage, IndexStageEntry { mode, object_id });
    }
    if snapshot.values().any(|entries| {
        entries
            .get(&0)
            .is_some_and(|entry| entry.object_id.is_zero())
    }) {
        return Err(invalid_stage_output(
            "Git index stage-zero entry has an intent-to-add object ID",
        ));
    }
    if snapshot
        .values()
        .any(|entries| entries.contains_key(&0) && entries.keys().any(|stage| *stage != 0))
    {
        return Err(invalid_stage_output(
            "Git index path mixes stage zero with unmerged stages",
        ));
    }
    if snapshot.values().any(|entries| {
        !entries.contains_key(&0) && entries.keys().filter(|stage| **stage != 0).count() < 2
    }) {
        return Err(invalid_stage_output(
            "Git index path has an incomplete unmerged stage set",
        ));
    }
    Ok(snapshot)
}

impl IndexObjectId {
    fn is_zero(&self) -> bool {
        match self {
            Self::Sha1(value) => value.iter().all(|byte| *byte == 0),
            Self::Sha256(value) => value.iter().all(|byte| *byte == 0),
        }
    }
}

fn parse_index_object_id(value: &[u8]) -> io::Result<IndexObjectId> {
    fn nibble(byte: u8) -> u8 {
        match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => unreachable!("caller validated lowercase hexadecimal"),
        }
    }
    let mut decoded = [0_u8; 32];
    for (index, pair) in value.chunks_exact(2).enumerate() {
        decoded[index] = (nibble(pair[0]) << 4) | nibble(pair[1]);
    }
    match value.len() {
        40 => {
            let mut sha1 = [0_u8; 20];
            sha1.copy_from_slice(&decoded[..20]);
            Ok(IndexObjectId::Sha1(sha1))
        }
        64 => Ok(IndexObjectId::Sha256(decoded)),
        _ => Err(invalid_stage_output("invalid Git index object ID length")),
    }
}

fn invalid_stage_output(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn index_object_format(
    repository_format: &BTreeMap<String, GitConfigEntry>,
) -> io::Result<IndexObjectFormat> {
    match repository_format
        .get("extensions.objectformat")
        .map(|entry| entry.value.as_str())
    {
        None | Some("sha1") => Ok(IndexObjectFormat::Sha1),
        Some("sha256") => Ok(IndexObjectFormat::Sha256),
        Some(value) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported Git repository object format {value:?}"),
        )),
    }
}

/// Preserve Git's conditional failure for an implicit known merge value.
/// The fixed app-owned valueless key is accepted by the generic config parser
/// but rejected when low-level merge initialization reads its string value.
/// Trivial three-way equalities return before that initialization.
fn append_conditional_invalid_merge_config(config_path: &Path) -> io::Result<()> {
    let mut config = std::fs::OpenOptions::new().append(true).open(config_path)?;
    config.write_all(b"\n[merge \"codex-conditional-invalid\"]\n\tdriver\n")?;
    config.flush()
}

fn normalize_shared_repository(value: &GitConfigValue) -> io::Result<String> {
    let GitConfigValue::Explicit(value) = value else {
        return Ok("group".to_string());
    };

    match value.as_str() {
        "umask" => return Ok("umask".to_string()),
        "group" => return Ok("group".to_string()),
        "all" | "world" | "everybody" => return Ok("all".to_string()),
        _ => {}
    }

    match parse_full_base8_i32(value) {
        FullBase8I32::Value(0) => return Ok("umask".to_string()),
        FullBase8I32::Value(1) => return Ok("group".to_string()),
        FullBase8I32::Value(2) => return Ok("all".to_string()),
        FullBase8I32::Value(mode) if mode & 0o600 == 0o600 => {
            return Ok(format!("0{:03o}", mode & 0o666));
        }
        FullBase8I32::Value(_) | FullBase8I32::Overflow => {
            return Err(invalid_shared_repository(value));
        }
        FullBase8I32::NotFull => {}
    }

    match parse_git_boolean_symmetric_i32(value.as_bytes()) {
        Some(true) => Ok("group".to_string()),
        Some(false) => Ok("umask".to_string()),
        None => Err(invalid_shared_repository(value)),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FullBase8I32 {
    NotFull,
    Value(i32),
    Overflow,
}

/// Reproduce the first numeric branch of Git's `git_config_perm`: a signed
/// base-8 `strtol` whose result is used only when it consumes the whole value.
/// Values outside `i32` are rejected because Git's subsequent `long` to `int`
/// narrowing is platform-dependent.
fn parse_full_base8_i32(value: &str) -> FullBase8I32 {
    if value.is_empty() {
        // `strtol("", &end, 8)` performs no conversion, but `end` still points
        // at the terminating NUL. Git therefore takes its numeric-zero branch.
        return FullBase8I32::Value(0);
    }

    let trimmed = value.trim_start_matches(|character: char| character.is_ascii_whitespace());
    let (negative, digits) = match trimmed.as_bytes().first() {
        Some(b'-') => (true, &trimmed[1..]),
        Some(b'+') => (false, &trimmed[1..]),
        Some(_) => (false, trimmed),
        None => return FullBase8I32::NotFull,
    };
    let digit_count = digits
        .bytes()
        .take_while(|byte| matches!(byte, b'0'..=b'7'))
        .count();
    if digit_count == 0 || digit_count != digits.len() {
        return FullBase8I32::NotFull;
    }

    let limit = if negative {
        i64::from(i32::MAX) + 1
    } else {
        i64::from(i32::MAX)
    };
    let mut magnitude = 0_i64;
    for digit in digits.bytes() {
        let Some(next) = magnitude
            .checked_mul(8)
            .and_then(|value| value.checked_add(i64::from(digit - b'0')))
        else {
            return FullBase8I32::Overflow;
        };
        if next > limit {
            return FullBase8I32::Overflow;
        }
        magnitude = next;
    }

    let signed = if negative { -magnitude } else { magnitude };
    match i32::try_from(signed) {
        Ok(value) => FullBase8I32::Value(value),
        Err(_) => FullBase8I32::Overflow,
    }
}

fn invalid_shared_repository(value: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("invalid core.sharedRepository value {value:?}"),
    )
}

fn projected_merge_attributes(
    paths: &BTreeMap<String, FrozenPathApplyAttributes>,
) -> io::Result<Vec<u8>> {
    if paths.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refusing to project an empty merge attribute snapshot",
        ));
    }
    let mut projection = Vec::new();
    for (path, attributes) in paths {
        let pattern = literal_attribute_pattern(path)?;
        let merge = match &attributes.merge {
            MergeSelection::Builtin(driver) => {
                format!("merge={}", driver.attribute_value()).into_bytes()
            }
            // Attribute-unset selects Git's built-in binary merge directly.
            // It cannot be redirected by merge.default or by a driver named
            // "binary", even if a later index/object race invalidates the
            // trivial-merge proof.
            MergeSelection::QuarantinedCustom { .. } => b"-merge".to_vec(),
        };
        let tokens = [
            merge,
            // Never retain an arbitrary filter selection in the final child.
            // This highest-precedence reset also masks later worktree changes.
            b"!filter".to_vec(),
            attributes.safe.whitespace.projected_token("whitespace"),
            attributes
                .safe
                .conflict_marker_size
                .projected_token("conflict-marker-size"),
            attributes.safe.text.projected_token("text"),
            attributes.safe.crlf.projected_token("crlf"),
            attributes.safe.eol.projected_token("eol"),
            attributes.safe.ident.projected_token("ident"),
            attributes
                .safe
                .working_tree_encoding
                .projected_token("working-tree-encoding"),
        ];
        append_projected_attribute_lines(&mut projection, pattern.as_bytes(), &tokens)?;
    }
    Ok(projection)
}

fn append_projected_attribute_lines(
    projection: &mut Vec<u8>,
    pattern: &[u8],
    tokens: &[Vec<u8>],
) -> io::Result<()> {
    if tokens.is_empty() {
        return Err(invalid_attribute_output(
            "refusing to project an empty Git attribute token set",
        ));
    }
    let mut line = pattern.to_vec();
    let mut tokens_on_line = 0_usize;
    for token in tokens {
        if token.is_empty() || token.iter().copied().any(is_git_attribute_token_separator) {
            return Err(invalid_attribute_output(
                "unrepresentable projected Git attribute token",
            ));
        }
        let candidate_length = line.len() + 1 + token.len();
        if candidate_length >= GIT_ATTRIBUTE_LINE_LENGTH_LIMIT {
            if tokens_on_line == 0 {
                return Err(invalid_attribute_output(
                    "projected Git attribute line exceeds Git's length limit",
                ));
            }
            projection.extend_from_slice(&line);
            projection.push(b'\n');
            line.clear();
            line.extend_from_slice(pattern);
            tokens_on_line = 0;
            if line.len() + 1 + token.len() >= GIT_ATTRIBUTE_LINE_LENGTH_LIMIT {
                return Err(invalid_attribute_output(
                    "projected Git attribute token exceeds Git's line length limit",
                ));
            }
        }
        line.push(b' ');
        line.extend_from_slice(token);
        tokens_on_line += 1;
    }
    projection.extend_from_slice(&line);
    projection.push(b'\n');
    Ok(())
}

#[derive(Default)]
struct PendingPathApplyAttributes {
    merge: Option<String>,
    whitespace: Option<ProjectedAttribute>,
    conflict_marker_size: Option<ProjectedAttribute>,
    text: Option<ProjectedAttribute>,
    crlf: Option<ProjectedAttribute>,
    eol: Option<ProjectedAttribute>,
    ident: Option<ProjectedAttribute>,
    filter: Option<ProjectedAttribute>,
    working_tree_encoding: Option<ProjectedAttribute>,
}

impl PendingPathApplyAttributes {
    fn insert(&mut self, attribute: &[u8], value: &[u8]) -> io::Result<()> {
        if attribute == b"merge" {
            let value = std::str::from_utf8(value)
                .map_err(|_| invalid_attribute_output("non-UTF-8 Git merge attribute value"))?;
            if self.merge.replace(value.to_string()).is_some() {
                return Err(invalid_attribute_output(
                    "duplicate Git merge attribute record",
                ));
            }
            return Ok(());
        }

        let value = ProjectedAttribute::from_check_attr(value)?;
        let slot = match attribute {
            b"whitespace" => &mut self.whitespace,
            b"conflict-marker-size" => &mut self.conflict_marker_size,
            b"text" => &mut self.text,
            b"crlf" => &mut self.crlf,
            b"eol" => &mut self.eol,
            b"ident" => &mut self.ident,
            b"filter" => &mut self.filter,
            b"working-tree-encoding" => &mut self.working_tree_encoding,
            _ => {
                return Err(invalid_attribute_output(
                    "unexpected Git attribute record name",
                ));
            }
        };
        if slot.replace(value).is_some() {
            return Err(invalid_attribute_output(
                "duplicate Git apply attribute record",
            ));
        }
        Ok(())
    }

    fn finish(self) -> io::Result<ParsedPathApplyAttributes> {
        Ok(ParsedPathApplyAttributes {
            merge: required_attribute(self.merge, "merge")?,
            safe: SafeApplyAttributes {
                whitespace: required_attribute(self.whitespace, "whitespace")?,
                conflict_marker_size: required_attribute(
                    self.conflict_marker_size,
                    "conflict-marker-size",
                )?,
                text: required_attribute(self.text, "text")?,
                crlf: required_attribute(self.crlf, "crlf")?,
                eol: required_attribute(self.eol, "eol")?,
                ident: required_attribute(self.ident, "ident")?,
                working_tree_encoding: required_attribute(
                    self.working_tree_encoding,
                    "working-tree-encoding",
                )?,
                _filter: required_attribute(self.filter, "filter")?,
            },
        })
    }
}

fn required_attribute<T>(value: Option<T>, name: &str) -> io::Result<T> {
    value.ok_or_else(|| invalid_attribute_output(&format!("missing Git {name} attribute record")))
}

/// Parse the strict NUL-framed output of the one fixed nine-attribute query.
/// Every authorized path must have exactly one record for every consumed name;
/// extra, duplicate, malformed, or unrepresentable data fails closed.
fn parse_three_way_attributes(
    output: &[u8],
    expected_paths: &[String],
) -> io::Result<BTreeMap<String, ParsedPathApplyAttributes>> {
    let Some(body) = output.strip_suffix(&[0]) else {
        return Err(invalid_attribute_output(
            "unterminated Git attribute output",
        ));
    };
    let fields = body.split(|byte| *byte == 0).collect::<Vec<_>>();
    if fields.len() % 3 != 0 {
        return Err(invalid_attribute_output("incomplete Git attribute record"));
    }
    let expected = expected_paths
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    if expected.len() != expected_paths.len() {
        return Err(invalid_attribute_output(
            "duplicate expected Git attribute path",
        ));
    }
    let mut pending = expected_paths
        .iter()
        .cloned()
        .map(|path| (path, PendingPathApplyAttributes::default()))
        .collect::<BTreeMap<_, _>>();
    for record in fields.chunks_exact(3) {
        let path = std::str::from_utf8(record[0])
            .map_err(|_| invalid_attribute_output("non-UTF-8 Git attribute path"))?;
        if !expected.contains(path) {
            return Err(invalid_attribute_output(
                "unexpected Git attribute record path",
            ));
        }
        pending
            .get_mut(path)
            .ok_or_else(|| invalid_attribute_output("missing expected Git attribute path"))?
            .insert(record[1], record[2])?;
    }
    pending
        .into_iter()
        .map(|(path, attributes)| Ok((path, attributes.finish()?)))
        .collect()
}

fn validate_projected_attribute_value(value: &[u8]) -> io::Result<()> {
    if value.iter().copied().any(is_git_attribute_token_separator) {
        return Err(invalid_attribute_output(
            "unrepresentable Git attribute value",
        ));
    }
    Ok(())
}

fn is_git_attribute_token_separator(byte: u8) -> bool {
    matches!(byte, 0 | b' ' | b'\t' | b'\r' | b'\n')
}

fn invalid_attribute_output(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// Quote an exact repository-relative path as a root-anchored attributes
/// pattern. The first escaping layer protects Git wildmatch metacharacters;
/// the second is Git's C-style double-quoted attribute syntax.
fn literal_attribute_pattern(path: &str) -> io::Result<String> {
    if path.is_empty() || path.starts_with('/') || path.as_bytes().contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "merge attribute path is not a repository-relative Git path",
        ));
    }

    let mut literal = String::with_capacity(path.len() + 1);
    literal.push('/');
    for character in path.chars() {
        if matches!(character, '*' | '?' | '[' | ']' | '\\') {
            literal.push('\\');
        }
        literal.push(character);
    }

    let mut quoted = String::with_capacity(literal.len() + 2);
    quoted.push('"');
    for character in literal.chars() {
        match character {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            '\x07' => quoted.push_str("\\a"),
            '\x08' => quoted.push_str("\\b"),
            '\x0c' => quoted.push_str("\\f"),
            '\x0b' => quoted.push_str("\\v"),
            _ => quoted.push(character),
        }
    }
    quoted.push('"');
    Ok(quoted)
}

fn merge_attribute_input(paths: &[String]) -> io::Result<std::fs::File> {
    use std::io::Seek;
    use std::io::Write;

    if paths.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refusing to inspect merge attributes for an empty patch path set",
        ));
    }
    let mut input = tempfile::tempfile()?;
    for path in paths {
        if path.as_bytes().contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "merge attribute path contains NUL",
            ));
        }
        input.write_all(path.as_bytes())?;
        input.write_all(&[0])?;
    }
    input.rewind()?;
    Ok(input)
}

#[cfg(test)]
thread_local! {
    static MERGE_CONFIG_READ_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static MERGE_ATTRIBUTE_READ_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static MERGE_OVERLAY_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_merge_policy_counts() {
    MERGE_CONFIG_READ_COUNT.with(|count| count.set(0));
    MERGE_ATTRIBUTE_READ_COUNT.with(|count| count.set(0));
    MERGE_OVERLAY_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn merge_config_read_count() -> usize {
    MERGE_CONFIG_READ_COUNT.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(crate) fn merge_attribute_read_count() -> usize {
    MERGE_ATTRIBUTE_READ_COUNT.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(crate) fn merge_overlay_count() -> usize {
    MERGE_OVERLAY_COUNT.with(std::cell::Cell::get)
}

#[cfg(test)]
#[path = "merge_overlay_tests.rs"]
mod tests;
