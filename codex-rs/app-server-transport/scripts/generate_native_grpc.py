#!/usr/bin/env python3

from pathlib import Path
import os
import re
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from schema_proto import NotificationEntry
from schema_proto import RpcEntry
from schema_proto import SchemaProto
from schema_proto import parse_notification_entries
from schema_proto import parse_rpc_entries
from schema_proto import render_native_impls


CODEX_RS = Path(__file__).resolve().parents[2]
COMMON_RS = CODEX_RS / "app-server-protocol/src/protocol/common.rs"
PROTO = CODEX_RS / "app-server-transport/src/transport/proto/codex.app_server.v2.proto"
CONVERSIONS = CODEX_RS / "app-server-transport/src/transport/grpc_api_conversions.rs"
SCHEMA_NATIVE_TYPES = (
    CODEX_RS / "app-server-transport/src/transport/grpc_schema_native_types.rs"
)
EXPERIMENTAL_SCHEMA_ROOT = Path(
    os.environ.get(
        "CODEX_APP_SERVER_GRPC_SCHEMA_ROOT",
        "/tmp/codex-app-server-grpc-schema",
    )
)
V2_SCHEMA = EXPERIMENTAL_SCHEMA_ROOT / "json/codex_app_server_protocol.v2.schemas.json"
LEGACY_SCHEMA = EXPERIMENTAL_SCHEMA_ROOT / "json/codex_app_server_protocol.schemas.json"

CLIENT_REQUEST_NATIVE = {
    "ThreadArchive": "ThreadIdParams",
    "ThreadIncrementElicitation": "ThreadIdParams",
    "ThreadDecrementElicitation": "ThreadIdParams",
    "ThreadSetName": "ThreadSetNameParams",
    "ThreadGoalSet": "ThreadGoalSetParams",
    "ThreadGoalGet": "ThreadIdParams",
    "ThreadGoalClear": "ThreadIdParams",
    "ThreadMemoryModeSet": "ThreadMemoryModeSetParams",
    "MemoryReset": "Empty",
    "ThreadCompactStart": "ThreadIdParams",
    "ThreadShellCommand": "ThreadShellCommandParams",
    "ThreadBackgroundTerminalsClean": "ThreadIdParams",
    "PluginShareList": "Empty",
    "FsReadFile": "FsPathParams",
    "FsWriteFile": "FsWriteFileParams",
    "FsCreateDirectory": "FsCreateDirectoryParams",
    "FsGetMetadata": "FsPathParams",
    "FsReadDirectory": "FsPathParams",
    "FsRemove": "FsRemoveParams",
    "FsCopy": "FsCopyParams",
    "FsWatch": "FsWatchParams",
    "FsUnwatch": "FsUnwatchParams",
    "TurnInterrupt": "TurnInterruptParams",
    "ThreadRealtimeAppendText": "ThreadRealtimeAppendTextParams",
    "ThreadRealtimeStop": "ThreadIdParams",
    "ThreadRealtimeListVoices": "Empty",
    "ModelProviderCapabilitiesRead": "Empty",
    "RemoteControlEnable": "Empty",
    "RemoteControlDisable": "Empty",
    "RemoteControlStatusRead": "Empty",
    "RemoteControlPairingStart": "RemoteControlPairingStartParams",
    "RemoteControlClientsRevoke": "RemoteControlClientsRevokeParams",
    "CollaborationModeList": "Empty",
    "EnvironmentAdd": "EnvironmentAddParams",
    "McpServerRefresh": "Empty",
    "LoginAccount": "LoginAccountParams",
    "CancelLoginAccount": "CancelLoginAccountParams",
    "LogoutAccount": "Empty",
    "GetAccountRateLimits": "Empty",
    "SendAddCreditsNudgeEmail": "SendAddCreditsNudgeEmailParams",
    "OneOffCommandExec": "CommandExecParams",
    "CommandExecWrite": "CommandExecWriteParams",
    "CommandExecTerminate": "CommandExecTerminateParams",
    "CommandExecResize": "CommandExecResizeParams",
    "ProcessSpawn": "ProcessSpawnParams",
    "ProcessWriteStdin": "ProcessWriteStdinParams",
    "ProcessKill": "ProcessKillParams",
    "ProcessResizePty": "ProcessResizePtyParams",
    "ConfigRequirementsRead": "Empty",
    "GetAccount": "GetAccountParams",
    "FuzzyFileSearchSessionStart": "FuzzyFileSearchSessionStartParams",
    "FuzzyFileSearchSessionUpdate": "FuzzyFileSearchSessionUpdateParams",
    "FuzzyFileSearchSessionStop": "FuzzyFileSearchSessionStopParams",
}

CLIENT_RESPONSE_NATIVE = {
    "ThreadArchive": "Empty",
    "ThreadIncrementElicitation": "ElicitationCounterResponse",
    "ThreadDecrementElicitation": "ElicitationCounterResponse",
    "ThreadSetName": "Empty",
    "ThreadGoalSet": "ThreadGoalResponse",
    "ThreadGoalGet": "ThreadGoalResponse",
    "ThreadGoalClear": "ThreadGoalClearResponse",
    "ThreadSettingsUpdate": "Empty",
    "ThreadMemoryModeSet": "Empty",
    "MemoryReset": "Empty",
    "ThreadCompactStart": "Empty",
    "ThreadShellCommand": "Empty",
    "ThreadApproveGuardianDeniedAction": "Empty",
    "ThreadBackgroundTerminalsClean": "Empty",
    "ThreadInjectItems": "Empty",
    "SkillsExtraRootsSet": "Empty",
    "PluginShareDelete": "Empty",
    "PluginUninstall": "Empty",
    "FsReadFile": "FsReadFileResponse",
    "FsWriteFile": "Empty",
    "FsCreateDirectory": "Empty",
    "FsGetMetadata": "FsGetMetadataResponse",
    "FsReadDirectory": "FsReadDirectoryResponse",
    "FsRemove": "Empty",
    "FsCopy": "Empty",
    "FsWatch": "FsWatchResponse",
    "FsUnwatch": "Empty",
    "TurnInterrupt": "Empty",
    "ThreadRealtimeStart": "Empty",
    "ThreadRealtimeAppendAudio": "Empty",
    "ThreadRealtimeAppendText": "Empty",
    "ThreadRealtimeStop": "Empty",
    "RemoteControlPairingStart": "RemoteControlPairingStartResponse",
    "RemoteControlClientsRevoke": "Empty",
    "EnvironmentAdd": "Empty",
    "McpServerRefresh": "Empty",
    "LoginAccount": "LoginAccountResponse",
    "CancelLoginAccount": "CancelLoginAccountResponse",
    "LogoutAccount": "Empty",
    "SendAddCreditsNudgeEmail": "SendAddCreditsNudgeEmailResponse",
    "OneOffCommandExec": "CommandExecResponse",
    "CommandExecWrite": "Empty",
    "CommandExecTerminate": "Empty",
    "CommandExecResize": "Empty",
    "ProcessSpawn": "Empty",
    "ProcessWriteStdin": "Empty",
    "ProcessKill": "Empty",
    "ProcessResizePty": "Empty",
    "ExternalAgentConfigImport": "Empty",
    "FuzzyFileSearchSessionStart": "Empty",
    "FuzzyFileSearchSessionUpdate": "Empty",
    "FuzzyFileSearchSessionStop": "Empty",
}

SERVER_REQUEST_NATIVE = {
    "ChatgptAuthTokensRefresh": "ChatgptAuthTokensRefreshParams",
    "AttestationGenerate": "Empty",
}

SERVER_RESPONSE_NATIVE = {
    "ChatgptAuthTokensRefresh": "ChatgptAuthTokensRefreshResponse",
    "AttestationGenerate": "AttestationGenerateResponse",
}

NOTIFICATION_NATIVE = {
    "ThreadArchived": "ThreadIdParams",
    "ThreadUnarchived": "ThreadIdParams",
    "ThreadClosed": "ThreadIdParams",
    "SkillsChanged": "Empty",
    "ThreadNameUpdated": "ThreadNameUpdatedNotification",
    "ThreadGoalUpdated": "ThreadGoalUpdatedNotification",
    "ThreadGoalCleared": "ThreadIdParams",
    "CommandExecOutputDelta": "CommandExecOutputDeltaNotification",
    "ProcessOutputDelta": "ProcessOutputDeltaNotification",
    "ProcessExited": "ProcessExitedNotification",
    "FsChanged": "FsChangedNotification",
    "Warning": "WarningNotification",
    "GuardianWarning": "GuardianWarningNotification",
    "DeprecationNotice": "DeprecationNoticeNotification",
    "FuzzyFileSearchSessionCompleted": "FuzzyFileSearchSessionCompletedNotification",
    "ThreadRealtimeTranscriptDelta": "ThreadRealtimeTranscriptDeltaNotification",
    "ThreadRealtimeTranscriptDone": "ThreadRealtimeTranscriptDoneNotification",
    "ThreadRealtimeSdp": "ThreadRealtimeSdpNotification",
    "ThreadRealtimeError": "ThreadRealtimeErrorNotification",
    "ThreadRealtimeClosed": "ThreadRealtimeClosedNotification",
    "ExternalAgentConfigImportCompleted": "Empty",
    "AccountLoginCompleted": "AccountLoginCompletedNotification",
}

NATIVE_PROTO_MESSAGES = r"""
message OptionalInt64 {
  oneof value {
    int64 some = 1;
    Empty null = 2;
  }
}

message OptionalUint64 {
  oneof value {
    uint64 some = 1;
    Empty null = 2;
  }
}

message NullableString {
  optional string value = 1;
}

message NullableStringMap {
  map<string, NullableString> values = 1;
}

message ThreadIdParams {
  string thread_id = 1;
}

message ThreadSetNameParams {
  string thread_id = 1;
  string name = 2;
}

message ElicitationCounterResponse {
  uint64 count = 1;
  bool paused = 2;
}

enum ThreadGoalStatus {
  THREAD_GOAL_STATUS_UNSPECIFIED = 0;
  THREAD_GOAL_STATUS_ACTIVE = 1;
  THREAD_GOAL_STATUS_PAUSED = 2;
  THREAD_GOAL_STATUS_BLOCKED = 3;
  THREAD_GOAL_STATUS_USAGE_LIMITED = 4;
  THREAD_GOAL_STATUS_BUDGET_LIMITED = 5;
  THREAD_GOAL_STATUS_COMPLETE = 6;
}

message ThreadGoal {
  string thread_id = 1;
  string objective = 2;
  ThreadGoalStatus status = 3;
  optional int64 token_budget = 4;
  int64 tokens_used = 5;
  int64 time_used_seconds = 6;
  int64 created_at = 7;
  int64 updated_at = 8;
}

message ThreadGoalSetParams {
  string thread_id = 1;
  optional string objective = 2;
  optional ThreadGoalStatus status = 3;
  OptionalInt64 token_budget = 4;
}

message ThreadGoalResponse {
  optional ThreadGoal goal = 1;
}

message ThreadGoalClearResponse {
  bool cleared = 1;
}

enum ThreadMemoryMode {
  THREAD_MEMORY_MODE_UNSPECIFIED = 0;
  THREAD_MEMORY_MODE_ENABLED = 1;
  THREAD_MEMORY_MODE_DISABLED = 2;
}

message ThreadMemoryModeSetParams {
  string thread_id = 1;
  ThreadMemoryMode mode = 2;
}

message ThreadShellCommandParams {
  string thread_id = 1;
  string command = 2;
}

message TurnInterruptParams {
  string thread_id = 1;
  string turn_id = 2;
}

message ThreadNameUpdatedNotification {
  string thread_id = 1;
  optional string thread_name = 2;
}

message ThreadGoalUpdatedNotification {
  string thread_id = 1;
  optional string turn_id = 2;
  ThreadGoal goal = 3;
}

message FsPathParams {
  string path = 1;
}

message FsWriteFileParams {
  string path = 1;
  string data_base64 = 2;
}

message FsCreateDirectoryParams {
  string path = 1;
  optional bool recursive = 2;
}

message FsRemoveParams {
  string path = 1;
  optional bool recursive = 2;
  optional bool force = 3;
}

message FsCopyParams {
  string source_path = 1;
  string destination_path = 2;
  bool recursive = 3;
}

message FsWatchParams {
  string watch_id = 1;
  string path = 2;
}

message FsUnwatchParams {
  string watch_id = 1;
}

message FsReadFileResponse {
  string data_base64 = 1;
}

message FsGetMetadataResponse {
  bool is_directory = 1;
  bool is_file = 2;
  bool is_symlink = 3;
  int64 created_at_ms = 4;
  int64 modified_at_ms = 5;
}

message FsReadDirectoryEntry {
  string file_name = 1;
  bool is_directory = 2;
  bool is_file = 3;
}

message FsReadDirectoryResponse {
  repeated FsReadDirectoryEntry entries = 1;
}

message FsWatchResponse {
  string path = 1;
}

message FsChangedNotification {
  string watch_id = 1;
  repeated string changed_paths = 2;
}

message EnvironmentAddParams {
  string environment_id = 1;
  string exec_server_url = 2;
}

message TerminalSize {
  uint32 rows = 1;
  uint32 cols = 2;
}

message SandboxPolicy {
  oneof kind {
    Empty danger_full_access = 1;
    SandboxReadOnly read_only = 2;
    SandboxExternal external_sandbox = 3;
    SandboxWorkspaceWrite workspace_write = 4;
  }
}

message SandboxReadOnly {
  bool network_access = 1;
}

enum NetworkAccess {
  NETWORK_ACCESS_UNSPECIFIED = 0;
  NETWORK_ACCESS_RESTRICTED = 1;
  NETWORK_ACCESS_ENABLED = 2;
}

message SandboxExternal {
  NetworkAccess network_access = 1;
}

message SandboxWorkspaceWrite {
  repeated string writable_roots = 1;
  bool network_access = 2;
  bool exclude_tmpdir_env_var = 3;
  bool exclude_slash_tmp = 4;
}

message CommandExecParams {
  repeated string command = 1;
  optional string process_id = 2;
  bool tty = 3;
  bool stream_stdin = 4;
  bool stream_stdout_stderr = 5;
  optional uint64 output_bytes_cap = 6;
  bool disable_output_cap = 7;
  bool disable_timeout = 8;
  optional int64 timeout_ms = 9;
  optional string cwd = 10;
  NullableStringMap env = 11;
  TerminalSize size = 12;
  SandboxPolicy sandbox_policy = 13;
  optional string permission_profile = 14;
}

message CommandExecResponse {
  int32 exit_code = 1;
  string stdout = 2;
  string stderr = 3;
}

message CommandExecWriteParams {
  string process_id = 1;
  optional string delta_base64 = 2;
  bool close_stdin = 3;
}

message CommandExecTerminateParams {
  string process_id = 1;
}

message CommandExecResizeParams {
  string process_id = 1;
  TerminalSize size = 2;
}

enum OutputStream {
  OUTPUT_STREAM_UNSPECIFIED = 0;
  OUTPUT_STREAM_STDOUT = 1;
  OUTPUT_STREAM_STDERR = 2;
}

message CommandExecOutputDeltaNotification {
  string process_id = 1;
  OutputStream stream = 2;
  string delta_base64 = 3;
  bool cap_reached = 4;
}

message ProcessSpawnParams {
  repeated string command = 1;
  string process_handle = 2;
  string cwd = 3;
  bool tty = 4;
  bool stream_stdin = 5;
  bool stream_stdout_stderr = 6;
  OptionalUint64 output_bytes_cap = 7;
  OptionalInt64 timeout_ms = 8;
  NullableStringMap env = 9;
  TerminalSize size = 10;
}

message ProcessWriteStdinParams {
  string process_handle = 1;
  optional string delta_base64 = 2;
  bool close_stdin = 3;
}

message ProcessKillParams {
  string process_handle = 1;
}

message ProcessResizePtyParams {
  string process_handle = 1;
  TerminalSize size = 2;
}

message ProcessOutputDeltaNotification {
  string process_handle = 1;
  OutputStream stream = 2;
  string delta_base64 = 3;
  bool cap_reached = 4;
}

message ProcessExitedNotification {
  string process_handle = 1;
  int32 exit_code = 2;
  string stdout = 3;
  bool stdout_cap_reached = 4;
  string stderr = 5;
  bool stderr_cap_reached = 6;
}

message LoginAccountParams {
  oneof account {
    LoginApiKey api_key = 1;
    LoginChatgpt chatgpt = 2;
    Empty chatgpt_device_code = 3;
    LoginChatgptAuthTokens chatgpt_auth_tokens = 4;
  }
}

message LoginApiKey {
  string api_key = 1;
}

message LoginChatgpt {
  bool codex_streamlined_login = 1;
}

message LoginChatgptAuthTokens {
  string access_token = 1;
  string chatgpt_account_id = 2;
  optional string chatgpt_plan_type = 3;
}

message LoginAccountResponse {
  oneof account {
    Empty api_key = 1;
    LoginChatgptResponse chatgpt = 2;
    LoginChatgptDeviceCodeResponse chatgpt_device_code = 3;
    Empty chatgpt_auth_tokens = 4;
  }
}

message LoginChatgptResponse {
  string login_id = 1;
  string auth_url = 2;
}

message LoginChatgptDeviceCodeResponse {
  string login_id = 1;
  string verification_url = 2;
  string user_code = 3;
}

message CancelLoginAccountParams {
  string login_id = 1;
}

enum CancelLoginAccountStatus {
  CANCEL_LOGIN_ACCOUNT_STATUS_UNSPECIFIED = 0;
  CANCEL_LOGIN_ACCOUNT_STATUS_CANCELED = 1;
  CANCEL_LOGIN_ACCOUNT_STATUS_NOT_FOUND = 2;
}

message CancelLoginAccountResponse {
  CancelLoginAccountStatus status = 1;
}

enum AddCreditsNudgeCreditType {
  ADD_CREDITS_NUDGE_CREDIT_TYPE_UNSPECIFIED = 0;
  ADD_CREDITS_NUDGE_CREDIT_TYPE_CREDITS = 1;
  ADD_CREDITS_NUDGE_CREDIT_TYPE_USAGE_LIMIT = 2;
}

message SendAddCreditsNudgeEmailParams {
  AddCreditsNudgeCreditType credit_type = 1;
}

enum AddCreditsNudgeEmailStatus {
  ADD_CREDITS_NUDGE_EMAIL_STATUS_UNSPECIFIED = 0;
  ADD_CREDITS_NUDGE_EMAIL_STATUS_SENT = 1;
  ADD_CREDITS_NUDGE_EMAIL_STATUS_COOLDOWN_ACTIVE = 2;
}

message SendAddCreditsNudgeEmailResponse {
  AddCreditsNudgeEmailStatus status = 1;
}

message GetAccountParams {
  bool refresh_token = 1;
}

message ChatgptAuthTokensRefreshParams {
  optional string previous_account_id = 1;
}

message ChatgptAuthTokensRefreshResponse {
  string access_token = 1;
  string chatgpt_account_id = 2;
  optional string chatgpt_plan_type = 3;
}

message AttestationGenerateResponse {
  string token = 1;
}

message AccountLoginCompletedNotification {
  optional string login_id = 1;
  bool success = 2;
  optional string error = 3;
}

message RemoteControlPairingStartParams {
  bool manual_code = 1;
}

message RemoteControlPairingStartResponse {
  string pairing_code = 1;
  optional string manual_pairing_code = 2;
  string environment_id = 3;
  int64 expires_at = 4;
}

message RemoteControlClientsRevokeParams {
  string environment_id = 1;
  string client_id = 2;
}

message FuzzyFileSearchSessionStartParams {
  string session_id = 1;
  repeated string roots = 2;
}

message FuzzyFileSearchSessionUpdateParams {
  string session_id = 1;
  string query = 2;
}

message FuzzyFileSearchSessionStopParams {
  string session_id = 1;
}

message FuzzyFileSearchSessionCompletedNotification {
  string session_id = 1;
}

message ThreadRealtimeAppendTextParams {
  string thread_id = 1;
  string text = 2;
}

message ThreadRealtimeTranscriptDeltaNotification {
  string thread_id = 1;
  string role = 2;
  string delta = 3;
}

message ThreadRealtimeTranscriptDoneNotification {
  string thread_id = 1;
  string role = 2;
  string text = 3;
}

message ThreadRealtimeSdpNotification {
  string thread_id = 1;
  string sdp = 2;
}

message ThreadRealtimeErrorNotification {
  string thread_id = 1;
  string message = 2;
}

message ThreadRealtimeClosedNotification {
  string thread_id = 1;
  optional string reason = 2;
}

message WarningNotification {
  optional string thread_id = 1;
  string message = 2;
}

message GuardianWarningNotification {
  string thread_id = 1;
  string message = 2;
}

message DeprecationNoticeNotification {
  string summary = 1;
  optional string details = 2;
}
"""


def macro_body(source: str, name: str, end_marker: str) -> str:
    start_marker = f"{name}! {{"
    start = source.index(start_marker) + len(start_marker)
    end = source.index(end_marker, start)
    return source[start:end]


def block_variants(body: str) -> list[str]:
    return re.findall(
        r"(?m)^\s{4}([A-Z][A-Za-z0-9]+)(?:\s*=>\s*\"[^\"]+\")?\s*\{",
        body,
    )


def tuple_variants(body: str) -> list[str]:
    return re.findall(
        r"(?m)^\s{4}([A-Z][A-Za-z0-9]+)(?:\s*=>\s*\"[^\"]+\")?\s*\(",
        body,
    )


def exported_rust_type(rust_type: str) -> str:
    return rust_type.removeprefix("v1::").removeprefix("v2::")


def write_experimental_schemas() -> None:
    subprocess.run(
        [
            "cargo",
            "run",
            "-p",
            "codex-app-server-protocol",
            "--features",
            "serde-compat",
            "--bin",
            "write_schema_fixtures",
            "--",
            "--schema-root",
            str(EXPERIMENTAL_SCHEMA_ROOT),
            "--experimental",
        ],
        check=True,
        cwd=CODEX_RS,
    )


def add_schema_types(
    entries: list[RpcEntry],
    field: str,
    native_types: dict[str, str],
    schema: SchemaProto,
    generated_types: dict[str, str],
    manual_types: dict[str, str],
) -> None:
    for entry in entries:
        rust_type = getattr(entry, field)
        if entry.variant in native_types:
            manual_types.setdefault(
                exported_rust_type(rust_type), native_types[entry.variant]
            )
            continue
        exported = exported_rust_type(rust_type)
        proto_type = manual_types.get(exported)
        if proto_type is None:
            proto_type = schema.proto_for_rust_type(rust_type)
            generated_types.setdefault(exported, proto_type)
        native_types[entry.variant] = proto_type


def record_manual_rpc_types(
    entries: list[RpcEntry],
    field: str,
    native_types: dict[str, str],
    manual_types: dict[str, str],
) -> None:
    for entry in entries:
        if entry.variant in native_types:
            manual_types.setdefault(
                exported_rust_type(getattr(entry, field)),
                native_types[entry.variant],
            )


def record_manual_notification_types(
    entries: list[NotificationEntry],
    native_types: dict[str, str],
    manual_types: dict[str, str],
) -> None:
    for entry in entries:
        if entry.variant in native_types:
            manual_types.setdefault(
                exported_rust_type(entry.payload),
                native_types[entry.variant],
            )


def add_notification_schema_types(
    entries: list[NotificationEntry],
    native_types: dict[str, str],
    schema: SchemaProto,
    generated_types: dict[str, str],
    manual_types: dict[str, str],
) -> None:
    for entry in entries:
        if entry.variant in native_types:
            manual_types.setdefault(
                exported_rust_type(entry.payload), native_types[entry.variant]
            )
            continue
        exported = exported_rust_type(entry.payload)
        proto_type = manual_types.get(exported)
        if proto_type is None:
            proto_type = schema.proto_for_rust_type(entry.payload)
            generated_types.setdefault(exported, proto_type)
        native_types[entry.variant] = proto_type


def snake_case(name: str) -> str:
    first = re.sub(r"(.)([A-Z][a-z]+)", r"\1_\2", name)
    return re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", first).lower()


def oneof_fields(
    variants: list[str],
    native_types: dict[str, str],
    start: int = 10,
) -> str:
    return "\n".join(
        f"    {native_types[variant]} {snake_case(variant)} = {tag};"
        for tag, variant in enumerate(variants, start)
    )


def decode_match(
    proto_enum: str,
    rust_enum: str,
    variants: list[str],
    fields: str,
) -> str:
    arms = []
    for variant in variants:
        if fields == "request":
            expression = (
                f"{rust_enum}::{variant} {{ request_id, "
                "params: decode_native(payload)? }"
            )
        elif fields == "response":
            expression = (
                f"{rust_enum}::{variant} {{ request_id, "
                "response: decode_native(payload)? }"
            )
        else:
            raise ValueError(fields)
        arms.append(f"        {proto_enum}::{variant}(payload) => {expression},")
    return "\n".join(arms)


def encode_match(
    rust_enum: str,
    proto_enum: str,
    variants: list[str],
    fields: str,
) -> str:
    arms = []
    for variant in variants:
        value_field = "params" if fields == "request" else "response"
        arms.append(
            f"        {rust_enum}::{variant} {{ request_id, {value_field} }} => "
            f"(request_id, {proto_enum}::{variant}(encode_native({value_field})?)),"
        )
    return "\n".join(arms)


def encode_tuple_match(
    rust_enum: str,
    proto_enum: str,
    variants: list[str],
) -> str:
    arms = []
    for variant in variants:
        arms.append(
            f"        {rust_enum}::{variant}(payload) => "
            f"{proto_enum}::{variant}(encode_native(payload)?),"
        )
    return "\n".join(arms)


def decode_tuple_match(
    proto_enum: str,
    rust_enum: str,
    variants: list[str],
) -> str:
    arms = []
    for variant in variants:
        arms.append(
            f"        {proto_enum}::{variant}(payload) => "
            f"{rust_enum}::{variant}(decode_native(payload)?),"
        )
    return "\n".join(arms)


def main() -> None:
    write_experimental_schemas()
    source = COMMON_RS.read_text()
    client_request_body = macro_body(
        source,
        "client_request_definitions",
        "\n}\n\n/// Generates an `enum ServerRequest`",
    )
    server_request_body = macro_body(
        source,
        "server_request_definitions",
        "\n}\n\n#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]",
    )
    notification_body = macro_body(
        source,
        "server_notification_definitions",
        "\n}\n\nclient_notification_definitions!",
    )
    client_entries = parse_rpc_entries(client_request_body)
    server_entries = parse_rpc_entries(server_request_body)
    notification_entries = parse_notification_entries(notification_body)
    client_requests = [entry.variant for entry in client_entries]
    server_requests = [entry.variant for entry in server_entries]
    notifications = [entry.variant for entry in notification_entries]

    if len(client_requests) < 100:
        raise RuntimeError(
            f"expected at least 100 client requests, found {len(client_requests)}"
        )
    if len(server_requests) < 8:
        raise RuntimeError(
            f"expected at least 8 server requests, found {len(server_requests)}"
        )
    if len(notifications) < 50:
        raise RuntimeError(
            f"expected at least 50 notifications, found {len(notifications)}"
        )

    client_request_native = dict(CLIENT_REQUEST_NATIVE)
    client_response_native = dict(CLIENT_RESPONSE_NATIVE)
    server_request_native = dict(SERVER_REQUEST_NATIVE)
    server_response_native = dict(SERVER_RESPONSE_NATIVE)
    notification_native = dict(NOTIFICATION_NATIVE)
    schema = SchemaProto(V2_SCHEMA, LEGACY_SCHEMA)
    generated_types: dict[str, str] = {}
    manual_types: dict[str, str] = {"Option<()>": "Empty"}
    record_manual_rpc_types(
        client_entries, "params", client_request_native, manual_types
    )
    record_manual_rpc_types(
        client_entries, "response", client_response_native, manual_types
    )
    record_manual_rpc_types(
        server_entries, "params", server_request_native, manual_types
    )
    record_manual_rpc_types(
        server_entries, "response", server_response_native, manual_types
    )
    record_manual_notification_types(
        notification_entries, notification_native, manual_types
    )

    add_schema_types(
        client_entries,
        "params",
        client_request_native,
        schema,
        generated_types,
        manual_types,
    )
    add_schema_types(
        client_entries,
        "response",
        client_response_native,
        schema,
        generated_types,
        manual_types,
    )
    add_schema_types(
        server_entries,
        "params",
        server_request_native,
        schema,
        generated_types,
        manual_types,
    )
    add_schema_types(
        server_entries,
        "response",
        server_response_native,
        schema,
        generated_types,
        manual_types,
    )
    add_notification_schema_types(
        notification_entries,
        notification_native,
        schema,
        generated_types,
        manual_types,
    )

    expected_maps = (
        (client_requests, client_request_native),
        (client_requests, client_response_native),
        (server_requests, server_request_native),
        (server_requests, server_response_native),
        (notifications, notification_native),
    )
    for variants, native_types in expected_maps:
        missing = set(variants) - set(native_types)
        if missing:
            raise RuntimeError(f"missing native protobuf types: {sorted(missing)}")

    schema_proto = schema.render()
    SCHEMA_NATIVE_TYPES.write_text(
        render_native_impls(generated_types, schema.nullable_wrappers)
    )

    proto = f"""syntax = "proto3";

package codex.app_server.v2;

// Native protobuf app-server session protocol.
//
// Every registered app-server method has an explicit oneof variant and a
// concrete protobuf message. DynamicValue is reserved for fields whose API
// contract is intentionally open JSON or cannot be represented by proto3.
service CodexAppServer {{
  rpc Session(stream ClientMessage) returns (stream ServerMessage);
  rpc Health(HealthRequest) returns (HealthResponse);
  rpc Schema(SchemaRequest) returns (SchemaResponse);
}}

message Empty {{}}

message RequestId {{
  oneof value {{
    string string_id = 1;
    int64 integer_id = 2;
  }}
}}

message TraceContext {{
  optional string traceparent = 1;
  optional string tracestate = 2;
}}

message DynamicValue {{
  oneof kind {{
    Empty null_value = 1;
    bool bool_value = 2;
    int64 integer_value = 3;
    uint64 unsigned_integer_value = 4;
    double number_value = 5;
    string string_value = 6;
    DynamicList list_value = 7;
    DynamicObject object_value = 8;
  }}
}}

message DynamicList {{
  repeated DynamicValue values = 1;
}}

message DynamicObject {{
  map<string, DynamicValue> fields = 1;
}}

{NATIVE_PROTO_MESSAGES}

{schema_proto}

message ClientMessage {{
  oneof payload {{
    ClientRequest request = 1;
    ClientNotification notification = 2;
    ServerResponse response = 3;
    ClientError error = 4;
  }}
}}

message ClientRequest {{
  RequestId id = 1;
  TraceContext trace = 2;
  oneof method {{
{oneof_fields(client_requests, client_request_native)}
  }}
}}

message ClientNotification {{
  oneof method {{
    Empty initialized = 1;
  }}
}}

message ServerResponse {{
  RequestId id = 1;
  oneof method {{
{oneof_fields(server_requests, server_response_native)}
  }}
}}

message ClientError {{
  RequestId id = 1;
  RpcError error = 2;
}}

message ServerMessage {{
  oneof payload {{
    ClientResponse response = 1;
    RpcErrorResponse error = 2;
    ServerNotification notification = 3;
    ServerRequest request = 4;
  }}
}}

message ClientResponse {{
  RequestId id = 1;
  oneof method {{
{oneof_fields(client_requests, client_response_native)}
  }}
}}

message RpcErrorResponse {{
  RequestId id = 1;
  RpcError error = 2;
}}

message RpcError {{
  int64 code = 1;
  string message = 2;
  DynamicValue data = 3;
}}

message ServerNotification {{
  oneof method {{
{oneof_fields(notifications, notification_native)}
  }}
}}

message ServerRequest {{
  RequestId id = 1;
  oneof method {{
{oneof_fields(server_requests, server_request_native)}
  }}
}}

message HealthRequest {{}}

message HealthResponse {{
  enum ServingStatus {{
    SERVING_STATUS_UNSPECIFIED = 0;
    SERVING = 1;
  }}
  ServingStatus status = 1;
}}

message SchemaRequest {{}}

message SchemaResponse {{
  string proto_source = 1;
}}
"""
    PROTO.write_text(proto)

    conversions = f"""// Generated by scripts/generate_native_grpc.py. Do not edit manually.

use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::RpcError;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ServerResponse;
use serde_json::Number;
use serde_json::Value;
use tonic::Status;

use super::grpc::proto;
use super::grpc_native_types::decode_native;
use super::grpc_native_types::encode_native;

pub(crate) fn decode_client_request(
    request: proto::ClientRequest,
) -> Result<(ClientRequest, Option<codex_protocol::protocol::W3cTraceContext>), Status> {{
    let request_id = decode_request_id(required(request.id, "request id")?)?;
    let trace = request.trace.map(|trace| codex_protocol::protocol::W3cTraceContext {{
        traceparent: trace.traceparent,
        tracestate: trace.tracestate,
    }});
    let method = required(request.method, "request method")?;
    let request = match method {{
{decode_match("proto::client_request::Method", "ClientRequest", client_requests, "request")}
    }};
    Ok((request, trace))
}}

pub(crate) fn encode_client_request(
    request: ClientRequest,
    trace: Option<codex_protocol::protocol::W3cTraceContext>,
) -> Result<proto::ClientRequest, Status> {{
    let (request_id, method) = match request {{
{encode_match("ClientRequest", "proto::client_request::Method", client_requests, "request")}
    }};
    Ok(proto::ClientRequest {{
        id: Some(encode_request_id(request_id)),
        trace: trace.map(|trace| proto::TraceContext {{
            traceparent: trace.traceparent,
            tracestate: trace.tracestate,
        }}),
        method: Some(method),
    }})
}}

pub(crate) fn decode_server_response(
    response: proto::ServerResponse,
) -> Result<ServerResponse, Status> {{
    let request_id = decode_request_id(required(response.id, "response id")?)?;
    let method = required(response.method, "response method")?;
    Ok(match method {{
{decode_match("proto::server_response::Method", "ServerResponse", server_requests, "response")}
    }})
}}

pub(crate) fn encode_server_response(
    response: ServerResponse,
) -> Result<proto::ServerResponse, Status> {{
    let (request_id, method) = match response {{
{encode_match("ServerResponse", "proto::server_response::Method", server_requests, "response")}
    }};
    Ok(proto::ServerResponse {{
        id: Some(encode_request_id(request_id)),
        method: Some(method),
    }})
}}

pub(crate) fn encode_client_response(
    response: ClientResponse,
) -> Result<proto::ClientResponse, Status> {{
    let (request_id, method) = match response {{
{encode_match("ClientResponse", "proto::client_response::Method", client_requests, "response")}
    }};
    Ok(proto::ClientResponse {{
        id: Some(encode_request_id(request_id)),
        method: Some(method),
    }})
}}

pub(crate) fn decode_client_response(
    response: proto::ClientResponse,
) -> Result<ClientResponse, Status> {{
    let request_id = decode_request_id(required(response.id, "response id")?)?;
    let method = required(response.method, "response method")?;
    Ok(match method {{
{decode_match("proto::client_response::Method", "ClientResponse", client_requests, "response")}
    }})
}}

pub(crate) fn encode_server_request(
    request: ServerRequest,
) -> Result<proto::ServerRequest, Status> {{
    let (request_id, method) = match request {{
{encode_match("ServerRequest", "proto::server_request::Method", server_requests, "request")}
    }};
    Ok(proto::ServerRequest {{
        id: Some(encode_request_id(request_id)),
        method: Some(method),
    }})
}}

pub(crate) fn decode_server_request(
    request: proto::ServerRequest,
) -> Result<ServerRequest, Status> {{
    let request_id = decode_request_id(required(request.id, "request id")?)?;
    let method = required(request.method, "request method")?;
    Ok(match method {{
{decode_match("proto::server_request::Method", "ServerRequest", server_requests, "request")}
    }})
}}

pub(crate) fn encode_server_notification(
    notification: ServerNotification,
) -> Result<proto::ServerNotification, Status> {{
    let method = match notification {{
{encode_tuple_match("ServerNotification", "proto::server_notification::Method", notifications)}
    }};
    Ok(proto::ServerNotification {{ method: Some(method) }})
}}

pub(crate) fn decode_server_notification(
    notification: proto::ServerNotification,
) -> Result<ServerNotification, Status> {{
    let method = required(notification.method, "notification method")?;
    Ok(match method {{
{decode_tuple_match("proto::server_notification::Method", "ServerNotification", notifications)}
    }})
}}

pub(crate) fn encode_client_error(
    request_id: RequestId,
    error: RpcError,
) -> Result<proto::ClientError, Status> {{
    Ok(proto::ClientError {{
        id: Some(encode_request_id(request_id)),
        error: Some(proto::RpcError {{
            code: error.code,
            message: error.message,
            data: error.data.map(encode_dynamic_value).transpose()?,
        }}),
    }})
}}

pub(crate) fn decode_error(error: proto::ClientError) -> Result<(RequestId, RpcError), Status> {{
    let request_id = decode_request_id(required(error.id, "error id")?)?;
    let error = required(error.error, "error payload")?;
    Ok((
        request_id,
        RpcError {{
            code: error.code,
            message: error.message,
            data: error.data.map(decode_dynamic_value).transpose()?,
        }},
    ))
}}

pub(crate) fn encode_error(
    request_id: RequestId,
    error: RpcError,
) -> Result<proto::RpcErrorResponse, Status> {{
    Ok(proto::RpcErrorResponse {{
        id: Some(encode_request_id(request_id)),
        error: Some(proto::RpcError {{
            code: error.code,
            message: error.message,
            data: error.data.map(encode_dynamic_value).transpose()?,
        }}),
    }})
}}

pub(crate) fn decode_error_response(
    error: proto::RpcErrorResponse,
) -> Result<(RequestId, RpcError), Status> {{
    let request_id = decode_request_id(required(error.id, "error id")?)?;
    let error = required(error.error, "error payload")?;
    Ok((
        request_id,
        RpcError {{
            code: error.code,
            message: error.message,
            data: error.data.map(decode_dynamic_value).transpose()?,
        }},
    ))
}}

fn required<T>(value: Option<T>, name: &'static str) -> Result<T, Status> {{
    value.ok_or_else(|| Status::invalid_argument(format!("missing {{name}}")))
}}

fn decode_request_id(request_id: proto::RequestId) -> Result<RequestId, Status> {{
    match required(request_id.value, "request id value")? {{
        proto::request_id::Value::StringId(value) => Ok(RequestId::String(value)),
        proto::request_id::Value::IntegerId(value) => Ok(RequestId::Integer(value)),
    }}
}}

fn encode_request_id(request_id: RequestId) -> proto::RequestId {{
    let value = match request_id {{
        RequestId::String(value) => proto::request_id::Value::StringId(value),
        RequestId::Integer(value) => proto::request_id::Value::IntegerId(value),
    }};
    proto::RequestId {{ value: Some(value) }}
}}

pub(crate) fn decode_dynamic_value(value: proto::DynamicValue) -> Result<Value, Status> {{
    let kind = required(value.kind, "dynamic value kind")?;
    Ok(match kind {{
        proto::dynamic_value::Kind::NullValue(_) => Value::Null,
        proto::dynamic_value::Kind::BoolValue(value) => Value::Bool(value),
        proto::dynamic_value::Kind::IntegerValue(value) => Value::Number(value.into()),
        proto::dynamic_value::Kind::UnsignedIntegerValue(value) => Value::Number(value.into()),
        proto::dynamic_value::Kind::NumberValue(value) => Value::Number(
            Number::from_f64(value)
                .ok_or_else(|| Status::invalid_argument("non-finite protobuf number"))?,
        ),
        proto::dynamic_value::Kind::StringValue(value) => Value::String(value),
        proto::dynamic_value::Kind::ListValue(value) => Value::Array(
            value
                .values
                .into_iter()
                .map(decode_dynamic_value)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        proto::dynamic_value::Kind::ObjectValue(value) => Value::Object(
            value
                .fields
                .into_iter()
                .map(|(key, value)| decode_dynamic_value(value).map(|value| (key, value)))
                .collect::<Result<serde_json::Map<_, _>, _>>()?,
        ),
    }})
}}

pub(crate) fn encode_dynamic_value(value: Value) -> Result<proto::DynamicValue, Status> {{
    let kind = match value {{
        Value::Null => proto::dynamic_value::Kind::NullValue(proto::Empty {{}}),
        Value::Bool(value) => proto::dynamic_value::Kind::BoolValue(value),
        Value::Number(value) => {{
            if let Some(value) = value.as_i64() {{
                proto::dynamic_value::Kind::IntegerValue(value)
            }} else if let Some(value) = value.as_u64() {{
                proto::dynamic_value::Kind::UnsignedIntegerValue(value)
            }} else {{
                proto::dynamic_value::Kind::NumberValue(
                    value
                        .as_f64()
                        .ok_or_else(|| Status::internal("invalid JSON number"))?,
                )
            }}
        }}
        Value::String(value) => proto::dynamic_value::Kind::StringValue(value),
        Value::Array(values) => proto::dynamic_value::Kind::ListValue(proto::DynamicList {{
            values: values
                .into_iter()
                .map(encode_dynamic_value)
                .collect::<Result<Vec<_>, _>>()?,
        }}),
        Value::Object(fields) => proto::dynamic_value::Kind::ObjectValue(proto::DynamicObject {{
            fields: fields
                .into_iter()
                .map(|(key, value)| encode_dynamic_value(value).map(|value| (key, value)))
                .collect::<Result<_, _>>()?,
        }}),
    }};
    Ok(proto::DynamicValue {{ kind: Some(kind) }})
}}
"""
    CONVERSIONS.write_text(conversions)

    print(
        f"generated {len(client_requests)} client requests, "
        f"{len(server_requests)} server requests, and "
        f"{len(notifications)} notifications"
    )


if __name__ == "__main__":
    main()
