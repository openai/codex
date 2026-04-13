use codex_bridge::BridgeField;
use codex_bridge::BridgeMethod;
use codex_bridge::BridgeSchema;
use codex_bridge::BridgeType;
use codex_bridge::OpaqueField;

/// Returns the Rust-authored schema manifest used to generate Python bridge types.
pub fn thread_store_bridge_schema() -> BridgeSchema {
    BridgeSchema {
        namespace: "codex.thread_store.v1".to_string(),
        version: 1,
        types: vec![
            struct_type(
                "OpaqueBytes",
                vec![
                    field("codec", "str"),
                    field("contentType", "str"),
                    field("data", "bytes"),
                ],
            ),
            struct_type(
                "ThreadOwner",
                vec![
                    optional_field("chatgptUserId", "str"),
                    optional_field("accountId", "str"),
                    optional_field("authMode", "str"),
                ],
            ),
            struct_type(
                "RemoteThreadMetadata",
                vec![
                    field("threadId", "str"),
                    optional_field("forkedFromId", "str"),
                    field("owner", "ThreadOwner"),
                    field("preview", "str"),
                    optional_field("name", "str"),
                    field("modelProvider", "str"),
                    optional_field("model", "str"),
                    optional_field("serviceTier", "Any"),
                    optional_field("reasoningEffort", "Any"),
                    field("createdAt", "int"),
                    field("updatedAt", "int"),
                    optional_field("archivedAt", "int"),
                    field("cwd", "str"),
                    field("cliVersion", "str"),
                    field("source", "Any"),
                    optional_field("agentNickname", "str"),
                    optional_field("agentRole", "str"),
                    optional_field("agentPath", "str"),
                    optional_field("gitInfo", "Any"),
                    field("approvalMode", "Any"),
                    field("sandboxPolicy", "Any"),
                    optional_field("tokenUsage", "Any"),
                    optional_field("firstUserMessage", "str"),
                    optional_field("memoryMode", "str"),
                ],
            ),
            struct_type(
                "ThreadIndexPatch",
                vec![
                    field("updatedAt", "int"),
                    optional_field("firstUserMessage", "str"),
                    optional_field("preview", "str"),
                    optional_field("name", "str"),
                    optional_field("tokenUsage", "Any"),
                    optional_field("memoryMode", "str"),
                ],
            ),
            struct_type(
                "CreateThreadRequest",
                vec![
                    field("thread", "RemoteThreadMetadata"),
                    opaque_field("initialItems", "rollout_items", "Vec<RolloutItem>"),
                    field("initialPayloadCodec", "str"),
                    field("eventPersistenceMode", "str"),
                ],
            ),
            struct_type(
                "ResumeThreadRecorderRequest",
                vec![
                    field("threadId", "str"),
                    field("owner", "ThreadOwner"),
                    field("includeArchived", "bool"),
                    field("eventPersistenceMode", "str"),
                ],
            ),
            struct_type(
                "AppendThreadRequest",
                vec![
                    field("threadId", "str"),
                    field("owner", "ThreadOwner"),
                    optional_field("idempotencyKey", "str"),
                    optional_field("updatedAt", "int"),
                    optional_field("newThreadMemoryMode", "str"),
                    optional_field("eventPersistenceMode", "str"),
                    field("indexPatch", "ThreadIndexPatch"),
                    opaque_field("items", "rollout_items", "Vec<RolloutItem>"),
                    field("payloadCodec", "str"),
                ],
            ),
            struct_type(
                "LoadThreadHistoryRequest",
                vec![
                    field("threadId", "str"),
                    field("owner", "ThreadOwner"),
                    field("includeArchived", "bool"),
                ],
            ),
            struct_type(
                "LoadThreadHistoryResponse",
                vec![
                    field("threadId", "str"),
                    field("payloadCodec", "str"),
                    opaque_field("history", "stored_thread_history", "StoredThreadHistory"),
                ],
            ),
            struct_type(
                "ReadThreadRequest",
                vec![
                    field("threadId", "str"),
                    field("owner", "ThreadOwner"),
                    field("includeArchived", "bool"),
                    field("includeHistory", "bool"),
                ],
            ),
            struct_type(
                "ReadThreadResponse",
                vec![
                    field("thread", "RemoteThreadMetadata"),
                    optional_field("historyPayloadCodec", "str"),
                    optional_opaque_field(
                        "history",
                        "stored_thread_history",
                        "StoredThreadHistory",
                    ),
                ],
            ),
            struct_type(
                "ListThreadsRequest",
                vec![
                    field("owner", "ThreadOwner"),
                    field("pageSize", "int"),
                    optional_field("cursor", "str"),
                    field("sortKey", "str"),
                    field("allowedSources", "list[Any]"),
                    optional_field("modelProviders", "list[str]"),
                    field("archived", "bool"),
                    optional_field("cwd", "str"),
                    optional_field("searchTerm", "str"),
                ],
            ),
            struct_type(
                "ListThreadsResponse",
                vec![
                    field("items", "list[RemoteThreadMetadata]"),
                    optional_field("nextCursor", "str"),
                    optional_field("scanned", "int"),
                ],
            ),
            struct_type(
                "FindThreadByNameRequest",
                vec![
                    field("owner", "ThreadOwner"),
                    field("name", "str"),
                    field("includeArchived", "bool"),
                    optional_field("cwd", "str"),
                    field("allowedSources", "list[Any]"),
                    optional_field("modelProviders", "list[str]"),
                ],
            ),
            struct_type(
                "FindThreadByNameResponse",
                vec![optional_field("thread", "RemoteThreadMetadata")],
            ),
            struct_type(
                "SetThreadNameRequest",
                vec![
                    field("threadId", "str"),
                    field("owner", "ThreadOwner"),
                    field("name", "str"),
                ],
            ),
            struct_type(
                "UpdateThreadMetadataRequest",
                vec![
                    field("threadId", "str"),
                    field("owner", "ThreadOwner"),
                    field("patch", "Any"),
                ],
            ),
            struct_type(
                "ArchiveThreadRequest",
                vec![field("threadId", "str"), field("owner", "ThreadOwner")],
            ),
            struct_type(
                "DynamicToolsRequest",
                vec![field("threadId", "str"), field("owner", "ThreadOwner")],
            ),
            struct_type(
                "DynamicToolsResponse",
                vec![optional_field("dynamicTools", "list[Any]")],
            ),
            struct_type(
                "MemoryModeRequest",
                vec![field("threadId", "str"), field("owner", "ThreadOwner")],
            ),
            struct_type(
                "MemoryModeResponse",
                vec![optional_field("memoryMode", "str")],
            ),
            struct_type(
                "SetMemoryModeRequest",
                vec![
                    field("threadId", "str"),
                    field("owner", "ThreadOwner"),
                    field("memoryMode", "str"),
                ],
            ),
            struct_type(
                "ThreadSpawnEdgeRecord",
                vec![
                    field("parentThreadId", "str"),
                    field("childThreadId", "str"),
                    field("status", "str"),
                ],
            ),
            struct_type(
                "ListThreadSpawnEdgesRequest",
                vec![
                    field("threadId", "str"),
                    field("owner", "ThreadOwner"),
                    field("recursive", "bool"),
                    optional_field("status", "str"),
                ],
            ),
            struct_type(
                "ListThreadSpawnEdgesResponse",
                vec![field("edges", "list[ThreadSpawnEdgeRecord]")],
            ),
            struct_type(
                "FindThreadSpawnByPathRequest",
                vec![
                    field("threadId", "str"),
                    field("owner", "ThreadOwner"),
                    field("recursive", "bool"),
                    field("agentPath", "str"),
                ],
            ),
            struct_type(
                "FindThreadSpawnByPathResponse",
                vec![optional_field("threadId", "str")],
            ),
        ],
        methods: vec![
            method("thread_store/create_thread", "CreateThreadRequest", "None"),
            method(
                "thread_store/resume_thread_recorder",
                "ResumeThreadRecorderRequest",
                "None",
            ),
            method(
                "thread_store/append_thread_items",
                "AppendThreadRequest",
                "None",
            ),
            method(
                "thread_store/load_thread_history",
                "LoadThreadHistoryRequest",
                "LoadThreadHistoryResponse",
            ),
            method(
                "thread_store/read_thread",
                "ReadThreadRequest",
                "ReadThreadResponse",
            ),
            method(
                "thread_store/list_threads",
                "ListThreadsRequest",
                "ListThreadsResponse",
            ),
            method(
                "thread_store/find_thread_by_name",
                "FindThreadByNameRequest",
                "FindThreadByNameResponse",
            ),
            method(
                "thread_store/set_thread_name",
                "SetThreadNameRequest",
                "None",
            ),
            method(
                "thread_store/update_thread_metadata",
                "UpdateThreadMetadataRequest",
                "ReadThreadResponse",
            ),
            method(
                "thread_store/archive_thread",
                "ArchiveThreadRequest",
                "None",
            ),
            method(
                "thread_store/unarchive_thread",
                "ArchiveThreadRequest",
                "ReadThreadResponse",
            ),
            method(
                "thread_store/dynamic_tools",
                "DynamicToolsRequest",
                "DynamicToolsResponse",
            ),
            method(
                "thread_store/memory_mode",
                "MemoryModeRequest",
                "MemoryModeResponse",
            ),
            method(
                "thread_store/set_memory_mode",
                "SetMemoryModeRequest",
                "None",
            ),
            method(
                "thread_store/mark_memory_mode_polluted",
                "MemoryModeRequest",
                "None",
            ),
            method(
                "thread_store/upsert_thread_spawn_edge",
                "ThreadSpawnEdgeRecord",
                "None",
            ),
            method(
                "thread_store/list_thread_spawn_edges",
                "ListThreadSpawnEdgesRequest",
                "ListThreadSpawnEdgesResponse",
            ),
            method(
                "thread_store/find_thread_spawn_by_path",
                "FindThreadSpawnByPathRequest",
                "FindThreadSpawnByPathResponse",
            ),
        ],
    }
}

fn struct_type(name: &str, fields: Vec<BridgeField>) -> BridgeType {
    BridgeType {
        name: name.to_string(),
        kind: "struct".to_string(),
        fields,
        variants: Vec::new(),
    }
}

fn field(name: &str, python_type: &str) -> BridgeField {
    BridgeField {
        name: name.to_string(),
        python_type: python_type.to_string(),
        optional: false,
        opaque: None,
    }
}

fn optional_field(name: &str, python_type: &str) -> BridgeField {
    BridgeField {
        name: name.to_string(),
        python_type: python_type.to_string(),
        optional: true,
        opaque: None,
    }
}

fn opaque_field(name: &str, codec: &str, rust_type: &str) -> BridgeField {
    BridgeField {
        name: name.to_string(),
        python_type: "OpaqueBytes".to_string(),
        optional: false,
        opaque: Some(OpaqueField {
            codec: codec.to_string(),
            rust_type: rust_type.to_string(),
        }),
    }
}

fn optional_opaque_field(name: &str, codec: &str, rust_type: &str) -> BridgeField {
    BridgeField {
        name: name.to_string(),
        python_type: "OpaqueBytes".to_string(),
        optional: true,
        opaque: Some(OpaqueField {
            codec: codec.to_string(),
            rust_type: rust_type.to_string(),
        }),
    }
}

fn method(name: &str, request: &str, response: &str) -> BridgeMethod {
    BridgeMethod {
        name: name.to_string(),
        request: request.to_string(),
        response: response.to_string(),
    }
}
