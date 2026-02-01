//! Sandbox mode configuration for command execution.

use serde::Deserialize;
use serde::Serialize;

/// Sandbox mode for command execution.
///
/// Defines the level of filesystem access allowed during command execution.
/// This is used to control what operations the agent can perform on the filesystem.
///
/// # Example
///
/// ```
/// use cocode_protocol::SandboxMode;
///
/// let mode = SandboxMode::default();
/// assert_eq!(mode, SandboxMode::ReadOnly);
///
/// // Parse from JSON
/// let mode: SandboxMode = serde_json::from_str("\"workspace-write\"").unwrap();
/// assert_eq!(mode, SandboxMode::WorkspaceWrite);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SandboxMode {
    /// Read-only access (safest).
    ///
    /// The agent can only read files but cannot modify anything on disk.
    /// This is the most restrictive and safest mode.
    #[default]
    ReadOnly,

    /// Write access to workspace directories only.
    ///
    /// The agent can write to files within the workspace directories
    /// (specified in `writable_roots`), but cannot modify files outside
    /// these directories.
    WorkspaceWrite,

    /// Full access (dangerous, use with caution).
    ///
    /// The agent has unrestricted filesystem access.
    /// Only use this mode when you trust the agent completely.
    FullAccess,
}

impl SandboxMode {
    /// Check if this mode allows any write operations.
    pub fn allows_write(&self) -> bool {
        !matches!(self, SandboxMode::ReadOnly)
    }

    /// Check if this mode has full unrestricted access.
    pub fn is_full_access(&self) -> bool {
        matches!(self, SandboxMode::FullAccess)
    }

    /// Get the mode as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            SandboxMode::ReadOnly => "read-only",
            SandboxMode::WorkspaceWrite => "workspace-write",
            SandboxMode::FullAccess => "full-access",
        }
    }
}

impl std::fmt::Display for SandboxMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for SandboxMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "read-only" | "readonly" | "read_only" => Ok(SandboxMode::ReadOnly),
            "workspace-write" | "workspacewrite" | "workspace_write" => {
                Ok(SandboxMode::WorkspaceWrite)
            }
            "full-access" | "fullaccess" | "full_access" => Ok(SandboxMode::FullAccess),
            _ => Err(format!(
                "unknown sandbox mode: '{}'. Expected: read-only, workspace-write, or full-access",
                s
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        assert_eq!(SandboxMode::default(), SandboxMode::ReadOnly);
    }

    #[test]
    fn test_allows_write() {
        assert!(!SandboxMode::ReadOnly.allows_write());
        assert!(SandboxMode::WorkspaceWrite.allows_write());
        assert!(SandboxMode::FullAccess.allows_write());
    }

    #[test]
    fn test_is_full_access() {
        assert!(!SandboxMode::ReadOnly.is_full_access());
        assert!(!SandboxMode::WorkspaceWrite.is_full_access());
        assert!(SandboxMode::FullAccess.is_full_access());
    }

    #[test]
    fn test_as_str() {
        assert_eq!(SandboxMode::ReadOnly.as_str(), "read-only");
        assert_eq!(SandboxMode::WorkspaceWrite.as_str(), "workspace-write");
        assert_eq!(SandboxMode::FullAccess.as_str(), "full-access");
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", SandboxMode::ReadOnly), "read-only");
        assert_eq!(
            format!("{}", SandboxMode::WorkspaceWrite),
            "workspace-write"
        );
        assert_eq!(format!("{}", SandboxMode::FullAccess), "full-access");
    }

    #[test]
    fn test_from_str() {
        // Primary format
        assert_eq!(
            "read-only".parse::<SandboxMode>().unwrap(),
            SandboxMode::ReadOnly
        );
        assert_eq!(
            "workspace-write".parse::<SandboxMode>().unwrap(),
            SandboxMode::WorkspaceWrite
        );
        assert_eq!(
            "full-access".parse::<SandboxMode>().unwrap(),
            SandboxMode::FullAccess
        );

        // Alternative formats
        assert_eq!(
            "readonly".parse::<SandboxMode>().unwrap(),
            SandboxMode::ReadOnly
        );
        assert_eq!(
            "read_only".parse::<SandboxMode>().unwrap(),
            SandboxMode::ReadOnly
        );
    }

    #[test]
    fn test_from_str_error() {
        let result = "invalid".parse::<SandboxMode>();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown sandbox mode"));
    }

    #[test]
    fn test_serde_roundtrip() {
        for mode in [
            SandboxMode::ReadOnly,
            SandboxMode::WorkspaceWrite,
            SandboxMode::FullAccess,
        ] {
            let json = serde_json::to_string(&mode).unwrap();
            let parsed: SandboxMode = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, mode);
        }
    }

    #[test]
    fn test_serde_kebab_case() {
        // Verify kebab-case serialization
        assert_eq!(
            serde_json::to_string(&SandboxMode::ReadOnly).unwrap(),
            "\"read-only\""
        );
        assert_eq!(
            serde_json::to_string(&SandboxMode::WorkspaceWrite).unwrap(),
            "\"workspace-write\""
        );
        assert_eq!(
            serde_json::to_string(&SandboxMode::FullAccess).unwrap(),
            "\"full-access\""
        );
    }

    #[test]
    fn test_serde_deserialize() {
        assert_eq!(
            serde_json::from_str::<SandboxMode>("\"read-only\"").unwrap(),
            SandboxMode::ReadOnly
        );
        assert_eq!(
            serde_json::from_str::<SandboxMode>("\"workspace-write\"").unwrap(),
            SandboxMode::WorkspaceWrite
        );
        assert_eq!(
            serde_json::from_str::<SandboxMode>("\"full-access\"").unwrap(),
            SandboxMode::FullAccess
        );
    }
}
