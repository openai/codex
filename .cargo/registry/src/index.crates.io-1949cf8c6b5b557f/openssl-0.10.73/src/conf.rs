//! Interface for processing OpenSSL configuration files.

foreign_type_and_impl_send_sync! {
    type CType = ffi::CONF;
    fn drop = ffi::NCONF_free;

    pub struct Conf;
    pub struct ConfRef;
}

#[cfg(not(any(boringssl, libressl400, awslc)))]
mod methods {
    use super::Conf;
    use crate::cvt_p;
    use crate::error::ErrorStack;
    use openssl_macros::corresponds;

    pub struct ConfMethod(*mut ffi::CONF_METHOD);

    impl ConfMethod {
        /// Retrieve handle to the default OpenSSL configuration file processing function.
        #[corresponds(NCONF_default)]
        #[allow(clippy::should_implement_trait)]
        pub fn default() -> ConfMethod {
            unsafe {
                ffi::init();
                // `NCONF` stands for "New Conf", as described in crypto/conf/conf_lib.c. This is
                // a newer API than the "CONF classic" functions.
                ConfMethod(ffi::NCONF_default())
            }
        }

        /// Construct from raw pointer.
        ///
        /// # Safety
        ///
        /// The caller must ensure that the pointer is valid.
        pub unsafe fn from_ptr(ptr: *mut ffi::CONF_METHOD) -> ConfMethod {
            ConfMethod(ptr)
        }

        /// Convert to raw pointer.
        pub fn as_ptr(&self) -> *mut ffi::CONF_METHOD {
            self.0
        }
    }

    impl Conf {
        /// Create a configuration parser.
        ///
        /// # Examples
        ///
        /// ```
        /// use openssl::conf::{Conf, ConfMethod};
        ///
        /// let conf = Conf::new(ConfMethod::default());
        /// ```
        #[corresponds(NCONF_new)]
        pub fn new(method: ConfMethod) -> Result<Conf, ErrorStack> {
            unsafe { cvt_p(ffi::NCONF_new(method.as_ptr())).map(Conf) }
        }
    }
}
#[cfg(not(any(boringssl, libressl400, awslc)))]
pub use methods::*;
