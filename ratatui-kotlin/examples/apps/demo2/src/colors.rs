use palette::{IntoColor, Okhsv, Srgb};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::Widget;

/// A widget that renders a color swatch of RGB colors.
///
/// The widget is rendered as a rectangle with the hue changing along the x-axis from 0.0 to 360.0
/// and the value changing along the y-axis (from 1.0 to 0.0). Each pixel is rendered as a block
/// character with the top half slightly lighter than the bottom half.
pub struct RgbSwatch;

impl Widget for RgbSwatch {
    #[expect(clippy::cast_precision_loss, clippy::similar_names)]
    fn render(self, area: Rect, buf: &mut Buffer) {
        for (yi, y) in (area.top()..area.bottom()).enumerate() {
            let value = f32::from(area.height) - yi as f32;
            let value_fg = value / f32::from(area.height);
            let value_bg = (value - 0.5) / f32::from(area.height);
            for (xi, x) in (area.left()..area.right()).enumerate() {
                let hue = xi as f32 * 360.0 / f32::from(area.width);
                let fg = color_from_oklab(hue, Okhsv::max_saturation(), value_fg);
                let bg = color_from_oklab(hue, Okhsv::max_saturation(), value_bg);
                buf[(x, y)].set_char('▀').set_fg(fg).set_bg(bg);
            }
        }
    }
}

/// Convert a hue and value into an RGB color via the Oklab color space.
///
/// See <https://bottosson.github.io/posts/oklab/> for more details.
pub fn color_from_oklab(hue: f32, saturation: f32, value: f32) -> Color {
    let color: Srgb = Okhsv::new(hue, saturation, value).into_color();
    let color = color.into_format();
    Color::Rgb(color.red, color.green, color.blue)
}
