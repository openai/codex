use super::*;
use codex_protocol::approvals::ElicitationRequest as CoreElicitationRequest;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::AgentMessageItem;
use codex_protocol::items::FileChangeItem;
use codex_protocol::items::ImageViewItem;
use codex_protocol::items::McpToolCallItem;
use codex_protocol::items::McpToolCallStatus as CoreMcpToolCallStatus;
use codex_protocol::items::ReasoningItem;
use codex_protocol::items::TurnItem;
use codex_protocol::items::UserMessageItem;
use codex_protocol::items::WebSearchItem;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::memory_citation::MemoryCitation as CoreMemoryCitation;
use codex_protocol::memory_citation::MemoryCitationEntry as CoreMemoryCitationEntry;
use codex_protocol::models::AdditionalPermissionProfile as CoreAdditionalPermissionProfile;
use codex_protocol::models::FileSystemPermissions as CoreFileSystemPermissions;
use codex_protocol::models::ManagedFileSystemPermissions as CoreManagedFileSystemPermissions;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::NetworkPermissions as CoreNetworkPermissions;
use codex_protocol::models::WebSearchAction as CoreWebSearchAction;
use codex_protocol::permissions::FileSystemAccessMode as CoreFileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath as CoreFileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry as CoreFileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSpecialPath as CoreFileSystemSpecialPath;
use codex_protocol::protocol::AgentStatus as CoreAgentStatus;
use codex_protocol::protocol::AskForApproval as CoreAskForApproval;
use codex_protocol::protocol::GranularApprovalConfig as CoreGranularApprovalConfig;
use codex_protocol::protocol::NetworkAccess as CoreNetworkAccess;
use codex_protocol::request_permissions::RequestPermissionProfile as CoreRequestPermissionProfile;
use codex_protocol::user_input::UserInput as CoreUserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use pretty_assertions::assert_eq;
use serde_json::Value as JsonValue;
use serde_json::json;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::time::Duration;

fn absolute_path_string(path: &str) -> String {
    let path = format!("/{}", path.trim_start_matches('/'));
    test_path_buf(&path).display().to_string()
}

fn absolute_path(path: &str) -> AbsolutePathBuf {
    let path = format!("/{}", path.trim_start_matches('/'));
    test_path_buf(&path).abs()
}

fn test_absolute_path() -> AbsolutePathBuf {
    absolute_path("readable")
}

#[test]
fn approvals_reviewer_serializes_auto_review_and_accepts_legacy_guardian_subagent() {
    assert_eq!(
        serde_json::to_string(&ApprovalsReviewer::User).expect("serialize reviewer"),
        "\"user\""
    );
    assert_eq!(
        serde_json::to_string(&ApprovalsReviewer::AutoReview).expect("serialize reviewer"),
        "\"guardian_subagent\""
    );

    for value in ["user", "auto_review", "guardian_subagent"] {
        let json = format!("\"{value}\"");
        let reviewer: ApprovalsReviewer =
            serde_json::from_str(&json).expect("deserialize reviewer");
        let expected = if value == "user" {
            ApprovalsReviewer::User
        } else {
            ApprovalsReviewer::AutoReview
        };
        assert_eq!(expected, reviewer);
    }
}

#[test]
fn turn_defaults_legacy_missing_items_view_to_full() {
    let turn: Turn = serde_json::from_value(json!({
        "id": "turn_123",
        "items": [],
        "status": "completed",
        "error": null,
        "startedAt": null,
        "completedAt": null,
        "durationMs": null,
    }))
    .expect("legacy turn should deserialize");

    assert_eq!(turn.items_view, TurnItemsView::Full);
}

#[test]
fn thread_list_params_accepts_single_cwd() {
    let params = serde_json::from_value::<ThreadListParams>(json!({
        "cwd": "/workspace",
    }))
    .expect("single cwd should deserialize");

    assert_eq!(
        params.cwd,
        Some(ThreadListCwdFilter::One("/workspace".to_string()))
    );
    assert!(!params.use_state_db_only);
}

#[test]
fn thread_list_params_accepts_multiple_cwds() {
    let params = serde_json::from_value::<ThreadListParams>(json!({
        "cwd": ["/workspace", "/other-workspace"],
    }))
    .expect("cwd array should deserialize");

    assert_eq!(
        params.cwd,
        Some(ThreadListCwdFilter::Many(vec![
            "/workspace".to_string(),
            "/other-workspace".to_string(),
        ]))
    );
}

#[test]
fn thread_list_params_accepts_state_db_only_flag() {
    let params = serde_json::from_value::<ThreadListParams>(json!({
        "useStateDbOnly": true,
    }))
    .expect("state db only flag should deserialize");

    assert!(params.use_state_db_only);
}

#[test]
fn collab_agent_state_maps_interrupted_status() {
    assert_eq!(
        CollabAgentState::from(CoreAgentStatus::Interrupted),
        CollabAgentState {
            status: CollabAgentStatus::Interrupted,
            message: None,
        }
    );
}

#[test]
fn external_agent_config_plugins_details_round_trip() {
    let item: ExternalAgentConfigMigrationItem = serde_json::from_value(json!({
        "itemType": "PLUGINS",
        "description": "Install supported plugins from Claude settings",
        "cwd": absolute_path_string("repo"),
        "details": {
            "plugins": [
                {
                    "marketplaceName": "team-marketplace",
                    "pluginNames": ["asana"]
                }
            ]
        }
    }))
    .expect("plugins migration item should deserialize");

    assert_eq!(
        item,
        ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Plugins,
            description: "Install supported plugins from Claude settings".to_string(),
            cwd: Some(PathBuf::from(absolute_path_string("repo"))),
            details: Some(MigrationDetails {
                plugins: vec![PluginsMigration {
                    marketplace_name: "team-marketplace".to_string(),
                    plugin_names: vec!["asana".to_string()],
                }],
                ..Default::default()
            }),
        }
    );
}

#[test]
fn external_agent_config_import_params_accept_legacy_plugin_details() {
    let params: ExternalAgentConfigImportParams = serde_json::from_value(json!({
        "migrationItems": [{
            "itemType": "PLUGINS",
            "description": "Install supported plugins from Claude settings",
            "cwd": absolute_path_string("repo"),
            "details": {
                "plugins": [
                    {
                        "marketplaceName": "team-marketplace",
                        "pluginNames": ["asana"]
                    }
                ]
            }
        }]
    }))
    .expect("legacy plugin import params should deserialize");

    assert_eq!(
        params,
        ExternalAgentConfigImportParams {
            migration_items: vec![ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Plugins,
                description: "Install supported plugins from Claude settings".to_string(),
                cwd: Some(PathBuf::from(absolute_path_string("repo"))),
                details: Some(MigrationDetails {
                    plugins: vec![PluginsMigration {
                        marketplace_name: "team-marketplace".to_string(),
                        plugin_names: vec!["asana".to_string()],
                    }],
                    ..Default::default()
                }),
            }],
        }
    );
}

#[test]
fn command_execution_request_approval_rejects_relative_additional_permission_paths() {
    let err = serde_json::from_value::<CommandExecutionRequestApprovalParams>(json!({
        "threadId": "thr_123",
        "turnId": "turn_123",
        "itemId": "call_123",
        "command": "cat file",
        "cwd": absolute_path_string("tmp"),
        "commandActions": null,
        "reason": null,
        "networkApprovalContext": null,
        "additionalPermissions": {
            "network": null,
            "fileSystem": {
                "read": ["relative/path"],
                "write": null
            }
        },
        "proposedExecpolicyAmendment": null,
        "proposedNetworkPolicyAmendments": null,
        "availableDecisions": null
    }))
    .expect_err("relative additional permission paths should fail");
    assert!(
        err.to_string()
            .contains("AbsolutePathBuf deserialized without a base path"),
        "unexpected error: {err}"
    );
}

#[test]
fn permissions_request_approval_uses_request_permission_profile() {
    let read_only_path = if cfg!(windows) {
        r"C:\tmp\read-only"
    } else {
        "/tmp/read-only"
    };
    let read_write_path = if cfg!(windows) {
        r"C:\tmp\read-write"
    } else {
        "/tmp/read-write"
    };
    let params = serde_json::from_value::<PermissionsRequestApprovalParams>(json!({
        "threadId": "thr_123",
        "turnId": "turn_123",
        "itemId": "call_123",
        "cwd": absolute_path_string("repo"),
        "reason": "Select a workspace root",
        "permissions": {
            "network": {
                "enabled": true,
            },
            "fileSystem": {
                "read": [read_only_path],
                "write": [read_write_path],
            },
        },
    }))
    .expect("permissions request should deserialize");

    assert_eq!(params.cwd, absolute_path("repo"));
    assert_eq!(
        params.permissions,
        RequestPermissionProfile {
            network: Some(AdditionalNetworkPermissions {
                enabled: Some(true),
            }),
            file_system: Some(AdditionalFileSystemPermissions {
                read: Some(vec![
                    AbsolutePathBuf::try_from(PathBuf::from(read_only_path))
                        .expect("path must be absolute"),
                ]),
                write: Some(vec![
                    AbsolutePathBuf::try_from(PathBuf::from(read_write_path))
                        .expect("path must be absolute"),
                ]),
                glob_scan_max_depth: None,
                entries: None,
            }),
        }
    );

    assert_eq!(
        CoreRequestPermissionProfile::from(params.permissions),
        CoreRequestPermissionProfile {
            network: Some(CoreNetworkPermissions {
                enabled: Some(true),
            }),
            file_system: Some(CoreFileSystemPermissions::from_read_write_roots(
                Some(vec![
                    AbsolutePathBuf::try_from(PathBuf::from(read_only_path))
                        .expect("path must be absolute"),
                ]),
                Some(vec![
                    AbsolutePathBuf::try_from(PathBuf::from(read_write_path))
                        .expect("path must be absolute"),
                ]),
            )),
        }
    );
}

#[test]
fn permissions_request_approval_rejects_macos_permissions() {
    let err = serde_json::from_value::<PermissionsRequestApprovalParams>(json!({
        "threadId": "thr_123",
        "turnId": "turn_123",
        "itemId": "call_123",
        "cwd": absolute_path_string("repo"),
        "reason": "Select a workspace root",
        "permissions": {
            "network": null,
            "fileSystem": null,
            "macos": {
                "preferences": "read_only",
                "automations": "none",
                "launchServices": false,
                "accessibility": false,
                "calendar": false,
                "reminders": false,
                "contacts": "none",
            },
        },
    }))
    .expect_err("permissions request should reject macos permissions");

    assert!(
        err.to_string().contains("unknown field `macos`"),
        "unexpected error: {err}"
    );
}

#[test]
fn additional_file_system_permissions_preserves_canonical_entries() {
    let core_permissions = CoreFileSystemPermissions {
        entries: vec![
            CoreFileSystemSandboxEntry {
                path: CoreFileSystemPath::Special {
                    value: CoreFileSystemSpecialPath::Root,
                },
                access: CoreFileSystemAccessMode::Write,
            },
            CoreFileSystemSandboxEntry {
                path: CoreFileSystemPath::GlobPattern {
                    pattern: "**/*.env".to_string(),
                },
                access: CoreFileSystemAccessMode::None,
            },
        ],
        glob_scan_max_depth: NonZeroUsize::new(2),
    };

    let permissions = AdditionalFileSystemPermissions::from(core_permissions.clone());
    assert_eq!(
        permissions,
        AdditionalFileSystemPermissions {
            read: None,
            write: None,
            glob_scan_max_depth: NonZeroUsize::new(2),
            entries: Some(vec![
                FileSystemSandboxEntry {
                    path: FileSystemPath::Special {
                        value: FileSystemSpecialPath::Root,
                    },
                    access: FileSystemAccessMode::Write,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::GlobPattern {
                        pattern: "**/*.env".to_string(),
                    },
                    access: FileSystemAccessMode::None,
                },
            ]),
        }
    );
    assert_eq!(
        CoreFileSystemPermissions::from(permissions),
        core_permissions
    );
}

#[test]
fn additional_file_system_permissions_populates_entries_for_legacy_roots() {
    let read_only_path = absolute_path("read-only");
    let read_write_path = absolute_path("read-write");
    let core_permissions = CoreFileSystemPermissions::from_read_write_roots(
        Some(vec![read_only_path.clone()]),
        Some(vec![read_write_path.clone()]),
    );

    let permissions = AdditionalFileSystemPermissions::from(core_permissions.clone());

    assert_eq!(
        permissions,
        AdditionalFileSystemPermissions {
            read: Some(vec![read_only_path.clone()]),
            write: Some(vec![read_write_path.clone()]),
            glob_scan_max_depth: None,
            entries: Some(vec![
                FileSystemSandboxEntry {
                    path: FileSystemPath::Path {
                        path: read_only_path,
                    },
                    access: FileSystemAccessMode::Read,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Path {
                        path: read_write_path,
                    },
                    access: FileSystemAccessMode::Write,
                },
            ]),
        }
    );
    assert_eq!(
        CoreFileSystemPermissions::from(permissions),
        core_permissions
    );
}

#[test]
fn additional_file_system_permissions_rejects_zero_glob_scan_depth() {
    serde_json::from_value::<AdditionalFileSystemPermissions>(json!({
        "read": null,
        "write": null,
        "globScanMaxDepth": 0,
        "entries": [],
    }))
    .expect_err("zero glob scan depth should fail deserialization");
}

#[test]
fn permission_profile_file_system_permissions_preserves_glob_scan_depth() {
    let core_permissions = CoreManagedFileSystemPermissions::Restricted {
        entries: vec![CoreFileSystemSandboxEntry {
            path: CoreFileSystemPath::GlobPattern {
                pattern: "**/*.env".to_string(),
            },
            access: CoreFileSystemAccessMode::None,
        }],
        glob_scan_max_depth: NonZeroUsize::new(2),
    };

    let permissions = PermissionProfileFileSystemPermissions::from(core_permissions.clone());

    assert_eq!(
        permissions,
        PermissionProfileFileSystemPermissions::Restricted {
            entries: vec![FileSystemSandboxEntry {
                path: FileSystemPath::GlobPattern {
                    pattern: "**/*.env".to_string(),
                },
                access: FileSystemAccessMode::None,
            }],
            glob_scan_max_depth: NonZeroUsize::new(2),
        }
    );
    assert_eq!(
        CoreManagedFileSystemPermissions::from(permissions),
        core_permissions
    );
}

#[test]
fn permission_profile_file_system_permissions_rejects_zero_glob_scan_depth() {
    serde_json::from_value::<PermissionProfileFileSystemPermissions>(json!({
        "type": "restricted",
        "entries": [],
        "globScanMaxDepth": 0,
    }))
    .expect_err("zero glob scan depth should fail deserialization");
}

#[test]
fn legacy_current_working_directory_special_path_deserializes_as_project_roots() {
    let special_path = serde_json::from_value::<FileSystemSpecialPath>(json!({
        "kind": "current_working_directory",
    }))
    .expect("legacy cwd special path should deserialize");

    assert_eq!(
        special_path,
        FileSystemSpecialPath::ProjectRoots { subpath: None }
    );
    assert_eq!(
        serde_json::to_value(&special_path).expect("serialize special path"),
        json!({
            "kind": "project_roots",
            "subpath": null,
        })
    );
}

#[test]
fn permissions_request_approval_response_uses_granted_permission_profile_without_macos() {
    let read_only_path = if cfg!(windows) {
        r"C:\tmp\read-only"
    } else {
        "/tmp/read-only"
    };
    let read_write_path = if cfg!(windows) {
        r"C:\tmp\read-write"
    } else {
        "/tmp/read-write"
    };
    let response = serde_json::from_value::<PermissionsRequestApprovalResponse>(json!({
        "permissions": {
            "network": {
                "enabled": true,
            },
            "fileSystem": {
                "read": [read_only_path],
                "write": [read_write_path],
            },
        },
    }))
    .expect("permissions response should deserialize");

    assert_eq!(
        response.permissions,
        GrantedPermissionProfile {
            network: Some(AdditionalNetworkPermissions {
                enabled: Some(true),
            }),
            file_system: Some(AdditionalFileSystemPermissions {
                read: Some(vec![
                    AbsolutePathBuf::try_from(PathBuf::from(read_only_path))
                        .expect("path must be absolute"),
                ]),
                write: Some(vec![
                    AbsolutePathBuf::try_from(PathBuf::from(read_write_path))
                        .expect("path must be absolute"),
                ]),
                glob_scan_max_depth: None,
                entries: None,
            }),
        }
    );

    assert_eq!(
        CoreAdditionalPermissionProfile::from(response.permissions),
        CoreAdditionalPermissionProfile {
            network: Some(CoreNetworkPermissions {
                enabled: Some(true),
            }),
            file_system: Some(CoreFileSystemPermissions::from_read_write_roots(
                Some(vec![
                    AbsolutePathBuf::try_from(PathBuf::from(read_only_path))
                        .expect("path must be absolute"),
                ]),
                Some(vec![
                    AbsolutePathBuf::try_from(PathBuf::from(read_write_path))
                        .expect("path must be absolute"),
                ]),
            )),
        }
    );
}

#[test]
fn permissions_request_approval_response_defaults_scope_to_turn() {
    let response = serde_json::from_value::<PermissionsRequestApprovalResponse>(json!({
        "permissions": {},
    }))
    .expect("response should deserialize");

    assert_eq!(response.scope, PermissionGrantScope::Turn);
    assert_eq!(response.strict_auto_review, None);
}

#[test]
fn permissions_request_approval_response_accepts_strict_auto_review() {
    let response = serde_json::from_value::<PermissionsRequestApprovalResponse>(json!({
        "permissions": {},
        "strictAutoReview": true,
    }))
    .expect("response should deserialize");

    assert_eq!(response.strict_auto_review, Some(true));
}

#[test]
fn fs_get_metadata_response_round_trips_minimal_fields() {
    let response = FsGetMetadataResponse {
        is_directory: false,
        is_file: true,
        is_symlink: false,
        created_at_ms: 123,
        modified_at_ms: 456,
    };

    let value = serde_json::to_value(&response).expect("serialize fs/getMetadata response");
    assert_eq!(
        value,
        json!({
            "isDirectory": false,
            "isFile": true,
            "isSymlink": false,
            "createdAtMs": 123,
            "modifiedAtMs": 456,
        })
    );

    let decoded = serde_json::from_value::<FsGetMetadataResponse>(value)
        .expect("deserialize fs/getMetadata response");
    assert_eq!(decoded, response);
}

#[test]
fn fs_read_file_response_round_trips_base64_data() {
    let response = FsReadFileResponse {
        data_base64: "aGVsbG8=".to_string(),
    };

    let value = serde_json::to_value(&response).expect("serialize fs/readFile response");
    assert_eq!(
        value,
        json!({
            "dataBase64": "aGVsbG8=",
        })
    );

    let decoded = serde_json::from_value::<FsReadFileResponse>(value)
        .expect("deserialize fs/readFile response");
    assert_eq!(decoded, response);
}

#[test]
fn fs_read_file_params_round_trip() {
    let params = FsReadFileParams {
        path: absolute_path("tmp/example.txt"),
    };

    let value = serde_json::to_value(&params).expect("serialize fs/readFile params");
    assert_eq!(
        value,
        json!({
            "path": absolute_path_string("tmp/example.txt"),
        })
    );

    let decoded =
        serde_json::from_value::<FsReadFileParams>(value).expect("deserialize fs/readFile params");
    assert_eq!(decoded, params);
}

#[test]
fn device_key_create_params_round_trip_uses_protection_policy() {
    let params = DeviceKeyCreateParams {
        protection_policy: None,
        account_user_id: "account-user-1".to_string(),
        client_id: "cli_123".to_string(),
    };

    let value = serde_json::to_value(&params).expect("serialize device/key/create params");
    assert_eq!(
        value,
        json!({
            "accountUserId": "account-user-1",
            "clientId": "cli_123",
            "protectionPolicy": null,
        })
    );

    let decoded = serde_json::from_value::<DeviceKeyCreateParams>(value)
        .expect("deserialize device/key/create params");
    assert_eq!(decoded, params);

    let params = DeviceKeyCreateParams {
        protection_policy: Some(DeviceKeyProtectionPolicy::AllowOsProtectedNonextractable),
        account_user_id: "account-user-1".to_string(),
        client_id: "cli_123".to_string(),
    };
    let value = serde_json::to_value(&params)
        .expect("serialize device/key/create params with protection policy");
    assert_eq!(
        value,
        json!({
            "accountUserId": "account-user-1",
            "clientId": "cli_123",
            "protectionPolicy": "allow_os_protected_nonextractable",
        })
    );
}

#[test]
fn device_key_create_response_round_trips_protection_class() {
    let response = DeviceKeyCreateResponse {
        key_id: "dk_123".to_string(),
        public_key_spki_der_base64: "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE".to_string(),
        algorithm: DeviceKeyAlgorithm::EcdsaP256Sha256,
        protection_class: DeviceKeyProtectionClass::OsProtectedNonextractable,
    };

    let value = serde_json::to_value(&response).expect("serialize device/key/create response");
    assert_eq!(
        value,
        json!({
            "keyId": "dk_123",
            "publicKeySpkiDerBase64": "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE",
            "algorithm": "ecdsa_p256_sha256",
            "protectionClass": "os_protected_nonextractable",
        })
    );

    let decoded = serde_json::from_value::<DeviceKeyCreateResponse>(value)
        .expect("deserialize device/key/create response");
    assert_eq!(decoded, response);
}

#[test]
fn device_key_sign_params_round_trip_uses_accepted_payload_enum() {
    let params = DeviceKeySignParams {
        key_id: "dk_123".to_string(),
        payload: DeviceKeySignPayload::RemoteControlClientConnection {
            nonce: "nonce-1".to_string(),
            audience: RemoteControlClientConnectionAudience::RemoteControlClientWebsocket,
            session_id: "wssess_123".to_string(),
            target_origin: "https://chatgpt.com".to_string(),
            target_path: "/api/codex/remote/control/client".to_string(),
            account_user_id: "account-user-1".to_string(),
            client_id: "cli_123".to_string(),
            token_sha256_base64url: "47DEQpj8HBSa-_TImW-5JCeuQeRkm5NMpJWZG3hSuFU".to_string(),
            token_expires_at: 1_700_000_000,
            scopes: vec!["remote_control_controller_websocket".to_string()],
        },
    };

    let value = serde_json::to_value(&params).expect("serialize device/key/sign params");
    assert_eq!(
        value,
        json!({
            "keyId": "dk_123",
            "payload": {
                "type": "remoteControlClientConnection",
                "nonce": "nonce-1",
                "audience": "remote_control_client_websocket",
                "sessionId": "wssess_123",
                "targetOrigin": "https://chatgpt.com",
                "targetPath": "/api/codex/remote/control/client",
                "accountUserId": "account-user-1",
                "clientId": "cli_123",
                "tokenSha256Base64url": "47DEQpj8HBSa-_TImW-5JCeuQeRkm5NMpJWZG3hSuFU",
                "tokenExpiresAt": 1_700_000_000,
                "scopes": ["remote_control_controller_websocket"],
            },
        })
    );

    let decoded = serde_json::from_value::<DeviceKeySignParams>(value)
        .expect("deserialize device/key/sign params");
    assert_eq!(decoded, params);
}

#[test]
fn device_key_sign_params_round_trip_uses_enrollment_payload() {
    let params = DeviceKeySignParams {
        key_id: "dk_123".to_string(),
        payload: DeviceKeySignPayload::RemoteControlClientEnrollment {
            nonce: "nonce-1".to_string(),
            audience: RemoteControlClientEnrollmentAudience::RemoteControlClientEnrollment,
            challenge_id: "rch_123".to_string(),
            target_origin: "https://chatgpt.com".to_string(),
            target_path: "/wham/remote/control/client/enroll".to_string(),
            account_user_id: "account-user-1".to_string(),
            client_id: "cli_123".to_string(),
            device_identity_sha256_base64url: "47DEQpj8HBSa-_TImW-5JCeuQeRkm5NMpJWZG3hSuFU"
                .to_string(),
            challenge_expires_at: 1_700_000_000,
        },
    };

    let value = serde_json::to_value(&params)
        .expect("serialize device/key/sign params with enrollment payload");
    assert_eq!(
        value,
        json!({
            "keyId": "dk_123",
            "payload": {
                "type": "remoteControlClientEnrollment",
                "nonce": "nonce-1",
                "audience": "remote_control_client_enrollment",
                "challengeId": "rch_123",
                "targetOrigin": "https://chatgpt.com",
                "targetPath": "/wham/remote/control/client/enroll",
                "accountUserId": "account-user-1",
                "clientId": "cli_123",
                "deviceIdentitySha256Base64url": "47DEQpj8HBSa-_TImW-5JCeuQeRkm5NMpJWZG3hSuFU",
                "challengeExpiresAt": 1_700_000_000,
            },
        })
    );

    let decoded = serde_json::from_value::<DeviceKeySignParams>(value)
        .expect("deserialize device/key/sign params with enrollment payload");
    assert_eq!(decoded, params);
}

#[test]
fn device_key_sign_response_returns_signed_payload_bytes() {
    let response = DeviceKeySignResponse {
        signature_der_base64: "MEUCIQD".to_string(),
        signed_payload_base64: "eyJkb21haW4iOiJjb2RleA".to_string(),
        algorithm: DeviceKeyAlgorithm::EcdsaP256Sha256,
    };

    let value = serde_json::to_value(&response).expect("serialize device/key/sign response");
    assert_eq!(
        value,
        json!({
            "signatureDerBase64": "MEUCIQD",
            "signedPayloadBase64": "eyJkb21haW4iOiJjb2RleA",
            "algorithm": "ecdsa_p256_sha256",
        })
    );

    let decoded = serde_json::from_value::<DeviceKeySignResponse>(value)
        .expect("deserialize device/key/sign response");
    assert_eq!(decoded, response);
}

#[test]
fn fs_create_directory_params_round_trip_with_default_recursive() {
    let params = FsCreateDirectoryParams {
        path: absolute_path("tmp/example"),
        recursive: None,
    };

    let value = serde_json::to_value(&params).expect("serialize fs/createDirectory params");
    assert_eq!(
        value,
        json!({
            "path": absolute_path_string("tmp/example"),
            "recursive": null,
        })
    );

    let decoded = serde_json::from_value::<FsCreateDirectoryParams>(value)
        .expect("deserialize fs/createDirectory params");
    assert_eq!(decoded, params);
}

#[test]
fn fs_write_file_params_round_trip_with_base64_data() {
    let params = FsWriteFileParams {
        path: absolute_path("tmp/example.bin"),
        data_base64: "AAE=".to_string(),
    };

    let value = serde_json::to_value(&params).expect("serialize fs/writeFile params");
    assert_eq!(
        value,
        json!({
            "path": absolute_path_string("tmp/example.bin"),
            "dataBase64": "AAE=",
        })
    );

    let decoded = serde_json::from_value::<FsWriteFileParams>(value)
        .expect("deserialize fs/writeFile params");
    assert_eq!(decoded, params);
}

#[test]
fn fs_copy_params_round_trip_with_recursive_directory_copy() {
    let params = FsCopyParams {
        source_path: absolute_path("tmp/source"),
        destination_path: absolute_path("tmp/destination"),
        recursive: true,
    };

    let value = serde_json::to_value(&params).expect("serialize fs/copy params");
    assert_eq!(
        value,
        json!({
            "sourcePath": absolute_path_string("tmp/source"),
            "destinationPath": absolute_path_string("tmp/destination"),
            "recursive": true,
        })
    );

    let decoded =
        serde_json::from_value::<FsCopyParams>(value).expect("deserialize fs/copy params");
    assert_eq!(decoded, params);
}

#[test]
fn thread_shell_command_params_round_trip() {
    let params = ThreadShellCommandParams {
        thread_id: "thr_123".to_string(),
        command: "printf 'hello world\\n'".to_string(),
    };

    let value = serde_json::to_value(&params).expect("serialize thread/shellCommand params");
    assert_eq!(
        value,
        json!({
            "threadId": "thr_123",
            "command": "printf 'hello world\\n'",
        })
    );

    let decoded = serde_json::from_value::<ThreadShellCommandParams>(value)
        .expect("deserialize thread/shellCommand params");
    assert_eq!(decoded, params);
}

#[test]
fn thread_shell_command_response_round_trip() {
    let response = ThreadShellCommandResponse {};

    let value = serde_json::to_value(&response).expect("serialize thread/shellCommand response");
    assert_eq!(value, json!({}));

    let decoded = serde_json::from_value::<ThreadShellCommandResponse>(value)
        .expect("deserialize thread/shellCommand response");
    assert_eq!(decoded, response);
}

#[test]
fn fs_changed_notification_round_trips() {
    let notification = FsChangedNotification {
        watch_id: "0195ec6b-1d6f-7c2e-8c7a-56f2c4a8b9d1".to_string(),
        changed_paths: vec![
            absolute_path("tmp/repo/.git/HEAD"),
            absolute_path("tmp/repo/.git/FETCH_HEAD"),
        ],
    };

    let value = serde_json::to_value(&notification).expect("serialize fs/changed notification");
    assert_eq!(
        value,
        json!({
            "watchId": "0195ec6b-1d6f-7c2e-8c7a-56f2c4a8b9d1",
            "changedPaths": [
                absolute_path_string("tmp/repo/.git/HEAD"),
                absolute_path_string("tmp/repo/.git/FETCH_HEAD"),
            ],
        })
    );

    let decoded = serde_json::from_value::<FsChangedNotification>(value)
        .expect("deserialize fs/changed notification");
    assert_eq!(decoded, notification);
}

#[test]
fn command_exec_params_default_optional_streaming_flags() {
    let params = serde_json::from_value::<CommandExecParams>(json!({
        "command": ["ls", "-la"],
        "timeoutMs": 1000,
        "cwd": "/tmp"
    }))
    .expect("command/exec payload should deserialize");

    assert_eq!(
        params,
        CommandExecParams {
            command: vec!["ls".to_string(), "-la".to_string()],
            process_id: None,
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: Some(1000),
            cwd: Some(PathBuf::from("/tmp")),
            env: None,
            size: None,
            sandbox_policy: None,
            permission_profile: None,
        }
    );
}

#[test]
fn command_exec_params_round_trips_disable_timeout() {
    let params = CommandExecParams {
        command: vec!["sleep".to_string(), "30".to_string()],
        process_id: Some("sleep-1".to_string()),
        tty: false,
        stream_stdin: false,
        stream_stdout_stderr: false,
        output_bytes_cap: None,
        disable_output_cap: false,
        disable_timeout: true,
        timeout_ms: None,
        cwd: None,
        env: None,
        size: None,
        sandbox_policy: None,
        permission_profile: None,
    };

    let value = serde_json::to_value(&params).expect("serialize command/exec params");
    assert_eq!(
        value,
        json!({
            "command": ["sleep", "30"],
            "processId": "sleep-1",
            "disableTimeout": true,
            "timeoutMs": null,
            "cwd": null,
            "env": null,
            "size": null,
            "sandboxPolicy": null,
            "permissionProfile": null,
            "outputBytesCap": null,
        })
    );

    let decoded =
        serde_json::from_value::<CommandExecParams>(value).expect("deserialize round-trip");
    assert_eq!(decoded, params);
}

#[test]
fn process_spawn_params_round_trips_without_sandbox_policy() {
    let params = ProcessSpawnParams {
        command: vec!["sleep".to_string(), "30".to_string()],
        process_handle: "sleep-1".to_string(),
        cwd: test_absolute_path(),
        tty: false,
        stream_stdin: false,
        stream_stdout_stderr: false,
        output_bytes_cap: None,
        timeout_ms: None,
        env: None,
        size: None,
    };

    let value = serde_json::to_value(&params).expect("serialize process/spawn params");
    assert_eq!(
        value,
        json!({
            "command": ["sleep", "30"],
            "processHandle": "sleep-1",
            "cwd": absolute_path_string("readable"),
            "env": null,
            "size": null,
        })
    );

    let decoded =
        serde_json::from_value::<ProcessSpawnParams>(value).expect("deserialize round-trip");
    assert_eq!(decoded, params);
}

#[test]
fn process_spawn_params_distinguish_omitted_null_and_value_limits() {
    let base = json!({
        "command": ["sleep", "30"],
        "processHandle": "sleep-1",
        "cwd": absolute_path_string("readable"),
    });

    let expected_omitted = ProcessSpawnParams {
        command: vec!["sleep".to_string(), "30".to_string()],
        process_handle: "sleep-1".to_string(),
        cwd: test_absolute_path(),
        tty: false,
        stream_stdin: false,
        stream_stdout_stderr: false,
        output_bytes_cap: None,
        timeout_ms: None,
        env: None,
        size: None,
    };
    let decoded =
        serde_json::from_value::<ProcessSpawnParams>(base).expect("deserialize omitted limits");
    assert_eq!(decoded, expected_omitted);

    let decoded = serde_json::from_value::<ProcessSpawnParams>(json!({
        "command": ["sleep", "30"],
        "processHandle": "sleep-1",
        "cwd": absolute_path_string("readable"),
        "outputBytesCap": null,
        "timeoutMs": null,
    }))
    .expect("deserialize disabled limits");
    assert_eq!(
        decoded,
        ProcessSpawnParams {
            output_bytes_cap: Some(None),
            timeout_ms: Some(None),
            ..expected_omitted.clone()
        }
    );

    let decoded = serde_json::from_value::<ProcessSpawnParams>(json!({
        "command": ["sleep", "30"],
        "processHandle": "sleep-1",
        "cwd": absolute_path_string("readable"),
        "outputBytesCap": 123,
        "timeoutMs": 456,
    }))
    .expect("deserialize explicit limits");
    assert_eq!(
        decoded,
        ProcessSpawnParams {
            output_bytes_cap: Some(Some(123)),
            timeout_ms: Some(Some(456)),
            ..expected_omitted
        }
    );
}

#[test]
fn command_exec_params_round_trips_disable_output_cap() {
    let params = CommandExecParams {
        command: vec!["yes".to_string()],
        process_id: Some("yes-1".to_string()),
        tty: false,
        stream_stdin: false,
        stream_stdout_stderr: true,
        output_bytes_cap: None,
        disable_output_cap: true,
        disable_timeout: false,
        timeout_ms: None,
        cwd: None,
        env: None,
        size: None,
        sandbox_policy: None,
        permission_profile: None,
    };

    let value = serde_json::to_value(&params).expect("serialize command/exec params");
    assert_eq!(
        value,
        json!({
            "command": ["yes"],
            "processId": "yes-1",
            "streamStdoutStderr": true,
            "outputBytesCap": null,
            "disableOutputCap": true,
            "timeoutMs": null,
            "cwd": null,
            "env": null,
            "size": null,
            "sandboxPolicy": null,
            "permissionProfile": null,
        })
    );

    let decoded =
        serde_json::from_value::<CommandExecParams>(value).expect("deserialize round-trip");
    assert_eq!(decoded, params);
}

#[test]
fn command_exec_params_round_trips_env_overrides_and_unsets() {
    let params = CommandExecParams {
        command: vec!["printenv".to_string(), "FOO".to_string()],
        process_id: Some("env-1".to_string()),
        tty: false,
        stream_stdin: false,
        stream_stdout_stderr: false,
        output_bytes_cap: None,
        disable_output_cap: false,
        disable_timeout: false,
        timeout_ms: None,
        cwd: None,
        env: Some(HashMap::from([
            ("FOO".to_string(), Some("override".to_string())),
            ("BAR".to_string(), Some("added".to_string())),
            ("BAZ".to_string(), None),
        ])),
        size: None,
        sandbox_policy: None,
        permission_profile: None,
    };

    let value = serde_json::to_value(&params).expect("serialize command/exec params");
    assert_eq!(
        value,
        json!({
            "command": ["printenv", "FOO"],
            "processId": "env-1",
            "outputBytesCap": null,
            "timeoutMs": null,
            "cwd": null,
            "env": {
                "FOO": "override",
                "BAR": "added",
                "BAZ": null,
            },
            "size": null,
            "sandboxPolicy": null,
            "permissionProfile": null,
        })
    );

    let decoded =
        serde_json::from_value::<CommandExecParams>(value).expect("deserialize round-trip");
    assert_eq!(decoded, params);
}

#[test]
fn command_exec_write_round_trips_close_only_payload() {
    let params = CommandExecWriteParams {
        process_id: "proc-7".to_string(),
        delta_base64: None,
        close_stdin: true,
    };

    let value = serde_json::to_value(&params).expect("serialize command/exec/write params");
    assert_eq!(
        value,
        json!({
            "processId": "proc-7",
            "deltaBase64": null,
            "closeStdin": true,
        })
    );

    let decoded =
        serde_json::from_value::<CommandExecWriteParams>(value).expect("deserialize round-trip");
    assert_eq!(decoded, params);
}

#[test]
fn command_exec_terminate_round_trips() {
    let params = CommandExecTerminateParams {
        process_id: "proc-8".to_string(),
    };

    let value = serde_json::to_value(&params).expect("serialize command/exec/terminate params");
    assert_eq!(
        value,
        json!({
            "processId": "proc-8",
        })
    );

    let decoded = serde_json::from_value::<CommandExecTerminateParams>(value)
        .expect("deserialize round-trip");
    assert_eq!(decoded, params);
}

#[test]
fn command_exec_params_round_trip_with_size() {
    let params = CommandExecParams {
        command: vec!["top".to_string()],
        process_id: Some("pty-1".to_string()),
        tty: true,
        stream_stdin: false,
        stream_stdout_stderr: false,
        output_bytes_cap: None,
        disable_output_cap: false,
        disable_timeout: false,
        timeout_ms: None,
        cwd: None,
        env: None,
        size: Some(CommandExecTerminalSize {
            rows: 40,
            cols: 120,
        }),
        sandbox_policy: None,
        permission_profile: None,
    };

    let value = serde_json::to_value(&params).expect("serialize command/exec params");
    assert_eq!(
        value,
        json!({
            "command": ["top"],
            "processId": "pty-1",
            "tty": true,
            "outputBytesCap": null,
            "timeoutMs": null,
            "cwd": null,
            "env": null,
            "size": {
                "rows": 40,
                "cols": 120,
            },
            "sandboxPolicy": null,
            "permissionProfile": null,
        })
    );

    let decoded =
        serde_json::from_value::<CommandExecParams>(value).expect("deserialize round-trip");
    assert_eq!(decoded, params);
}

#[test]
fn command_exec_resize_round_trips() {
    let params = CommandExecResizeParams {
        process_id: "proc-9".to_string(),
        size: CommandExecTerminalSize {
            rows: 50,
            cols: 160,
        },
    };

    let value = serde_json::to_value(&params).expect("serialize command/exec/resize params");
    assert_eq!(
        value,
        json!({
            "processId": "proc-9",
            "size": {
                "rows": 50,
                "cols": 160,
            },
        })
    );

    let decoded =
        serde_json::from_value::<CommandExecResizeParams>(value).expect("deserialize round-trip");
    assert_eq!(decoded, params);
}

#[test]
fn command_exec_output_delta_round_trips() {
    let notification = CommandExecOutputDeltaNotification {
        process_id: "proc-1".to_string(),
        stream: CommandExecOutputStream::Stdout,
        delta_base64: "AQI=".to_string(),
        cap_reached: false,
    };

    let value = serde_json::to_value(&notification)
        .expect("serialize command/exec/outputDelta notification");
    assert_eq!(
        value,
        json!({
            "processId": "proc-1",
            "stream": "stdout",
            "deltaBase64": "AQI=",
            "capReached": false,
        })
    );

    let decoded = serde_json::from_value::<CommandExecOutputDeltaNotification>(value)
        .expect("deserialize round-trip");
    assert_eq!(decoded, notification);
}

#[test]
fn process_control_params_round_trip() {
    let write = ProcessWriteStdinParams {
        process_handle: "proc-7".to_string(),
        delta_base64: None,
        close_stdin: true,
    };
    let value = serde_json::to_value(&write).expect("serialize process/writeStdin params");
    assert_eq!(
        value,
        json!({
            "processHandle": "proc-7",
            "deltaBase64": null,
            "closeStdin": true,
        })
    );
    let decoded = serde_json::from_value::<ProcessWriteStdinParams>(value)
        .expect("deserialize process/writeStdin params");
    assert_eq!(decoded, write);

    let resize = ProcessResizePtyParams {
        process_handle: "proc-7".to_string(),
        size: ProcessTerminalSize {
            rows: 50,
            cols: 160,
        },
    };
    let value = serde_json::to_value(&resize).expect("serialize process/resizePty params");
    assert_eq!(
        value,
        json!({
            "processHandle": "proc-7",
            "size": {
                "rows": 50,
                "cols": 160,
            },
        })
    );
    let decoded = serde_json::from_value::<ProcessResizePtyParams>(value)
        .expect("deserialize process/resizePty params");
    assert_eq!(decoded, resize);

    let kill = ProcessKillParams {
        process_handle: "proc-7".to_string(),
    };
    let value = serde_json::to_value(&kill).expect("serialize process/kill params");
    assert_eq!(
        value,
        json!({
            "processHandle": "proc-7",
        })
    );
    let decoded =
        serde_json::from_value::<ProcessKillParams>(value).expect("deserialize process/kill");
    assert_eq!(decoded, kill);
}

#[test]
fn process_notifications_round_trip() {
    let delta = ProcessOutputDeltaNotification {
        process_handle: "proc-1".to_string(),
        stream: ProcessOutputStream::Stdout,
        delta_base64: "AQI=".to_string(),
        cap_reached: false,
    };
    let value = serde_json::to_value(&delta).expect("serialize process/outputDelta");
    assert_eq!(
        value,
        json!({
            "processHandle": "proc-1",
            "stream": "stdout",
            "deltaBase64": "AQI=",
            "capReached": false,
        })
    );
    let decoded = serde_json::from_value::<ProcessOutputDeltaNotification>(value)
        .expect("deserialize process/outputDelta");
    assert_eq!(decoded, delta);

    let exited = ProcessExitedNotification {
        process_handle: "proc-1".to_string(),
        exit_code: 0,
        stdout: "out".to_string(),
        stdout_cap_reached: false,
        stderr: "err".to_string(),
        stderr_cap_reached: true,
    };
    let value = serde_json::to_value(&exited).expect("serialize process/exited");
    assert_eq!(
        value,
        json!({
            "processHandle": "proc-1",
            "exitCode": 0,
            "stdout": "out",
            "stdoutCapReached": false,
            "stderr": "err",
            "stderrCapReached": true,
        })
    );
    let decoded = serde_json::from_value::<ProcessExitedNotification>(value)
        .expect("deserialize process/exited");
    assert_eq!(decoded, exited);
}

#[test]
fn command_execution_output_delta_round_trips() {
    let notification = CommandExecutionOutputDeltaNotification {
        thread_id: "thread-1".to_string(),
        turn_id: "turn-1".to_string(),
        item_id: "item-1".to_string(),
        delta: "\u{fffd}a\n".to_string(),
    };

    let value = serde_json::to_value(&notification)
        .expect("serialize item/commandExecution/outputDelta notification");
    assert_eq!(
        value,
        json!({
            "threadId": "thread-1",
            "turnId": "turn-1",
            "itemId": "item-1",
            "delta": "\u{fffd}a\n",
        })
    );

    let decoded = serde_json::from_value::<CommandExecutionOutputDeltaNotification>(value)
        .expect("deserialize round-trip");
    assert_eq!(decoded, notification);
}

#[test]
fn sandbox_policy_round_trips_external_sandbox_network_access() {
    let v2_policy = SandboxPolicy::ExternalSandbox {
        network_access: NetworkAccess::Enabled,
    };

    let core_policy = v2_policy.to_core();
    assert_eq!(
        core_policy,
        codex_protocol::protocol::SandboxPolicy::ExternalSandbox {
            network_access: CoreNetworkAccess::Enabled,
        }
    );

    let back_to_v2 = SandboxPolicy::from(core_policy);
    assert_eq!(back_to_v2, v2_policy);
}

#[test]
fn sandbox_policy_round_trips_read_only_network_access() {
    let v2_policy = SandboxPolicy::ReadOnly {
        network_access: true,
    };

    let core_policy = v2_policy.to_core();
    assert_eq!(
        core_policy,
        codex_protocol::protocol::SandboxPolicy::ReadOnly {
            network_access: true,
        }
    );

    let back_to_v2 = SandboxPolicy::from(core_policy);
    assert_eq!(back_to_v2, v2_policy);
}

#[test]
fn ask_for_approval_granular_round_trips_request_permissions_flag() {
    let v2_policy = AskForApproval::Granular {
        sandbox_approval: true,
        rules: false,
        skill_approval: false,
        request_permissions: true,
        mcp_elicitations: false,
    };

    let core_policy = v2_policy.to_core();
    assert_eq!(
        core_policy,
        CoreAskForApproval::Granular(CoreGranularApprovalConfig {
            sandbox_approval: true,
            rules: false,
            skill_approval: false,
            request_permissions: true,
            mcp_elicitations: false,
        })
    );

    let back_to_v2 = AskForApproval::from(core_policy);
    assert_eq!(back_to_v2, v2_policy);
}

#[test]
fn ask_for_approval_granular_defaults_missing_optional_flags_to_false() {
    let decoded = serde_json::from_value::<AskForApproval>(serde_json::json!({
        "granular": {
            "sandbox_approval": true,
            "rules": false,
            "mcp_elicitations": true,
        }
    }))
    .expect("granular approval policy should deserialize");

    assert_eq!(
        decoded,
        AskForApproval::Granular {
            sandbox_approval: true,
            rules: false,
            skill_approval: false,
            request_permissions: false,
            mcp_elicitations: true,
        }
    );
}

#[test]
fn ask_for_approval_granular_is_marked_experimental() {
    let reason =
        crate::experimental_api::ExperimentalApi::experimental_reason(&AskForApproval::Granular {
            sandbox_approval: true,
            rules: false,
            skill_approval: false,
            request_permissions: false,
            mcp_elicitations: true,
        });

    assert_eq!(reason, Some("askForApproval.granular"));
    assert_eq!(
        crate::experimental_api::ExperimentalApi::experimental_reason(&AskForApproval::OnRequest,),
        None
    );
}

#[test]
fn profile_v2_granular_approval_policy_is_marked_experimental() {
    let reason = crate::experimental_api::ExperimentalApi::experimental_reason(&ProfileV2 {
        model: None,
        model_provider: None,
        approval_policy: Some(AskForApproval::Granular {
            sandbox_approval: true,
            rules: false,
            skill_approval: false,
            request_permissions: true,
            mcp_elicitations: false,
        }),
        approvals_reviewer: None,
        service_tier: None,
        model_reasoning_effort: None,
        model_reasoning_summary: None,
        model_verbosity: None,
        web_search: None,
        tools: None,
        chatgpt_base_url: None,
        additional: HashMap::new(),
    });

    assert_eq!(reason, Some("askForApproval.granular"));
}

#[test]
fn config_granular_approval_policy_is_marked_experimental() {
    let reason = crate::experimental_api::ExperimentalApi::experimental_reason(&Config {
        model: None,
        review_model: None,
        model_context_window: None,
        model_auto_compact_token_limit: None,
        model_provider: None,
        approval_policy: Some(AskForApproval::Granular {
            sandbox_approval: false,
            rules: true,
            skill_approval: false,
            request_permissions: false,
            mcp_elicitations: true,
        }),
        approvals_reviewer: None,
        sandbox_mode: None,
        sandbox_workspace_write: None,
        forced_chatgpt_workspace_id: None,
        forced_login_method: None,
        web_search: None,
        tools: None,
        profile: None,
        profiles: HashMap::new(),
        instructions: None,
        developer_instructions: None,
        compact_prompt: None,
        model_reasoning_effort: None,
        model_reasoning_summary: None,
        model_verbosity: None,
        service_tier: None,
        analytics: None,
        apps: None,
        additional: HashMap::new(),
    });

    assert_eq!(reason, Some("askForApproval.granular"));
}

#[test]
fn config_approvals_reviewer_is_marked_experimental() {
    let reason = crate::experimental_api::ExperimentalApi::experimental_reason(&Config {
        model: None,
        review_model: None,
        model_context_window: None,
        model_auto_compact_token_limit: None,
        model_provider: None,
        approval_policy: None,
        approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
        sandbox_mode: None,
        sandbox_workspace_write: None,
        forced_chatgpt_workspace_id: None,
        forced_login_method: None,
        web_search: None,
        tools: None,
        profile: None,
        profiles: HashMap::new(),
        instructions: None,
        developer_instructions: None,
        compact_prompt: None,
        model_reasoning_effort: None,
        model_reasoning_summary: None,
        model_verbosity: None,
        service_tier: None,
        analytics: None,
        apps: None,
        additional: HashMap::new(),
    });

    assert_eq!(reason, Some("config/read.approvalsReviewer"));
}

#[test]
fn config_nested_profile_granular_approval_policy_is_marked_experimental() {
    let reason = crate::experimental_api::ExperimentalApi::experimental_reason(&Config {
        model: None,
        review_model: None,
        model_context_window: None,
        model_auto_compact_token_limit: None,
        model_provider: None,
        approval_policy: None,
        approvals_reviewer: None,
        sandbox_mode: None,
        sandbox_workspace_write: None,
        forced_chatgpt_workspace_id: None,
        forced_login_method: None,
        web_search: None,
        tools: None,
        profile: None,
        profiles: HashMap::from([(
            "default".to_string(),
            ProfileV2 {
                model: None,
                model_provider: None,
                approval_policy: Some(AskForApproval::Granular {
                    sandbox_approval: true,
                    rules: false,
                    skill_approval: false,
                    request_permissions: false,
                    mcp_elicitations: true,
                }),
                approvals_reviewer: None,
                service_tier: None,
                model_reasoning_effort: None,
                model_reasoning_summary: None,
                model_verbosity: None,
                web_search: None,
                tools: None,
                chatgpt_base_url: None,
                additional: HashMap::new(),
            },
        )]),
        instructions: None,
        developer_instructions: None,
        compact_prompt: None,
        model_reasoning_effort: None,
        model_reasoning_summary: None,
        model_verbosity: None,
        service_tier: None,
        analytics: None,
        apps: None,
        additional: HashMap::new(),
    });

    assert_eq!(reason, Some("askForApproval.granular"));
}

#[test]
fn config_nested_profile_approvals_reviewer_is_marked_experimental() {
    let reason = crate::experimental_api::ExperimentalApi::experimental_reason(&Config {
        model: None,
        review_model: None,
        model_context_window: None,
        model_auto_compact_token_limit: None,
        model_provider: None,
        approval_policy: None,
        approvals_reviewer: None,
        sandbox_mode: None,
        sandbox_workspace_write: None,
        forced_chatgpt_workspace_id: None,
        forced_login_method: None,
        web_search: None,
        tools: None,
        profile: None,
        profiles: HashMap::from([(
            "default".to_string(),
            ProfileV2 {
                model: None,
                model_provider: None,
                approval_policy: None,
                approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
                service_tier: None,
                model_reasoning_effort: None,
                model_reasoning_summary: None,
                model_verbosity: None,
                web_search: None,
                tools: None,
                chatgpt_base_url: None,
                additional: HashMap::new(),
            },
        )]),
        instructions: None,
        developer_instructions: None,
        compact_prompt: None,
        model_reasoning_effort: None,
        model_reasoning_summary: None,
        model_verbosity: None,
        service_tier: None,
        analytics: None,
        apps: None,
        additional: HashMap::new(),
    });

    assert_eq!(reason, Some("config/read.approvalsReviewer"));
}

#[test]
fn config_requirements_granular_allowed_approval_policy_is_marked_experimental() {
    let reason =
        crate::experimental_api::ExperimentalApi::experimental_reason(&ConfigRequirements {
            allowed_approval_policies: Some(vec![AskForApproval::Granular {
                sandbox_approval: true,
                rules: true,
                skill_approval: false,
                request_permissions: false,
                mcp_elicitations: false,
            }]),
            allowed_approvals_reviewers: None,
            allowed_sandbox_modes: None,
            allowed_web_search_modes: None,
            feature_requirements: None,
            hooks: None,
            enforce_residency: None,
            network: None,
        });

    assert_eq!(reason, Some("askForApproval.granular"));
}

#[test]
fn client_request_thread_start_granular_approval_policy_is_marked_experimental() {
    let reason = crate::experimental_api::ExperimentalApi::experimental_reason(
        &crate::ClientRequest::ThreadStart {
            request_id: crate::RequestId::Integer(1),
            params: ThreadStartParams {
                approval_policy: Some(AskForApproval::Granular {
                    sandbox_approval: true,
                    rules: false,
                    skill_approval: false,
                    request_permissions: true,
                    mcp_elicitations: false,
                }),
                ..Default::default()
            },
        },
    );

    assert_eq!(reason, Some("askForApproval.granular"));
}

#[test]
fn client_request_thread_resume_granular_approval_policy_is_marked_experimental() {
    let reason = crate::experimental_api::ExperimentalApi::experimental_reason(
        &crate::ClientRequest::ThreadResume {
            request_id: crate::RequestId::Integer(2),
            params: ThreadResumeParams {
                thread_id: "thr_123".to_string(),
                approval_policy: Some(AskForApproval::Granular {
                    sandbox_approval: false,
                    rules: true,
                    skill_approval: false,
                    request_permissions: false,
                    mcp_elicitations: true,
                }),
                ..Default::default()
            },
        },
    );

    assert_eq!(reason, Some("askForApproval.granular"));
}

#[test]
fn client_request_thread_fork_granular_approval_policy_is_marked_experimental() {
    let reason = crate::experimental_api::ExperimentalApi::experimental_reason(
        &crate::ClientRequest::ThreadFork {
            request_id: crate::RequestId::Integer(3),
            params: ThreadForkParams {
                thread_id: "thr_456".to_string(),
                approval_policy: Some(AskForApproval::Granular {
                    sandbox_approval: true,
                    rules: false,
                    skill_approval: false,
                    request_permissions: false,
                    mcp_elicitations: true,
                }),
                ..Default::default()
            },
        },
    );

    assert_eq!(reason, Some("askForApproval.granular"));
}

#[test]
fn client_request_turn_start_granular_approval_policy_is_marked_experimental() {
    let reason = crate::experimental_api::ExperimentalApi::experimental_reason(
        &crate::ClientRequest::TurnStart {
            request_id: crate::RequestId::Integer(4),
            params: TurnStartParams {
                thread_id: "thr_123".to_string(),
                input: Vec::new(),
                approval_policy: Some(AskForApproval::Granular {
                    sandbox_approval: false,
                    rules: true,
                    skill_approval: false,
                    request_permissions: false,
                    mcp_elicitations: true,
                }),
                ..Default::default()
            },
        },
    );

    assert_eq!(reason, Some("askForApproval.granular"));
}

#[test]
fn mcp_server_elicitation_response_round_trips_rmcp_result() {
    let rmcp_result = rmcp::model::CreateElicitationResult {
        action: rmcp::model::ElicitationAction::Accept,
        content: Some(json!({
            "confirmed": true,
        })),
    };

    let v2_response = McpServerElicitationRequestResponse::from(rmcp_result.clone());
    assert_eq!(
        v2_response,
        McpServerElicitationRequestResponse {
            action: McpServerElicitationAction::Accept,
            content: Some(json!({
                "confirmed": true,
            })),
            meta: None,
        }
    );
    assert_eq!(
        rmcp::model::CreateElicitationResult::from(v2_response),
        rmcp_result
    );
}

#[test]
fn mcp_server_elicitation_request_from_core_url_request() {
    let request = McpServerElicitationRequest::try_from(CoreElicitationRequest::Url {
        meta: None,
        message: "Finish sign-in".to_string(),
        url: "https://example.com/complete".to_string(),
        elicitation_id: "elicitation-123".to_string(),
    })
    .expect("URL request should convert");

    assert_eq!(
        request,
        McpServerElicitationRequest::Url {
            meta: None,
            message: "Finish sign-in".to_string(),
            url: "https://example.com/complete".to_string(),
            elicitation_id: "elicitation-123".to_string(),
        }
    );
}

#[test]
fn mcp_server_elicitation_request_from_core_form_request() {
    let request = McpServerElicitationRequest::try_from(CoreElicitationRequest::Form {
        meta: None,
        message: "Allow this request?".to_string(),
        requested_schema: json!({
            "type": "object",
            "properties": {
                "confirmed": {
                    "type": "boolean",
                }
            },
            "required": ["confirmed"],
        }),
    })
    .expect("form request should convert");

    let expected_schema: McpElicitationSchema = serde_json::from_value(json!({
        "type": "object",
        "properties": {
            "confirmed": {
                "type": "boolean",
            }
        },
        "required": ["confirmed"],
    }))
    .expect("expected schema should deserialize");

    assert_eq!(
        request,
        McpServerElicitationRequest::Form {
            meta: None,
            message: "Allow this request?".to_string(),
            requested_schema: expected_schema,
        }
    );
}

#[test]
fn mcp_elicitation_schema_matches_mcp_2025_11_25_primitives() {
    let schema: McpElicitationSchema = serde_json::from_value(json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "properties": {
            "email": {
                "type": "string",
                "title": "Email",
                "description": "Work email address",
                "format": "email",
                "default": "dev@example.com",
            },
            "count": {
                "type": "integer",
                "title": "Count",
                "description": "How many items to create",
                "minimum": 1,
                "maximum": 5,
                "default": 3,
            },
            "confirmed": {
                "type": "boolean",
                "title": "Confirm",
                "description": "Approve the pending action",
                "default": true,
            },
            "legacyChoice": {
                "type": "string",
                "title": "Action",
                "description": "Legacy titled enum form",
                "enum": ["allow", "deny"],
                "enumNames": ["Allow", "Deny"],
                "default": "allow",
            },
        },
        "required": ["email", "confirmed"],
    }))
    .expect("schema should deserialize");

    assert_eq!(
        schema,
        McpElicitationSchema {
            schema_uri: Some("https://json-schema.org/draft/2020-12/schema".to_string()),
            type_: McpElicitationObjectType::Object,
            properties: BTreeMap::from([
                (
                    "confirmed".to_string(),
                    McpElicitationPrimitiveSchema::Boolean(McpElicitationBooleanSchema {
                        type_: McpElicitationBooleanType::Boolean,
                        title: Some("Confirm".to_string()),
                        description: Some("Approve the pending action".to_string()),
                        default: Some(true),
                    }),
                ),
                (
                    "count".to_string(),
                    McpElicitationPrimitiveSchema::Number(McpElicitationNumberSchema {
                        type_: McpElicitationNumberType::Integer,
                        title: Some("Count".to_string()),
                        description: Some("How many items to create".to_string()),
                        minimum: Some(1.0),
                        maximum: Some(5.0),
                        default: Some(3.0),
                    }),
                ),
                (
                    "email".to_string(),
                    McpElicitationPrimitiveSchema::String(McpElicitationStringSchema {
                        type_: McpElicitationStringType::String,
                        title: Some("Email".to_string()),
                        description: Some("Work email address".to_string()),
                        min_length: None,
                        max_length: None,
                        format: Some(McpElicitationStringFormat::Email),
                        default: Some("dev@example.com".to_string()),
                    }),
                ),
                (
                    "legacyChoice".to_string(),
                    McpElicitationPrimitiveSchema::Enum(McpElicitationEnumSchema::Legacy(
                        McpElicitationLegacyTitledEnumSchema {
                            type_: McpElicitationStringType::String,
                            title: Some("Action".to_string()),
                            description: Some("Legacy titled enum form".to_string()),
                            enum_: vec!["allow".to_string(), "deny".to_string()],
                            enum_names: Some(vec!["Allow".to_string(), "Deny".to_string(),]),
                            default: Some("allow".to_string()),
                        },
                    )),
                ),
            ]),
            required: Some(vec!["email".to_string(), "confirmed".to_string()]),
        }
    );
}

#[test]
fn mcp_server_elicitation_request_rejects_null_core_form_schema() {
    let result = McpServerElicitationRequest::try_from(CoreElicitationRequest::Form {
        meta: Some(json!({
            "persist": "session",
        })),
        message: "Allow this request?".to_string(),
        requested_schema: JsonValue::Null,
    });

    assert!(result.is_err());
}

#[test]
fn mcp_server_elicitation_request_rejects_invalid_core_form_schema() {
    let result = McpServerElicitationRequest::try_from(CoreElicitationRequest::Form {
        meta: None,
        message: "Allow this request?".to_string(),
        requested_schema: json!({
            "type": "object",
            "properties": {
                "confirmed": {
                    "type": "object",
                }
            },
        }),
    });

    assert!(result.is_err());
}

#[test]
fn mcp_server_elicitation_response_serializes_nullable_content() {
    let response = McpServerElicitationRequestResponse {
        action: McpServerElicitationAction::Decline,
        content: None,
        meta: None,
    };

    assert_eq!(
        serde_json::to_value(response).expect("response should serialize"),
        json!({
            "action": "decline",
            "content": null,
            "_meta": null,
        })
    );
}

#[test]
fn sandbox_policy_round_trips_workspace_write_access() {
    let v2_policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        network_access: true,
        exclude_tmpdir_env_var: false,
        exclude_slash_tmp: false,
    };

    let core_policy = v2_policy.to_core();
    assert_eq!(
        core_policy,
        codex_protocol::protocol::SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            network_access: true,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        }
    );

    let back_to_v2 = SandboxPolicy::from(core_policy);
    assert_eq!(back_to_v2, v2_policy);
}

#[test]
fn sandbox_policy_deserializes_legacy_read_only_full_access_field() {
    let policy = serde_json::from_value::<SandboxPolicy>(json!({
        "type": "readOnly",
        "access": {
            "type": "fullAccess"
        },
        "networkAccess": true
    }))
    .expect("read-only policy should ignore legacy fullAccess field");
    assert_eq!(
        policy,
        SandboxPolicy::ReadOnly {
            network_access: true
        }
    );
}

#[test]
fn sandbox_policy_deserializes_legacy_workspace_write_full_access_field() {
    let writable_root = absolute_path("/workspace");
    let policy = serde_json::from_value::<SandboxPolicy>(json!({
        "type": "workspaceWrite",
        "writableRoots": [writable_root],
        "readOnlyAccess": {
            "type": "fullAccess"
        },
        "networkAccess": true,
        "excludeTmpdirEnvVar": true,
        "excludeSlashTmp": true
    }))
    .expect("workspace-write policy should ignore legacy fullAccess field");
    assert_eq!(
        policy,
        SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![absolute_path("/workspace")],
            network_access: true,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        }
    );
}

#[test]
fn sandbox_policy_rejects_legacy_read_only_restricted_access_field() {
    let err = serde_json::from_value::<SandboxPolicy>(json!({
        "type": "readOnly",
        "access": {
            "type": "restricted",
            "includePlatformDefaults": false,
            "readableRoots": []
        }
    }))
    .expect_err("read-only policy should reject removed restricted access field");
    assert!(err.to_string().contains("readOnly.access"));
}

#[test]
fn sandbox_policy_rejects_legacy_workspace_write_restricted_read_access_field() {
    let err = serde_json::from_value::<SandboxPolicy>(json!({
        "type": "workspaceWrite",
        "writableRoots": [],
        "readOnlyAccess": {
            "type": "restricted",
            "includePlatformDefaults": false,
            "readableRoots": []
        },
        "networkAccess": false,
        "excludeTmpdirEnvVar": false,
        "excludeSlashTmp": false
    }))
    .expect_err("workspace-write policy should reject removed restricted readOnlyAccess field");
    assert!(err.to_string().contains("workspaceWrite.readOnlyAccess"));
}

#[test]
fn automatic_approval_review_deserializes_aborted_status() {
    let review: GuardianApprovalReview = serde_json::from_value(json!({
        "status": "aborted",
        "riskLevel": null,
        "userAuthorization": null,
        "rationale": null
    }))
    .expect("aborted automatic review should deserialize");
    assert_eq!(
        review,
        GuardianApprovalReview {
            status: GuardianApprovalReviewStatus::Aborted,
            risk_level: None,
            user_authorization: None,
            rationale: None,
        }
    );
}

#[test]
fn guardian_approval_review_action_round_trips_command_shape() {
    let value = json!({
        "type": "command",
        "source": "shell",
        "command": "rm -rf /tmp/example.sqlite",
        "cwd": absolute_path_string("tmp"),
    });
    let action: GuardianApprovalReviewAction =
        serde_json::from_value(value.clone()).expect("guardian review action");

    assert_eq!(
        action,
        GuardianApprovalReviewAction::Command {
            source: GuardianCommandSource::Shell,
            command: "rm -rf /tmp/example.sqlite".to_string(),
            cwd: absolute_path("tmp"),
        }
    );
    assert_eq!(
        serde_json::to_value(&action).expect("serialize guardian review action"),
        value
    );
}

#[test]
fn network_requirements_deserializes_legacy_fields() {
    let requirements: NetworkRequirements = serde_json::from_value(json!({
        "allowedDomains": ["api.openai.com"],
        "deniedDomains": ["blocked.example.com"],
        "allowUnixSockets": ["/tmp/proxy.sock"]
    }))
    .expect("legacy network requirements should deserialize");

    assert_eq!(
        requirements,
        NetworkRequirements {
            enabled: None,
            http_port: None,
            socks_port: None,
            allow_upstream_proxy: None,
            dangerously_allow_non_loopback_proxy: None,
            dangerously_allow_all_unix_sockets: None,
            domains: None,
            managed_allowed_domains_only: None,
            allowed_domains: Some(vec!["api.openai.com".to_string()]),
            denied_domains: Some(vec!["blocked.example.com".to_string()]),
            unix_sockets: None,
            allow_unix_sockets: Some(vec!["/tmp/proxy.sock".to_string()]),
            allow_local_binding: None,
        }
    );
}

#[test]
fn network_requirements_serializes_canonical_and_legacy_fields() {
    let requirements = NetworkRequirements {
        enabled: Some(true),
        http_port: Some(8080),
        socks_port: Some(1080),
        allow_upstream_proxy: Some(false),
        dangerously_allow_non_loopback_proxy: Some(false),
        dangerously_allow_all_unix_sockets: Some(true),
        domains: Some(BTreeMap::from([
            ("api.openai.com".to_string(), NetworkDomainPermission::Allow),
            (
                "blocked.example.com".to_string(),
                NetworkDomainPermission::Deny,
            ),
        ])),
        managed_allowed_domains_only: Some(true),
        allowed_domains: Some(vec!["api.openai.com".to_string()]),
        denied_domains: Some(vec!["blocked.example.com".to_string()]),
        unix_sockets: Some(BTreeMap::from([
            (
                "/tmp/proxy.sock".to_string(),
                NetworkUnixSocketPermission::Allow,
            ),
            (
                "/tmp/ignored.sock".to_string(),
                NetworkUnixSocketPermission::None,
            ),
        ])),
        allow_unix_sockets: Some(vec!["/tmp/proxy.sock".to_string()]),
        allow_local_binding: Some(true),
    };

    assert_eq!(
        serde_json::to_value(requirements).expect("network requirements should serialize"),
        json!({
            "enabled": true,
            "httpPort": 8080,
            "socksPort": 1080,
            "allowUpstreamProxy": false,
            "dangerouslyAllowNonLoopbackProxy": false,
            "dangerouslyAllowAllUnixSockets": true,
            "domains": {
                "api.openai.com": "allow",
                "blocked.example.com": "deny"
            },
            "managedAllowedDomainsOnly": true,
            "allowedDomains": ["api.openai.com"],
            "deniedDomains": ["blocked.example.com"],
            "unixSockets": {
                "/tmp/ignored.sock": "none",
                "/tmp/proxy.sock": "allow"
            },
            "allowUnixSockets": ["/tmp/proxy.sock"],
            "allowLocalBinding": true
        })
    );
}

#[test]
fn core_turn_item_into_thread_item_converts_supported_variants() {
    let user_item = TurnItem::UserMessage(UserMessageItem {
        id: "user-1".to_string(),
        content: vec![
            CoreUserInput::Text {
                text: "hello".to_string(),
                text_elements: Vec::new(),
            },
            CoreUserInput::Image {
                image_url: "https://example.com/image.png".to_string(),
            },
            CoreUserInput::LocalImage {
                path: PathBuf::from("local/image.png"),
            },
            CoreUserInput::Skill {
                name: "skill-creator".to_string(),
                path: PathBuf::from("/repo/.codex/skills/skill-creator/SKILL.md"),
            },
            CoreUserInput::Mention {
                name: "Demo App".to_string(),
                path: "app://demo-app".to_string(),
            },
        ],
    });

    assert_eq!(
        ThreadItem::from(user_item),
        ThreadItem::UserMessage {
            id: "user-1".to_string(),
            content: vec![
                UserInput::Text {
                    text: "hello".to_string(),
                    text_elements: Vec::new(),
                },
                UserInput::Image {
                    url: "https://example.com/image.png".to_string(),
                },
                UserInput::LocalImage {
                    path: PathBuf::from("local/image.png"),
                },
                UserInput::Skill {
                    name: "skill-creator".to_string(),
                    path: PathBuf::from("/repo/.codex/skills/skill-creator/SKILL.md"),
                },
                UserInput::Mention {
                    name: "Demo App".to_string(),
                    path: "app://demo-app".to_string(),
                },
            ],
        }
    );

    let agent_item = TurnItem::AgentMessage(AgentMessageItem {
        id: "agent-1".to_string(),
        content: vec![
            AgentMessageContent::Text {
                text: "Hello ".to_string(),
            },
            AgentMessageContent::Text {
                text: "world".to_string(),
            },
        ],
        phase: None,
        memory_citation: None,
    });

    assert_eq!(
        ThreadItem::from(agent_item),
        ThreadItem::AgentMessage {
            id: "agent-1".to_string(),
            text: "Hello world".to_string(),
            phase: None,
            memory_citation: None,
        }
    );

    let agent_item_with_phase = TurnItem::AgentMessage(AgentMessageItem {
        id: "agent-2".to_string(),
        content: vec![AgentMessageContent::Text {
            text: "final".to_string(),
        }],
        phase: Some(MessagePhase::FinalAnswer),
        memory_citation: Some(CoreMemoryCitation {
            entries: vec![CoreMemoryCitationEntry {
                path: "MEMORY.md".to_string(),
                line_start: 1,
                line_end: 2,
                note: "summary".to_string(),
            }],
            rollout_ids: vec!["rollout-1".to_string()],
        }),
    });

    assert_eq!(
        ThreadItem::from(agent_item_with_phase),
        ThreadItem::AgentMessage {
            id: "agent-2".to_string(),
            text: "final".to_string(),
            phase: Some(MessagePhase::FinalAnswer),
            memory_citation: Some(MemoryCitation {
                entries: vec![MemoryCitationEntry {
                    path: "MEMORY.md".to_string(),
                    line_start: 1,
                    line_end: 2,
                    note: "summary".to_string(),
                }],
                thread_ids: vec!["rollout-1".to_string()],
            }),
        }
    );

    let reasoning_item = TurnItem::Reasoning(ReasoningItem {
        id: "reasoning-1".to_string(),
        summary_text: vec!["line one".to_string(), "line two".to_string()],
        raw_content: vec![],
    });

    assert_eq!(
        ThreadItem::from(reasoning_item),
        ThreadItem::Reasoning {
            id: "reasoning-1".to_string(),
            summary: vec!["line one".to_string(), "line two".to_string()],
            content: vec![],
        }
    );

    let search_item = TurnItem::WebSearch(WebSearchItem {
        id: "search-1".to_string(),
        query: "docs".to_string(),
        action: CoreWebSearchAction::Search {
            query: Some("docs".to_string()),
            queries: None,
        },
    });

    assert_eq!(
        ThreadItem::from(search_item),
        ThreadItem::WebSearch {
            id: "search-1".to_string(),
            query: "docs".to_string(),
            action: Some(WebSearchAction::Search {
                query: Some("docs".to_string()),
                queries: None,
            }),
        }
    );

    let image_view_item = TurnItem::ImageView(ImageViewItem {
        id: "view-image-1".to_string(),
        path: test_path_buf("/tmp/view-image.png").abs(),
    });

    assert_eq!(
        ThreadItem::from(image_view_item),
        ThreadItem::ImageView {
            id: "view-image-1".to_string(),
            path: test_path_buf("/tmp/view-image.png").abs(),
        }
    );

    let file_change_item = TurnItem::FileChange(FileChangeItem {
        id: "patch-1".to_string(),
        changes: [(
            PathBuf::from("README.md"),
            codex_protocol::protocol::FileChange::Add {
                content: "hello\n".to_string(),
            },
        )]
        .into_iter()
        .collect(),
        status: Some(codex_protocol::protocol::PatchApplyStatus::Completed),
        auto_approved: None,
        stdout: Some("Done!".to_string()),
        stderr: Some(String::new()),
    });

    assert_eq!(
        ThreadItem::from(file_change_item),
        ThreadItem::FileChange {
            id: "patch-1".to_string(),
            changes: vec![FileUpdateChange {
                path: "README.md".to_string(),
                kind: PatchChangeKind::Add,
                diff: "hello\n".to_string(),
            }],
            status: PatchApplyStatus::Completed,
        }
    );

    let mcp_tool_call_item = TurnItem::McpToolCall(McpToolCallItem {
        id: "mcp-1".to_string(),
        server: "server".to_string(),
        tool: "tool".to_string(),
        arguments: json!({"arg": "value"}),
        mcp_app_resource_uri: Some("app://connector".to_string()),
        status: CoreMcpToolCallStatus::InProgress,
        result: None,
        error: None,
        duration: None,
    });

    assert_eq!(
        ThreadItem::from(mcp_tool_call_item),
        ThreadItem::McpToolCall {
            id: "mcp-1".to_string(),
            server: "server".to_string(),
            tool: "tool".to_string(),
            status: McpToolCallStatus::InProgress,
            arguments: json!({"arg": "value"}),
            mcp_app_resource_uri: Some("app://connector".to_string()),
            result: None,
            error: None,
            duration_ms: None,
        }
    );

    let completed_mcp_tool_call_item = TurnItem::McpToolCall(McpToolCallItem {
        id: "mcp-2".to_string(),
        server: "server".to_string(),
        tool: "tool".to_string(),
        arguments: JsonValue::Null,
        mcp_app_resource_uri: None,
        status: CoreMcpToolCallStatus::Completed,
        result: Some(CallToolResult {
            content: vec![json!({"type": "text", "text": "ok"})],
            structured_content: Some(json!({"ok": true})),
            is_error: Some(false),
            meta: Some(json!({"trace": "1"})),
        }),
        error: None,
        duration: Some(Duration::from_millis(42)),
    });

    assert_eq!(
        ThreadItem::from(completed_mcp_tool_call_item),
        ThreadItem::McpToolCall {
            id: "mcp-2".to_string(),
            server: "server".to_string(),
            tool: "tool".to_string(),
            status: McpToolCallStatus::Completed,
            arguments: JsonValue::Null,
            mcp_app_resource_uri: None,
            result: Some(Box::new(McpToolCallResult {
                content: vec![json!({"type": "text", "text": "ok"})],
                structured_content: Some(json!({"ok": true})),
                meta: Some(json!({"trace": "1"})),
            })),
            error: None,
            duration_ms: Some(42),
        }
    );
}

#[test]
fn skills_list_params_serialization_uses_force_reload() {
    assert_eq!(
        serde_json::to_value(SkillsListParams {
            cwds: Vec::new(),
            force_reload: false,
            per_cwd_extra_user_roots: None,
        })
        .unwrap(),
        json!({
            "perCwdExtraUserRoots": null,
        }),
    );

    assert_eq!(
        serde_json::to_value(SkillsListParams {
            cwds: vec![PathBuf::from("/repo")],
            force_reload: true,
            per_cwd_extra_user_roots: Some(vec![SkillsListExtraRootsForCwd {
                cwd: PathBuf::from("/repo"),
                extra_user_roots: vec![PathBuf::from("/shared/skills"), PathBuf::from("/tmp/x")],
            }]),
        })
        .unwrap(),
        json!({
            "cwds": ["/repo"],
            "forceReload": true,
            "perCwdExtraUserRoots": [
                {
                    "cwd": "/repo",
                    "extraUserRoots": ["/shared/skills", "/tmp/x"],
                }
            ],
        }),
    );
}

#[test]
fn plugin_source_serializes_local_git_and_remote_variants() {
    let local_path = if cfg!(windows) {
        r"C:\plugins\linear"
    } else {
        "/plugins/linear"
    };
    let local_path = AbsolutePathBuf::try_from(PathBuf::from(local_path)).unwrap();
    let local_path_json = local_path.as_path().display().to_string();

    assert_eq!(
        serde_json::to_value(PluginSource::Local { path: local_path }).unwrap(),
        json!({
            "type": "local",
            "path": local_path_json,
        }),
    );

    assert_eq!(
        serde_json::to_value(PluginSource::Git {
            url: "https://github.com/openai/example.git".to_string(),
            path: Some("plugins/example".to_string()),
            ref_name: Some("main".to_string()),
            sha: Some("abc123".to_string()),
        })
        .unwrap(),
        json!({
            "type": "git",
            "url": "https://github.com/openai/example.git",
            "path": "plugins/example",
            "refName": "main",
            "sha": "abc123",
        }),
    );

    assert_eq!(
        serde_json::to_value(PluginSource::Remote).unwrap(),
        json!({
            "type": "remote",
        }),
    );
}

#[test]
fn marketplace_add_params_serialization_uses_optional_ref_name_and_sparse_paths() {
    assert_eq!(
        serde_json::to_value(MarketplaceAddParams {
            source: "owner/repo".to_string(),
            ref_name: None,
            sparse_paths: None,
        })
        .unwrap(),
        json!({
            "source": "owner/repo",
            "refName": null,
            "sparsePaths": null,
        }),
    );

    assert_eq!(
        serde_json::to_value(MarketplaceAddParams {
            source: "owner/repo".to_string(),
            ref_name: Some("main".to_string()),
            sparse_paths: Some(vec!["plugins/foo".to_string()]),
        })
        .unwrap(),
        json!({
            "source": "owner/repo",
            "refName": "main",
            "sparsePaths": ["plugins/foo"],
        }),
    );
}

#[test]
fn marketplace_upgrade_params_serialization_uses_optional_marketplace_name() {
    assert_eq!(
        serde_json::to_value(MarketplaceUpgradeParams {
            marketplace_name: None,
        })
        .unwrap(),
        json!({
            "marketplaceName": null,
        }),
    );

    assert_eq!(
        serde_json::from_value::<MarketplaceUpgradeParams>(json!({})).unwrap(),
        MarketplaceUpgradeParams {
            marketplace_name: None,
        },
    );

    assert_eq!(
        serde_json::to_value(MarketplaceUpgradeParams {
            marketplace_name: Some("debug".to_string()),
        })
        .unwrap(),
        json!({
            "marketplaceName": "debug",
        }),
    );
}

#[test]
fn plugin_marketplace_entry_serializes_remote_only_path_as_null() {
    assert_eq!(
        serde_json::to_value(PluginMarketplaceEntry {
            name: "openai-curated".to_string(),
            path: None,
            interface: None,
            plugins: Vec::new(),
        })
        .unwrap(),
        json!({
            "name": "openai-curated",
            "path": null,
            "interface": null,
            "plugins": [],
        }),
    );
}

#[test]
fn plugin_interface_serializes_local_paths_and_remote_urls_separately() {
    let composer_icon = if cfg!(windows) {
        r"C:\plugins\linear\icon.png"
    } else {
        "/plugins/linear/icon.png"
    };
    let composer_icon = AbsolutePathBuf::try_from(PathBuf::from(composer_icon)).unwrap();
    let composer_icon_json = composer_icon.as_path().display().to_string();

    let interface = PluginInterface {
        display_name: Some("Linear".to_string()),
        short_description: None,
        long_description: None,
        developer_name: None,
        category: Some("Productivity".to_string()),
        capabilities: Vec::new(),
        website_url: None,
        privacy_policy_url: None,
        terms_of_service_url: None,
        default_prompt: None,
        brand_color: None,
        composer_icon: Some(composer_icon),
        composer_icon_url: Some("https://example.com/linear/icon.png".to_string()),
        logo: None,
        logo_url: Some("https://example.com/linear/logo.png".to_string()),
        screenshots: Vec::new(),
        screenshot_urls: vec!["https://example.com/linear/screenshot.png".to_string()],
    };

    assert_eq!(
        serde_json::to_value(interface).unwrap(),
        json!({
            "displayName": "Linear",
            "shortDescription": null,
            "longDescription": null,
            "developerName": null,
            "category": "Productivity",
            "capabilities": [],
            "websiteUrl": null,
            "privacyPolicyUrl": null,
            "termsOfServiceUrl": null,
            "defaultPrompt": null,
            "brandColor": null,
            "composerIcon": composer_icon_json,
            "composerIconUrl": "https://example.com/linear/icon.png",
            "logo": null,
            "logoUrl": "https://example.com/linear/logo.png",
            "screenshots": [],
            "screenshotUrls": ["https://example.com/linear/screenshot.png"],
        }),
    );
}

#[test]
fn plugin_list_params_ignore_removed_force_remote_sync_field() {
    assert_eq!(
        serde_json::from_value::<PluginListParams>(json!({
            "cwds": null,
            "forceRemoteSync": true,
        }))
        .unwrap(),
        PluginListParams { cwds: None },
    );
}

#[test]
fn plugin_read_params_serialization_uses_install_source_fields() {
    let marketplace_path = if cfg!(windows) {
        r"C:\plugins\marketplace.json"
    } else {
        "/plugins/marketplace.json"
    };
    let marketplace_path = AbsolutePathBuf::try_from(PathBuf::from(marketplace_path)).unwrap();
    let marketplace_path_json = marketplace_path.as_path().display().to_string();
    assert_eq!(
        serde_json::to_value(PluginReadParams {
            marketplace_path: Some(marketplace_path.clone()),
            remote_marketplace_name: None,
            plugin_name: "gmail".to_string(),
        })
        .unwrap(),
        json!({
            "marketplacePath": marketplace_path_json,
            "remoteMarketplaceName": null,
            "pluginName": "gmail",
        }),
    );

    assert_eq!(
        serde_json::from_value::<PluginReadParams>(json!({
            "marketplacePath": marketplace_path_json,
            "pluginName": "gmail",
            "forceRemoteSync": true,
        }))
        .unwrap(),
        PluginReadParams {
            marketplace_path: Some(marketplace_path),
            remote_marketplace_name: None,
            plugin_name: "gmail".to_string(),
        },
    );

    assert_eq!(
        serde_json::from_value::<PluginReadParams>(json!({
            "remoteMarketplaceName": "openai-curated",
            "pluginName": "gmail",
        }))
        .unwrap(),
        PluginReadParams {
            marketplace_path: None,
            remote_marketplace_name: Some("openai-curated".to_string()),
            plugin_name: "gmail".to_string(),
        },
    );
}

#[test]
fn plugin_install_params_serialization_omits_force_remote_sync() {
    let marketplace_path = if cfg!(windows) {
        r"C:\plugins\marketplace.json"
    } else {
        "/plugins/marketplace.json"
    };
    let marketplace_path = AbsolutePathBuf::try_from(PathBuf::from(marketplace_path)).unwrap();
    let marketplace_path_json = marketplace_path.as_path().display().to_string();
    assert_eq!(
        serde_json::to_value(PluginInstallParams {
            marketplace_path: Some(marketplace_path.clone()),
            remote_marketplace_name: None,
            plugin_name: "gmail".to_string(),
        })
        .unwrap(),
        json!({
            "marketplacePath": marketplace_path_json,
            "remoteMarketplaceName": null,
            "pluginName": "gmail",
        }),
    );

    assert_eq!(
        serde_json::from_value::<PluginInstallParams>(json!({
            "marketplacePath": marketplace_path_json,
            "pluginName": "gmail",
            "forceRemoteSync": true,
        }))
        .unwrap(),
        PluginInstallParams {
            marketplace_path: Some(marketplace_path),
            remote_marketplace_name: None,
            plugin_name: "gmail".to_string(),
        },
    );

    assert_eq!(
        serde_json::from_value::<PluginInstallParams>(json!({
            "remoteMarketplaceName": "openai-curated",
            "pluginName": "gmail",
            "forceRemoteSync": true,
        }))
        .unwrap(),
        PluginInstallParams {
            marketplace_path: None,
            remote_marketplace_name: Some("openai-curated".to_string()),
            plugin_name: "gmail".to_string(),
        },
    );
}

#[test]
fn plugin_skill_read_params_serialization_uses_remote_plugin_id() {
    assert_eq!(
        serde_json::to_value(PluginSkillReadParams {
            remote_marketplace_name: "chatgpt-global".to_string(),
            remote_plugin_id: "plugins~Plugin_00000000000000000000000000000000".to_string(),
            skill_name: "plan-work".to_string(),
        })
        .unwrap(),
        json!({
            "remoteMarketplaceName": "chatgpt-global",
            "remotePluginId": "plugins~Plugin_00000000000000000000000000000000",
            "skillName": "plan-work",
        }),
    );
}

#[test]
fn plugin_share_params_and_response_serialization_use_camel_case_fields() {
    let plugin_path = if cfg!(windows) {
        r"C:\plugins\gmail"
    } else {
        "/plugins/gmail"
    };
    let plugin_path = AbsolutePathBuf::try_from(PathBuf::from(plugin_path)).unwrap();
    let plugin_path_json = plugin_path.as_path().display().to_string();

    assert_eq!(
        serde_json::to_value(PluginShareSaveParams {
            plugin_path: plugin_path.clone(),
            remote_plugin_id: None,
        })
        .unwrap(),
        json!({
            "pluginPath": plugin_path_json,
            "remotePluginId": null,
        }),
    );

    assert_eq!(
        serde_json::to_value(PluginShareSaveParams {
            plugin_path,
            remote_plugin_id: Some("plugins~Plugin_00000000000000000000000000000000".to_string(),),
        })
        .unwrap(),
        json!({
            "pluginPath": plugin_path_json,
            "remotePluginId": "plugins~Plugin_00000000000000000000000000000000",
        }),
    );

    assert_eq!(
        serde_json::to_value(PluginShareSaveResponse {
            remote_plugin_id: "plugins~Plugin_00000000000000000000000000000000".to_string(),
            share_url: String::new(),
        })
        .unwrap(),
        json!({
            "remotePluginId": "plugins~Plugin_00000000000000000000000000000000",
            "shareUrl": "",
        }),
    );

    assert_eq!(
        serde_json::from_value::<PluginShareListParams>(json!({})).unwrap(),
        PluginShareListParams {},
    );

    assert_eq!(
        serde_json::to_value(PluginShareDeleteParams {
            remote_plugin_id: "plugins~Plugin_00000000000000000000000000000000".to_string(),
        })
        .unwrap(),
        json!({
            "remotePluginId": "plugins~Plugin_00000000000000000000000000000000",
        }),
    );
}

#[test]
fn plugin_share_list_response_serializes_share_items() {
    assert_eq!(
        serde_json::to_value(PluginShareListResponse {
            data: vec![PluginShareListItem {
                plugin: PluginSummary {
                    id: "plugins~Plugin_00000000000000000000000000000000".to_string(),
                    name: "gmail".to_string(),
                    source: PluginSource::Remote,
                    installed: false,
                    enabled: false,
                    install_policy: PluginInstallPolicy::Available,
                    auth_policy: PluginAuthPolicy::OnUse,
                    availability: PluginAvailability::Available,
                    interface: None,
                },
                share_url: "https://chatgpt.example/plugins/share/share-key-1".to_string(),
                local_plugin_path: None,
            }],
        })
        .unwrap(),
        json!({
            "data": [{
                "plugin": {
                    "id": "plugins~Plugin_00000000000000000000000000000000",
                    "name": "gmail",
                    "source": { "type": "remote" },
                    "installed": false,
                    "enabled": false,
                    "installPolicy": "AVAILABLE",
                    "authPolicy": "ON_USE",
                    "availability": "AVAILABLE",
                    "interface": null,
                },
                "shareUrl": "https://chatgpt.example/plugins/share/share-key-1",
                "localPluginPath": null,
            }],
        }),
    );
}

#[test]
fn plugin_summary_defaults_missing_availability_to_available() {
    let summary: PluginSummary = serde_json::from_value(json!({
        "id": "plugins~Plugin_00000000000000000000000000000000",
        "name": "gmail",
        "source": { "type": "remote" },
        "installed": false,
        "enabled": false,
        "installPolicy": "AVAILABLE",
        "authPolicy": "ON_USE",
        "interface": null,
    }))
    .unwrap();

    assert_eq!(summary.availability, PluginAvailability::Available);
}

#[test]
fn plugin_availability_deserializes_enabled_alias() {
    let availability: PluginAvailability = serde_json::from_value(json!("ENABLED")).unwrap();

    assert_eq!(availability, PluginAvailability::Available);
    assert_eq!(
        serde_json::to_value(availability).unwrap(),
        json!("AVAILABLE")
    );
}

#[test]
fn plugin_uninstall_params_serialization_omits_force_remote_sync() {
    assert_eq!(
        serde_json::to_value(PluginUninstallParams {
            plugin_id: "gmail@openai-curated".to_string(),
        })
        .unwrap(),
        json!({
            "pluginId": "gmail@openai-curated",
        }),
    );

    assert_eq!(
        serde_json::from_value::<PluginUninstallParams>(json!({
            "pluginId": "gmail@openai-curated",
            "forceRemoteSync": true,
        }))
        .unwrap(),
        PluginUninstallParams {
            plugin_id: "gmail@openai-curated".to_string(),
        },
    );

    assert_eq!(
        serde_json::to_value(PluginUninstallParams {
            plugin_id: "plugins~Plugin_gmail".to_string(),
        })
        .unwrap(),
        json!({
            "pluginId": "plugins~Plugin_gmail",
        }),
    );

    assert_eq!(
        serde_json::from_value::<PluginUninstallParams>(json!({
            "pluginId": "plugins~Plugin_gmail",
            "forceRemoteSync": true,
        }))
        .unwrap(),
        PluginUninstallParams {
            plugin_id: "plugins~Plugin_gmail".to_string(),
        },
    );
}

#[test]
fn marketplace_remove_response_serializes_nullable_installed_root() {
    let installed_root = if cfg!(windows) {
        r"C:\marketplaces\debug"
    } else {
        "/tmp/marketplaces/debug"
    };
    let installed_root = AbsolutePathBuf::try_from(PathBuf::from(installed_root)).unwrap();
    let installed_root_json = installed_root.as_path().display().to_string();
    assert_eq!(
        serde_json::to_value(MarketplaceRemoveResponse {
            marketplace_name: "debug".to_string(),
            installed_root: Some(installed_root),
        })
        .unwrap(),
        json!({
            "marketplaceName": "debug",
            "installedRoot": installed_root_json,
        }),
    );

    assert_eq!(
        serde_json::to_value(MarketplaceRemoveResponse {
            marketplace_name: "debug".to_string(),
            installed_root: None,
        })
        .unwrap(),
        json!({
            "marketplaceName": "debug",
            "installedRoot": null,
        }),
    );
}

#[test]
fn marketplace_upgrade_response_serializes_camel_case_fields() {
    let upgraded_root = if cfg!(windows) {
        r"C:\marketplaces\debug"
    } else {
        "/tmp/marketplaces/debug"
    };
    let upgraded_root = AbsolutePathBuf::try_from(PathBuf::from(upgraded_root)).unwrap();
    let upgraded_root_json = upgraded_root.as_path().display().to_string();

    assert_eq!(
        serde_json::to_value(MarketplaceUpgradeResponse {
            selected_marketplaces: vec!["debug".to_string()],
            upgraded_roots: vec![upgraded_root],
            errors: vec![MarketplaceUpgradeErrorInfo {
                marketplace_name: "broken".to_string(),
                message: "failed to clone".to_string(),
            }],
        })
        .unwrap(),
        json!({
            "selectedMarketplaces": ["debug"],
            "upgradedRoots": [upgraded_root_json],
            "errors": [{
                "marketplaceName": "broken",
                "message": "failed to clone",
            }],
        }),
    );
}

#[test]
fn codex_error_info_serializes_http_status_code_in_camel_case() {
    let value = CodexErrorInfo::ResponseTooManyFailedAttempts {
        http_status_code: Some(401),
    };

    assert_eq!(
        serde_json::to_value(value).unwrap(),
        json!({
            "responseTooManyFailedAttempts": {
                "httpStatusCode": 401
            }
        })
    );
}

#[test]
fn codex_error_info_serializes_cyber_policy_in_camel_case() {
    assert_eq!(
        serde_json::to_value(CodexErrorInfo::CyberPolicy).unwrap(),
        json!("cyberPolicy")
    );
}

#[test]
fn codex_error_info_serializes_active_turn_not_steerable_turn_kind_in_camel_case() {
    let value = CodexErrorInfo::ActiveTurnNotSteerable {
        turn_kind: NonSteerableTurnKind::Review,
    };

    assert_eq!(
        serde_json::to_value(value).unwrap(),
        json!({
            "activeTurnNotSteerable": {
                "turnKind": "review"
            }
        })
    );
}

#[test]
fn dynamic_tool_response_serializes_content_items() {
    let value = serde_json::to_value(DynamicToolCallResponse {
        content_items: vec![DynamicToolCallOutputContentItem::InputText {
            text: "dynamic-ok".to_string(),
        }],
        success: true,
    })
    .unwrap();

    assert_eq!(
        value,
        json!({
            "contentItems": [
                {
                    "type": "inputText",
                    "text": "dynamic-ok"
                }
            ],
            "success": true,
        })
    );
}

#[test]
fn dynamic_tool_response_serializes_text_and_image_content_items() {
    let value = serde_json::to_value(DynamicToolCallResponse {
        content_items: vec![
            DynamicToolCallOutputContentItem::InputText {
                text: "dynamic-ok".to_string(),
            },
            DynamicToolCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,AAA".to_string(),
            },
        ],
        success: true,
    })
    .unwrap();

    assert_eq!(
        value,
        json!({
            "contentItems": [
                {
                    "type": "inputText",
                    "text": "dynamic-ok"
                },
                {
                    "type": "inputImage",
                    "imageUrl": "data:image/png;base64,AAA"
                }
            ],
            "success": true,
        })
    );
}

#[test]
fn dynamic_tool_spec_deserializes_defer_loading() {
    let value = json!({
        "name": "lookup_ticket",
        "description": "Fetch a ticket",
        "inputSchema": {
            "type": "object",
            "properties": {
                "id": { "type": "string" }
            }
        },
        "deferLoading": true,
    });

    let actual: DynamicToolSpec = serde_json::from_value(value).expect("deserialize");

    assert_eq!(
        actual,
        DynamicToolSpec {
            namespace: None,
            name: "lookup_ticket".to_string(),
            description: "Fetch a ticket".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" }
                }
            }),
            defer_loading: true,
        }
    );
}

#[test]
fn dynamic_tool_spec_legacy_expose_to_context_inverts_to_defer_loading() {
    let value = json!({
        "name": "lookup_ticket",
        "description": "Fetch a ticket",
        "inputSchema": {
            "type": "object",
            "properties": {}
        },
        "exposeToContext": false,
    });

    let actual: DynamicToolSpec = serde_json::from_value(value).expect("deserialize");

    assert!(actual.defer_loading);
}

#[test]
fn thread_start_params_preserve_explicit_null_service_tier() {
    let params: ThreadStartParams =
        serde_json::from_value(json!({ "serviceTier": null })).expect("params should deserialize");
    assert_eq!(params.service_tier, Some(None));

    let serialized = serde_json::to_value(&params).expect("params should serialize");
    assert_eq!(
        serialized.get("serviceTier"),
        Some(&serde_json::Value::Null)
    );

    let serialized_without_override =
        serde_json::to_value(ThreadStartParams::default()).expect("params should serialize");
    assert_eq!(serialized_without_override.get("serviceTier"), None);
}

#[test]
fn thread_lifecycle_responses_default_missing_compat_fields() {
    let response = json!({
        "thread": {
            "id": "thread-id",
            "forkedFromId": null,
            "preview": "",
            "ephemeral": false,
            "modelProvider": "openai",
            "createdAt": 1,
            "updatedAt": 1,
            "status": { "type": "idle" },
            "path": null,
            "cwd": absolute_path_string("tmp"),
            "cliVersion": "0.0.0",
            "source": "exec",
            "agentNickname": null,
            "agentRole": null,
            "gitInfo": null,
            "name": null,
            "turns": []
        },
        "model": "gpt-5",
        "modelProvider": "openai",
        "serviceTier": null,
        "cwd": absolute_path_string("tmp"),
        "approvalPolicy": "on-failure",
        "approvalsReviewer": "user",
        "sandbox": { "type": "dangerFullAccess" },
        "reasoningEffort": null
    });

    let start: ThreadStartResponse =
        serde_json::from_value(response.clone()).expect("thread/start response");
    let resume: ThreadResumeResponse =
        serde_json::from_value(response.clone()).expect("thread/resume response");
    let fork: ThreadForkResponse = serde_json::from_value(response).expect("thread/fork response");

    assert_eq!(start.instruction_sources, Vec::<AbsolutePathBuf>::new());
    assert_eq!(resume.instruction_sources, Vec::<AbsolutePathBuf>::new());
    assert_eq!(fork.instruction_sources, Vec::<AbsolutePathBuf>::new());
    assert_eq!(start.permission_profile, None);
    assert_eq!(resume.permission_profile, None);
    assert_eq!(fork.permission_profile, None);
    assert_eq!(start.active_permission_profile, None);
    assert_eq!(resume.active_permission_profile, None);
    assert_eq!(fork.active_permission_profile, None);
}

#[test]
fn turn_start_params_preserve_explicit_null_service_tier() {
    let params: TurnStartParams = serde_json::from_value(json!({
        "threadId": "thread_123",
        "input": [],
        "serviceTier": null
    }))
    .expect("params should deserialize");
    assert_eq!(params.service_tier, Some(None));

    let serialized = serde_json::to_value(&params).expect("params should serialize");
    assert_eq!(
        serialized.get("serviceTier"),
        Some(&serde_json::Value::Null)
    );

    let without_override = TurnStartParams {
        thread_id: "thread_123".to_string(),
        input: vec![],
        responsesapi_client_metadata: None,
        environments: None,
        cwd: None,
        approval_policy: None,
        approvals_reviewer: None,
        sandbox_policy: None,
        permissions: None,
        model: None,
        service_tier: None,
        effort: None,
        summary: None,
        output_schema: None,
        collaboration_mode: None,
        personality: None,
    };
    let serialized_without_override =
        serde_json::to_value(&without_override).expect("params should serialize");
    assert_eq!(serialized_without_override.get("serviceTier"), None);
}

#[test]
fn turn_start_params_round_trip_environments() {
    let cwd = test_absolute_path();
    let params: TurnStartParams = serde_json::from_value(json!({
        "threadId": "thread_123",
        "input": [],
        "environments": [
            {
                "environmentId": "local",
                "cwd": cwd
            }
        ],
    }))
    .expect("params should deserialize");

    assert_eq!(
        params.environments,
        Some(vec![TurnEnvironmentParams {
            environment_id: "local".to_string(),
            cwd: cwd.clone(),
        }])
    );
    assert_eq!(
        crate::experimental_api::ExperimentalApi::experimental_reason(&params),
        Some("turn/start.environments")
    );

    let serialized = serde_json::to_value(&params).expect("params should serialize");
    assert_eq!(
        serialized.get("environments"),
        Some(&json!([
            {
                "environmentId": "local",
                "cwd": cwd
            }
        ]))
    );
}

#[test]
fn turn_start_params_preserve_empty_environments() {
    let params: TurnStartParams = serde_json::from_value(json!({
        "threadId": "thread_123",
        "input": [],
        "environments": [],
    }))
    .expect("params should deserialize");

    assert_eq!(params.environments, Some(Vec::new()));
    assert_eq!(
        crate::experimental_api::ExperimentalApi::experimental_reason(&params),
        Some("turn/start.environments")
    );

    let serialized = serde_json::to_value(&params).expect("params should serialize");
    assert_eq!(serialized.get("environments"), Some(&json!([])));
}

#[test]
fn turn_start_params_treat_null_or_omitted_environments_as_default() {
    let null_environments: TurnStartParams = serde_json::from_value(json!({
        "threadId": "thread_123",
        "input": [],
        "environments": null,
    }))
    .expect("params should deserialize");
    let omitted_environments: TurnStartParams = serde_json::from_value(json!({
        "threadId": "thread_123",
        "input": [],
    }))
    .expect("params should deserialize");

    assert_eq!(null_environments.environments, None);
    assert_eq!(omitted_environments.environments, None);
    assert_eq!(
        crate::experimental_api::ExperimentalApi::experimental_reason(&null_environments),
        None
    );
    assert_eq!(
        crate::experimental_api::ExperimentalApi::experimental_reason(&omitted_environments),
        None
    );
}

#[test]
fn turn_start_params_reject_relative_environment_cwd() {
    let err = serde_json::from_value::<TurnStartParams>(json!({
        "threadId": "thread_123",
        "input": [],
        "environments": [
            {
                "environmentId": "local",
                "cwd": "relative"
            }
        ],
    }))
    .expect_err("relative environment cwd should fail");

    assert!(
        err.to_string()
            .contains("AbsolutePathBuf deserialized without a base path"),
        "unexpected error: {err}"
    );
}
