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
use crossterm::cursor::MoveTo;
use crossterm::cursor::RestorePosition;
use crossterm::cursor::SavePosition;
use crossterm::queue;
use image::DynamicImage;
use image::ImageDecoder;
use image::ImageFormat;
use image::RgbaImage;
use ratatui::layout::Rect;

use crate::pets::ImageProtocol;

// Separate from the two image ids reserved for pets (0xC0DE and 0xC0DF).
pub(crate) const FIRST_IMAGE_ID: u32 = 0xC100;

pub(crate) struct ImagePlacement {
    pub(crate) image: Arc<TranscriptImage>,
    pub(crate) area: Rect,
    pub(crate) first_row: usize,
    pub(crate) file: Option<tempfile::NamedTempFile>,
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

impl TranscriptImage {
    fn load_pixels(path: &Path, max_width: u32, max_height: u32) -> Option<RgbaImage> {
        let metadata = path.metadata().ok()?;
        if max_width == 0 || !metadata.is_file() || metadata.len() > 32 * 1024 * 1024 {
            return None;
        }
        let mut reader = image::ImageReader::open(path)
            .ok()?
            .with_guessed_format()
            .ok()?;
        // The reader checks the decoded size before allocating the raster. Keep normal
        // screenshots usable, but never allocate hundreds of MB for an optional thumbnail.
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(32 * 1024 * 1024);
        reader.limits(limits.clone());
        let mut decoder = reader.into_decoder().ok()?;
        limits.reserve(decoder.total_bytes()).ok()?;
        decoder.set_limits(limits).ok()?;
        let orientation = decoder.orientation().ok();
        let mut image = DynamicImage::from_decoder(decoder).ok()?;
        if let Some(orientation) = orientation {
            image.apply_orientation(orientation);
        }
        Some(
            image
                .thumbnail(max_width.min(image.width()), max_height.min(image.height()))
                .to_rgba8(),
        )
    }

    pub(crate) fn load(path: &Path, width: u16, protocol: ImageProtocol) -> Option<Self> {
        let columns = width.saturating_sub(2).min(48);
        // Sixel paints physical pixels. Guessing could paint outside the reserved rows.
        let (cell_width, cell_height) = match protocol {
            ImageProtocol::Sixel => terminal_cell_size()?,
            ImageProtocol::Kitty | ImageProtocol::KittyLocalFile => (8, 16),
        };
        // The sprite encoder scans each active color per Sixel band. Bound both its CPU
        // work and worst-case output even with large terminal cells or high-entropy images.
        let (max_width, max_height) = match protocol {
            ImageProtocol::Sixel => (160, 80),
            ImageProtocol::Kitty | ImageProtocol::KittyLocalFile => (384, 160),
        };
        let pixels = Self::load_pixels(
            path,
            (u32::from(columns) * cell_width).min(max_width),
            (6 * cell_height).min(max_height),
        )?;
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
        id: u32,
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
                    let command = transmit(png.path(), self.columns, area.height, Some(id))?;
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
        queue!(writer, SavePosition, MoveTo(area.x, area.y))?;
        writer.write_all(&payload)?;
        queue!(writer, RestorePosition)?;
        Ok(file)
    }
}
