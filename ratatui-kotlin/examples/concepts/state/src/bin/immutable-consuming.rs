//! # Consuming Widget Pattern with Immutable State
//!
//! This example demonstrates implementing the `Widget` trait directly on the widget type,
//! causing it to be consumed when rendered. This was the original pattern in Ratatui and
//! is still commonly used, especially for simple widgets that are created fresh each frame.
//!
//! This example runs with the Ratatui library code in the branch that you are currently
//! reading. See the [`latest`] branch for the code which works with the most recent Ratatui
//! release.
//!
//! [`latest`]: https://github.com/ratatui/ratatui/tree/latest
//!
//! ## When to Use This Pattern
//!
//! - You're working with existing code that uses this pattern
//! - Your widgets are simple and created fresh each frame
//! - You want maximum compatibility with older Ratatui code
//! - You don't need to reuse widget instances
//!
//! ## Trade-offs
//!
//! **Pros:**
//! - Simple - straightforward implementation
//! - Compatible - works with all Ratatui versions
//! - Familiar - widely used pattern in existing code
//! - No borrowing - no need to manage references
//!
//! **Cons:**
//! - Consuming - widget is destroyed after each render
//! - Inefficient - requires reconstruction for repeated use
//! - Limited reuse - cannot store and reuse widget instances
//!
//! ## Example Usage
//!
//! The widget implements `Widget` directly on the owned type, meaning it's consumed
//! when rendered and must be recreated for subsequent renders.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use ratatui_state_examples::is_exit_key_pressed;

/// Demonstrates the consuming widget pattern for immutable state rendering.
///
/// Creates a new counter widget instance each frame, showing how the consuming
/// pattern works with immutable state that's managed externally.
fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    ratatui::run(|terminal| {
        let mut count = 0;
        loop {
            terminal.draw(|frame| {
                // Widget is created fresh each time and consumed when rendered
                let counter = Counter::new(count);
                frame.render_widget(counter, frame.area());
            })?;
            // State updates happen outside of widget lifecycle
            count += 1;
            if is_exit_key_pressed()? {
                break Ok(());
            }
        }
    })
}

/// A simple counter widget that displays a count value.
///
/// Implements `Widget` directly on the owned type, meaning the widget is consumed
/// when rendered. The count state is managed externally and passed in during construction.
struct Counter {
    count: usize,
}

impl Counter {
    /// Create a new counter widget with the given count.
    fn new(count: usize) -> Self {
        Self { count }
    }
}

impl Widget for Counter {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Widget is consumed here - self is moved, not borrowed
        format!("Counter: {}", self.count).render(area, buf);
    }
}
