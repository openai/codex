use cfg_if::cfg_if;
use foreign_types::ForeignType;
use foreign_types::ForeignTypeRef;
#[cfg(any(ossl111, not(osslconf = "OPENSSL_NO_PSK")))]
use libc::c_char;
#[cfg(ossl111)]
use libc::size_t;
use libc::{c_int, c_uchar, c_uint, c_void};
#[cfg(any(ossl111, not(osslconf = "OPENSSL_NO_PSK")))]
use std::ffi::CStr;
use std::mem;
use std::ptr;
#[cfg(any(ossl111, boringssl, awslc))]
use std::str;
use std::sync::Arc;

use crate::dh::Dh;
#[cfg(all(ossl101, not(ossl110)))]
use crate::ec::EcKey;
use crate::error::ErrorStack;
use crate::pkey::Params;
#[cfg(any(ossl102, libressl261, boringssl, awslc))]
use crate::ssl::AlpnError;
use crate::ssl::{
    try_get_session_ctx_index, SniError, Ssl, SslAlert, SslContext, SslContextRef, SslRef,
    SslSession, SslSessionRef,
};
#[cfg(ossl111)]
use crate::ssl::{ClientHelloResponse, ExtensionContext};
use crate::util;
#[cfg(any(ossl111, boringssl, awslc))]
use crate::util::ForeignTypeRefExt;
#[cfg(ossl111)]
use crate::x509::X509Ref;
use crate::x509::{X509StoreContext, X509StoreContextRef};

pub extern "C" fn raw_verify<F>(preverify_ok: c_int, x509_ctx: *mut ffi::X509_STORE_CTX) -> c_int
where
    F: Fn(bool, &mut X509StoreContextRef) -> bool + 'static + Sync + Send,
{
    unsafe {
        let ctx = X509StoreContextRef::from_ptr_mut(x509_ctx);
        let ssl_idx = X509StoreContext::ssl_idx().expect("BUG: store context ssl index missing");
        let verify_idx = SslContext::cached_ex_index::<F>();

        // raw pointer shenanigans to break the borrow of ctx
        // the callback can't mess with its own ex_data slot so this is safe
        let verify = ctx
            .ex_data(ssl_idx)
            .expect("BUG: store context missing ssl")
            .ssl_context()
            .ex_data(verify_idx)
            .expect("BUG: verify callback missing") as *const F;

        (*verify)(preverify_ok != 0, ctx) as c_int
    }
}

#[cfg(not(osslconf = "OPENSSL_NO_PSK"))]
pub extern "C" fn raw_client_psk<F>(
    ssl: *mut ffi::SSL,
    hint: *const c_char,
    identity: *mut c_char,
    max_identity_len: c_uint,
    psk: *mut c_uchar,
    max_psk_len: c_uint,
) -> c_uint
where
    F: Fn(&mut SslRef, Option<&[u8]>, &mut [u8], &mut [u8]) -> Result<usize, ErrorStack>
        + 'static
        + Sync
        + Send,
{
    unsafe {
        let ssl = SslRef::from_ptr_mut(ssl);
        let callback_idx = SslContext::cached_ex_index::<F>();

        let callback = ssl
            .ssl_context()
            .ex_data(callback_idx)
            .expect("BUG: psk callback missing") as *const F;
        let hint = if !hint.is_null() {
            Some(CStr::from_ptr(hint).to_bytes())
        } else {
            None
        };
        // Give the callback mutable slices into which it can write the identity and psk.
        let identity_sl = util::from_raw_parts_mut(identity as *mut u8, max_identity_len as usize);
        #[allow(clippy::unnecessary_cast)]
        let psk_sl = util::from_raw_parts_mut(psk as *mut u8, max_psk_len as usize);
        match (*callback)(ssl, hint, identity_sl, psk_sl) {
            Ok(psk_len) => psk_len as u32,
            Err(e) => {
                e.put();
                0
            }
        }
    }
}

#[cfg(not(osslconf = "OPENSSL_NO_PSK"))]
pub extern "C" fn raw_server_psk<F>(
    ssl: *mut ffi::SSL,
    identity: *const c_char,
    psk: *mut c_uchar,
    max_psk_len: c_uint,
) -> c_uint
where
    F: Fn(&mut SslRef, Option<&[u8]>, &mut [u8]) -> Result<usize, ErrorStack>
        + 'static
        + Sync
        + Send,
{
    unsafe {
        let ssl = SslRef::from_ptr_mut(ssl);
        let callback_idx = SslContext::cached_ex_index::<F>();

        let callback = ssl
            .ssl_context()
            .ex_data(callback_idx)
            .expect("BUG: psk callback missing") as *const F;
        let identity = if identity.is_null() {
            None
        } else {
            Some(CStr::from_ptr(identity).to_bytes())
        };
        // Give the callback mutable slices into which it can write the psk.
        #[allow(clippy::unnecessary_cast)]
        let psk_sl = util::from_raw_parts_mut(psk as *mut u8, max_psk_len as usize);
        match (*callback)(ssl, identity, psk_sl) {
            Ok(psk_len) => psk_len as u32,
            Err(e) => {
                e.put();
                0
            }
        }
    }
}

pub extern "C" fn ssl_raw_verify<F>(
    preverify_ok: c_int,
    x509_ctx: *mut ffi::X509_STORE_CTX,
) -> c_int
where
    F: Fn(bool, &mut X509StoreContextRef) -> bool + 'static + Sync + Send,
{
    unsafe {
        let ctx = X509StoreContextRef::from_ptr_mut(x509_ctx);
        let ssl_idx = X509StoreContext::ssl_idx().expect("BUG: store context ssl index missing");
        let callback_idx = Ssl::cached_ex_index::<Arc<F>>();

        let callback = ctx
            .ex_data(ssl_idx)
            .expect("BUG: store context missing ssl")
            .ex_data(callback_idx)
            .expect("BUG: ssl verify callback missing")
            .clone();

        callback(preverify_ok != 0, ctx) as c_int
    }
}

pub extern "C" fn raw_sni<F>(ssl: *mut ffi::SSL, al: *mut c_int, arg: *mut c_void) -> c_int
where
    F: Fn(&mut SslRef, &mut SslAlert) -> Result<(), SniError> + 'static + Sync + Send,
{
    unsafe {
        let ssl = SslRef::from_ptr_mut(ssl);
        let callback = arg as *const F;
        let mut alert = SslAlert(*al);

        let r = (*callback)(ssl, &mut alert);
        *al = alert.0;
        match r {
            Ok(()) => ffi::SSL_TLSEXT_ERR_OK,
            Err(e) => e.0,
        }
    }
}

#[cfg(any(ossl102, libressl261, boringssl, awslc))]
pub extern "C" fn raw_alpn_select<F>(
    ssl: *mut ffi::SSL,
    out: *mut *const c_uchar,
    outlen: *mut c_uchar,
    inbuf: *const c_uchar,
    inlen: c_uint,
    _arg: *mut c_void,
) -> c_int
where
    F: for<'a> Fn(&mut SslRef, &'a [u8]) -> Result<&'a [u8], AlpnError> + 'static + Sync + Send,
{
    unsafe {
        let ssl = SslRef::from_ptr_mut(ssl);
        let callback = ssl
            .ssl_context()
            .ex_data(SslContext::cached_ex_index::<F>())
            .expect("BUG: alpn callback missing") as *const F;
        #[allow(clippy::unnecessary_cast)]
        let protos = util::from_raw_parts(inbuf as *const u8, inlen as usize);

        match (*callback)(ssl, protos) {
            Ok(proto) => {
                *out = proto.as_ptr() as *const c_uchar;
                *outlen = proto.len() as c_uchar;
                ffi::SSL_TLSEXT_ERR_OK
            }
            Err(e) => e.0,
        }
    }
}

pub unsafe extern "C" fn raw_tmp_dh<F>(
    ssl: *mut ffi::SSL,
    is_export: c_int,
    keylength: c_int,
) -> *mut ffi::DH
where
    F: Fn(&mut SslRef, bool, u32) -> Result<Dh<Params>, ErrorStack> + 'static + Sync + Send,
{
    let ssl = SslRef::from_ptr_mut(ssl);
    let callback = ssl
        .ssl_context()
        .ex_data(SslContext::cached_ex_index::<F>())
        .expect("BUG: tmp dh callback missing") as *const F;

    match (*callback)(ssl, is_export != 0, keylength as u32) {
        Ok(dh) => {
            let ptr = dh.as_ptr();
            mem::forget(dh);
            ptr
        }
        Err(e) => {
            e.put();
            ptr::null_mut()
        }
    }
}

#[cfg(all(ossl101, not(ossl110)))]
pub unsafe extern "C" fn raw_tmp_ecdh<F>(
    ssl: *mut ffi::SSL,
    is_export: c_int,
    keylength: c_int,
) -> *mut ffi::EC_KEY
where
    F: Fn(&mut SslRef, bool, u32) -> Result<EcKey<Params>, ErrorStack> + 'static + Sync + Send,
{
    let ssl = SslRef::from_ptr_mut(ssl);
    let callback = ssl
        .ssl_context()
        .ex_data(SslContext::cached_ex_index::<F>())
        .expect("BUG: tmp ecdh callback missing") as *const F;

    match (*callback)(ssl, is_export != 0, keylength as u32) {
        Ok(ec_key) => {
            let ptr = ec_key.as_ptr();
            mem::forget(ec_key);
            ptr
        }
        Err(e) => {
            e.put();
            ptr::null_mut()
        }
    }
}

pub unsafe extern "C" fn raw_tmp_dh_ssl<F>(
    ssl: *mut ffi::SSL,
    is_export: c_int,
    keylength: c_int,
) -> *mut ffi::DH
where
    F: Fn(&mut SslRef, bool, u32) -> Result<Dh<Params>, ErrorStack> + 'static + Sync + Send,
{
    let ssl = SslRef::from_ptr_mut(ssl);
    let callback = ssl
        .ex_data(Ssl::cached_ex_index::<Arc<F>>())
        .expect("BUG: ssl tmp dh callback missing")
        .clone();

    match callback(ssl, is_export != 0, keylength as u32) {
        Ok(dh) => {
            let ptr = dh.as_ptr();
            mem::forget(dh);
            ptr
        }
        Err(e) => {
            e.put();
            ptr::null_mut()
        }
    }
}

#[cfg(all(ossl101, not(ossl110)))]
pub unsafe extern "C" fn raw_tmp_ecdh_ssl<F>(
    ssl: *mut ffi::SSL,
    is_export: c_int,
    keylength: c_int,
) -> *mut ffi::EC_KEY
where
    F: Fn(&mut SslRef, bool, u32) -> Result<EcKey<Params>, ErrorStack> + 'static + Sync + Send,
{
    let ssl = SslRef::from_ptr_mut(ssl);
    let callback = ssl
        .ex_data(Ssl::cached_ex_index::<Arc<F>>())
        .expect("BUG: ssl tmp ecdh callback missing")
        .clone();

    match callback(ssl, is_export != 0, keylength as u32) {
        Ok(ec_key) => {
            let ptr = ec_key.as_ptr();
            mem::forget(ec_key);
            ptr
        }
        Err(e) => {
            e.put();
            ptr::null_mut()
        }
    }
}

pub unsafe extern "C" fn raw_tlsext_status<F>(ssl: *mut ffi::SSL, _: *mut c_void) -> c_int
where
    F: Fn(&mut SslRef) -> Result<bool, ErrorStack> + 'static + Sync + Send,
{
    let ssl = SslRef::from_ptr_mut(ssl);
    let callback = ssl
        .ssl_context()
        .ex_data(SslContext::cached_ex_index::<F>())
        .expect("BUG: ocsp callback missing") as *const F;
    let ret = (*callback)(ssl);

    if ssl.is_server() {
        match ret {
            Ok(true) => ffi::SSL_TLSEXT_ERR_OK,
            Ok(false) => ffi::SSL_TLSEXT_ERR_NOACK,
            Err(e) => {
                e.put();
                ffi::SSL_TLSEXT_ERR_ALERT_FATAL
            }
        }
    } else {
        match ret {
            Ok(true) => 1,
            Ok(false) => 0,
            Err(e) => {
                e.put();
                -1
            }
        }
    }
}

pub unsafe extern "C" fn raw_new_session<F>(
    ssl: *mut ffi::SSL,
    session: *mut ffi::SSL_SESSION,
) -> c_int
where
    F: Fn(&mut SslRef, SslSession) + 'static + Sync + Send,
{
    let session_ctx_index =
        try_get_session_ctx_index().expect("BUG: session context index initialization failed");
    let ssl = SslRef::from_ptr_mut(ssl);
    let callback = ssl
        .ex_data(*session_ctx_index)
        .expect("BUG: session context missing")
        .ex_data(SslContext::cached_ex_index::<F>())
        .expect("BUG: new session callback missing") as *const F;
    let session = SslSession::from_ptr(session);

    (*callback)(ssl, session);

    // the return code doesn't indicate error vs success, but whether or not we consumed the session
    1
}

pub unsafe extern "C" fn raw_remove_session<F>(
    ctx: *mut ffi::SSL_CTX,
    session: *mut ffi::SSL_SESSION,
) where
    F: Fn(&SslContextRef, &SslSessionRef) + 'static + Sync + Send,
{
    let ctx = SslContextRef::from_ptr(ctx);
    let callback = ctx
        .ex_data(SslContext::cached_ex_index::<F>())
        .expect("BUG: remove session callback missing");
    let session = SslSessionRef::from_ptr(session);

    callback(ctx, session)
}

cfg_if! {
    if #[cfg(any(ossl110, libressl280, boringssl, awslc))] {
        type DataPtr = *const c_uchar;
    } else {
        type DataPtr = *mut c_uchar;
    }
}

pub unsafe extern "C" fn raw_get_session<F>(
    ssl: *mut ffi::SSL,
    data: DataPtr,
    len: c_int,
    copy: *mut c_int,
) -> *mut ffi::SSL_SESSION
where
    F: Fn(&mut SslRef, &[u8]) -> Option<SslSession> + 'static + Sync + Send,
{
    let session_ctx_index =
        try_get_session_ctx_index().expect("BUG: session context index initialization failed");
    let ssl = SslRef::from_ptr_mut(ssl);
    let callback = ssl
        .ex_data(*session_ctx_index)
        .expect("BUG: session context missing")
        .ex_data(SslContext::cached_ex_index::<F>())
        .expect("BUG: get session callback missing") as *const F;
    #[allow(clippy::unnecessary_cast)]
    let data = util::from_raw_parts(data as *const u8, len as usize);

    match (*callback)(ssl, data) {
        Some(session) => {
            let p = session.as_ptr();
            mem::forget(session);
            *copy = 0;
            p
        }
        None => ptr::null_mut(),
    }
}

#[cfg(any(ossl111, boringssl, awslc))]
pub unsafe extern "C" fn raw_keylog<F>(ssl: *const ffi::SSL, line: *const c_char)
where
    F: Fn(&SslRef, &str) + 'static + Sync + Send,
{
    let ssl = SslRef::from_const_ptr(ssl);
    let callback = ssl
        .ssl_context()
        .ex_data(SslContext::cached_ex_index::<F>())
        .expect("BUG: get session callback missing");
    let line = CStr::from_ptr(line).to_bytes();
    let line = str::from_utf8_unchecked(line);

    callback(ssl, line);
}

#[cfg(ossl111)]
pub unsafe extern "C" fn raw_stateless_cookie_generate<F>(
    ssl: *mut ffi::SSL,
    cookie: *mut c_uchar,
    cookie_len: *mut size_t,
) -> c_int
where
    F: Fn(&mut SslRef, &mut [u8]) -> Result<usize, ErrorStack> + 'static + Sync + Send,
{
    let ssl = SslRef::from_ptr_mut(ssl);
    let callback = ssl
        .ssl_context()
        .ex_data(SslContext::cached_ex_index::<F>())
        .expect("BUG: stateless cookie generate callback missing") as *const F;
    #[allow(clippy::unnecessary_cast)]
    let slice = util::from_raw_parts_mut(cookie as *mut u8, ffi::SSL_COOKIE_LENGTH as usize);
    match (*callback)(ssl, slice) {
        Ok(len) => {
            *cookie_len = len as size_t;
            1
        }
        Err(e) => {
            e.put();
            0
        }
    }
}

#[cfg(ossl111)]
pub unsafe extern "C" fn raw_stateless_cookie_verify<F>(
    ssl: *mut ffi::SSL,
    cookie: *const c_uchar,
    cookie_len: size_t,
) -> c_int
where
    F: Fn(&mut SslRef, &[u8]) -> bool + 'static + Sync + Send,
{
    let ssl = SslRef::from_ptr_mut(ssl);
    let callback = ssl
        .ssl_context()
        .ex_data(SslContext::cached_ex_index::<F>())
        .expect("BUG: stateless cookie verify callback missing") as *const F;
    #[allow(clippy::unnecessary_cast)]
    let slice = util::from_raw_parts(cookie as *const c_uchar as *const u8, cookie_len);
    (*callback)(ssl, slice) as c_int
}

#[cfg(not(any(boringssl, awslc)))]
pub extern "C" fn raw_cookie_generate<F>(
    ssl: *mut ffi::SSL,
    cookie: *mut c_uchar,
    cookie_len: *mut c_uint,
) -> c_int
where
    F: Fn(&mut SslRef, &mut [u8]) -> Result<usize, ErrorStack> + 'static + Sync + Send,
{
    unsafe {
        let ssl = SslRef::from_ptr_mut(ssl);
        let callback = ssl
            .ssl_context()
            .ex_data(SslContext::cached_ex_index::<F>())
            .expect("BUG: cookie generate callback missing") as *const F;
        // We subtract 1 from DTLS1_COOKIE_LENGTH as the ostensible value, 256, is erroneous but retained for
        // compatibility. See comments in dtls1.h.
        #[allow(clippy::unnecessary_cast)]
        let slice =
            util::from_raw_parts_mut(cookie as *mut u8, ffi::DTLS1_COOKIE_LENGTH as usize - 1);
        match (*callback)(ssl, slice) {
            Ok(len) => {
                *cookie_len = len as c_uint;
                1
            }
            Err(e) => {
                e.put();
                0
            }
        }
    }
}

#[cfg(not(any(boringssl, awslc)))]
cfg_if! {
    if #[cfg(any(ossl110, libressl280))] {
        type CookiePtr = *const c_uchar;
    } else {
        type CookiePtr = *mut c_uchar;
    }
}

#[cfg(not(any(boringssl, awslc)))]
pub extern "C" fn raw_cookie_verify<F>(
    ssl: *mut ffi::SSL,
    cookie: CookiePtr,
    cookie_len: c_uint,
) -> c_int
where
    F: Fn(&mut SslRef, &[u8]) -> bool + 'static + Sync + Send,
{
    unsafe {
        let ssl = SslRef::from_ptr_mut(ssl);
        let callback = ssl
            .ssl_context()
            .ex_data(SslContext::cached_ex_index::<F>())
            .expect("BUG: cookie verify callback missing") as *const F;
        #[allow(clippy::unnecessary_cast)]
        let slice =
            util::from_raw_parts(cookie as *const c_uchar as *const u8, cookie_len as usize);
        (*callback)(ssl, slice) as c_int
    }
}

#[cfg(ossl111)]
pub struct CustomExtAddState<T>(Option<T>);

#[cfg(ossl111)]
pub extern "C" fn raw_custom_ext_add<F, T>(
    ssl: *mut ffi::SSL,
    _: c_uint,
    context: c_uint,
    out: *mut *const c_uchar,
    outlen: *mut size_t,
    x: *mut ffi::X509,
    chainidx: size_t,
    al: *mut c_int,
    _: *mut c_void,
) -> c_int
where
    F: Fn(&mut SslRef, ExtensionContext, Option<(usize, &X509Ref)>) -> Result<Option<T>, SslAlert>
        + 'static
        + Sync
        + Send,
    T: AsRef<[u8]> + 'static + Sync + Send,
{
    unsafe {
        let ssl = SslRef::from_ptr_mut(ssl);
        let callback = ssl
            .ssl_context()
            .ex_data(SslContext::cached_ex_index::<F>())
            .expect("BUG: custom ext add callback missing") as *const F;
        let ectx = ExtensionContext::from_bits_truncate(context);
        let cert = if ectx.contains(ExtensionContext::TLS1_3_CERTIFICATE) {
            Some((chainidx, X509Ref::from_ptr(x)))
        } else {
            None
        };
        match (*callback)(ssl, ectx, cert) {
            Ok(None) => 0,
            Ok(Some(buf)) => {
                *outlen = buf.as_ref().len();
                *out = buf.as_ref().as_ptr();

                let idx = Ssl::cached_ex_index::<CustomExtAddState<T>>();
                let mut buf = Some(buf);
                let new = match ssl.ex_data_mut(idx) {
                    Some(state) => {
                        state.0 = buf.take();
                        false
                    }
                    None => true,
                };
                if new {
                    ssl.set_ex_data(idx, CustomExtAddState(buf));
                }
                1
            }
            Err(alert) => {
                *al = alert.0;
                -1
            }
        }
    }
}

#[cfg(ossl111)]
pub extern "C" fn raw_custom_ext_free<T>(
    ssl: *mut ffi::SSL,
    _: c_uint,
    _: c_uint,
    _: *const c_uchar,
    _: *mut c_void,
) where
    T: 'static + Sync + Send,
{
    unsafe {
        let ssl = SslRef::from_ptr_mut(ssl);
        let idx = Ssl::cached_ex_index::<CustomExtAddState<T>>();
        if let Some(state) = ssl.ex_data_mut(idx) {
            state.0 = None;
        }
    }
}

#[cfg(ossl111)]
pub extern "C" fn raw_custom_ext_parse<F>(
    ssl: *mut ffi::SSL,
    _: c_uint,
    context: c_uint,
    input: *const c_uchar,
    inlen: size_t,
    x: *mut ffi::X509,
    chainidx: size_t,
    al: *mut c_int,
    _: *mut c_void,
) -> c_int
where
    F: Fn(&mut SslRef, ExtensionContext, &[u8], Option<(usize, &X509Ref)>) -> Result<(), SslAlert>
        + 'static
        + Sync
        + Send,
{
    unsafe {
        let ssl = SslRef::from_ptr_mut(ssl);
        let callback = ssl
            .ssl_context()
            .ex_data(SslContext::cached_ex_index::<F>())
            .expect("BUG: custom ext parse callback missing") as *const F;
        let ectx = ExtensionContext::from_bits_truncate(context);
        #[allow(clippy::unnecessary_cast)]
        let slice = util::from_raw_parts(input as *const u8, inlen);
        let cert = if ectx.contains(ExtensionContext::TLS1_3_CERTIFICATE) {
            Some((chainidx, X509Ref::from_ptr(x)))
        } else {
            None
        };
        match (*callback)(ssl, ectx, slice, cert) {
            Ok(()) => 1,
            Err(alert) => {
                *al = alert.0;
                0
            }
        }
    }
}

#[cfg(ossl111)]
pub unsafe extern "C" fn raw_client_hello<F>(
    ssl: *mut ffi::SSL,
    al: *mut c_int,
    arg: *mut c_void,
) -> c_int
where
    F: Fn(&mut SslRef, &mut SslAlert) -> Result<ClientHelloResponse, ErrorStack>
        + 'static
        + Sync
        + Send,
{
    let ssl = SslRef::from_ptr_mut(ssl);
    let callback = arg as *const F;
    let mut alert = SslAlert(*al);

    let r = (*callback)(ssl, &mut alert);
    *al = alert.0;
    match r {
        Ok(c) => c.0,
        Err(e) => {
            e.put();
            ffi::SSL_CLIENT_HELLO_ERROR
        }
    }
}
