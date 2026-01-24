//! Streaming types for real-time generation.
//!
//! This module provides types and APIs for consuming streaming responses from AI models.
//!
//! # API Levels
//!
//! The streaming API has three levels of abstraction:
//!
//! 1. **Low-level**: [`StreamResponse`] with `next_event()` - direct event iteration
//! 2. **Callback-based**: [`StreamCallbacks`] trait - implement callbacks for events
//! 3. **Processor-based**: [`StreamProcessor`] - Crush-like accumulated state (recommended)
//!
//! # Crush-like Streaming (Recommended)
//!
//! The [`StreamProcessor`] provides a Crush-like API where a single message is
//! continuously updated during streaming:
//!
//! ```ignore
//! let response = model.stream(request).await?.into_processor()
//!     .on_update(|snapshot| async move {
//!         // UPDATE same message with accumulated state
//!         db.update_message(msg_id, &snapshot.text).await?;
//!         Ok(())
//!     })
//!     .await?;
//! ```
//!
//! # Quick Examples
//!
//! ## Collect to response
//! ```ignore
//! let response = model.stream(request).await?.into_processor().collect().await?;
//! ```
//!
//! ## Print to stdout
//! ```ignore
//! let response = model.stream(request).await?.into_processor().println().await?;
//! ```

pub mod callbacks;
pub mod events;
pub mod processor;
pub mod response;
pub mod snapshot;
pub mod update;

// Internal processor state modules
pub(crate) mod processor_state;

// Callbacks API
pub use callbacks::CollectTextCallbacks;
pub use callbacks::PrintCallbacks;
pub use callbacks::StreamCallbacks;

// Events API
pub use events::StreamError;
pub use events::StreamEvent;

// Response API
pub use response::DEFAULT_IDLE_TIMEOUT;
pub use response::EventStream;
pub use response::StreamConfig;
pub use response::StreamResponse;

// Processor API (Crush-like)
pub use processor::StreamProcessor;
pub use snapshot::StreamSnapshot;
pub use snapshot::ThinkingSnapshot;
pub use snapshot::ToolCallSnapshot;
pub use update::StreamUpdate;
