use std::fmt;
use std::io;
use std::io::Write;

use crate::tui;
use crossterm::Command;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::Color as CColor;
use crossterm::style::Colors;
use crossterm::style::Print;
use crossterm::style::SetAttribute;
use crossterm::style::SetBackgroundColor;
use crossterm::style::SetColors;
use crossterm::style::SetForegroundColor;
use ratatui::layout::Size;
use ratatui::prelude::Backend;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::text::Line;
use ratatui::text::Span;

/// Insert `lines` above the viewport.
pub(crate) fn insert_history_lines(terminal: &mut tui::Tui, lines: Vec<Line>) {
    let screen_size = terminal.backend().size().unwrap_or(Size::new(0, 0));
    let cursor_pos = terminal.get_cursor_position().ok();

    let mut area = terminal.get_frame().area();

    let wrapped_lines = wrapped_line_count(&lines, area.width);
    let cursor_top = if area.bottom() < screen_size.height {
        // If the viewport is not at the bottom of the screen, scroll it down to make room.
        // Don't scroll it past the bottom of the screen.
        let scroll_amount = wrapped_lines.min(screen_size.height - area.bottom());
        terminal
            .backend_mut()
            .scroll_region_down(area.top()..screen_size.height, scroll_amount)
            .ok();
        let cursor_top = area.top().saturating_sub(1);
        area.y += scroll_amount;
        terminal.set_viewport_area(area);
        cursor_top
    } else {
        area.top().saturating_sub(1)
    };

    // Limit the scroll region to the lines from the top of the screen to the
    // top of the viewport. With this in place, when we add lines inside this
    // area, only the lines in this area will be scrolled. We place the cursor
    // at the end of the scroll region, and add lines starting there.
    //
    // ┌─Screen───────────────────────┐
    // │┌╌Scroll region╌╌╌╌╌╌╌╌╌╌╌╌╌╌┐│
    // │┆                            ┆│
    // │┆                            ┆│
    // │┆                            ┆│
    // │█╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┘│
    // │╭─Viewport───────────────────╮│
    // ││                            ││
    // │╰────────────────────────────╯│
    // └──────────────────────────────┘
    queue!(std::io::stdout(), SetScrollRegion(1..area.top())).ok();

    // NB: we are using MoveTo instead of set_cursor_position here to avoid messing with the
    // terminal's last_known_cursor_position, which hopefully will still be accurate after we
    // fetch/restore the cursor position. insert_history_lines should be cursor-position-neutral :)
    queue!(std::io::stdout(), MoveTo(0, cursor_top)).ok();

    for line in lines {
        queue!(std::io::stdout(), Print("\r\n")).ok();
        write_spans(&mut std::io::stdout(), line.iter()).ok();
    }

    queue!(std::io::stdout(), ResetScrollRegion).ok();

    // Restore the cursor position to where it was before we started.
    if let Some(cursor_pos) = cursor_pos {
        queue!(std::io::stdout(), MoveTo(cursor_pos.x, cursor_pos.y)).ok();
    }
}

fn wrapped_line_count(lines: &[Line], width: u16) -> u16 {
    let mut count = 0;
    for line in lines {
        count += line_height(line, width);
    }
    count
}

fn line_height(line: &Line, width: u16) -> u16 {
    use unicode_width::UnicodeWidthStr;
    // get the total display width of the line, accounting for double-width chars
    let total_width = line
        .spans
        .iter()
        .map(|span| span.content.width())
        .sum::<usize>();
    // divide by width to get the number of lines, rounding up
    if width == 0 {
        1
    } else {
        (total_width as u16).div_ceil(width).max(1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetScrollRegion(pub std::ops::Range<u16>);

impl Command for SetScrollRegion {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "\x1b[{};{}r", self.0.start, self.0.end)
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        panic!("tried to execute SetScrollRegion command using WinAPI, use ANSI instead");
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        // TODO(nornagon): is this supported on Windows?
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResetScrollRegion;

impl Command for ResetScrollRegion {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "\x1b[r")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        panic!("tried to execute ResetScrollRegion command using WinAPI, use ANSI instead");
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        // TODO(nornagon): is this supported on Windows?
        true
    }
}

struct ModifierDiff {
    pub from: Modifier,
    pub to: Modifier,
}

impl ModifierDiff {
    fn queue<W>(self, mut w: W) -> io::Result<()>
    where
        W: io::Write,
    {
        use crossterm::style::Attribute as CAttribute;
        let removed = self.from - self.to;
        if removed.contains(Modifier::REVERSED) {
            queue!(w, SetAttribute(CAttribute::NoReverse))?;
        }
        if removed.contains(Modifier::BOLD) {
            queue!(w, SetAttribute(CAttribute::NormalIntensity))?;
            if self.to.contains(Modifier::DIM) {
                queue!(w, SetAttribute(CAttribute::Dim))?;
            }
        }
        if removed.contains(Modifier::ITALIC) {
            queue!(w, SetAttribute(CAttribute::NoItalic))?;
        }
        if removed.contains(Modifier::UNDERLINED) {
            queue!(w, SetAttribute(CAttribute::NoUnderline))?;
        }
        if removed.contains(Modifier::DIM) {
            queue!(w, SetAttribute(CAttribute::NormalIntensity))?;
        }
        if removed.contains(Modifier::CROSSED_OUT) {
            queue!(w, SetAttribute(CAttribute::NotCrossedOut))?;
        }
        if removed.contains(Modifier::SLOW_BLINK) || removed.contains(Modifier::RAPID_BLINK) {
            queue!(w, SetAttribute(CAttribute::NoBlink))?;
        }

        let added = self.to - self.from;
        if added.contains(Modifier::REVERSED) {
            queue!(w, SetAttribute(CAttribute::Reverse))?;
        }
        if added.contains(Modifier::BOLD) {
            queue!(w, SetAttribute(CAttribute::Bold))?;
        }
        if added.contains(Modifier::ITALIC) {
            queue!(w, SetAttribute(CAttribute::Italic))?;
        }
        if added.contains(Modifier::UNDERLINED) {
            queue!(w, SetAttribute(CAttribute::Underlined))?;
        }
        if added.contains(Modifier::DIM) {
            queue!(w, SetAttribute(CAttribute::Dim))?;
        }
        if added.contains(Modifier::CROSSED_OUT) {
            queue!(w, SetAttribute(CAttribute::CrossedOut))?;
        }
        if added.contains(Modifier::SLOW_BLINK) {
            queue!(w, SetAttribute(CAttribute::SlowBlink))?;
        }
        if added.contains(Modifier::RAPID_BLINK) {
            queue!(w, SetAttribute(CAttribute::RapidBlink))?;
        }

        Ok(())
    }
}

fn write_spans<'a, I>(mut writer: &mut impl Write, content: I) -> io::Result<()>
where
    I: Iterator<Item = &'a Span<'a>>,
{
    let mut fg = Color::Reset;
    let mut bg = Color::Reset;
    let mut modifier = Modifier::empty();
    for span in content {
        let mut next_modifier = modifier;
        next_modifier.insert(span.style.add_modifier);
        next_modifier.remove(span.style.sub_modifier);
        if next_modifier != modifier {
            let diff = ModifierDiff {
                from: modifier,
                to: next_modifier,
            };
            diff.queue(&mut writer)?;
            modifier = next_modifier;
        }
        let next_fg = span.style.fg.unwrap_or(Color::Reset);
        let next_bg = span.style.bg.unwrap_or(Color::Reset);
        if next_fg != fg || next_bg != bg {
            queue!(
                writer,
                SetColors(Colors::new(next_fg.into(), next_bg.into()))
            )?;
            fg = next_fg;
            bg = next_bg;
        }

        queue!(writer, Print(span.content.clone()))?;
    }

    queue!(
        writer,
        SetForegroundColor(CColor::Reset),
        SetBackgroundColor(CColor::Reset),
        SetAttribute(crossterm::style::Attribute::Reset),
    )
}
