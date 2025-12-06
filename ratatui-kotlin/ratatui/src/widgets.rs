#![warn(missing_docs)]
//! Widgets are the building blocks of user interfaces in Ratatui.
//!
//! They are used to create and manage the layout and style of the terminal interface. Widgets can
//! be combined and nested to create complex UIs, and can be easily customized to suit the needs of
//! your application.
//!
//! Ratatui provides a wide variety of built-in widgets that can be used to quickly create UIs.
//! Additionally, [`String`], [`&str`], [`Span`], [`Line`], and [`Text`] can be used as widgets
//! (though often [`Paragraph`] is used instead of these directly as it allows wrapping and
//! surrounding the text with a block).
//!
//! # Crate Organization
//!
//! Starting with Ratatui 0.30.0, the project was split into multiple crates for better modularity:
//!
//! - **[`ratatui-core`]**: Contains the core widget traits ([`Widget`], [`StatefulWidget`]) and
//!   text-related types ([`String`], [`&str`], [`Span`], [`Line`], [`Text`])
//! - **[`ratatui-widgets`]**: Contains all the built-in widget implementations ([`Block`],
//!   [`Paragraph`], [`List`], etc.)
//! - **[`ratatui`](crate)**: The main crate that re-exports everything for convenience. The
//!   unstable [`WidgetRef`] and [`StatefulWidgetRef`] traits are defined in the main `ratatui`
//!   crate as they are experimental.
//!
//! This split serves different user needs:
//!
//! - **App Authors**: Most application developers should use the main [`ratatui`](crate) crate,
//!   which provides everything needed to build terminal applications with widgets, backends, and
//!   layout systems
//! - **Widget Library Authors**: When creating third-party widget libraries, consider depending
//!   only on [`ratatui-core`] to avoid pulling in unnecessary built-in widgets and reduce
//!   compilation time for your users
//! - **Minimalist Projects**: Use [`ratatui-core`] directly if you only need the fundamental traits
//!   and text types without any built-in widgets
//!
//! The modular structure allows widget library authors to create lightweight dependencies while
//! still being compatible with the broader Ratatui ecosystem.
//!
//! [`ratatui-core`]: https://crates.io/crates/ratatui-core
//! [`ratatui-widgets`]: https://crates.io/crates/ratatui-widgets
//!
//! # Built-in Widgets
//!
//! Ratatui provides a comprehensive set of built-in widgets:
//!
//! - [`Block`]: a basic widget that draws a block with optional borders, titles and styles.
//! - [`BarChart`]: displays multiple datasets as bars with optional grouping.
//! - [`calendar::Monthly`]: displays a single month.
//! - [`Canvas`]: draws arbitrary shapes using drawing characters.
//! - [`Chart`]: displays multiple datasets as a lines or scatter graph.
//! - [`Clear`]: clears the area it occupies. Useful to render over previously drawn widgets.
//! - [`Gauge`]: displays progress percentage using block characters.
//! - [`LineGauge`]: display progress as a line.
//! - [`List`]: displays a list of items and allows selection.
//! - [`Paragraph`]: displays a paragraph of optionally styled and wrapped text.
//! - [`Scrollbar`]: displays a scrollbar.
//! - [`Sparkline`]: display a single data set as a sparkline.
//! - [`Table`]: displays multiple rows and columns in a grid and allows selection.
//! - [`Tabs`]: displays a tab bar and allows selection.
//! - [`RatatuiLogo`]: displays the Ratatui logo.
//! - [`RatatuiMascot`]: displays the Ratatui mascot.
//!
//! Additionally, primitive text types implement [`Widget`]:
//! - [`String`]: renders the owned string content
//! - [`&str`]: renders the string slice content
//! - [`Line`]: renders a single line of styled text spans
//! - [`Span`]: renders a styled text segment
//! - [`Text`]: renders multiple lines of styled text
//!
//! For more information on these widgets, you can view the widget showcase and examples.
//!
//! # Third-Party Widgets
//!
//! Beyond the built-in widgets, there's a rich ecosystem of third-party widgets available that
//! extend Ratatui's functionality. These community-contributed widgets provide specialized UI
//! components for various use cases.
//!
//! To discover third-party widgets:
//!
//! - **Search crates.io**: Look for crates with "tui" or "ratatui" in their names or descriptions
//! - **Awesome Ratatui**: Check the [Awesome Ratatui](https://github.com/ratatui-org/awesome-ratatui)
//!   repository for a curated list of widgets, libraries, and applications
//! - **Widget Showcase**: Browse the [third-party widgets showcase](https://ratatui.rs/showcase/third-party-widgets/)
//!   on the Ratatui website to see widgets in action
//!
//! These third-party widgets cover a wide range of functionality including specialized input
//! components, data visualization widgets, layout helpers, and domain-specific UI elements.
//!
//! [`Canvas`]: crate::widgets::canvas::Canvas
//! [`Frame`]: crate::Frame
//! [`Terminal::draw`]: crate::Terminal::draw
//! [`Line`]: crate::text::Line
//! [`Span`]: crate::text::Span
//! [`Text`]: crate::text::Text
//! [`String`]: alloc::string::String
//! [`&str`]: str
//!
//! # Widget Traits
//!
//! In Ratatui, widgets are implemented as Rust traits, which allow for easy implementation and
//! extension. The main traits for widgets are:
//!
//! - [`Widget`]: Basic trait for stateless widgets that are consumed when rendered
//! - [`StatefulWidget`]: Trait for widgets that maintain state between renders
//! - [`WidgetRef`]: Trait for rendering widgets by reference (unstable)
//! - [`StatefulWidgetRef`]: Trait for rendering stateful widgets by reference (unstable)
//!
//! ## `Widget`
//!
//! The [`Widget`] trait is the most basic trait for widgets in Ratatui. It provides the basic
//! functionality for rendering a widget onto a buffer. Widgets implementing this trait are consumed
//! when rendered.
//!
//! ```rust
//! # use ratatui_core::{buffer::Buffer, layout::Rect};
//! pub trait Widget {
//!     fn render(self, area: Rect, buf: &mut Buffer);
//! }
//! ```
//!
//! Prior to Ratatui 0.26.0, widgets were generally created for each frame as they were consumed
//! during rendering. This meant that they were not meant to be stored but used as *commands* to
//! draw common figures in the UI. Starting with 0.26.0, implementing widgets on references became
//! the preferred pattern for reusability.
//!
//! ## `StatefulWidget`
//!
//! The [`StatefulWidget`] trait is similar to the [`Widget`] trait, but also includes state that
//! can be managed and updated during rendering. This is useful for widgets that need to remember
//! things between draw calls, such as scroll position or selection state.
//!
//! ```rust
//! # use ratatui_core::{buffer::Buffer, layout::Rect};
//! pub trait StatefulWidget {
//!     type State;
//!     fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State);
//! }
//! ```
//!
//! For example, the built-in [`List`] widget can highlight the currently selected item. This
//! requires maintaining an offset to ensure the selected item is visible within the viewport.
//! Without state, the widget could only provide basic scrolling behavior, but with access to the
//! previous offset, it can implement natural scrolling where the offset is preserved until the
//! selected item moves out of view.
//!
//! ## `WidgetRef` and `StatefulWidgetRef`
//!
//! The [`WidgetRef`] and [`StatefulWidgetRef`] traits were introduced in Ratatui 0.26.0 to enable
//! rendering widgets by reference instead of consuming them. These traits address several important
//! use cases that the original `Widget` and `StatefulWidget` traits couldn't handle elegantly.
//!
//! ```rust
//! # use ratatui_core::{buffer::Buffer, layout::Rect};
//! # #[cfg(feature = "unstable-widget-ref")]
//! pub trait WidgetRef {
//!     fn render_ref(&self, area: Rect, buf: &mut Buffer);
//! }
//!
//! # #[cfg(feature = "unstable-widget-ref")]
//! pub trait StatefulWidgetRef {
//!     type State;
//!     fn render_ref(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State);
//! }
//! ```
//!
//! The reference-based traits solve several key problems:
//!
//! - **Reusability**: Widgets can be rendered multiple times without being consumed
//! - **Collections**: Store heterogeneous widgets in collections like `Vec<Box<dyn WidgetRef>>`
//! - **Borrowing**: Render widgets when you only have a reference, not ownership
//! - **Efficiency**: Avoid unnecessary cloning or reconstruction for repeated renders
//!
//! These traits are currently **experimental** and gated behind the `unstable-widget-ref` feature
//! flag. This means:
//!
//! - The API may change in future releases
//! - Method names, signatures, or behavior might be adjusted based on community feedback
//! - You must explicitly enable the feature flag to use them: `features = ["unstable-widget-ref"]`
//! - They are not covered by semantic versioning guarantees until stabilized
//!
//! The traits are being evaluated for potential breaking changes and improvements. See the
//! [tracking issue](https://github.com/ratatui/ratatui/issues/1287) for ongoing discussions and
//! design considerations.
//!
//! # Rendering Widgets
//!
//! Widgets are typically rendered using the [`Frame`] type, which provides methods for rendering
//! both consuming and reference-based widgets. These methods are usually called from the closure
//! passed to [`Terminal::draw`].
//!
//! ## Rendering Consuming Widgets
//!
//! Most widgets in Ratatui are rendered using `Frame::render_widget()`, which consumes the widget
//! when rendering. This is the standard approach for stateless widgets that don't need to persist
//! data between frames.
//!
//! ```rust
//! # use ratatui::{backend::TestBackend, Terminal};
//! # use ratatui::widgets::Paragraph;
//! # let backend = TestBackend::new(10, 3);
//! # let mut terminal = Terminal::new(backend).unwrap();
//! terminal.draw(|frame| {
//!     let widget = Paragraph::new("Hello, world!");
//!     frame.render_widget(widget, frame.area());
//! });
//! ```
//!
//! ## Rendering Widget References
//!
//! When you implement widgets on references (`Widget for &MyWidget`), you can render them directly
//! using the same `Frame::render_widget()` method. This approach enables widget reuse without
//! reconstruction and is the recommended pattern for new widgets.
//!
//! ```rust
//! # use ratatui::{backend::TestBackend, Terminal};
//! # use ratatui::widgets::{Block, Paragraph};
//! # let backend = TestBackend::new(10, 3);
//! # let mut terminal = Terminal::new(backend).unwrap();
//! // Create the widget outside the draw closure
//! let paragraph = Paragraph::new("Hello, world!").block(Block::bordered());
//!
//! terminal.draw(|frame| {
//!     // Widget can be rendered by reference without being consumed
//!     frame.render_widget(&paragraph, frame.area());
//! });
//!
//! // The widget can be used again in subsequent frames
//! terminal.draw(|frame| {
//!     frame.render_widget(&paragraph, frame.area());
//! });
//! ```
//!
//! ## Rendering Stateful Widgets
//!
//! Widgets that need to maintain state between frames use `Frame::render_stateful_widget()`. This
//! method takes both the widget and a mutable reference to its state, allowing the widget to read
//! and modify state during rendering (such as updating scroll positions or handling selections).
//!
//! ```rust
//! # use ratatui::{backend::TestBackend, Terminal};
//! # use ratatui::widgets::{List, ListItem, ListState};
//! # let backend = TestBackend::new(10, 3);
//! # let mut terminal = Terminal::new(backend).unwrap();
//! let mut list_state = ListState::default();
//! terminal.draw(|frame| {
//!     let items = vec![ListItem::new("Item 1"), ListItem::new("Item 2")];
//!     let list = List::new(items);
//!     frame.render_stateful_widget(list, frame.area(), &mut list_state);
//! });
//! ```
//!
//! ## Single Root Widget Pattern
//!
//! A common compositional pattern in Ratatui applications is to have a single root widget (often an
//! `App` struct) that represents your entire application state. This widget is passed to
//! `Frame::render_widget()`, and within its render method, it calls render on child widgets
//! directly. This pattern provides a clean separation between your application logic and rendering
//! code, and allows for easy composition of complex UIs from simpler components.
//!
//! ```rust
//! # use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};
//! # use ratatui::widgets::{Block, Paragraph};
//! #[derive(Default)]
//! struct App {
//!     should_quit: bool,
//! }
//!
//! impl Widget for &App {
//!     fn render(self, area: Rect, buf: &mut Buffer) {
//!         // Render header
//!         let header = Paragraph::new("My App").block(Block::bordered());
//!         header.render(Rect::new(area.x, area.y, area.width, 3), buf);
//!
//!         // Render main content
//!         let content = Paragraph::new("Main content area");
//!         content.render(
//!             Rect::new(area.x, area.y + 3, area.width, area.height - 3),
//!             buf,
//!         );
//!     }
//! }
//! ```
//!
//! # Authoring Custom Widgets
//!
//! When implementing custom widgets in Ratatui, you'll make fundamental decisions about how your
//! widget manages state and how it's used by applications. Understanding these choices will help
//! you create widgets that fit well into your application's architecture. Widget implementation
//! involves several key architectural decisions that work together to determine how your widget
//! behaves - these decisions are independent but complementary, allowing you to mix and match
//! approaches based on your specific needs.
//!
//! **State Management**: The first choice is where state lives. Some widgets need to track
//! information between renders - things like scroll positions, selections, or counters. You can
//! either build this state into the widget itself (widget-owned state) or keep it separate and pass
//! it in during rendering (external state).
//!
//! **Ownership Model**: The second choice is how the widget is consumed. Widgets can either be
//! consumed when rendered (taking ownership) or work by reference (borrowing). Reference-based
//! widgets can be stored and reused across multiple frames, while consuming widgets are created
//! fresh each time.
//!
//! **`StatefulWidget` vs Mutable References**: When your widget needs state, you have two main
//! approaches. The [`StatefulWidget`] trait represents the established pattern - it separates the
//! widget from its state, allowing the application to own and manage the state independently. This
//! is what you'll see in most existing Ratatui code and built-in widgets like [`List`] and
//! [`Table`]. The mutable reference approach (`Widget for &mut MyWidget`) is newer and less common,
//! but useful when the state is intrinsic to the widget's identity. With mutable references, the
//! widget owns its state directly.
//!
//! The key question for state management is: "If I recreate this widget, should the state reset?"
//! If yes (like a counter that should start at zero), use mutable references with widget-owned
//! state. If no (like a list selection that should persist), use [`StatefulWidget`] with external
//! state that the application manages.
//!
//! **Evolution and Current Recommendations**: Ratatui's patterns have evolved significantly. Before
//! version 0.26.0, widgets were typically consuming (`Widget for MyWidget`) and created fresh each
//! frame. Starting with 0.26.0, reference-based widgets (`Widget for &MyWidget`) became possible,
//! allowing widgets to be stored and reused. You'll encounter both patterns in existing code, but
//! reference-based implementations are now recommended for new widgets because they enable
//! reusability and automatic [`WidgetRef`] support through blanket implementations.
//!
//! For new widgets, implement [`Widget`] or [`StatefulWidget`] on references to your widget types
//! (`&MyWidget` or `&mut MyWidget`). This provides reusability and automatic [`WidgetRef`] support.
//! You can optionally implement the consuming version for backward compatibility.
//!
//! ## State Management Patterns
//!
//! For a comprehensive exploration of different approaches to handling both mutable and immutable
//! state in widgets, see the [state examples] in the Ratatui repository. These examples demonstrate
//! various patterns including:
//!
//! **Immutable State Patterns** (recommended for most use cases):
//! - Function-based immutable state (`fn render(frame: &mut Frame, area: Rect, state: &State)`)
//! - Shared reference widgets (`impl Widget for &MyWidget`)
//! - Consuming widgets (`impl Widget for MyWidget`)
//!
//! **Mutable State Patterns** (for widgets that modify state during rendering):
//! - Function-based mutable state (`fn render(frame: &mut Frame, area: Rect, state: &mut State)`)
//! - Mutable widget references (`impl Widget for &mut MyWidget`)
//! - `StatefulWidget` pattern (`impl StatefulWidget for MyWidget`)
//! - Custom component traits (`trait MyComponent { fn render(&mut self, frame: &mut Frame, area:
//!   Rect) }`)
//! - Interior mutability with `RefCell` (`struct MyWidget { state: Rc<RefCell<State>> }`)
//! - Lifetime-based mutable references (`struct MyWidget<'a> { state: &'a mut State }`)
//! - Nested widget hierarchies (compositions with owned or external state)
//!
//! Each pattern has different trade-offs in terms of complexity, performance, and architectural
//! fit, making them suitable for different use cases and application designs. For most
//! applications, start with immutable patterns as they are simpler to reason about and less prone
//! to borrowing issues.
//!
//! [state examples]: https://github.com/ratatui/ratatui/tree/main/examples/concepts/state
//!
//! ## Shared References (`&Widget`)
//!
//! The recommended pattern for most new widgets implements [`Widget`] on a shared reference,
//! allowing the widget to be rendered multiple times without being consumed. This approach is ideal
//! for immutable widgets that don't need to modify their internal state during rendering, and it's
//! the most common pattern you should use for new widgets.
//!
//! ```rust
//! # use ratatui_core::{buffer::Buffer, layout::Rect, text::Line, widgets::Widget};
//! struct MyWidget {
//!     content: String,
//! }
//!
//! impl Widget for &MyWidget {
//!     fn render(self, area: Rect, buf: &mut Buffer) {
//!         Line::raw(&self.content).render(area, buf);
//!     }
//! }
//! ```
//!
//! This automatically provides [`WidgetRef`] support through blanket implementations and enables
//! widgets to be stored and reused across frames without reconstruction. For most use cases where
//! the widget doesn't need to change its internal state during rendering, this is the best choice.
//!
//! ## Mutable References (`&mut Widget`)
//!
//! For widgets that need to modify their internal state during rendering, implement [`Widget`] on a
//! mutable reference. This is a newer pattern that's less common but useful when the state is
//! intrinsic to the widget's identity and behavior. Use this pattern when the widget should own and
//! manage its state directly, rather than having external state passed in.
//!
//! ```rust
//! # use ratatui_core::{buffer::Buffer, layout::Rect, text::Line, widgets::Widget};
//! struct CounterWidget {
//!     count: u32, // This state belongs to the widget
//!     label: String,
//! }
//!
//! impl Widget for &mut CounterWidget {
//!     fn render(self, area: Rect, buf: &mut Buffer) {
//!         self.count += 1; // State changes as part of rendering behavior
//!         let text = format!("{label}: {count}", label = self.label, count = self.count);
//!         Line::raw(text).render(area, buf);
//!     }
//! }
//! ```
//!
//! This pattern works well when the widget owns its state and the state is part of the widget's
//! identity. It's ideal for counters, animations, cursors, progress indicators, or other
//! widget-specific behavior where the state should reset when you create a new widget instance.
//!
//! ## Consuming Widget Implementation
//!
//! The consuming widget pattern was the original approach in Ratatui and remains very common in
//! existing codebases. You'll encounter this pattern frequently when reading examples and community
//! code. Widgets implementing this pattern take ownership when rendered, which means they're
//! consumed on each use. While not the recommended approach for new widgets, it's still useful to
//! understand this pattern for compatibility and when working with existing code.
//!
//! ```rust
//! # use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, text::{Line, Span}, widgets::Widget};
//! struct GreetingWidget {
//!     name: String,
//! }
//!
//! impl Widget for GreetingWidget {
//!     fn render(self, area: Rect, buf: &mut Buffer) {
//!         let hello = Span::raw("Hello, ");
//!         let name = Span::styled(self.name, Modifier::BOLD);
//!         let line = Line::from(vec![hello, name]);
//!         line.render(area, buf);
//!     }
//! }
//! ```
//!
//! This approach is simpler and works well for widgets created fresh each frame, but it means the
//! widget cannot be reused. Before reference-based widgets were introduced in version 0.26.0, this
//! was the standard pattern, and it's still valid for simple use cases or when following existing
//! code patterns.
//!
//! The easiest way to implement this pattern when you have a reference-based widget is to implement
//! the consuming version on the owned type, which can then call the reference-based implementation:
//!
//! ```rust
//! # use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};
//! # struct GreetingWidget;
//! # impl Widget for &GreetingWidget {
//! #     fn render(self, area: Rect, buf: &mut Buffer) {}
//! # }
//! impl Widget for GreetingWidget {
//!     fn render(self, area: Rect, buf: &mut Buffer) {
//!         // Call the reference-based implementation
//!         (&self).render(area, buf);
//!     }
//! }
//! ``````
//!
//! ## `StatefulWidget` Implementation
//!
//! When your widget needs to work with external state - data that exists independently of the
//! widget and should persist between widget instances - implement [`StatefulWidget`]. This is the
//! established pattern used by built-in widgets like [`List`] and [`Table`], where the widget
//! configuration is separate from application state like selections or scroll positions.
//!
//! Like [`Widget`], you can implement [`StatefulWidget`] on references to allow reuse, though it's
//! more common to see this trait implemented on owned types which are consumed during rendering.
//!
//! ```rust
//! # use ratatui_core::{buffer::Buffer, layout::Rect, text::Line, widgets::{StatefulWidget, Widget}};
//! struct ListView {
//!     items: Vec<String>,
//! }
//!
//! #[derive(Default)]
//! struct ListState {
//!     selected: Option<usize>, // This is application state
//!     scroll_offset: usize,
//! }
//!
//! impl StatefulWidget for ListView {
//!     type State = ListState;
//!
//!     fn render(self, area: Rect, buf: &mut Buffer, state: &mut ListState) {
//!         // Render based on external state, possibly modify for scrolling
//!         let display_text = state
//!             .selected
//!             .and_then(|i| self.items.get(i))
//!             .map_or("None selected", |s| s.as_str());
//!         Line::raw(display_text).render(area, buf);
//!     }
//! }
//! ```
//!
//! This pattern is ideal for selections, scroll positions, form data, or any state that should
//! persist between renders or be shared across your application. The state exists independently of
//! the widget, so recreating the widget doesn't reset the state.
//!
//! ### Automatic `WidgetRef` Support
//!
//! When you implement `Widget for &MyWidget`, you automatically get [`WidgetRef`] support without
//! any additional code. Ratatui provides blanket implementations that automatically implement these
//! traits for any type that implements [`Widget`] or [`StatefulWidget`] on a reference. This means
//! that implementing `Widget for &MyWidget` gives you both the standard widget functionality and
//! the unstable [`WidgetRef`] capabilities for free.
//!
//! ## Manual `WidgetRef` Implementation (Advanced)
//!
//! Manual implementation of [`WidgetRef`] or [`StatefulWidgetRef`] is only necessary when you need
//! to store widgets as trait objects (`Box<dyn WidgetRef>`) or when you want a different API than
//! the reference-based [`Widget`] implementation provides. In most cases, the automatic
//! implementation via blanket implementations is sufficient.
//!
//! These traits enable several benefits:
//! - Widgets can be stored and rendered multiple times without reconstruction
//! - Collections of widgets with different types can be stored using `Box<dyn WidgetRef>`
//! - Avoids the consumption model while maintaining backward compatibility
//!
//! Manual implementation is only needed when you want to use trait objects or need a different API
//! than the reference-based [`Widget`] implementation:
//!
//! ```rust
//! # #[cfg(feature = "unstable-widget-ref")] {
//! # use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, text::{Line, Span}};
//! # use ratatui::widgets::{Widget, WidgetRef};
//! struct GreetingWidget {
//!     name: String,
//! }
//!
//! // Manual WidgetRef implementation (usually not needed)
//! impl WidgetRef for GreetingWidget {
//!     fn render_ref(&self, area: Rect, buf: &mut Buffer) {
//!         let hello = Span::raw("Hello, ");
//!         let name = Span::styled(&self.name, Modifier::BOLD);
//!         let line = Line::from(vec![hello, name]);
//!         line.render(area, buf);
//!     }
//! }
//!
//! // For backward compatibility
//! impl Widget for GreetingWidget {
//!     fn render(self, area: Rect, buf: &mut Buffer) {
//!         self.render_ref(area, buf);
//!     }
//! }
//! # }
//! ```
//!
//! This pattern allows the widget to be stored and rendered multiple times:
//!
//! ```rust
//! # #[cfg(feature = "unstable-widget-ref")] {
//! # use ratatui_core::{buffer::Buffer, layout::Rect};
//! # use ratatui::widgets::WidgetRef;
//! # struct GreetingWidget { name: String }
//! # impl WidgetRef for GreetingWidget {
//! #     fn render_ref(&self, area: Rect, buf: &mut Buffer) {}
//! # }
//! struct App {
//!     greeting: GreetingWidget,
//! }
//!
//! // The widget can be rendered multiple times without reconstruction
//! fn render_app(app: &App, area: Rect, buf: &mut Buffer) {
//!     app.greeting.render_ref(area, buf);
//! }
//! # }
//! ```
//!
//! ### Using Trait Objects for Dynamic Collections
//!
//! The main benefit of manual [`WidgetRef`] implementation is the ability to create collections of
//! different widget types using trait objects. This is useful when you need to store widgets with
//! types that are not known at compile time:
//!
//! ```rust
//! # #[cfg(feature = "unstable-widget-ref")] {
//! # use ratatui_core::{buffer::Buffer, layout::Rect};
//! # use ratatui::widgets::WidgetRef;
//! # struct Greeting;
//! # struct Farewell;
//! # impl WidgetRef for Greeting { fn render_ref(&self, area: Rect, buf: &mut Buffer) {} }
//! # impl WidgetRef for Farewell { fn render_ref(&self, area: Rect, buf: &mut Buffer) {} }
//! # let area = Rect::new(0, 0, 10, 3);
//! # let mut buf = &mut Buffer::empty(area);
//! let widgets: Vec<Box<dyn WidgetRef>> = vec![Box::new(Greeting), Box::new(Farewell)];
//!
//! for widget in &widgets {
//!     widget.render_ref(area, buf);
//! }
//! # }
//! ```
//!
//! However, if you implement `Widget for &MyWidget`, you can achieve similar functionality by
//! storing references or using the automatic [`WidgetRef`] implementation without needing to
//! manually implement the trait.
//!
//! ## Authoring Custom Widget Libraries
//!
//! When creating a library of custom widgets for distribution, there are specific considerations
//! that will make your library more compatible and accessible to a wider range of users. Following
//! these guidelines will help ensure your widget library works well in various environments and
//! can be easily integrated into different types of applications.
//!
//! ### Depend on `ratatui_core`
//!
//! For widget libraries, depend on [`ratatui-core`] instead of the full `ratatui` crate. This
//! provides all the essential types and traits needed for widget development while avoiding
//! unnecessary dependencies on backends and other components that widget libraries don't need.
//!
//! This approach offers several key advantages for both library authors and users:
//!
//! - **Lighter dependencies**: Users don't pull in backend code they don't need, keeping their
//!   dependency tree smaller and more focused
//! - **Better compile times**: Fewer dependencies mean faster builds for both development and
//!   end-user projects
//! - **Future-proofing**: Your library remains compatible as Ratatui evolves its architecture,
//!   since core widget functionality is stable across versions
//!
//! ### Make Your Crate `no_std` Compatible
//!
//! For maximum compatibility, especially in embedded environments, consider making your widget
//! library `no_std` compatible. This is often easier than you might expect and broadens the range
//! of projects that can use your widgets.
//!
//! To implement `no_std` compatibility, add the `#![no_std]` attribute at the top of your `lib.rs`.
//! When working in a `no_std` environment, you'll need to make a few adjustments:
//!
//! - Use `core::` instead of `std::` for basic functionality
//! - Add `extern crate alloc;` to access allocation types
//! - Use `alloc::` for heap-allocated types like `String`, `Vec`, and `Box`
//!
//! Here's a complete example of a `no_std` compatible widget:
//!
//! ```ignore
//! #![no_std]
//!
//! extern crate alloc;
//!
//! use alloc::string::String;
//!
//! use ratatui_core::buffer::Buffer;
//! use ratatui_core::layout::Rect;
//! use ratatui_core::text::Line;
//! use ratatui_core::widgets::Widget;
//!
//! struct MyWidget {
//!     content: String,
//! }
//!
//! impl Widget for &MyWidget {
//!     fn render(self, area: Rect, buf: &mut Buffer) {
//!         Line::raw(&self.content).render(area, buf);
//!     }
//! }
//! ```
//!
//! The benefits of `no_std` compatibility include:
//!
//! - **Broader compatibility**: Your widgets work seamlessly in embedded environments and other
//!   `no_std` contexts where standard library functionality isn't available
//! - **Easy to adopt**: Even if you haven't worked with `no_std` development before, the changes
//!   are typically minimal for widget libraries. Most widget logic involves basic data manipulation
//!   and rendering operations that work well within `no_std` constraints, making this compatibility
//!   straightforward to implement
//!
//! [`ratatui-core`]: https://crates.io/crates/ratatui-core

pub use ratatui_core::widgets::{StatefulWidget, Widget};
pub use ratatui_widgets::barchart::{Bar, BarChart, BarGroup};
pub use ratatui_widgets::block::{Block, BlockExt, Padding, TitlePosition};
pub use ratatui_widgets::borders::{BorderType, Borders};
#[cfg(feature = "widget-calendar")]
pub use ratatui_widgets::calendar;
pub use ratatui_widgets::canvas;
pub use ratatui_widgets::chart::{Axis, Chart, Dataset, GraphType, LegendPosition};
pub use ratatui_widgets::clear::Clear;
pub use ratatui_widgets::gauge::{Gauge, LineGauge};
pub use ratatui_widgets::list::{List, ListDirection, ListItem, ListState};
pub use ratatui_widgets::logo::{RatatuiLogo, Size as RatatuiLogoSize};
pub use ratatui_widgets::mascot::{MascotEyeColor, RatatuiMascot};
pub use ratatui_widgets::paragraph::{Paragraph, Wrap};
pub use ratatui_widgets::scrollbar::{
    ScrollDirection, Scrollbar, ScrollbarOrientation, ScrollbarState,
};
pub use ratatui_widgets::sparkline::{RenderDirection, Sparkline, SparklineBar};
pub use ratatui_widgets::table::{Cell, HighlightSpacing, Row, Table, TableState};
pub use ratatui_widgets::tabs::Tabs;
#[instability::unstable(feature = "widget-ref")]
pub use {stateful_widget_ref::StatefulWidgetRef, widget_ref::WidgetRef};

mod stateful_widget_ref;
mod widget_ref;

use ratatui_core::layout::Rect;

/// Extension trait for [`Frame`] that provides methods to render [`WidgetRef`] and
/// [`StatefulWidgetRef`] to the current buffer.
#[instability::unstable(feature = "widget-ref")]
pub trait FrameExt {
    /// Render a [`WidgetRef`] to the current buffer using [`WidgetRef::render_ref`].
    ///
    /// Usually the area argument is the size of the current frame or a sub-area of the current
    /// frame (which can be obtained using [`Layout`] to split the total area).
    ///
    /// # Example
    ///
    /// ```rust
    /// # #[cfg(feature = "unstable-widget-ref")] {
    /// # use ratatui::{backend::TestBackend, Terminal};
    /// # let backend = TestBackend::new(5, 5);
    /// # let mut terminal = Terminal::new(backend).unwrap();
    /// # let mut frame = terminal.get_frame();
    /// use ratatui::layout::Rect;
    /// use ratatui::widgets::{Block, FrameExt};
    ///
    /// let block = Block::new();
    /// let area = Rect::new(0, 0, 5, 5);
    /// frame.render_widget_ref(&block, area);
    /// # }
    /// ```
    ///
    /// [`Layout`]: crate::layout::Layout
    fn render_widget_ref<W: WidgetRef>(&mut self, widget: W, area: Rect);

    /// Render a [`StatefulWidgetRef`] to the current buffer using
    /// [`StatefulWidgetRef::render_ref`].
    ///
    /// Usually the area argument is the size of the current frame or a sub-area of the current
    /// frame (which can be obtained using [`Layout`] to split the total area).
    ///
    /// The last argument should be an instance of the [`StatefulWidgetRef::State`] associated to
    /// the given [`StatefulWidgetRef`].
    ///
    /// # Example
    ///
    /// ```rust
    /// # #[cfg(feature = "unstable-widget-ref")] {
    /// # use ratatui::{backend::TestBackend, Terminal};
    /// # let backend = TestBackend::new(5, 5);
    /// # let mut terminal = Terminal::new(backend).unwrap();
    /// # let mut frame = terminal.get_frame();
    /// use ratatui::layout::Rect;
    /// use ratatui::widgets::{FrameExt, List, ListItem, ListState};
    ///
    /// let mut state = ListState::default().with_selected(Some(1));
    /// let list = List::new(vec![ListItem::new("Item 1"), ListItem::new("Item 2")]);
    /// let area = Rect::new(0, 0, 5, 5);
    /// frame.render_stateful_widget_ref(&list, area, &mut state);
    /// # }
    /// ```
    /// [`Layout`]: crate::layout::Layout
    fn render_stateful_widget_ref<W>(&mut self, widget: W, area: Rect, state: &mut W::State)
    where
        W: StatefulWidgetRef;
}

#[cfg(feature = "unstable-widget-ref")]
impl FrameExt for ratatui_core::terminal::Frame<'_> {
    fn render_widget_ref<W: WidgetRef>(&mut self, widget: W, area: Rect) {
        widget.render_ref(area, self.buffer_mut());
    }

    fn render_stateful_widget_ref<W>(&mut self, widget: W, area: Rect, state: &mut W::State)
    where
        W: StatefulWidgetRef,
    {
        widget.render_ref(area, self.buffer_mut(), state);
    }
}
