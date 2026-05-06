#[cfg(all(target_os = "linux", bwrap_available))]
fn main() {
    use std::ffi::CStr;
    use std::ffi::CString;
    use std::os::raw::c_char;
    use std::os::unix::ffi::OsStrExt;

    unsafe extern "C" {
        fn bwrap_main(argc: libc::c_int, argv: *const *const c_char) -> libc::c_int;
    }

    let cstrings = std::env::args_os()
        .map(|arg| {
            CString::new(arg.as_os_str().as_bytes())
                .unwrap_or_else(|err| panic!("failed to convert argv to CString: {err}"))
        })
        .collect::<Vec<_>>();
    let mut argv_ptrs = cstrings
        .iter()
        .map(CString::as_c_str)
        .map(CStr::as_ptr)
        .collect::<Vec<*const c_char>>();
    argv_ptrs.push(std::ptr::null());

    // SAFETY: We provide a null-terminated argv vector whose pointers remain
    // valid for the duration of the call.
    let exit_code = unsafe { bwrap_main(cstrings.len() as libc::c_int, argv_ptrs.as_ptr()) };
    std::process::exit(exit_code);
}

#[cfg(all(target_os = "linux", not(bwrap_available)))]
fn main() {
    panic!(
        r#"bubblewrap is not available in this build.
Notes:
- ensure the target OS is Linux
- libcap headers must be available via pkg-config
- bubblewrap sources expected at codex-rs/vendor/bubblewrap (default)"#
    );
}

#[cfg(not(target_os = "linux"))]
fn main() {
    panic!("bwrap is only supported on Linux");
}
