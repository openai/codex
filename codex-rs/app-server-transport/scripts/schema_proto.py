#!/usr/bin/env python3

from collections import OrderedDict
from dataclasses import dataclass
import json
from pathlib import Path
import re
from typing import Any


Json = dict[str, Any]
_ACTIVE_SCHEMA: "SchemaProto | None" = None

RUST_VARIANT_OVERRIDES = {
    ("AskForApproval", "untrusted"): "UnlessTrusted",
    ("AuthMode", "apikey"): "ApiKey",
    ("SessionSource", "vscode"): "VsCode",
    ("ThreadSourceKind", "vscode"): "VsCode",
}

RUST_FIELD_OVERRIDES = {
    "type": "type_",
    "enum": "enum_",
    "const": "const_",
    "oneOf": "one_of",
    "$schema": "schema",
    "async": "r#async",
    "_default": "default",
    "_meta": "meta",
}

RUST_OWNER_FIELD_OVERRIDES = {
    ("McpElicitationSchema", "$schema"): "schema_uri",
    ("SkillToolDependency", "type"): "r#type",
}

RUST_STRING_NEW_TYPES = {
    "AbsolutePathBuf",
    "AgentPath",
    "ReasoningEffort",
    "ThreadId",
}

RUST_TYPE_PATHS = {
    ("legacy", "ParsedCommand"): "codex_protocol::parse_command::ParsedCommand",
    ("v2", "AutoCompactTokenLimitScope"): "codex_protocol::config_types::AutoCompactTokenLimitScope",
    ("v2", "CollaborationMode"): "codex_protocol::config_types::CollaborationMode",
    ("v2", "ContentItem"): "codex_protocol::models::ContentItem",
    ("v2", "ForcedLoginMethod"): "codex_protocol::config_types::ForcedLoginMethod",
    ("v2", "FunctionCallOutputBody"): "codex_protocol::models::FunctionCallOutputBody",
    ("v2", "FunctionCallOutputContentItem"): "codex_protocol::models::FunctionCallOutputContentItem",
    ("v2", "ImageDetail"): "codex_protocol::models::ImageDetail",
    ("v2", "InputModality"): "codex_protocol::openai_models::InputModality",
    ("v2", "LocalShellAction"): "codex_protocol::models::LocalShellAction",
    ("v2", "LocalShellStatus"): "codex_protocol::models::LocalShellStatus",
    ("v2", "McpServerInfo"): "codex_protocol::mcp::McpServerInfo",
    ("v2", "MessagePhase"): "codex_protocol::models::MessagePhase",
    ("v2", "ModeKind"): "codex_protocol::config_types::ModeKind",
    ("v2", "Personality"): "codex_protocol::config_types::Personality",
    ("v2", "PlanType"): "codex_protocol::account::PlanType",
    ("v2", "RealtimeConversationVersion"): "codex_protocol::protocol::RealtimeConversationVersion",
    ("v2", "RealtimeOutputModality"): "codex_protocol::protocol::RealtimeOutputModality",
    ("v2", "RealtimeVoice"): "codex_protocol::protocol::RealtimeVoice",
    ("v2", "RealtimeVoicesList"): "codex_protocol::protocol::RealtimeVoicesList",
    ("v2", "ReasoningEffort"): "codex_protocol::openai_models::ReasoningEffort",
    ("v2", "ReasoningItemContent"): "codex_protocol::models::ReasoningItemContent",
    ("v2", "ReasoningItemReasoningSummary"): "codex_protocol::models::ReasoningItemReasoningSummary",
    ("v2", "ReasoningSummary"): "codex_protocol::config_types::ReasoningSummary",
    ("v2", "Resource"): "codex_protocol::mcp::Resource",
    ("v2", "ResourceContent"): "codex_protocol::mcp::ResourceContent",
    ("v2", "ResourceTemplate"): "codex_protocol::mcp::ResourceTemplate",
    ("v2", "ResponseItem"): "codex_protocol::models::ResponseItem",
    ("v2", "ResponsesApiWebSearchAction"): "codex_protocol::models::WebSearchAction",
    ("v2", "Settings"): "codex_protocol::config_types::Settings",
    ("v2", "SubAgentSource"): "codex_protocol::protocol::SubAgentSource",
    ("v2", "Tool"): "codex_protocol::mcp::Tool",
    ("v2", "Verbosity"): "codex_protocol::config_types::Verbosity",
    ("v2", "WebSearchContextSize"): "codex_protocol::config_types::WebSearchContextSize",
    ("v2", "WebSearchLocation"): "codex_protocol::config_types::WebSearchLocation",
    ("v2", "WebSearchMode"): "codex_protocol::config_types::WebSearchMode",
    ("v2", "WebSearchToolConfig"): "codex_protocol::config_types::WebSearchToolConfig",
    ("legacy", "ReviewDecision"): "codex_protocol::protocol::ReviewDecision",
    ("legacy", "FileChange"): "codex_protocol::protocol::FileChange",
}

FLATTENED_MAP_FIELDS: dict[str, tuple[str, Json]] = {
    "AnalyticsConfig": ("additional", {}),
    "AppToolsConfig": (
        "tools",
        {"$ref": "#/definitions/AppToolConfig"},
    ),
    "AppsConfig": (
        "apps",
        {"$ref": "#/definitions/AppConfig"},
    ),
    "Config": ("additional", {}),
}

EMPTY_STRUCT_VARIANTS = {
    ("ConfiguredHookHandler", "Prompt"),
    ("ConfiguredHookHandler", "Agent"),
}

UNSUPPORTED_SCHEMA_TYPES: set[str] = set()

MANUAL_SCHEMA_TYPES = {
    "Account",
    "ApprovalsReviewer",
    "AuthMode",
    "CollabAgentTool",
    "CollabAgentToolCallStatus",
    "CommandExecutionSource",
    "CommandExecutionStatus",
    "ContentItem",
    "DynamicToolCallStatus",
    "ExecPolicyAmendment",
    "FileSystemSpecialPath",
    "ForcedChatgptWorkspaceIds",
    "FunctionCallOutputBody",
    "FunctionCallOutputContentItem",
    "GetConversationSummaryParams",
    "GetConversationSummaryResponse",
    "HookSource",
    "LocalShellAction",
    "LocalShellStatus",
    "McpToolCallStatus",
    "NetworkPolicyAmendment",
    "NetworkPolicyRuleAction",
    "PatchApplyStatus",
    "PermissionGrantScope",
    "PlanType",
    "PluginAvailability",
    "ReasoningItemContent",
    "ReasoningItemReasoningSummary",
    "RequestId",
    "ResourceContent",
    "ResponseItem",
    "ResponsesApiWebSearchAction",
    "ReviewDecision",
    "SandboxPolicy",
    "TextElement",
    "ThreadId",
    "ThreadItem",
    "ThreadListCwdFilter",
    "TurnItemsView",
}

DOUBLE_OPTION_FIELDS = {
    ("V2CollaborationModeMask", "reasoning_effort"),
    ("V2ThreadForkParams", "serviceTier"),
    ("V2ThreadMetadataGitInfoUpdateParams", "branch"),
    ("V2ThreadMetadataGitInfoUpdateParams", "originUrl"),
    ("V2ThreadMetadataGitInfoUpdateParams", "sha"),
    ("V2ThreadRealtimeStartParams", "prompt"),
    ("V2ThreadResumeParams", "serviceTier"),
    ("V2ThreadSettingsUpdateParams", "serviceTier"),
    ("V2ThreadStartParams", "serviceTier"),
    ("V2TurnStartParams", "serviceTier"),
}

RUST_TRANSPARENT_LIST_FIELDS = {
    (
        "CommandExecutionApprovalDecision",
        "execpolicy_amendment",
    ): "codex_app_server_protocol::ExecPolicyAmendment",
    (
        "CommandExecutionRequestApprovalParams",
        "proposedExecpolicyAmendment",
    ): "codex_app_server_protocol::ExecPolicyAmendment",
}

DIRECT_MESSAGE_FIELDS = {
    ("HookRunSummary", "source"): (
        "codex_app_server_protocol::HookSource",
        "V2HookSource",
        '"unknown".to_owned()',
    ),
    ("PermissionsRequestApprovalResponse", "scope"): (
        "codex_app_server_protocol::PermissionGrantScope",
        "V2PermissionGrantScope",
        '"turn".to_owned()',
    ),
    ("PluginSummary", "availability"): (
        "codex_app_server_protocol::PluginAvailability",
        "V2PluginAvailability",
        '"AVAILABLE".to_owned()',
    ),
    ("Turn", "itemsView"): (
        "codex_app_server_protocol::TurnItemsView",
        "V2TurnItemsView",
        '"full".to_owned()',
    ),
}

FORCE_PROTO_ONEOF_TYPES = {
    "McpElicitationEnumSchema",
    "McpElicitationMultiSelectEnumSchema",
    "McpElicitationPrimitiveSchema",
    "McpElicitationSingleSelectEnumSchema",
}

UNTAGGED_REF_VARIANTS = {
    (
        "McpElicitationEnumSchema",
        "McpElicitationLegacyTitledEnumSchema",
    ): "Legacy",
    (
        "McpElicitationEnumSchema",
        "McpElicitationMultiSelectEnumSchema",
    ): "MultiSelect",
    (
        "McpElicitationEnumSchema",
        "McpElicitationSingleSelectEnumSchema",
    ): "SingleSelect",
    (
        "McpElicitationMultiSelectEnumSchema",
        "McpElicitationTitledMultiSelectEnumSchema",
    ): "Titled",
    (
        "McpElicitationMultiSelectEnumSchema",
        "McpElicitationUntitledMultiSelectEnumSchema",
    ): "Untitled",
    (
        "McpElicitationPrimitiveSchema",
        "McpElicitationBooleanSchema",
    ): "Boolean",
    (
        "McpElicitationPrimitiveSchema",
        "McpElicitationEnumSchema",
    ): "Enum",
    (
        "McpElicitationPrimitiveSchema",
        "McpElicitationNumberSchema",
    ): "Number",
    (
        "McpElicitationPrimitiveSchema",
        "McpElicitationStringSchema",
    ): "String",
    (
        "McpElicitationSingleSelectEnumSchema",
        "McpElicitationTitledSingleSelectEnumSchema",
    ): "Titled",
    (
        "McpElicitationSingleSelectEnumSchema",
        "McpElicitationUntitledSingleSelectEnumSchema",
    ): "Untitled",
}


@dataclass(frozen=True)
class RpcEntry:
    variant: str
    params: str
    response: str


@dataclass(frozen=True)
class NotificationEntry:
    variant: str
    payload: str


def parse_rpc_entries(body: str) -> list[RpcEntry]:
    entry_pattern = re.compile(
        r'(?ms)^    ([A-Z][A-Za-z0-9]+)(?:\s*=>\s*"[^"]+")?\s*\{(.*?)^    \},'
    )
    entries = []
    for match in entry_pattern.finditer(body):
        variant, entry_body = match.groups()
        params = re.search(
            r"params:\s*(?:#\[[^\]]+\]\s*)*([^,\n]+),", entry_body
        )
        response = re.search(r"response:\s*([^,\n]+),", entry_body)
        if params is None or response is None:
            raise RuntimeError(f"failed to parse RPC entry {variant}")
        entries.append(
            RpcEntry(variant, params.group(1).strip(), response.group(1).strip())
        )
    return entries


def parse_notification_entries(body: str) -> list[NotificationEntry]:
    pattern = re.compile(
        r'(?m)^    ([A-Z][A-Za-z0-9]+)(?:\s*=>\s*"[^"]+")?\s*\(([^)]+)\),'
    )
    return [
        NotificationEntry(variant, payload.strip())
        for variant, payload in pattern.findall(body)
    ]


def proto_pascal(value: str) -> str:
    parts = re.split(r"[^A-Za-z0-9]+", value)
    return "".join(part[:1].upper() + part[1:] for part in parts if part)


def proto_snake(value: str) -> str:
    value = re.sub(r"(.)([A-Z][a-z]+)", r"\1_\2", value)
    value = re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", value)
    value = re.sub(r"[^A-Za-z0-9_]", "_", value).lower()
    if not value or value[0].isdigit():
        value = f"field_{value}"
    if value in {
        "enum",
        "extensions",
        "group",
        "import",
        "map",
        "max",
        "message",
        "oneof",
        "optional",
        "package",
        "public",
        "repeated",
        "reserved",
        "returns",
        "rpc",
        "service",
        "syntax",
        "to",
        "weak",
    }:
        value = f"{value}_field"
    return value


def rust_snake(value: str) -> str:
    value = re.sub(r"(.)([A-Z][a-z]+)", r"\1_\2", value)
    value = re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", value)
    value = re.sub(r"[^A-Za-z0-9_]", "_", value).lower()
    if not value or value[0].isdigit():
        value = f"field_{value}"
    return value


class SchemaProto:
    def __init__(self, v2_schema: Path, legacy_schema: Path) -> None:
        global _ACTIVE_SCHEMA
        self.v2 = json.loads(v2_schema.read_text())
        self.legacy = json.loads(legacy_schema.read_text())
        self.declarations: OrderedDict[str, str] = OrderedDict()
        self.transparent: set[str] = set()
        self.untagged: set[str] = set()
        self.custom_serde_messages: set[str] = set()
        self.nullable_fields: set[str] = set()
        self.nullable_wrappers: dict[str, str] = {}
        self.field_renames: dict[str, str] = {}
        self.named_types: dict[tuple[str, str], str] = {}
        self.inline_names: dict[str, str] = {}
        self.in_progress: set[tuple[str, str]] = set()
        self.proto_schemas: dict[str, tuple[str, Json]] = {}
        self.named_proto_keys: dict[str, tuple[str, str]] = {}
        _ACTIVE_SCHEMA = self

    def proto_for_rust_type(self, rust_type: str) -> str:
        if rust_type == "Option<()>":
            return "Empty"
        if rust_type.startswith("v2::"):
            return self.ensure_named("v2", rust_type.removeprefix("v2::"))
        if rust_type.startswith("v1::"):
            return self.ensure_named("legacy", rust_type.removeprefix("v1::"))
        return self.ensure_named("legacy", rust_type)

    def _is_double_option(self, owner: str, json_name: str) -> bool:
        return (owner, json_name) in DOUBLE_OPTION_FIELDS or (
            f"V2{owner}",
            json_name,
        ) in DOUBLE_OPTION_FIELDS

    def ensure_named(self, namespace: str, key: str) -> str:
        cache_key = (namespace, key)
        if cache_key in self.named_types:
            return self.named_types[cache_key]

        schema = self._definition(namespace, key)
        if "$ref" in schema:
            proto_type = self._ref_type(namespace, schema["$ref"])
            self.named_types[cache_key] = proto_type
            return proto_type

        name = self._named_proto_name(namespace, key)
        self.named_types[cache_key] = name
        self.named_proto_keys[name] = cache_key
        if cache_key in self.in_progress:
            return name
        self.in_progress.add(cache_key)
        self._declare_named(name, schema, namespace)
        self.in_progress.remove(cache_key)
        return name

    def render(self) -> str:
        return "\n\n".join(self.declarations.values())

    def serde_config(self) -> Json:
        return {
            "types": [],
            "transparent": [],
            "untagged": [],
            "nullableFields": [],
            "fields": {},
        }

    def _definition(self, namespace: str, key: str) -> Json:
        definitions = (
            self.v2["definitions"] if namespace == "v2" else self.legacy["definitions"]
        )
        if key not in definitions:
            if namespace == "v2" and key in self.legacy["definitions"]:
                schema = self.legacy["definitions"][key]
                if isinstance(schema, dict):
                    if key == "McpServerElicitationRequestParams":
                        return self._restore_flattened_elicitation_request(schema)
                    return schema
            override = self._legacy_override(key)
            if override is not None:
                return override
            raise KeyError(f"missing {namespace} schema definition {key}")
        schema = definitions[key]
        if not isinstance(schema, dict):
            raise TypeError(f"invalid {namespace} schema definition {key}")
        if namespace == "legacy" and key == "McpServerElicitationRequestParams":
            return self._restore_flattened_elicitation_request(schema)
        return schema

    def _legacy_override(self, key: str) -> Json | None:
        string = {"type": "string"}
        nullable_string = {"type": ["string", "null"]}
        overrides = {
            "GetConversationSummaryParams": {
                "oneOf": [
                    {
                        "type": "object",
                        "properties": {"rolloutPath": string},
                        "required": ["rolloutPath"],
                    },
                    {
                        "type": "object",
                        "properties": {"conversationId": string},
                        "required": ["conversationId"],
                    },
                ]
            },
            "GetConversationSummaryResponse": {
                "type": "object",
                "properties": {
                    "summary": {
                        "type": "object",
                        "properties": {
                            "conversationId": string,
                            "path": string,
                            "preview": string,
                            "timestamp": nullable_string,
                            "updatedAt": nullable_string,
                            "modelProvider": string,
                            "cwd": string,
                            "cliVersion": string,
                            "source": string,
                            "gitInfo": {
                                "type": ["object", "null"],
                                "properties": {
                                    "sha": nullable_string,
                                    "branch": nullable_string,
                                    "origin_url": nullable_string,
                                },
                            },
                        },
                        "required": [
                            "conversationId",
                            "path",
                            "preview",
                            "modelProvider",
                            "cwd",
                            "cliVersion",
                            "source",
                        ],
                    }
                },
                "required": ["summary"],
            },
            "GitDiffToRemoteParams": {
                "type": "object",
                "properties": {"cwd": string},
                "required": ["cwd"],
            },
            "GitDiffToRemoteResponse": {
                "type": "object",
                "properties": {"sha": string, "diff": string},
                "required": ["sha", "diff"],
            },
            "GetAuthStatusParams": {
                "type": "object",
                "properties": {
                    "includeToken": {"type": ["boolean", "null"]},
                    "refreshToken": {"type": ["boolean", "null"]},
                },
            },
            "GetAuthStatusResponse": {
                "type": "object",
                "properties": {
                    "authMethod": {"type": ["string", "null"]},
                    "authToken": nullable_string,
                    "requiresOpenaiAuth": {"type": ["boolean", "null"]},
                },
            },
            "McpServerElicitationRequest": {
                "oneOf": self.legacy["definitions"]
                .get(
                    "McpServerElicitationRequest",
                    self.legacy["definitions"]["McpServerElicitationRequestParams"],
                )["oneOf"],
            },
        }
        return overrides.get(key)

    def _restore_flattened_elicitation_request(self, schema: Json) -> Json:
        restored = dict(schema)
        restored.pop("oneOf", None)
        properties = OrderedDict(restored.get("properties", {}))
        properties["request"] = {
            "$ref": "#/definitions/McpServerElicitationRequest"
        }
        restored["properties"] = properties
        required = list(restored.get("required", []))
        if "request" not in required:
            required.append("request")
        restored["required"] = required
        return restored

    def _named_proto_name(self, namespace: str, key: str) -> str:
        prefix = "V2" if namespace == "v2" else "Legacy"
        return f"{prefix}{proto_pascal(key)}"

    def _ref_type(self, namespace: str, ref: str) -> str:
        prefix = "#/definitions/"
        if not ref.startswith(prefix):
            return "DynamicValue"
        key = ref.removeprefix(prefix)
        if key.startswith("v2/"):
            return self.ensure_named("v2", key.removeprefix("v2/"))
        return self.ensure_named(namespace, key)

    def _declare_named(self, name: str, schema: Json, namespace: str) -> None:
        self.proto_schemas[name] = (namespace, schema)
        base, _nullable = self._without_null(schema)
        if "enum" in base:
            self._declare_transparent_scalar(name, "string")
            return

        schema_type = base.get("type")
        if schema_type == "object" or self._object_shape(base, namespace) is not None:
            self._declare_object(name, base, namespace)
        elif schema_type == "array":
            item_type = self._value_type(
                base.get("items", {}), f"{name}Item", namespace
            )
            self._declare_transparent_list(name, item_type)
        elif schema_type in {"string", "boolean", "integer", "number"}:
            self._declare_transparent_scalar(name, self._scalar_type(base))
        elif "oneOf" in base or "anyOf" in base:
            rust_name = name.removeprefix("V2").removeprefix("Legacy")
            if (
                rust_name not in FORCE_PROTO_ONEOF_TYPES
                and self._union_object_shape(base, namespace) is not None
            ):
                self._declare_object(name, base, namespace)
            elif scalar_type := self._resolved_scalar_type(base, namespace):
                self._declare_transparent_scalar(name, scalar_type)
            else:
                self._declare_union(name, base, namespace)
        else:
            self._declare_transparent_scalar(name, "DynamicValue")

    def _declare_object(self, name: str, schema: Json, namespace: str) -> None:
        shape = self._object_shape(schema, namespace)
        if shape is None:
            shape = self._union_object_shape(schema, namespace)
        if shape is None:
            self._declare_transparent_scalar(name, "DynamicValue")
            return

        properties, required, additional = shape
        properties = OrderedDict(properties)
        required = set(required)
        rust_name = name.removeprefix("V2").removeprefix("Legacy")
        flattened = FLATTENED_MAP_FIELDS.get(rust_name)
        if flattened is not None:
            field, value_schema = flattened
            properties[field] = {
                "type": "object",
                "additionalProperties": value_schema,
            }
            required.add(field)
        if not properties and additional is not None and additional is not False:
            value_schema = additional if isinstance(additional, dict) else {}
            value_type = self._value_type(value_schema, f"{name}Value", namespace)
            self.declarations[name] = (
                f"message {name} {{\n  map<string, {value_type}> values = 1;\n}}"
            )
            self.transparent.add(name)
            return

        lines = [f"message {name} {{"]
        used_fields: set[str] = set()
        for number, (json_name, property_schema) in enumerate(
            properties.items(), start=1
        ):
            field_name = proto_snake(json_name)
            while field_name in used_fields:
                field_name = f"{field_name}_{number}"
            used_fields.add(field_name)
            declaration = self._field_declaration(
                property_schema,
                name,
                json_name,
                number,
                json_name in required,
                namespace,
            )
            lines.append(f"  {declaration}")
            self.field_renames[f"{name}.{field_name}"] = json_name
        lines.append("}")
        self.declarations[name] = "\n".join(lines)

    def _declare_transparent_scalar(self, name: str, proto_type: str) -> None:
        self.declarations[name] = (
            f"message {name} {{\n  {proto_type} value = 1;\n}}"
        )
        self.transparent.add(name)

    def _declare_transparent_list(self, name: str, item_type: str) -> None:
        self.declarations[name] = (
            f"message {name} {{\n  repeated {item_type} values = 1;\n}}"
        )
        self.transparent.add(name)

    def _declare_union(self, name: str, schema: Json, namespace: str) -> None:
        variants = schema.get("oneOf") or schema.get("anyOf")
        if not isinstance(variants, list) or not variants:
            self._declare_transparent_scalar(name, "DynamicValue")
            return

        lines = [f"message {name} {{", "  oneof value {"]
        used_fields: set[str] = set()
        for number, variant in enumerate(variants, start=1):
            base, _nullable = self._without_null(variant)
            title = base.get("title") if isinstance(base, dict) else None
            scalar_type = self._resolved_scalar_type(base, namespace)
            if scalar_type is not None:
                proto_type = scalar_type
                field_name = f"{scalar_type}_value"
            else:
                suggested = f"{name}{proto_pascal(title or f'Variant{number}')}"
                proto_type = self._value_type(base, suggested, namespace)
                field_name = proto_snake(title or f"variant_{number}")
            while field_name in used_fields:
                field_name = f"{field_name}_{number}"
            used_fields.add(field_name)
            lines.append(f"    {proto_type} {field_name} = {number};")
        lines.extend(["  }", "}"])
        self.declarations[name] = "\n".join(lines)
        self.transparent.add(name)
        self.untagged.add(f"{name}.value")

    def _ensure_nullable_wrapper(self, value_type: str) -> str:
        wrappers = {
            "string": "NullableString",
            "int64": "OptionalInt64",
            "uint64": "OptionalUint64",
        }
        if value_type in wrappers:
            name = wrappers[value_type]
            self.nullable_wrappers.setdefault(name, value_type)
            return name

        name = f"Nullable{proto_pascal(value_type)}"
        if name not in self.declarations:
            self.declarations[name] = "\n".join(
                [
                    f"message {name} {{",
                    "  oneof value {",
                    f"    {value_type} some = 1;",
                    "    Empty null = 2;",
                    "  }",
                    "}",
                ]
            )
            self.custom_serde_messages.add(name)
        self.nullable_wrappers.setdefault(name, value_type)
        return name

    def _field_declaration(
        self,
        schema: Json,
        owner: str,
        json_name: str,
        number: int,
        required: bool,
        namespace: str,
    ) -> str:
        base, nullable = self._without_null(schema)
        field_name = proto_snake(json_name)
        schema_type = base.get("type")

        if self._is_double_option(owner, json_name):
            value_type = self._value_type(
                base, f"{owner}{proto_pascal(json_name)}Value", namespace
            )
            wrapper = self._ensure_nullable_wrapper(value_type)
            self.nullable_fields.add(f"{owner}.{field_name}")
            return f"{wrapper} {field_name} = {number};"

        if schema_type == "array":
            item_type = self._value_type(
                base.get("items", {}),
                f"{owner}{proto_pascal(json_name)}Item",
                namespace,
            )
            if required and not nullable:
                return f"repeated {item_type} {field_name} = {number};"
            wrapper = self._ensure_inline(
                f"{owner}{proto_pascal(json_name)}List",
                {"type": "array", "items": base.get("items", {})},
                namespace,
            )
            return f"{wrapper} {field_name} = {number};"

        if schema_type == "object" and (
            "additionalProperties" in base and not base.get("properties")
        ):
            wrapper = self._ensure_inline(
                f"{owner}{proto_pascal(json_name)}Map", base, namespace
            )
            return f"{wrapper} {field_name} = {number};"

        proto_type = self._value_type(
            base, f"{owner}{proto_pascal(json_name)}", namespace
        )
        if proto_type in {"string", "bool", "int64", "uint64", "double"} and (
            nullable or not required
        ):
            return f"optional {proto_type} {field_name} = {number};"
        return f"{proto_type} {field_name} = {number};"

    def _value_type(self, schema: Json, suggested: str, namespace: str) -> str:
        base, _nullable = self._without_null(schema)
        if "$ref" in base:
            return self._ref_type(namespace, base["$ref"])
        if "enum" in base:
            return "string"
        if "allOf" in base and len(base["allOf"]) == 1:
            return self._value_type(base["allOf"][0], suggested, namespace)

        schema_type = base.get("type")
        if schema_type in {"string", "boolean", "integer", "number"}:
            return self._scalar_type(base)
        if schema_type in {"object", "array"}:
            return self._ensure_inline(suggested, base, namespace)
        if "oneOf" in base or "anyOf" in base:
            if self._union_object_shape(base, namespace) is not None:
                return self._ensure_inline(suggested, base, namespace)
            if scalar_type := self._resolved_scalar_type(base, namespace):
                return scalar_type
            return self._ensure_inline(suggested, base, namespace)
        return "DynamicValue"

    def _ensure_inline(self, suggested: str, schema: Json, namespace: str) -> str:
        fingerprint = json.dumps(
            {"namespace": namespace, "schema": schema}, sort_keys=True
        )
        if fingerprint in self.inline_names:
            return self.inline_names[fingerprint]

        name = proto_pascal(suggested)
        original = name
        suffix = 2
        while name in self.declarations:
            name = f"{original}{suffix}"
            suffix += 1
        self.inline_names[fingerprint] = name
        self._declare_named(name, schema, namespace)
        return name

    def _without_null(self, schema: Json) -> tuple[Json, bool]:
        if not isinstance(schema, dict):
            return {}, False
        schema_type = schema.get("type")
        if isinstance(schema_type, list) and "null" in schema_type:
            remaining = [item for item in schema_type if item != "null"]
            base = dict(schema)
            base["type"] = remaining[0] if len(remaining) == 1 else remaining
            return base, True

        for keyword in ("anyOf", "oneOf"):
            variants = schema.get(keyword)
            if isinstance(variants, list):
                non_null = [
                    variant
                    for variant in variants
                    if not (
                        isinstance(variant, dict)
                        and (
                            variant.get("type") == "null"
                            or variant.get("enum") == [None]
                        )
                    )
                ]
                if len(non_null) != len(variants):
                    if len(non_null) == 1:
                        return non_null[0], True
                    base = dict(schema)
                    base[keyword] = non_null
                    return base, True
        return schema, False

    def _scalar_type(self, schema: Json) -> str:
        match schema.get("type"):
            case "string":
                return "string"
            case "boolean":
                return "bool"
            case "integer":
                if str(schema.get("format", "")).startswith("uint"):
                    return "uint64"
                if schema.get("minimum", -1) >= 0:
                    return "uint64"
                return "int64"
            case "number":
                return "double"
            case _:
                return "DynamicValue"

    def _resolved_scalar_type(self, schema: Json, namespace: str) -> str | None:
        if not isinstance(schema, dict):
            return None
        base, _nullable = self._without_null(schema)
        if "$ref" in base:
            ref = base["$ref"]
            prefix = "#/definitions/"
            if not ref.startswith(prefix):
                return None
            key = ref.removeprefix(prefix)
            if key.startswith("v2/"):
                return self._resolved_scalar_type(
                    self._definition("v2", key.removeprefix("v2/")), "v2"
                )
            return self._resolved_scalar_type(
                self._definition(namespace, key), namespace
            )
        if "allOf" in base and len(base["allOf"]) == 1:
            return self._resolved_scalar_type(base["allOf"][0], namespace)
        if "enum" in base:
            values = base["enum"]
            if all(isinstance(value, str) for value in values):
                return "string"
            if all(isinstance(value, bool) for value in values):
                return "bool"
        if base.get("type") in {"string", "boolean", "integer", "number"}:
            return self._scalar_type(base)
        variants = base.get("oneOf") or base.get("anyOf")
        if isinstance(variants, list) and variants:
            resolved = [
                self._resolved_scalar_type(variant, namespace)
                for variant in variants
            ]
            scalar_types = set(resolved)
            if None not in scalar_types and len(scalar_types) == 1:
                return resolved[0]
        return None

    def _object_shape(
        self, schema: Json, namespace: str
    ) -> tuple[OrderedDict[str, Json], set[str], Any] | None:
        if not isinstance(schema, dict):
            return None
        if "$ref" in schema:
            ref = schema["$ref"]
            prefix = "#/definitions/"
            if not ref.startswith(prefix):
                return None
            key = ref.removeprefix(prefix)
            if key.startswith("v2/"):
                return self._object_shape(
                    self._definition("v2", key.removeprefix("v2/")), "v2"
                )
            return self._object_shape(self._definition(namespace, key), namespace)

        if "allOf" in schema:
            properties: OrderedDict[str, Json] = OrderedDict()
            required: set[str] = set()
            additional: Any = None
            for variant in schema["allOf"]:
                shape = self._object_shape(variant, namespace)
                if shape is None:
                    continue
                variant_properties, variant_required, variant_additional = shape
                properties.update(variant_properties)
                required.update(variant_required)
                if variant_additional is not None:
                    additional = variant_additional
            if properties or additional is not None:
                return properties, required, additional

        if schema.get("type") != "object" and "properties" not in schema:
            return None
        return (
            OrderedDict(schema.get("properties", {})),
            set(schema.get("required", [])),
            schema.get("additionalProperties"),
        )

    def _union_object_shape(
        self, schema: Json, namespace: str
    ) -> tuple[OrderedDict[str, Json], set[str], Any] | None:
        variants = schema.get("oneOf") or schema.get("anyOf")
        if not isinstance(variants, list) or not variants:
            return None
        shapes = [self._object_shape(variant, namespace) for variant in variants]
        if any(shape is None for shape in shapes):
            return None

        concrete_shapes = [shape for shape in shapes if shape is not None]
        property_names: OrderedDict[str, None] = OrderedDict()
        for properties, _required, _additional in concrete_shapes:
            for property_name in properties:
                property_names.setdefault(property_name, None)

        merged: OrderedDict[str, Json] = OrderedDict()
        for property_name in property_names:
            candidates = [
                properties[property_name]
                for properties, _required, _additional in concrete_shapes
                if property_name in properties
            ]
            merged[property_name] = self._merge_schemas(candidates, namespace)

        required = set(property_names)
        for properties, variant_required, _additional in concrete_shapes:
            required.intersection_update(variant_required)
            required.intersection_update(properties)
        return merged, required, False

    def _merge_schemas(self, schemas: list[Json], namespace: str) -> Json:
        unique = {
            json.dumps(schema, sort_keys=True): schema
            for schema in schemas
        }
        if len(unique) == 1:
            return next(iter(unique.values()))

        values = list(unique.values())
        scalar_types = [
            self._resolved_scalar_type(value, namespace) for value in values
        ]
        if scalar_types and all(
            scalar_type == scalar_types[0] for scalar_type in scalar_types
        ):
            match scalar_types[0]:
                case "string":
                    return {"type": "string"}
                case "bool":
                    return {"type": "boolean"}
                case "uint64":
                    return {"type": "integer", "minimum": 0}
                case "int64":
                    return {"type": "integer"}
                case "double":
                    return {"type": "number"}
        shapes = [self._object_shape(value, namespace) for value in values]
        if all(shape is not None for shape in shapes):
            return {"oneOf": values}

        types = {self._without_null(value)[0].get("type") for value in values}
        if types == {"array"}:
            items = [self._without_null(value)[0].get("items", {}) for value in values]
            return {"type": "array", "items": self._merge_schemas(items, namespace)}
        if types <= {"string"}:
            return {"type": "string"}
        if types <= {"integer"}:
            if any(self._scalar_type(value) == "uint64" for value in values):
                return {"type": "integer", "minimum": 0}
            return {"type": "integer"}
        if types <= {"number", "integer"}:
            return {"type": "number"}
        return {"anyOf": values}


def rust_variant(value: str, owner: str = "") -> str:
    override = RUST_VARIANT_OVERRIDES.get((owner, value))
    if override is not None:
        return override
    if re.fullmatch(r"[A-Z0-9_]+", value):
        return "".join(part.title() for part in value.split("_"))
    return proto_pascal(value)


def rust_field(owner: str, json_name: str) -> str:
    name = RUST_OWNER_FIELD_OVERRIDES.get(
        (owner, json_name),
        RUST_FIELD_OVERRIDES.get(json_name, rust_snake(json_name)),
    )
    return f"r#{name}" if name in RUST_KEYWORDS else name


RUST_KEYWORDS = {
    "as",
    "async",
    "await",
    "break",
    "const",
    "continue",
    "crate",
    "dyn",
    "else",
    "enum",
    "extern",
    "false",
    "fn",
    "for",
    "gen",
    "if",
    "impl",
    "in",
    "let",
    "loop",
    "match",
    "mod",
    "move",
    "mut",
    "pub",
    "ref",
    "return",
    "self",
    "Self",
    "static",
    "struct",
    "super",
    "trait",
    "true",
    "type",
    "union",
    "unsafe",
    "use",
    "where",
    "while",
}


def rust_proto_field(json_name: str) -> str:
    name = proto_snake(json_name).lstrip("_")
    return f"r#{name}" if name in RUST_KEYWORDS else name


class NativeRenderer:
    def __init__(
        self,
        schema: SchemaProto,
        type_pairs: dict[str, str],
        nullable_wrappers: dict[str, str],
    ) -> None:
        self.schema = schema
        self.type_pairs = type_pairs
        self.nullable_wrappers = nullable_wrappers
        self.rendered: set[str] = set()

    def render(self) -> str:
        lines = self._preamble()
        for proto_name, (namespace, key) in sorted(
            self.schema.named_proto_keys.items()
        ):
            if proto_name in self.schema.declarations:
                lines.extend(self._render_named(namespace, key, proto_name))
        for rust_type, proto_type in sorted(self.type_pairs.items()):
            lines.extend(
                [
                    f"impl NativeProto for codex_app_server_protocol::{rust_type} {{",
                    f"    type Proto = proto::{proto_type};",
                    "",
                    "    fn decode(payload: Self::Proto) -> Result<Self, Status> {",
                    "        DirectSchemaProto::decode_schema(payload)",
                    "    }",
                    "",
                    "    fn encode(self) -> Result<Self::Proto, Status> {",
                    "        DirectSchemaProto::encode_schema(self)",
                    "    }",
                    "}",
                    "",
                ]
            )
        lines.extend(
            [
                '#[path = "grpc_schema_model_overrides.rs"]',
                "mod model_overrides;",
                '#[path = "grpc_schema_item_overrides.rs"]',
                "mod item_overrides;",
                '#[path = "grpc_schema_misc_overrides.rs"]',
                "mod misc_overrides;",
                '#[path = "grpc_schema_extra_overrides.rs"]',
                "mod extra_overrides;",
                '#[path = "grpc_schema_union_overrides.rs"]',
                "mod union_overrides;",
                "",
            ]
        )
        return "\n".join(lines)

    def _preamble(self) -> list[str]:
        return [
            "// Generated by scripts/generate_native_grpc.py. Do not edit manually.",
            "",
            "use super::*;",
            "",
            "trait DirectSchemaProto<P>: Sized {",
            "    fn decode_schema(payload: P) -> Result<Self, Status>;",
            "    fn encode_schema(self) -> Result<P, Status>;",
            "}",
            "",
            "fn missing(field: &'static str) -> Status {",
            '    Status::invalid_argument(format!("missing protobuf field `{field}`"))',
            "}",
            "",
            "fn invalid(field: &'static str, error: impl std::fmt::Display) -> Status {",
            '    Status::invalid_argument(format!("invalid protobuf field `{field}`: {error}"))',
            "}",
            "",
            "fn encode_error(field: &'static str, error: impl std::fmt::Display) -> Status {",
            '    Status::internal(format!("failed to encode protobuf field `{field}`: {error}"))',
            "}",
            "",
            "trait DirectProtoString: Sized {",
            "    fn decode_string(value: String, field: &'static str) -> Result<Self, Status>;",
            "    fn encode_string(self) -> String;",
            "}",
            "",
            "impl DirectProtoString for String {",
            "    fn decode_string(value: String, _field: &'static str) -> Result<Self, Status> {",
            "        Ok(value)",
            "    }",
            "",
            "    fn encode_string(self) -> String {",
            "        self",
            "    }",
            "}",
            "",
            "impl DirectProtoString for std::path::PathBuf {",
            "    fn decode_string(value: String, _field: &'static str) -> Result<Self, Status> {",
            "        Ok(value.into())",
            "    }",
            "",
            "    fn encode_string(self) -> String {",
            "        self.to_string_lossy().into_owned()",
            "    }",
            "}",
            "",
            "impl DirectProtoString for codex_app_server_protocol::GitSha {",
            "    fn decode_string(value: String, _field: &'static str) -> Result<Self, Status> {",
            "        Ok(Self(value))",
            "    }",
            "",
            "    fn encode_string(self) -> String {",
            "        self.0",
            "    }",
            "}",
            "",
            "fn decode_string<T: DirectProtoString>(",
            "    value: String,",
            "    field: &'static str,",
            ") -> Result<T, Status> {",
            "    T::decode_string(value, field)",
            "}",
            "",
            "fn encode_string<T: DirectProtoString>(value: T) -> String {",
            "    value.encode_string()",
            "}",
            "",
            "fn decode_newtype_string<T>(",
            "    value: String,",
            "    field: &'static str,",
            ") -> Result<T, Status>",
            "where",
            "    T: TryFrom<String>,",
            "    T::Error: std::fmt::Display,",
            "{",
            "    value.try_into().map_err(|error| invalid(field, error))",
            "}",
            "",
            "fn decode_integer<T>(value: i64, field: &'static str) -> Result<T, Status>",
            "where",
            "    T: TryFrom<i64>,",
            "    T::Error: std::fmt::Display,",
            "{",
            "    value.try_into().map_err(|error| invalid(field, error))",
            "}",
            "",
            "fn encode_integer<T>(value: T, field: &'static str) -> Result<i64, Status>",
            "where",
            "    T: TryInto<i64>,",
            "    T::Error: std::fmt::Display,",
            "{",
            "    value.try_into().map_err(|error| encode_error(field, error))",
            "}",
            "",
            "trait DirectProtoUnsigned: Sized {",
            "    fn decode_unsigned(value: u64, field: &'static str) -> Result<Self, Status>;",
            "    fn encode_unsigned(self, field: &'static str) -> Result<u64, Status>;",
            "}",
            "",
            "macro_rules! direct_proto_unsigned {",
            "    ($($type:ty),+ $(,)?) => {",
            "        $(",
            "            impl DirectProtoUnsigned for $type {",
            "                fn decode_unsigned(value: u64, field: &'static str) -> Result<Self, Status> {",
            "                    value.try_into().map_err(|error| invalid(field, error))",
            "                }",
            "",
            "                fn encode_unsigned(self, _field: &'static str) -> Result<u64, Status> {",
            "                    Ok(self as u64)",
            "                }",
            "            }",
            "        )+",
            "    };",
            "}",
            "",
            "direct_proto_unsigned!(u16, u32, u64, usize);",
            "",
            "impl DirectProtoUnsigned for std::num::NonZeroUsize {",
            "    fn decode_unsigned(value: u64, field: &'static str) -> Result<Self, Status> {",
            "        let value: usize = value.try_into().map_err(|error| invalid(field, error))?;",
            "        Self::new(value).ok_or_else(|| invalid(field, \"expected a non-zero value\"))",
            "    }",
            "",
            "    fn encode_unsigned(self, _field: &'static str) -> Result<u64, Status> {",
            "        Ok(self.get() as u64)",
            "    }",
            "}",
            "",
            "fn decode_unsigned<T: DirectProtoUnsigned>(",
            "    value: u64,",
            "    field: &'static str,",
            ") -> Result<T, Status> {",
            "    T::decode_unsigned(value, field)",
            "}",
            "",
            "fn encode_unsigned<T: DirectProtoUnsigned>(",
            "    value: T,",
            "    field: &'static str,",
            ") -> Result<u64, Status> {",
            "    value.encode_unsigned(field)",
            "}",
            "",
        ]

    def _render_named(
        self, namespace: str, key: str, proto_name: str
    ) -> list[str]:
        if proto_name in self.rendered:
            return []
        self.rendered.add(proto_name)
        if key in MANUAL_SCHEMA_TYPES:
            return []
        schema = self.schema._definition(namespace, key)
        base, _nullable = self.schema._without_null(schema)
        rust_type = self._rust_type(namespace, key)
        if key in UNSUPPORTED_SCHEMA_TYPES:
            return self._unsupported_impl(
                rust_type, proto_name, key, "schema shape loses tagged fields"
            )
        if values := self._unit_enum_values(base):
            return self._render_unit_enum(key, rust_type, proto_name, values)
        if self._tagged_variants(base, namespace) is not None:
            return self._render_tagged_enum(
                namespace, key, rust_type, proto_name, base
            )
        variants = base.get("oneOf") or base.get("anyOf")
        if isinstance(variants, list):
            if (
                key not in FORCE_PROTO_ONEOF_TYPES
                and self.schema._union_object_shape(base, namespace) is not None
            ):
                return self._render_merged_union(
                    namespace, key, rust_type, proto_name, variants
                )
            return self._render_oneof_union(
                namespace, key, rust_type, proto_name, variants
            )
        schema_type = base.get("type")
        if schema_type == "object":
            if key == "TextElement":
                return self._render_text_element(
                    namespace, rust_type, proto_name, base
                )
            return self._render_struct(
                namespace, key, rust_type, proto_name, base
            )
        if schema_type in {"string", "boolean", "integer", "number"}:
            if key in RUST_STRING_NEW_TYPES:
                return []
            return self._render_scalar(key, rust_type, proto_name, base)
        return []

    def _render_text_element(
        self,
        namespace: str,
        rust_type: str,
        proto_name: str,
        schema: Json,
    ) -> list[str]:
        shape = self.schema._object_shape(schema, namespace)
        if shape is None:
            return self._unsupported_impl(
                rust_type, proto_name, "TextElement", "object"
            )
        properties, required, _additional = shape
        byte_range = self._decode_field(
            namespace,
            "TextElement",
            "byteRange",
            properties["byteRange"],
            "payload.byte_range",
            "byteRange" in required,
        )
        placeholder = self._decode_field(
            namespace,
            "TextElement",
            "placeholder",
            properties["placeholder"],
            "payload.placeholder",
            "placeholder" in required,
        )
        encoded_range = self._encode_field(
            namespace,
            "TextElement",
            "byteRange",
            properties["byteRange"],
            "self.byte_range",
            "byteRange" in required,
        )
        encoded_placeholder = self._encode_field(
            namespace,
            "TextElement",
            "placeholder",
            properties["placeholder"],
            "self.placeholder().map(str::to_owned)",
            "placeholder" in required,
        )
        return [
            f"impl DirectSchemaProto<proto::{proto_name}> for {rust_type} {{",
            f"    fn decode_schema(payload: proto::{proto_name}) -> Result<Self, Status> {{",
            f"        Ok(Self::new({byte_range}, {placeholder}))",
            "    }",
            "",
            f"    fn encode_schema(self) -> Result<proto::{proto_name}, Status> {{",
            f"        Ok(proto::{proto_name} {{",
            f"            byte_range: {encoded_range},",
            f"            placeholder: {encoded_placeholder},",
            "        })",
            "    }",
            "}",
            "",
        ]

    def _render_scalar(
        self, key: str, rust_type: str, proto_name: str, schema: Json
    ) -> list[str]:
        scalar = self.schema._scalar_type(schema)
        decode = self._decode_scalar(scalar, "payload.value", key)
        encode = self._encode_scalar(scalar, "self", key)
        return [
            f"impl DirectSchemaProto<proto::{proto_name}> for {rust_type} {{",
            f"    fn decode_schema(payload: proto::{proto_name}) -> Result<Self, Status> {{",
            f"        Ok({decode})",
            "    }",
            "",
            f"    fn encode_schema(self) -> Result<proto::{proto_name}, Status> {{",
            f"        Ok(proto::{proto_name} {{ value: {encode} }})",
            "    }",
            "}",
            "",
        ]

    def _render_unit_enum(
        self,
        key: str,
        rust_type: str,
        proto_name: str,
        values: list[str],
    ) -> list[str]:
        lines = [
            f"impl DirectSchemaProto<proto::{proto_name}> for {rust_type} {{",
            f"    fn decode_schema(payload: proto::{proto_name}) -> Result<Self, Status> {{",
            "        match payload.value.as_str() {",
        ]
        for value in values:
            lines.append(
                f'            {json.dumps(value)} => Ok(Self::{rust_variant(value, key)}),'
            )
        lines.extend(
            [
                f'            value => Err(invalid("{key}", format!("unknown value `{{value}}`"))),',
                "        }",
                "    }",
                "",
                f"    fn encode_schema(self) -> Result<proto::{proto_name}, Status> {{",
                "        let value = match self {",
            ]
        )
        for value in values:
            lines.append(
                f"            Self::{rust_variant(value, key)} => {json.dumps(value)},"
            )
        lines.extend(
            [
                f'            _ => return Err(Status::unimplemented({json.dumps(f"unsupported hidden {key} variant")})),',
                "        };",
                f"        Ok(proto::{proto_name} {{ value: value.to_owned() }})",
                "    }",
                "}",
                "",
            ]
        )
        return lines

    def _render_struct(
        self,
        namespace: str,
        key: str,
        rust_type: str,
        proto_name: str,
        schema: Json,
    ) -> list[str]:
        shape = self._struct_shape(key, schema, namespace)
        if shape is None:
            return []
        properties, required, _additional = shape
        lines = [
            f"impl DirectSchemaProto<proto::{proto_name}> for {rust_type} {{",
            f"    fn decode_schema(payload: proto::{proto_name}) -> Result<Self, Status> {{",
            "        Ok(Self {",
        ]
        for json_name, field_schema in properties.items():
            rust_name = rust_field(key, json_name)
            proto_name_field = rust_proto_field(json_name)
            expression = self._decode_field(
                namespace,
                key,
                json_name,
                field_schema,
                f"payload.{proto_name_field}",
                json_name in required,
            )
            lines.append(f"            {rust_name}: {expression},")
        lines.extend(
            [
                "        })",
                "    }",
                "",
                f"    fn encode_schema(self) -> Result<proto::{proto_name}, Status> {{",
                f"        Ok(proto::{proto_name} {{",
            ]
        )
        for json_name, field_schema in properties.items():
            rust_name = rust_field(key, json_name)
            proto_name_field = rust_proto_field(json_name)
            expression = self._encode_field(
                namespace,
                key,
                json_name,
                field_schema,
                f"self.{rust_name}",
                json_name in required,
            )
            lines.append(f"            {proto_name_field}: {expression},")
        lines.extend(["        })", "    }", "}", ""])
        return lines

    def _struct_shape(
        self, key: str, schema: Json, namespace: str
    ) -> tuple[OrderedDict[str, Json], set[str], Any] | None:
        shape = self.schema._object_shape(schema, namespace)
        if shape is None:
            return None
        properties, required, additional = shape
        properties = OrderedDict(properties)
        required = set(required)
        flattened = FLATTENED_MAP_FIELDS.get(key)
        if flattened is not None:
            field, value_schema = flattened
            properties[field] = {
                "type": "object",
                "additionalProperties": value_schema,
            }
            required.add(field)
        return properties, required, additional

    def _unit_enum_values(self, schema: Json) -> list[str]:
        if "enum" in schema and all(
            isinstance(value, str) for value in schema["enum"]
        ):
            return list(schema["enum"])
        variants = schema.get("oneOf") or schema.get("anyOf")
        if not isinstance(variants, list) or not variants:
            return []
        values = []
        for variant in variants:
            base, _nullable = self.schema._without_null(variant)
            enum_values = base.get("enum")
            if (
                not isinstance(enum_values, list)
                or not enum_values
                or not all(isinstance(value, str) for value in enum_values)
            ):
                return []
            values.extend(enum_values)
        return values

    def _tagged_variants(
        self, schema: Json, namespace: str
    ) -> tuple[str, list[tuple[str, Json]]] | None:
        variants = schema.get("oneOf") or schema.get("anyOf")
        if not isinstance(variants, list) or not variants:
            return None
        object_variants = []
        for variant in variants:
            shape = self.schema._object_shape(variant, namespace)
            if shape is None:
                return None
            properties, _required, _additional = shape
            object_variants.append((variant, properties))
        common = set(object_variants[0][1])
        for _variant, properties in object_variants[1:]:
            common.intersection_update(properties)
        for field in common:
            tagged = []
            for variant, properties in object_variants:
                property_schema = properties[field]
                if not isinstance(property_schema, dict):
                    break
                values = property_schema.get("enum")
                if not isinstance(values, list) or len(values) != 1:
                    break
                tagged.append((values[0], variant))
            else:
                if all(isinstance(value, str) for value, _variant in tagged):
                    return field, tagged
        return None

    def _render_tagged_enum(
        self,
        namespace: str,
        key: str,
        rust_type: str,
        proto_name: str,
        schema: Json,
    ) -> list[str]:
        tagged = self._tagged_variants(schema, namespace)
        if tagged is None:
            return self._unsupported_impl(rust_type, proto_name, key, "tagged union")
        tag_field, variants = tagged
        merged = self.schema._union_object_shape(schema, namespace)
        if merged is None:
            return self._unsupported_impl(rust_type, proto_name, key, "tagged union")
        merged_properties, merged_required, _additional = merged
        tag_proto_field = rust_proto_field(tag_field)
        lines = [
            f"impl DirectSchemaProto<proto::{proto_name}> for {rust_type} {{",
            f"    fn decode_schema(payload: proto::{proto_name}) -> Result<Self, Status> {{",
            f"        match payload.{tag_proto_field}.as_str() {{",
        ]
        for tag, variant_schema in variants:
            shape = self.schema._object_shape(variant_schema, namespace)
            if shape is None:
                continue
            properties, required, _additional = shape
            variant_name = self._variant_name(key, variant_schema, str(tag))
            fields = [
                (json_name, field_schema)
                for json_name, field_schema in properties.items()
                if json_name != tag_field
            ]
            if not fields:
                suffix = (
                    " {}"
                    if (key, variant_name) in EMPTY_STRUCT_VARIANTS
                    else ""
                )
                construction = f"Self::{variant_name}{suffix}"
            else:
                rendered_fields = []
                for json_name, field_schema in fields:
                    expression = self._decode_field(
                        namespace,
                        key,
                        json_name,
                        field_schema,
                        f"payload.{rust_proto_field(json_name)}",
                        json_name in required,
                        proto_required=json_name in merged_required,
                        proto_schema=merged_properties.get(json_name),
                    )
                    rendered_fields.append(
                        f"{rust_field(key, json_name)}: {expression}"
                    )
                construction = (
                    f"Self::{variant_name} {{ " + ", ".join(rendered_fields) + " }"
                )
            lines.append(f"            {json.dumps(tag)} => Ok({construction}),")
        lines.extend(
            [
                f'            value => Err(invalid("{key}.{tag_field}", format!("unknown tag `{{value}}`"))),',
                "        }",
                "    }",
                "",
                f"    fn encode_schema(self) -> Result<proto::{proto_name}, Status> {{",
                "        match self {",
            ]
        )
        for tag, variant_schema in variants:
            shape = self.schema._object_shape(variant_schema, namespace)
            if shape is None:
                continue
            properties, required, _additional = shape
            variant_name = self._variant_name(key, variant_schema, str(tag))
            fields = [
                (json_name, field_schema)
                for json_name, field_schema in properties.items()
                if json_name != tag_field
            ]
            if fields:
                bindings = ", ".join(rust_field(key, name) for name, _ in fields)
                pattern = f"Self::{variant_name} {{ {bindings} }}"
            else:
                suffix = (
                    " {}"
                    if (key, variant_name) in EMPTY_STRUCT_VARIANTS
                    else ""
                )
                pattern = f"Self::{variant_name}{suffix}"
            lines.extend(
                [
                    f"            {pattern} => Ok(proto::{proto_name} {{",
                    f"                {tag_proto_field}: {json.dumps(tag)}.to_owned(),",
                ]
            )
            for json_name, field_schema in fields:
                rust_name = rust_field(key, json_name)
                expression = self._encode_field(
                    namespace,
                    key,
                    json_name,
                    field_schema,
                    rust_name,
                    json_name in required,
                    proto_required=json_name in merged_required,
                    proto_schema=merged_properties.get(json_name),
                )
                lines.append(
                    f"                {rust_proto_field(json_name)}: {expression},"
                )
            lines.extend(["                ..Default::default()", "            }),"])
        lines.extend(["        }", "    }", "}", ""])
        return lines

    def _render_merged_union(
        self,
        namespace: str,
        key: str,
        rust_type: str,
        proto_name: str,
        variants: list[Json],
    ) -> list[str]:
        branches = []
        for number, variant in enumerate(variants, start=1):
            shape = self.schema._object_shape(variant, namespace)
            if shape is None:
                return self._unsupported_impl(
                    rust_type, proto_name, key, "heterogeneous merged union"
                )
            properties, required, _additional = shape
            if len(properties) != 1:
                return self._unsupported_impl(
                    rust_type, proto_name, key, "multi-field untagged union"
                )
            json_name, field_schema = next(iter(properties.items()))
            variant_name = self._variant_name(key, variant, json_name)
            branches.append(
                (number, variant_name, json_name, field_schema, json_name in required)
            )
        lines = [
            f"impl DirectSchemaProto<proto::{proto_name}> for {rust_type} {{",
            f"    fn decode_schema(payload: proto::{proto_name}) -> Result<Self, Status> {{",
        ]
        for _number, variant_name, json_name, field_schema, required in branches:
            field = rust_proto_field(json_name)
            inner = self._decode_value(
                namespace,
                field_schema,
                "value",
                f"{key}.{json_name}",
            )
            construction = (
                f"Self::{variant_name} {{ {rust_field(key, json_name)}: {inner} }}"
                if key == "GetConversationSummaryParams"
                else f"Self::{variant_name}({inner})"
            )
            lines.extend(
                [
                    f"        if let Some(value) = payload.{field} {{",
                    f"            return Ok({construction});",
                    "        }",
                ]
            )
        lines.extend(
            [
                f'        Err(invalid("{key}", "no untagged union branch matched"))',
                "    }",
                "",
                f"    fn encode_schema(self) -> Result<proto::{proto_name}, Status> {{",
                "        match self {",
            ]
        )
        for _number, variant_name, json_name, field_schema, _required in branches:
            encoded = self._encode_value(
                namespace, field_schema, "value", f"{key}.{json_name}"
            )
            pattern = (
                f"Self::{variant_name} {{ {rust_field(key, json_name)}: value }}"
                if key == "GetConversationSummaryParams"
                else f"Self::{variant_name}(value)"
            )
            lines.extend(
                [
                    f"            {pattern} => Ok(proto::{proto_name} {{",
                    f"                {rust_proto_field(json_name)}: Some({encoded}),",
                    "                ..Default::default()",
                    "            }),",
                ]
            )
        lines.extend(["        }", "    }", "}", ""])
        return lines

    def _render_oneof_union(
        self,
        namespace: str,
        key: str,
        rust_type: str,
        proto_name: str,
        variants: list[Json],
    ) -> list[str]:
        module = proto_snake(proto_name)
        rendered = []
        used_fields: set[str] = set()
        for number, variant in enumerate(variants, start=1):
            base, _nullable = self.schema._without_null(variant)
            title = base.get("title")
            scalar = self.schema._resolved_scalar_type(base, namespace)
            if scalar is not None:
                field_name = f"{scalar}_value"
                while field_name in used_fields:
                    field_name = f"{field_name}_{number}"
                used_fields.add(field_name)
                values = self._unit_enum_values(base)
                if not values:
                    return self._unsupported_impl(
                        rust_type, proto_name, key, "open scalar oneof"
                    )
                rendered.append(("scalar", field_name, values, base, number))
                continue
            field_name = proto_snake(title or f"variant_{number}")
            while field_name in used_fields:
                field_name = f"{field_name}_{number}"
            used_fields.add(field_name)
            if "$ref" in base:
                ref_namespace, ref_key, ref_proto = self._ref_info(
                    namespace, base["$ref"]
                )
                rendered.append(
                    (
                        "ref",
                        field_name,
                        (ref_namespace, ref_key, ref_proto),
                        base,
                        number,
                    )
                )
                continue
            shape = self.schema._object_shape(base, namespace)
            if shape is None or len(shape[0]) != 1:
                return self._unsupported_impl(
                    rust_type, proto_name, key, "complex protobuf oneof"
                )
            rendered.append(("object", field_name, title, base, number))
        lines = [
            f"impl DirectSchemaProto<proto::{proto_name}> for {rust_type} {{",
            f"    fn decode_schema(payload: proto::{proto_name}) -> Result<Self, Status> {{",
            f"        match payload.value.ok_or_else(|| missing(\"{key}.value\"))? {{",
        ]
        for kind, field_name, detail, variant, _number in rendered:
            proto_variant = proto_pascal(field_name)
            if kind == "scalar":
                lines.append(
                    f"            proto::{module}::Value::{proto_variant}(value) => match value.as_str() {{"
                )
                for value in detail:
                    lines.append(
                        f"                {json.dumps(value)} => Ok(Self::{rust_variant(value, key)}),"
                    )
                lines.extend(
                    [
                        f'                value => Err(invalid("{key}", format!("unknown value `{{value}}`"))),',
                        "            },",
                    ]
                )
            elif kind == "ref":
                ref_namespace, ref_key, ref_proto = detail
                variant_name = UNTAGGED_REF_VARIANTS.get(
                    (key, ref_key), rust_variant(ref_key, key)
                )
                lines.append(
                    f"            proto::{module}::Value::{proto_variant}(value) => "
                    f"Ok(Self::{variant_name}(<{self._rust_type(ref_namespace, ref_key)} as "
                    f"DirectSchemaProto<proto::{ref_proto}>>::decode_schema(value)?)),"
                )
            else:
                shape = self.schema._object_shape(variant, namespace)
                assert shape is not None
                properties, _required, _additional = shape
                json_name, field_schema = next(iter(properties.items()))
                variant_name = self._variant_name(key, variant, json_name)
                nested = self.schema._object_shape(field_schema, namespace)
                if nested is None:
                    inner = self._decode_field(
                        namespace,
                        key,
                        json_name,
                        field_schema,
                        f"value.{rust_proto_field(json_name)}",
                        True,
                    )
                    construction = f"Self::{variant_name}({inner})"
                    lines.append(
                        f"            proto::{module}::Value::{proto_variant}(value) => Ok({construction}),"
                    )
                else:
                    nested_properties, nested_required, _additional = nested
                    inner_name = self.schema._value_type(
                        field_schema,
                        (
                            f"{proto_name}{proto_pascal(str(detail) if detail else f'Variant{_number}')}"
                            f"{proto_pascal(json_name)}"
                        ),
                        namespace,
                    )
                    inner_value = "nested_value"
                    fields = []
                    for nested_name, nested_schema in nested_properties.items():
                        decoded = self._decode_field(
                            namespace,
                            key,
                            nested_name,
                            nested_schema,
                            f"{inner_value}.{rust_proto_field(nested_name)}",
                            nested_name in nested_required,
                        )
                        fields.append(
                            f"{rust_field(key, nested_name)}: {decoded}"
                        )
                    _ = inner_name
                    construction = (
                        f"Self::{variant_name} {{ " + ", ".join(fields) + " }"
                    )
                    lines.extend(
                        [
                            f"            proto::{module}::Value::{proto_variant}(value) => {{",
                            (
                                f"                let nested_value = value.{rust_proto_field(json_name)}"
                                f".ok_or_else(|| missing({json.dumps(f'{key}.{json_name}')}))?;"
                            ),
                            f"                Ok({construction})",
                            "            },",
                        ]
                    )
        lines.extend(
            [
                "        }",
                "    }",
                "",
                f"    fn encode_schema(self) -> Result<proto::{proto_name}, Status> {{",
                "        let value = match self {",
            ]
        )
        for kind, field_name, detail, variant, _number in rendered:
            proto_variant = proto_pascal(field_name)
            if kind == "scalar":
                for value in detail:
                    lines.append(
                        f"            Self::{rust_variant(value, key)} => proto::{module}::Value::{proto_variant}({json.dumps(value)}.to_owned()),"
                    )
            elif kind == "ref":
                ref_namespace, ref_key, ref_proto = detail
                variant_name = UNTAGGED_REF_VARIANTS.get(
                    (key, ref_key), rust_variant(ref_key, key)
                )
                lines.append(
                    f"            Self::{variant_name}(value) => "
                    f"proto::{module}::Value::{proto_variant}("
                    f"<{self._rust_type(ref_namespace, ref_key)} as "
                    f"DirectSchemaProto<proto::{ref_proto}>>::encode_schema(value)?),"
                )
            else:
                shape = self.schema._object_shape(variant, namespace)
                assert shape is not None
                properties, required, _additional = shape
                json_name, field_schema = next(iter(properties.items()))
                variant_name = self._variant_name(key, variant, json_name)
                inline_type = self.schema._value_type(
                    variant,
                    f"{proto_name}{proto_pascal(str(detail) if detail else f'Variant{_number}')}",
                    namespace,
                )
                nested = self.schema._object_shape(field_schema, namespace)
                if nested is None:
                    encoded = self._encode_field(
                        namespace,
                        key,
                        json_name,
                        field_schema,
                        "value",
                        json_name in required,
                    )
                    pattern = f"Self::{variant_name}(value)"
                    outer_value = encoded
                else:
                    nested_properties, nested_required, _additional = nested
                    bindings = ", ".join(
                        rust_field(key, name) for name in nested_properties
                    )
                    pattern = f"Self::{variant_name} {{ {bindings} }}"
                    nested_type = self.schema._value_type(
                        field_schema,
                        (
                            f"{proto_name}{proto_pascal(str(detail) if detail else f'Variant{_number}')}"
                            f"{proto_pascal(json_name)}"
                        ),
                        namespace,
                    )
                    nested_fields = []
                    for nested_name, nested_schema in nested_properties.items():
                        encoded = self._encode_field(
                            namespace,
                            key,
                            nested_name,
                            nested_schema,
                            rust_field(key, nested_name),
                            nested_name in nested_required,
                        )
                        nested_fields.append(
                            f"{rust_proto_field(nested_name)}: {encoded}"
                        )
                    outer_value = (
                        f"Some(proto::{nested_type} {{ "
                        + ", ".join(nested_fields)
                        + " })"
                    )
                lines.append(
                    f"            {pattern} => proto::{module}::Value::{proto_variant}(proto::{inline_type} {{ {rust_proto_field(json_name)}: {outer_value} }}),"
                )
        lines.extend(
            [
                "        };",
                f"        Ok(proto::{proto_name} {{ value: Some(value) }})",
                "    }",
                "}",
                "",
            ]
        )
        return lines

    def _unsupported_impl(
        self, rust_type: str, proto_name: str, key: str, shape: str
    ) -> list[str]:
        message = f"direct protobuf conversion for {key} ({shape}) is not implemented"
        return [
            f"impl DirectSchemaProto<proto::{proto_name}> for {rust_type} {{",
            f"    fn decode_schema(_payload: proto::{proto_name}) -> Result<Self, Status> {{",
            f"        Err(Status::unimplemented({json.dumps(message)}))",
            "    }",
            "",
            f"    fn encode_schema(self) -> Result<proto::{proto_name}, Status> {{",
            "        let _ = self;",
            f"        Err(Status::unimplemented({json.dumps(message)}))",
            "    }",
            "}",
            "",
        ]

    def _variant_name(self, owner: str, schema: Json, fallback: str) -> str:
        title = schema.get("title")
        if isinstance(title, str):
            for suffix in (owner, f"{owner}Variant"):
                if title.endswith(suffix) and title != suffix:
                    return title[: -len(suffix)]
            return title
        overrides = {
            ("GetConversationSummaryParams", "rolloutPath"): "RolloutPath",
            ("GetConversationSummaryParams", "conversationId"): "ThreadId",
        }
        return overrides.get((owner, fallback), rust_variant(fallback, owner))

    def _decode_field(
        self,
        namespace: str,
        owner: str,
        json_name: str,
        schema: Json,
        expression: str,
        required: bool,
        *,
        proto_required: bool | None = None,
        proto_schema: Json | None = None,
    ) -> str:
        proto_required = required if proto_required is None else proto_required
        base, nullable = self.schema._without_null(schema)
        proto_base, _proto_nullable = self.schema._without_null(
            schema if proto_schema is None else proto_schema
        )
        context = f"{owner}.{json_name}"
        if self._is_double_option(owner, json_name):
            return self._decode_double_option(
                namespace, owner, json_name, base, expression
            )
        if direct_message := DIRECT_MESSAGE_FIELDS.get((owner, json_name)):
            rust_type, proto_type, default = direct_message
            value = (
                f"{expression}.unwrap_or(proto::{proto_type} {{ value: {default} }})"
            )
            return (
                f"<{rust_type} as DirectSchemaProto<proto::{proto_type}>>::"
                f"decode_schema({value})?"
            )
        if rust_type := RUST_TRANSPARENT_LIST_FIELDS.get((owner, json_name)):
            return self._decode_transparent_list_field(
                namespace,
                owner,
                json_name,
                base,
                expression,
                required,
                proto_required,
                rust_type,
            )
        string_newtype = self._string_newtype(namespace, base)
        if string_newtype is not None and self._is_scalar_string(
            namespace, proto_base
        ):
            value = expression
            if not proto_required:
                if required:
                    value = (
                        f"{expression}.ok_or_else(|| missing({json.dumps(context)}))?"
                    )
                else:
                    inner = self._decode_string_newtype(
                        string_newtype, "value", context
                    )
                    return (
                        f"{expression}.map(|value| Ok::<_, Status>({inner}))"
                        ".transpose()?"
                    )
            return self._decode_string_newtype(string_newtype, value, context)
        if not required and isinstance(schema, dict) and "default" in schema:
            default = schema["default"]
            scalar = self.schema._resolved_scalar_type(base, namespace)
            if scalar is not None and isinstance(
                default, (str, bool, int, float)
            ):
                if self._is_message(base, namespace) and "$ref" in base:
                    ref_namespace, key, proto_type = self._ref_info(
                        namespace, base["$ref"]
                    )
                    decoded = (
                        f"<{self._rust_type(ref_namespace, key)} as "
                        f"DirectSchemaProto<proto::{proto_type}>>::decode_schema(value)?"
                    )
                    default_proto = (
                        f"proto::{proto_type} {{ value: "
                        f"{self._rust_literal(default)} }}"
                    )
                    return (
                        f"match {expression} {{ Some(value) => {decoded}, "
                        f"None => <{self._rust_type(ref_namespace, key)} as "
                        f"DirectSchemaProto<proto::{proto_type}>>::decode_schema({default_proto})? }}"
                    )
                value = f"{expression}.unwrap_or({self._rust_literal(default)})"
                return self._decode_scalar(scalar, value, context)
        if base.get("type") == "array":
            item = base.get("items", {})
            if proto_required:
                return self._decode_vec(namespace, item, expression, context)
            wrapped = f"{expression}.map(|wrapper| wrapper.values)"
            if not nullable:
                return self._decode_vec(
                    namespace,
                    item,
                    (
                        f"{expression}.ok_or_else(|| missing({json.dumps(context)}))?.values"
                        if required
                        else f"{expression}.map(|wrapper| wrapper.values).unwrap_or_default()"
                    ),
                    context,
                )
            inner = self._decode_vec(namespace, item, "values", context)
            return (
                f"{wrapped}.map(|values| Ok::<_, Status>({inner})).transpose()?"
            )
        if base.get("type") == "object" and (
            "additionalProperties" in base and not base.get("properties")
        ):
            value_schema = (
                base["additionalProperties"]
                if isinstance(base["additionalProperties"], dict)
                else {}
            )
            if not nullable:
                return self._decode_map(
                    namespace,
                    value_schema,
                    (
                        f"{expression}.ok_or_else(|| missing({json.dumps(context)}))?.values"
                        if required
                        else f"{expression}.map(|wrapper| wrapper.values).unwrap_or_default()"
                    ),
                    context,
                )
            inner = self._decode_map(namespace, value_schema, "wrapper.values", context)
            return (
                f"{expression}.map(|wrapper| Ok::<_, Status>({inner})).transpose()?"
            )
        is_message = self._is_message(base, namespace)
        if is_message:
            if required and not nullable:
                value = (
                    f"{expression}.ok_or_else(|| missing({json.dumps(context)}))?"
                )
                return self._decode_value(namespace, base, value, context)
            inner = self._decode_value(namespace, base, "value", context)
            return (
                f"{expression}.map(|value| Ok::<_, Status>({inner})).transpose()?"
            )
        if proto_required:
            return self._decode_value(namespace, base, expression, context)
        if not nullable:
            value = f"{expression}.unwrap_or_default()"
            return self._decode_value(namespace, base, value, context)
        if required and not nullable:
            value = f"{expression}.ok_or_else(|| missing({json.dumps(context)}))?"
            return self._decode_value(namespace, base, value, context)
        inner = self._decode_value(namespace, base, "value", context)
        return (
            f"{expression}.map(|value| Ok::<_, Status>({inner})).transpose()?"
        )

    def _encode_field(
        self,
        namespace: str,
        owner: str,
        json_name: str,
        schema: Json,
        expression: str,
        required: bool,
        *,
        proto_required: bool | None = None,
        proto_schema: Json | None = None,
    ) -> str:
        proto_required = required if proto_required is None else proto_required
        base, nullable = self.schema._without_null(schema)
        proto_base, _proto_nullable = self.schema._without_null(
            schema if proto_schema is None else proto_schema
        )
        context = f"{owner}.{json_name}"
        if self._is_double_option(owner, json_name):
            return self._encode_double_option(
                namespace, owner, json_name, base, expression
            )
        if direct_message := DIRECT_MESSAGE_FIELDS.get((owner, json_name)):
            rust_type, proto_type, _default = direct_message
            return (
                f"Some(<{rust_type} as DirectSchemaProto<proto::{proto_type}>>::"
                f"encode_schema({expression})?)"
            )
        if rust_type := RUST_TRANSPARENT_LIST_FIELDS.get((owner, json_name)):
            return self._encode_transparent_list_field(
                namespace,
                owner,
                json_name,
                base,
                expression,
                required,
                proto_required,
                rust_type,
            )
        string_newtype = self._string_newtype(namespace, base)
        if string_newtype is not None and self._is_scalar_string(
            namespace, proto_base
        ):
            if proto_required:
                return self._encode_string_newtype(
                    string_newtype, expression
                )
            if required:
                encoded = self._encode_string_newtype(
                    string_newtype, expression
                )
                return f"Some({encoded})"
            encoded = self._encode_string_newtype(string_newtype, "value")
            return (
                f"{expression}.map(|value| Ok::<_, Status>({encoded}))"
                ".transpose()?"
            )
        if not required and isinstance(schema, dict) and "default" in schema:
            default = schema["default"]
            scalar = self.schema._resolved_scalar_type(base, namespace)
            if scalar is not None and isinstance(
                default, (str, bool, int, float)
            ):
                if self._is_message(base, namespace) and "$ref" in base:
                    ref_namespace, key, proto_type = self._ref_info(
                        namespace, base["$ref"]
                    )
                    encoded = (
                        f"<{self._rust_type(ref_namespace, key)} as "
                        f"DirectSchemaProto<proto::{proto_type}>>::encode_schema({expression})?"
                    )
                    return f"Some({encoded})"
                encoded = self._encode_scalar(
                    scalar, expression, context
                )
                return f"Some({encoded})"
        if base.get("type") == "array":
            item = base.get("items", {})
            if proto_required:
                return self._encode_vec(namespace, item, expression, context)
            if not nullable:
                values = self._encode_vec(namespace, item, expression, context)
                wrapper = self.schema._ensure_inline(
                    f"{owner}{proto_pascal(json_name)}List",
                    {"type": "array", "items": item},
                    namespace,
                )
                return f"Some(proto::{wrapper} {{ values: {values} }})"
            values = self._encode_vec(namespace, item, "values", context)
            wrapper = self.schema._ensure_inline(
                f"{owner}{proto_pascal(json_name)}List",
                {"type": "array", "items": item},
                namespace,
            )
            return (
                f"{expression}.map(|values| Ok::<_, Status>(proto::{wrapper} {{ values: {values} }}))"
                ".transpose()?"
            )
        if base.get("type") == "object" and (
            "additionalProperties" in base and not base.get("properties")
        ):
            value_schema = (
                base["additionalProperties"]
                if isinstance(base["additionalProperties"], dict)
                else {}
            )
            wrapper = self.schema._ensure_inline(
                f"{owner}{proto_pascal(json_name)}Map", base, namespace
            )
            if not nullable:
                values = self._encode_map(
                    namespace, value_schema, expression, context
                )
                return f"Some(proto::{wrapper} {{ values: {values} }})"
            values = self._encode_map(namespace, value_schema, "values", context)
            return (
                f"{expression}.map(|values| Ok::<_, Status>(proto::{wrapper} {{ values: {values} }}))"
                ".transpose()?"
            )
        encoded = self._encode_value(namespace, base, expression, context)
        is_message = self._is_message(base, namespace)
        if is_message:
            if required and not nullable:
                return f"Some({encoded})"
            encoded_value = self._encode_value(namespace, base, "value", context)
            return (
                f"{expression}.map(|value| Ok::<_, Status>({encoded_value})).transpose()?"
            )
        if proto_required:
            return encoded
        if not nullable:
            return f"Some({encoded})"
        if required and not nullable:
            return f"Some({encoded})"
        encoded_value = self._encode_value(namespace, base, "value", context)
        return (
            f"{expression}.map(|value| Ok::<_, Status>({encoded_value})).transpose()?"
        )

    def _decode_value(
        self, namespace: str, schema: Json, expression: str, context: str
    ) -> str:
        base, _nullable = self.schema._without_null(schema)
        if "$ref" in base:
            ref_namespace, key, proto_type = self._ref_info(namespace, base["$ref"])
            ref_schema = self.schema._definition(ref_namespace, key)
            ref_base, _ref_nullable = self.schema._without_null(ref_schema)
            if (
                key in RUST_STRING_NEW_TYPES
                and self.schema._resolved_scalar_type(ref_base, ref_namespace)
                == "string"
            ):
                return self._decode_string_newtype(
                    key, f"{expression}.value", context
                )
            return (
                f"<{self._rust_type(ref_namespace, key)} as "
                f"DirectSchemaProto<proto::{proto_type}>>::decode_schema({expression})?"
            )
        if "allOf" in base and len(base["allOf"]) == 1:
            return self._decode_value(
                namespace, base["allOf"][0], expression, context
            )
        scalar = self.schema._resolved_scalar_type(base, namespace)
        if scalar is not None:
            return self._decode_scalar(scalar, expression, context)
        if base.get("type") == "array":
            return self._decode_vec(
                namespace, base.get("items", {}), expression, context
            )
        if not base or self.schema._scalar_type(base) == "DynamicValue":
            return (
                "super::super::grpc_api_conversions::"
                f"decode_dynamic_value({expression})?"
            )
        return (
            f"return Err(Status::unimplemented({json.dumps(f'direct decode for {context}')}))"
        )

    def _encode_value(
        self, namespace: str, schema: Json, expression: str, context: str
    ) -> str:
        base, _nullable = self.schema._without_null(schema)
        if "$ref" in base:
            ref_namespace, key, proto_type = self._ref_info(namespace, base["$ref"])
            ref_schema = self.schema._definition(ref_namespace, key)
            ref_base, _ref_nullable = self.schema._without_null(ref_schema)
            if (
                key in RUST_STRING_NEW_TYPES
                and self.schema._resolved_scalar_type(ref_base, ref_namespace)
                == "string"
            ):
                value = self._encode_string_newtype(key, expression)
                return f"proto::{proto_type} {{ value: {value} }}"
            return (
                f"<{self._rust_type(ref_namespace, key)} as "
                f"DirectSchemaProto<proto::{proto_type}>>::encode_schema({expression})?"
            )
        if "allOf" in base and len(base["allOf"]) == 1:
            return self._encode_value(
                namespace, base["allOf"][0], expression, context
            )
        scalar = self.schema._resolved_scalar_type(base, namespace)
        if scalar is not None:
            return self._encode_scalar(scalar, expression, context)
        if base.get("type") == "array":
            return self._encode_vec(
                namespace, base.get("items", {}), expression, context
            )
        if not base or self.schema._scalar_type(base) == "DynamicValue":
            return (
                "super::super::grpc_api_conversions::"
                f"encode_dynamic_value({expression})?"
            )
        return (
            f"return Err(Status::unimplemented({json.dumps(f'direct encode for {context}')}))"
        )

    def _decode_scalar(self, scalar: str, expression: str, context: str) -> str:
        if scalar == "string":
            return f"decode_string({expression}, {json.dumps(context)})?"
        if scalar == "int64":
            return f"decode_integer({expression}, {json.dumps(context)})?"
        if scalar == "uint64":
            return f"decode_unsigned({expression}, {json.dumps(context)})?"
        return expression

    def _encode_scalar(self, scalar: str, expression: str, context: str) -> str:
        if scalar == "string":
            return f"encode_string({expression})"
        if scalar == "int64":
            return f"encode_integer({expression}, {json.dumps(context)})?"
        if scalar == "uint64":
            return f"encode_unsigned({expression}, {json.dumps(context)})?"
        return expression

    def _decode_vec(
        self, namespace: str, item_schema: Json, expression: str, context: str
    ) -> str:
        item = self._decode_value(namespace, item_schema, "value", f"{context}[]")
        return (
            f"{expression}.into_iter().map(|value| Ok::<_, Status>({item}))"
            ".collect::<Result<Vec<_>, Status>>()?"
        )

    def _encode_vec(
        self, namespace: str, item_schema: Json, expression: str, context: str
    ) -> str:
        item = self._encode_value(namespace, item_schema, "value", f"{context}[]")
        return (
            f"{expression}.into_iter().map(|value| Ok::<_, Status>({item}))"
            ".collect::<Result<Vec<_>, Status>>()?"
        )

    def _decode_map(
        self, namespace: str, value_schema: Json, expression: str, context: str
    ) -> str:
        value = self._decode_value(
            namespace, value_schema, "value", f"{context}{{value}}"
        )
        return (
            f"{expression}.into_iter().map(|(key, value)| "
            f"Ok::<_, Status>((decode_string(key, {json.dumps(context)})?, {value})))"
            ".collect::<Result<_, Status>>()?"
        )

    def _encode_map(
        self, namespace: str, value_schema: Json, expression: str, context: str
    ) -> str:
        value = self._encode_value(
            namespace, value_schema, "value", f"{context}{{value}}"
        )
        return (
            f"{expression}.into_iter().map(|(key, value)| "
            f"Ok::<_, Status>((encode_string(key), {value})))"
            ".collect::<Result<_, Status>>()?"
        )

    def _decode_transparent_list_field(
        self,
        namespace: str,
        owner: str,
        json_name: str,
        schema: Json,
        expression: str,
        required: bool,
        proto_required: bool,
        rust_type: str,
    ) -> str:
        item_schema = schema.get("items", {})
        item_type = self._proto_rust_scalar_type(item_schema, namespace)
        proto_type = f"Vec<{item_type}>"
        decode = (
            f"<{rust_type} as DirectSchemaProto<{proto_type}>>::decode_schema"
        )
        if proto_required:
            return f"{decode}({expression})?"
        if required:
            values = (
                f"{expression}.ok_or_else(|| "
                f"missing({json.dumps(f'{owner}.{json_name}')}))?.values"
            )
            return f"{decode}({values})?"
        return (
            f"{expression}.map(|wrapper| {decode}(wrapper.values))"
            ".transpose()?"
        )

    def _encode_transparent_list_field(
        self,
        namespace: str,
        owner: str,
        json_name: str,
        schema: Json,
        expression: str,
        required: bool,
        proto_required: bool,
        rust_type: str,
    ) -> str:
        item_schema = schema.get("items", {})
        item_type = self._proto_rust_scalar_type(item_schema, namespace)
        proto_type = f"Vec<{item_type}>"
        encode = (
            f"<{rust_type} as DirectSchemaProto<{proto_type}>>::encode_schema"
        )
        if proto_required:
            return f"{encode}({expression})?"
        wrapper = self.schema._ensure_inline(
            f"{owner}{proto_pascal(json_name)}List",
            {"type": "array", "items": item_schema},
            namespace,
        )
        if required:
            return (
                f"Some(proto::{wrapper} {{ values: {encode}({expression})? }})"
            )
        return (
            f"{expression}.map(|value| Ok::<_, Status>(proto::{wrapper} {{ "
            f"values: {encode}(value)? }})).transpose()?"
        )

    def _proto_rust_scalar_type(self, schema: Json, namespace: str) -> str:
        scalar = self.schema._resolved_scalar_type(schema, namespace)
        return {
            "string": "String",
            "bool": "bool",
            "int64": "i64",
            "uint64": "u64",
            "double": "f64",
        }.get(scalar or "", "proto::DynamicValue")

    def _string_newtype(self, namespace: str, schema: Json) -> str | None:
        base, _nullable = self.schema._without_null(schema)
        if "allOf" in base and len(base["allOf"]) == 1:
            return self._string_newtype(namespace, base["allOf"][0])
        if "$ref" not in base:
            return None
        ref_namespace, key, _proto_type = self._ref_info(namespace, base["$ref"])
        ref_schema = self.schema._definition(ref_namespace, key)
        ref_base, _ref_nullable = self.schema._without_null(ref_schema)
        if (
            key in RUST_STRING_NEW_TYPES
            and self.schema._resolved_scalar_type(ref_base, ref_namespace)
            == "string"
        ):
            return key
        return None

    def _is_scalar_string(self, namespace: str, schema: Json) -> bool:
        return (
            not self._is_message(schema, namespace)
            and self.schema._resolved_scalar_type(schema, namespace) == "string"
        )

    def _decode_string_newtype(
        self, key: str, expression: str, context: str
    ) -> str:
        if key == "ReasoningEffort":
            return (
                f"{expression}.parse()"
                f".map_err(|error| invalid({json.dumps(context)}, error))?"
            )
        return (
            f"decode_newtype_string({expression}, {json.dumps(context)})?"
        )

    def _encode_string_newtype(self, key: str, expression: str) -> str:
        if key == "AbsolutePathBuf":
            return f"{expression}.as_path().to_string_lossy().into_owned()"
        return f"{expression}.to_string()"

    def _decode_double_option(
        self,
        namespace: str,
        owner: str,
        json_name: str,
        schema: Json,
        expression: str,
    ) -> str:
        value_type = self.schema._value_type(
            schema, f"{owner}{proto_pascal(json_name)}Value", namespace
        )
        wrapper = self.schema._ensure_nullable_wrapper(value_type)
        context = f"{owner}.{json_name}"
        if wrapper == "NullableString":
            inner = self._decode_value(namespace, schema, "value", context)
            return (
                f"match {expression} {{ None => None, Some(wrapper) => "
                f"Some(wrapper.value.map(|value| Ok::<_, Status>({inner})).transpose()?) }}"
            )
        module = proto_snake(wrapper)
        inner = self._decode_value(namespace, schema, "value", context)
        return (
            f"match {expression} {{ None => None, Some(wrapper) => Some(match "
            f"wrapper.value.ok_or_else(|| missing({json.dumps(context)}))? {{ "
            f"proto::{module}::Value::Some(value) => Some({inner}), "
            f"proto::{module}::Value::Null(_) => None }}) }}"
        )

    def _encode_double_option(
        self,
        namespace: str,
        owner: str,
        json_name: str,
        schema: Json,
        expression: str,
    ) -> str:
        value_type = self.schema._value_type(
            schema, f"{owner}{proto_pascal(json_name)}Value", namespace
        )
        wrapper = self.schema._ensure_nullable_wrapper(value_type)
        context = f"{owner}.{json_name}"
        inner = self._encode_value(namespace, schema, "value", context)
        if wrapper == "NullableString":
            return (
                f"{expression}.map(|value| Ok::<_, Status>(proto::{wrapper} {{ value: "
                f"value.map(|value| Ok::<_, Status>({inner})).transpose()? }})).transpose()?"
            )
        module = proto_snake(wrapper)
        return (
            f"{expression}.map(|value| Ok::<_, Status>(proto::{wrapper} {{ value: Some(match value {{ "
            f"Some(value) => proto::{module}::Value::Some({inner}), "
            f"None => proto::{module}::Value::Null(proto::Empty {{}}) }}) }})).transpose()?"
        )

    def _is_message(self, schema: Json, namespace: str) -> bool:
        base, _nullable = self.schema._without_null(schema)
        if "$ref" in base:
            return True
        if "allOf" in base and len(base["allOf"]) == 1:
            return self._is_message(base["allOf"][0], namespace)
        if self.schema._resolved_scalar_type(base, namespace) is not None:
            return False
        return True

    def _ref_info(self, namespace: str, ref: str) -> tuple[str, str, str]:
        prefix = "#/definitions/"
        if not ref.startswith(prefix):
            raise RuntimeError(f"unsupported external schema reference {ref}")
        key = ref.removeprefix(prefix)
        ref_namespace = namespace
        if key.startswith("v2/"):
            ref_namespace = "v2"
            key = key.removeprefix("v2/")
        proto_type = self.schema.ensure_named(ref_namespace, key)
        return ref_namespace, key, proto_type

    def _rust_type(self, namespace: str, key: str) -> str:
        return RUST_TYPE_PATHS.get(
            (namespace, key),
            f"codex_app_server_protocol::{key}",
        )

    def _is_double_option(self, owner: str, json_name: str) -> bool:
        return (owner, json_name) in DOUBLE_OPTION_FIELDS or (
            f"V2{owner}",
            json_name,
        ) in DOUBLE_OPTION_FIELDS

    def _rust_literal(self, value: Any) -> str:
        if value is True:
            return "true"
        if value is False:
            return "false"
        if isinstance(value, str):
            return f"{json.dumps(value)}.to_owned()"
        return str(value)


def render_native_impls(
    type_pairs: dict[str, str], nullable_wrappers: dict[str, str]
) -> str:
    if _ACTIVE_SCHEMA is None:
        raise RuntimeError("SchemaProto must be constructed before native conversions")
    return NativeRenderer(_ACTIVE_SCHEMA, type_pairs, nullable_wrappers).render()
