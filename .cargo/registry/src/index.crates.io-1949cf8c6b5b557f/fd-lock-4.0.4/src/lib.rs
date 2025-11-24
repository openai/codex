//! Advisory reader-writer locks for files.
//!
//! # Notes on Advisory Locks
//!
//! "advisory locks" are locks which programs must opt-in to adhere to. This
//! means that they can be used to coordinate file access, but not prevent
//! access. Use this to coordinate file access between multiple instances of the
//! same program. But do not use this to prevent actors from accessing or
//! modifying files.
//!
//! # Example
//!
//! ```no_run
//! # use std::io;
//! use std::io::prelude::*;
//! use std::fs::File;
//! use fd_lock::RwLock;
//!
//! # fn main() -> io::Result<()> {
//! // Lock a file and write to it.
//! let mut f = RwLock::new(File::open("foo.txt")?);
//! write!(f.write()?, "chashu cat")?;
//!
//! // A lock can also be held across multiple operations.
//! let mut f = f.write()?;
//! write!(f, "nori cat")?;
//! write!(f, "bird!")?;
//! # Ok(()) }
//! ```

#![forbid(future_incompatible)]
#![deny(missing_debug_implementations, nonstandard_style)]
#![cfg_attr(doc, warn(missing_docs, rustdoc::missing_doc_code_examples))]

mod read_guard;
mod rw_lock;
mod write_guard;

pub(crate) mod sys;

pub use read_guard::RwLockReadGuard;
pub use rw_lock::RwLock;
pub use write_guard::RwLockWriteGuard;
