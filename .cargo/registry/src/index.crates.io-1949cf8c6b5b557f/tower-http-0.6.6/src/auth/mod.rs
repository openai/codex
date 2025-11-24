//! Authorization related middleware.

pub mod add_authorization;
pub mod async_require_authorization;
pub mod require_authorization;

#[doc(inline)]
pub use self::{
    add_authorization::{AddAuthorization, AddAuthorizationLayer},
    async_require_authorization::{
        AsyncAuthorizeRequest, AsyncRequireAuthorization, AsyncRequireAuthorizationLayer,
    },
};
