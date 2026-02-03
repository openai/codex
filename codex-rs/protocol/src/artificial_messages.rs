//! Artificial message helpers shared between prompt construction and protocol parsing.
//!
//! The `artificial_messages!` macro invocation in this module defines:
//!
//! - The `ArtificialMessage` enum variants declared below (`Skill`, `ModelWarning`,
//!   `Permission`, `UserShellCommand`, `CollaborationMode`, `PersonalitySpec`,
//!   `ExecPolicyAmendment`) with their configured string fields.
//! - `ArtificialMessage::tag(&self) -> &'static str`: returns the top-level XML tag for
//!   the current variant (for example `skill`).
//! - `ArtificialMessage::role(&self) -> &'static str`: returns the role that should be
//!   used when this message is emitted as a `ResponseItem`.
//! - `ArtificialMessage::render(&self) -> String`: serializes the message to the tagged
//!   XML-like envelope.
//! - `ArtificialMessage::parse(input: &str) -> Result<Self, ArtificialMessageParseError>`:
//!   parses an envelope back into an enum variant.
//! - `ArtificialMessage::detect_tag(input: &str) -> Option<&'static str>`: returns the
//!   known artificial-message tag if the input is a recognized envelope.
//! - `ArtificialMessage::is_artificial(input: &str) -> bool`: convenience boolean check
//!   built on top of `detect_tag`.
//! - `ArtificialMessage::to_response_item(&self) -> ResponseItem`: converts the rendered
//!   content into a message-shaped `ResponseItem`.
//!
//! This module also provides `impl From<ArtificialMessage> for ResponseItem` as a
//! convenience wrapper around `to_response_item`.
//!
use crate::models::ContentItem;
use crate::models::ResponseItem;
use thiserror::Error;

#[macro_use]
mod macros;

pub const TAG_SKILL: &str = "skill";
pub const TAG_MODEL_WARNING: &str = "warning";
pub const TAG_USER_SHELL_COMMAND: &str = "user_shell_cmd";
pub const TAG_PERMISSION: &str = "permissions_instructions";
pub const TAG_COLLABORATION_MODE: &str = "collaboration_mode";
pub const TAG_SPEC: &str = "personality_spec";
pub const TAG_EXEC_POLICY_AMENDMENT: &str = "exec_policy_amendment";

artificial_messages! {
    Skill {
        tag: TAG_SKILL,
        role: "user",
        fields: {
            tagged(name, "name"),
            tagged(path, "path"),
            raw(body)
        }
    },
    ModelWarning {
        tag: TAG_MODEL_WARNING,
        role: "user",
        fields: {
            raw(body)
        }
    },
    Permission {
        tag: TAG_PERMISSION,
        role: "developer",
        fields: {
            raw(body)
        }
    },
    UserShellCommand {
        tag: TAG_USER_SHELL_COMMAND,
        role: "user",
        fields: {
            raw(body)
        }
    },
    CollaborationMode {
        tag: TAG_COLLABORATION_MODE,
        role: "developer",
        fields: {
            raw(body)
        }
    },
    PersonalitySpec {
        tag: TAG_SPEC,
        role: "developer",
        fields: {
            raw(body)
        }
    },
    ExecPolicyAmendment {
        tag: TAG_EXEC_POLICY_AMENDMENT,
        role: "developer",
        fields: {
            raw(body)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ArtificialMessageParseError {
    #[error("invalid artificial message envelope")]
    InvalidEnvelope,
    #[error("unknown artificial message tag: {0}")]
    UnknownTopLevelTag(String),
    #[error("missing field <{field_tag}> in message <{message_tag}>")]
    MissingField {
        message_tag: &'static str,
        field_tag: &'static str,
    },
}

impl From<ArtificialMessage> for ResponseItem {
    fn from(value: ArtificialMessage) -> Self {
        value.to_response_item()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn render_and_parse_skill_round_trip() {
        let message = ArtificialMessage::Skill {
            name: "demo".to_string(),
            path: "skills/demo/SKILL.md".to_string(),
            body: "body".to_string(),
        };

        let rendered = message.render();
        assert_eq!(
            rendered,
            "<skill>\n<name>demo</name>\n<path>skills/demo/SKILL.md</path>\nbody\n</skill>"
        );

        let parsed = ArtificialMessage::parse(&rendered).expect("parse skill");
        assert_eq!(parsed, message);
    }

    #[test]
    fn render_and_parse_warning_round_trip() {
        let message = ArtificialMessage::ModelWarning {
            body: "be careful".to_string(),
        };
        let rendered = message.render();
        assert_eq!(rendered, "<warning>be careful</warning>");

        let parsed = ArtificialMessage::parse(&rendered).expect("parse warning");
        assert_eq!(parsed, message);
    }

    #[test]
    fn render_and_parse_permission_round_trip() {
        let message = ArtificialMessage::Permission {
            body: "policy text".to_string(),
        };
        let rendered = message.render();
        assert_eq!(
            rendered,
            "<permissions_instructions>policy text</permissions_instructions>"
        );

        let parsed = ArtificialMessage::parse(&rendered).expect("parse permission");
        assert_eq!(parsed, message);
    }

    #[test]
    fn detect_tag_returns_known_tag() {
        let text = "<permissions_instructions>abc</permissions_instructions>";
        assert_eq!(ArtificialMessage::detect_tag(text), Some(TAG_PERMISSION));
        assert!(ArtificialMessage::is_artificial(text));
    }

    #[test]
    fn parse_rejects_unknown_tag() {
        let text = "<unknown>abc</unknown>";
        let err = ArtificialMessage::parse(text).expect_err("unknown tag should fail");
        assert_eq!(
            err,
            ArtificialMessageParseError::UnknownTopLevelTag("unknown".to_string())
        );
    }

    #[test]
    fn render_and_parse_user_shell_command_round_trip() {
        let message = ArtificialMessage::UserShellCommand {
            body: "<command>echo hi</command>".to_string(),
        };
        let rendered = message.render();
        assert_eq!(
            rendered,
            "<user_shell_cmd><command>echo hi</command></user_shell_cmd>"
        );

        let parsed = ArtificialMessage::parse(&rendered).expect("parse user shell command");
        assert_eq!(parsed, message);
    }

    #[test]
    fn render_and_parse_collaboration_mode_round_trip() {
        let message = ArtificialMessage::CollaborationMode {
            body: "plan first".to_string(),
        };
        let rendered = message.render();
        assert_eq!(
            rendered,
            "<collaboration_mode>plan first</collaboration_mode>"
        );

        let parsed = ArtificialMessage::parse(&rendered).expect("parse collaboration mode");
        assert_eq!(parsed, message);
    }

    #[test]
    fn render_and_parse_personality_spec_round_trip() {
        let message = ArtificialMessage::PersonalitySpec {
            body: "be pragmatic".to_string(),
        };
        let rendered = message.render();
        assert_eq!(
            rendered,
            "<personality_spec>be pragmatic</personality_spec>"
        );

        let parsed = ArtificialMessage::parse(&rendered).expect("parse personality spec");
        assert_eq!(parsed, message);
    }

    #[test]
    fn render_and_parse_exec_policy_amendment_round_trip() {
        let message = ArtificialMessage::ExecPolicyAmendment {
            body: "Approved command prefix saved:\n- `echo`".to_string(),
        };
        let rendered = message.render();
        assert_eq!(
            rendered,
            "<exec_policy_amendment>Approved command prefix saved:\n- `echo`</exec_policy_amendment>"
        );

        let parsed = ArtificialMessage::parse(&rendered).expect("parse exec policy amendment");
        assert_eq!(parsed, message);
    }
}
