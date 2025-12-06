//! # Function-Based Pattern with Immutable State
//!
//! This example demonstrates using standalone functions for rendering widgets with immutable
//! state. This pattern keeps state management completely separate from widget rendering logic,
//! making it easy to test and reason about.
//!
//! This example runs with the Ratatui library code in the branch that you are currently
//! reading. See the [`latest`] branch for the code which works with the most recent Ratatui
//! release.
//!
//! [`latest`]: https://github.com/ratatui/ratatui/tree/latest
//!
//! ## When to Use This Pattern
//!
//! - You prefer functional programming approaches
//! - Your rendering logic is simple and doesn't need complex widget hierarchies
//! - You want clear separation between state and rendering
//! - You're building simple UIs or prototyping quickly
//!
//! ## Trade-offs
//!
//! **Pros:**
//! - Simple - easy to understand and test
//! - Pure functions - no side effects in rendering
//! - Flexible - can easily compose multiple render functions
//! - Clear separation - state management is completely separate from rendering
//!
//! **Cons:**
//! - Limited - doesn't integrate with Ratatui's widget ecosystem
//! - Verbose - requires passing state explicitly to every function
//! - No reuse - can't be used with existing Ratatui widget infrastructure
//!
//! ## Example Usage
//!
//! The function takes immutable references to both the frame and state, ensuring that
//! rendering is a pure operation that doesn't modify state.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui_state_examples::is_exit_key_pressed;

/// Demonstrates the function-based pattern for immutable state rendering.
///
/// Creates a counter state and renders it using a pure function, incrementing the counter
/// in the application loop rather than during rendering.
fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    ratatui::run(|terminal| {
        let mut counter_state = Counter::default();
        loop {
            terminal.draw(|frame| render_counter(frame, frame.area(), &counter_state))?;
            // State updates happen outside of rendering
            counter_state.increment();
            if is_exit_key_pressed()? {
                break Ok(());
            }
        }
    })
}

/// State for the counter.
///
/// This state is managed externally and passed to render functions as an immutable reference.
#[derive(Default)]
struct Counter {
    count: usize,
}

impl Counter {
    /// Increment the counter value.
    fn increment(&mut self) {
        self.count += 1;
    }
}

/// Pure render function that displays the counter state.
///
/// Takes immutable references to ensure rendering has no side effects on state.
/// This function can be easily tested and composed with other render functions.
fn render_counter(frame: &mut Frame, area: Rect, state: &Counter) {
    frame.render_widget(format!("Counter: {}", state.count), area);
}
