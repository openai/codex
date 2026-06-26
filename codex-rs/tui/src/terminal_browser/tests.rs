use codex_app_server_protocol::DynamicToolCallOutputContentItem;
use codex_app_server_protocol::DynamicToolCallResponse;
use codex_app_server_protocol::DynamicToolNamespaceTool;
use codex_app_server_protocol::DynamicToolSpec;
use codex_terminal_browser::BrowserCell;
use codex_terminal_browser::BrowserColor;
use codex_terminal_browser::BrowserScreen;
use codex_terminal_browser::BrowserStatus;
use codex_terminal_browser::BrowserToolOutput;
use codex_terminal_browser::BrowserView;
use codex_terminal_browser::TerminalSize;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use insta::assert_snapshot;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::style::Stylize;
use tokio::sync::mpsc::unbounded_channel;

use crate::app_event::AppEvent;
use crate::app_event::TerminalBrowserProfileCommand;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::ListSelectionView;
use crate::render::renderable::Renderable;

use super::browser_key_input;
use super::browser_mouse_input;
use super::overlay::overlay_area;
use super::overlay::render_screen_for_test;
use super::overlay::render_view_for_test;
use super::overlay::style_for_test;
use super::profile_approval::profile_approval_view_params;
use super::requested_profile_command;
use super::tools::TERMINAL_BROWSER_NAMESPACE;
use super::tools::dynamic_tool_response;
use super::tools::dynamic_tool_specs;

#[test]
fn floating_area_is_centered_and_narrow_terminals_use_the_full_area() {
    assert_eq!(
        overlay_area(Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 100, /*height*/ 40,
        )),
        Rect::new(
            /*x*/ 10, /*y*/ 4, /*width*/ 80, /*height*/ 32,
        )
    );
    assert_eq!(
        overlay_area(Rect::new(
            /*x*/ 3, /*y*/ 2, /*width*/ 60, /*height*/ 18,
        )),
        Rect::new(
            /*x*/ 3, /*y*/ 2, /*width*/ 60, /*height*/ 18,
        )
    );
}

#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "the assertion verifies exact Carbonyl RGB and indexed color preservation"
)]
fn browser_cell_style_maps_vt_attributes() {
    let cell = BrowserCell {
        text: "x".to_string(),
        foreground: BrowserColor::Rgb(1, 2, 3),
        background: BrowserColor::Indexed(4),
        bold: true,
        dim: false,
        italic: true,
        underlined: true,
        reversed: false,
        wide_continuation: false,
    };

    let style = style_for_test(&cell);
    assert_eq!(
        style,
        Style::default()
            .fg(Color::Rgb(1, 2, 3))
            .bg(Color::Indexed(4))
            .underline_color(Color::Reset)
            .bold()
            .italic()
            .underlined()
    );
}

#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "the assertion verifies the clipped browser cell keeps its exact terminal background"
)]
fn cropped_wide_glyph_is_replaced_with_a_blank_cell() {
    let mut wide = cell("\u{754c}");
    wide.background = BrowserColor::Indexed(4);
    let mut continuation = cell("");
    continuation.wide_continuation = true;
    let view = BrowserView {
        status: BrowserStatus::Running,
        title: None,
        url: None,
        visible: true,
        human_control: false,
        screen: BrowserScreen {
            rows: 1,
            cols: 2,
            cells: vec![wide, continuation],
            cursor: None,
        },
    };
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 1, /*height*/ 1,
    );
    let mut buffer = Buffer::empty(area);

    render_screen_for_test(&view, area, &mut buffer);

    assert_eq!(buffer[(0, 0)].symbol(), " ");
    assert_eq!(buffer[(0, 0)].bg, Color::Indexed(4));
}

#[test]
fn dynamic_tools_are_namespaced_and_deferred() {
    let specs = dynamic_tool_specs();
    let [DynamicToolSpec::Namespace(namespace)] = specs.as_slice() else {
        panic!("expected one terminal-browser namespace");
    };
    assert_eq!(namespace.name, TERMINAL_BROWSER_NAMESPACE);
    let tools = namespace
        .tools
        .iter()
        .map(|DynamicToolNamespaceTool::Function(tool)| tool)
        .collect::<Vec<_>>();
    assert_eq!(
        tools
            .iter()
            .map(|spec| spec.name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "open",
            "navigate",
            "wait",
            "profile",
            "snapshot",
            "click",
            "fill",
            "press",
            "scroll",
            "screenshot",
            "set_visibility",
            "close",
        ]
    );
    assert!(tools.iter().all(|spec| spec.defer_loading));
    for tool_name in ["click", "fill"] {
        let node_id_pattern = tools
            .iter()
            .find(|spec| spec.name == tool_name)
            .and_then(|spec| spec.input_schema.pointer("/properties/nodeId/pattern"))
            .and_then(serde_json::Value::as_str);
        assert_eq!(
            node_id_pattern,
            Some("^d[0-9a-f]{16}n[0-9]{1,20}$"),
            "{tool_name} must accept the document-scoped node IDs returned by snapshot"
        );
    }
    let profile_actions = tools
        .iter()
        .find(|spec| spec.name == "profile")
        .and_then(|spec| spec.input_schema.pointer("/properties/action/enum"))
        .and_then(serde_json::Value::as_array)
        .expect("profile action enum");
    assert!(profile_actions.contains(&serde_json::json!("requestEphemeral")));
}

#[test]
fn text_tool_output_maps_to_a_successful_dynamic_tool_response() {
    let response = dynamic_tool_response(Ok(BrowserToolOutput::Text("done".to_string())));
    assert_eq!(
        response,
        DynamicToolCallResponse {
            content_items: vec![DynamicToolCallOutputContentItem::InputText {
                text: "done".to_string(),
            }],
            success: true,
        }
    );
}

#[test]
fn tool_errors_are_structured_without_internal_details() {
    let response =
        dynamic_tool_response(Err(anyhow::anyhow!("CDP failed with password=do-not-leak")));
    let [DynamicToolCallOutputContentItem::InputText { text }] = response.content_items.as_slice()
    else {
        panic!("expected one text error");
    };
    let error: serde_json::Value = serde_json::from_str(text).expect("structured error JSON");

    assert!(!response.success);
    assert_eq!(error["error"]["code"], "internal");
    assert!(!text.contains("do-not-leak"));
}

#[test]
fn terminal_browser_overlay_snapshot() {
    let view = BrowserView {
        status: BrowserStatus::Running,
        title: Some("Example".to_string()),
        url: Some("https://example.com".to_string()),
        visible: true,
        human_control: false,
        screen: BrowserScreen {
            rows: 2,
            cols: 4,
            cells: vec![
                cell("C"),
                cell("o"),
                cell("d"),
                cell("e"),
                cell("x"),
                cell(" "),
                cell("U"),
                cell("I"),
            ],
            cursor: Some((1, 0)),
        },
    };
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 60, /*height*/ 12,
    );
    let mut buffer = Buffer::empty(area);
    render_view_for_test(&view, area, &mut buffer);

    assert_snapshot!(buffer_text(&buffer, area));
}

#[test]
fn terminal_browser_human_control_overlay_snapshot() {
    let view = BrowserView {
        status: BrowserStatus::Running,
        title: Some("Example".to_string()),
        url: Some("https://example.com".to_string()),
        visible: true,
        human_control: true,
        screen: BrowserScreen {
            rows: 1,
            cols: 4,
            cells: vec![cell("U"), cell("s"), cell("e"), cell("r")],
            cursor: None,
        },
    };
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 60, /*height*/ 12,
    );
    let mut buffer = Buffer::empty(area);
    render_view_for_test(&view, area, &mut buffer);

    assert_snapshot!(buffer_text(&buffer, area));
}

#[test]
fn human_control_maps_keyboard_and_mouse_input() {
    let key = browser_key_input(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL))
        .expect("mapped key");
    assert_eq!(key.key, "a");
    assert_eq!(key.code, "KeyA");
    assert_eq!(key.text, None);
    assert!(key.modifiers.control);

    let mouse = browser_mouse_input(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 12,
            row: 8,
            modifiers: KeyModifiers::SHIFT,
        },
        Rect::new(
            /*x*/ 10, /*y*/ 5, /*width*/ 40, /*height*/ 20,
        ),
    )
    .expect("mapped mouse");
    assert_eq!(mouse.column, 2);
    assert_eq!(mouse.row, 3);
    assert_eq!(mouse.viewport_cols, 40);
    assert_eq!(mouse.viewport_rows, 20);
    assert!(mouse.modifiers.shift);
}

#[test]
fn terminal_browser_unavailable_overlay_wraps_reason_snapshot() {
    let view = BrowserView {
        status: BrowserStatus::Unavailable {
            reason: "Carbonyl was not found on PATH; install it or set CODEX_CARBONYL_BINARY"
                .to_string(),
        },
        title: None,
        url: None,
        visible: true,
        human_control: false,
        screen: BrowserScreen::blank(TerminalSize { rows: 1, cols: 1 }),
    };
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 48, /*height*/ 10,
    );
    let mut buffer = Buffer::empty(area);
    render_view_for_test(&view, area, &mut buffer);

    assert_snapshot!(buffer_text(&buffer, area));
}

#[test]
fn terminal_browser_profile_forget_approval_snapshot() {
    let (tx, _rx) = unbounded_channel::<AppEvent>();
    let view = ListSelectionView::new(
        profile_approval_view_params(TerminalBrowserProfileCommand::Forget("work".to_string())),
        AppEventSender::new(tx),
        crate::keymap::RuntimeKeymap::defaults().list,
    );
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 64, /*height*/ 12,
    );
    let mut buffer = Buffer::empty(area);

    view.render(area, &mut buffer);

    assert_snapshot!(buffer_text(&buffer, area));
}

#[test]
fn valid_model_profile_mutations_route_to_explicit_approval() {
    assert_eq!(
        requested_profile_command(&serde_json::json!({
            "action": "requestForget",
            "name": "work",
        })),
        Some(TerminalBrowserProfileCommand::Forget("work".to_string()))
    );
    assert_eq!(
        requested_profile_command(&serde_json::json!({
            "action": "requestCreate",
            "name": "../unsafe",
        })),
        None
    );
}

fn cell(text: &str) -> BrowserCell {
    BrowserCell {
        text: text.to_string(),
        foreground: BrowserColor::Default,
        background: BrowserColor::Default,
        bold: false,
        dim: false,
        italic: false,
        underlined: false,
        reversed: false,
        wide_continuation: false,
    }
}

fn buffer_text(buffer: &Buffer, area: Rect) -> String {
    (area.y..area.bottom())
        .map(|y| {
            (area.x..area.right())
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
