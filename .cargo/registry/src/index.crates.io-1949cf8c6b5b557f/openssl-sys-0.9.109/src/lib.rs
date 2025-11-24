#![allow(
    clippy::missing_safety_doc,
    dead_code,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_imports
)]
#![doc(html_root_url = "https://docs.rs/openssl-sys/0.9")]
#![recursion_limit = "128"] // configure fixed limit across all rust versions

extern crate libc;
pub use libc::c_int;

#[cfg(feature = "unstable_boringssl")]
extern crate bssl_sys;

#[cfg(boringssl)]
#[path = "."]
mod boringssl {
    #[cfg(feature = "unstable_boringssl")]
    pub use bssl_sys::*;
    #[cfg(not(feature = "unstable_boringssl"))]
    include!(concat!(env!("OUT_DIR"), "/bindgen.rs"));

    // BoringSSL does not require initialization.
    pub fn init() {}
}
#[cfg(boringssl)]
pub use boringssl::*;

#[cfg(feature = "aws-lc")]
extern crate aws_lc_sys;

#[cfg(awslc)]
#[path = "."]
mod aws_lc {
    #[cfg(feature = "aws-lc")]
    pub use aws_lc_sys::*;

    #[cfg(not(feature = "aws-lc"))]
    include!(concat!(env!("OUT_DIR"), "/bindgen.rs"));

    use libc::{c_char, c_long, c_void};

    pub fn init() {
        unsafe { CRYPTO_library_init() }
    }

    // BIO_get_mem_data is a C preprocessor macro by definition
    #[allow(non_snake_case, clippy::not_unsafe_ptr_arg_deref)]
    pub fn BIO_get_mem_data(b: *mut BIO, pp: *mut *mut c_char) -> c_long {
        unsafe { BIO_ctrl(b, BIO_CTRL_INFO, 0, pp.cast::<c_void>()) }
    }
}
#[cfg(awslc)]
pub use aws_lc::*;

#[cfg(openssl)]
#[path = "."]
mod openssl {
    use libc::*;

    #[cfg(feature = "bindgen")]
    include!(concat!(env!("OUT_DIR"), "/bindgen.rs"));

    pub use self::aes::*;
    pub use self::asn1::*;
    pub use self::bio::*;
    pub use self::bn::*;
    pub use self::cms::*;
    pub use self::crypto::*;
    pub use self::dtls1::*;
    pub use self::ec::*;
    pub use self::err::*;
    pub use self::evp::*;
    #[cfg(not(feature = "bindgen"))]
    pub use self::handwritten::*;
    pub use self::obj_mac::*;
    pub use self::ocsp::*;
    pub use self::pem::*;
    pub use self::pkcs7::*;
    pub use self::rsa::*;
    pub use self::sha::*;
    pub use self::srtp::*;
    pub use self::ssl::*;
    pub use self::ssl3::*;
    pub use self::tls1::*;
    pub use self::types::*;
    pub use self::x509::*;
    pub use self::x509_vfy::*;
    pub use self::x509v3::*;

    #[macro_use]
    mod macros;

    mod aes;
    mod asn1;
    mod bio;
    mod bn;
    mod cms;
    mod crypto;
    mod dtls1;
    mod ec;
    mod err;
    mod evp;
    #[cfg(not(feature = "bindgen"))]
    mod handwritten;
    mod obj_mac;
    mod ocsp;
    mod pem;
    mod pkcs7;
    mod rsa;
    mod sha;
    mod srtp;
    mod ssl;
    mod ssl3;
    mod tls1;
    mod types;
    mod x509;
    mod x509_vfy;
    mod x509v3;

    use std::sync::Once;
    // explicitly initialize to work around https://github.com/openssl/openssl/issues/3505
    static INIT: Once = Once::new();

    // FIXME remove
    pub type PasswordCallback = unsafe extern "C" fn(
        buf: *mut c_char,
        size: c_int,
        rwflag: c_int,
        user_data: *mut c_void,
    ) -> c_int;

    #[cfg(ossl110)]
    pub fn init() {
        use std::ptr;

        #[cfg(not(ossl111b))]
        let init_options = OPENSSL_INIT_LOAD_SSL_STRINGS;
        #[cfg(ossl111b)]
        let init_options = OPENSSL_INIT_LOAD_SSL_STRINGS | OPENSSL_INIT_NO_ATEXIT;

        INIT.call_once(|| unsafe {
            OPENSSL_init_ssl(init_options, ptr::null_mut());
        })
    }

    #[cfg(not(ossl110))]
    pub fn init() {
        use std::io::{self, Write};
        use std::mem;
        use std::process;
        use std::sync::{Mutex, MutexGuard};

        static mut MUTEXES: *mut Vec<Mutex<()>> = 0 as *mut Vec<Mutex<()>>;
        static mut GUARDS: *mut Vec<Option<MutexGuard<'static, ()>>> =
            0 as *mut Vec<Option<MutexGuard<'static, ()>>>;

        unsafe extern "C" fn locking_function(
            mode: c_int,
            n: c_int,
            _file: *const c_char,
            _line: c_int,
        ) {
            let mutex = &(*MUTEXES)[n as usize];

            if mode & CRYPTO_LOCK != 0 {
                (*GUARDS)[n as usize] = Some(mutex.lock().unwrap());
            } else {
                if let None = (*GUARDS)[n as usize].take() {
                    let _ = writeln!(
                        io::stderr(),
                        "BUG: rust-openssl lock {} already unlocked, aborting",
                        n
                    );
                    process::abort();
                }
            }
        }

        cfg_if! {
            if #[cfg(unix)] {
                fn set_id_callback() {
                    unsafe extern "C" fn thread_id() -> c_ulong {
                        ::libc::pthread_self() as c_ulong
                    }

                    unsafe {
                        CRYPTO_set_id_callback__fixed_rust(Some(thread_id));
                    }
                }
            } else {
                fn set_id_callback() {}
            }
        }

        INIT.call_once(|| unsafe {
            SSL_library_init();
            SSL_load_error_strings();
            OPENSSL_add_all_algorithms_noconf();

            let num_locks = CRYPTO_num_locks();
            let mut mutexes = Box::new(Vec::new());
            for _ in 0..num_locks {
                mutexes.push(Mutex::new(()));
            }
            MUTEXES = mem::transmute(mutexes);
            let guards: Box<Vec<Option<MutexGuard<()>>>> =
                Box::new((0..num_locks).map(|_| None).collect());
            GUARDS = mem::transmute(guards);

            CRYPTO_set_locking_callback__fixed_rust(Some(locking_function));
            set_id_callback();
        })
    }

    /// Disable explicit initialization of the openssl libs.
    ///
    /// This is only appropriate to use if the openssl crate is being consumed by an application
    /// that will be performing the initialization explicitly.
    ///
    /// # Safety
    ///
    /// In some versions of openssl, skipping initialization will fall back to the default procedure
    /// while other will cause difficult to debug errors so care must be taken when calling this.
    pub unsafe fn assume_init() {
        INIT.call_once(|| {});
    }
}
#[cfg(openssl)]
pub use openssl::*;
