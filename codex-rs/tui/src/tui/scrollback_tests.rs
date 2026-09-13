use super::ScrollbackStrategy;
use crate::custom_terminal::Terminal;
use crate::insert_history::HistoryLineWrapPolicy;
use crate::insert_history::InsertHistoryMode;
use crate::insert_history::insert_history_lines_with_mode_and_wrap_policy;
use crate::test_backend::VT100Backend;
use codex_terminal_detection::Multiplexer;
use codex_terminal_detection::TerminalInfo;
use codex_terminal_detection::TerminalName;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::Print;
use pretty_assertions::assert_eq;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::layout::Size;
use ratatui::text::Line;

#[test]
fn windows_terminal_uses_full_screen_unless_zellij_is_active() {
    let mut terminal = TerminalInfo {
        name: TerminalName::WindowsTerminal,
        term_program: None,
        version: None,
        term: None,
        multiplexer: None,
    };
    let mut strategy = ScrollbackStrategy::detect(&terminal);

    assert_eq!(
        [
            strategy.history_insertion_mode(HistoryLineWrapPolicy::PreWrap),
            strategy.history_insertion_mode(HistoryLineWrapPolicy::Terminal),
        ],
        [InsertHistoryMode::FullScreen, InsertHistoryMode::FullScreen]
    );

    terminal.multiplexer = Some(Multiplexer::Zellij { version: None });
    strategy = ScrollbackStrategy::detect(&terminal);
    assert_eq!(strategy, ScrollbackStrategy::Zellij);
}

#[test]
fn zellij_only_uses_full_screen_insertion_for_terminal_wrapped_history() {
    assert_eq!(
        [
            ScrollbackStrategy::Zellij.history_insertion_mode(HistoryLineWrapPolicy::PreWrap),
            ScrollbackStrategy::Zellij.history_insertion_mode(HistoryLineWrapPolicy::Terminal),
        ],
        [InsertHistoryMode::Standard, InsertHistoryMode::FullScreen]
    );
}

#[test]
fn full_screen_history_insertion_preserves_terminal_scrollback() {
    let width = 24;
    let height = 6;
    let backend = VT100Backend::with_scrollback(width, height, /*scrollback_len*/ 32);
    let mut terminal = Terminal::with_options(backend).expect("terminal with scrollback");
    terminal.set_viewport_area(Rect::new(
        /*x*/ 0,
        /*y*/ height - 2,
        width,
        /*height*/ 2,
    ));

    for (row, line) in [
        "oldest-history-row",
        "history-row-2",
        "history-row-3",
        "history-row-4",
        "stale-composer-1",
        "stale-composer-2",
    ]
    .into_iter()
    .enumerate()
    {
        queue!(
            terminal.backend_mut(),
            MoveTo(/*x*/ 0, row as u16),
            Print(line)
        )
        .expect("seed terminal row");
    }

    insert_history_lines_with_mode_and_wrap_policy(
        &mut terminal,
        vec![Line::from("new-history-row")],
        ScrollbackStrategy::FullScreen.history_insertion_mode(HistoryLineWrapPolicy::PreWrap),
        HistoryLineWrapPolicy::PreWrap,
    )
    .expect("insert history through the full screen");

    let visible = terminal.backend().vt100().screen().contents();
    let mut scrollback_screen = terminal.backend().vt100().screen().clone();
    scrollback_screen.set_scrollback(/*rows*/ usize::MAX);
    let scrollback = scrollback_screen.contents();

    insta::assert_snapshot!(format!("SCROLLBACK:\n{scrollback}\nVISIBLE:\n{visible}"), @r"
    SCROLLBACK:
    oldest-history-row
    history-row-2
    history-row-3
    history-row-4
    new-history-row
    VISIBLE:
    history-row-2
    history-row-3
    history-row-4
    new-history-row
    ");
}

#[test]
fn full_screen_viewport_growth_preserves_terminal_scrollback() {
    let width = 24;
    let height = 6;
    let backend = VT100Backend::with_scrollback(width, height, /*scrollback_len*/ 32);
    let mut terminal = Terminal::with_options(backend).expect("terminal with scrollback");
    terminal.set_viewport_area(Rect::new(
        /*x*/ 0,
        /*y*/ height - 2,
        width,
        /*height*/ 2,
    ));

    for (row, line) in [
        "oldest-history-row",
        "history-row-2",
        "history-row-3",
        "history-row-4",
        "stale-composer-1",
        "stale-composer-2",
    ]
    .into_iter()
    .enumerate()
    {
        queue!(
            terminal.backend_mut(),
            MoveTo(/*x*/ 0, row as u16),
            Print(line)
        )
        .expect("seed terminal row");
    }

    ScrollbackStrategy::FullScreen
        .grow_viewport(
            &mut terminal,
            /*viewport_top*/ height - 2,
            Size::new(width, height),
            /*scroll_by*/ 2,
        )
        .expect("grow viewport through full-screen scrolling");

    let visible = terminal.backend().vt100().screen().contents();
    let mut scrollback_screen = terminal.backend().vt100().screen().clone();
    scrollback_screen.set_scrollback(/*rows*/ usize::MAX);
    let scrollback = scrollback_screen.contents();

    insta::assert_snapshot!(format!("SCROLLBACK:\n{scrollback}\nVISIBLE:\n{visible}"), @r"
    SCROLLBACK:
    oldest-history-row
    history-row-2
    history-row-3
    history-row-4
    VISIBLE:
    history-row-3
    history-row-4
    ");
}

#[test]
fn standard_viewport_growth_preserves_composer_and_cursor() {
    for scroll_by in [1, 3, 4] {
        let width = 24;
        let height = 6;
        let backend = VT100Backend::with_scrollback(width, height, /*scrollback_len*/ 32);
        let mut terminal = Terminal::with_options(backend).expect("terminal with scrollback");
        let viewport_top = 4;
        terminal.set_viewport_area(Rect::new(
            /*x*/ 0,
            viewport_top,
            width,
            /*height*/ 2,
        ));
        for (row, text) in [
            "history-1",
            "history-2",
            "history-3",
            "history-4",
            "composer-1",
            "composer-2",
        ]
        .into_iter()
        .enumerate()
        {
            queue!(
                terminal.backend_mut(),
                MoveTo(/*x*/ 0, row as u16),
                Print(text)
            )
            .expect("seed terminal row");
        }
        let cursor = Position::new(/*x*/ 7, /*y*/ 5);
        terminal
            .set_cursor_position(cursor)
            .expect("position cursor");

        ScrollbackStrategy::Standard
            .grow_viewport(
                &mut terminal,
                viewport_top,
                Size::new(width, height),
                scroll_by,
            )
            .expect("grow standard viewport");

        let screen = terminal.backend().vt100().screen();
        assert_eq!(
            (screen.cursor_position(), terminal.last_known_cursor_pos),
            ((cursor.y, cursor.x), cursor)
        );
        // vt100 intentionally omits scrollback for partial regions, even for newlines. Snapshot
        // visible geometry here; the wire test below and real-terminal smoke cover scrollback.
        insta::assert_snapshot!(
            format!("standard_viewport_growth_{scroll_by}"),
            screen.contents()
        );

        // A newline at the physical bottom must scroll the full screen after the region reset.
        queue!(
            terminal.backend_mut(),
            MoveTo(/*x*/ 0, height - 1),
            Print("\r\nnext-row")
        )
        .expect("write after restoring the scroll region");
        assert_eq!(
            terminal
                .backend()
                .vt100()
                .screen()
                .rows(/*start*/ 0, width)
                .collect::<Vec<_>>()[4..],
            ["composer-2", "next-row"]
        );
    }
}

#[test]
fn standard_viewport_growth_uses_scrollback_preserving_newlines() {
    for scroll_by in [1, 3, 4] {
        let screen_size = Size::new(/*width*/ 24, /*height*/ 6);
        let cursor = Position::new(/*x*/ 7, /*y*/ 5);
        let backend = CrosstermBackend::new(Vec::<u8>::new());
        let mut terminal =
            Terminal::with_screen_size_and_cursor_position_for_test(backend, screen_size, cursor);
        ScrollbackStrategy::Standard
            .grow_viewport(
                &mut terminal,
                /*viewport_top*/ 4,
                screen_size,
                scroll_by,
            )
            .expect("grow standard viewport");
        assert_eq!(
            terminal.backend().writer(),
            format!(
                "\x1b[1;4r\x1b[4;1H{}\x1b[r\x1b[6;8H",
                "\r\n".repeat(usize::from(scroll_by))
            )
            .as_bytes()
        );
    }
}

#[test]
fn standard_viewport_growth_with_one_history_row_preserves_scrollback() {
    let width = 24;
    let height = 3;
    let backend = VT100Backend::with_scrollback(width, height, /*scrollback_len*/ 32);
    let mut terminal = Terminal::with_options(backend).expect("terminal with scrollback");
    queue!(
        terminal.backend_mut(),
        Print("history\r\nstale-composer\r\nstale-footer")
    )
    .expect("seed terminal");
    ScrollbackStrategy::Standard
        .grow_viewport(
            &mut terminal,
            /*viewport_top*/ 1,
            Size::new(width, height),
            /*scroll_by*/ 1,
        )
        .expect("grow past the single history row");
    let screen = terminal.backend().vt100().screen();
    let mut history = screen.clone();
    history.set_scrollback(/*rows*/ usize::MAX);
    insta::assert_snapshot!(format!("SCROLLBACK:\n{}\nVISIBLE:\n{}", history.contents(), screen.contents()), @r"
    SCROLLBACK:
    history
    VISIBLE:
    ");
}

#[test]
fn standard_viewport_growth_without_rows_is_cursor_neutral() {
    for (viewport_top, scroll_by) in [(4, 0), (0, 1)] {
        let backend = VT100Backend::new(/*width*/ 24, /*height*/ 6);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        let cursor = Position::new(/*x*/ 7, /*y*/ 5);
        terminal
            .set_cursor_position(cursor)
            .expect("position cursor");
        let before = terminal.backend().vt100().screen().state_formatted();
        ScrollbackStrategy::Standard
            .grow_viewport(
                &mut terminal,
                viewport_top,
                Size::new(/*width*/ 24, /*height*/ 6),
                scroll_by,
            )
            .expect("no-op viewport growth");
        assert_eq!(
            terminal.backend().vt100().screen().state_formatted(),
            before
        );
    }
}
