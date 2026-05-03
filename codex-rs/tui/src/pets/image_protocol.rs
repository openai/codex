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
use codex_terminal_detection::Multiplexer;
use codex_terminal_detection::TerminalInfo;
use codex_terminal_detection::TerminalName;
use codex_terminal_detection::terminal_info;
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
pub(crate) enum PetImageSupport {
    Supported(ImageProtocol),
    Unsupported(PetImageUnsupportedReason),
}

impl PetImageSupport {
    pub(crate) fn protocol(self) -> Option<ImageProtocol> {
        match self {
            Self::Supported(protocol) => Some(protocol),
            Self::Unsupported(_) => None,
        }
    }

    pub(crate) fn unsupported_message(self) -> Option<&'static str> {
        match self {
            Self::Supported(_) => None,
            Self::Unsupported(reason) => Some(reason.message()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PetImageUnsupportedReason {
    Tmux,
    Zellij,
    Terminal,
}

impl PetImageUnsupportedReason {
    fn message(self) -> &'static str {
        match self {
            Self::Tmux => {
                "Pets are disabled in tmux. Terminal images don’t stay pane-local in tmux and can corrupt scrollback or move between panes. Run Codex outside tmux to use pets."
            }
            Self::Zellij => {
                "Pets are disabled in Zellij. Terminal images don’t stay reliably pane-local in Zellij. Run Codex outside Zellij to use pets."
            }
            Self::Terminal => {
                "Pets aren’t available in this terminal. Terminal pets need image support, and this terminal environment doesn’t expose a supported image protocol. Try a terminal with Kitty graphics or Sixel support, or run Codex outside tmux."
            }
        }
    }
}

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

pub(crate) fn detect_pet_image_support() -> PetImageSupport {
    if env::var_os("TMUX").is_some() || env::var_os("TMUX_PANE").is_some() {
        return PetImageSupport::Unsupported(PetImageUnsupportedReason::Tmux);
    }

    if env::var_os("ZELLIJ").is_some()
        || env::var_os("ZELLIJ_SESSION_NAME").is_some()
        || env::var_os("ZELLIJ_VERSION").is_some()
    {
        return PetImageSupport::Unsupported(PetImageUnsupportedReason::Zellij);
    }

    if env::var_os("KITTY_WINDOW_ID").is_some() {
        return PetImageSupport::Supported(ImageProtocol::Kitty);
    }

    if env::var_os("WEZTERM_EXECUTABLE").is_some() || env::var_os("WEZTERM_VERSION").is_some() {
        return PetImageSupport::Supported(ImageProtocol::Sixel);
    }

    pet_image_support_for_terminal(&terminal_info())
}

fn pet_image_support_for_terminal(info: &TerminalInfo) -> PetImageSupport {
    match info.multiplexer {
        Some(Multiplexer::Tmux { .. }) => {
            return PetImageSupport::Unsupported(PetImageUnsupportedReason::Tmux);
        }
        Some(Multiplexer::Zellij {}) => {
            return PetImageSupport::Unsupported(PetImageUnsupportedReason::Zellij);
        }
        None => {}
    }

    if supports_kitty_graphics(info) {
        return PetImageSupport::Supported(ImageProtocol::Kitty);
    }

    if supports_sixel(info) {
        return PetImageSupport::Supported(ImageProtocol::Sixel);
    }

    PetImageSupport::Unsupported(PetImageUnsupportedReason::Terminal)
}

fn supports_kitty_graphics(info: &TerminalInfo) -> bool {
    matches!(info.name, TerminalName::Ghostty | TerminalName::Kitty)
        || terminal_field_contains(info.term.as_deref(), "kitty")
        || terminal_field_contains(info.term.as_deref(), "ghostty")
        || terminal_field_contains(info.term_program.as_deref(), "kitty")
        || terminal_field_contains(info.term_program.as_deref(), "ghostty")
}

fn supports_sixel(info: &TerminalInfo) -> bool {
    matches!(info.name, TerminalName::Iterm2 | TerminalName::WezTerm)
        || terminal_field_contains(info.term.as_deref(), "sixel")
        || terminal_field_contains(info.term.as_deref(), "mlterm")
        || terminal_field_contains(info.term.as_deref(), "foot")
        || terminal_field_contains(info.term_program.as_deref(), "wezterm")
        || terminal_field_contains(info.term_program.as_deref(), "iterm")
}

fn terminal_field_contains(value: Option<&str>, needle: &str) -> bool {
    value
        .map(|value| value.to_ascii_lowercase().contains(needle))
        .unwrap_or(false)
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
    #[serial]
    fn auto_protocol_is_disabled_inside_tmux() {
        let _guard = TmuxEnvGuard::new(Some("session"));

        assert_eq!(
            ProtocolSelection::Auto.resolve(),
            PetImageSupport::Unsupported(PetImageUnsupportedReason::Tmux)
        );
    }

    #[test]
    #[serial]
    fn explicit_protocol_still_resolves_inside_tmux() {
        let _guard = TmuxEnvGuard::new(Some("session"));

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
    fn pet_image_support_prefers_multiplexer_safety() {
        assert_eq!(
            pet_image_support_for_terminal(&terminal_info_for_test(
                TerminalName::Ghostty,
                Some(Multiplexer::Tmux { version: None }),
                Some("Ghostty"),
                /*term*/ None,
            )),
            PetImageSupport::Unsupported(PetImageUnsupportedReason::Tmux)
        );
        assert_eq!(
            pet_image_support_for_terminal(&terminal_info_for_test(
                TerminalName::Kitty,
                Some(Multiplexer::Zellij {}),
                Some("kitty"),
                /*term*/ None,
            )),
            PetImageSupport::Unsupported(PetImageUnsupportedReason::Zellij)
        );
    }

    #[test]
    fn pet_image_support_detects_kitty_graphics_terminals() {
        for info in [
            terminal_info_for_test(
                TerminalName::Ghostty,
                /*multiplexer*/ None,
                Some("Ghostty"),
                /*term*/ None,
            ),
            terminal_info_for_test(
                TerminalName::Kitty,
                /*multiplexer*/ None,
                Some("kitty"),
                /*term*/ None,
            ),
            terminal_info_for_test(
                TerminalName::Unknown,
                /*multiplexer*/ None,
                /*term_program*/ None,
                Some("xterm-kitty"),
            ),
        ] {
            assert_eq!(
                pet_image_support_for_terminal(&info),
                PetImageSupport::Supported(ImageProtocol::Kitty)
            );
        }
    }

    #[test]
    fn pet_image_support_detects_sixel_terminals() {
        for info in [
            terminal_info_for_test(
                TerminalName::WezTerm,
                /*multiplexer*/ None,
                Some("WezTerm"),
                /*term*/ None,
            ),
            terminal_info_for_test(
                TerminalName::Iterm2,
                /*multiplexer*/ None,
                Some("iTerm.app"),
                /*term*/ None,
            ),
            terminal_info_for_test(
                TerminalName::Unknown,
                /*multiplexer*/ None,
                /*term_program*/ None,
                Some("xterm-sixel"),
            ),
            terminal_info_for_test(
                TerminalName::Unknown,
                /*multiplexer*/ None,
                /*term_program*/ None,
                Some("foot"),
            ),
            terminal_info_for_test(
                TerminalName::Unknown,
                /*multiplexer*/ None,
                /*term_program*/ None,
                Some("mlterm"),
            ),
        ] {
            assert_eq!(
                pet_image_support_for_terminal(&info),
                PetImageSupport::Supported(ImageProtocol::Sixel)
            );
        }
    }

    #[test]
    fn pet_image_support_rejects_unknown_terminals() {
        assert_eq!(
            pet_image_support_for_terminal(&terminal_info_for_test(
                TerminalName::Unknown,
                /*multiplexer*/ None,
                /*term_program*/ None,
                Some("xterm-256color"),
            )),
            PetImageSupport::Unsupported(PetImageUnsupportedReason::Terminal)
        );
    }

    fn terminal_info_for_test(
        name: TerminalName,
        multiplexer: Option<Multiplexer>,
        term_program: Option<&str>,
        term: Option<&str>,
    ) -> TerminalInfo {
        TerminalInfo {
            name,
            term_program: term_program.map(str::to_string),
            version: /*version*/ None,
            term: term.map(str::to_string),
            multiplexer,
        }
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
