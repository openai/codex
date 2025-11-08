//! Codex-specific MCP tools for sub-agent delegation
//!
//! These tools allow sub-agents to call Codex capabilities via MCP protocol.

use serde_json::Value;

// Tool implementations
mod apply_patch;
mod codebase_search;
mod grep;
mod read_file;
mod shell;

#[cfg(feature = "cuda")]
pub mod cuda;

#[cfg(all(target_os = "macos", feature = "metal"))]
pub mod metal;

#[cfg(feature = "openxr")]
pub mod vr;

/// Codex MCP tool definitions for sub-agents
#[derive(Debug, Clone)]
pub struct CodexMcpTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

impl CodexMcpTool {
    /// Get all safe (read-only) tools
    pub fn safe_tools() -> Vec<Self> {
        vec![Self::read_file(), Self::grep(), Self::codebase_search()]
    }

    /// Get all tools (including write/shell)
    pub fn all_tools() -> Vec<Self> {
        let mut tools = vec![
            Self::read_file(),
            Self::grep(),
            Self::codebase_search(),
            Self::apply_patch(),
            Self::shell(),
        ];

        #[cfg(feature = "cuda")]
        {
            tools.push(Self::cuda_execute());
        }

        #[cfg(all(target_os = "macos", feature = "metal"))]
        {
            tools.push(Self::metal_execute());
        }

        #[cfg(feature = "openxr")]
        {
            tools.push(Self::vr_execute());
        }

        tools
    }

    /// CUDA GPU acceleration tool
    #[cfg(feature = "cuda")]
    pub fn cuda_execute() -> Self {
        use serde_json::json;

        Self {
            name: "codex_cuda_execute".to_string(),
            description: "Execute GPU-accelerated computation with CUDA".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["vec_add", "mat_mul", "custom"],
                        "description": "CUDA operation type"
                    },
                    "input_data": {
                        "type": "array",
                        "items": { "type": "number" },
                        "description": "Input data for computation"
                    },
                    "custom_code": {
                        "type": "string",
                        "description": "Custom CUDA kernel code (for 'custom' operation)"
                    }
                },
                "required": ["operation", "input_data"]
            }),
        }
    }

    /// Metal GPU acceleration tool (macOS)
    #[cfg(all(target_os = "macos", feature = "metal"))]
    pub fn metal_execute() -> Self {
        use serde_json::json;

        Self {
            name: "codex_metal_execute".to_string(),
            description: "Execute GPU-accelerated computation with Metal (macOS)".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["matrix_multiply", "neural_inference", "custom"],
                        "description": "Metal operation type"
                    },
                    "input_data": {
                        "type": "array",
                        "items": { "type": "number" },
                        "description": "Input data for computation"
                    },
                    "use_mps": {
                        "type": "boolean",
                        "description": "Use Metal Performance Shaders (MPS) if available"
                    }
                },
                "required": ["operation", "input_data"]
            }),
        }
    }

    /// VR device tool (OpenXR)
    #[cfg(feature = "openxr")]
    pub fn vr_execute() -> Self {
        use serde_json::json;

        Self {
            name: "codex_vr_execute".to_string(),
            description: "Execute VR/AR operations via OpenXR".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["get_stats", "render_frame", "track_pose"],
                        "description": "VR operation type"
                    },
                    "device_id": {
                        "type": "integer",
                        "description": "VR device ID (0 for primary device)"
                    }
                },
                "required": ["operation"]
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codex_tools_defined() {
        let safe_tools = CodexMcpTool::safe_tools();
        assert_eq!(safe_tools.len(), 3);
        assert_eq!(safe_tools[0].name, "codex_read_file");

        let all_tools = CodexMcpTool::all_tools();
        assert_eq!(all_tools.len(), 5);
    }
}
