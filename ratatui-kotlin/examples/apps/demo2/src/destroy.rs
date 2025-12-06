use rand::Rng;
use rand_chacha::rand_core::SeedableRng;
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Flex, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Text;
use ratatui::widgets::Widget;

/// delay the start of the animation so it doesn't start immediately
const DELAY: usize = 120;
/// higher means more pixels per frame are modified in the animation
const DRIP_SPEED: usize = 500;
/// delay the start of the text animation so it doesn't start immediately after the initial delay
const TEXT_DELAY: usize = 180;

/// Destroy mode activated by pressing `d`
pub fn destroy(frame: &mut Frame<'_>) {
    let frame_count = frame.count().saturating_sub(DELAY);
    if frame_count == 0 {
        return;
    }

    let area = frame.area();
    let buf = frame.buffer_mut();

    drip(frame_count, area, buf);
    text(frame_count, area, buf);
}

/// Move a bunch of random pixels down one row.
///
/// Each pick some random pixels and move them each down one row. This is a very inefficient way to
/// do this, but it works well enough for this demo.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn drip(frame_count: usize, area: Rect, buf: &mut Buffer) {
    // a seeded rng as we have to move the same random pixels each frame
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(10);
    let ramp_frames = 450;
    let fractional_speed = frame_count as f64 / f64::from(ramp_frames);
    let variable_speed = DRIP_SPEED as f64 * fractional_speed * fractional_speed * fractional_speed;
    let pixel_count = (frame_count as f64 * variable_speed).floor() as usize;
    for _ in 0..pixel_count {
        let src_x = rng.random_range(0..area.width);
        let src_y = rng.random_range(1..area.height - 2);
        let src = buf[(src_x, src_y)].clone();
        // 1% of the time, move a blank or pixel (10:1) to the top line of the screen
        if rng.random_ratio(1, 100) {
            let dest_x = rng
                .random_range(src_x.saturating_sub(5)..src_x.saturating_add(5))
                .clamp(area.left(), area.right() - 1);
            let dest_y = area.top() + 1;

            let dest = &mut buf[(dest_x, dest_y)];
            // copy the cell to the new location about 1/10 of the time blank out the cell the rest
            // of the time. This has the effect of gradually removing the pixels from the screen.
            if rng.random_ratio(1, 10) {
                *dest = src;
            } else {
                dest.reset();
            }
        } else {
            // move the pixel down one row
            let dest_x = src_x;
            let dest_y = src_y.saturating_add(1).min(area.bottom() - 2);
            // copy the cell to the new location
            buf[(dest_x, dest_y)] = src;
        }
    }
}

/// draw some text fading in and out from black to red and back
#[expect(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn text(frame_count: usize, area: Rect, buf: &mut Buffer) {
    let sub_frame = frame_count.saturating_sub(TEXT_DELAY);
    if sub_frame == 0 {
        return;
    }

    let logo = indoc::indoc! {"
        ██████      ████    ██████    ████    ██████  ██    ██  ██
        ██    ██  ██    ██    ██    ██    ██    ██    ██    ██  ██
        ██████    ████████    ██    ████████    ██    ██    ██  ██
        ██  ██    ██    ██    ██    ██    ██    ██    ██    ██  ██
        ██    ██  ██    ██    ██    ██    ██    ██      ████    ██
    "};
    let logo_text = Text::styled(logo, Color::Rgb(255, 255, 255));
    let area = centered_rect(area, logo_text.width() as u16, logo_text.height() as u16);

    let mask_buf = &mut Buffer::empty(area);
    logo_text.render(area, mask_buf);

    let percentage = (sub_frame as f64 / 480.0).clamp(0.0, 1.0);

    for row in area.rows() {
        for col in row.columns() {
            let cell = &mut buf[(col.x, col.y)];
            let mask_cell = &mut mask_buf[(col.x, col.y)];
            cell.set_symbol(mask_cell.symbol());

            // blend the mask cell color with the cell color
            let cell_color = cell.style().bg.unwrap_or(Color::Rgb(0, 0, 0));
            let mask_color = mask_cell.style().fg.unwrap_or(Color::Rgb(255, 0, 0));

            let color = blend(mask_color, cell_color, percentage);
            cell.set_style(Style::new().fg(color));
        }
    }
}

fn blend(mask_color: Color, cell_color: Color, percentage: f64) -> Color {
    let Color::Rgb(mask_red, mask_green, mask_blue) = mask_color else {
        return mask_color;
    };
    let Color::Rgb(cell_red, cell_green, cell_blue) = cell_color else {
        return mask_color;
    };

    let remain = 1.0 - percentage;

    let red = f64::from(mask_red).mul_add(percentage, f64::from(cell_red) * remain);
    let green = f64::from(mask_green).mul_add(percentage, f64::from(cell_green) * remain);
    let blue = f64::from(mask_blue).mul_add(percentage, f64::from(cell_blue) * remain);

    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Color::Rgb(red as u8, green as u8, blue as u8)
}

/// a centered rect of the given size
fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let horizontal = Layout::horizontal([width]).flex(Flex::Center);
    let vertical = Layout::vertical([height]).flex(Flex::Center);
    let [area] = area.layout(&vertical);
    let [area] = area.layout(&horizontal);
    area
}
