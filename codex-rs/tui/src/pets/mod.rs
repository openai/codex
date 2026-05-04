//! Ambient terminal pets configured from the /pets slash command.
//!
//! The Codex app stores default pets as bundled spritesheet assets and custom pets under
//! $CODEX_HOME/pets/<pet-id>/pet.json.
//! This module keeps that package shape intact while rendering the selected pet inline in the TUI.

use std::io::Write;

mod ambient;
mod catalog;
mod frames;
mod image_protocol;
mod model;
mod picker;
mod preview;

use anyhow::Context;
use anyhow::Result;

pub(crate) use ambient::AmbientPet;
pub(crate) use ambient::AmbientPetDraw;
pub(crate) use ambient::PetNotificationKind;
#[cfg(test)]
pub(crate) use image_protocol::ImageProtocol;
pub(crate) use image_protocol::PetImageSupport;
#[cfg(test)]
pub(crate) use image_protocol::PetImageUnsupportedReason;
#[cfg(not(test))]
pub(crate) use image_protocol::detect_pet_image_support;
pub(crate) use picker::PET_PICKER_VIEW_ID;
pub(crate) use picker::build_pet_picker_params;
pub(crate) use preview::PetPickerPreviewState;

pub(crate) const DEFAULT_PET_ID: &str = "codex";
pub(crate) const DISABLED_PET_ID: &str = "disabled";

pub(crate) fn render_ambient_pet_image(
    writer: &mut impl Write,
    request: Option<AmbientPetDraw>,
) -> Result<()> {
    render_pet_image(writer, /*image_id*/ 0xC0DE, request)
}

pub(crate) fn render_pet_picker_preview_image(
    writer: &mut impl Write,
    request: Option<AmbientPetDraw>,
) -> Result<()> {
    render_pet_image(writer, /*image_id*/ 0xC0DF, request)
}

fn render_pet_image(
    writer: &mut impl Write,
    image_id: u32,
    request: Option<AmbientPetDraw>,
) -> Result<()> {
    use crossterm::cursor::MoveTo;
    use crossterm::cursor::RestorePosition;
    use crossterm::cursor::SavePosition;
    use crossterm::queue;
    use image_protocol::ImageProtocol;

    write!(writer, "{}", image_protocol::kitty_delete_image(image_id))?;
    let Some(request) = request else {
        writer.flush()?;
        return Ok(());
    };

    let payload = match request.protocol {
        ImageProtocol::Kitty => {
            AmbientPetPayload::Text(image_protocol::kitty_transmit_png_with_id(
                &request.frame,
                request.columns,
                request.rows,
                Some(image_id),
            )?)
        }
        ImageProtocol::Sixel => {
            let path =
                image_protocol::sixel_frame(&request.frame, &request.sixel_dir, request.height_px)?;
            let sixel = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
            AmbientPetPayload::Bytes(sixel)
        }
    };

    queue!(writer, SavePosition, MoveTo(request.x, request.y))?;
    match payload {
        AmbientPetPayload::Text(payload) => write!(writer, "{payload}")?,
        AmbientPetPayload::Bytes(payload) => writer.write_all(&payload)?,
    }
    queue!(writer, RestorePosition)?;
    writer.flush()?;
    Ok(())
}

enum AmbientPetPayload {
    Text(String),
    Bytes(Vec<u8>),
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::image_protocol::ImageProtocol;
    use super::*;

    #[test]
    fn ambient_pet_image_restores_cursor_after_drawing() {
        let dir = tempfile::tempdir().unwrap();
        let frame = dir.path().join("frame.png");
        std::fs::write(&frame, b"png").unwrap();
        let request = AmbientPetDraw {
            frame,
            protocol: ImageProtocol::Kitty,
            x: 2,
            y: 3,
            columns: 4,
            rows: 5,
            height_px: 75,
            sixel_dir: PathBuf::new(),
        };
        let mut output = Vec::new();

        render_ambient_pet_image(&mut output, Some(request)).unwrap();

        let output = String::from_utf8(output).unwrap();
        let save = output.find("\x1b7").expect("saves cursor position");
        let move_to = output.find("\x1b[4;3H").expect("moves to pet position");
        let image = output.find("cG5n").expect("writes image payload");
        let restore = output.find("\x1b8").expect("restores cursor position");
        assert!(save < move_to);
        assert!(move_to < image);
        assert!(image < restore);
    }

    #[test]
    fn ambient_pet_image_clear_deletes_without_moving_cursor() {
        let mut output = Vec::new();

        render_ambient_pet_image(&mut output, /*request*/ None).unwrap();

        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("Ga=d,d=I,i=49374,q=2;"));
        assert!(!output.contains("\x1b7"));
        assert!(!output.contains("\x1b["));
        assert!(!output.contains("\x1b8"));
    }
}
