//! Bounded local image previews for the transcript and composer.
//!
//! Only decoded local image pixels reach the terminal encoders. Previews are never source text;
//! their reserved rows belong to layout. The terminal retains visible placements and any local
//! transport files until those placements change.

pub(crate) mod composer;

use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use crossterm::cursor::RestorePosition;
use crossterm::cursor::SavePosition;
use crossterm::queue;
use image::DynamicImage;
use image::ImageFormat;
use image::RgbaImage;
use ratatui::layout::Rect;

use crate::pets::ImageProtocol;
use crate::pets::KittyPlacement;

// Separate from the two image ids reserved for pets (0xC0DE and 0xC0DF).
pub(crate) const FIRST_IMAGE_ID: u32 = 0xC100;

pub(crate) struct ImagePlacement {
    pub(crate) image: Arc<TranscriptImage>,
    pub(crate) area: Rect,
    pub(crate) first_row: usize,
    pub(crate) file: Option<tempfile::NamedTempFile>,
    pub(crate) terminal_id: Option<u32>,
}

impl ImagePlacement {
    /// Register the same pixels for another frame. The terminal owns the transport file.
    pub(crate) fn registration(&self) -> Self {
        Self {
            image: Arc::clone(&self.image),
            area: self.area,
            first_row: self.first_row,
            file: None,
            terminal_id: None,
        }
    }
}

impl PartialEq for ImagePlacement {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.image, &other.image)
            && self.area == other.area
            && self.first_row == other.first_row
    }
}

pub(crate) struct TranscriptImage {
    pub(crate) protocol: ImageProtocol,
    pub(crate) columns: u16,
    pub(crate) rows: u16,
    pub(crate) pixels: RgbaImage,
    pub(crate) cell_width: u32,
    pub(crate) cell_height: u32,
}

pub(crate) fn terminal_cell_size() -> Option<(u32, u32)> {
    crossterm::terminal::window_size()
        .ok()
        .filter(|size| size.columns > 0 && size.rows > 0 && size.width > 0 && size.height > 0)
        .map(|size| {
            (
                u32::from(size.width / size.columns).max(1),
                u32::from(size.height / size.rows).max(1),
            )
        })
}

/// Generated output is larger than an image shown as a tool detail.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreviewKind {
    Inspected,
    Generated,
}

impl TranscriptImage {
    fn load_pixels(path: &Path, max_width: u32, max_height: u32) -> Option<RgbaImage> {
        if max_width == 0 || max_height == 0 {
            return None;
        }
        let image = Self::decode(path)?;
        Some(
            image
                .thumbnail(max_width.min(image.width()), max_height.min(image.height()))
                .to_rgba8(),
        )
    }

    fn decode(path: &Path) -> Option<DynamicImage> {
        let metadata = path.metadata().ok()?;
        if !metadata.is_file() || metadata.len() > 32 * 1024 * 1024 {
            return None;
        }
        let reader = image::ImageReader::open(path)
            .ok()?
            .with_guessed_format()
            .ok()?;
        // The reader checks the decoded size before allocating the raster. Keep normal
        // screenshots usable, but never allocate hundreds of MB for an optional thumbnail.
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(32 * 1024 * 1024);
        crate::image_decode::decode_oriented(reader, limits).ok()
    }

    pub(crate) fn load(
        path: &Path,
        width: u16,
        protocol: ImageProtocol,
        kind: PreviewKind,
    ) -> Option<Self> {
        let columns = width.saturating_sub(2);
        if columns == 0 {
            return None;
        }
        // Sixel paints physical pixels. Guessing could paint outside the reserved rows.
        let (cell_width, cell_height) = match protocol {
            ImageProtocol::Sixel => terminal_cell_size()?,
            ImageProtocol::Kitty | ImageProtocol::KittyLocalFile => match kind {
                PreviewKind::Inspected => (8, 16),
                PreviewKind::Generated => terminal_cell_size().unwrap_or((8, 16)),
            },
        };
        let image = Self::decode(path)?;
        // Keep the current Sixel encoding and tmux placeholder budgets.
        let (columns, max_width, max_height) = match (protocol, kind) {
            (ImageProtocol::Sixel, _) => (columns.min(48), 160, 80.min(6 * cell_height)),
            (_, PreviewKind::Inspected) => (columns.min(48), 384, 6 * cell_height),
            (_, PreviewKind::Generated) => {
                let ratio = f64::from(image.width()) / f64::from(image.height());
                let max_width = if ratio >= 2.0 {
                    768
                } else if ratio >= 1.01 {
                    480
                } else {
                    400
                };
                let max_height = if crate::tui::tmux_fullscreen_image_base().is_some() {
                    6 * cell_height
                } else {
                    400
                };
                (columns, max_width, max_height)
            }
        };
        let max_width = (u32::from(columns) * cell_width).min(max_width);
        let pixels = image
            .thumbnail(max_width.min(image.width()), max_height.min(image.height()))
            .to_rgba8();
        Some(Self {
            protocol,
            columns: pixels.width().div_ceil(cell_width) as u16,
            rows: pixels.height().div_ceil(cell_height) as u16,
            pixels,
            cell_width,
            cell_height,
        })
    }

    /// Draw only the visible vertical slice, never covering the prompt header or composer.
    pub(crate) fn draw(
        &self,
        writer: &mut impl Write,
        area: Rect,
        first_row: usize,
        placement: KittyPlacement,
    ) -> Result<Option<tempfile::NamedTempFile>> {
        let (top, bottom) = match self.protocol {
            ImageProtocol::Sixel => (
                first_row as u32 * self.cell_height,
                (first_row as u32 + u32::from(area.height)) * self.cell_height,
            ),
            // Kitty scales the entire raster to r=rows; clip at those scaled boundaries.
            ImageProtocol::Kitty | ImageProtocol::KittyLocalFile => (
                first_row as u32 * self.pixels.height() / u32::from(self.rows),
                (first_row as u32 + u32::from(area.height)) * self.pixels.height()
                    / u32::from(self.rows),
            ),
        };
        let top = top.min(self.pixels.height());
        let height = bottom.min(self.pixels.height()) - top;
        if height == 0 {
            return Ok(None);
        }
        let cropped =
            image::imageops::crop_imm(&self.pixels, 0, top, self.pixels.width(), height).to_image();
        let mut file = None;
        // Encoding and temp-file failures must not abort the TUI. Only writes to the actual
        // terminal below are fatal; the existing text entry remains usable without its preview.
        let payload = (|| -> Result<_> {
            Ok(match self.protocol {
                ImageProtocol::Kitty | ImageProtocol::KittyLocalFile => {
                    let mut png = tempfile::Builder::new().suffix(".png").tempfile()?;
                    DynamicImage::ImageRgba8(cropped).write_to(&mut png, ImageFormat::Png)?;
                    png.flush()?;
                    let transmit = if self.protocol == ImageProtocol::KittyLocalFile {
                        crate::pets::kitty_transmit_png_file_with_id
                    } else {
                        crate::pets::kitty_transmit_png_with_id
                    };
                    let command = transmit(png.path(), self.columns, area.height, placement)?;
                    file = (self.protocol == ImageProtocol::KittyLocalFile).then_some(png);
                    command.into_bytes()
                }
                ImageProtocol::Sixel => {
                    crate::pets::encode_rgba(cropped.as_raw(), cropped.width(), cropped.height())?
                }
            })
        })();
        let Ok(payload) = payload else {
            return Ok(None);
        };
        queue!(writer, SavePosition)?;
        placement.draw(writer, area, &payload)?;
        queue!(writer, RestorePosition)?;
        Ok(file)
    }
}
