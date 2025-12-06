//! # Lifetime-Based Mutable References Pattern
//!
//! This example demonstrates storing mutable references directly in widget structs using explicit
//! lifetimes. This is an advanced pattern that provides zero-cost state access but requires
//! careful lifetime management.
//!
//! This example runs with the Ratatui library code in the branch that you are currently
//! reading. See the [`latest`] branch for the code which works with the most recent Ratatui
//! release.
//!
//! [`latest`]: https://github.com/ratatui/ratatui/tree/latest
//!
//! ## When to Use This Pattern
//!
//! - You need maximum performance with zero runtime overhead
//! - You have a good understanding of Rust lifetimes and borrowing
//! - State lifetime is clearly defined and relatively simple
//! - You're building performance-critical applications
//!
//! ## Trade-offs
//!
//! **Pros:**
//! - Zero runtime cost - no reference counting or runtime borrow checking
//! - Compile-time safety - borrow checker ensures memory safety
//! - Direct access to state without indirection
//! - Maximum performance for state access
//!
//! **Cons:**
//! - Complex lifetime management - requires deep Rust knowledge
//! - Easy to create compilation errors that are hard to understand
//! - Inflexible - lifetime constraints can make code harder to refactor
//! - Not suitable for beginners - requires advanced Rust skills
//! - Widget structs become less reusable due to lifetime constraints
//!
//! ## Important Considerations
//!
//! - The widget's lifetime is tied to the state's lifetime
//! - You must ensure the state outlives the widget
//! - Lifetime annotations can become complex in larger applications
//! - Consider simpler patterns unless performance is critical
//!
//! ## Example Usage
//!
//! The widget stores a mutable reference to external state, allowing direct access without
//! runtime overhead. The widget must be recreated for each render call due to the lifetime
//! constraints.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use ratatui_state_examples::is_exit_key_pressed;

/// Demonstrates the lifetime-based mutable references pattern for mutable state management.
///
/// Creates a counter widget using mutable references with explicit lifetimes and runs the
/// application loop, updating the counter on each render cycle until the user exits.
fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    ratatui::run(|terminal| {
        let mut count = 0;
        loop {
            let counter = CounterWidget { count: &mut count };
            terminal.draw(|frame| frame.render_widget(counter, frame.area()))?;
            if is_exit_key_pressed()? {
                break Ok(());
            }
        }
    })
}

/// A counter widget that holds a mutable reference to external state.
///
/// Demonstrates the lifetime-based pattern where the widget directly stores a
/// mutable reference to external state.
struct CounterWidget<'a> {
    count: &'a mut usize,
}

impl Widget for CounterWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        *self.count += 1;
        format!("Counter: {count}", count = self.count).render(area, buf);
    }
}
