#![allow(clippy::reversed_empty_ranges)] // This is intentional.

use futures::io::Window;

#[test]
fn set() {
    let mut buffer = Window::new(&[1, 2, 3]);
    buffer.set(..3);
    assert_eq!(buffer.as_ref(), &[1, 2, 3]);
    buffer.set(3..3);
    assert_eq!(buffer.as_ref(), &[]);
    buffer.set(3..=2); // == 3..3
    assert_eq!(buffer.as_ref(), &[]);
    buffer.set(0..2);
    assert_eq!(buffer.as_ref(), &[1, 2]);
}

#[test]
#[should_panic]
fn set_panic_out_of_bounds() {
    let mut buffer = Window::new(&[1, 2, 3]);
    buffer.set(2..4);
}

#[test]
#[should_panic]
fn set_panic_start_is_greater_than_end() {
    let mut buffer = Window::new(&[1, 2, 3]);
    buffer.set(3..2);
}
