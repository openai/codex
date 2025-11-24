//! A basic building block for building an Eventsource from a Stream of bytes array like objects. To
//! learn more about Server Sent Events (SSE) take a look at [the MDN
//! docs](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events)
//!
//! # Example
//!
//! ```ignore
//! let mut stream = reqwest::Client::new()
//!     .get("http://localhost:7020/notifications")
//!     .send()
//!     .await?
//!     .bytes_stream()
//!     .eventsource();
//!
//!
//! while let Some(event) = stream.next().await {
//!     match event {
//!         Ok(event) => println!(
//!             "received event[type={}]: {}",
//!             event.event,
//!             event.data
//!         ),
//!         Err(e) => eprintln!("error occured: {}", e),
//!     }
//! }
//! ```

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

mod event;
mod event_stream;
mod parser;
mod traits;
mod utf8_stream;

pub use event::Event;
pub use event_stream::{EventStream, EventStreamError};
pub use traits::Eventsource;
