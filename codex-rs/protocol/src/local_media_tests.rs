use super::*;
use anyhow::Result;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

const TINY_PNG_BYTES: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156, 99, 96, 0, 2, 0, 0, 5, 0, 1,
    122, 94, 171, 63, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

#[test]
fn snapshots_local_image_user_input_with_requested_detail() -> Result<()> {
    let temp_dir = tempdir()?;
    let image_path = temp_dir.path().join("sample.png");
    std::fs::write(&image_path, TINY_PNG_BYTES)?;

    for detail in [
        None,
        Some(ImageDetail::Auto),
        Some(ImageDetail::Low),
        Some(ImageDetail::High),
        Some(ImageDetail::Original),
    ] {
        let mut input = UserInput::LocalImage {
            path: image_path.clone(),
            detail,
        };

        snapshot_local_user_input(&mut input)?;

        assert_eq!(
            UserInput::Image {
                image_url: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR4nGNgAAIAAAUAAXpeqz8AAAAASUVORK5CYII="
                    .to_string(),
                detail,
            },
            input
        );
    }

    Ok(())
}

#[test]
fn snapshots_local_audio_user_input_with_supported_mime_types() -> Result<()> {
    let temp_dir = tempdir()?;

    for (extension, mime) in [
        ("wav", "audio/wav"),
        ("MP3", "audio/mpeg"),
        ("m4a", "audio/mp4"),
        ("webm", "audio/webm"),
        ("ogg", "audio/ogg"),
    ] {
        let audio_path = temp_dir.path().join(format!("sample.{extension}"));
        std::fs::write(&audio_path, b"audio")?;
        let mut input = UserInput::LocalAudio { path: audio_path };

        snapshot_local_user_input(&mut input)?;

        assert_eq!(
            UserInput::Audio {
                audio_url: format!("data:{mime};base64,YXVkaW8="),
            },
            input
        );
    }

    Ok(())
}

#[test]
fn rejects_invalid_local_media_without_changing_user_input() -> Result<()> {
    let temp_dir = tempdir()?;
    let invalid_image_path = temp_dir.path().join("invalid.png");
    let unsupported_audio_path = temp_dir.path().join("unsupported.flac");
    std::fs::write(&invalid_image_path, b"invalid image")?;
    std::fs::write(&unsupported_audio_path, b"audio")?;

    let mut invalid_image = UserInput::LocalImage {
        path: invalid_image_path,
        detail: Some(ImageDetail::Original),
    };
    let original_image = invalid_image.clone();
    let image_error = snapshot_local_user_input(&mut invalid_image)
        .expect_err("an invalid local image must be rejected");
    assert_eq!(io::ErrorKind::Other, image_error.kind());
    assert_eq!(original_image, invalid_image);

    let mut unsupported_audio = UserInput::LocalAudio {
        path: unsupported_audio_path,
    };
    let original_audio = unsupported_audio.clone();
    let audio_error = snapshot_local_user_input(&mut unsupported_audio)
        .expect_err("an unsupported local audio format must be rejected");
    assert_eq!(io::ErrorKind::InvalidData, audio_error.kind());
    assert_eq!(original_audio, unsupported_audio);

    Ok(())
}

#[test]
fn rejects_oversized_local_media_without_reading_it() -> Result<()> {
    let temp_dir = tempdir()?;

    for (name, limit, is_image) in [
        ("oversized.png", MAX_PROMPT_IMAGE_INPUT_BYTES, true),
        ("oversized.mp3", MAX_PROMPT_AUDIO_INPUT_BYTES, false),
    ] {
        let path = temp_dir.path().join(name);
        std::fs::File::create(&path)?.set_len(limit as u64 + 1)?;
        let mut input = if is_image {
            UserInput::LocalImage { path, detail: None }
        } else {
            UserInput::LocalAudio { path }
        };
        let original = input.clone();

        let error = snapshot_local_user_input(&mut input)
            .expect_err("oversized local media must be rejected before it is read");

        assert_eq!(io::ErrorKind::InvalidData, error.kind());
        assert_eq!(original, input);
    }

    Ok(())
}
