//! A structural mark covers siblings in a single frozen layout. Horizontal motion moves
//! both ends to one sibling, while vertical motion moves the active end. The caret
//! offsets remember where to descend after selecting a parent, even for equal-sized blocks.

use super::*;
use crate::transcript_view::text::blocks::BlockRange;

#[derive(Clone, Copy)]
pub(super) struct BlockMark {
    anchor: usize,
    active: usize,
    // Expanding a rectangle to its table can be reversed until a sibling move.
    table: Option<table::TableMark>,
    // The range that entered section expansion. The path from it to the current
    // range lets BackTab restore the previous level, even when both cover the same text.
    section_origin: Option<(usize, usize)>,
}

impl BlockMark {
    pub(super) fn swap(&mut self) {
        std::mem::swap(&mut self.anchor, &mut self.active);
        if let Some((anchor, active)) = &mut self.section_origin {
            std::mem::swap(anchor, active);
        }
        if let Some(table) = &mut self.table {
            std::mem::swap(&mut table.anchor, &mut table.active);
        }
    }

    fn expansion_target(self, blocks: &[BlockRange]) -> Option<usize> {
        let active = blocks.get(self.active)?;
        let anchor = blocks.get(self.anchor)?;
        if active.expand_to() == anchor.expand_to() {
            active.expand_to()
        } else if active.parent == anchor.parent {
            active.parent
        } else {
            None
        }
    }
}

impl TranscriptView {
    fn block_context(&self) -> Option<(BlockMark, Anchor, &TextLayout)> {
        let mark = self.select_mode?.mark?;
        let Scope::Block(block) = mark.scope else {
            return None;
        };
        let layout = self
            .selection
            .as_ref()?
            .snapshot
            .pinned
            .get(&mark.active.key)?;
        Some((block, mark.active, layout))
    }

    pub(in crate::transcript_view) fn marked_block(&self) -> Option<&BlockRange> {
        let (block, _, layout) = self.block_context()?;
        layout.blocks().get(block.active)
    }

    pub(in crate::transcript_view) fn block_mark_hints(&self) -> Option<[String; 4]> {
        let (block, cursor, layout) = self.block_context()?;
        let blocks = layout.blocks();
        let current = blocks.get(block.active)?;
        let label = if block.anchor == block.active {
            current.label()
        } else if current.heading.is_some() {
            "sections"
        } else {
            "blocks"
        };
        let (expand, compact_expand) = match block.expansion_target(blocks) {
            Some(parent) => (
                format!(" · tab expand to {}", blocks[parent].label()),
                " · tab expand",
            ),
            None => (String::new(), ""),
        };
        let can_shrink = block.anchor == block.active
            && (current.heading.is_some()
                || block.section_origin.is_some()
                || block.table.is_some()
                || layout.block_at(cursor.offset) != Some(block.active));
        let shrink = if can_shrink {
            let backtab = crate::key_hint::shift(KeyCode::Tab).display_label();
            format!(" · {backtab} shrink")
        } else {
            String::new()
        };
        let motion = "←→/hl move · ↑↓/jk extend";
        Some([
            format!(
                "Markdown {label} · {motion}{expand}{shrink} · v mark text · y copy · esc clear"
            ),
            format!("{label} · {motion}{compact_expand}{shrink} · y copy"),
            format!("{label} · ←→ move · ↑↓ extend · y copy"),
            format!("{label} · y copy"),
        ])
    }

    pub(in crate::transcript_view) fn block_hint(&self) -> Option<&'static str> {
        if self.select_cursor_is_on_table() {
            return Some("cell");
        }
        let cursor = self.select_mode?.cursor(self.selection.as_ref()?);
        let layout = &self
            .visible
            .iter()
            .find(|row| row.key == cursor.key)?
            .layout;
        let id = layout.block_at(cursor.offset)?;
        Some(layout.blocks()[id].label())
    }

    pub(super) fn toggle_markdown(&mut self, cells: &[Arc<dyn HistoryCell>]) -> bool {
        if self.marked_block().is_some() {
            self.clear_select_mark();
            return true;
        }
        if self.start_select_table(cells) {
            return true;
        }
        let (Some(mode), Some(selection)) = (self.select_mode, self.selection.as_ref()) else {
            return false;
        };
        let cursor = mode.cursor(selection);
        let snapshot = self.snapshot_cells();
        let cells = snapshot.as_deref().unwrap_or(cells);
        let Some(layout) = self.layout(cells, self.resolve(cells, cursor)) else {
            return false;
        };
        let Some(id) = layout.block_at(cursor.offset) else {
            return false;
        };
        let mark = Mark::at(cursor, SelectionUnit::Character);
        self.install_blocks(
            cells,
            mark,
            BlockMark {
                anchor: id,
                active: id,
                table: None,
                section_origin: None,
            },
        );
        true
    }

    fn install_blocks(&mut self, cells: &[Arc<dyn HistoryCell>], mut mark: Mark, block: BlockMark) {
        mark.scope = Scope::Block(block);
        self.cancel_beginning();
        self.cancel_pending_select_search();
        if let Some(mode) = &mut self.select_mode {
            mode.mark = Some(mark);
            mode.table_caret = None;
            mode.preferred_column = None;
        }
        self.sync_select_range(cells);
        self.reveal_select_cursor(cells);
    }

    pub(super) fn sync_block_range(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        mark: Mark,
        block: BlockMark,
    ) {
        let index = self.resolve(cells, mark.anchor);
        let Some(layout) = self.layout(cells, index) else {
            return;
        };
        let blocks = layout.blocks();
        let (Some(a), Some(b)) = (blocks.get(block.anchor), blocks.get(block.active)) else {
            self.clear_select_mark();
            return;
        };
        let endpoint = |offset| Anchor {
            index,
            offset,
            row_bias: 0,
            ..mark.anchor
        };
        if let Some(selection) = &mut self.selection {
            selection.set_keyboard_block_range(
                endpoint(a.range.start.min(b.range.start)),
                endpoint(a.range.end.max(b.range.end)),
                layout,
            );
        }
    }

    pub(super) fn change_block_level(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> bool {
        let expand = match (code, modifiers) {
            (KeyCode::Up, KeyModifiers::ALT) | (KeyCode::Tab, KeyModifiers::NONE) => true,
            (KeyCode::Down, KeyModifiers::ALT) | (KeyCode::BackTab, KeyModifiers::NONE) => false,
            _ => return false,
        };
        let Some(mut mark) = self.select_mode.and_then(|mode| mode.mark) else {
            return false;
        };
        let snapshot = self.snapshot_cells();
        let cells = snapshot.as_deref().unwrap_or(cells);
        let Some(layout) = self.layout(cells, self.resolve(cells, mark.active)) else {
            return false;
        };
        let blocks = layout.blocks();
        match mark.scope {
            Scope::Table(table) if expand => {
                if let Some(id) = layout.block_at(mark.active.offset) {
                    self.install_blocks(
                        cells,
                        mark,
                        BlockMark {
                            anchor: id,
                            active: id,
                            table: Some(table),
                            section_origin: None,
                        },
                    );
                }
            }
            Scope::Text | Scope::Table(_) => return false,
            Scope::Block(mut block) => {
                let Some(current) = blocks.get(block.active) else {
                    return false;
                };
                if expand {
                    if let Some(parent) = block.expansion_target(blocks) {
                        if blocks[parent].heading.is_some() || current.heading.is_some() {
                            block
                                .section_origin
                                .get_or_insert((block.anchor, block.active));
                        }
                        block.anchor = parent;
                        block.active = parent;
                        self.install_blocks(cells, mark, block);
                    }
                } else if block.active == block.anchor {
                    let origins = block.section_origin.or_else(|| {
                        layout
                            .block_at(mark.anchor.offset)
                            .zip(layout.block_at(mark.active.offset))
                    });
                    let child = |mut id: usize| {
                        while id != block.active {
                            let parent = if block.section_origin.is_some() {
                                blocks[id].expand_to()
                            } else {
                                blocks[id].parent
                            }?;
                            if parent == block.active {
                                return Some(id);
                            }
                            id = parent;
                        }
                        None
                    };
                    let children = origins.and_then(|(a, b)| child(a).zip(child(b)));
                    if let Some((anchor, active)) = children {
                        if block.section_origin == Some((anchor, active))
                            && blocks[active].heading.is_none()
                        {
                            block.section_origin = None;
                        }
                        block.anchor = anchor;
                        block.active = active;
                        self.install_blocks(cells, mark, block);
                    } else if let Some(heading) = current.heading {
                        block.anchor = heading;
                        block.active = heading;
                        block.section_origin = None;
                        self.install_blocks(cells, mark, block);
                    } else if let Some(table) = block.table {
                        mark.scope = Scope::Table(table);
                        if let Some(mode) = &mut self.select_mode {
                            mode.mark = Some(mark);
                        }
                        self.sync_select_range(cells);
                    }
                }
            }
        }
        true
    }

    pub(super) fn move_block(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        mut mark: Mark,
        block: BlockMark,
        motion: CursorMotion,
    ) {
        let Some(layout) = self.layout(cells, self.resolve(cells, mark.active)) else {
            return;
        };
        let blocks = layout.blocks();
        let Some(current) = blocks.get(block.active) else {
            return;
        };
        let mut siblings = blocks.iter().enumerate().filter(|(_, b)| {
            b.parent == current.parent && b.heading.is_some() == current.heading.is_some()
        });
        let next = match motion {
            CursorMotion::Character(KeyCode::Left) => siblings.rfind(|(id, _)| *id < block.active),
            CursorMotion::Character(KeyCode::Right) => siblings.find(|(id, _)| *id > block.active),
            CursorMotion::Rows(n) if n < 0 => siblings
                .rev()
                .filter(|(id, _)| *id < block.active)
                .take(n.unsigned_abs())
                .last(),
            CursorMotion::Rows(n) => siblings
                .filter(|(id, _)| *id > block.active)
                .take(n.unsigned_abs())
                .last(),
            CursorMotion::Beginning | CursorMotion::LineStart => siblings.next(),
            CursorMotion::End | CursorMotion::LineEnd => siblings.next_back(),
            CursorMotion::Character(_)
            | CursorMotion::WordForward
            | CursorMotion::WordBackward
            | CursorMotion::WordEnd => None,
        };
        if let Some((id, next)) = next
            && id != block.active
            && blocks.get(block.anchor).is_some()
        {
            mark.active = Anchor {
                offset: next.range.start,
                row_bias: 0,
                ..mark.active
            };
            let anchor = if matches!(motion, CursorMotion::Character(_)) {
                mark.anchor = mark.active;
                id
            } else {
                block.anchor
            };
            self.install_blocks(
                cells,
                mark,
                BlockMark {
                    anchor,
                    active: id,
                    table: None,
                    section_origin: None,
                },
            );
        }
    }
}
