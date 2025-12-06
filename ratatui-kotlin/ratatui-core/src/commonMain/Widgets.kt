#![warn(missing_docs)]
//! The `widgets` module contains the `Widget` and `StatefulWidget` traits, which are used to
//! render UI elements on the screen.

pub use self::stateful_widget::StatefulWidget;
pub use self::widget::Widget;

mod stateful_widget;
mod widget;
