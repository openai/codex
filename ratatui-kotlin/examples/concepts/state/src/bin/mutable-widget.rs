//! # Mutable Widget Pattern
//!
//! This example demonstrates implementing the `Widget` trait on a mutable reference (`&mut T`)
//! to allow direct state mutation during rendering. This is one of the simplest approaches for
//! widgets that need to maintain their own state.
//!
//! This example runs with the Ratatui library code in the branch that you are currently
//! reading. See the [`latest`] branch for the code which works with the most recent Ratatui
//! release.
//!
//! [`latest`]: https://github.com/ratatui/ratatui/tree/latest
//!
//! ## When to Use This Pattern
//!
//! - You have self-contained widgets with their own state
//! - You prefer an object-oriented approach to widget design
//! - Your widget's state is simple and doesn't need complex sharing
//! - You want to encapsulate state within the widget itself
//!
//! ## Trade-offs
//!
//! **Pros:**
//! - Simple and intuitive - state is encapsulated within the widget
//! - Familiar pattern for developers coming from OOP backgrounds
//! - Direct state access without external state management
//! - Works well with Rust's ownership system for simple cases
//!
//! **Cons:**
//! - Can lead to borrowing challenges in complex scenarios
//! - Requires mutable access to the widget, which may not always be available
//! - Less flexible than `StatefulWidget` for shared or complex state patterns
//! - May require careful lifetime management in nested scenarios
//!
//! ## Example Usage
//!
//! The widget implements `Widget` for `&mut Self`, allowing it to mutate its internal state
//! during the render call. Each render increments a counter, demonstrating state mutation.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use ratatui_state_examples::is_exit_key_pressed;

/// Demonstrates the mutable widget pattern for mutable state management.
///
/// Creates a counter widget using `Widget` for `&mut Self` and runs the application loop,
/// updating the counter on each render cycle until the user exits.
fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    ratatui::run(|terminal| {
        let mut counter = Counter::default();
        loop {
            terminal.draw(|frame| frame.render_widget(&mut counter, frame.area()))?;
            if is_exit_key_pressed()? {
                break Ok(());
            }
        }
    })
}

/// A counter widget that maintains its own state and increments on each render.
///
/// Demonstrates the mutable widget pattern by implementing `Widget` for `&mut Self`.
#[derive(Default)]
struct Counter {
    counter: usize,
}

impl Widget for &mut Counter {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.counter += 1;
        format!("Counter: {counter}", counter = self.counter).render(area, buf);
    }
}
