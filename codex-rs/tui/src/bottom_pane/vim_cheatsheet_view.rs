use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;

const TITLE_STYLE: Style = Style::new().add_modifier(Modifier::BOLD);
const KEY_STYLE: Style = Style::new().add_modifier(Modifier::BOLD);
const HINT_STYLE: Style = Style::new().add_modifier(Modifier::DIM);

struct CheatsheetSection {
    title: &'static str,
    entries: &'static [(&'static str, &'static str)],
}

const SECTIONS: &[CheatsheetSection] = &[
    CheatsheetSection {
        title: "モード",
        entries: &[
            ("Esc", "ノーマルモードに戻る"),
            ("i / a / I / A", "挿入モードへ (位置違い)"),
            ("o / O", "下/上に新行を挿入し挿入モード"),
        ],
    },
    CheatsheetSection {
        title: "移動",
        entries: &[
            ("h j k l", "左右/上下"),
            ("w / e / b", "単語先頭 / 末尾 / 前の単語"),
            ("0 / ^ / $", "行頭 / 最初の非空 / 行末"),
            ("gg / G", "先頭行 / 最終行"),
            ("f{char} / t{char}", "行内検索 (';' / ',' で反復)"),
        ],
    },
    CheatsheetSection {
        title: "オペレータ",
        entries: &[
            ("d{motion} / dd", "削除"),
            ("c{motion} / cc", "削除して挿入"),
            ("y{motion} / yy", "ヤンク"),
            ("ciw / diw / yiw", "単語オブジェクト"),
        ],
    },
    CheatsheetSection {
        title: "貼り付け・編集",
        entries: &[
            ("p / P", "後 / 前に貼り付け"),
            ("x / X", "1 文字削除 (前方/後方)"),
            ("r{char}", "1 文字置換"),
            (".", "直前の変更を反復"),
        ],
    },
    CheatsheetSection {
        title: "その他",
        entries: &[
            ("u / Ctrl+R", "Undo / Redo"),
            ("[count]{op/motion}", "回数指定 (例: 3dw, 2yy)"),
        ],
    },
];

pub(crate) struct VimCheatsheetView {
    complete: bool,
}

impl VimCheatsheetView {
    pub fn new() -> Self {
        Self { complete: false }
    }

    fn total_lines() -> u16 {
        let mut lines = 0u16;
        for section in SECTIONS {
            // title + entries + spacer line after each section
            lines = lines.saturating_add(1 + section.entries.len() as u16 + 1);
        }
        // Closing hint line
        lines.saturating_add(1)
    }
}

impl BottomPaneView for VimCheatsheetView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => {
                self.complete = true;
            }
            _ => {}
        }
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.complete = true;
        CancellationEvent::Handled
    }

    fn desired_height(&self, _width: u16) -> u16 {
        Self::total_lines()
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let mut y = area.y;
        for section in SECTIONS {
            if y >= area.y + area.height {
                break;
            }
            let title_line = Line::from(vec![Span::styled(section.title, TITLE_STYLE)]);
            Paragraph::new(title_line).render(
                Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: 1,
                },
                buf,
            );
            y = y.saturating_add(1);

            for (key, desc) in section.entries {
                if y >= area.y + area.height {
                    break;
                }
                let line = Line::from(vec![
                    Span::styled(*key, KEY_STYLE),
                    Span::raw("  "),
                    Span::raw(*desc),
                ]);
                Paragraph::new(line).render(
                    Rect {
                        x: area.x,
                        y,
                        width: area.width,
                        height: 1,
                    },
                    buf,
                );
                y = y.saturating_add(1);
            }

            if y < area.y + area.height {
                Paragraph::new(Line::from(vec![Span::raw(" ")])).render(
                    Rect {
                        x: area.x,
                        y,
                        width: area.width,
                        height: 1,
                    },
                    buf,
                );
                y = y.saturating_add(1);
            }
        }

        if y < area.y + area.height {
            let hint = Paragraph::new(Line::from(vec![Span::styled(
                "Esc / Enter / q で閉じる",
                HINT_STYLE,
            )]));
            hint.render(
                Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: 1,
                },
                buf,
            );
        }
    }
}
