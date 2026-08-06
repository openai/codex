use std::io;
use std::io::Read;
use std::path::Path;

use codex_utils_image::MAX_PROMPT_IMAGE_INPUT_BYTES;
use codex_utils_image::PromptImageMode;
use codex_utils_image::data_url_from_bytes;
use codex_utils_image::load_for_prompt_bytes;

use crate::models::ImageDetail;
use crate::user_input::UserInput;

/// Maximum accepted decoded byte length for prompt audio inputs.
///
/// This matches the Responses API audio input limit.
pub const MAX_PROMPT_AUDIO_INPUT_BYTES: usize = 50 * 1024 * 1024;

/// Snapshots local image and audio input into portable, validated data URLs.
pub fn snapshot_local_user_input(input: &mut UserInput) -> io::Result<()> {
    match input {
        UserInput::LocalImage { path, detail } => {
            let image_detail = *detail;
            let mode = match image_detail {
                Some(ImageDetail::Original) => PromptImageMode::Original,
                Some(ImageDetail::Auto | ImageDetail::Low | ImageDetail::High) | None => {
                    PromptImageMode::ResizeToFit
                }
            };
            let file_bytes = read_bounded_local_media(path, MAX_PROMPT_IMAGE_INPUT_BYTES, "image")?;
            let image = load_for_prompt_bytes(path, file_bytes, mode).map_err(io::Error::other)?;
            *input = UserInput::Image {
                image_url: image.into_data_url(),
                detail: image_detail,
            };
        }
        UserInput::LocalAudio { path } => {
            let mime = audio_mime_for_path(path).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "unsupported audio format")
            })?;
            let file_bytes = read_bounded_local_media(path, MAX_PROMPT_AUDIO_INPUT_BYTES, "audio")?;
            *input = UserInput::Audio {
                audio_url: data_url_from_bytes(mime, &file_bytes),
            };
        }
        UserInput::Text { .. }
        | UserInput::Image { .. }
        | UserInput::Audio { .. }
        | UserInput::Skill { .. }
        | UserInput::Mention { .. } => {}
    }
    Ok(())
}

fn read_bounded_local_media(path: &Path, max_bytes: usize, kind: &str) -> io::Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    if file.metadata()?.len() > max_bytes as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{kind} input exceeds {max_bytes} bytes"),
        ));
    }

    let mut bytes = Vec::new();
    file.take(max_bytes as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{kind} input exceeds {max_bytes} bytes"),
        ));
    }
    Ok(bytes)
}

pub(crate) fn audio_mime_for_path(path: &Path) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?;
    if extension.eq_ignore_ascii_case("wav") {
        Some("audio/wav")
    } else if extension.eq_ignore_ascii_case("mp3") {
        Some("audio/mpeg")
    } else if extension.eq_ignore_ascii_case("m4a") {
        Some("audio/mp4")
    } else if extension.eq_ignore_ascii_case("webm") {
        Some("audio/webm")
    } else if extension.eq_ignore_ascii_case("ogg") {
        Some("audio/ogg")
    } else {
        None
    }
}

#[cfg(test)]
#[path = "local_media_tests.rs"]
mod tests;
