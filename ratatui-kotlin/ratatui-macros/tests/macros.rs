use ratatui_core::layout::{Constraint, Rect};
use ratatui_macros::{constraints, horizontal, vertical};

#[test]
fn layout_constraints_macro() {
    let rect = Rect {
        x: 0,
        y: 0,
        width: 10,
        height: 10,
    };

    let [rect1, rect2] = vertical![==7, <=3].split(rect).to_vec().try_into().unwrap();
    assert_eq!(rect1, Rect::new(0, 0, 10, 7));
    assert_eq!(rect2, Rect::new(0, 7, 10, 3));

    let [rect1, rect2] = horizontal![==7, <=3]
        .split(rect)
        .to_vec()
        .try_into()
        .unwrap();
    assert_eq!(rect1, Rect::new(0, 0, 7, 10));
    assert_eq!(rect2, Rect::new(7, 0, 3, 10));

    let one = 1;
    let two = 2;
    let ten = 10;
    let zero = 0;
    let [a, b, c, d, e, f] = horizontal![==one, >=one, <=one, == 1 / two, == ten %, >=zero]
        .split(rect)
        .to_vec()
        .try_into()
        .unwrap();

    assert_eq!(a, Rect::new(0, 0, 1, 10));
    assert_eq!(b, Rect::new(1, 0, 1, 10));
    assert_eq!(c, Rect::new(2, 0, 1, 10));
    assert_eq!(d, Rect::new(3, 0, 5, 10));
    assert_eq!(e, Rect::new(8, 0, 1, 10));
    assert_eq!(f, Rect::new(9, 0, 1, 10));

    let one = 1;
    let two = 2;
    let ten = 10;
    let zero = 0;
    let [a, b, c, d, e, f] = horizontal![
      == one*one, // expr allowed here
      >= one+zero, // expr allowed here
      <= one-zero, // expr allowed here
      == 1/two, // only single token allowed in numerator and denominator
      == ten%, // only single token allowed before %
      >= zero // no trailing comma
    ]
    .split(rect)
    .to_vec()
    .try_into()
    .unwrap();

    assert_eq!(a, Rect::new(0, 0, 1, 10));
    assert_eq!(b, Rect::new(1, 0, 1, 10));
    assert_eq!(c, Rect::new(2, 0, 1, 10));
    assert_eq!(d, Rect::new(3, 0, 5, 10));
    assert_eq!(e, Rect::new(8, 0, 1, 10));
    assert_eq!(f, Rect::new(9, 0, 1, 10));

    let [a, b, c, d, e] = constraints![ >=0, ==1, <=5, ==10%, ==1/2 ];
    assert_eq!(a, Constraint::Min(0));
    assert_eq!(b, Constraint::Length(1));
    assert_eq!(c, Constraint::Max(5));
    assert_eq!(d, Constraint::Percentage(10));
    assert_eq!(e, Constraint::Ratio(1, 2));

    let [a, b, c, d, e] = constraints![ >=0; 5 ];
    assert_eq!(a, Constraint::Min(0));
    assert_eq!(b, Constraint::Min(0));
    assert_eq!(c, Constraint::Min(0));
    assert_eq!(d, Constraint::Min(0));
    assert_eq!(e, Constraint::Min(0));

    let [a, b, c, d, e] = constraints![ <=0; 5 ];
    assert_eq!(a, Constraint::Max(0));
    assert_eq!(b, Constraint::Max(0));
    assert_eq!(c, Constraint::Max(0));
    assert_eq!(d, Constraint::Max(0));
    assert_eq!(e, Constraint::Max(0));

    let [a, b] = constraints![ ==0; 2 ];
    assert_eq!(a, Constraint::Length(0));
    assert_eq!(b, Constraint::Length(0));

    let [a, b] = constraints![ == 50%; 2 ];
    assert_eq!(a, Constraint::Percentage(50));
    assert_eq!(b, Constraint::Percentage(50));

    let [a, b] = constraints![ == 1/2; 2 ];
    assert_eq!(a, Constraint::Ratio(1, 2));
    assert_eq!(b, Constraint::Ratio(1, 2));
}

#[test]
fn fails() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/fails.rs");
}
