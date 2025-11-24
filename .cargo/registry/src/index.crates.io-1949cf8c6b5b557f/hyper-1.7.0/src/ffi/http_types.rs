use std::ffi::{c_int, c_void};

use bytes::Bytes;

use super::body::hyper_body;
use super::error::hyper_code;
use super::task::{hyper_task_return_type, AsTaskType};
use super::{UserDataPointer, HYPER_ITER_CONTINUE};
use crate::body::Incoming as IncomingBody;
use crate::ext::{HeaderCaseMap, OriginalHeaderOrder, ReasonPhrase};
use crate::ffi::size_t;
use crate::header::{HeaderName, HeaderValue};
use crate::{HeaderMap, Method, Request, Response, Uri};

/// An HTTP request.
///
/// Once you've finished constructing a request, you can send it with
/// `hyper_clientconn_send`.
///
/// Methods:
///
/// - hyper_request_new:              Construct a new HTTP request.
/// - hyper_request_headers:          Gets a mutable reference to the HTTP headers of this request
/// - hyper_request_set_body:         Set the body of the request.
/// - hyper_request_set_method:       Set the HTTP Method of the request.
/// - hyper_request_set_uri:          Set the URI of the request.
/// - hyper_request_set_uri_parts:    Set the URI of the request with separate scheme, authority, and path/query strings.
/// - hyper_request_set_version:      Set the preferred HTTP version of the request.
/// - hyper_request_on_informational: Set an informational (1xx) response callback.
/// - hyper_request_free:             Free an HTTP request.
pub struct hyper_request(pub(super) Request<IncomingBody>);

/// An HTTP response.
///
/// Obtain one of these by making a request with `hyper_clientconn_send`, then
/// polling the executor unntil you get a `hyper_task` of type
/// `HYPER_TASK_RESPONSE`. To figure out which request this response
/// corresponds to, check the userdata of the task, which you should
/// previously have set to an application-specific identifier for the
/// request.
///
/// Methods:
///
/// - hyper_response_status:            Get the HTTP-Status code of this response.
/// - hyper_response_version:           Get the HTTP version used by this response.
/// - hyper_response_reason_phrase:     Get a pointer to the reason-phrase of this response.
/// - hyper_response_reason_phrase_len: Get the length of the reason-phrase of this response.
/// - hyper_response_headers:           Gets a reference to the HTTP headers of this response.
/// - hyper_response_body:              Take ownership of the body of this response.
/// - hyper_response_free:              Free an HTTP response.
pub struct hyper_response(pub(super) Response<IncomingBody>);

/// An HTTP header map.
///
/// These can be part of a request or response.
///
/// Obtain a pointer to read or modify these from `hyper_request_headers`
/// or `hyper_response_headers`.
///
/// Methods:
///
/// - hyper_headers_add:     Adds the provided value to the list of the provided name.
/// - hyper_headers_foreach: Iterates the headers passing each name and value pair to the callback.
/// - hyper_headers_set:     Sets the header with the provided name to the provided value.
#[derive(Clone)]
pub struct hyper_headers {
    pub(super) headers: HeaderMap,
    orig_casing: HeaderCaseMap,
    orig_order: OriginalHeaderOrder,
}

#[derive(Clone)]
struct OnInformational {
    func: hyper_request_on_informational_callback,
    data: UserDataPointer,
}

type hyper_request_on_informational_callback = extern "C" fn(*mut c_void, *mut hyper_response);

// ===== impl hyper_request =====

ffi_fn! {
    /// Construct a new HTTP request.
    ///
    /// The default request has an empty body. To send a body, call `hyper_request_set_body`.
    ///
    ///
    /// To avoid a memory leak, the request must eventually be consumed by
    /// `hyper_request_free` or `hyper_clientconn_send`.
    fn hyper_request_new() -> *mut hyper_request {
        Box::into_raw(Box::new(hyper_request(Request::new(IncomingBody::empty()))))
    } ?= std::ptr::null_mut()
}

ffi_fn! {
    /// Free an HTTP request.
    ///
    /// This should only be used if the request isn't consumed by
    /// `hyper_clientconn_send`.
    fn hyper_request_free(req: *mut hyper_request) {
        drop(non_null!(Box::from_raw(req) ?= ()));
    }
}

ffi_fn! {
    /// Set the HTTP Method of the request.
    fn hyper_request_set_method(req: *mut hyper_request, method: *const u8, method_len: size_t) -> hyper_code {
        let bytes = unsafe {
            std::slice::from_raw_parts(method, method_len as usize)
        };
        let req = non_null!(&mut *req ?= hyper_code::HYPERE_INVALID_ARG);
        match Method::from_bytes(bytes) {
            Ok(m) => {
                *req.0.method_mut() = m;
                hyper_code::HYPERE_OK
            },
            Err(_) => {
                hyper_code::HYPERE_INVALID_ARG
            }
        }
    }
}

ffi_fn! {
    /// Set the URI of the request.
    ///
    /// The request's URI is best described as the `request-target` from the RFCs. So in HTTP/1,
    /// whatever is set will get sent as-is in the first line (GET $uri HTTP/1.1). It
    /// supports the 4 defined variants, origin-form, absolute-form, authority-form, and
    /// asterisk-form.
    ///
    /// The underlying type was built to efficiently support HTTP/2 where the request-target is
    /// split over :scheme, :authority, and :path. As such, each part can be set explicitly, or the
    /// type can parse a single contiguous string and if a scheme is found, that slot is "set". If
    /// the string just starts with a path, only the path portion is set. All pseudo headers that
    /// have been parsed/set are sent when the connection type is HTTP/2.
    ///
    /// To set each slot explicitly, use `hyper_request_set_uri_parts`.
    fn hyper_request_set_uri(req: *mut hyper_request, uri: *const u8, uri_len: size_t) -> hyper_code {
        let bytes = unsafe {
            std::slice::from_raw_parts(uri, uri_len as usize)
        };
        let req = non_null!(&mut *req ?= hyper_code::HYPERE_INVALID_ARG);
        match Uri::from_maybe_shared(bytes) {
            Ok(u) => {
                *req.0.uri_mut() = u;
                hyper_code::HYPERE_OK
            },
            Err(_) => {
                hyper_code::HYPERE_INVALID_ARG
            }
        }
    }
}

ffi_fn! {
    /// Set the URI of the request with separate scheme, authority, and
    /// path/query strings.
    ///
    /// Each of `scheme`, `authority`, and `path_and_query` should either be
    /// null, to skip providing a component, or point to a UTF-8 encoded
    /// string. If any string pointer argument is non-null, its corresponding
    /// `len` parameter must be set to the string's length.
    fn hyper_request_set_uri_parts(
        req: *mut hyper_request,
        scheme: *const u8,
        scheme_len: size_t,
        authority: *const u8,
        authority_len: size_t,
        path_and_query: *const u8,
        path_and_query_len: size_t
    ) -> hyper_code {
        let mut builder = Uri::builder();
        if !scheme.is_null() {
            let scheme_bytes = unsafe {
                std::slice::from_raw_parts(scheme, scheme_len as usize)
            };
            builder = builder.scheme(scheme_bytes);
        }
        if !authority.is_null() {
            let authority_bytes = unsafe {
                std::slice::from_raw_parts(authority, authority_len as usize)
            };
            builder = builder.authority(authority_bytes);
        }
        if !path_and_query.is_null() {
            let path_and_query_bytes = unsafe {
                std::slice::from_raw_parts(path_and_query, path_and_query_len as usize)
            };
            builder = builder.path_and_query(path_and_query_bytes);
        }
        match builder.build() {
            Ok(u) => {
                *unsafe { &mut *req }.0.uri_mut() = u;
                hyper_code::HYPERE_OK
            },
            Err(_) => {
                hyper_code::HYPERE_INVALID_ARG
            }
        }
    }
}

ffi_fn! {
    /// Set the preferred HTTP version of the request.
    ///
    /// The version value should be one of the `HYPER_HTTP_VERSION_` constants.
    ///
    /// Note that this won't change the major HTTP version of the connection,
    /// since that is determined at the handshake step.
    fn hyper_request_set_version(req: *mut hyper_request, version: c_int) -> hyper_code {
        use http::Version;

        let req = non_null!(&mut *req ?= hyper_code::HYPERE_INVALID_ARG);
        *req.0.version_mut() = match version {
            super::HYPER_HTTP_VERSION_NONE => Version::HTTP_11,
            super::HYPER_HTTP_VERSION_1_0 => Version::HTTP_10,
            super::HYPER_HTTP_VERSION_1_1 => Version::HTTP_11,
            super::HYPER_HTTP_VERSION_2 => Version::HTTP_2,
            _ => {
                // We don't know this version
                return hyper_code::HYPERE_INVALID_ARG;
            }
        };
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Gets a mutable reference to the HTTP headers of this request
    ///
    /// This is not an owned reference, so it should not be accessed after the
    /// `hyper_request` has been consumed.
    fn hyper_request_headers(req: *mut hyper_request) -> *mut hyper_headers {
        hyper_headers::get_or_default(unsafe { &mut *req }.0.extensions_mut())
    } ?= std::ptr::null_mut()
}

ffi_fn! {
    /// Set the body of the request.
    ///
    /// You can get a `hyper_body` by calling `hyper_body_new`.
    ///
    /// This takes ownership of the `hyper_body *`, you must not use it or
    /// free it after setting it on the request.
    fn hyper_request_set_body(req: *mut hyper_request, body: *mut hyper_body) -> hyper_code {
        let body = non_null!(Box::from_raw(body) ?= hyper_code::HYPERE_INVALID_ARG);
        let req = non_null!(&mut *req ?= hyper_code::HYPERE_INVALID_ARG);
        *req.0.body_mut() = body.0;
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Set an informational (1xx) response callback.
    ///
    /// The callback is called each time hyper receives an informational (1xx)
    /// response for this request.
    ///
    /// The third argument is an opaque user data pointer, which is passed to
    /// the callback each time.
    ///
    /// The callback is passed the `void *` data pointer, and a
    /// `hyper_response *` which can be inspected as any other response. The
    /// body of the response will always be empty.
    ///
    /// NOTE: The `hyper_response *` is just borrowed data, and will not
    /// be valid after the callback finishes. You must copy any data you wish
    /// to persist.
    fn hyper_request_on_informational(req: *mut hyper_request, callback: hyper_request_on_informational_callback, data: *mut c_void) -> hyper_code {
        #[cfg(feature = "client")]
        {
        let ext = OnInformational {
            func: callback,
            data: UserDataPointer(data),
        };
        let req = non_null!(&mut *req ?= hyper_code::HYPERE_INVALID_ARG);
        crate::ext::on_informational_raw(&mut req.0, ext);
        hyper_code::HYPERE_OK
        }
        #[cfg(not(feature = "client"))]
        {
        drop((req, callback, data));
        hyper_code::HYPERE_FEATURE_NOT_ENABLED
        }
    }
}

impl hyper_request {
    pub(super) fn finalize_request(&mut self) {
        if let Some(headers) = self.0.extensions_mut().remove::<hyper_headers>() {
            *self.0.headers_mut() = headers.headers;
            self.0.extensions_mut().insert(headers.orig_casing);
            self.0.extensions_mut().insert(headers.orig_order);
        }
    }
}

// ===== impl hyper_response =====

ffi_fn! {
    /// Free an HTTP response.
    ///
    /// This should be used for any response once it is no longer needed.
    fn hyper_response_free(resp: *mut hyper_response) {
        drop(non_null!(Box::from_raw(resp) ?= ()));
    }
}

ffi_fn! {
    /// Get the HTTP-Status code of this response.
    ///
    /// It will always be within the range of 100-599.
    fn hyper_response_status(resp: *const hyper_response) -> u16 {
        non_null!(&*resp ?= 0).0.status().as_u16()
    }
}

ffi_fn! {
    /// Get a pointer to the reason-phrase of this response.
    ///
    /// This buffer is not null-terminated.
    ///
    /// This buffer is owned by the response, and should not be used after
    /// the response has been freed.
    ///
    /// Use `hyper_response_reason_phrase_len()` to get the length of this
    /// buffer.
    fn hyper_response_reason_phrase(resp: *const hyper_response) -> *const u8 {
        non_null!(&*resp ?= std::ptr::null()).reason_phrase().as_ptr()
    } ?= std::ptr::null()
}

ffi_fn! {
    /// Get the length of the reason-phrase of this response.
    ///
    /// Use `hyper_response_reason_phrase()` to get the buffer pointer.
    fn hyper_response_reason_phrase_len(resp: *const hyper_response) -> size_t {
        non_null!(&*resp ?= 0).reason_phrase().len()
    }
}

ffi_fn! {
    /// Get the HTTP version used by this response.
    ///
    /// The returned value could be:
    ///
    /// - `HYPER_HTTP_VERSION_1_0`
    /// - `HYPER_HTTP_VERSION_1_1`
    /// - `HYPER_HTTP_VERSION_2`
    /// - `HYPER_HTTP_VERSION_NONE` if newer (or older).
    fn hyper_response_version(resp: *const hyper_response) -> c_int {
        use http::Version;

        match non_null!(&*resp ?= 0).0.version() {
            Version::HTTP_10 => super::HYPER_HTTP_VERSION_1_0,
            Version::HTTP_11 => super::HYPER_HTTP_VERSION_1_1,
            Version::HTTP_2 => super::HYPER_HTTP_VERSION_2,
            _ => super::HYPER_HTTP_VERSION_NONE,
        }
    }
}

ffi_fn! {
    /// Gets a reference to the HTTP headers of this response.
    ///
    /// This is not an owned reference, so it should not be accessed after the
    /// `hyper_response` has been freed.
    fn hyper_response_headers(resp: *mut hyper_response) -> *mut hyper_headers {
        hyper_headers::get_or_default(unsafe { &mut *resp }.0.extensions_mut())
    } ?= std::ptr::null_mut()
}

ffi_fn! {
    /// Take ownership of the body of this response.
    ///
    /// It is safe to free the response even after taking ownership of its body.
    ///
    /// To avoid a memory leak, the body must eventually be consumed by
    /// `hyper_body_free`, `hyper_body_foreach`, or `hyper_request_set_body`.
    fn hyper_response_body(resp: *mut hyper_response) -> *mut hyper_body {
        let body = std::mem::replace(non_null!(&mut *resp ?= std::ptr::null_mut()).0.body_mut(), IncomingBody::empty());
        Box::into_raw(Box::new(hyper_body(body)))
    } ?= std::ptr::null_mut()
}

impl hyper_response {
    pub(super) fn wrap(mut resp: Response<IncomingBody>) -> hyper_response {
        let headers = std::mem::take(resp.headers_mut());
        let orig_casing = resp
            .extensions_mut()
            .remove::<HeaderCaseMap>()
            .unwrap_or_else(HeaderCaseMap::default);
        let orig_order = resp
            .extensions_mut()
            .remove::<OriginalHeaderOrder>()
            .unwrap_or_else(OriginalHeaderOrder::default);
        resp.extensions_mut().insert(hyper_headers {
            headers,
            orig_casing,
            orig_order,
        });

        hyper_response(resp)
    }

    fn reason_phrase(&self) -> &[u8] {
        if let Some(reason) = self.0.extensions().get::<ReasonPhrase>() {
            return reason.as_bytes();
        }

        if let Some(reason) = self.0.status().canonical_reason() {
            return reason.as_bytes();
        }

        &[]
    }
}

unsafe impl AsTaskType for hyper_response {
    fn as_task_type(&self) -> hyper_task_return_type {
        hyper_task_return_type::HYPER_TASK_RESPONSE
    }
}

// ===== impl Headers =====

type hyper_headers_foreach_callback =
    extern "C" fn(*mut c_void, *const u8, size_t, *const u8, size_t) -> c_int;

impl hyper_headers {
    pub(super) fn get_or_default(ext: &mut http::Extensions) -> &mut hyper_headers {
        if let None = ext.get_mut::<hyper_headers>() {
            ext.insert(hyper_headers::default());
        }

        ext.get_mut::<hyper_headers>().unwrap()
    }
}

ffi_fn! {
    /// Iterates the headers passing each name and value pair to the callback.
    ///
    /// The `userdata` pointer is also passed to the callback.
    ///
    /// The callback should return `HYPER_ITER_CONTINUE` to keep iterating, or
    /// `HYPER_ITER_BREAK` to stop.
    fn hyper_headers_foreach(headers: *const hyper_headers, func: hyper_headers_foreach_callback, userdata: *mut c_void) {
        let headers = non_null!(&*headers ?= ());
        // For each header name/value pair, there may be a value in the casemap
        // that corresponds to the HeaderValue. So, we iterator all the keys,
        // and for each one, try to pair the originally cased name with the value.
        //
        // TODO: consider adding http::HeaderMap::entries() iterator
        let mut ordered_iter =  headers.orig_order.get_in_order().peekable();
        if ordered_iter.peek().is_some() {
            for (name, idx) in ordered_iter {
                let (name_ptr, name_len) = if let Some(orig_name) = headers.orig_casing.get_all(name).nth(*idx) {
                    (orig_name.as_ref().as_ptr(), orig_name.as_ref().len())
                } else {
                    (
                    name.as_str().as_bytes().as_ptr(),
                    name.as_str().as_bytes().len(),
                    )
                };

                let val_ptr;
                let val_len;
                if let Some(value) = headers.headers.get_all(name).iter().nth(*idx) {
                    val_ptr = value.as_bytes().as_ptr();
                    val_len = value.as_bytes().len();
                } else {
                    // Stop iterating, something has gone wrong.
                    return;
                }

                if HYPER_ITER_CONTINUE != func(userdata, name_ptr, name_len, val_ptr, val_len) {
                    return;
                }
            }
        } else {
            for name in headers.headers.keys() {
                let mut names = headers.orig_casing.get_all(name);

                for value in headers.headers.get_all(name) {
                    let (name_ptr, name_len) = if let Some(orig_name) = names.next() {
                        (orig_name.as_ref().as_ptr(), orig_name.as_ref().len())
                    } else {
                        (
                            name.as_str().as_bytes().as_ptr(),
                            name.as_str().as_bytes().len(),
                        )
                    };

                    let val_ptr = value.as_bytes().as_ptr();
                    let val_len = value.as_bytes().len();

                    if HYPER_ITER_CONTINUE != func(userdata, name_ptr, name_len, val_ptr, val_len) {
                        return;
                    }
                }
            }
        }
    }
}

ffi_fn! {
    /// Sets the header with the provided name to the provided value.
    ///
    /// This overwrites any previous value set for the header.
    fn hyper_headers_set(headers: *mut hyper_headers, name: *const u8, name_len: size_t, value: *const u8, value_len: size_t) -> hyper_code {
        let headers = non_null!(&mut *headers ?= hyper_code::HYPERE_INVALID_ARG);
        match unsafe { raw_name_value(name, name_len, value, value_len) } {
            Ok((name, value, orig_name)) => {
                headers.headers.insert(&name, value);
                headers.orig_casing.insert(name.clone(), orig_name.clone());
                headers.orig_order.insert(name);
                hyper_code::HYPERE_OK
            }
            Err(code) => code,
        }
    }
}

ffi_fn! {
    /// Adds the provided value to the list of the provided name.
    ///
    /// If there were already existing values for the name, this will append the
    /// new value to the internal list.
    fn hyper_headers_add(headers: *mut hyper_headers, name: *const u8, name_len: size_t, value: *const u8, value_len: size_t) -> hyper_code {
        let headers = non_null!(&mut *headers ?= hyper_code::HYPERE_INVALID_ARG);

        match unsafe { raw_name_value(name, name_len, value, value_len) } {
            Ok((name, value, orig_name)) => {
                headers.headers.append(&name, value);
                headers.orig_casing.append(&name, orig_name.clone());
                headers.orig_order.append(name);
                hyper_code::HYPERE_OK
            }
            Err(code) => code,
        }
    }
}

impl Default for hyper_headers {
    fn default() -> Self {
        Self {
            headers: Default::default(),
            orig_casing: HeaderCaseMap::default(),
            orig_order: OriginalHeaderOrder::default(),
        }
    }
}

unsafe fn raw_name_value(
    name: *const u8,
    name_len: size_t,
    value: *const u8,
    value_len: size_t,
) -> Result<(HeaderName, HeaderValue, Bytes), hyper_code> {
    let name = std::slice::from_raw_parts(name, name_len);
    let orig_name = Bytes::copy_from_slice(name);
    let name = match HeaderName::from_bytes(name) {
        Ok(name) => name,
        Err(_) => return Err(hyper_code::HYPERE_INVALID_ARG),
    };
    let value = std::slice::from_raw_parts(value, value_len);
    let value = match HeaderValue::from_bytes(value) {
        Ok(val) => val,
        Err(_) => return Err(hyper_code::HYPERE_INVALID_ARG),
    };

    Ok((name, value, orig_name))
}

// ===== impl OnInformational =====

#[cfg(feature = "client")]
impl crate::ext::OnInformationalCallback for OnInformational {
    fn on_informational(&self, res: http::Response<()>) {
        let res = res.map(|()| IncomingBody::empty());
        let mut res = hyper_response::wrap(res);
        (self.func)(self.data.0, &mut res);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headers_foreach_cases_preserved() {
        let mut headers = hyper_headers::default();

        let name1 = b"Set-CookiE";
        let value1 = b"a=b";
        hyper_headers_add(
            &mut headers,
            name1.as_ptr(),
            name1.len(),
            value1.as_ptr(),
            value1.len(),
        );

        let name2 = b"SET-COOKIE";
        let value2 = b"c=d";
        hyper_headers_add(
            &mut headers,
            name2.as_ptr(),
            name2.len(),
            value2.as_ptr(),
            value2.len(),
        );

        let mut vec = Vec::<u8>::new();
        hyper_headers_foreach(&headers, concat, &mut vec as *mut _ as *mut c_void);

        assert_eq!(vec, b"Set-CookiE: a=b\r\nSET-COOKIE: c=d\r\n");

        extern "C" fn concat(
            vec: *mut c_void,
            name: *const u8,
            name_len: usize,
            value: *const u8,
            value_len: usize,
        ) -> c_int {
            unsafe {
                let vec = &mut *(vec as *mut Vec<u8>);
                let name = std::slice::from_raw_parts(name, name_len);
                let value = std::slice::from_raw_parts(value, value_len);
                vec.extend(name);
                vec.extend(b": ");
                vec.extend(value);
                vec.extend(b"\r\n");
            }
            HYPER_ITER_CONTINUE
        }
    }

    #[cfg(all(feature = "http1", feature = "ffi"))]
    #[test]
    fn test_headers_foreach_order_preserved() {
        let mut headers = hyper_headers::default();

        let name1 = b"Set-CookiE";
        let value1 = b"a=b";
        hyper_headers_add(
            &mut headers,
            name1.as_ptr(),
            name1.len(),
            value1.as_ptr(),
            value1.len(),
        );

        let name2 = b"Content-Encoding";
        let value2 = b"gzip";
        hyper_headers_add(
            &mut headers,
            name2.as_ptr(),
            name2.len(),
            value2.as_ptr(),
            value2.len(),
        );

        let name3 = b"SET-COOKIE";
        let value3 = b"c=d";
        hyper_headers_add(
            &mut headers,
            name3.as_ptr(),
            name3.len(),
            value3.as_ptr(),
            value3.len(),
        );

        let mut vec = Vec::<u8>::new();
        hyper_headers_foreach(&headers, concat, &mut vec as *mut _ as *mut c_void);

        println!("{}", std::str::from_utf8(&vec).unwrap());
        assert_eq!(
            vec,
            b"Set-CookiE: a=b\r\nContent-Encoding: gzip\r\nSET-COOKIE: c=d\r\n"
        );

        extern "C" fn concat(
            vec: *mut c_void,
            name: *const u8,
            name_len: usize,
            value: *const u8,
            value_len: usize,
        ) -> c_int {
            unsafe {
                let vec = &mut *(vec as *mut Vec<u8>);
                let name = std::slice::from_raw_parts(name, name_len);
                let value = std::slice::from_raw_parts(value, value_len);
                vec.extend(name);
                vec.extend(b": ");
                vec.extend(value);
                vec.extend(b"\r\n");
            }
            HYPER_ITER_CONTINUE
        }
    }
}
