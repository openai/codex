//! Managed version of the pool.
//!
//! "Managed" means that it requires a [`Manager`] which is responsible for
//! creating and recycling objects as they are needed.
//!
//! # Example
//!
//! ```rust
//! use deadpool::managed;
//!
//! #[derive(Debug)]
//! enum Error { Fail }
//!
//! struct Computer {}
//!
//! impl Computer {
//!     async fn get_answer(&self) -> i32 {
//!         42
//!     }
//! }
//!
//! struct Manager {}
//!
//! impl managed::Manager for Manager {
//!     type Type = Computer;
//!     type Error = Error;
//!
//!     async fn create(&self) -> Result<Computer, Error> {
//!         Ok(Computer {})
//!     }
//!     async fn recycle(&self, conn: &mut Computer, _: &managed::Metrics) -> managed::RecycleResult<Error> {
//!         Ok(())
//!     }
//! }
//!
//! type Pool = managed::Pool<Manager>;
//!
//! #[tokio::main]
//! async fn main() {
//!     let mgr = Manager {};
//!     let pool = Pool::builder(mgr).max_size(16).build().unwrap();
//!     let mut conn = pool.get().await.unwrap();
//!     let answer = conn.get_answer().await;
//!     assert_eq!(answer, 42);
//! }
//! ```
//!
//! For a more complete example please see
//! [`deadpool-postgres`](https://crates.io/crates/deadpool-postgres) crate.

mod builder;
mod config;
mod dropguard;
mod errors;
mod hooks;
mod manager;
mod metrics;
mod object;
mod pool;
pub mod reexports;

pub use crate::Status;

pub use self::{
    builder::{BuildError, PoolBuilder},
    config::{CreatePoolError, PoolConfig, QueueMode, Timeouts},
    errors::{PoolError, RecycleError, TimeoutType},
    hooks::{Hook, HookError, HookFuture, HookResult},
    manager::{Manager, RecycleResult},
    metrics::Metrics,
    object::{Object, ObjectId},
    pool::{Pool, RetainResult, WeakPool},
};
