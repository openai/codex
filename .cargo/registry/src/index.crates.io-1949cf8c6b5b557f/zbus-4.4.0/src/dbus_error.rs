use crate::{
    message::{Header, Message},
    names::ErrorName,
    Result,
};

/// A trait that needs to be implemented by error types to be returned from D-Bus methods.
///
/// Typically, you'd use the [`crate::fdo::Error`] since that covers quite a lot of failures but
/// occasionally you might find yourself needing to use a custom error type. You'll need to
/// implement this trait for your error type. The easiest way to achieve that is to make use of the
/// [`DBusError` macro][dm].
///
/// [dm]: derive.DBusError.html
pub trait DBusError {
    /// Generate an error reply message for the given method call.
    fn create_reply(&self, msg: &Header<'_>) -> Result<Message>;

    // The name of the error.
    //
    // Every D-Bus error must have a name. See [`ErrorName`] for more information.
    fn name(&self) -> ErrorName<'_>;

    // The optional description for the error.
    fn description(&self) -> Option<&str>;
}
