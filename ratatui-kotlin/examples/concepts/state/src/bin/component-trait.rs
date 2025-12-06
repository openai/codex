//! # Custom Component Trait Pattern
//!
//! This example demonstrates using a custom trait instead of the standard `Widget` trait for
//! handling mutable state during rendering. This pattern is useful when you want to implement
//! consistent behavior across multiple widget types without implementing the `Widget` trait for
//! each one.
//!
//! This example runs with the Ratatui library code in the branch that you are currently
//! reading. See the [`latest`] branch for the code which works with the most recent Ratatui
//! release.
//!
//! [`latest`]: https://github.com/ratatui/ratatui/tree/latest
//!
//! ## When to Use This Pattern
//!
//! - You're building a widget framework or library with custom behavior
//! - You want a consistent API across multiple widget types
//! - You need more control over the render method signature
//! - You're prototyping widget behavior before standardizing on `Widget` or `StatefulWidget`
//!
//! ## Trade-offs
//!
//! **Pros:**
//! - Flexible - you can define custom method signatures
//! - Consistent - enforces the same behavior across widget types
//! - Simple - no need to understand `StatefulWidget` complexity
//!
//! **Cons:**
//! - Non-standard - users must learn your custom API instead of Ratatui's standard traits
//! - Less discoverable - doesn't integrate with Ratatui's widget ecosystem
//! - Limited reuse - can't be used with existing Ratatui functions expecting `Widget`
//!
//! ## Example Usage
//!
//! The custom `Component` trait allows widgets to mutate their state directly during rendering
//! by taking `&mut self` instead of `self`. This is similar to the mutable widget pattern but
//! with a custom trait interface.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui_state_examples::is_exit_key_pressed;

/// Demonstrates the custom component trait pattern for mutable state management.
///
/// Creates a counter widget using a custom `Component` trait and runs the application loop,
/// updating the counter on each render cycle until the user exits.
fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    ratatui::run(|terminal| {
        let mut counter = Counter::default();
        loop {
            terminal.draw(|frame| counter.render(frame, frame.area()))?;
            if is_exit_key_pressed()? {
                break Ok(());
            }
        }
    })
}

/// A custom trait for components that can render themselves while mutating their state.
///
/// This trait provides an alternative to the standard `Widget` trait by allowing components to
/// take `&mut self`, enabling direct state mutation during rendering.
trait Component {
    /// Render the component to the given area of the frame.
    fn render(&mut self, frame: &mut Frame, area: Rect);
}

/// A simple counter component that increments its value each time it's rendered.
///
/// Demonstrates how the custom `Component` trait allows widgets to maintain and mutate
/// their own state during the rendering process.
#[derive(Default)]
struct Counter {
    count: usize,
}

impl Component for Counter {
    fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.count += 1;
        frame.render_widget(format!("Counter: {count}", count = self.count), area);
    }
}
