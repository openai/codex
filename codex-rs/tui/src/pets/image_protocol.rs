use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use base64::Engine as _;
use base64::engine::general_purpose;
use icy_sixel::BackgroundMode;
use icy_sixel::PixelAspectRatio;
use icy_sixel::SixelImage;
use image::imageops::FilterType;

const ESC: &str = "\x1b";
const ST: &str = "\x1b\\";
const KITTY_CHUNK_SIZE: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageProtocol {
    Kitty,
    Sixel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolSelection {
    Auto,
    Kitty,
    Sixel,
}

impl ProtocolSelection {
    pub fn resolve(self) -> ImageProtocol {
        match self {
            Self::Kitty => ImageProtocol::Kitty,
            Self::Sixel => ImageProtocol::Sixel,
            Self::Auto => detect_protocol(),
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

fn detect_protocol() -> ImageProtocol {
    let term = env::var("TERM").unwrap_or_default().to_ascii_lowercase();
    if env::var_os("KITTY_WINDOW_ID").is_some() || term.contains("kitty") {
        return ImageProtocol::Kitty;
    }

    let term_program = env::var("TERM_PROGRAM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if term.contains("sixel")
        || term.contains("mlterm")
        || term.contains("foot")
        || env::var_os("WEZTERM_EXECUTABLE").is_some()
        || term_program.contains("wezterm")
        || term_program.contains("iterm")
    {
        return ImageProtocol::Sixel;
    }

    ImageProtocol::Kitty
}

pub fn kitty_delete_image(image_id: u32) -> String {
    wrap_for_tmux_if_needed(&format!("{ESC}_Ga=d,d=I,i={image_id},q=2;{ST}"))
}

pub fn kitty_transmit_png_with_id(
    path: &Path,
    columns: u16,
    rows: u16,
    image_id: Option<u32>,
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
        if index == 0 {
            let image_id = image_id
                .map(|image_id| format!(",i={image_id}"))
                .unwrap_or_default();
            command.push_str(&format!(
                "{ESC}_Ga=T,t=d,f=100,c={columns},r={rows},q=2{image_id},m={};{chunk}{ST}",
                if has_more { 1 } else { 0 },
            ));
        } else {
            command.push_str(&format!(
                "{ESC}_Gm={};{chunk}{ST}",
                if has_more { 1 } else { 0 },
            ));
        }
    }

    Ok(wrap_for_tmux_if_needed(&command))
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
    let path = cache_dir.join(format!("{stem}_h{height_px}.six"));
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
    let sixel = SixelImage::try_from_rgba(rgba.into_raw(), width as usize, height as usize)
        .map_err(anyhow::Error::from)?
        .with_aspect_ratio(PixelAspectRatio::Square)
        .with_background_mode(BackgroundMode::Transparent)
        .encode()
        .map_err(anyhow::Error::from)?;

    fs::write(&path, sixel).with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use serial_test::serial;

    use super::*;

    struct TmuxEnvGuard {
        previous: Option<std::ffi::OsString>,
    }

    impl TmuxEnvGuard {
        fn new(value: Option<&str>) -> Self {
            let previous = env::var_os("TMUX");
            match value {
                Some(value) => unsafe { env::set_var("TMUX", value) },
                None => unsafe { env::remove_var("TMUX") },
            }
            Self { previous }
        }
    }

    impl Drop for TmuxEnvGuard {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(value) => unsafe { env::set_var("TMUX", value) },
                None => unsafe { env::remove_var("TMUX") },
            }
        }
    }

    #[test]
    #[serial]
    fn kitty_png_transmission_encodes_inline_data() {
        let _guard = TmuxEnvGuard::new(/*value*/ None);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("frame.png");
        fs::write(&path, b"png").unwrap();

        let command = kitty_transmit_png_with_id(
            &path, /*columns*/ 4, /*rows*/ 3, /*image_id*/ None,
        )
        .unwrap();

        assert!(command.starts_with("\x1b_Ga=T,t=d,f=100,c=4,r=3,q=2,m=0;"));
        assert!(command.contains("cG5n"));
        assert!(command.ends_with("\x1b\\"));
    }

    #[test]
    #[serial]
    fn tmux_passthrough_wraps_and_escapes_control_sequence() {
        let _guard = TmuxEnvGuard::new(Some("session"));
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
    fn sixel_frame_encodes_with_rust_crate() {
        let dir = tempfile::tempdir().unwrap();
        let frame_path = dir.path().join("frame.png");
        let rgba = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]));
        rgba.save(&frame_path).unwrap();

        let sixel_path =
            sixel_frame(&frame_path, &dir.path().join("sixel"), /*height_px*/ 1).unwrap();
        let sixel = fs::read_to_string(sixel_path).unwrap();

        assert!(sixel.starts_with("\x1bP"));
        assert!(sixel.ends_with("\x1b\\"));
    }
}
