# Ratatui State Management Examples

This collection demonstrates various patterns for handling both mutable and immutable state in
Ratatui applications. Each example solves a counter problem - incrementing a counter value - but
uses different architectural approaches. These patterns represent common solutions to state
management challenges you'll encounter when building TUI applications.

For more information about widgets in Ratatui, see the [widgets module documentation](https://docs.rs/ratatui/latest/ratatui/widgets/index.html).

## When to Use Each Pattern

Choose the pattern that best fits your application's architecture and complexity:

- **Simple applications**: Use `render-function` or `mutable-widget` patterns
- **Clean separation**: Consider `stateful-widget` or `component-trait` patterns
- **Complex hierarchies**: Use `nested-*` patterns for parent-child relationships
- **Shared state**: Use `refcell` when multiple widgets need access to the same state
- **Advanced scenarios**: Use `widget-with-mutable-ref` when you understand Rust lifetimes well

## Running the Examples

To run any example, use:

```bash
cargo run --bin example-name
```

Press any key (or resize the terminal) to increment the counter. Press `<Esc>` or `q` to exit.

## Examples

### Immutable State Patterns

These patterns keep widget state immutable during rendering, with state updates happening outside
the render cycle. They're generally easier to reason about and less prone to borrowing issues.

#### [`immutable-function.rs`] - Function-Based Immutable State

**Best for**: Simple applications with pure rendering functions
**Pros**: Pure functions, easy to test, clear separation of concerns
**Cons**: Verbose parameter passing, limited integration with Ratatui ecosystem

Uses standalone functions that take immutable references to state. State updates happen in the
application loop outside of rendering.

#### [`immutable-shared-ref.rs`] - Shared Reference Pattern (Recommended)

**Best for**: Most modern Ratatui applications
**Pros**: Reusable widgets, efficient, integrates with Ratatui ecosystem, modern best practice
**Cons**: Requires external state management for dynamic behavior

Implements [`Widget`](https://docs.rs/ratatui/latest/ratatui/widgets/trait.Widget.html) for
`&T`, allowing widgets to be rendered multiple times by reference without being consumed.

#### [`immutable-consuming.rs`] - Consuming Widget Pattern

**Best for**: Compatibility with older code, simple widgets created fresh each frame
**Pros**: Simple implementation, widely compatible, familiar pattern
**Cons**: Widget consumed on each render, requires reconstruction for reuse

Implements [`Widget`](https://docs.rs/ratatui/latest/ratatui/widgets/trait.Widget.html) directly
on the owned type, consuming the widget when rendered.

### Mutable State Patterns

These patterns allow widgets to modify their state during rendering, useful for widgets that need
to update state as part of their rendering behavior.

#### [`mutable-function.rs`] - Function-Based Mutable State

**Best for**: Simple applications with minimal mutable state
**Pros**: Easy to understand, no traits to implement, direct control
**Cons**: State gets passed around as function parameters, harder to organize as complexity grows

Uses simple functions that accept mutable state references. State is managed at the application
level and passed down to render functions.

#### [`mutable-widget.rs`] - Mutable Widget Pattern

**Best for**: Self-contained widgets with their own mutable state
**Pros**: Encapsulates state within the widget, familiar OOP-style approach
**Cons**: Requires `&mut` references, can be challenging with complex borrowing scenarios

Implements [`Widget`](https://docs.rs/ratatui/latest/ratatui/widgets/trait.Widget.html) for
`&mut T`, allowing the widget to mutate its own state during rendering.

### Intermediate Patterns

#### [`stateful-widget.rs`] - Stateful Widget Pattern

**Best for**: Clean separation of widget logic from state
**Pros**: Separates widget logic from state, reusable, idiomatic Ratatui pattern
**Cons**: State must be managed externally

Uses [`StatefulWidget`](https://docs.rs/ratatui/latest/ratatui/widgets/trait.StatefulWidget.html)
to keep rendering logic separate from state management.

#### [`component-trait.rs`] - Custom Component Trait

**Best for**: Implementing consistent behavior across multiple widget types
**Pros**: Flexible, allows custom render signatures, good for widget frameworks
**Cons**: Non-standard, requires users to learn your custom API

Creates a custom trait similar to [`Widget`](https://docs.rs/ratatui/latest/ratatui/widgets/trait.Widget.html)
but with a `&mut self` render method for direct mutation.

### Advanced Patterns

#### [`nested-mutable-widget.rs`] - Nested Mutable Widgets

**Best for**: Parent-child widget relationships with mutable state
**Pros**: Hierarchical organization, each widget manages its own state
**Cons**: Complex borrowing, requires careful lifetime management

Demonstrates how to nest widgets that both need mutable access to their state.

#### [`nested-stateful-widget.rs`] - Nested Stateful Widgets

**Best for**: Complex applications with hierarchical state management
**Pros**: Clean separation, composable, scales well with application complexity
**Cons**: More boilerplate, requires understanding of nested state patterns

Shows how to compose multiple [`StatefulWidget`](https://docs.rs/ratatui/latest/ratatui/widgets/trait.StatefulWidget.html)s
in a parent-child hierarchy.

#### [`refcell.rs`] - Interior Mutability Pattern

**Best for**: Shared state across multiple widgets, complex state sharing scenarios
**Pros**: Allows shared mutable access, works with immutable widget references
**Cons**: Runtime borrow checking, potential panics, harder to debug

Uses [`Rc<RefCell<T>>`](https://doc.rust-lang.org/std/rc/struct.Rc.html) for interior mutability
when multiple widgets need access to the same state.

#### [`widget-with-mutable-ref.rs`] - Lifetime-Based Mutable References

**Best for**: Advanced users who need precise control over state lifetime
**Pros**: Zero-cost abstraction, explicit lifetime management
**Cons**: Complex lifetimes, requires deep Rust knowledge, easy to get wrong

Stores mutable references directly in widget structs using explicit lifetimes.

## Choosing the Right Pattern

**For most applications, start with immutable patterns:**

1. **Simple apps**: Use `immutable-function` for basic rendering with external state management
2. **Modern Ratatui**: Use `immutable-shared-ref` for reusable, efficient widgets (recommended)
3. **Legacy compatibility**: Use `immutable-consuming` when working with older code patterns

**Use mutable patterns when widgets need to update state during rendering:**

1. **Simple mutable state**: Begin with `mutable-function` or `mutable-widget` for prototypes
2. **Clean separation**: Use `stateful-widget` when you want to separate widget logic from state
3. **Hierarchical widgets**: Use `nested-*` patterns for complex widget relationships
4. **Shared state**: Use `refcell` when multiple widgets need the same state
5. **Performance critical**: Consider `widget-with-mutable-ref` for advanced lifetime management

## Common Pitfalls

- **Borrowing issues**: The borrow checker can be challenging with mutable state.
  [`StatefulWidget`](https://docs.rs/ratatui/latest/ratatui/widgets/trait.StatefulWidget.html)
  often provides the cleanest solution.
- **Overengineering**: Don't use complex patterns like `refcell` or `widget-with-mutable-ref`
  unless you actually need them.
- **State organization**: Keep state close to where it's used. Don't pass state through many
  layers unnecessarily.

[`component-trait.rs`]: ./src/bin/component-trait.rs
[`immutable-consuming.rs`]: ./src/bin/immutable-consuming.rs
[`immutable-function.rs`]: ./src/bin/immutable-function.rs
[`immutable-shared-ref.rs`]: ./src/bin/immutable-shared-ref.rs
[`mutable-widget.rs`]: ./src/bin/mutable-widget.rs
[`nested-mutable-widget.rs`]: ./src/bin/nested-mutable-widget.rs
[`nested-stateful-widget.rs`]: ./src/bin/nested-stateful-widget.rs
[`refcell.rs`]: ./src/bin/refcell.rs
[`mutable-function.rs`]: ./src/bin/mutable-function.rs
[`stateful-widget.rs`]: ./src/bin/stateful-widget.rs
[`widget-with-mutable-ref.rs`]: ./src/bin/widget-with-mutable-ref.rs
