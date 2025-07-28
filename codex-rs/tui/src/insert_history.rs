use std::io;
use std::io::Write;

use crate::tui;
use crossterm::queue;
use crossterm::style::Color as CColor;
use crossterm::style::Colors;
use crossterm::style::Print;
use crossterm::style::SetAttribute;
use crossterm::style::SetBackgroundColor;
use crossterm::style::SetColors;
use crossterm::style::SetForegroundColor;
use ratatui::layout::Position;
use ratatui::prelude::Backend;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::text::Line;
use ratatui::text::Span;

pub(crate) fn insert_history_lines(terminal: &mut tui::Tui, lines: Vec<Line<'static>>) {
    let screen_height = terminal
        .backend()
        .size()
        .map(|s| s.height)
        .unwrap_or(0xffffu16);
    let mut area = terminal.get_frame().area();
    // We scroll up one line at a time because we can't position the cursor
    // above the top of the screen. i.e. if
    //   lines.len() > screen_height - area.top()
    // we would need to print the first line above the top of the screen, which
    // can't be done.
    for line in lines.into_iter() {
        // 1. Scroll everything above the viewport up by one line
        if area.bottom() >= screen_height {
            let top = area.top();
            terminal.backend_mut().scroll_region_up(0..top, 1).ok();
            // 2. Move the cursor to the blank line
            terminal.set_cursor_position(Position::new(0, top - 1)).ok();
        } else {
            // If the viewport isn't at the bottom of the screen, scroll down instead
            terminal
                .backend_mut()
                .scroll_region_down(area.top()..area.bottom() + 1, 1)
                .ok();
            terminal
                .set_cursor_position(Position::new(0, area.top()))
                .ok();
            area.y += 1;
        }
        // 3. Write the line
        write_spans(&mut std::io::stdout(), line.iter()).ok();
    }
    terminal.set_viewport_area(area);
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
