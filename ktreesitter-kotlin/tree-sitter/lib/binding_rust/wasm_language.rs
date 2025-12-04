use std::{
    error,
    ffi::{CStr, CString},
    fmt,
    mem::{self, MaybeUninit},
    os::raw::c_char,
};

pub use wasmtime_c_api::wasmtime;

use crate::{ffi, Language, LanguageError, Parser, FREE_FN};

// Force Cargo to include wasmtime-c-api as a dependency of this crate,
// even though it is only used by the C code.
#[allow(unused)]
fn _use_wasmtime() {
    wasmtime_c_api::wasm_engine_new();
}

#[repr(C)]
#[derive(Clone)]
#[allow(non_camel_case_types)]
pub struct wasm_engine_t {
    pub(crate) engine: wasmtime::Engine,
}

pub struct WasmStore(*mut ffi::TSWasmStore);

unsafe impl Send for WasmStore {}
unsafe impl Sync for WasmStore {}

#[derive(Debug, PartialEq, Eq)]
pub struct WasmError {
    pub kind: WasmErrorKind,
    pub message: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum WasmErrorKind {
    Parse,
    Compile,
    Instantiate,
    Other,
}

impl WasmStore {
    pub fn new(engine: &wasmtime::Engine) -> Result<Self, WasmError> {
        unsafe {
            let mut error = MaybeUninit::<ffi::TSWasmError>::uninit();
            let store = ffi::ts_wasm_store_new(
                std::ptr::from_ref::<wasmtime::Engine>(engine)
                    .cast_mut()
                    .cast(),
                error.as_mut_ptr(),
            );
            if store.is_null() {
                Err(WasmError::new(error.assume_init()))
            } else {
                Ok(Self(store))
            }
        }
    }

    pub fn load_language(&mut self, name: &str, bytes: &[u8]) -> Result<Language, WasmError> {
        let name = CString::new(name).unwrap();
        unsafe {
            let mut error = MaybeUninit::<ffi::TSWasmError>::uninit();
            let language = ffi::ts_wasm_store_load_language(
                self.0,
                name.as_ptr(),
                bytes.as_ptr().cast::<c_char>(),
                bytes.len() as u32,
                error.as_mut_ptr(),
            );
            if language.is_null() {
                Err(WasmError::new(error.assume_init()))
            } else {
                Ok(Language(language))
            }
        }
    }

    #[must_use]
    pub fn language_count(&self) -> usize {
        unsafe { ffi::ts_wasm_store_language_count(self.0) }
    }
}

impl WasmError {
    unsafe fn new(error: ffi::TSWasmError) -> Self {
        let message = CStr::from_ptr(error.message).to_str().unwrap().to_string();
        (FREE_FN)(error.message.cast());
        Self {
            kind: match error.kind {
                ffi::TSWasmErrorKindParse => WasmErrorKind::Parse,
                ffi::TSWasmErrorKindCompile => WasmErrorKind::Compile,
                ffi::TSWasmErrorKindInstantiate => WasmErrorKind::Instantiate,
                _ => WasmErrorKind::Other,
            },
            message,
        }
    }
}

impl Language {
    #[must_use]
    pub fn is_wasm(&self) -> bool {
        unsafe { ffi::ts_language_is_wasm(self.0) }
    }
}

impl Parser {
    pub fn set_wasm_store(&mut self, store: WasmStore) -> Result<(), LanguageError> {
        unsafe { ffi::ts_parser_set_wasm_store(self.0.as_ptr(), store.0) };
        mem::forget(store);
        Ok(())
    }

    pub fn take_wasm_store(&mut self) -> Option<WasmStore> {
        let ptr = unsafe { ffi::ts_parser_take_wasm_store(self.0.as_ptr()) };
        if ptr.is_null() {
            None
        } else {
            Some(WasmStore(ptr))
        }
    }
}

impl Drop for WasmStore {
    fn drop(&mut self) {
        unsafe { ffi::ts_wasm_store_delete(self.0) };
    }
}

impl fmt::Display for WasmError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let kind = match self.kind {
            WasmErrorKind::Parse => "Failed to parse wasm",
            WasmErrorKind::Compile => "Failed to compile wasm",
            WasmErrorKind::Instantiate => "Failed to instantiate wasm module",
            WasmErrorKind::Other => "Unknown error",
        };
        write!(f, "{kind}: {}", self.message)
    }
}

impl error::Error for WasmError {}
