//! Trait aliases for Services that produce specific types of Responses.

mod make_connection;
mod make_service;

pub use self::make_connection::MakeConnection;
pub use self::make_service::shared::Shared;
pub use self::make_service::{AsService, IntoService, MakeService};

pub mod future {
    //! Future types

    pub use super::make_service::shared::SharedFuture;
}
