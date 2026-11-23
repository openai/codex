//! Decode a workspace image once and reuse the transcript's terminal image lifecycle.

use std::io::Cursor;
use std::sync::Arc;

use color_eyre::eyre::OptionExt;
use color_eyre::eyre::Result;
use image::DynamicImage;
use image::ImageFormat;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Widget;

use crate::pets::ImageProtocol;
use crate::transcript_image::ImagePlacement;
use crate::transcript_image::TranscriptImage;
use crate::transcript_image::terminal_cell_size;

#[derive(Debug)]
pub(crate) enum DocumentContent {
    Text(String),
    Image(Box<SidebarImage>),
}

pub(crate) struct SidebarImage {
    label: String,
    source_size: (u32, u32),
    raster: DynamicImage,
    fitted: Option<Arc<TranscriptImage>>,
}

impl std::fmt::Debug for SidebarImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SidebarImage")
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}

pub(crate) fn supports_path(path: &str) -> bool {
    path.rsplit_once('.')
        .and_then(|(_, extension)| ImageFormat::from_extension(extension))
        .is_some_and(supported_format)
}

fn supported_format(format: ImageFormat) -> bool {
    matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Gif | ImageFormat::WebP
    )
}

impl SidebarImage {
    pub(crate) fn decode(bytes: &[u8]) -> Result<Self> {
        let reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
        let format = reader
            .format()
            .filter(|format| supported_format(*format))
            .ok_or_eyre("Unsupported image format")?;
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(32 * 1024 * 1024);
        let image = crate::image_decode::decode_oriented(reader, limits)?;
        let (width, height) = (image.width(), image.height());
        Ok(Self {
            label: format!(
                "{} · {width} × {height}",
                format.extensions_str()[0].to_ascii_uppercase()
            ),
            source_size: (width, height),
            raster: if width > 768 || height > 768 {
                image.thumbnail(768, 768)
            } else {
                image
            },
            fitted: None,
        })
    }

    pub(crate) fn paint(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        protocol: Option<ImageProtocol>,
    ) -> Option<ImagePlacement> {
        Line::from(self.label.as_str()).dim().render(area, buf);
        let available_rows = area.height.saturating_sub(1 + u16::from(area.height > 2));
        // tmux uses Kitty's virtual placements. The shared placeholder renderer
        // currently emits row marks for up to six rows.
        let rows = if crate::tui::tmux_fullscreen_image_base().is_some() {
            available_rows.min(6)
        } else {
            available_rows
        };
        let body = Rect::new(area.x, area.y.saturating_add(1), area.width, rows);
        if body.is_empty() {
            return None;
        }
        let Some((protocol, (cw, ch))) = protocol.and_then(|protocol| {
            let size = terminal_cell_size()
                .or_else(|| (protocol != ImageProtocol::Sixel).then_some((8, 16)))?;
            Some((protocol, size))
        }) else {
            Line::from("Image preview unavailable in this terminal").render(body, buf);
            return None;
        };
        let (width, height) = self.source_size;
        let (max_width, max_height) = match protocol {
            ImageProtocol::Sixel => (160, 80),
            ImageProtocol::Kitty | ImageProtocol::KittyLocalFile => (width, height),
        };
        let scale = (f64::from((u32::from(body.width) * cw).min(max_width)) / f64::from(width))
            .min(f64::from((u32::from(body.height) * ch).min(max_height)) / f64::from(height))
            .min(1.0);
        let pixel_width = (f64::from(width) * scale).round().max(1.0) as u32;
        let pixel_height = (f64::from(height) * scale).round().max(1.0) as u32;
        let columns = pixel_width.div_ceil(cw) as u16;
        let rows = pixel_height.div_ceil(ch) as u16;
        if !self.fitted.as_ref().is_some_and(|image| {
            image.protocol == protocol
                && image.cell_width == cw
                && image.cell_height == ch
                && image.columns == columns
                && image.rows == rows
        }) {
            let pixels = self
                .raster
                .thumbnail(
                    pixel_width.min(self.raster.width()),
                    pixel_height.min(self.raster.height()),
                )
                .to_rgba8();
            self.fitted = Some(Arc::new(TranscriptImage {
                protocol,
                columns,
                rows,
                pixels,
                cell_width: cw,
                cell_height: ch,
            }));
        }
        let image = self.fitted.as_ref()?;
        Some(ImagePlacement {
            image: Arc::clone(image),
            area: Rect::new(
                body.x + (body.width - image.columns) / 2,
                body.y + (body.height - image.rows) / 2,
                image.columns,
                image.rows,
            ),
            first_row: 0,
            file: None,
            terminal_id: None,
        })
    }
}
