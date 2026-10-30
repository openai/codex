//! The draft thumbnail strip keeps only visible rasters between draws and places them
//! in textarea reference order. Attachment numbering and submission order are independent.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use image::Rgba;
use image::RgbaImage;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::Wrap;

use super::ImagePlacement;
use super::TranscriptImage;
use super::terminal_cell_size;
use crate::bottom_pane::LocalImageAttachment;
use crate::bottom_pane::TextArea;
use crate::pets::ImageProtocol;

// Six columns by three rows is square with typical 8 × 16 pixel cells.
const WIDTH: u16 = 6;
const HEIGHT: u16 = 3;

#[derive(Hash, PartialEq, Eq)]
struct TileKey {
    path: PathBuf,
    version: Option<(u64, std::time::SystemTime)>,
}

pub(crate) struct ComposerImages {
    protocol: ImageProtocol,
    cell_size: (u32, u32),
    background: (u8, u8, u8),
    cache: RefCell<HashMap<TileKey, Option<Arc<TranscriptImage>>>>,
    pub(crate) placements: RefCell<Vec<ImagePlacement>>,
}

impl ComposerImages {
    pub(crate) fn for_terminal(protocol: ImageProtocol, previous: Option<Self>) -> Option<Self> {
        // Kitty can scale a nominal raster into its cell placement. Sixel needs physical pixels.
        let cell_size = terminal_cell_size()
            .or_else(|| (protocol != ImageProtocol::Sixel).then_some((8, 16)))?;
        let background = crate::terminal_palette::default_bg()
            .map(crate::style::user_message_bg_rgb)
            .unwrap_or((48, 48, 48));
        if previous.as_ref().is_some_and(|old| {
            (old.protocol, old.cell_size, old.background) == (protocol, cell_size, background)
        }) {
            return previous;
        }
        Some(Self {
            protocol,
            cell_size,
            background,
            cache: RefCell::default(),
            placements: RefCell::default(),
        })
    }

    /// Reserve a blank row between any visible strip and the draft.
    pub(crate) fn height(&self, width: u16, count: usize) -> u16 {
        let (visible, hidden) = visible_thumbnails(width, count);
        if visible > 0 {
            HEIGHT + 1
        } else if hidden > 0 && u32::from(width) >= 2 + hidden.ilog10() {
            2
        } else {
            0
        }
    }

    pub(crate) fn render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        attachments: &[LocalImageAttachment],
        textarea: &TextArea,
    ) {
        let (visible, hidden) = visible_thumbnails(area.width, attachments.len());
        let mut cache = self.cache.borrow_mut();
        let mut previous = std::mem::take(&mut *cache);
        let mut placements = self.placements.borrow_mut();
        placements.clear();
        let style = crate::style::secondary_text_style();
        let mut x = area.x;
        if area.is_empty() {
            return;
        }
        let mut ordered: Vec<_> = attachments.iter().collect();
        if ordered.len() > 1 {
            let positions: HashMap<_, _> = textarea
                .text_element_ranges()
                .map(|range| (&textarea.text()[range.clone()], range.start))
                .collect();
            ordered.sort_by_key(|image| {
                positions
                    .get(image.placeholder.as_str())
                    .copied()
                    .unwrap_or(usize::MAX)
            });
        }
        for attachment in ordered.into_iter().take(visible) {
            let path = &attachment.path;
            let key = TileKey {
                path: path.clone(),
                version: path
                    .metadata()
                    .ok()
                    .and_then(|m| Some((m.len(), m.modified().ok()?))),
            };
            let image = cache.entry(key).or_insert_with_key(|key| {
                previous
                    .remove(key)
                    .unwrap_or_else(|| self.load_tile(path).map(Arc::new))
            });
            let tile = Rect::new(x, area.y, WIDTH, HEIGHT);
            if let Some(image) = image {
                placements.push(ImagePlacement {
                    image: image.clone(),
                    area: tile,
                    first_row: 0,
                    file: None,
                });
            } else {
                Paragraph::new(attachment.placeholder.as_str())
                    .style(style)
                    .wrap(Wrap { trim: false })
                    .render(tile, buf);
            }
            x = x.saturating_add(WIDTH + 1);
        }
        if hidden > 0 {
            buf.set_string(x, area.y, format!("+{hidden}"), style);
        }
    }

    fn load_tile(&self, path: &Path) -> Option<TranscriptImage> {
        let (cell_width, cell_height) = self.cell_size;
        let width = u32::from(WIDTH) * cell_width;
        let height = u32::from(HEIGHT) * cell_height;
        let scale = width.div_ceil(384).max(height.div_ceil(160));
        // Kitty scales the raster to the tile. Sixel must paint at the physical cell size.
        if scale > 1 && self.protocol == ImageProtocol::Sixel {
            return None;
        }
        let width = (width / scale).max(3);
        let height = (height / scale).max(3);
        let pixels = TranscriptImage::load_pixels(path, width - 2, height - 2)?;
        let (r, g, b) = self.background;
        let contrast = if crate::color::is_light(self.background) {
            0
        } else {
            255
        };
        let (red, green, blue) = crate::color::blend(
            (contrast, contrast, contrast),
            self.background,
            /*alpha*/ 0.45,
        );
        let edge = Rgba([red, green, blue, 255]);
        let mut tile = RgbaImage::from_fn(width, height, |x, y| {
            if x == 0 || y == 0 || x == width - 1 || y == height - 1 {
                edge
            } else {
                Rgba([r, g, b, 255])
            }
        });
        image::imageops::overlay(
            &mut tile,
            &pixels,
            i64::from((width - pixels.width()) / 2),
            i64::from((height - pixels.height()) / 2),
        );
        Some(TranscriptImage {
            protocol: self.protocol,
            columns: WIDTH,
            rows: HEIGHT,
            pixels: tile,
            cell_width,
            cell_height,
        })
    }
}

fn visible_thumbnails(width: u16, count: usize) -> (usize, usize) {
    let mut visible = ((usize::from(width) + 1) / usize::from(WIDTH + 1)).min(count);
    while visible > 0 && visible < count {
        let badge = 2 + (count - visible).ilog10() as usize;
        if visible * usize::from(WIDTH + 1) + badge <= usize::from(width) {
            break;
        }
        visible -= 1;
    }
    (visible, count - visible)
}
