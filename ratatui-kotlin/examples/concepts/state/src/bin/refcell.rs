//! # Interior Mutability Pattern (RefCell)
//!
//! This example demonstrates using `Rc<RefCell<T>>` for interior mutability, allowing multiple
//! widgets to share and mutate the same state. This pattern is useful when you need shared
//! mutable state but can't use mutable references due to borrowing constraints.
//!
//! This example runs with the Ratatui library code in the branch that you are currently
//! reading. See the [`latest`] branch for the code which works with the most recent Ratatui
//! release.
//!
//! [`latest`]: https://github.com/ratatui/ratatui/tree/latest
//!
//! ## When to Use This Pattern
//!
//! - Multiple widgets need to access and modify the same state
//! - You can't use mutable references due to borrowing constraints
//! - You need shared ownership of mutable data
//! - Complex widget hierarchies where state needs to be accessed from multiple locations
//!
//! ## Trade-offs
//!
//! **Pros:**
//! - Allows shared mutable access to state
//! - Works with immutable widget references
//! - Enables complex state sharing patterns
//! - Can be cloned cheaply (reference counting)
//!
//! **Cons:**
//! - Runtime borrow checking - potential for panics if you violate borrowing rules
//! - Less efficient than compile-time borrow checking
//! - Harder to debug when borrow violations occur
//! - More complex than simpler state management patterns
//! - Can lead to subtle bugs if not used carefully
//!
//! ## Important Safety Notes
//!
//! - Only one mutable borrow can exist at a time
//! - Violating this rule will cause a panic at runtime
//! - Always minimize the scope of borrows to avoid conflicts
//!
//! ## Example Usage
//!
//! The widget wraps its state in `Rc<RefCell<T>>`, allowing the state to be shared and mutated
//! even when the widget itself is used by value (as required by the `Widget` trait).

use std::cell::RefCell;
use std::ops::AddAssign;
use std::rc::Rc;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use ratatui_state_examples::is_exit_key_pressed;

/// Demonstrates the interior mutability pattern for mutable state management.
///
/// Creates a counter widget using `Rc<RefCell<T>>` and runs the application loop,
/// updating the counter on each render cycle until the user exits.
fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    ratatui::run(|terminal| {
        let counter = Counter::default();
        loop {
            terminal.draw(|frame| frame.render_widget(counter.clone(), frame.area()))?;
            if is_exit_key_pressed()? {
                break Ok(());
            }
        }
    })
}

/// A counter widget that uses interior mutability for shared state management.
///
/// Demonstrates how `Rc<RefCell<T>>` enables mutable state access even when the
/// widget itself is used by value.
#[derive(Default, Clone)]
struct Counter {
    count: Rc<RefCell<usize>>,
}

impl Widget for Counter {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.count.borrow_mut().add_assign(1);
        format!("Counter: {count}", count = self.count.borrow()).render(area, buf);
    }
}
