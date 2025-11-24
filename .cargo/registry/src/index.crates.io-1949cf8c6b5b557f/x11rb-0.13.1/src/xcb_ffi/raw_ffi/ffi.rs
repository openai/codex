//! The code in this module allows to call libxcb's C functions.
//!
//! At its heart, this module only contains an `extern "C"` block that defines the C functions so
//! that they can be called from Rust.
//!
//! However, this module also contains the implementation of the `dl-libxcb` features. When this
//! future is enabled, we do not link against libxcb, but instead use `libloading` to load
//! `libxcb.so` at runtime. Most of the code is actually responsible for this later feature.

use super::{
    c_char, c_int, c_uint, c_void, iovec, xcb_connection_t, xcb_generic_error_t,
    xcb_generic_event_t, xcb_protocol_request_t, xcb_setup_t, xcb_void_cookie_t,
};

#[cfg(feature = "dl-libxcb")]
pub(crate) mod libxcb_library {
    use super::LibxcbFuncs;
    use crate::errors::LibxcbLoadError;

    pub(super) struct LibxcbLibrary {
        // Needed to keep the library loaded
        _library: libloading::Library,
        pub(super) funcs: LibxcbFuncs,
    }

    impl LibxcbLibrary {
        /// # Safety
        ///
        /// The functions pointers in `funcs` do not have lifetime,
        /// but they must not outlive the returned result.
        #[cold]
        #[inline(never)]
        unsafe fn load() -> Result<Self, LibxcbLoadError> {
            // TODO: Names for non-unix platforms
            #[cfg(not(unix))]
            compile_error!("dl-libxcb feature is not supported on non-unix");

            #[cfg(all(unix, target_os = "linux"))]
            const LIB_NAME: &str = "libxcb.so.1";

            // libtool turns -version-info differently into SONAMES on NetBSD.
            // Also, the library is apparently not in the default search path, hence use a full path.
            #[cfg(all(unix, target_os = "netbsd"))]
            const LIB_NAME: &str = "/usr/X11R7/lib/libxcb.so.2";

            // If we do not know anything, just assume libxcb.so and hope for the best.
            // This is actually the right thing to do on OpenBSD since the dynamic linker then does
            // some magic to find the right SONAME.
            #[cfg(all(unix, not(any(target_os = "linux", target_os = "netbsd"))))]
            const LIB_NAME: &str = "libxcb.so";

            let library = libloading::Library::new(LIB_NAME)
                .map_err(|e| LibxcbLoadError::OpenLibError(LIB_NAME.into(), e.to_string()))?;
            let funcs = LibxcbFuncs::new(&library).map_err(|(symbol, e)| {
                LibxcbLoadError::GetSymbolError(symbol.into(), e.to_string())
            })?;
            Ok(Self {
                _library: library,
                funcs,
            })
        }
    }

    use once_cell::sync::Lazy;

    static LIBXCB_LIBRARY: Lazy<Result<LibxcbLibrary, LibxcbLoadError>> =
        Lazy::new(|| unsafe { LibxcbLibrary::load() });

    pub(super) fn get_libxcb() -> &'static LibxcbLibrary {
        #[cold]
        #[inline(never)]
        fn failed(e: &LibxcbLoadError) -> ! {
            panic!("failed to load libxcb: {}", e);
        }
        match *LIBXCB_LIBRARY {
            Ok(ref library) => library,
            Err(ref e) => failed(e),
        }
    }

    /// Tries to dynamically load libxcb, returning an error on failure.
    ///
    /// It is not required to call this function, as libxcb will be lazily loaded.
    /// However, if a lazy load fails, a panic will be raised, missing the chance
    /// to (nicely) handle the error.
    ///
    /// It is safe to call this function more than once from the same or different
    /// threads. Only the first call will try to load libxcb, subsequent calls will
    /// always return the same result.
    pub fn load_libxcb() -> Result<(), LibxcbLoadError> {
        match Lazy::force(&LIBXCB_LIBRARY) {
            Ok(_) => Ok(()),
            Err(e) => Err(e.clone()),
        }
    }
}

macro_rules! make_ffi_fn_defs {
    {
        $(
            $(#[$fn_attr:meta])*
            fn $fn_name:ident($($fn_arg_name:ident: $fn_arg_type:ty),*) $(-> $fn_ret_ty:ty)?;
        )*
    } => {
        #[cfg(not(feature = "dl-libxcb"))]
        #[link(name = "xcb")]
        extern "C" {
            $(
                $(#[$fn_attr])*
                pub(crate) fn $fn_name($($fn_arg_name: $fn_arg_type),*) $(-> $fn_ret_ty)?;
            )*
        }

        #[cfg(feature = "dl-libxcb")]
        struct LibxcbFuncs {
            $(
                $(#[$fn_attr])*
                $fn_name: unsafe extern "C" fn($($fn_arg_name: $fn_arg_type),*) $(-> $fn_ret_ty)?,
            )*
        }

        #[cfg(feature = "dl-libxcb")]
        impl LibxcbFuncs {
            unsafe fn new(library: &libloading::Library) -> Result<Self, (&'static [u8], libloading::Error)> {
                Ok(Self {
                    $($fn_name: {
                        let symbol_name = concat!(stringify!($fn_name), "\0").as_bytes();
                        *library.get(symbol_name).map_err(|e| (stringify!($fn_name).as_bytes(), e))?
                    },)*
                })
            }
        }

        $(
            #[cfg(feature = "dl-libxcb")]
            $(#[$fn_attr])*
            pub(crate) unsafe fn $fn_name($($fn_arg_name: $fn_arg_type),*) $(-> $fn_ret_ty)? {
                (libxcb_library::get_libxcb().funcs.$fn_name)($($fn_arg_name),*)
            }
        )*
    };
}

make_ffi_fn_defs! {
    // From xcb.h
    fn xcb_flush(c: *mut xcb_connection_t) -> c_int;
    fn xcb_get_maximum_request_length(c: *mut xcb_connection_t) -> u32;
    fn xcb_prefetch_maximum_request_length(c: *mut xcb_connection_t);
    fn xcb_wait_for_event(c: *mut xcb_connection_t) -> *mut xcb_generic_event_t;
    fn xcb_poll_for_event(c: *mut xcb_connection_t) -> *mut xcb_generic_event_t;
    fn xcb_request_check(
        c: *mut xcb_connection_t,
        void_cookie: xcb_void_cookie_t
    ) -> *mut xcb_generic_error_t;
    fn xcb_discard_reply64(c: *mut xcb_connection_t, sequence: u64);
    fn xcb_get_setup(c: *mut xcb_connection_t) -> *const xcb_setup_t;
    #[cfg(unix)]
    fn xcb_get_file_descriptor(c: *mut xcb_connection_t) -> c_int;
    fn xcb_connection_has_error(c: *mut xcb_connection_t) -> c_int;
    fn xcb_disconnect(c: *mut xcb_connection_t);
    fn xcb_connect(
        displayname: *const c_char,
        screenp: *mut c_int
    ) -> *mut xcb_connection_t;
    fn xcb_generate_id(c: *mut xcb_connection_t) -> u32;

    // From xcbext.h
    fn xcb_send_request64(
        c: *mut xcb_connection_t,
        flags: c_int,
        vector: *mut iovec,
        request: *const xcb_protocol_request_t
    ) -> u64;
    #[cfg(unix)]
    fn xcb_send_request_with_fds64(
        c: *mut xcb_connection_t,
        flags: c_int,
        vector: *mut iovec,
        request: *const xcb_protocol_request_t,
        num_fds: c_uint,
        fds: *mut c_int
    ) -> u64;
    fn xcb_wait_for_reply64(
        c: *mut xcb_connection_t,
        request: u64,
        e: *mut *mut xcb_generic_error_t
    ) -> *mut c_void;
    fn xcb_poll_for_reply64(
        c: *mut xcb_connection_t,
        request: u64,
        reply: *mut *mut c_void,
        error: *mut *mut xcb_generic_error_t
    ) -> c_int;
}
