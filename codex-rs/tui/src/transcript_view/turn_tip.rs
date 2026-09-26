//! Reserve a temporary tip before completion metadata, outside selectable source text.
//! Return its final draw area only when at least one response row remains visible.

use super::*;
use crate::turn_tip::TurnTip;

impl TranscriptView {
    pub(crate) fn render_with_turn_tip_space(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        cells: &[Arc<dyn HistoryCell>],
        tip: Option<&TurnTip>,
    ) -> Option<Rect> {
        let tip = tip.filter(|_| area.height > 1);
        self.turn_tip_key = tip.and_then(|_| {
            cells
                .last()
                .filter(|cell| {
                    cell.as_any()
                        .is::<crate::history_cell::FinalMessageSeparator>()
                        && !cell.display_lines(area.width).is_empty()
                })
                .map(EntryKey::cell)
        });
        self.render(
            Rect {
                height: area.height - u16::from(tip.is_some() && self.turn_tip_key.is_none()),
                ..area
            },
            buf,
            cells,
        );
        // The final viewport includes prompt-header and footer reservations. Let the tip yield
        // rather than leave only completion metadata visible on a short terminal.
        if let Some(key) = self.turn_tip_key
            && self.visible.first().is_none_or(|row| row.key == key)
        {
            self.turn_tip_key = None;
            self.render(area, buf, cells);
            return None;
        }
        tip?;
        let row = match self.turn_tip_key {
            Some(key) => self
                .visible
                .iter()
                .position(|row| row.key == key && row.row == 0)?,
            None => self.visible.len(),
        };
        let y = self.area.y.saturating_add(row as u16);
        (self.tail_visible && !self.visible.is_empty() && y < area.bottom()).then_some(Rect::new(
            self.area.x,
            y,
            self.area.width,
            /*height*/ 1,
        ))
    }
}
