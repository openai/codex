use crate::error::ErrorStack;
use crate::lib_ctx::LibCtxRef;
use crate::{cvt, cvt_p};
use foreign_types::{ForeignType, ForeignTypeRef};
use openssl_macros::corresponds;
use std::ffi::CString;
use std::ptr;

foreign_type_and_impl_send_sync! {
    type CType = ffi::OSSL_PROVIDER;
    fn drop = ossl_provider_free;

    pub struct Provider;
    /// A reference to a [`Provider`].
    pub struct ProviderRef;
}

#[inline]
unsafe fn ossl_provider_free(p: *mut ffi::OSSL_PROVIDER) {
    ffi::OSSL_PROVIDER_unload(p);
}

impl Provider {
    /// Loads a new provider into the specified library context, disabling the fallback providers.
    ///
    /// If `ctx` is `None`, the provider will be loaded in to the default library context.
    #[corresponds(OSSL_provider_load)]
    pub fn load(ctx: Option<&LibCtxRef>, name: &str) -> Result<Self, ErrorStack> {
        let name = CString::new(name).unwrap();
        unsafe {
            let p = cvt_p(ffi::OSSL_PROVIDER_load(
                ctx.map_or(ptr::null_mut(), ForeignTypeRef::as_ptr),
                name.as_ptr(),
            ))?;

            Ok(Provider::from_ptr(p))
        }
    }

    /// Loads a new provider into the specified library context, disabling the fallback providers if `retain_fallbacks`
    /// is `false` and the load succeeds.
    ///
    /// If `ctx` is `None`, the provider will be loaded into the default library context.
    #[corresponds(OSSL_provider_try_load)]
    pub fn try_load(
        ctx: Option<&LibCtxRef>,
        name: &str,
        retain_fallbacks: bool,
    ) -> Result<Self, ErrorStack> {
        let name = CString::new(name).unwrap();
        unsafe {
            let p = cvt_p(ffi::OSSL_PROVIDER_try_load(
                ctx.map_or(ptr::null_mut(), ForeignTypeRef::as_ptr),
                name.as_ptr(),
                retain_fallbacks as _,
            ))?;

            // OSSL_PROVIDER_try_load seems to leave errors on the stack, even
            // when it succeeds.
            let _ = ErrorStack::get();

            Ok(Provider::from_ptr(p))
        }
    }

    /// Specifies the default search path that is to be used for looking for providers in the specified library context.
    /// If left unspecified, an environment variable and a fall back default value will be used instead
    ///
    /// If `ctx` is `None`, the provider will be loaded into the default library context.
    #[corresponds(OSSL_PROVIDER_set_default_search_path)]
    pub fn set_default_search_path(ctx: Option<&LibCtxRef>, path: &str) -> Result<(), ErrorStack> {
        let path = CString::new(path).unwrap();
        unsafe {
            cvt(ffi::OSSL_PROVIDER_set_default_search_path(
                ctx.map_or(ptr::null_mut(), ForeignTypeRef::as_ptr),
                path.as_ptr(),
            ))
            .map(|_| ())
        }
    }
}
