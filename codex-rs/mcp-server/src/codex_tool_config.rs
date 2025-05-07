//! Configuration object accepted by the `codex` MCP tool-call.

use std::path::PathBuf;

use mcp_types::Tool;
use mcp_types::ToolInputSchema;
use schemars::JsonSchema;
use schemars::r#gen::SchemaSettings;
use serde::Deserialize;

use codex_core::protocol::AskForApproval;
use codex_core::protocol::SandboxPolicy;

/// Client-supplied configuration for a `codex` tool-call.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct CodexToolCallParam {
    /// The *initial user prompt* to start the Codex conversation.
    pub prompt: String,

    /// Optional override for the model name (e.g. "o3", "o4-mini")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Working directory for the session. If relative, it is resolved against
    /// the server process's current working directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,

    /// Execution approval policy expressed as the kebab-case variant name
    /// (`unless-allow-listed`, `auto-edit`, `on-failure`, `never`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<CodexToolCallApprovalPolicy>,

    /// Sandbox permissions using the same string values accepted by the CLI
    /// (e.g. "disk-write-cwd", "network-full-access").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox_permissions: Option<Vec<CodexToolCallSandboxPermission>>,

    /// Disable server-side response storage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_response_storage: Option<bool>,
    // Custom system instructions.
    // #[serde(default, skip_serializing_if = "Option::is_none")]
    // pub instructions: Option<String>,
}

// Create custom enums for use with `CodexToolCallApprovalPolicy` where we
// intentionally exclude docstrings from the generated schema because they
// introduce anyOf in the the generated JSON schema, which makes it more complex
// without adding any real value since we aspire to use self-descriptive names.

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CodexToolCallApprovalPolicy {
    AutoEdit,
    UnlessAllowListed,
    OnFailure,
    Never,
}

impl From<CodexToolCallApprovalPolicy> for AskForApproval {
    fn from(value: CodexToolCallApprovalPolicy) -> Self {
        match value {
            CodexToolCallApprovalPolicy::AutoEdit => AskForApproval::AutoEdit,
            CodexToolCallApprovalPolicy::UnlessAllowListed => AskForApproval::UnlessAllowListed,
            CodexToolCallApprovalPolicy::OnFailure => AskForApproval::OnFailure,
            CodexToolCallApprovalPolicy::Never => AskForApproval::Never,
        }
    }
}

// TODO: Support additional writable folders via a separate property on
// CodexToolCallParam.

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CodexToolCallSandboxPermission {
    DiskFullReadAccess,
    DiskWriteCwd,
    DiskWritePlatformUserTempFolder,
    DiskWritePlatformGlobalTempFolder,
    DiskFullWriteAccess,
    NetworkFullAccess,
}

impl From<CodexToolCallSandboxPermission> for codex_core::protocol::SandboxPermission {
    fn from(value: CodexToolCallSandboxPermission) -> Self {
        match value {
            CodexToolCallSandboxPermission::DiskFullReadAccess => {
                codex_core::protocol::SandboxPermission::DiskFullReadAccess
            }
            CodexToolCallSandboxPermission::DiskWriteCwd => {
                codex_core::protocol::SandboxPermission::DiskWriteCwd
            }
            CodexToolCallSandboxPermission::DiskWritePlatformUserTempFolder => {
                codex_core::protocol::SandboxPermission::DiskWritePlatformUserTempFolder
            }
            CodexToolCallSandboxPermission::DiskWritePlatformGlobalTempFolder => {
                codex_core::protocol::SandboxPermission::DiskWritePlatformGlobalTempFolder
            }
            CodexToolCallSandboxPermission::DiskFullWriteAccess => {
                codex_core::protocol::SandboxPermission::DiskFullWriteAccess
            }
            CodexToolCallSandboxPermission::NetworkFullAccess => {
                codex_core::protocol::SandboxPermission::NetworkFullAccess
            }
        }
    }
}

pub(crate) fn create_tool_for_codex_tool_call_param() -> Tool {
    let schema = SchemaSettings::draft2019_09()
        .with(|s| {
            s.inline_subschemas = true;
            s.option_add_null_type = false
        })
        .into_generator()
        .into_root_schema_for::<CodexToolCallParam>();
    let schema_value =
        serde_json::to_value(&schema).expect("Codex tool schema should serialise to JSON");

    let tool_input_schema =
        serde_json::from_value::<ToolInputSchema>(schema_value).unwrap_or_else(|e| {
            panic!("failed to create Tool from schema: {e}");
        });
    Tool {
        name: "codex".to_string(),
        input_schema: tool_input_schema,
        description: Some(
            "Run a Codex session. Accepts configuration parameters matching the Codex Config struct."
                .to_string(),
        ),
        annotations: None,
    }
}

impl CodexToolCallParam {
    /// Returns the initial user prompt to start the Codex conversation and the
    /// Config.
    pub fn into_config(self) -> std::io::Result<(String, codex_core::config::Config)> {
        let Self {
            prompt,
            model,
            cwd,
            approval_policy,
            sandbox_permissions,
            disable_response_storage,
        } = self;
        let sandbox_policy = sandbox_permissions.map(|perms| {
            SandboxPolicy::from(perms.into_iter().map(Into::into).collect::<Vec<_>>())
        });

        // Build ConfigOverrides recognised by codex-core.
        let overrides = codex_core::config::ConfigOverrides {
            model,
            cwd: cwd.map(PathBuf::from),
            approval_policy: approval_policy.map(Into::into),
            sandbox_policy,
            disable_response_storage,
        };

        let cfg = codex_core::config::Config::load_with_overrides(overrides)?;

        Ok((prompt, cfg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// We include a test to verify the exact JSON schema as "executable
    /// documentation" for the schema. When can track changes to this test as a
    /// way to audit changes to the generated schema.
    ///
    /// Seeing the fully expanded schema makes it easier to casually verify that
    /// the generated JSON for enum types such as "approval-policy" is compact.
    /// Ideally, modelcontextprotocol/inspector would provide a simpler UI for
    /// enum fields versus open string fields to take advantage of this.
    ///
    /// As of 2025-05-04, there is an open PR for this:
    /// https://github.com/modelcontextprotocol/inspector/pull/196
    #[test]
    fn verify_codex_tool_json_schema() {
        let tool = create_tool_for_codex_tool_call_param();
        let tool_json = serde_json::to_value(&tool).expect("tool serializes");
        let expected_tool_json = serde_json::json!({
          "name": "codex",
          "description": "Run a Codex session. Accepts configuration parameters matching the Codex Config struct.",
          "inputSchema": {
            "type": "object",
            "properties": {
              "approval-policy": {
                "description": "Execution approval policy expressed as the kebab-case variant name (`unless-allow-listed`, `auto-edit`, `on-failure`, `never`).",
                "enum": [
                  "auto-edit",
                  "unless-allow-listed",
                  "on-failure",
                  "never"
                ],
                "type": "string"
              },
              "cwd": {
                "description": "Working directory for the session. If relative, it is resolved against the server process's current working directory.",
                "type": "string"
              },
              "disable-response-storage": {
                "description": "Disable server-side response storage.",
                "type": "boolean"
              },
              "model": {
                "description": "Optional override for the model name (e.g. \"o3\", \"o4-mini\")",
                "type": "string"
              },
              "prompt": {
                "description": "The *initial user prompt* to start the Codex conversation.",
                "type": "string"
              },
              "sandbox-permissions": {
                "description": "Sandbox permissions using the same string values accepted by the CLI (e.g. \"disk-write-cwd\", \"network-full-access\").",
                "items": {
                  "enum": [
                    "disk-full-read-access",
                    "disk-write-cwd",
                    "disk-write-platform-user-temp-folder",
                    "disk-write-platform-global-temp-folder",
                    "disk-full-write-access",
                    "network-full-access"
                  ],
                  "type": "string"
                },
                "type": "array"
              }
            },
            "required": [
              "prompt"
            ]
          }
        });
        assert_eq!(expected_tool_json, tool_json);
    }
}
