use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use base64::Engine as _;
use base64::engine::general_purpose;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use image::imageops::FilterType;
use ratatui::layout::Rect;

use super::sixel;

const ESC: &str = "\x1b";
const ST: &str = "\x1b\\";
const KITTY_CHUNK_SIZE: usize = 4096;
const SIXEL_CACHE_VERSION: &str = "v2";
pub(crate) use crate::terminal_capabilities::images::ImageProtocol;
pub(crate) use crate::terminal_capabilities::images::PetImageSupport;
#[cfg(test)]
pub(crate) use crate::terminal_capabilities::images::PetImageUnsupportedReason;
pub(crate) use crate::terminal_capabilities::images::detect_pet_image_support;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolSelection {
    Auto,
    Kitty,
    Sixel,
}

impl ProtocolSelection {
    pub(crate) fn resolve(self) -> PetImageSupport {
        match self {
            Self::Kitty => PetImageSupport::Supported(ImageProtocol::Kitty),
            Self::Sixel => PetImageSupport::Supported(ImageProtocol::Sixel),
            Self::Auto => detect_pet_image_support(),
        }
    }
}

impl FromStr for ProtocolSelection {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "auto" => Ok(Self::Auto),
            "kitty" => Ok(Self::Kitty),
            "sixel" => Ok(Self::Sixel),
            other => bail!("unknown protocol {other}; expected auto, kitty, or sixel"),
        }
    }
}

pub fn kitty_delete_image(image_id: u32) -> String {
    wrap_for_tmux_if_needed(&format!("{ESC}_Ga=d,d=I,i={image_id},q=2;{ST}"))
}

#[derive(Clone, Copy)]
pub(crate) enum KittyPlacement {
    Cursor(u32),
    Virtual(u32),
}

impl KittyPlacement {
    pub(crate) fn draw(
        self,
        writer: &mut impl Write,
        area: Rect,
        payload: &[u8],
    ) -> std::io::Result<()> {
        queue!(writer, MoveTo(area.x, area.y))?;
        writer.write_all(payload)?;
        if let KittyPlacement::Virtual(id) = self {
            // tmux moves these cells with the pane. Kitty inherits subsequent columns in each row.
            let [_, r, g, b] = id.to_be_bytes();
            write!(writer, "\x1b[38;2;{r};{g};{b}m")?;
            let remaining_cells = "\u{10eeee}".repeat(usize::from(area.width.saturating_sub(1)));
            for (y, mark) in (area.y..area.bottom()).zip([
                '\u{305}', '\u{30d}', '\u{30e}', '\u{310}', '\u{312}', '\u{33d}',
            ]) {
                queue!(writer, MoveTo(area.x, y))?;
                write!(writer, "\u{10eeee}{mark}{remaining_cells}")?;
            }
            writer.write_all(b"\x1b[39m")?;
        }
        Ok(())
    }
}

pub(crate) fn kitty_move_image(id: u32, area: ratatui::layout::Rect) -> String {
    let (columns, rows) = (area.width, area.height);
    // Lowercase i removes placements but keeps the uploaded pixel data.
    wrap_for_tmux_if_needed(&format!(
        "{ESC}_Ga=d,d=i,i={id},q=2;{ST}{ESC}_Ga=p,i={id},c={columns},r={rows},C=1,q=2;{ST}"
    ))
}

pub fn kitty_transmit_png_with_id(
    path: &Path,
    columns: u16,
    rows: u16,
    placement: KittyPlacement,
) -> Result<String> {
    let png = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let payload = general_purpose::STANDARD.encode(png);
    let chunks = payload
        .as_bytes()
        .chunks(KITTY_CHUNK_SIZE)
        .collect::<Vec<_>>();

    let mut command = String::new();
    for (index, chunk) in chunks.iter().enumerate() {
        let chunk = std::str::from_utf8(chunk).context("base64 payload is not valid UTF-8")?;
        let has_more = index + 1 < chunks.len();
        let more_flag = u8::from(has_more);
        if index == 0 {
            let image_id = kitty_image_id_arg(placement);
            command.push_str(&format!(
                "{ESC}_Ga=T,t=d,f=100,c={columns},r={rows},q=2{image_id},m={more_flag},C=1;{chunk}{ST}",
            ));
        } else {
            command.push_str(&format!("{ESC}_Gm={more_flag};{chunk}{ST}"));
        }
    }

    Ok(wrap_for_tmux_if_needed(&command))
}

pub fn kitty_transmit_png_file_with_id(
    path: &Path,
    columns: u16,
    rows: u16,
    placement: KittyPlacement,
) -> Result<String> {
    let path = path
        .canonicalize()
        .with_context(|| format!("canonicalize {}", path.display()))?;
    let payload = general_purpose::STANDARD.encode(path.to_string_lossy().as_bytes());
    let image_id = kitty_image_id_arg(placement);
    let command =
        format!("{ESC}_Ga=T,t=f,f=100,c={columns},r={rows},q=2{image_id},C=1;{payload}{ST}");

    Ok(wrap_for_tmux_if_needed(&command))
}

fn kitty_image_id_arg(placement: KittyPlacement) -> String {
    match placement {
        KittyPlacement::Cursor(id) => format!(",i={id}"),
        KittyPlacement::Virtual(id) => format!(",i={id},U=1"),
    }
}

fn wrap_for_tmux_if_needed(command: &str) -> String {
    if env::var_os("TMUX").is_none() {
        return command.to_string();
    }

    let escaped = command.replace(ESC, "\x1b\x1b");
    format!("{ESC}Ptmux;{escaped}{ST}")
}

pub fn sixel_frame(frame_path: &Path, cache_dir: &Path, height_px: u16) -> Result<PathBuf> {
    fs::create_dir_all(cache_dir).with_context(|| format!("create {}", cache_dir.display()))?;

    let stem = frame_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .context("frame path has no valid file stem")?;
    let path = cache_dir.join(format!("{stem}_h{height_px}_{SIXEL_CACHE_VERSION}.six"));
    if path.exists() {
        return Ok(path);
    }

    let frame =
        image::open(frame_path).with_context(|| format!("read {}", frame_path.display()))?;
    let height = u32::from(height_px).max(1);
    let width = ((u64::from(frame.width()) * u64::from(height)) / u64::from(frame.height()))
        .try_into()
        .unwrap_or(u32::MAX)
        .max(1);
    let rgba = frame.resize(width, height, FilterType::Lanczos3).to_rgba8();
    let (width, height) = rgba.dimensions();
    let sixel = sixel::encode_rgba(&rgba.into_raw(), width, height)?;

    fs::write(&path, sixel).with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use serial_test::serial;

    use super::*;

    use crate::terminal_capabilities::images::tests::EnvVarGuard;

    #[test]
    #[serial]
    fn kitty_png_transmission_encodes_inline_data() {
        let _guard = EnvVarGuard::new("TMUX", /*value*/ None);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("frame.png");
        fs::write(&path, b"png").unwrap();

        let command = kitty_transmit_png_with_id(
            &path,
            /*columns*/ 4,
            /*rows*/ 3,
            KittyPlacement::Cursor(7),
        )
        .unwrap();

        assert!(command.starts_with("\x1b_Ga=T,t=d,f=100,c=4,r=3,q=2,i=7,m=0,C=1;"));
        assert!(command.contains("cG5n"));
        assert!(command.ends_with("\x1b\\"));
    }

    #[test]
    #[serial]
    fn tmux_passthrough_wraps_and_escapes_control_sequence() {
        let _guard = EnvVarGuard::new("TMUX", Some("session"));
        assert_eq!(
            wrap_for_tmux_if_needed("\x1b_Gx;\x1b\\"),
            "\x1bPtmux;\x1b\x1b_Gx;\x1b\x1b\\\x1b\\"
        );
    }

    #[test]
    fn parses_protocol_selection() {
        assert_eq!(
            "auto".parse::<ProtocolSelection>().unwrap(),
            ProtocolSelection::Auto
        );
        assert_eq!(
            "kitty".parse::<ProtocolSelection>().unwrap(),
            ProtocolSelection::Kitty
        );
        assert_eq!(
            "sixel".parse::<ProtocolSelection>().unwrap(),
            ProtocolSelection::Sixel
        );
    }

    #[test]
    #[serial]
    fn auto_protocol_is_disabled_inside_tmux() {
        let _guard = EnvVarGuard::new("TMUX", Some("session"));

        assert_eq!(
            ProtocolSelection::Auto.resolve(),
            PetImageSupport::Unsupported(PetImageUnsupportedReason::Tmux)
        );
    }

    #[test]
    #[serial]
    fn explicit_protocol_still_resolves_inside_tmux() {
        let _guard = EnvVarGuard::new("TMUX", Some("session"));

        assert_eq!(
            ProtocolSelection::Kitty.resolve(),
            PetImageSupport::Supported(ImageProtocol::Kitty)
        );
        assert_eq!(
            ProtocolSelection::Sixel.resolve(),
            PetImageSupport::Supported(ImageProtocol::Sixel)
        );
    }

    #[test]
    fn sixel_frame_encodes_without_external_crate() {
        let dir = tempfile::tempdir().unwrap();
        let frame_path = dir.path().join("frame.png");
        let rgba = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]));
        rgba.save(&frame_path).unwrap();

        let sixel_path =
            sixel_frame(&frame_path, &dir.path().join("sixel"), /*height_px*/ 1).unwrap();
        let sixel = fs::read_to_string(sixel_path).unwrap();

        assert!(sixel.starts_with("\x1bP9;1;0q\"1;1;1;1"));
        assert!(sixel.contains("#224;2;100;0;0"));
        assert!(sixel.contains("#224@"));
        assert!(sixel.ends_with("\x1b\\"));
    }

    #[test]
    #[serial]
    fn kitty_file_png_transmission_encodes_local_file_reference() {
        let _guard = EnvVarGuard::new("TMUX", /*value*/ None);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("frame.png");
        fs::write(&path, b"png").unwrap();

        let command = kitty_transmit_png_file_with_id(
            &path,
            /*columns*/ 4,
            /*rows*/ 3,
            KittyPlacement::Cursor(7),
        )
        .unwrap();
        let path = path.canonicalize().unwrap();
        let payload = general_purpose::STANDARD.encode(path.to_string_lossy().as_bytes());

        assert_eq!(
            command,
            format!("\x1b_Ga=T,t=f,f=100,c=4,r=3,q=2,i=7,C=1;{payload}\x1b\\")
        );
    }
}
