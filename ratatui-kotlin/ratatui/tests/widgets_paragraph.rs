use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Alignment;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Padding, Paragraph, Wrap};

/// Tests the [`Paragraph`] widget against the expected [`Buffer`] by rendering it onto an equal
/// area and comparing the rendered and expected content.
#[track_caller]
fn test_case(paragraph: Paragraph, expected: &Buffer) {
    let backend = TestBackend::new(expected.area.width, expected.area.height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| f.render_widget(paragraph, f.area()))
        .unwrap();
    terminal.backend().assert_buffer(expected);
}

#[test]
fn widgets_paragraph_renders_double_width_graphemes() {
    let s = "コンピュータ上で文字を扱う場合、典型的には文字による通信を行う場合にその両端点では、";

    let text = vec![Line::from(s)];
    let paragraph = Paragraph::new(text)
        .block(Block::bordered())
        .wrap(Wrap { trim: true });

    test_case(
        paragraph,
        &Buffer::with_lines([
            "┌────────┐",
            "│コンピュ│",
            "│ータ上で│",
            "│文字を扱│",
            "│う場合、│",
            "│典型的に│",
            "│は文字に│",
            "│よる通信│",
            "│を行う場│",
            "└────────┘",
        ]),
    );
}

#[test]
fn widgets_paragraph_renders_mixed_width_graphemes() {
    let backend = TestBackend::new(10, 7);
    let mut terminal = Terminal::new(backend).unwrap();

    let s = "aコンピュータ上で文字を扱う場合、";
    terminal
        .draw(|f| {
            let text = vec![Line::from(s)];
            let paragraph = Paragraph::new(text)
                .block(Block::bordered())
                .wrap(Wrap { trim: true });
            f.render_widget(paragraph, f.area());
        })
        .unwrap();
    terminal.backend().assert_buffer_lines([
        // The internal width is 8 so only 4 slots for double-width characters.
        "┌────────┐",
        "│aコンピ │", // Here we have 1 latin character so only 3 double-width ones can fit.
        "│ュータ上│",
        "│で文字を│",
        "│扱う場合│",
        "│、      │",
        "└────────┘",
    ]);
}

#[test]
fn widgets_paragraph_can_wrap_with_a_trailing_nbsp() {
    let nbsp = "\u{00a0}";
    let line = Line::from(vec![Span::raw("NBSP"), Span::raw(nbsp)]);
    let paragraph = Paragraph::new(line).block(Block::bordered());

    test_case(
        paragraph,
        &Buffer::with_lines([
            "┌──────────────────┐",
            "│NBSP\u{00a0}             │",
            "└──────────────────┘",
        ]),
    );
}

#[test]
fn widgets_paragraph_can_scroll_horizontally() {
    let text =
        Text::from("段落现在可以水平滚动了！\nParagraph can scroll horizontally!\nLittle line");
    let paragraph = Paragraph::new(text).block(Block::bordered());

    test_case(
        paragraph.clone().alignment(Alignment::Left).scroll((0, 7)),
        &Buffer::with_lines([
            "┌──────────────────┐",
            "│在可以水平滚动了！│",
            "│ph can scroll hori│",
            "│line              │",
            "│                  │",
            "│                  │",
            "│                  │",
            "│                  │",
            "│                  │",
            "└──────────────────┘",
        ]),
    );
    // only support Alignment::Left
    test_case(
        paragraph.clone().alignment(Alignment::Right).scroll((0, 7)),
        &Buffer::with_lines([
            "┌──────────────────┐",
            "│段落现在可以水平滚│",
            "│Paragraph can scro│",
            "│       Little line│",
            "│                  │",
            "│                  │",
            "│                  │",
            "│                  │",
            "│                  │",
            "└──────────────────┘",
        ]),
    );
}

const SAMPLE_STRING: &str = "The library is based on the principle of immediate rendering with \
     intermediate buffers. This means that at each new frame you should build all widgets that are \
     supposed to be part of the UI. While providing a great flexibility for rich and \
     interactive UI, this may introduce overhead for highly dynamic content.";

#[test]
fn widgets_paragraph_can_wrap_its_content() {
    let text = vec![Line::from(SAMPLE_STRING)];
    let paragraph = Paragraph::new(text)
        .block(Block::bordered())
        .wrap(Wrap { trim: true });

    test_case(
        paragraph.clone().alignment(Alignment::Left),
        &Buffer::with_lines([
            "┌──────────────────┐",
            "│The library is    │",
            "│based on the      │",
            "│principle of      │",
            "│immediate         │",
            "│rendering with    │",
            "│intermediate      │",
            "│buffers. This     │",
            "│means that at each│",
            "└──────────────────┘",
        ]),
    );
    test_case(
        paragraph.clone().alignment(Alignment::Center),
        &Buffer::with_lines([
            "┌──────────────────┐",
            "│  The library is  │",
            "│   based on the   │",
            "│   principle of   │",
            "│     immediate    │",
            "│  rendering with  │",
            "│   intermediate   │",
            "│   buffers. This  │",
            "│means that at each│",
            "└──────────────────┘",
        ]),
    );
    test_case(
        paragraph.clone().alignment(Alignment::Right),
        &Buffer::with_lines([
            "┌──────────────────┐",
            "│    The library is│",
            "│      based on the│",
            "│      principle of│",
            "│         immediate│",
            "│    rendering with│",
            "│      intermediate│",
            "│     buffers. This│",
            "│means that at each│",
            "└──────────────────┘",
        ]),
    );
}

#[test]
fn widgets_paragraph_works_with_padding() {
    let block = Block::bordered().padding(Padding {
        left: 2,
        right: 2,
        top: 1,
        bottom: 1,
    });
    let paragraph = Paragraph::new(vec![Line::from(SAMPLE_STRING)])
        .block(block.clone())
        .wrap(Wrap { trim: true });

    test_case(
        paragraph.clone().alignment(Alignment::Left),
        &Buffer::with_lines([
            "┌────────────────────┐",
            "│                    │",
            "│  The library is    │",
            "│  based on the      │",
            "│  principle of      │",
            "│  immediate         │",
            "│  rendering with    │",
            "│  intermediate      │",
            "│  buffers. This     │",
            "│  means that at     │",
            "│                    │",
            "└────────────────────┘",
        ]),
    );
    test_case(
        paragraph.clone().alignment(Alignment::Right),
        &Buffer::with_lines([
            "┌────────────────────┐",
            "│                    │",
            "│    The library is  │",
            "│      based on the  │",
            "│      principle of  │",
            "│         immediate  │",
            "│    rendering with  │",
            "│      intermediate  │",
            "│     buffers. This  │",
            "│     means that at  │",
            "│                    │",
            "└────────────────────┘",
        ]),
    );

    let paragraph = Paragraph::new(vec![
        Line::from("This is always centered.").alignment(Alignment::Center),
        Line::from(SAMPLE_STRING),
    ])
    .block(block)
    .wrap(Wrap { trim: true });

    test_case(
        paragraph.alignment(Alignment::Right),
        &Buffer::with_lines([
            "┌────────────────────┐",
            "│                    │",
            "│   This is always   │",
            "│      centered.     │",
            "│    The library is  │",
            "│      based on the  │",
            "│      principle of  │",
            "│         immediate  │",
            "│    rendering with  │",
            "│      intermediate  │",
            "│     buffers. This  │",
            "│     means that at  │",
            "│                    │",
            "└────────────────────┘",
        ]),
    );
}

#[test]
fn widgets_paragraph_can_align_spans() {
    let right_s = "This string will override the paragraph alignment to be right aligned.";
    let default_s = "This string will be aligned based on the alignment of the paragraph.";

    let text = vec![
        Line::from(right_s).alignment(Alignment::Right),
        Line::from(default_s),
    ];
    let paragraph = Paragraph::new(text)
        .block(Block::bordered())
        .wrap(Wrap { trim: true });

    test_case(
        paragraph.clone().alignment(Alignment::Left),
        &Buffer::with_lines([
            "┌──────────────────┐",
            "│  This string will│",
            "│      override the│",
            "│         paragraph│",
            "│   alignment to be│",
            "│    right aligned.│",
            "│This string will  │",
            "│be aligned based  │",
            "│on the alignment  │",
            "└──────────────────┘",
        ]),
    );
    test_case(
        paragraph.alignment(Alignment::Center),
        &Buffer::with_lines([
            "┌──────────────────┐",
            "│  This string will│",
            "│      override the│",
            "│         paragraph│",
            "│   alignment to be│",
            "│    right aligned.│",
            "│ This string will │",
            "│ be aligned based │",
            "│ on the alignment │",
            "└──────────────────┘",
        ]),
    );

    let left_lines = vec!["This string", "will override the paragraph alignment"]
        .into_iter()
        .map(|s| Line::from(s).alignment(Alignment::Left))
        .collect::<Vec<_>>();
    let mut lines = vec![
        "This",
        "must be pretty long",
        "in order to effectively show",
        "truncation.",
    ]
    .into_iter()
    .map(Line::from)
    .collect::<Vec<_>>();

    let mut text = left_lines.clone();
    text.append(&mut lines);
    let paragraph = Paragraph::new(text).block(Block::bordered());

    test_case(
        paragraph.clone().alignment(Alignment::Right),
        &Buffer::with_lines([
            "┌──────────────────┐",
            "│This string       │",
            "│will override the │",
            "│              This│",
            "│must be pretty lon│",
            "│in order to effect│",
            "│       truncation.│",
            "│                  │",
            "│                  │",
            "└──────────────────┘",
        ]),
    );
    test_case(
        paragraph.alignment(Alignment::Left),
        &Buffer::with_lines([
            "┌──────────────────┐",
            "│This string       │",
            "│will override the │",
            "│This              │",
            "│must be pretty lon│",
            "│in order to effect│",
            "│truncation.       │",
            "│                  │",
            "└──────────────────┘",
        ]),
    );
}
