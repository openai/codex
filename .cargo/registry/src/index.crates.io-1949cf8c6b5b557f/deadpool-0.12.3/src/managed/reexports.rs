//! This module contains all things that should be reexported
//! by backend implementations in order to avoid direct dependencies
//! on the `deadpool` crate itself.
//!
//! Crates based on `deadpool::managed` should include this line:
//! ```rust,ignore
//! pub use deadpool::managed::reexports::*;
//! deadpool::managed_reexports!(
//!     "name_of_crate",
//!     Manager,
//!     Object<Manager>,
//!     Error,
//!     ConfigError
//! );
//! ```

pub use crate::{
    managed::{Metrics, ObjectId, PoolConfig, Status, TimeoutType, Timeouts},
    Runtime,
};

/// This macro creates all the type aliases usually reexported by
/// deadpool-* crates. Crates that implement a deadpool manager should
/// be considered stand alone crates and users of it should not need
/// to use `deadpool` directly.
#[macro_export]
macro_rules! managed_reexports {
    ($crate_name:literal, $Manager:ty, $Wrapper:ty, $Error:ty, $ConfigError:ty) => {

        #[doc=concat!("Type alias for using [`deadpool::managed::Pool`] with [`", $crate_name, "`].")]
        pub type Pool = deadpool::managed::Pool<$Manager, $Wrapper>;

        #[doc=concat!("Type alias for using [`deadpool::managed::Pool`] with [`", $crate_name, "`].")]
        pub type WeakPool = deadpool::managed::WeakPool<$Manager, $Wrapper>;

        #[doc=concat!("Type alias for using [`deadpool::managed::PoolBuilder`] with [`", $crate_name, "`].")]
        pub type PoolBuilder = deadpool::managed::PoolBuilder<$Manager, $Wrapper>;

        #[doc=concat!("Type alias for using [`deadpool::managed::BuildError`] with [`", $crate_name, "`].")]
        pub type BuildError = deadpool::managed::BuildError;

        #[doc=concat!("Type alias for using [`deadpool::managed::CreatePoolError`] with [`", $crate_name, "`].")]
        pub type CreatePoolError = deadpool::managed::CreatePoolError<$ConfigError>;

        #[doc=concat!("Type alias for using [`deadpool::managed::PoolError`] with [`", $crate_name, "`].")]
        pub type PoolError = deadpool::managed::PoolError<$Error>;

        #[doc=concat!("Type alias for using [`deadpool::managed::Object`] with [`", $crate_name, "`].")]
        pub type Object = deadpool::managed::Object<$Manager>;

        #[doc=concat!("Type alias for using [`deadpool::managed::Hook`] with [`", $crate_name, "`].")]
        pub type Hook = deadpool::managed::Hook<$Manager>;

        #[doc=concat!("Type alias for using [`deadpool::managed::HookError`] with [`", $crate_name, "`].")]
        pub type HookError = deadpool::managed::HookError<$Error>;

    };
}
