use ratatui_core::layout::Constraint;
use ratatui_macros::{constraints, span};

fn main() {
    constraints![,];

    // TODO: Make this compiler error pass
    let [a, b] = constraints![
      == 1/2,
      == 2,
    ];
    assert_eq!(a, Constraint::Ratio(1, 2));
    assert_eq!(b, Constraint::Length(2));

    let [a, b, c] = constraints![ == 1, == 10%, == 2; 4];

    let _ = span!(Modifier::BOLD, "hello world");

    let _ = span!("hello", "hello world");
}
