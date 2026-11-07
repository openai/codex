use self::launch::PreparedLaunchPolicy;
use crate::LinuxSandboxPidNamespace;
#[cfg(windows)]
use crate::WindowsSandboxFilesystemOverrides;
#[cfg(target_os = "linux")]
use crate::bwrap::WSL1_BWRAP_WARNING;
use crate::landlock::CODEX_LINUX_SANDBOX_ARG0;
use crate::policy_transforms::effective_permission_profile;
use crate::policy_transforms::should_require_platform_sandbox;
#[cfg(target_os = "macos")]
use crate::seatbelt::MacosSeatbeltProfile;
use codex_network_proxy::ManagedNetworkSandboxContext;
use codex_network_proxy::NetworkProxy;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::sandbox::SandboxOverride;
pub use codex_protocol::sandbox::SandboxType;
use codex_utils_absolute_path::AbsolutePathBuf;
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
    pub managed_network: Option<ManagedNetworkSandboxContext>,
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

/// Bundled arguments for sandbox transformation.
///
/// This keeps call sites self-documenting when several fields are optional.
pub struct SandboxTransformRequest<'a> {
    pub command: SandboxCommand,
    pub permissions: &'a PermissionProfile,
    /// Bind symbolic workspace rules to these executor roots. `None` means they are already resolved.
    pub workspace_roots: Option<&'a [PathUri]>,
    pub sandbox: SandboxType,
    pub enforce_managed_network: bool,
    pub environment_id: Option<&'a str>,
    // TODO(viyatb): Evaluate switching this to Option<Arc<NetworkProxy>>
    // to make shared ownership explicit across runtime/sandbox plumbing.
    pub network: Option<&'a NetworkProxy>,
    pub sandbox_policy_cwd: &'a PathUri,
    pub sandbox_exe: Option<&'a Path>,
    // TODO(anp): Reconcile these backend inputs with the supplied sandbox context
    // (TurnEnvironment::sandbox_context for turns) so selection shares its authority.
    pub use_legacy_landlock: bool,
    pub windows_sandbox_level: WindowsSandboxLevel,
}

/// Bundled arguments for a sandbox transformation whose result will be spawned
/// directly from argv.
///
/// Direct-spawn callers will not run a later platform-specific launcher, so the
/// returned command must encode any sandbox wrapper it needs.
pub struct SandboxDirectSpawnTransformRequest<'a> {
    pub transform: SandboxTransformRequest<'a>,
    pub workspace_roots: &'a [AbsolutePathBuf],
    pub windows_sandbox_proxy_settings_mode: codex_windows_sandbox::WindowsSandboxProxySettingsMode,
}

// TODO(anp): Revisit this preparation type once this module's PathUri migration is complete.
struct PendingSandboxedExecRequest {
    native_command_cwd: AbsolutePathBuf,
    native_sandbox_policy_cwd: AbsolutePathBuf,
    effective_permission_profile: PermissionProfile,
}

impl PendingSandboxedExecRequest {
    fn new(
        command_cwd: &PathUri,
        sandbox_policy_cwd: &PathUri,
        effective_permission_profile: PermissionProfile,
        managed_mitm_ca_trust_bundle_path: Option<&AbsolutePathBuf>,
    ) -> Result<Self, SandboxTransformError> {
        // TODO(anp): Move PathUri conversion into the platform sandbox implementations.
        let native_command_cwd = command_cwd.to_abs_path().map_err(|source| {
            SandboxTransformError::InvalidCommandCwd {
                cwd: command_cwd.clone(),
                source,
            }
        })?;
        let native_sandbox_policy_cwd = sandbox_policy_cwd.to_abs_path().map_err(|source| {
            SandboxTransformError::InvalidSandboxPolicyCwd {
                cwd: sandbox_policy_cwd.clone(),
                source,
            }
        })?;
        let effective_permission_profile = with_managed_mitm_ca_readable_root(
            effective_permission_profile,
            managed_mitm_ca_trust_bundle_path,
            native_sandbox_policy_cwd.as_path(),
        );
        Ok(Self {
            native_command_cwd,
            native_sandbox_policy_cwd,
            effective_permission_profile,
        })
    }
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
            Self::MissingLinuxSandboxExecutable => None,
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

#[derive(Clone, Default)]
pub struct SandboxManager {
    linux_sandbox_pid_namespace: LinuxSandboxPidNamespace,
    #[cfg(target_os = "macos")]
    seatbelt_profile: MacosSeatbeltProfile,
    #[cfg(target_os = "macos")]
    allowed_symlinked_codex_home: Option<AbsolutePathBuf>,
}

impl SandboxManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a manager that applies the narrower runtime profile required by filesystem helpers.
    pub fn for_file_system_helpers() -> Self {
        Self {
            linux_sandbox_pid_namespace: LinuxSandboxPidNamespace::default(),
            #[cfg(target_os = "macos")]
            seatbelt_profile: MacosSeatbeltProfile::FileSystemHelper,
            #[cfg(target_os = "macos")]
            allowed_symlinked_codex_home: None,
        }
    }

    /// Applies a trusted executor startup policy, never a command or repository setting.
    pub fn with_linux_sandbox_pid_namespace(mut self, mode: LinuxSandboxPidNamespace) -> Self {
        self.linux_sandbox_pid_namespace = mode;
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
                should_require_platform_sandbox(
                    &file_system_policy,
                    network_policy,
                    has_managed_network_requirements,
                )
            }
        }
    }

    /// Prepare filesystem and network enforcement for a command on the execution host.
    pub fn transform(
        &self,
        request: SandboxTransformRequest<'_>,
    ) -> Result<SandboxExecRequest, SandboxTransformError> {
        #[cfg(windows)]
        if request.sandbox == SandboxType::WindowsRestrictedToken
            && request.enforce_managed_network
            && request.windows_sandbox_level != WindowsSandboxLevel::Elevated
        {
            return Err(SandboxTransformError::WindowsSandboxPreparation(
                "managed networking requires the elevated Windows sandbox backend".into(),
            ));
        }
        let backend = request.sandbox;
        let command = request.command;
        let managed_network = command
            .managed_network
            .filter(|_| request.enforce_managed_network || backend == SandboxType::MacosSeatbelt);
        let managed_mitm_ca_trust_bundle_path = request
            .network
            .and_then(NetworkProxy::managed_mitm_ca_trust_bundle_path);
        let permissions = effective_permission_profile(
            request.permissions,
            command.additional_permissions.as_ref(),
        );
        let policy = PreparedLaunchPolicy::resolve(
            permissions,
            request.sandbox_policy_cwd,
            request.workspace_roots,
        )
        .map_err(SandboxTransformError::InvalidPolicy)?;
        let mut argv = Vec::with_capacity(1 + command.args.len());
        argv.push(command.program.to_string_lossy().into_owned());
        argv.extend(command.args);
        let mut exec = SandboxExecRequest {
            sandbox_override: SandboxOverride::NoOverride,
            command: argv,
            cwd: command.cwd,
            sandbox_policy_cwd: request.sandbox_policy_cwd.clone(),
            env: command.env,
            network: request.network.cloned(),
            network_environment_id: request.environment_id.map(str::to_owned),
            sandbox: backend,
            windows_sandbox_level: request.windows_sandbox_level,
            permission_profile: policy.permissions,
            arg0: None,
        };
        if backend == SandboxType::None {
            return Ok(exec);
        }
        let pending = PendingSandboxedExecRequest::new(
            &exec.cwd,
            &exec.sandbox_policy_cwd,
            exec.permission_profile,
            managed_mitm_ca_trust_bundle_path.as_ref(),
        );
        match backend {
            SandboxType::None => unreachable!("native launch returned above"),
            SandboxType::WindowsMxc => {
                if !codex_mxc_sandbox::is_available() {
                    return Err(SandboxTransformError::WindowsMxcPreparation(
                        "native MXC is unavailable on this executor".into(),
                    ));
                }
                let route = match managed_network {
                    None if request.enforce_managed_network => {
                        let proxy = request.network.ok_or_else(|| {
                            SandboxTransformError::WindowsMxcPreparation(
                                "managed networking requires an executor-local proxy".to_string(),
                            )
                        })?;
                        let prepared = proxy
                            .prepare_for_optional_environment(exec.env, request.environment_id)
                            .map_err(|err| {
                                SandboxTransformError::EnvironmentNetworkProxy(err.to_string())
                            })?;
                        exec.env = prepared.env;
                        Some(prepared.sandbox_context)
                    }
                    route => route,
                };
                let pending = pending?;
                exec.permission_profile = pending.effective_permission_profile;
                let exe = request.sandbox_exe.ok_or_else(|| {
                    SandboxTransformError::WindowsMxcPreparation(
                        "missing Codex executable path".into(),
                    )
                })?;
                let args = codex_mxc_sandbox::create_command_args(
                    codex_mxc_sandbox::CreateMxcCommandArgsParams {
                        command: std::mem::take(&mut exec.command),
                        permission_profile: &exec.permission_profile,
                        sandbox_policy_cwd: &pending.native_sandbox_policy_cwd,
                        managed_network: route.as_ref(),
                        env: &mut exec.env,
                    },
                )
                .map_err(|err| SandboxTransformError::WindowsMxcPreparation(err.to_string()))?;
                exec.command = std::iter::once(exe.to_string_lossy().into_owned())
                    .chain(args)
                    .collect();
            }
            #[cfg(target_os = "macos")]
            SandboxType::MacosSeatbelt => {
                use crate::seatbelt::CreateSeatbeltCommandArgsParams;
                use crate::seatbelt::MACOS_PATH_TO_SEATBELT_EXECUTABLE;
                use crate::seatbelt::SeatbeltPreparationError;
                use crate::seatbelt::create_seatbelt_command_args_with_profile;

                let pending = pending?;
                exec.permission_profile = pending.effective_permission_profile;
                let (file_system_sandbox_policy, network_sandbox_policy) =
                    exec.permission_profile.to_runtime_permissions();
                let args = create_seatbelt_command_args_with_profile(
                    CreateSeatbeltCommandArgsParams {
                        command: std::mem::take(&mut exec.command),
                        file_system_sandbox_policy: &file_system_sandbox_policy,
                        network_sandbox_policy,
                        sandbox_policy_cwd: &pending.native_sandbox_policy_cwd,
                        enforce_managed_network: request.enforce_managed_network,
                        managed_network: managed_network.as_ref(),
                        environment_id: request.environment_id,
                        network: request.network,
                        extra_allow_unix_sockets: &[],
                    },
                    self.seatbelt_profile,
                    self.allowed_symlinked_codex_home.as_ref(),
                )
                .map_err(|err| match err {
                    SeatbeltPreparationError::FileSystem(message) => {
                        SandboxTransformError::SeatbeltPreparation(message)
                    }
                    SeatbeltPreparationError::EnvironmentNetworkProxy(message) => {
                        SandboxTransformError::EnvironmentNetworkProxy(message)
                    }
                })?;
                exec.command = std::iter::once(MACOS_PATH_TO_SEATBELT_EXECUTABLE.to_owned())
                    .chain(args)
                    .collect();
            }
            #[cfg(not(target_os = "macos"))]
            SandboxType::MacosSeatbelt => return Err(SandboxTransformError::SeatbeltUnavailable),
            SandboxType::LinuxSeccomp => {
                let pending = pending?;
                exec.permission_profile = pending.effective_permission_profile;
                let exe = request
                    .sandbox_exe
                    .ok_or(SandboxTransformError::MissingLinuxSandboxExecutable)?;
                let route = match (
                    request.enforce_managed_network,
                    managed_network,
                    request.network,
                ) {
                    (false, _, _) => None,
                    (true, None, Some(proxy)) => {
                        let prepared = proxy
                            .prepare_for_optional_environment(exec.env, request.environment_id)
                            .map_err(|err| {
                                SandboxTransformError::EnvironmentNetworkProxy(err.to_string())
                            })?;
                        exec.env = prepared.env;
                        Some(prepared.sandbox_context)
                    }
                    (true, context, _) => Some(context.unwrap_or_default()),
                };
                self.wrap_linux(
                    &mut exec,
                    (
                        &pending.native_command_cwd,
                        &pending.native_sandbox_policy_cwd,
                    ),
                    exe,
                    request.use_legacy_landlock,
                    route.as_ref(),
                )?;
            }
            SandboxType::WindowsRestrictedToken => {
                exec.permission_profile = pending?.effective_permission_profile;
            }
        }
        Ok(exec)
    }

    fn wrap_linux(
        &self,
        exec: &mut SandboxExecRequest,
        (cwd, policy_cwd): (&AbsolutePathBuf, &AbsolutePathBuf),
        exe: &Path,
        use_legacy_landlock: bool,
        managed_network: Option<&ManagedNetworkSandboxContext>,
    ) -> Result<(), SandboxTransformError> {
        #[cfg(target_os = "linux")]
        ensure_linux_bubblewrap_is_supported(
            &exec.permission_profile.file_system_sandbox_policy(),
            use_legacy_landlock,
            managed_network.is_some(),
            crate::bwrap::is_wsl1(),
        )?;
        let program = exe.to_string_lossy().into_owned();
        let mut args = crate::landlock::create_linux_sandbox_command_args_for_permission_profile(
            std::mem::take(&mut exec.command),
            cwd,
            &exec.permission_profile,
            policy_cwd,
            use_legacy_landlock,
            managed_network,
        );
        // Only the startup opt-in requires a helper that understands PID inheritance.
        if self.linux_sandbox_pid_namespace == LinuxSandboxPidNamespace::Inherit {
            args.insert(0, "--inherit-pid-namespace".to_owned());
        }
        exec.command = std::iter::once(program.clone()).chain(args).collect();
        let arg0 = if exe.file_name() == Some(OsStr::new(CODEX_LINUX_SANDBOX_ARG0)) {
            &program
        } else {
            CODEX_LINUX_SANDBOX_ARG0
        };
        exec.arg0 = Some(arg0.to_owned());
        Ok(())
    }

    pub fn transform_for_direct_spawn(
        &self,
        request: SandboxDirectSpawnTransformRequest<'_>,
    ) -> Result<SandboxExecRequest, SandboxTransformError> {
        #[cfg(windows)]
        {
            self.transform_for_direct_spawn_with_codex_home(request, /*codex_home*/ None)
        }
        #[cfg(not(windows))]
        {
            self.transform(request.transform)
        }
    }

    #[cfg(target_os = "windows")]
    fn transform_for_direct_spawn_with_codex_home(
        &self,
        request: SandboxDirectSpawnTransformRequest<'_>,
        codex_home: Option<&Path>,
    ) -> Result<SandboxExecRequest, SandboxTransformError> {
        let roots = request.transform.workspace_roots;
        let fallback_roots = request.workspace_roots;
        let proxy_settings_mode = request.windows_sandbox_proxy_settings_mode;
        let sandbox_exe = request.transform.sandbox_exe;
        let mut request = self.transform(request.transform)?;
        if request.sandbox != SandboxType::WindowsRestrictedToken {
            return Ok(request);
        }
        let owned_codex_home;
        let codex_home = if let Some(codex_home) = codex_home {
            codex_home
        } else {
            owned_codex_home = codex_utils_home_dir::find_codex_home()
                .map_err(|err| SandboxTransformError::WindowsSandboxPreparation(err.to_string()))?;
            owned_codex_home.as_path()
        };
        let workspace_roots = roots
            .map(launch::native_workspace_roots)
            .transpose()
            .map_err(SandboxTransformError::InvalidPolicy)?;
        let sandbox_exe = sandbox_exe.ok_or_else(|| {
            SandboxTransformError::WindowsSandboxPreparation(
                "missing Codex executable path".to_string(),
            )
        })?;
        wrap_windows_sandbox_exec_request_for_direct_spawn(
            &mut request,
            workspace_roots.as_deref().unwrap_or(fallback_roots),
            codex_home,
            sandbox_exe,
            proxy_settings_mode,
        )?;
        Ok(request)
    }
}

#[cfg(target_os = "windows")]
fn wrap_windows_sandbox_exec_request_for_direct_spawn(
    request: &mut SandboxExecRequest,
    workspace_roots: &[AbsolutePathBuf],
    codex_home: &Path,
    sandbox_exe: &Path,
    proxy_settings_mode: codex_windows_sandbox::WindowsSandboxProxySettingsMode,
) -> Result<(), SandboxTransformError> {
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
    let network_proxy_restricting_sid = request
        .network
        .as_ref()
        .map(|network| {
            network
                .network_proxy_restricting_sid(request.network_environment_id.as_deref())
                .ok_or_else(|| {
                    SandboxTransformError::WindowsSandboxPreparation(
                        "managed Windows proxy route is missing its restricting SID".to_string(),
                    )
                })
        })
        .transpose()?;
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
            proxy_settings_mode,
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

pub fn compatibility_sandbox_policy_for_permission_profile(
    permissions: &PermissionProfile,
    cwd: &Path,
) -> SandboxPolicy {
    permissions
        .to_legacy_sandbox_policy(cwd)
        .unwrap_or_else(|_| {
            let (file_system_policy, network_policy) = permissions.to_runtime_permissions();
            compatibility_workspace_write_policy(file_system_policy, network_policy, cwd)
        })
}

fn compatibility_workspace_write_policy(
    file_system_policy: FileSystemSandboxPolicy,
    network_policy: NetworkSandboxPolicy,
    cwd: &Path,
) -> SandboxPolicy {
    let cwd_abs = AbsolutePathBuf::from_absolute_path(cwd).ok();
    let writable_roots = file_system_policy
        .get_writable_roots_with_cwd(cwd)
        .into_iter()
        .map(|root| root.root)
        .filter(|root| cwd_abs.as_ref() != Some(root))
        .collect();
    let tmpdir_writable = std::env::var_os("TMPDIR")
        .filter(|tmpdir| !tmpdir.is_empty())
        .and_then(|tmpdir| {
            AbsolutePathBuf::from_absolute_path(std::path::PathBuf::from(tmpdir)).ok()
        })
        .is_some_and(|tmpdir| {
            file_system_policy.can_write_local_path_with_cwd(tmpdir.as_path(), cwd)
        });
    let slash_tmp = Path::new("/tmp");
    let slash_tmp_writable = slash_tmp.is_absolute()
        && slash_tmp.is_dir()
        && file_system_policy.can_write_local_path_with_cwd(slash_tmp, cwd);

    SandboxPolicy::WorkspaceWrite {
        writable_roots,
        network_access: network_policy.is_enabled(),
        exclude_tmpdir_env_var: !tmpdir_writable,
        exclude_slash_tmp: !slash_tmp_writable,
    }
}

#[cfg(target_os = "linux")]
fn ensure_linux_bubblewrap_is_supported(
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    use_legacy_landlock: bool,
    allow_network_for_proxy: bool,
    is_wsl1: bool,
) -> Result<(), SandboxTransformError> {
    let requires_bubblewrap = allow_network_for_proxy
        || (!use_legacy_landlock && !file_system_sandbox_policy.has_full_disk_write_access());
    if is_wsl1 && requires_bubblewrap {
        return Err(SandboxTransformError::Wsl1UnsupportedForBubblewrap);
    }

    Ok(())
}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
