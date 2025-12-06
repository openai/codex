//! # Render Function Pattern
//!
//! This example demonstrates the simplest approach to handling mutable state - using regular
//! functions that accept mutable state references. This pattern works well for simple applications
//! and prototypes where you don't need the complexity of widget traits.
//!
//! This example runs with the Ratatui library code in the branch that you are currently
//! reading. See the [`latest`] branch for the code which works with the most recent Ratatui
//! release.
//!
//! [`latest`]: https://github.com/ratatui/ratatui/tree/latest
//!
//! ## When to Use This Pattern
//!
//! - Simple applications with minimal state management needs
//! - Prototypes and quick experiments
//! - When you prefer functional programming over object-oriented approaches
//! - Applications where state is naturally managed at the top level
//!
//! ## Trade-offs
//!
//! **Pros:**
//! - Extremely simple - no traits to implement or understand
//! - Direct and explicit - state flow is obvious
//! - Flexible - easy to modify without interface constraints
//! - Beginner-friendly - uses basic Rust concepts
//!
//! **Cons:**
//! - State must be passed through function parameters
//! - Harder to organize as application complexity grows
//! - No encapsulation - state management is scattered
//! - Less reusable than widget-based approaches
//! - Can lead to parameter passing through many layers
//!
//! ## Example Usage
//!
//! State is managed at the application level and passed to render functions as needed.
//! This approach works well when state is simple and doesn't need complex organization.

use ratatui::Frame;
use ratatui_state_examples::is_exit_key_pressed;

/// Demonstrates the render function pattern for mutable state management.
///
/// Creates a counter using simple functions and runs the application loop,
/// updating the counter on each render cycle until the user exits.
fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    ratatui::run(|terminal| {
        let mut counter = 0;
        loop {
            terminal.draw(|frame| render(frame, &mut counter))?;
            if is_exit_key_pressed()? {
                break Ok(());
            }
        }
    })
}

/// Renders a counter using a simple function-based approach.
///
/// Demonstrates the functional approach to state management where state is managed externally
/// and passed in as a parameter.
fn render(frame: &mut Frame, counter: &mut usize) {
    *counter += 1;
    frame.render_widget(format!("Counter: {counter}"), frame.area());
}
