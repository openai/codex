use codex_app_server_protocol::*;
use std::collections::HashMap;
use std::path::PathBuf;
use tonic::Status;

use super::grpc::proto;

pub(crate) trait NativeProto: Sized {
    type Proto;

    fn decode(payload: Self::Proto) -> Result<Self, Status>;
    fn encode(self) -> Result<Self::Proto, Status>;
}

pub(crate) fn decode_native<T: NativeProto>(payload: T::Proto) -> Result<T, Status> {
    T::decode(payload)
}

pub(crate) fn encode_native<T: NativeProto>(payload: T) -> Result<T::Proto, Status> {
    payload.encode()
}

macro_rules! empty_native {
    ($($type:ty),+ $(,)?) => {
        $(
            impl NativeProto for $type {
                type Proto = proto::Empty;

                fn decode(_: Self::Proto) -> Result<Self, Status> {
                    Ok(Self {})
                }

                fn encode(self) -> Result<Self::Proto, Status> {
                    Ok(proto::Empty {})
                }
            }
        )+
    };
}

empty_native!(
    ThreadArchiveResponse,
    ThreadSetNameResponse,
    ThreadSettingsUpdateResponse,
    ThreadMemoryModeSetResponse,
    MemoryResetResponse,
    ThreadCompactStartResponse,
    ThreadShellCommandResponse,
    ThreadApproveGuardianDeniedActionResponse,
    ThreadBackgroundTerminalsCleanResponse,
    ThreadInjectItemsResponse,
    SkillsExtraRootsSetResponse,
    PluginShareDeleteResponse,
    PluginUninstallResponse,
    FsWriteFileResponse,
    FsCreateDirectoryResponse,
    FsRemoveResponse,
    FsCopyResponse,
    FsUnwatchResponse,
    TurnInterruptResponse,
    ThreadRealtimeStartResponse,
    ThreadRealtimeAppendAudioResponse,
    ThreadRealtimeAppendTextResponse,
    ThreadRealtimeStopResponse,
    EnvironmentAddResponse,
    McpServerRefreshResponse,
    LogoutAccountResponse,
    CommandExecWriteResponse,
    CommandExecTerminateResponse,
    CommandExecResizeResponse,
    ProcessSpawnResponse,
    ProcessWriteStdinResponse,
    ProcessKillResponse,
    ProcessResizePtyResponse,
    RemoteControlClientsRevokeResponse,
    ExternalAgentConfigImportResponse,
    FuzzyFileSearchSessionStartResponse,
    FuzzyFileSearchSessionUpdateResponse,
    FuzzyFileSearchSessionStopResponse,
    PluginShareListParams,
    ThreadRealtimeListVoicesParams,
    ModelProviderCapabilitiesReadParams,
    CollaborationModeListParams,
    AttestationGenerateParams,
    SkillsChangedNotification,
    ExternalAgentConfigImportCompletedNotification,
);

impl NativeProto for Option<()> {
    type Proto = proto::Empty;

    fn decode(_: Self::Proto) -> Result<Self, Status> {
        Ok(None)
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::Empty {})
    }
}

macro_rules! thread_id_native {
    ($($type:ty),+ $(,)?) => {
        $(
            impl NativeProto for $type {
                type Proto = proto::ThreadIdParams;

                fn decode(payload: Self::Proto) -> Result<Self, Status> {
                    Ok(Self {
                        thread_id: payload.thread_id,
                    })
                }

                fn encode(self) -> Result<Self::Proto, Status> {
                    Ok(proto::ThreadIdParams {
                        thread_id: self.thread_id,
                    })
                }
            }
        )+
    };
}

thread_id_native!(
    ThreadArchiveParams,
    ThreadIncrementElicitationParams,
    ThreadDecrementElicitationParams,
    ThreadGoalGetParams,
    ThreadGoalClearParams,
    ThreadCompactStartParams,
    ThreadBackgroundTerminalsCleanParams,
    ThreadRealtimeStopParams,
    ThreadArchivedNotification,
    ThreadUnarchivedNotification,
    ThreadClosedNotification,
    ThreadGoalClearedNotification,
);

impl NativeProto for ThreadSetNameParams {
    type Proto = proto::ThreadSetNameParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            thread_id: payload.thread_id,
            name: payload.name,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::ThreadSetNameParams {
            thread_id: self.thread_id,
            name: self.name,
        })
    }
}

macro_rules! elicitation_counter_native {
    ($($type:ty),+ $(,)?) => {
        $(
            impl NativeProto for $type {
                type Proto = proto::ElicitationCounterResponse;

                fn decode(payload: Self::Proto) -> Result<Self, Status> {
                    Ok(Self {
                        count: payload.count,
                        paused: payload.paused,
                    })
                }

                fn encode(self) -> Result<Self::Proto, Status> {
                    Ok(proto::ElicitationCounterResponse {
                        count: self.count,
                        paused: self.paused,
                    })
                }
            }
        )+
    };
}

elicitation_counter_native!(
    ThreadIncrementElicitationResponse,
    ThreadDecrementElicitationResponse,
);

fn decode_thread_goal_status(value: i32) -> Result<ThreadGoalStatus, Status> {
    match proto::ThreadGoalStatus::try_from(value)
        .map_err(|_| Status::invalid_argument("invalid thread goal status"))?
    {
        proto::ThreadGoalStatus::Unspecified => {
            Err(Status::invalid_argument("unspecified thread goal status"))
        }
        proto::ThreadGoalStatus::Active => Ok(ThreadGoalStatus::Active),
        proto::ThreadGoalStatus::Paused => Ok(ThreadGoalStatus::Paused),
        proto::ThreadGoalStatus::Blocked => Ok(ThreadGoalStatus::Blocked),
        proto::ThreadGoalStatus::UsageLimited => Ok(ThreadGoalStatus::UsageLimited),
        proto::ThreadGoalStatus::BudgetLimited => Ok(ThreadGoalStatus::BudgetLimited),
        proto::ThreadGoalStatus::Complete => Ok(ThreadGoalStatus::Complete),
    }
}

fn encode_thread_goal_status(value: ThreadGoalStatus) -> i32 {
    match value {
        ThreadGoalStatus::Active => proto::ThreadGoalStatus::Active as i32,
        ThreadGoalStatus::Paused => proto::ThreadGoalStatus::Paused as i32,
        ThreadGoalStatus::Blocked => proto::ThreadGoalStatus::Blocked as i32,
        ThreadGoalStatus::UsageLimited => proto::ThreadGoalStatus::UsageLimited as i32,
        ThreadGoalStatus::BudgetLimited => proto::ThreadGoalStatus::BudgetLimited as i32,
        ThreadGoalStatus::Complete => proto::ThreadGoalStatus::Complete as i32,
    }
}

fn decode_thread_goal(payload: proto::ThreadGoal) -> Result<ThreadGoal, Status> {
    Ok(ThreadGoal {
        thread_id: payload.thread_id,
        objective: payload.objective,
        status: decode_thread_goal_status(payload.status)?,
        token_budget: payload.token_budget,
        tokens_used: payload.tokens_used,
        time_used_seconds: payload.time_used_seconds,
        created_at: payload.created_at,
        updated_at: payload.updated_at,
    })
}

fn encode_thread_goal(payload: ThreadGoal) -> proto::ThreadGoal {
    proto::ThreadGoal {
        thread_id: payload.thread_id,
        objective: payload.objective,
        status: encode_thread_goal_status(payload.status),
        token_budget: payload.token_budget,
        tokens_used: payload.tokens_used,
        time_used_seconds: payload.time_used_seconds,
        created_at: payload.created_at,
        updated_at: payload.updated_at,
    }
}

fn decode_optional_i64(payload: proto::OptionalInt64) -> Result<Option<i64>, Status> {
    match payload.value {
        Some(proto::optional_int64::Value::Some(value)) => Ok(Some(value)),
        Some(proto::optional_int64::Value::Null(_)) => Ok(None),
        None => Err(Status::invalid_argument("missing optional int64 value")),
    }
}

fn encode_optional_i64(payload: Option<i64>) -> proto::OptionalInt64 {
    let value = match payload {
        Some(value) => proto::optional_int64::Value::Some(value),
        None => proto::optional_int64::Value::Null(proto::Empty {}),
    };
    proto::OptionalInt64 { value: Some(value) }
}

fn decode_optional_u64(payload: proto::OptionalUint64) -> Result<Option<u64>, Status> {
    match payload.value {
        Some(proto::optional_uint64::Value::Some(value)) => Ok(Some(value)),
        Some(proto::optional_uint64::Value::Null(_)) => Ok(None),
        None => Err(Status::invalid_argument("missing optional uint64 value")),
    }
}

fn encode_optional_u64(payload: Option<u64>) -> proto::OptionalUint64 {
    let value = match payload {
        Some(value) => proto::optional_uint64::Value::Some(value),
        None => proto::optional_uint64::Value::Null(proto::Empty {}),
    };
    proto::OptionalUint64 { value: Some(value) }
}

impl NativeProto for ThreadGoalSetParams {
    type Proto = proto::ThreadGoalSetParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            thread_id: payload.thread_id,
            objective: payload.objective,
            status: payload.status.map(decode_thread_goal_status).transpose()?,
            token_budget: payload.token_budget.map(decode_optional_i64).transpose()?,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::ThreadGoalSetParams {
            thread_id: self.thread_id,
            objective: self.objective,
            status: self.status.map(encode_thread_goal_status),
            token_budget: self.token_budget.map(encode_optional_i64),
        })
    }
}

impl NativeProto for ThreadGoalSetResponse {
    type Proto = proto::ThreadGoalResponse;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            goal: decode_thread_goal(required(payload.goal, "thread goal")?)?,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::ThreadGoalResponse {
            goal: Some(encode_thread_goal(self.goal)),
        })
    }
}

impl NativeProto for ThreadGoalGetResponse {
    type Proto = proto::ThreadGoalResponse;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            goal: payload.goal.map(decode_thread_goal).transpose()?,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::ThreadGoalResponse {
            goal: self.goal.map(encode_thread_goal),
        })
    }
}

impl NativeProto for ThreadGoalClearResponse {
    type Proto = proto::ThreadGoalClearResponse;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            cleared: payload.cleared,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::ThreadGoalClearResponse {
            cleared: self.cleared,
        })
    }
}

impl NativeProto for ThreadMemoryModeSetParams {
    type Proto = proto::ThreadMemoryModeSetParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        let mode = match proto::ThreadMemoryMode::try_from(payload.mode)
            .map_err(|_| Status::invalid_argument("invalid thread memory mode"))?
        {
            proto::ThreadMemoryMode::Unspecified => {
                return Err(Status::invalid_argument("unspecified thread memory mode"));
            }
            proto::ThreadMemoryMode::Enabled => ThreadMemoryMode::Enabled,
            proto::ThreadMemoryMode::Disabled => ThreadMemoryMode::Disabled,
        };
        Ok(Self {
            thread_id: payload.thread_id,
            mode,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        let mode = match self.mode {
            ThreadMemoryMode::Enabled => proto::ThreadMemoryMode::Enabled,
            ThreadMemoryMode::Disabled => proto::ThreadMemoryMode::Disabled,
        };
        Ok(proto::ThreadMemoryModeSetParams {
            thread_id: self.thread_id,
            mode: mode as i32,
        })
    }
}

impl NativeProto for ThreadShellCommandParams {
    type Proto = proto::ThreadShellCommandParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            thread_id: payload.thread_id,
            command: payload.command,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::ThreadShellCommandParams {
            thread_id: self.thread_id,
            command: self.command,
        })
    }
}

impl NativeProto for TurnInterruptParams {
    type Proto = proto::TurnInterruptParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            thread_id: payload.thread_id,
            turn_id: payload.turn_id,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::TurnInterruptParams {
            thread_id: self.thread_id,
            turn_id: self.turn_id,
        })
    }
}

impl NativeProto for ThreadNameUpdatedNotification {
    type Proto = proto::ThreadNameUpdatedNotification;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            thread_id: payload.thread_id,
            thread_name: payload.thread_name,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::ThreadNameUpdatedNotification {
            thread_id: self.thread_id,
            thread_name: self.thread_name,
        })
    }
}

impl NativeProto for ThreadGoalUpdatedNotification {
    type Proto = proto::ThreadGoalUpdatedNotification;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            thread_id: payload.thread_id,
            turn_id: payload.turn_id,
            goal: decode_thread_goal(required(payload.goal, "thread goal")?)?,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::ThreadGoalUpdatedNotification {
            thread_id: self.thread_id,
            turn_id: self.turn_id,
            goal: Some(encode_thread_goal(self.goal)),
        })
    }
}

macro_rules! fs_path_native {
    ($($type:ty),+ $(,)?) => {
        $(
            impl NativeProto for $type {
                type Proto = proto::FsPathParams;

                fn decode(payload: Self::Proto) -> Result<Self, Status> {
                    Ok(Self {
                        path: payload.path.try_into().map_err(invalid_path)?,
                    })
                }

                fn encode(self) -> Result<Self::Proto, Status> {
                    Ok(proto::FsPathParams {
                        path: self.path.to_string_lossy().into_owned(),
                    })
                }
            }
        )+
    };
}

fs_path_native!(FsReadFileParams, FsGetMetadataParams, FsReadDirectoryParams);

fn invalid_path(error: std::io::Error) -> Status {
    Status::invalid_argument(format!("invalid absolute path: {error}"))
}

impl NativeProto for FsWriteFileParams {
    type Proto = proto::FsWriteFileParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            path: payload.path.try_into().map_err(invalid_path)?,
            data_base64: payload.data_base64,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::FsWriteFileParams {
            path: self.path.to_string_lossy().into_owned(),
            data_base64: self.data_base64,
        })
    }
}

impl NativeProto for FsCreateDirectoryParams {
    type Proto = proto::FsCreateDirectoryParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            path: payload.path.try_into().map_err(invalid_path)?,
            recursive: payload.recursive,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::FsCreateDirectoryParams {
            path: self.path.to_string_lossy().into_owned(),
            recursive: self.recursive,
        })
    }
}

impl NativeProto for FsRemoveParams {
    type Proto = proto::FsRemoveParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            path: payload.path.try_into().map_err(invalid_path)?,
            recursive: payload.recursive,
            force: payload.force,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::FsRemoveParams {
            path: self.path.to_string_lossy().into_owned(),
            recursive: self.recursive,
            force: self.force,
        })
    }
}

impl NativeProto for FsCopyParams {
    type Proto = proto::FsCopyParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            source_path: payload.source_path.try_into().map_err(invalid_path)?,
            destination_path: payload.destination_path.try_into().map_err(invalid_path)?,
            recursive: payload.recursive,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::FsCopyParams {
            source_path: self.source_path.to_string_lossy().into_owned(),
            destination_path: self.destination_path.to_string_lossy().into_owned(),
            recursive: self.recursive,
        })
    }
}

impl NativeProto for FsWatchParams {
    type Proto = proto::FsWatchParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            watch_id: payload.watch_id,
            path: payload.path.try_into().map_err(invalid_path)?,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::FsWatchParams {
            watch_id: self.watch_id,
            path: self.path.to_string_lossy().into_owned(),
        })
    }
}

impl NativeProto for FsUnwatchParams {
    type Proto = proto::FsUnwatchParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            watch_id: payload.watch_id,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::FsUnwatchParams {
            watch_id: self.watch_id,
        })
    }
}

impl NativeProto for FsReadFileResponse {
    type Proto = proto::FsReadFileResponse;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            data_base64: payload.data_base64,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::FsReadFileResponse {
            data_base64: self.data_base64,
        })
    }
}

impl NativeProto for FsGetMetadataResponse {
    type Proto = proto::FsGetMetadataResponse;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            is_directory: payload.is_directory,
            is_file: payload.is_file,
            is_symlink: payload.is_symlink,
            created_at_ms: payload.created_at_ms,
            modified_at_ms: payload.modified_at_ms,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::FsGetMetadataResponse {
            is_directory: self.is_directory,
            is_file: self.is_file,
            is_symlink: self.is_symlink,
            created_at_ms: self.created_at_ms,
            modified_at_ms: self.modified_at_ms,
        })
    }
}

impl NativeProto for FsReadDirectoryResponse {
    type Proto = proto::FsReadDirectoryResponse;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            entries: payload
                .entries
                .into_iter()
                .map(|entry| FsReadDirectoryEntry {
                    file_name: entry.file_name,
                    is_directory: entry.is_directory,
                    is_file: entry.is_file,
                })
                .collect(),
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::FsReadDirectoryResponse {
            entries: self
                .entries
                .into_iter()
                .map(|entry| proto::FsReadDirectoryEntry {
                    file_name: entry.file_name,
                    is_directory: entry.is_directory,
                    is_file: entry.is_file,
                })
                .collect(),
        })
    }
}

impl NativeProto for FsWatchResponse {
    type Proto = proto::FsWatchResponse;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            path: payload.path.try_into().map_err(invalid_path)?,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::FsWatchResponse {
            path: self.path.to_string_lossy().into_owned(),
        })
    }
}

impl NativeProto for FsChangedNotification {
    type Proto = proto::FsChangedNotification;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            watch_id: payload.watch_id,
            changed_paths: payload
                .changed_paths
                .into_iter()
                .map(|path| path.try_into().map_err(invalid_path))
                .collect::<Result<_, _>>()?,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::FsChangedNotification {
            watch_id: self.watch_id,
            changed_paths: self
                .changed_paths
                .into_iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
        })
    }
}

impl NativeProto for EnvironmentAddParams {
    type Proto = proto::EnvironmentAddParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            environment_id: payload.environment_id,
            exec_server_url: payload.exec_server_url,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::EnvironmentAddParams {
            environment_id: self.environment_id,
            exec_server_url: self.exec_server_url,
        })
    }
}

fn decode_terminal_size(payload: proto::TerminalSize) -> Result<(u16, u16), Status> {
    Ok((
        payload
            .rows
            .try_into()
            .map_err(|_| Status::invalid_argument("terminal rows exceed u16"))?,
        payload
            .cols
            .try_into()
            .map_err(|_| Status::invalid_argument("terminal cols exceed u16"))?,
    ))
}

fn encode_terminal_size(rows: u16, cols: u16) -> proto::TerminalSize {
    proto::TerminalSize {
        rows: rows.into(),
        cols: cols.into(),
    }
}

fn decode_env(env: HashMap<String, proto::NullableString>) -> HashMap<String, Option<String>> {
    env.into_iter()
        .map(|(key, value)| (key, value.value))
        .collect()
}

fn encode_env(env: HashMap<String, Option<String>>) -> HashMap<String, proto::NullableString> {
    env.into_iter()
        .map(|(key, value)| (key, proto::NullableString { value }))
        .collect()
}

fn decode_sandbox_policy(payload: proto::SandboxPolicy) -> Result<SandboxPolicy, Status> {
    match required(payload.kind, "sandbox policy")? {
        proto::sandbox_policy::Kind::DangerFullAccess(_) => Ok(SandboxPolicy::DangerFullAccess),
        proto::sandbox_policy::Kind::ReadOnly(policy) => Ok(SandboxPolicy::ReadOnly {
            network_access: policy.network_access,
        }),
        proto::sandbox_policy::Kind::ExternalSandbox(policy) => {
            let network_access = match proto::NetworkAccess::try_from(policy.network_access)
                .map_err(|_| Status::invalid_argument("invalid network access"))?
            {
                proto::NetworkAccess::Unspecified => {
                    return Err(Status::invalid_argument("unspecified network access"));
                }
                proto::NetworkAccess::Restricted => NetworkAccess::Restricted,
                proto::NetworkAccess::Enabled => NetworkAccess::Enabled,
            };
            Ok(SandboxPolicy::ExternalSandbox { network_access })
        }
        proto::sandbox_policy::Kind::WorkspaceWrite(policy) => Ok(SandboxPolicy::WorkspaceWrite {
            writable_roots: policy
                .writable_roots
                .into_iter()
                .map(|path| path.try_into().map_err(invalid_path))
                .collect::<Result<_, _>>()?,
            network_access: policy.network_access,
            exclude_tmpdir_env_var: policy.exclude_tmpdir_env_var,
            exclude_slash_tmp: policy.exclude_slash_tmp,
        }),
    }
}

fn encode_sandbox_policy(payload: SandboxPolicy) -> proto::SandboxPolicy {
    let kind = match payload {
        SandboxPolicy::DangerFullAccess => {
            proto::sandbox_policy::Kind::DangerFullAccess(proto::Empty {})
        }
        SandboxPolicy::ReadOnly { network_access } => {
            proto::sandbox_policy::Kind::ReadOnly(proto::SandboxReadOnly { network_access })
        }
        SandboxPolicy::ExternalSandbox { network_access } => {
            let network_access = match network_access {
                NetworkAccess::Restricted => proto::NetworkAccess::Restricted,
                NetworkAccess::Enabled => proto::NetworkAccess::Enabled,
            };
            proto::sandbox_policy::Kind::ExternalSandbox(proto::SandboxExternal {
                network_access: network_access as i32,
            })
        }
        SandboxPolicy::WorkspaceWrite {
            writable_roots,
            network_access,
            exclude_tmpdir_env_var,
            exclude_slash_tmp,
        } => proto::sandbox_policy::Kind::WorkspaceWrite(proto::SandboxWorkspaceWrite {
            writable_roots: writable_roots
                .into_iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
            network_access,
            exclude_tmpdir_env_var,
            exclude_slash_tmp,
        }),
    };
    proto::SandboxPolicy { kind: Some(kind) }
}

impl NativeProto for CommandExecParams {
    type Proto = proto::CommandExecParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        let size = payload
            .size
            .map(decode_terminal_size)
            .transpose()?
            .map(|(rows, cols)| CommandExecTerminalSize { rows, cols });
        Ok(Self {
            command: payload.command,
            process_id: payload.process_id,
            tty: payload.tty,
            stream_stdin: payload.stream_stdin,
            stream_stdout_stderr: payload.stream_stdout_stderr,
            output_bytes_cap: payload
                .output_bytes_cap
                .map(|value| {
                    value
                        .try_into()
                        .map_err(|_| Status::invalid_argument("output byte cap exceeds usize"))
                })
                .transpose()?,
            disable_output_cap: payload.disable_output_cap,
            disable_timeout: payload.disable_timeout,
            timeout_ms: payload.timeout_ms,
            cwd: payload.cwd.map(PathBuf::from),
            env: payload.env.map(|env| decode_env(env.values)),
            size,
            sandbox_policy: payload
                .sandbox_policy
                .map(decode_sandbox_policy)
                .transpose()?,
            permission_profile: payload.permission_profile,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::CommandExecParams {
            command: self.command,
            process_id: self.process_id,
            tty: self.tty,
            stream_stdin: self.stream_stdin,
            stream_stdout_stderr: self.stream_stdout_stderr,
            output_bytes_cap: self
                .output_bytes_cap
                .map(u64::try_from)
                .transpose()
                .map_err(|_| Status::internal("output byte cap exceeds u64"))?,
            disable_output_cap: self.disable_output_cap,
            disable_timeout: self.disable_timeout,
            timeout_ms: self.timeout_ms,
            cwd: self.cwd.map(|path| path.to_string_lossy().into_owned()),
            env: self.env.map(|env| proto::NullableStringMap {
                values: encode_env(env),
            }),
            size: self
                .size
                .map(|size| encode_terminal_size(size.rows, size.cols)),
            sandbox_policy: self.sandbox_policy.map(encode_sandbox_policy),
            permission_profile: self.permission_profile,
        })
    }
}

impl NativeProto for CommandExecResponse {
    type Proto = proto::CommandExecResponse;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            exit_code: payload.exit_code,
            stdout: payload.stdout,
            stderr: payload.stderr,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::CommandExecResponse {
            exit_code: self.exit_code,
            stdout: self.stdout,
            stderr: self.stderr,
        })
    }
}

impl NativeProto for CommandExecWriteParams {
    type Proto = proto::CommandExecWriteParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            process_id: payload.process_id,
            delta_base64: payload.delta_base64,
            close_stdin: payload.close_stdin,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::CommandExecWriteParams {
            process_id: self.process_id,
            delta_base64: self.delta_base64,
            close_stdin: self.close_stdin,
        })
    }
}

impl NativeProto for CommandExecTerminateParams {
    type Proto = proto::CommandExecTerminateParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            process_id: payload.process_id,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::CommandExecTerminateParams {
            process_id: self.process_id,
        })
    }
}

impl NativeProto for CommandExecResizeParams {
    type Proto = proto::CommandExecResizeParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        let (rows, cols) = decode_terminal_size(required(payload.size, "terminal size")?)?;
        Ok(Self {
            process_id: payload.process_id,
            size: CommandExecTerminalSize { rows, cols },
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::CommandExecResizeParams {
            process_id: self.process_id,
            size: Some(encode_terminal_size(self.size.rows, self.size.cols)),
        })
    }
}

fn decode_output_stream(value: i32) -> Result<proto::OutputStream, Status> {
    match proto::OutputStream::try_from(value)
        .map_err(|_| Status::invalid_argument("invalid output stream"))?
    {
        proto::OutputStream::Unspecified => {
            Err(Status::invalid_argument("unspecified output stream"))
        }
        stream => Ok(stream),
    }
}

impl NativeProto for CommandExecOutputDeltaNotification {
    type Proto = proto::CommandExecOutputDeltaNotification;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        let stream = match decode_output_stream(payload.stream)? {
            proto::OutputStream::Stdout => CommandExecOutputStream::Stdout,
            proto::OutputStream::Stderr => CommandExecOutputStream::Stderr,
            proto::OutputStream::Unspecified => unreachable!(),
        };
        Ok(Self {
            process_id: payload.process_id,
            stream,
            delta_base64: payload.delta_base64,
            cap_reached: payload.cap_reached,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        let stream = match self.stream {
            CommandExecOutputStream::Stdout => proto::OutputStream::Stdout,
            CommandExecOutputStream::Stderr => proto::OutputStream::Stderr,
        };
        Ok(proto::CommandExecOutputDeltaNotification {
            process_id: self.process_id,
            stream: stream as i32,
            delta_base64: self.delta_base64,
            cap_reached: self.cap_reached,
        })
    }
}

impl NativeProto for ProcessSpawnParams {
    type Proto = proto::ProcessSpawnParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        let size = payload
            .size
            .map(decode_terminal_size)
            .transpose()?
            .map(|(rows, cols)| ProcessTerminalSize { rows, cols });
        Ok(Self {
            command: payload.command,
            process_handle: payload.process_handle,
            cwd: payload.cwd.try_into().map_err(invalid_path)?,
            tty: payload.tty,
            stream_stdin: payload.stream_stdin,
            stream_stdout_stderr: payload.stream_stdout_stderr,
            output_bytes_cap: payload
                .output_bytes_cap
                .map(decode_optional_u64)
                .transpose()?
                .map(|value| {
                    value
                        .map(usize::try_from)
                        .transpose()
                        .map_err(|_| Status::invalid_argument("output byte cap exceeds usize"))
                })
                .transpose()?,
            timeout_ms: payload.timeout_ms.map(decode_optional_i64).transpose()?,
            env: payload.env.map(|env| decode_env(env.values)),
            size,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        let output_bytes_cap = self
            .output_bytes_cap
            .map(|value| {
                value
                    .map(u64::try_from)
                    .transpose()
                    .map(encode_optional_u64)
                    .map_err(|_| Status::internal("output byte cap exceeds u64"))
            })
            .transpose()?;
        Ok(proto::ProcessSpawnParams {
            command: self.command,
            process_handle: self.process_handle,
            cwd: self.cwd.to_string_lossy().into_owned(),
            tty: self.tty,
            stream_stdin: self.stream_stdin,
            stream_stdout_stderr: self.stream_stdout_stderr,
            output_bytes_cap,
            timeout_ms: self.timeout_ms.map(encode_optional_i64),
            env: self.env.map(|env| proto::NullableStringMap {
                values: encode_env(env),
            }),
            size: self
                .size
                .map(|size| encode_terminal_size(size.rows, size.cols)),
        })
    }
}

impl NativeProto for ProcessWriteStdinParams {
    type Proto = proto::ProcessWriteStdinParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            process_handle: payload.process_handle,
            delta_base64: payload.delta_base64,
            close_stdin: payload.close_stdin,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::ProcessWriteStdinParams {
            process_handle: self.process_handle,
            delta_base64: self.delta_base64,
            close_stdin: self.close_stdin,
        })
    }
}

impl NativeProto for ProcessKillParams {
    type Proto = proto::ProcessKillParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            process_handle: payload.process_handle,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::ProcessKillParams {
            process_handle: self.process_handle,
        })
    }
}

impl NativeProto for ProcessResizePtyParams {
    type Proto = proto::ProcessResizePtyParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        let (rows, cols) = decode_terminal_size(required(payload.size, "terminal size")?)?;
        Ok(Self {
            process_handle: payload.process_handle,
            size: ProcessTerminalSize { rows, cols },
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::ProcessResizePtyParams {
            process_handle: self.process_handle,
            size: Some(encode_terminal_size(self.size.rows, self.size.cols)),
        })
    }
}

impl NativeProto for ProcessOutputDeltaNotification {
    type Proto = proto::ProcessOutputDeltaNotification;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        let stream = match decode_output_stream(payload.stream)? {
            proto::OutputStream::Stdout => ProcessOutputStream::Stdout,
            proto::OutputStream::Stderr => ProcessOutputStream::Stderr,
            proto::OutputStream::Unspecified => unreachable!(),
        };
        Ok(Self {
            process_handle: payload.process_handle,
            stream,
            delta_base64: payload.delta_base64,
            cap_reached: payload.cap_reached,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        let stream = match self.stream {
            ProcessOutputStream::Stdout => proto::OutputStream::Stdout,
            ProcessOutputStream::Stderr => proto::OutputStream::Stderr,
        };
        Ok(proto::ProcessOutputDeltaNotification {
            process_handle: self.process_handle,
            stream: stream as i32,
            delta_base64: self.delta_base64,
            cap_reached: self.cap_reached,
        })
    }
}

impl NativeProto for ProcessExitedNotification {
    type Proto = proto::ProcessExitedNotification;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            process_handle: payload.process_handle,
            exit_code: payload.exit_code,
            stdout: payload.stdout,
            stdout_cap_reached: payload.stdout_cap_reached,
            stderr: payload.stderr,
            stderr_cap_reached: payload.stderr_cap_reached,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::ProcessExitedNotification {
            process_handle: self.process_handle,
            exit_code: self.exit_code,
            stdout: self.stdout,
            stdout_cap_reached: self.stdout_cap_reached,
            stderr: self.stderr,
            stderr_cap_reached: self.stderr_cap_reached,
        })
    }
}

impl NativeProto for LoginAccountParams {
    type Proto = proto::LoginAccountParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        match required(payload.account, "login account type")? {
            proto::login_account_params::Account::ApiKey(payload) => Ok(Self::ApiKey {
                api_key: payload.api_key,
            }),
            proto::login_account_params::Account::Chatgpt(payload) => Ok(Self::Chatgpt {
                codex_streamlined_login: payload.codex_streamlined_login,
            }),
            proto::login_account_params::Account::ChatgptDeviceCode(_) => {
                Ok(Self::ChatgptDeviceCode)
            }
            proto::login_account_params::Account::ChatgptAuthTokens(payload) => {
                Ok(Self::ChatgptAuthTokens {
                    access_token: payload.access_token,
                    chatgpt_account_id: payload.chatgpt_account_id,
                    chatgpt_plan_type: payload.chatgpt_plan_type,
                })
            }
        }
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        let account = match self {
            Self::ApiKey { api_key } => {
                proto::login_account_params::Account::ApiKey(proto::LoginApiKey { api_key })
            }
            Self::Chatgpt {
                codex_streamlined_login,
            } => proto::login_account_params::Account::Chatgpt(proto::LoginChatgpt {
                codex_streamlined_login,
            }),
            Self::ChatgptDeviceCode => {
                proto::login_account_params::Account::ChatgptDeviceCode(proto::Empty {})
            }
            Self::ChatgptAuthTokens {
                access_token,
                chatgpt_account_id,
                chatgpt_plan_type,
            } => proto::login_account_params::Account::ChatgptAuthTokens(
                proto::LoginChatgptAuthTokens {
                    access_token,
                    chatgpt_account_id,
                    chatgpt_plan_type,
                },
            ),
        };
        Ok(proto::LoginAccountParams {
            account: Some(account),
        })
    }
}

impl NativeProto for LoginAccountResponse {
    type Proto = proto::LoginAccountResponse;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        match required(payload.account, "login account response type")? {
            proto::login_account_response::Account::ApiKey(_) => Ok(Self::ApiKey {}),
            proto::login_account_response::Account::Chatgpt(payload) => Ok(Self::Chatgpt {
                login_id: payload.login_id,
                auth_url: payload.auth_url,
            }),
            proto::login_account_response::Account::ChatgptDeviceCode(payload) => {
                Ok(Self::ChatgptDeviceCode {
                    login_id: payload.login_id,
                    verification_url: payload.verification_url,
                    user_code: payload.user_code,
                })
            }
            proto::login_account_response::Account::ChatgptAuthTokens(_) => {
                Ok(Self::ChatgptAuthTokens {})
            }
        }
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        let account = match self {
            Self::ApiKey {} => proto::login_account_response::Account::ApiKey(proto::Empty {}),
            Self::Chatgpt { login_id, auth_url } => {
                proto::login_account_response::Account::Chatgpt(proto::LoginChatgptResponse {
                    login_id,
                    auth_url,
                })
            }
            Self::ChatgptDeviceCode {
                login_id,
                verification_url,
                user_code,
            } => proto::login_account_response::Account::ChatgptDeviceCode(
                proto::LoginChatgptDeviceCodeResponse {
                    login_id,
                    verification_url,
                    user_code,
                },
            ),
            Self::ChatgptAuthTokens {} => {
                proto::login_account_response::Account::ChatgptAuthTokens(proto::Empty {})
            }
        };
        Ok(proto::LoginAccountResponse {
            account: Some(account),
        })
    }
}

impl NativeProto for CancelLoginAccountParams {
    type Proto = proto::CancelLoginAccountParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            login_id: payload.login_id,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::CancelLoginAccountParams {
            login_id: self.login_id,
        })
    }
}

impl NativeProto for CancelLoginAccountResponse {
    type Proto = proto::CancelLoginAccountResponse;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        let status = match proto::CancelLoginAccountStatus::try_from(payload.status)
            .map_err(|_| Status::invalid_argument("invalid cancel login status"))?
        {
            proto::CancelLoginAccountStatus::Unspecified => {
                return Err(Status::invalid_argument("unspecified cancel login status"));
            }
            proto::CancelLoginAccountStatus::Canceled => CancelLoginAccountStatus::Canceled,
            proto::CancelLoginAccountStatus::NotFound => CancelLoginAccountStatus::NotFound,
        };
        Ok(Self { status })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        let status = match self.status {
            CancelLoginAccountStatus::Canceled => proto::CancelLoginAccountStatus::Canceled,
            CancelLoginAccountStatus::NotFound => proto::CancelLoginAccountStatus::NotFound,
        };
        Ok(proto::CancelLoginAccountResponse {
            status: status as i32,
        })
    }
}

impl NativeProto for SendAddCreditsNudgeEmailParams {
    type Proto = proto::SendAddCreditsNudgeEmailParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        let credit_type = match proto::AddCreditsNudgeCreditType::try_from(payload.credit_type)
            .map_err(|_| Status::invalid_argument("invalid credit type"))?
        {
            proto::AddCreditsNudgeCreditType::Unspecified => {
                return Err(Status::invalid_argument("unspecified credit type"));
            }
            proto::AddCreditsNudgeCreditType::Credits => AddCreditsNudgeCreditType::Credits,
            proto::AddCreditsNudgeCreditType::UsageLimit => AddCreditsNudgeCreditType::UsageLimit,
        };
        Ok(Self { credit_type })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        let credit_type = match self.credit_type {
            AddCreditsNudgeCreditType::Credits => proto::AddCreditsNudgeCreditType::Credits,
            AddCreditsNudgeCreditType::UsageLimit => proto::AddCreditsNudgeCreditType::UsageLimit,
        };
        Ok(proto::SendAddCreditsNudgeEmailParams {
            credit_type: credit_type as i32,
        })
    }
}

impl NativeProto for SendAddCreditsNudgeEmailResponse {
    type Proto = proto::SendAddCreditsNudgeEmailResponse;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        let status = match proto::AddCreditsNudgeEmailStatus::try_from(payload.status)
            .map_err(|_| Status::invalid_argument("invalid nudge email status"))?
        {
            proto::AddCreditsNudgeEmailStatus::Unspecified => {
                return Err(Status::invalid_argument("unspecified nudge email status"));
            }
            proto::AddCreditsNudgeEmailStatus::Sent => AddCreditsNudgeEmailStatus::Sent,
            proto::AddCreditsNudgeEmailStatus::CooldownActive => {
                AddCreditsNudgeEmailStatus::CooldownActive
            }
        };
        Ok(Self { status })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        let status = match self.status {
            AddCreditsNudgeEmailStatus::Sent => proto::AddCreditsNudgeEmailStatus::Sent,
            AddCreditsNudgeEmailStatus::CooldownActive => {
                proto::AddCreditsNudgeEmailStatus::CooldownActive
            }
        };
        Ok(proto::SendAddCreditsNudgeEmailResponse {
            status: status as i32,
        })
    }
}

impl NativeProto for GetAccountParams {
    type Proto = proto::GetAccountParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            refresh_token: payload.refresh_token,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::GetAccountParams {
            refresh_token: self.refresh_token,
        })
    }
}

impl NativeProto for ChatgptAuthTokensRefreshParams {
    type Proto = proto::ChatgptAuthTokensRefreshParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            reason: ChatgptAuthTokensRefreshReason::Unauthorized,
            previous_account_id: payload.previous_account_id,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::ChatgptAuthTokensRefreshParams {
            previous_account_id: self.previous_account_id,
        })
    }
}

impl NativeProto for ChatgptAuthTokensRefreshResponse {
    type Proto = proto::ChatgptAuthTokensRefreshResponse;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            access_token: payload.access_token,
            chatgpt_account_id: payload.chatgpt_account_id,
            chatgpt_plan_type: payload.chatgpt_plan_type,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::ChatgptAuthTokensRefreshResponse {
            access_token: self.access_token,
            chatgpt_account_id: self.chatgpt_account_id,
            chatgpt_plan_type: self.chatgpt_plan_type,
        })
    }
}

impl NativeProto for AttestationGenerateResponse {
    type Proto = proto::AttestationGenerateResponse;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            token: payload.token,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::AttestationGenerateResponse { token: self.token })
    }
}

impl NativeProto for AccountLoginCompletedNotification {
    type Proto = proto::AccountLoginCompletedNotification;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            login_id: payload.login_id,
            success: payload.success,
            error: payload.error,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::AccountLoginCompletedNotification {
            login_id: self.login_id,
            success: self.success,
            error: self.error,
        })
    }
}

impl NativeProto for RemoteControlPairingStartParams {
    type Proto = proto::RemoteControlPairingStartParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            manual_code: payload.manual_code,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::RemoteControlPairingStartParams {
            manual_code: self.manual_code,
        })
    }
}

impl NativeProto for RemoteControlPairingStartResponse {
    type Proto = proto::RemoteControlPairingStartResponse;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            pairing_code: payload.pairing_code,
            manual_pairing_code: payload.manual_pairing_code,
            environment_id: payload.environment_id,
            expires_at: payload.expires_at,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::RemoteControlPairingStartResponse {
            pairing_code: self.pairing_code,
            manual_pairing_code: self.manual_pairing_code,
            environment_id: self.environment_id,
            expires_at: self.expires_at,
        })
    }
}

impl NativeProto for RemoteControlClientsRevokeParams {
    type Proto = proto::RemoteControlClientsRevokeParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            environment_id: payload.environment_id,
            client_id: payload.client_id,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::RemoteControlClientsRevokeParams {
            environment_id: self.environment_id,
            client_id: self.client_id,
        })
    }
}

impl NativeProto for FuzzyFileSearchSessionStartParams {
    type Proto = proto::FuzzyFileSearchSessionStartParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            session_id: payload.session_id,
            roots: payload.roots,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::FuzzyFileSearchSessionStartParams {
            session_id: self.session_id,
            roots: self.roots,
        })
    }
}

impl NativeProto for FuzzyFileSearchSessionUpdateParams {
    type Proto = proto::FuzzyFileSearchSessionUpdateParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            session_id: payload.session_id,
            query: payload.query,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::FuzzyFileSearchSessionUpdateParams {
            session_id: self.session_id,
            query: self.query,
        })
    }
}

impl NativeProto for FuzzyFileSearchSessionStopParams {
    type Proto = proto::FuzzyFileSearchSessionStopParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            session_id: payload.session_id,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::FuzzyFileSearchSessionStopParams {
            session_id: self.session_id,
        })
    }
}

impl NativeProto for FuzzyFileSearchSessionCompletedNotification {
    type Proto = proto::FuzzyFileSearchSessionCompletedNotification;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            session_id: payload.session_id,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::FuzzyFileSearchSessionCompletedNotification {
            session_id: self.session_id,
        })
    }
}

impl NativeProto for ThreadRealtimeAppendTextParams {
    type Proto = proto::ThreadRealtimeAppendTextParams;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            thread_id: payload.thread_id,
            text: payload.text,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::ThreadRealtimeAppendTextParams {
            thread_id: self.thread_id,
            text: self.text,
        })
    }
}

macro_rules! realtime_text_notification_native {
    ($type:ty, $proto:path, $field:ident) => {
        impl NativeProto for $type {
            type Proto = $proto;

            fn decode(payload: Self::Proto) -> Result<Self, Status> {
                Ok(Self {
                    thread_id: payload.thread_id,
                    role: payload.role,
                    $field: payload.$field,
                })
            }

            fn encode(self) -> Result<Self::Proto, Status> {
                let mut payload = <$proto>::default();
                payload.thread_id = self.thread_id;
                payload.role = self.role;
                payload.$field = self.$field;
                Ok(payload)
            }
        }
    };
}

realtime_text_notification_native!(
    ThreadRealtimeTranscriptDeltaNotification,
    proto::ThreadRealtimeTranscriptDeltaNotification,
    delta
);
realtime_text_notification_native!(
    ThreadRealtimeTranscriptDoneNotification,
    proto::ThreadRealtimeTranscriptDoneNotification,
    text
);

macro_rules! thread_string_notification_native {
    ($type:ty, $proto:path, $field:ident) => {
        impl NativeProto for $type {
            type Proto = $proto;

            fn decode(payload: Self::Proto) -> Result<Self, Status> {
                Ok(Self {
                    thread_id: payload.thread_id,
                    $field: payload.$field,
                })
            }

            fn encode(self) -> Result<Self::Proto, Status> {
                let mut payload = <$proto>::default();
                payload.thread_id = self.thread_id;
                payload.$field = self.$field;
                Ok(payload)
            }
        }
    };
}

thread_string_notification_native!(
    ThreadRealtimeSdpNotification,
    proto::ThreadRealtimeSdpNotification,
    sdp
);
thread_string_notification_native!(
    ThreadRealtimeErrorNotification,
    proto::ThreadRealtimeErrorNotification,
    message
);
thread_string_notification_native!(
    GuardianWarningNotification,
    proto::GuardianWarningNotification,
    message
);

impl NativeProto for ThreadRealtimeClosedNotification {
    type Proto = proto::ThreadRealtimeClosedNotification;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            thread_id: payload.thread_id,
            reason: payload.reason,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::ThreadRealtimeClosedNotification {
            thread_id: self.thread_id,
            reason: self.reason,
        })
    }
}

impl NativeProto for WarningNotification {
    type Proto = proto::WarningNotification;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            thread_id: payload.thread_id,
            message: payload.message,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::WarningNotification {
            thread_id: self.thread_id,
            message: self.message,
        })
    }
}

impl NativeProto for DeprecationNoticeNotification {
    type Proto = proto::DeprecationNoticeNotification;

    fn decode(payload: Self::Proto) -> Result<Self, Status> {
        Ok(Self {
            summary: payload.summary,
            details: payload.details,
        })
    }

    fn encode(self) -> Result<Self::Proto, Status> {
        Ok(proto::DeprecationNoticeNotification {
            summary: self.summary,
            details: self.details,
        })
    }
}

fn required<T>(value: Option<T>, name: &'static str) -> Result<T, Status> {
    value.ok_or_else(|| Status::invalid_argument(format!("missing {name}")))
}

#[path = "grpc_schema_native_types.rs"]
#[allow(unreachable_patterns)]
pub(crate) mod schema_types;

#[cfg(test)]
#[path = "grpc_native_types_tests.rs"]
mod tests;
