pub fn get_timezone_inner() -> Result<String, crate::GetTimezoneError> {
    Err(crate::GetTimezoneError::OsError)
}

#[cfg(not(feature = "fallback"))]
compile_error!(
    "iana-time-zone is currently implemented for Linux, Window, MacOS, FreeBSD, NetBSD, \
    OpenBSD, Dragonfly, WebAssembly (browser), iOS, Illumos, Android, AIX, Solaris and Haiku.",
);
