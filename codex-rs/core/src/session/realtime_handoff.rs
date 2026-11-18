//! Routes realtime input through ordinary turn admission.

use super::session::Session;
use super::turn_input;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::turn_input::NotSubmittedReason;
use codex_protocol::turn_input::TurnInputMode;
use codex_protocol::turn_input::TurnInputRequest;
use codex_protocol::turn_input::TurnInputSubmission;
use codex_protocol::turn_input::TurnStartOptions;
use codex_user_input::ResolvedUserInput;
use std::sync::Arc;
use uuid::Uuid;

impl Session {
    pub(crate) async fn route_realtime_text_input(
        self: &Arc<Self>,
        text: String,
    ) -> Result<(), &'static str> {
        let submission_id = Uuid::now_v7().to_string();
        let submission = turn_input::handle(
            self,
            TurnInputRequest::user_input(vec![ResolvedUserInput::text(text)]).on_start(
                TurnStartOptions {
                    turn_trigger: Some("realtime".to_string()),
                    ..Default::default()
                },
            ),
            TurnInputMode::StartOrSteer,
            submission_id.clone(),
        )
        .await;
        match submission {
            Ok(TurnInputSubmission::Accepted) => {
                unreachable!("realtime user input returns a turn outcome")
            }
            Ok(TurnInputSubmission::Started { .. } | TurnInputSubmission::Steered { .. }) => {}
            Ok(TurnInputSubmission::NotSubmitted {
                reason: NotSubmittedReason::ServerDraining,
            }) => {
                return Err("Server is draining; retry the turn after reconnecting");
            }
            Ok(TurnInputSubmission::NotSubmitted { reason }) => {
                self.send_event_raw(Event {
                    id: submission_id,
                    msg: EventMsg::Error(ErrorEvent {
                        misalignment: None,
                        message: format!("failed to submit turn input: {reason:?}"),
                        codex_error_info: Some(CodexErrorInfo::BadRequest),
                        rejected_audio_file_ids: Vec::new(),
                    }),
                })
                .await;
            }
            Err(error) => {
                self.send_event_raw(Event {
                    id: submission_id,
                    msg: EventMsg::Error(error.to_error_event(/*message_prefix*/ None)),
                })
                .await;
            }
        }
        Ok(())
    }
}
