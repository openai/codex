use nix::syslog::{openlog, syslog, Facility, LogFlags, Severity};

#[test]
fn test_syslog_hello_world() {
    let flags = LogFlags::LOG_PID;

    #[cfg(not(target_os = "linux"))]
    openlog(None::<&str>, flags, Facility::LOG_USER).unwrap();
    #[cfg(target_os = "linux")]
    openlog(None, flags, Facility::LOG_USER).unwrap();

    syslog(Severity::LOG_EMERG, "Hello, nix!").unwrap();
    let name = "syslog";
    syslog(Severity::LOG_NOTICE, &format!("Hello, {name}!")).unwrap();
}

#[test]
#[cfg(target_os = "linux")]
fn test_openlog_with_ident() {
    use std::ffi::CStr;

    const IDENT: &CStr = unsafe {
        CStr::from_bytes_with_nul_unchecked(b"test_openlog_with_ident\0")
    };

    let flags = LogFlags::LOG_PID;
    openlog(Some(IDENT), flags, Facility::LOG_USER).unwrap();
    syslog(Severity::LOG_EMERG, "Hello, ident!").unwrap();
}

#[test]
#[cfg(not(target_os = "linux"))]
fn test_openlog_with_ident() {
    let flags = LogFlags::LOG_PID;
    openlog(Some("test_openlog_with_ident"), flags, Facility::LOG_USER)
        .unwrap();
    syslog(Severity::LOG_EMERG, "Hello, ident!").unwrap();
}
