use crate::cvt_p;
use crate::error::ErrorStack;
use foreign_types::ForeignType;
use openssl_macros::corresponds;

foreign_type_and_impl_send_sync! {
    type CType = ffi::OSSL_LIB_CTX;
    fn drop = ffi::OSSL_LIB_CTX_free;

    pub struct LibCtx;
    pub struct LibCtxRef;
}

impl LibCtx {
    #[corresponds(OSSL_LIB_CTX_new)]
    pub fn new() -> Result<Self, ErrorStack> {
        unsafe {
            let ptr = cvt_p(ffi::OSSL_LIB_CTX_new())?;
            Ok(LibCtx::from_ptr(ptr))
        }
    }
}
