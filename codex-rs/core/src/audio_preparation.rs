use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ResponseItem;
use codex_utils_cache::BlockingLruCache;
use codex_utils_cache::sha1_digest;
use codex_utils_output_truncation::approx_token_count;
use std::io::Cursor;
use std::num::NonZeroUsize;
use std::sync::LazyLock;
use symphonia::core::formats::FormatOptions;
use symphonia::core::formats::TrackType;
use symphonia::core::formats::probe::Hint;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use tracing::warn;

const AUDIO_PROCESSING_ERROR_PLACEHOLDER: &str =
    "audio content omitted because it could not be processed";
const AUDIO_TOO_LARGE_PLACEHOLDER: &str =
    "audio content omitted because it exceeded the supported size limit; use a smaller audio file";
const UNSUPPORTED_AUDIO_FORMAT_PLACEHOLDER: &str =
    "audio content omitted because its format is not supported; use wav, mp3, m4a, webm, or ogg";

/// Maximum accepted decoded byte length for prompt audio inputs.
///
/// This matches the Responses API audio input limit.
const MAX_PROMPT_AUDIO_INPUT_BYTES: usize = 50 * 1024 * 1024;
const MAX_PROMPT_AUDIO_BASE64_BYTES: usize = MAX_PROMPT_AUDIO_INPUT_BYTES.div_ceil(3) * 4;
const AUDIO_TOKEN_ESTIMATE_CACHE_SIZE: usize = 32;
const AUDIO_TOKENS_PER_SECOND: f64 = 10.0;

static AUDIO_TOKEN_ESTIMATE_CACHE: LazyLock<BlockingLruCache<[u8; 20], usize>> =
    LazyLock::new(|| {
        BlockingLruCache::new(
            NonZeroUsize::new(AUDIO_TOKEN_ESTIMATE_CACHE_SIZE).unwrap_or(NonZeroUsize::MIN),
        )
    });

#[derive(Debug, thiserror::Error)]
enum AudioPreparationError {
    #[error("invalid audio data URL: {reason}")]
    InvalidDataUrl { reason: &'static str },
    #[error("unsupported audio format")]
    UnsupportedFormat,
    #[error("audio input is too large ({size} bytes; max {MAX_PROMPT_AUDIO_INPUT_BYTES} bytes)")]
    AudioTooLarge { size: usize },
}

impl AudioPreparationError {
    fn placeholder(&self) -> &'static str {
        match self {
            AudioPreparationError::InvalidDataUrl { .. } => AUDIO_PROCESSING_ERROR_PLACEHOLDER,
            AudioPreparationError::UnsupportedFormat => UNSUPPORTED_AUDIO_FORMAT_PLACEHOLDER,
            AudioPreparationError::AudioTooLarge { .. } => AUDIO_TOO_LARGE_PLACEHOLDER,
        }
    }
}

pub(crate) fn prepare_response_items(items: &mut [ResponseItem]) {
    for item in items {
        match item {
            ResponseItem::Message { content, .. } => prepare_message_content(content),
            ResponseItem::FunctionCallOutput { output, .. }
            | ResponseItem::CustomToolCallOutput { output, .. } => {
                if let Some(content) = output.content_items_mut() {
                    prepare_tool_output_content(content);
                }
            }
            ResponseItem::AdditionalTools { .. }
            | ResponseItem::Reasoning { .. }
            | ResponseItem::AgentMessage { .. }
            | ResponseItem::LocalShellCall { .. }
            | ResponseItem::FunctionCall { .. }
            | ResponseItem::ToolSearchCall { .. }
            | ResponseItem::CustomToolCall { .. }
            | ResponseItem::ToolSearchOutput { .. }
            | ResponseItem::WebSearchCall { .. }
            | ResponseItem::ImageGenerationCall { .. }
            | ResponseItem::Compaction { .. }
            | ResponseItem::CompactionTrigger { .. }
            | ResponseItem::ContextCompaction { .. }
            | ResponseItem::Other => {}
        }
    }
}

fn prepare_message_content(items: &mut [ContentItem]) {
    for item in items {
        if let ContentItem::InputAudio { audio_url } = item
            && let Err(error) = prepare_audio(audio_url)
        {
            warn!(%error, "failed to prepare message audio");
            *item = ContentItem::InputText {
                text: error.placeholder().to_string(),
            };
        }
    }
}

fn prepare_tool_output_content(items: &mut [FunctionCallOutputContentItem]) {
    for item in items {
        if let FunctionCallOutputContentItem::InputAudio { audio_url } = item
            && let Err(error) = prepare_audio(audio_url)
        {
            warn!(%error, "failed to prepare tool output audio");
            *item = FunctionCallOutputContentItem::InputText {
                text: error.placeholder().to_string(),
            };
        }
    }
}

fn is_data_url(audio_url: &str) -> bool {
    audio_url
        .get(.."data:".len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
}

fn canonical_audio_mime(mime: &str) -> Option<&'static str> {
    if mime.eq_ignore_ascii_case("audio/wav")
        || mime.eq_ignore_ascii_case("audio/x-wav")
        || mime.eq_ignore_ascii_case("audio/wave")
        || mime.eq_ignore_ascii_case("audio/vnd.wave")
    {
        Some("audio/wav")
    } else if mime.eq_ignore_ascii_case("audio/mpeg") || mime.eq_ignore_ascii_case("audio/mp3") {
        Some("audio/mpeg")
    } else if mime.eq_ignore_ascii_case("audio/mp4")
        || mime.eq_ignore_ascii_case("audio/m4a")
        || mime.eq_ignore_ascii_case("audio/x-m4a")
    {
        Some("audio/mp4")
    } else if mime.eq_ignore_ascii_case("audio/webm") {
        Some("audio/webm")
    } else if mime.eq_ignore_ascii_case("audio/ogg") {
        Some("audio/ogg")
    } else {
        None
    }
}

pub(crate) fn estimate_audio_token_count(audio_url: &str) -> usize {
    let key = sha1_digest(audio_url.as_bytes());
    AUDIO_TOKEN_ESTIMATE_CACHE.get_or_insert_with(key, || {
        let Some(duration_seconds) = audio_duration_seconds(audio_url) else {
            return approx_token_count(audio_url);
        };
        let token_count = (duration_seconds * AUDIO_TOKENS_PER_SECOND).ceil();
        if token_count >= usize::MAX as f64 {
            usize::MAX
        } else {
            token_count as usize
        }
    })
}

fn audio_duration_seconds(audio_url: &str) -> Option<f64> {
    let (metadata, payload) = audio_url.split_once(',')?;
    let metadata = metadata.get("data:".len()..)?;
    let mut metadata_parts = metadata.split(';');
    let canonical_mime = canonical_audio_mime(metadata_parts.next()?)?;
    if !metadata_parts.any(|part| part.eq_ignore_ascii_case("base64")) {
        return None;
    }

    let bytes = match BASE64_STANDARD.decode(payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::trace!(%error, "failed to decode audio payload for token estimation");
            return None;
        }
    };
    let media_source = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let mut hint = Hint::new();
    hint.mime_type(canonical_mime);
    let format = match symphonia::default::get_probe().probe(
        &hint,
        media_source,
        FormatOptions::default(),
        MetadataOptions::default(),
    ) {
        Ok(format) => format,
        Err(error) => {
            tracing::trace!(%error, "failed to read audio duration for token estimation");
            return None;
        }
    };
    let track = format.default_track(TrackType::Audio)?;
    let timing = track.time_base.zip(track.duration).or_else(|| {
        format
            .media_info()
            .time_base
            .zip(format.media_info().duration)
    });
    let (time_base, duration) = timing?;
    let duration_seconds =
        duration.get() as f64 * f64::from(time_base.numer.get()) / f64::from(time_base.denom.get());
    duration_seconds.is_finite().then_some(duration_seconds)
}

fn prepare_audio(audio_url: &mut String) -> Result<(), AudioPreparationError> {
    if !is_data_url(audio_url) {
        return Err(AudioPreparationError::InvalidDataUrl {
            reason: "audio input must be a data URL",
        });
    }

    let (metadata, payload) =
        audio_url
            .split_once(',')
            .ok_or(AudioPreparationError::InvalidDataUrl {
                reason: "missing payload separator",
            })?;
    let metadata = metadata
        .get("data:".len()..)
        .ok_or(AudioPreparationError::InvalidDataUrl {
            reason: "missing data URL prefix",
        })?;
    let mut metadata_parts = metadata.split(';');
    let mime = metadata_parts
        .next()
        .filter(|mime| !mime.is_empty())
        .ok_or(AudioPreparationError::InvalidDataUrl {
            reason: "missing media type",
        })?;
    let canonical_mime =
        canonical_audio_mime(mime).ok_or(AudioPreparationError::UnsupportedFormat)?;
    if !metadata_parts.any(|part| part.eq_ignore_ascii_case("base64")) {
        return Err(AudioPreparationError::InvalidDataUrl {
            reason: "audio payload is not base64 encoded",
        });
    }
    if payload.len() > MAX_PROMPT_AUDIO_BASE64_BYTES {
        return Err(AudioPreparationError::AudioTooLarge {
            size: payload.len(),
        });
    }

    let bytes =
        BASE64_STANDARD
            .decode(payload)
            .map_err(|_| AudioPreparationError::InvalidDataUrl {
                reason: "invalid base64 payload",
            })?;
    if bytes.len() > MAX_PROMPT_AUDIO_INPUT_BYTES {
        return Err(AudioPreparationError::AudioTooLarge { size: bytes.len() });
    }

    let encoded = BASE64_STANDARD.encode(bytes);
    *audio_url = format!("data:{canonical_mime};base64,{encoded}");
    Ok(())
}

#[cfg(test)]
#[path = "audio_preparation_tests.rs"]
mod tests;
