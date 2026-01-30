//! Build-time bubblewrap entrypoint.
//!
//! This module is intentionally behind a build-time opt-in. When enabled, the
//! build script compiles bubblewrap's C sources and exposes a `bwrap_main`
//! symbol that we can call via FFI.

#[cfg(vendored_bwrap_available)]
mod imp {
    use std::ffi::CString;
    use std::os::raw::c_char;

    unsafe extern "C" {
        fn bwrap_main(argc: libc::c_int, argv: *const *const c_char) -> libc::c_int;
    }

    /// Execute the build-time bubblewrap `main` function with the given argv.
    pub(crate) fn exec_vendored_bwrap(argv: Vec<String>) -> ! {
        let mut cstrings: Vec<CString> = Vec::with_capacity(argv.len());
        for arg in &argv {
            match CString::new(arg.as_str()) {
                Ok(value) => cstrings.push(value),
                Err(err) => panic!("failed to convert argv to CString: {err}"),
            }
        }

        let mut argv_ptrs: Vec<*const c_char> = cstrings.iter().map(|arg| arg.as_ptr()).collect();
        argv_ptrs.push(std::ptr::null());

        // SAFETY: We provide a null-terminated argv vector whose pointers
        // remain valid for the duration of the call.
        let exit_code = unsafe { bwrap_main(cstrings.len() as libc::c_int, argv_ptrs.as_ptr()) };
        std::process::exit(exit_code);
    }
}

#[cfg(not(vendored_bwrap_available))]
mod imp {
    /// Panics with a clear error when the build-time bwrap path is not enabled.
    pub(crate) fn exec_vendored_bwrap(_argv: Vec<String>) -> ! {
        panic!(
            "build-time bubblewrap is not available in this build.\n\
Rebuild codex-linux-sandbox on Linux with CODEX_BWRAP_ENABLE_FFI=1.\n\
Example:\n\
- cd codex-rs && CODEX_BWRAP_ENABLE_FFI=1 cargo build -p codex-linux-sandbox\n\
If this crate was already built without it, run:\n\
- cargo clean -p codex-linux-sandbox\n\
Notes:\n\
- libcap headers must be available via pkg-config\n\
- bubblewrap sources expected at codex-rs/vendor/bubblewrap (default)"
        );
    }
}

pub(crate) use imp::exec_vendored_bwrap;
