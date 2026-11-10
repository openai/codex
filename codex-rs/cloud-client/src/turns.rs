//! Explicit, non-retried mutations. A failed RPC may already have changed the thread.
use crate::Client;
use crate::Error;
use crate::TurnSummary;
use crate::transcript::DecodeFailure;
use crate::transcript::required;
use crate::turns_wire as wire;
use bytes::Bytes;
use prost::Message;
use std::fmt;

// CCA forwards the envelope through a 4 MiB boundary; reserve 256 KiB for forwarding.
const MAX_IMAGE_MESSAGE_BYTES: usize = 4 * 1024 * 1024 - 256 * 1024;

#[derive(Clone, Copy, Debug)]
pub enum SubmissionMode {
    Steer,
    Queue,
    StartIfIdle,
}
/// Text and images with optional conversation model settings. Persist the message ID with
/// the input before calling; never reuse it for a different logical submission.
#[derive(Clone)]
pub struct AddUserMessageRequest {
    pub thread_id: String,
    pub client_user_message_id: String,
    pub input: Vec<UserMessageInput>,
    /// Optional classification supplied by the caller for this turn.
    pub turn_trigger: Option<String>,
    /// Omit both to inherit the thread settings. A model alone uses its default effort.
    pub model: Option<String>,
    pub effort: Option<String>,
    pub mode: SubmissionMode,
    pub shared_access: bool,
}
/// Ordered input parts for one logical user message.
#[derive(Clone, PartialEq, Eq)]
pub enum UserMessageInput {
    Text {
        text: String,
        text_elements: Vec<UserMessageTextElement>,
    },
    Image {
        image: ImageReference,
        detail: Option<ImageDetail>,
    },
}
/// A placeholder span in the UTF-8 bytes of a text input part.
#[derive(Clone, PartialEq, Eq)]
pub struct UserMessageTextElement {
    pub start: usize,
    pub end: usize,
    pub placeholder: Option<String>,
}
/// An inline data URL or an uploaded file ID. HTTP image URLs are not supported.
#[derive(Clone, PartialEq, Eq)]
pub enum ImageReference {
    Url(String),
    FileId(String),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageDetail {
    Auto,
    Low,
    High,
    Original,
}
#[derive(Clone)]
pub struct InterruptTurnRequest {
    pub thread_id: String,
    pub turn_id: String,
    pub shared_access: bool,
}
#[derive(PartialEq)]
pub enum AdmissionOutcome {
    Started(TurnSummary),
    Steered(TurnSummary),
    /// The original turn ID, if assigned. Read history or observe Attach for its status.
    Duplicate {
        turn_id: Option<String>,
    },
    Queued {
        submission_id: String,
    },
}
pub struct Admission {
    pub thread_id: String,
    pub client_user_message_id: String,
    pub outcome: AdmissionOutcome,
    payload: Bytes,
}
impl Admission {
    /// Complete original AddUserMessageResponse, including untyped metadata.
    pub fn payload(&self) -> &Bytes {
        &self.payload
    }
}
/// Remote errors do not reliably identify whether admission happened. Inspect the
/// source explicitly, reconcile with the server, and never blindly resubmit.
#[derive(Debug, thiserror::Error)]
pub enum MutationError {
    #[error("invalid or oversized mutation request; nothing was sent")]
    InvalidRequest,
    #[error("mutation outcome unknown; do not automatically retry: {0}")]
    OutcomeUnknown(#[source] Error),
}
macro_rules! redacted_debug {
    ($($ty:ty),*) => {$(impl fmt::Debug for $ty {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(concat!(stringify!($ty), "([redacted])")) }
    })*};
}
redacted_debug!(
    AddUserMessageRequest,
    UserMessageInput,
    UserMessageTextElement,
    ImageReference,
    InterruptTurnRequest,
    Admission,
    AdmissionOutcome
);

impl Client {
    /// Sends once. Success is admission, not turn completion. Dropping this future
    /// after polling can leave admission unknown; it does not cancel remote work.
    pub async fn add_user_message(
        &self,
        request: &AddUserMessageRequest,
    ) -> Result<Admission, MutationError> {
        if request.thread_id.trim().is_empty()
            || request.client_user_message_id.trim().is_empty()
            || ((request.model.is_some() || request.effort.is_some())
                && matches!(request.mode, SubmissionMode::Queue))
            || !request.input.iter().any(|input| match input {
                UserMessageInput::Text { text, .. } => !text.trim().is_empty(),
                UserMessageInput::Image { .. } => true,
            })
            || request.input.iter().any(|input| match input {
                UserMessageInput::Text {
                    text,
                    text_elements,
                } => text_elements
                    .iter()
                    .any(|element| text.get(element.start..element.end).is_none()),
                UserMessageInput::Image { image, .. } => match image {
                    ImageReference::Url(value) => {
                        value.split_once(',').is_none_or(|(prefix, payload)| {
                            !prefix
                                .get(..5)
                                .is_some_and(|scheme| scheme.eq_ignore_ascii_case("data:"))
                                || payload.trim().is_empty()
                        })
                    }
                    ImageReference::FileId(value) => value.trim().is_empty(),
                },
            })
        {
            return Err(MutationError::InvalidRequest);
        }
        let value = wire::AddRequest {
            client_user_message_id: request.client_user_message_id.clone(),
            input: Some(wire::Input {
                values: request
                    .input
                    .iter()
                    .map(|input| wire::UserInput {
                        value: Some(match input {
                            UserMessageInput::Text {
                                text,
                                text_elements,
                            } => wire::Value::Text(wire::Text {
                                text: Some(text.clone()),
                                text_elements: (!text_elements.is_empty()).then(|| {
                                    wire::TextElements {
                                        values: text_elements
                                            .iter()
                                            .map(|element| wire::TextElement {
                                                byte_range: Some(wire::ByteRange {
                                                    end: Some(element.end as u64),
                                                    start: Some(element.start as u64),
                                                }),
                                                placeholder: element.placeholder.clone(),
                                            })
                                            .collect(),
                                    }
                                }),
                            }),
                            UserMessageInput::Image { image, detail } => {
                                let detail = detail.map(|detail| match detail {
                                    ImageDetail::Auto => 1,
                                    ImageDetail::Low => 2,
                                    ImageDetail::High => 3,
                                    ImageDetail::Original => 4,
                                });
                                let value = match image {
                                    ImageReference::Url(url) => {
                                        wire::ImageValue::Url(wire::ImageReference {
                                            detail,
                                            value: Some(url.clone()),
                                        })
                                    }
                                    ImageReference::FileId(file_id) => {
                                        wire::ImageValue::FileId(wire::ImageReference {
                                            detail,
                                            value: Some(file_id.clone()),
                                        })
                                    }
                                };
                                wire::Value::Image(wire::Image { value: Some(value) })
                            }
                        }),
                    })
                    .collect(),
            }),
            model: request.model.clone(),
            effort: request.effort.clone(),
            thread_id: request.thread_id.clone(),
            turn_trigger: request.turn_trigger.clone(),
            access: request.shared_access.then_some(1),
            mode: Some(match request.mode {
                SubmissionMode::Steer => 1,
                SubmissionMode::Queue => 2,
                SubmissionMode::StartIfIdle => 3,
            }),
        };
        if request
            .input
            .iter()
            .any(|input| matches!(input, UserMessageInput::Image { .. }))
            && value.encoded_len() > MAX_IMAGE_MESSAGE_BYTES
        {
            return Err(MutationError::InvalidRequest);
        }
        let payload = self
            .mutate(
                value,
                "/oaiproto.codex_app_server_backend.ThreadService/AddUserMessage",
            )
            .await?;
        let outcome = decode_admission(&request.client_user_message_id, payload.clone())
            .map_err(|_| MutationError::OutcomeUnknown(Error::InvalidResponse))?;
        Ok(Admission {
            thread_id: request.thread_id.clone(),
            client_user_message_id: request.client_user_message_id.clone(),
            outcome,
            payload,
        })
    }
    /// Interrupts this turn only, without stopping its thread or removing queued input.
    /// Success acknowledges the RPC; observe turn lifecycle events for completion.
    pub async fn interrupt_turn(
        &self,
        request: &InterruptTurnRequest,
    ) -> Result<(), MutationError> {
        if request.thread_id.trim().is_empty() || request.turn_id.trim().is_empty() {
            return Err(MutationError::InvalidRequest);
        }
        let payload = self
            .mutate(
                wire::InterruptRequest {
                    thread_id: request.thread_id.clone(),
                    turn_id: request.turn_id.clone(),
                    access: request.shared_access.then_some(1),
                },
                "/oaiproto.codex_app_server_backend.ThreadService/InterruptTurn",
            )
            .await?;
        crate::wire::EmptyResponse::decode(payload)
            .map_err(|_| MutationError::OutcomeUnknown(Error::InvalidResponse))?;
        Ok(())
    }
    pub(crate) async fn mutate<T: Message + Send + 'static>(
        &self,
        value: T,
        path: &'static str,
    ) -> Result<Bytes, MutationError> {
        if value.encoded_len() > crate::MAX_MESSAGE_BYTES {
            return Err(MutationError::InvalidRequest);
        }
        let mut grpc = self.grpc.clone();
        let mut request = tonic::Request::new(value);
        *request.metadata_mut() =
            tonic::metadata::MetadataMap::from_headers(self.credentials.headers.clone());
        // Read the final trailers separately: initial metadata cannot acknowledge
        // the mutation, and tonic permits HTTP 200 without a final gRPC status.
        tokio::time::timeout(crate::REQUEST_TIMEOUT, async {
            let response = grpc
                .server_streaming(
                    request,
                    http::uri::PathAndQuery::from_static(path),
                    crate::wire::RawResponseCodec::<T>(std::marker::PhantomData),
                )
                .await?;
            // An initial OK makes tonic skip final-status validation. A unary
            // success must carry its status after the response message instead.
            if response.metadata().contains_key("grpc-status") {
                return Err(Error::InvalidResponse);
            }
            let mut response = response.into_inner();
            let payload = response.message().await?.ok_or(Error::InvalidResponse)?;
            let trailers = response.trailers().await?.ok_or(Error::InvalidResponse)?;
            if trailers
                .get("grpc-status")
                .is_none_or(|status| status != "0")
            {
                return Err(Error::InvalidResponse);
            }
            Ok(payload)
        })
        .await
        .map_err(|_| MutationError::OutcomeUnknown(Error::Timeout))?
        .map_err(MutationError::OutcomeUnknown)
    }
}
fn decode_admission(message_id: &str, payload: Bytes) -> Result<AdmissionOutcome, DecodeFailure> {
    let response = wire::AddResponse::decode(payload)?;
    if required(response.client_user_message_id)? != message_id {
        return Err(DecodeFailure::InvalidResponse);
    }
    if response.outcome == Some(3) {
        let turn_id = response.turn.map(|turn| required(turn.id)).transpose()?;
        if turn_id.as_ref().is_some_and(|id| id.trim().is_empty()) {
            return Err(DecodeFailure::InvalidResponse);
        }
        return Ok(AdmissionOutcome::Duplicate { turn_id });
    }
    let turn = response.turn.map(TurnSummary::try_from).transpose()?;
    if turn.as_ref().is_some_and(|turn| turn.id.trim().is_empty()) {
        return Err(DecodeFailure::InvalidResponse);
    }
    Ok(match response.outcome {
        Some(1) => AdmissionOutcome::Started(required(turn)?),
        Some(2) => AdmissionOutcome::Steered(required(turn)?),
        Some(4) => {
            let queued = required(response.queued_submission)?;
            let submission_id = required(queued.id)?;
            if submission_id.trim().is_empty()
                || required(queued.client_user_message_id)? != message_id
            {
                return Err(DecodeFailure::InvalidResponse);
            }
            AdmissionOutcome::Queued { submission_id }
        }
        _ => return Err(DecodeFailure::InvalidResponse),
    })
}
