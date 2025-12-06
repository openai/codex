#![cfg(feature = "unstable-widget-ref")]

use std::any::{Any, type_name};
use std::cell::RefCell;

use pretty_assertions::assert_eq;
use ratatui::widgets::StatefulWidgetRef;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::text::Line;
use ratatui_core::widgets::Widget;

trait AnyWindow: StatefulWidgetRef<State = dyn Any> {
    fn title(&self) -> &str {
        type_name::<Self>()
    }
}

struct Window1;

struct Window1State {
    pub value: u32,
}

impl AnyWindow for Window1 {}

impl StatefulWidgetRef for Window1 {
    type State = dyn Any;

    fn render_ref(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let state = state.downcast_mut::<Window1State>().expect("window1 state");
        Line::from(format!("{}, u32: {}", self.title(), state.value)).render(area, buf);
    }
}

struct Window2;

struct Window2State {
    pub value: String,
}

impl AnyWindow for Window2 {}

impl StatefulWidgetRef for Window2 {
    type State = dyn Any;

    fn render_ref(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let state = state.downcast_mut::<Window2State>().expect("window2 state");
        Line::from(format!("{}, String: {}", self.title(), state.value)).render(area, buf);
    }
}

type BoxedWindow = Box<dyn AnyWindow>;
type BoxedState = Box<RefCell<dyn Any>>;

#[test]
fn render_dyn_widgets() {
    let windows: Vec<(BoxedWindow, BoxedState)> = vec![
        (
            Box::new(Window1),
            Box::new(RefCell::new(Window1State { value: 32 })),
        ),
        (
            Box::new(Window2),
            Box::new(RefCell::new(Window2State {
                value: "Some".to_string(),
            })),
        ),
        (
            Box::new(Window1),
            Box::new(RefCell::new(Window1State { value: 42 })),
        ),
    ];

    let mut buf = Buffer::empty(Rect::new(0, 0, 50, 3));

    let mut area = Rect::new(0, 0, 50, 1);
    for (w, s) in &windows {
        let mut s = s.borrow_mut();
        w.render_ref(area, &mut buf, &mut *s);
        area.y += 1;
    }

    assert_eq!(
        buf,
        Buffer::with_lines([
            "stateful_widget_ref_dyn::Window1, u32: 32         ",
            "stateful_widget_ref_dyn::Window2, String: Some    ",
            "stateful_widget_ref_dyn::Window1, u32: 42         ",
        ])
    );
}
