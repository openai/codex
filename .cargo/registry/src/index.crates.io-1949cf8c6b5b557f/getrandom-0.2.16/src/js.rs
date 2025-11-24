//! Implementation for WASM based on Web and Node.js
use crate::Error;

extern crate std;
use std::{mem::MaybeUninit, thread_local};

use js_sys::{global, Function, Uint8Array};
use wasm_bindgen::{prelude::wasm_bindgen, JsCast, JsValue};

// Size of our temporary Uint8Array buffer used with WebCrypto methods
// Maximum is 65536 bytes see https://developer.mozilla.org/en-US/docs/Web/API/Crypto/getRandomValues
const WEB_CRYPTO_BUFFER_SIZE: usize = 256;
// Node.js's crypto.randomFillSync requires the size to be less than 2**31.
const NODE_MAX_BUFFER_SIZE: usize = (1 << 31) - 1;

enum RngSource {
    Node(NodeCrypto),
    Web(WebCrypto, Uint8Array),
}

// JsValues are always per-thread, so we initialize RngSource for each thread.
//   See: https://github.com/rustwasm/wasm-bindgen/pull/955
thread_local!(
    static RNG_SOURCE: Result<RngSource, Error> = getrandom_init();
);

pub(crate) fn getrandom_inner(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    RNG_SOURCE.with(|result| {
        let source = result.as_ref().map_err(|&e| e)?;

        match source {
            RngSource::Node(n) => {
                for chunk in dest.chunks_mut(NODE_MAX_BUFFER_SIZE) {
                    // SAFETY: chunk is never used directly, the memory is only
                    // modified via the Uint8Array view, which is passed
                    // directly to JavaScript. Also, crypto.randomFillSync does
                    // not resize the buffer. We know the length is less than
                    // u32::MAX because of the chunking above.
                    // Note that this uses the fact that JavaScript doesn't
                    // have a notion of "uninitialized memory", this is purely
                    // a Rust/C/C++ concept.
                    let res = n.random_fill_sync(unsafe {
                        Uint8Array::view_mut_raw(chunk.as_mut_ptr() as *mut u8, chunk.len())
                    });
                    if res.is_err() {
                        return Err(Error::NODE_RANDOM_FILL_SYNC);
                    }
                }
            }
            RngSource::Web(crypto, buf) => {
                // getRandomValues does not work with all types of WASM memory,
                // so we initially write to browser memory to avoid exceptions.
                for chunk in dest.chunks_mut(WEB_CRYPTO_BUFFER_SIZE) {
                    // The chunk can be smaller than buf's length, so we call to
                    // JS to create a smaller view of buf without allocation.
                    let sub_buf = buf.subarray(0, chunk.len() as u32);

                    if crypto.get_random_values(&sub_buf).is_err() {
                        return Err(Error::WEB_GET_RANDOM_VALUES);
                    }

                    // SAFETY: `sub_buf`'s length is the same length as `chunk`
                    unsafe { sub_buf.raw_copy_to_ptr(chunk.as_mut_ptr() as *mut u8) };
                }
            }
        };
        Ok(())
    })
}

fn getrandom_init() -> Result<RngSource, Error> {
    let global: Global = global().unchecked_into();

    // Get the Web Crypto interface if we are in a browser, Web Worker, Deno,
    // or another environment that supports the Web Cryptography API. This
    // also allows for user-provided polyfills in unsupported environments.
    let crypto = match global.crypto() {
        // Standard Web Crypto interface
        c if c.is_object() => c,
        // Node.js CommonJS Crypto module
        _ if is_node(&global) => {
            // If module.require isn't a valid function, we are in an ES module.
            match Module::require_fn().and_then(JsCast::dyn_into::<Function>) {
                Ok(require_fn) => match require_fn.call1(&global, &JsValue::from_str("crypto")) {
                    Ok(n) => return Ok(RngSource::Node(n.unchecked_into())),
                    Err(_) => return Err(Error::NODE_CRYPTO),
                },
                Err(_) => return Err(Error::NODE_ES_MODULE),
            }
        }
        // IE 11 Workaround
        _ => match global.ms_crypto() {
            c if c.is_object() => c,
            _ => return Err(Error::WEB_CRYPTO),
        },
    };

    let buf = Uint8Array::new_with_length(WEB_CRYPTO_BUFFER_SIZE as u32);
    Ok(RngSource::Web(crypto, buf))
}

// Taken from https://www.npmjs.com/package/browser-or-node
fn is_node(global: &Global) -> bool {
    let process = global.process();
    if process.is_object() {
        let versions = process.versions();
        if versions.is_object() {
            return versions.node().is_string();
        }
    }
    false
}

#[wasm_bindgen]
extern "C" {
    // Return type of js_sys::global()
    type Global;

    // Web Crypto API: Crypto interface (https://www.w3.org/TR/WebCryptoAPI/)
    type WebCrypto;
    // Getters for the WebCrypto API
    #[wasm_bindgen(method, getter)]
    fn crypto(this: &Global) -> WebCrypto;
    #[wasm_bindgen(method, getter, js_name = msCrypto)]
    fn ms_crypto(this: &Global) -> WebCrypto;
    // Crypto.getRandomValues()
    #[wasm_bindgen(method, js_name = getRandomValues, catch)]
    fn get_random_values(this: &WebCrypto, buf: &Uint8Array) -> Result<(), JsValue>;

    // Node JS crypto module (https://nodejs.org/api/crypto.html)
    type NodeCrypto;
    // crypto.randomFillSync()
    #[wasm_bindgen(method, js_name = randomFillSync, catch)]
    fn random_fill_sync(this: &NodeCrypto, buf: Uint8Array) -> Result<(), JsValue>;

    // Ideally, we would just use `fn require(s: &str)` here. However, doing
    // this causes a Webpack warning. So we instead return the function itself
    // and manually invoke it using call1. This also lets us to check that the
    // function actually exists, allowing for better error messages. See:
    //   https://github.com/rust-random/getrandom/issues/224
    //   https://github.com/rust-random/getrandom/issues/256
    type Module;
    #[wasm_bindgen(getter, static_method_of = Module, js_class = module, js_name = require, catch)]
    fn require_fn() -> Result<JsValue, JsValue>;

    // Node JS process Object (https://nodejs.org/api/process.html)
    #[wasm_bindgen(method, getter)]
    fn process(this: &Global) -> Process;
    type Process;
    #[wasm_bindgen(method, getter)]
    fn versions(this: &Process) -> Versions;
    type Versions;
    #[wasm_bindgen(method, getter)]
    fn node(this: &Versions) -> JsValue;
}
