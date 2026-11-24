use self::launch::PreparedLaunchPolicy;
use self::launch::SandboxOperation;
use self::launch::WrappedSandboxCommand;
use crate::LinuxSandboxPidNamespace;
#[cfg(windows)]
use crate::WindowsSandboxFilesystemOverrides;
#[cfg(windows)]
use crate::WindowsSandboxProxySettingsMode;
#[cfg(target_os = "linux")]
use crate::bwrap::WSL1_BWRAP_WARNING;
use crate::policy_transforms::effective_permission_profile;
use crate::policy_transforms::should_require_platform_sandbox_for_convention;
use codex_network_proxy::NetworkProxy;
use codex_network_proxy::PreparedNetworkRoute;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::models::PermissionProfile;
use codex_protocol::sandbox::SandboxOverride;
pub use codex_protocol::sandbox::SandboxType;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io;
use std::path::Path;

#[path = "launch.rs"]
pub(super) mod launch;

#[cfg(target_os = "windows")]
const WINDOWS_SANDBOX_WRAPPER_SETUP_ENV_ALLOWLIST: &[&str] = &[
    "USERNAME",
    "USERPROFILE",
    // ShellExecuteExW needs SystemRoot to elevate the setup helper.
    "SYSTEMROOT",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SandboxablePreference {
    Auto,
    Require,
    Forbid,
}

pub fn get_platform_sandbox(windows_sandbox_enabled: bool) -> Option<SandboxType> {
    if cfg!(target_os = "macos") {
        Some(SandboxType::MacosSeatbelt)
    } else if cfg!(target_os = "linux") {
        Some(SandboxType::LinuxSeccomp)
    } else if cfg!(target_os = "windows") {
        if windows_sandbox_enabled {
            Some(SandboxType::WindowsRestrictedToken)
        } else {
            None
        }
    } else {
        None
    }
}

pub fn with_managed_mitm_ca_readable_root(
    permission_profile: PermissionProfile,
    managed_mitm_ca_trust_bundle_path: Option<&AbsolutePathBuf>,
    sandbox_policy_cwd: &Path,
) -> PermissionProfile {
    let Some(managed_mitm_ca_trust_bundle_path) = managed_mitm_ca_trust_bundle_path else {
        return permission_profile;
    };
    let (file_system_sandbox_policy, network_sandbox_policy) =
        permission_profile.to_runtime_permissions();
    let file_system_sandbox_policy = file_system_sandbox_policy.with_additional_readable_roots(
        sandbox_policy_cwd,
        std::slice::from_ref(managed_mitm_ca_trust_bundle_path),
    );
    PermissionProfile::from_runtime_permissions_with_enforcement(
        permission_profile.enforcement(),
        &file_system_sandbox_policy,
        network_sandbox_policy,
    )
}

#[derive(Debug)]
pub struct SandboxCommand {
    pub program: OsString,
    pub args: Vec<String>,
    pub cwd: PathUri,
    pub env: HashMap<String, String>,
    pub managed_network: Option<PreparedNetworkRoute>,
    pub additional_permissions: Option<AdditionalPermissionProfile>,
}

/// A host-native launch request produced after the sandbox manager validates URI inputs.
/// Build this only at the execution boundary: in exec-server, or in its logical equivalent within
/// app-server. Orchestration and transport code should retain [`PathUri`] values and defer
/// conversion to native paths until this request is created.
#[derive(Clone, Debug)]
pub struct SandboxExecRequest {
    /// Controller-selected override, retained for observations without altering the sandbox command.
    pub sandbox_override: SandboxOverride,
    pub command: Vec<String>,
    /// Exact host executable when its native path cannot be represented in `command`.
    pub native_program: Option<OsString>,
    pub cwd: PathUri,
    pub sandbox_policy_cwd: PathUri,
    pub env: HashMap<String, String>,
    pub network: Option<NetworkProxy>,
    pub network_environment_id: Option<String>,
    pub sandbox: SandboxType,
    // TODO(anp): Reconcile these backend copies with the supplied sandbox context
    // (TurnEnvironment::sandbox_context for turns), preserving this launch snapshot.
    pub windows_sandbox_level: WindowsSandboxLevel,
    pub permission_profile: PermissionProfile,
    pub arg0: Option<String>,
}

impl SandboxExecRequest {
    /// Borrow the executable and arguments to pass to a host process launcher.
    pub fn program_and_args(&self) -> Option<(&OsStr, &[String])> {
        let (program, args) = self.command.split_first()?;
        Some((
            self.native_program
                .as_deref()
                .unwrap_or_else(|| OsStr::new(program)),
            args,
        ))
    }
}

/// A backend chosen by the caller, or a sandbox selected from policy on the execution host.
#[derive(Clone, Copy, Debug)]
pub enum SandboxSelection {
    /// Use the selected backend, including an explicitly native launch.
    Explicit(SandboxType),
    Platform {
        preference: SandboxablePreference,
        windows_sandbox: SandboxType,
    },
}

/// Network enforcement for one launch. A proxy may be required without a live proxy
/// on this host. The command carries any already-prepared route.
#[derive(Default)]
pub struct SandboxNetwork<'a> {
    enforce_managed_network: bool,
    /// The caller keeps the proxy listeners alive while the child uses them.
    proxy: Option<&'a NetworkProxy>,
    environment_id: Option<&'a str>,
}

impl<'a> SandboxNetwork<'a> {
    pub fn new(
        enforce_managed_network: bool,
        proxy: Option<&'a NetworkProxy>,
        environment_id: Option<&'a str>,
    ) -> Self {
        Self {
            enforce_managed_network,
            proxy,
            environment_id,
        }
    }
}

/// Executables supplied by the host. Only executor-provisioned helpers can receive runtime read access.
#[derive(Clone, Copy)]
pub enum SandboxExecutables<'a> {
    /// A caller supplies its backend launcher and needs no additional executable grants.
    Launcher(Option<&'a Path>),
    /// Paths from executor startup and the optional argv[0] used by its command helper.
    Executor {
        codex: &'a AbsolutePathBuf,
        linux_sandbox: Option<&'a AbsolutePathBuf>,
        arg0: Option<&'a str>,
    },
}

/// Bundled arguments for sandbox transformation.
///
/// This keeps call sites self-documenting when several fields are optional.
pub struct SandboxTransformRequest<'a> {
    pub command: SandboxCommand,
    pub permissions: &'a PermissionProfile,
    /// Bind symbolic workspace rules to these executor roots. `None` means they are already resolved.
    pub workspace_roots: Option<&'a [PathUri]>,
    pub selection: SandboxSelection,
    pub network: SandboxNetwork<'a>,
    pub sandbox_policy_cwd: &'a PathUri,
    pub executables: SandboxExecutables<'a>,
    // TODO(anp): Reconcile these backend inputs with the supplied sandbox context
    // (TurnEnvironment::sandbox_context for turns) so selection shares its authority.
    pub use_legacy_landlock: bool,
    pub windows_sandbox_level: WindowsSandboxLevel,
}

fn prepare_native_sandbox(
    exec: &mut SandboxExecRequest,
    trust_bundle: Option<&AbsolutePathBuf>,
) -> Result<(AbsolutePathBuf, AbsolutePathBuf), SandboxTransformError> {
    let cwd =
        exec.cwd
            .to_abs_path()
            .map_err(|source| SandboxTransformError::InvalidCommandCwd {
                cwd: exec.cwd.clone(),
                source,
            })?;
    let policy_cwd = exec.sandbox_policy_cwd.to_abs_path().map_err(|source| {
        SandboxTransformError::InvalidSandboxPolicyCwd {
            cwd: exec.sandbox_policy_cwd.clone(),
            source,
        }
    })?;
    if let Some(trust_bundle) = trust_bundle {
        exec.permission_profile = with_managed_mitm_ca_readable_root(
            std::mem::take(&mut exec.permission_profile),
            Some(trust_bundle),
            &policy_cwd,
        );
    }
    Ok((cwd, policy_cwd))
}

#[derive(Debug)]
pub enum SandboxTransformError {
    InvalidPolicy(io::Error),
    InvalidCommandCwd {
        cwd: PathUri,
        source: io::Error,
    },
    InvalidSandboxPolicyCwd {
        cwd: PathUri,
        source: io::Error,
    },
    MissingLinuxSandboxExecutable,
    SandboxUnavailable,
    WindowsMxcPreparation(String),
    EnvironmentNetworkProxy(String),
    #[cfg(target_os = "macos")]
    SeatbeltPreparation(String),
    #[cfg(target_os = "linux")]
    Wsl1UnsupportedForBubblewrap,
    #[cfg(not(target_os = "macos"))]
    SeatbeltUnavailable,
    #[cfg(target_os = "windows")]
    WindowsSandboxPreparation(String),
}

impl std::fmt::Display for SandboxTransformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPolicy(err) => err.fmt(f),
            Self::InvalidCommandCwd { cwd, source } => {
                write!(
                    f,
                    "command cwd URI `{cwd}` is not valid on this host: {source}"
                )
            }
            Self::InvalidSandboxPolicyCwd { cwd, source } => write!(
                f,
                "sandbox policy cwd URI `{cwd}` is not valid on this host: {source}"
            ),
            Self::SandboxUnavailable => {
                write!(f, "sandbox intent cannot be enforced on this executor")
            }
            Self::MissingLinuxSandboxExecutable => {
                write!(f, "missing codex-linux-sandbox executable path")
            }
            Self::WindowsMxcPreparation(err) => {
                write!(f, "failed to prepare MXC sandbox: {err}")
            }
            Self::EnvironmentNetworkProxy(err) => {
                write!(f, "failed to prepare environment network proxy: {err}")
            }
            #[cfg(target_os = "macos")]
            Self::SeatbeltPreparation(err) => {
                write!(f, "failed to prepare Seatbelt sandbox: {err}")
            }
            #[cfg(target_os = "linux")]
            Self::Wsl1UnsupportedForBubblewrap => write!(f, "{WSL1_BWRAP_WARNING}"),
            #[cfg(not(target_os = "macos"))]
            Self::SeatbeltUnavailable => write!(f, "seatbelt sandbox is only available on macOS"),
            #[cfg(target_os = "windows")]
            Self::WindowsSandboxPreparation(err) => {
                write!(f, "failed to prepare Windows sandbox: {err}")
            }
        }
    }
}

impl std::error::Error for SandboxTransformError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidPolicy(source) => Some(source),
            Self::InvalidCommandCwd { source, .. }
            | Self::InvalidSandboxPolicyCwd { source, .. } => Some(source),
            Self::MissingLinuxSandboxExecutable | Self::SandboxUnavailable => None,
            Self::WindowsMxcPreparation(_) => None,
            Self::EnvironmentNetworkProxy(_) => None,
            #[cfg(target_os = "macos")]
            Self::SeatbeltPreparation(_) => None,
            #[cfg(target_os = "linux")]
            Self::Wsl1UnsupportedForBubblewrap => None,
            #[cfg(not(target_os = "macos"))]
            Self::SeatbeltUnavailable => None,
            #[cfg(target_os = "windows")]
            Self::WindowsSandboxPreparation(_) => None,
        }
    }
}

/// Translate the bound policy into platform arguments or a native Windows session.
pub(super) trait SandboxBackend {
    fn prepare(
        &mut self,
        exec: &mut SandboxExecRequest,
        cwd: &AbsolutePathBuf,
        policy_cwd: &AbsolutePathBuf,
    ) -> Result<(), SandboxTransformError>;
}

#[derive(Clone, Default)]
pub struct SandboxManager {
    executor_path_convention: Option<PathConvention>,
    #[cfg(windows)]
    windows_token_session: Option<WindowsSandboxProxySettingsMode>,
    linux_sandbox_pid_namespace: LinuxSandboxPidNamespace,
    linux_sandbox_transparent_tcp: bool,
    operation: SandboxOperation,
    #[cfg(target_os = "macos")]
    allowed_symlinked_codex_home: Option<AbsolutePathBuf>,
}

impl SandboxManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Evaluate the need for sandboxing using the selected executor's path convention.
    pub fn with_executor_path_convention(mut self, convention: PathConvention) -> Self {
        self.executor_path_convention = Some(convention);
        self
    }

    /// Creates a manager that applies the narrower runtime profile required by filesystem helpers.
    pub fn for_file_system_helpers() -> Self {
        Self {
            operation: SandboxOperation::FileSystemHelper,
            ..Self::default()
        }
    }

    /// Prepare commands that inherit the controlling terminal. On macOS this also
    /// grants access to the named Unix sockets and denies terminal input injection.
    pub fn for_inherited_terminal(allow_unix_sockets: &[AbsolutePathBuf]) -> Self {
        #[cfg(not(target_os = "macos"))]
        let _ = allow_unix_sockets;
        Self {
            #[cfg(target_os = "macos")]
            operation: SandboxOperation::InheritedTerminal {
                allow_unix_sockets: allow_unix_sockets.to_vec(),
            },
            ..Self::default()
        }
    }

    /// Prepare the Windows token session for `WrappedSandboxCommand::spawn`.
    /// By default the caller supplies its own token launcher.
    #[cfg(windows)]
    pub fn with_windows_token_session(
        mut self,
        proxy_settings: WindowsSandboxProxySettingsMode,
    ) -> Self {
        self.windows_token_session = Some(proxy_settings);
        self
    }

    /// Applies a trusted executor startup policy, never a command or repository setting.
    pub fn with_linux_sandbox_pid_namespace(mut self, mode: LinuxSandboxPidNamespace) -> Self {
        self.linux_sandbox_pid_namespace = mode;
        self
    }

    /// Route managed Linux command traffic through the transparent TCP adapter.
    pub fn with_linux_sandbox_transparent_tcp(mut self, enabled: bool) -> Self {
        self.linux_sandbox_transparent_tcp = enabled;
        self
    }

    /// Allows otherwise-authorized writable roots beneath the opted-in user home
    /// to follow symlinks, including targets outside that home.
    #[cfg(target_os = "macos")]
    pub fn with_allowed_symlinked_codex_home(
        mut self,
        allowed_symlinked_codex_home: Option<AbsolutePathBuf>,
    ) -> Self {
        self.allowed_symlinked_codex_home = allowed_symlinked_codex_home;
        self
    }

    pub fn select_initial(
        &self,
        permission_profile: &PermissionProfile,
        pref: SandboxablePreference,
        windows_sandbox_type: SandboxType,
        has_managed_network_requirements: bool,
    ) -> SandboxType {
        #[cfg(windows)]
        crate::windows_mxc::record_availability_once();

        if !self.should_sandbox(permission_profile, pref, has_managed_network_requirements) {
            return SandboxType::None;
        }
        if cfg!(windows) && windows_sandbox_type == SandboxType::WindowsMxc {
            return SandboxType::WindowsMxc;
        }
        get_platform_sandbox(windows_sandbox_type != SandboxType::None).unwrap_or(SandboxType::None)
    }

    /// Returns whether the request needs a sandbox, independently of whether
    /// this host can provide a concrete sandbox implementation.
    pub fn should_sandbox(
        &self,
        permission_profile: &PermissionProfile,
        pref: SandboxablePreference,
        has_managed_network_requirements: bool,
    ) -> bool {
        match pref {
            SandboxablePreference::Forbid => false,
            SandboxablePreference::Require => true,
            SandboxablePreference::Auto => {
                let (file_system_policy, network_policy) =
                    permission_profile.to_runtime_permissions();
                should_require_platform_sandbox_for_convention(
                    &file_system_policy,
                    network_policy,
                    has_managed_network_requirements,
                    self.executor_path_convention
                        .unwrap_or(PathConvention::native()),
                )
            }
        }
    }

    /// Prepare filesystem and network enforcement for a command on the execution host.
    pub fn transform(
        &self,
        request: SandboxTransformRequest<'_>,
    ) -> Result<WrappedSandboxCommand, SandboxTransformError> {
        let mut runtime_readable_paths = Vec::new();
        if let SandboxExecutables::Executor {
            codex,
            linux_sandbox,
            arg0,
        } = request.executables
        {
            if matches!(self.operation, SandboxOperation::FileSystemHelper) {
                if !request.use_legacy_landlock {
                    runtime_readable_paths.push(codex.clone());
                    runtime_readable_paths
                        .extend(linux_sandbox.filter(|path| *path != codex).cloned());
                }
            } else {
                if cfg!(target_os = "linux") || (cfg!(unix) && arg0.is_some()) {
                    runtime_readable_paths.push(codex.clone());
                }
                if cfg!(target_os = "linux") {
                    runtime_readable_paths.extend(linux_sandbox.cloned());
                }
            }
        }
        let mut command = request.command;
        let SandboxNetwork {
            enforce_managed_network,
            proxy: network,
            environment_id,
        } = request.network;
        let is_file_system_helper = matches!(self.operation, SandboxOperation::FileSystemHelper);
        if is_file_system_helper
            && (network.is_some() || enforce_managed_network || command.managed_network.is_some())
        {
            return Err(SandboxTransformError::InvalidPolicy(io::Error::new(
                io::ErrorKind::InvalidInput,
                "filesystem helpers cannot use managed networking",
            )));
        }
        // Bind the proxy before resolving HOME and TMPDIR from the child environment.
        let managed_network = command.managed_network.take();
        let managed_network = match (managed_network, network) {
            // A caller-prepared route already includes shell-specific credential handling.
            (None, Some(proxy)) => {
                let prepared = proxy
                    .prepare_for_optional_environment(
                        std::mem::take(&mut command.env),
                        environment_id,
                    )
                    .map_err(|err| {
                        SandboxTransformError::EnvironmentNetworkProxy(err.to_string())
                    })?;
                command.env = prepared.env;
                Some(prepared.sandbox_context)
            }
            (route, _) => route,
        };
        let permissions = effective_permission_profile(
            request.permissions,
            command.additional_permissions.as_ref(),
        );
        let policy = PreparedLaunchPolicy::resolve(
            permissions,
            request.sandbox_policy_cwd,
            request.workspace_roots,
            &runtime_readable_paths,
            &self.operation,
        )
        .map_err(SandboxTransformError::InvalidPolicy)?;
        let backend = match request.selection {
            SandboxSelection::Explicit(sandbox) => sandbox,
            SandboxSelection::Platform {
                preference,
                windows_sandbox,
            } => {
                let sandbox = self.select_initial(
                    request.permissions,
                    preference,
                    windows_sandbox,
                    enforce_managed_network,
                );
                if preference == SandboxablePreference::Require && sandbox == SandboxType::None {
                    return Err(SandboxTransformError::SandboxUnavailable);
                }
                sandbox
            }
        };
        if is_file_system_helper && backend == SandboxType::None {
            return Err(SandboxTransformError::SandboxUnavailable);
        }
        #[cfg(windows)]
        if backend == SandboxType::WindowsRestrictedToken
            && enforce_managed_network
            && request.windows_sandbox_level != WindowsSandboxLevel::Elevated
        {
            return Err(SandboxTransformError::WindowsSandboxPreparation(
                "managed networking requires the elevated Windows sandbox backend".into(),
            ));
        }
        let sandbox_exe = match request.executables {
            SandboxExecutables::Launcher(path) => path,
            SandboxExecutables::Executor { codex, .. } if cfg!(windows) => Some(codex.as_path()),
            SandboxExecutables::Executor { linux_sandbox, .. } => {
                linux_sandbox.map(AbsolutePathBuf::as_path)
            }
        };
        let native_program = match backend {
            SandboxType::None => Some(command.program.as_os_str()),
            SandboxType::LinuxSeccomp | SandboxType::WindowsMxc => sandbox_exe.map(Path::as_os_str),
            #[cfg(windows)]
            SandboxType::WindowsRestrictedToken if is_file_system_helper => {
                sandbox_exe.map(Path::as_os_str)
            }
            SandboxType::MacosSeatbelt | SandboxType::WindowsRestrictedToken => None,
        }
        .filter(|path| path.to_str().is_none())
        .map(OsStr::to_owned);
        let inner_arg_count = 1 + command.args.len();
        let wrapped = |request, windows_session| WrappedSandboxCommand {
            request,
            windows_session,
            inner_arg_count,
        };
        let mut argv = Vec::with_capacity(1 + command.args.len());
        argv.push(command.program.to_string_lossy().into_owned());
        argv.extend(command.args);
        let mut exec = SandboxExecRequest {
            sandbox_override: SandboxOverride::NoOverride,
            command: argv,
            native_program,
            cwd: command.cwd,
            sandbox_policy_cwd: request.sandbox_policy_cwd.clone(),
            env: command.env,
            network: network.cloned(),
            network_environment_id: environment_id.map(str::to_owned),
            sandbox: backend,
            windows_sandbox_level: request.windows_sandbox_level,
            permission_profile: policy.permissions,
            arg0: None,
        };
        if backend == SandboxType::None {
            return Ok(wrapped(exec, None));
        }
        let (managed_network, managed_mitm_ca_trust_bundle_path) = match managed_network {
            Some(route) => {
                let (portable, bundle) = route.into_sandbox_inputs(&exec.env);
                (Some(portable), bundle)
            }
            None => (None, None),
        };
        let managed_network = managed_network
            .filter(|_| enforce_managed_network || backend == SandboxType::MacosSeatbelt);
        #[cfg(windows)]
        let mut session = None;
        #[cfg(not(windows))]
        let session = None;
        let platform: &mut dyn SandboxBackend = match backend {
            SandboxType::None => unreachable!("native launch returned above"),
            SandboxType::LinuxSeccomp => &mut crate::linux::Linux {
                pid_namespace: self.linux_sandbox_pid_namespace,
                transparent_tcp: self.linux_sandbox_transparent_tcp,
                sandbox_exe,
                use_legacy_landlock: request.use_legacy_landlock,
                route: enforce_managed_network.then(|| managed_network.unwrap_or_default()),
            },
            #[cfg(target_os = "macos")]
            SandboxType::MacosSeatbelt => &mut crate::seatbelt::Seatbelt {
                operation: &self.operation,
                allowed_symlinked_codex_home: self.allowed_symlinked_codex_home.as_ref(),
                enforce_managed_network,
                route: managed_network.as_ref(),
            },
            #[cfg(not(target_os = "macos"))]
            SandboxType::MacosSeatbelt => return Err(SandboxTransformError::SeatbeltUnavailable),
            SandboxType::WindowsMxc => {
                if !codex_mxc_sandbox::is_available() {
                    return Err(SandboxTransformError::WindowsMxcPreparation(
                        "native MXC is unavailable on this executor".into(),
                    ));
                }
                if enforce_managed_network && managed_network.is_none() {
                    return Err(SandboxTransformError::WindowsMxcPreparation(
                        "managed networking requires an executor-local proxy".into(),
                    ));
                }
                &mut crate::windows_mxc::Mxc {
                    sandbox_exe,
                    route: managed_network.as_ref(),
                }
            }
            #[cfg(not(windows))]
            SandboxType::WindowsRestrictedToken => {
                prepare_native_sandbox(&mut exec, managed_mitm_ca_trust_bundle_path.as_ref())?;
                if is_file_system_helper {
                    return Err(SandboxTransformError::SandboxUnavailable);
                }
                return Ok(wrapped(exec, None));
            }
            #[cfg(windows)]
            SandboxType::WindowsRestrictedToken => &mut crate::windows::WindowsToken {
                proxy_settings: self.windows_token_session,
                file_system_helper: is_file_system_helper,
                sandbox_exe,
                workspace_roots: policy.workspace_roots,
                enforce_managed_network,
                network,
                environment_id,
                session: &mut session,
            },
        };
        let (cwd, policy_cwd) =
            prepare_native_sandbox(&mut exec, managed_mitm_ca_trust_bundle_path.as_ref())?;
        platform.prepare(&mut exec, &cwd, &policy_cwd)?;
        Ok(wrapped(exec, session))
    }
}

#[cfg(windows)]
pub(super) fn windows_proxy_restricting_sid(
    proxy: Option<&NetworkProxy>,
    environment_id: Option<&str>,
) -> Result<Option<String>, SandboxTransformError> {
    proxy
        .map(|network| {
            network
                .network_proxy_restricting_sid(environment_id)
                .ok_or_else(|| {
                    SandboxTransformError::WindowsSandboxPreparation(
                        "managed Windows proxy route is missing its restricting SID".to_string(),
                    )
                })
        })
        .transpose()
}

#[cfg(target_os = "windows")]
pub(super) fn wrap_windows_file_system_helper(
    request: &mut SandboxExecRequest,
    workspace_roots: &[AbsolutePathBuf],
    codex_home: &Path,
    sandbox_exe: Option<&Path>,
) -> Result<(), SandboxTransformError> {
    let sandbox_exe = sandbox_exe.ok_or_else(|| {
        SandboxTransformError::WindowsSandboxPreparation(
            "missing Codex executable path".to_string(),
        )
    })?;
    // TODO(anp): Keep PathUri through the Windows sandbox wrapper boundary.
    let native_cwd =
        request
            .cwd
            .to_abs_path()
            .map_err(|source| SandboxTransformError::InvalidCommandCwd {
                cwd: request.cwd.clone(),
                source,
            })?;
    let native_sandbox_policy_cwd = request.sandbox_policy_cwd.to_abs_path().map_err(|source| {
        SandboxTransformError::InvalidSandboxPolicyCwd {
            cwd: request.sandbox_policy_cwd.clone(),
            source,
        }
    })?;
    let Some(program) = request.command.first_mut() else {
        return Err(SandboxTransformError::WindowsSandboxPreparation(
            "sandbox command was empty".to_string(),
        ));
    };
    // transform() may have made the inner command a Codex helper. Only that
    // helper needs materializing; an arbitrary workload such as cmd.exe does not.
    if Path::new(program.as_str()) == sandbox_exe {
        let helper = codex_windows_sandbox::resolve_exe_for_launch(sandbox_exe, codex_home);
        *program = helper.to_string_lossy().into_owned();
    }

    let inner_command = std::mem::take(&mut request.command);
    let proxy_enforced = request.network.is_some();
    let network_proxy_restricting_sid = windows_proxy_restricting_sid(
        request.network.as_ref(),
        request.network_environment_id.as_deref(),
    )?;
    let overrides = WindowsSandboxFilesystemOverrides::resolve(
        request.sandbox,
        &request.permission_profile,
        &native_sandbox_policy_cwd,
        request.windows_sandbox_level,
        &request.env,
    )
    .map_err(SandboxTransformError::WindowsSandboxPreparation)?;
    let empty_paths: &[AbsolutePathBuf] = &[];
    let read_roots_override = overrides
        .as_ref()
        .and_then(|overrides| overrides.read_roots_override.as_deref());
    let read_roots_include_platform_defaults = overrides
        .as_ref()
        .is_some_and(|overrides| overrides.read_roots_include_platform_defaults);
    let write_roots_override = overrides
        .as_ref()
        .and_then(|overrides| overrides.write_roots_override.as_deref());
    let deny_read_paths_override = overrides.as_ref().map_or(empty_paths, |overrides| {
        overrides.additional_deny_read_paths.as_slice()
    });
    let deny_write_paths_override = overrides.as_ref().map_or(empty_paths, |overrides| {
        overrides.additional_deny_write_paths.as_slice()
    });
    let mut wrapper_args =
        codex_windows_sandbox::create_windows_sandbox_command_args_for_permission_profile(
            inner_command,
            &native_cwd,
            workspace_roots,
            &mut request.env,
            &request.permission_profile,
            request.windows_sandbox_level,
            proxy_enforced,
            network_proxy_restricting_sid.as_deref(),
            WindowsSandboxProxySettingsMode::Preserve,
            read_roots_override,
            read_roots_include_platform_defaults,
            write_roots_override,
            deny_read_paths_override,
            deny_write_paths_override,
            codex_home,
        )
        .map_err(|err| SandboxTransformError::WindowsSandboxPreparation(err.to_string()))?;

    request.command = Vec::with_capacity(1 + wrapper_args.len());
    // This outer process interprets Codex wrapper arguments, so it must be the
    // supplied Codex executable even when the inner command is another program.
    request
        .command
        .push(sandbox_exe.to_string_lossy().into_owned());
    request.command.append(&mut wrapper_args);
    request.sandbox = SandboxType::None;
    request.arg0 = None;
    add_windows_sandbox_wrapper_setup_env(&mut request.env);
    Ok(())
}

#[cfg(target_os = "windows")]
fn add_windows_sandbox_wrapper_setup_env(env: &mut HashMap<String, String>) {
    add_windows_sandbox_wrapper_setup_env_from_vars(
        env,
        std::env::vars_os(),
        codex_windows_sandbox::registered_core_requested(),
    );
}

#[cfg(target_os = "windows")]
fn add_windows_sandbox_wrapper_setup_env_from_vars(
    env: &mut HashMap<String, String>,
    vars: impl IntoIterator<Item = (std::ffi::OsString, std::ffi::OsString)>,
    registered_core: bool,
) {
    // This outer helper must use the parent's runtime selection, not shell-policy overrides.
    env.retain(|key, _| !key.eq_ignore_ascii_case("CODEX_WINDOWS_REGISTERED_CORE"));
    if registered_core {
        env.insert("CODEX_WINDOWS_REGISTERED_CORE".into(), "1".into());
    }
    for (key, value) in vars {
        let key = key.to_string_lossy().into_owned();
        if !WINDOWS_SANDBOX_WRAPPER_SETUP_ENV_ALLOWLIST
            .iter()
            .any(|allowed| key.eq_ignore_ascii_case(allowed))
        {
            continue;
        }
        env.retain(|existing, _| !existing.eq_ignore_ascii_case(&key));
        env.insert(key, value.to_string_lossy().into_owned());
    }
}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
