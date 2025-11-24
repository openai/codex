//! std-based process-wrap API.
//!
//! See the [crate-level doc](crate) for more information.
//!
//! The recommended usage is to star-import this module:
//!
//! ```rust
//! use process_wrap::std::*;
//! ```

#[doc(inline)]
pub use core::{StdChild, StdChildWrapper, StdCommandWrap, StdCommandWrapper};
#[cfg(all(windows, feature = "creation-flags"))]
#[doc(inline)]
pub use creation_flags::CreationFlags;
#[cfg(all(windows, feature = "job-object"))]
#[doc(inline)]
pub use job_object::{JobObject, JobObjectChild};
#[cfg(all(unix, feature = "process-group"))]
#[doc(inline)]
pub use process_group::{ProcessGroup, ProcessGroupChild};
#[cfg(all(unix, feature = "process-session"))]
#[doc(inline)]
pub use process_session::ProcessSession;
#[cfg(all(unix, feature = "reset-sigmask"))]
#[doc(inline)]
pub use reset_sigmask::ResetSigmask;

mod core;
#[cfg(all(windows, feature = "creation-flags"))]
mod creation_flags;
#[cfg(all(windows, feature = "job-object"))]
mod job_object;
#[cfg(all(unix, feature = "process-group"))]
mod process_group;
#[cfg(all(unix, feature = "process-session"))]
mod process_session;
#[cfg(all(unix, feature = "reset-sigmask"))]
mod reset_sigmask;
