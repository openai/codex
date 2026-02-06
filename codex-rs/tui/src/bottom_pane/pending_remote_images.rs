use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::render::renderable::Renderable;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_lines;

/// Widget that displays pending non-editable remote image URLs above the composer.
pub(crate) struct PendingRemoteImages {
    pub urls: Vec<String>,
}

impl PendingRemoteImages {
    pub(crate) fn new() -> Self {
        Self { urls: Vec::new() }
    }

    pub(crate) fn lines(&self, width: u16) -> Vec<Line<'static>> {
        if self.urls.is_empty() || width < 4 {
            return Vec::new();
        }

        let total_remote_images = self.urls.len();
        let mut lines = word_wrap_lines(
            self.urls.iter().enumerate().map(|(idx, url)| {
                remote_image_display_line(url, idx.saturating_add(1), total_remote_images)
            }),
            RtOptions::new(width as usize)
                .initial_indent(Line::from("  "))
                .subsequent_indent(Line::from("  ")),
        );
        lines.push(Line::from(""));
        lines
    }

    pub(crate) fn panel_height(&self, width: u16) -> u16 {
        self.lines(width).len() as u16
    }

    fn as_renderable(&self, width: u16) -> Box<dyn Renderable> {
        let lines = self.lines(width);
        if lines.is_empty() {
            return Box::new(());
        }

        Paragraph::new(lines).into()
    }
}

impl Renderable for PendingRemoteImages {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        self.as_renderable(area.width).render(area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.as_renderable(width).desired_height(width)
    }
}

fn inline_data_url_summary(url: &str) -> String {
    let Some(data_url_body) = url.strip_prefix("data:") else {
        return "image data URL (size unavailable)".to_string();
    };
    let Some((meta, payload)) = data_url_body.split_once(',') else {
        return "image data URL (size unavailable)".to_string();
    };
    let media_type = meta
        .split(';')
        .next()
        .filter(|media_type| !media_type.is_empty())
        .unwrap_or("image");
    let Some(payload_bytes) = data_url_payload_size_bytes(meta, payload) else {
        return format!("{media_type} data URL (size unavailable)");
    };
    format!("{media_type} data URL ({payload_bytes} bytes)")
}

fn data_url_payload_size_bytes(meta: &str, payload: &str) -> Option<usize> {
    if meta
        .split(';')
        .any(|part| part.eq_ignore_ascii_case("base64"))
    {
        return base64_decoded_len(payload);
    }
    percent_decoded_len(payload)
}

fn base64_decoded_len(payload: &str) -> Option<usize> {
    let mut data_len = 0usize;
    let mut padding = 0usize;
    let mut saw_padding = false;
    for byte in payload.bytes() {
        if byte.is_ascii_whitespace() {
            continue;
        }
        if byte == b'=' {
            saw_padding = true;
            padding = padding.saturating_add(1);
            continue;
        }
        if saw_padding {
            return None;
        }
        if is_base64_char(byte) {
            data_len = data_len.saturating_add(1);
        } else {
            return None;
        }
    }
    if padding > 2 {
        return None;
    }
    let total_len = data_len.saturating_add(padding);
    if !total_len.is_multiple_of(4) {
        return None;
    }
    let decoded_len = (total_len / 4).saturating_mul(3).saturating_sub(padding);
    Some(decoded_len)
}

fn is_base64_char(byte: u8) -> bool {
    matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'/' | b'-' | b'_')
}

fn percent_decoded_len(payload: &str) -> Option<usize> {
    let bytes = payload.as_bytes();
    let mut idx = 0usize;
    let mut decoded_len = 0usize;
    while idx < bytes.len() {
        if bytes[idx] == b'%' {
            if idx + 2 >= bytes.len() {
                return None;
            }
            if !bytes[idx + 1].is_ascii_hexdigit() || !bytes[idx + 2].is_ascii_hexdigit() {
                return None;
            }
            decoded_len = decoded_len.saturating_add(1);
            idx = idx.saturating_add(3);
        } else {
            decoded_len = decoded_len.saturating_add(1);
            idx = idx.saturating_add(1);
        }
    }
    Some(decoded_len)
}

fn remote_image_display_label(index: usize, total: usize) -> String {
    if total > 1 {
        format!("[external image {index}] ")
    } else {
        "[external image] ".to_string()
    }
}

fn remote_image_display_line(url: &str, index: usize, total: usize) -> Line<'static> {
    let label = remote_image_display_label(index, total);
    if url.starts_with("data:") {
        vec![label.dim(), inline_data_url_summary(url).dim()].into()
    } else {
        vec![label.dim(), url.to_string().cyan().underlined()].into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn desired_height_empty() {
        let images = PendingRemoteImages::new();
        assert_eq!(images.desired_height(40), 0);
    }

    #[test]
    fn desired_height_with_images() {
        let images = PendingRemoteImages {
            urls: vec!["https://example.com/a.png".to_string()],
        };
        assert_eq!(images.desired_height(60), 2);
    }
}
