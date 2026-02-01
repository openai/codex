//! Attachment configuration.
//!
//! Defines settings for response attachments.

use serde::Deserialize;
use serde::Serialize;

/// Attachment configuration.
///
/// Controls which attachments are included in responses.
///
/// # Environment Variables
///
/// - `COCODE_DISABLE_ATTACHMENTS`: Disable all attachments
/// - `COCODE_ENABLE_TOKEN_USAGE_ATTACHMENT`: Enable token usage attachment
///
/// # Example
///
/// ```json
/// {
///   "attachment": {
///     "disable_attachments": false,
///     "enable_token_usage_attachment": true
///   }
/// }
/// ```
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AttachmentConfig {
    /// Disable all attachments.
    #[serde(default)]
    pub disable_attachments: bool,

    /// Enable token usage attachment in responses.
    #[serde(default)]
    pub enable_token_usage_attachment: bool,
}

impl Default for AttachmentConfig {
    fn default() -> Self {
        Self {
            disable_attachments: false,
            enable_token_usage_attachment: false,
        }
    }
}

impl AttachmentConfig {
    /// Check if attachments are enabled.
    pub fn are_attachments_enabled(&self) -> bool {
        !self.disable_attachments
    }

    /// Check if token usage attachment should be included.
    pub fn should_include_token_usage(&self) -> bool {
        !self.disable_attachments && self.enable_token_usage_attachment
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attachment_config_default() {
        let config = AttachmentConfig::default();
        assert!(!config.disable_attachments);
        assert!(!config.enable_token_usage_attachment);
    }

    #[test]
    fn test_attachment_config_serde() {
        let json = r#"{"disable_attachments": true, "enable_token_usage_attachment": true}"#;
        let config: AttachmentConfig = serde_json::from_str(json).unwrap();
        assert!(config.disable_attachments);
        assert!(config.enable_token_usage_attachment);
    }

    #[test]
    fn test_attachment_config_serde_defaults() {
        let json = r#"{}"#;
        let config: AttachmentConfig = serde_json::from_str(json).unwrap();
        assert!(!config.disable_attachments);
        assert!(!config.enable_token_usage_attachment);
    }

    #[test]
    fn test_are_attachments_enabled() {
        let mut config = AttachmentConfig::default();
        assert!(config.are_attachments_enabled());

        config.disable_attachments = true;
        assert!(!config.are_attachments_enabled());
    }

    #[test]
    fn test_should_include_token_usage() {
        let mut config = AttachmentConfig::default();
        assert!(!config.should_include_token_usage());

        config.enable_token_usage_attachment = true;
        assert!(config.should_include_token_usage());

        config.disable_attachments = true;
        assert!(!config.should_include_token_usage());
    }
}
