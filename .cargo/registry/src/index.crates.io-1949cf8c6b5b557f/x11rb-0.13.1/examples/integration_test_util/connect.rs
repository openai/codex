/// Establish a new connection to an X11 server.
///
/// Returns a `XCBConnection` if `allow-unsafe-code`, otherwise returns a `RustConnection`.
/// This function is meant to test code with both connection types. Production code
/// usually wants to use `x11rb::connect`, `x11rb::rust_connection::RustConnection::connect`
/// or `x11rb::xcb_ffi::XCBConnection::connect`.
pub fn connect(
    dpy_name: Option<&str>,
) -> Result<(impl x11rb::connection::Connection + Send + Sync, usize), x11rb::errors::ConnectError>
{
    #[cfg(feature = "allow-unsafe-code")]
    {
        let dpy_name = dpy_name
            .map(std::ffi::CString::new)
            .transpose()
            .map_err(|_| x11rb::errors::DisplayParsingError::Unknown)?;
        let dpy_name = dpy_name.as_deref();
        x11rb::xcb_ffi::XCBConnection::connect(dpy_name)
    }
    #[cfg(not(feature = "allow-unsafe-code"))]
    {
        x11rb::rust_connection::RustConnection::connect(dpy_name)
    }
}
