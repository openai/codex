#![allow(dead_code, unused_imports)] // Keeps our cfg's from becoming too convoluted in here

trait Rng {
    fn u128() -> u128;
    fn u64() -> u64;
    fn u16() -> u16;
}

pub(crate) fn u128() -> u128 {
    imp::RngImp::u128()
}

pub(crate) fn u64() -> u64 {
    imp::RngImp::u64()
}

pub(crate) fn u16() -> u16 {
    imp::RngImp::u16()
}

#[cfg(not(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none"))))]
mod imp {
    /*
    Random support for non `wasm32-unknown-unknown` platforms.
    */

    use super::*;

    // Using `rand`
    #[cfg(any(feature = "rng-rand", feature = "fast-rng"))]
    pub(super) struct RngImp;

    #[cfg(any(feature = "rng-rand", feature = "fast-rng"))]
    impl Rng for RngImp {
        fn u128() -> u128 {
            rand::random()
        }

        fn u64() -> u64 {
            rand::random()
        }

        fn u16() -> u16 {
            rand::random()
        }
    }

    // Using `getrandom`
    #[cfg(all(not(feature = "fast-rng"), not(feature = "rng-rand")))]
    pub(super) struct RngImp;

    #[cfg(all(not(feature = "fast-rng"), not(feature = "rng-rand")))]
    impl Rng for RngImp {
        fn u128() -> u128 {
            let mut bytes = [0u8; 16];

            getrandom::fill(&mut bytes).unwrap_or_else(|err| {
                // NB: getrandom::Error has no source; this is adequate display
                panic!("could not retrieve random bytes for uuid: {}", err)
            });

            u128::from_ne_bytes(bytes)
        }

        fn u64() -> u64 {
            let mut bytes = [0u8; 8];

            getrandom::fill(&mut bytes).unwrap_or_else(|err| {
                // NB: getrandom::Error has no source; this is adequate display
                panic!("could not retrieve random bytes for uuid: {}", err)
            });

            u64::from_ne_bytes(bytes)
        }

        fn u16() -> u16 {
            let mut bytes = [0u8; 2];

            getrandom::fill(&mut bytes).unwrap_or_else(|err| {
                // NB: getrandom::Error has no source; this is adequate display
                panic!("could not retrieve random bytes for uuid: {}", err)
            });

            u16::from_ne_bytes(bytes)
        }
    }
}

#[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
mod imp {
    /*
    Random support for `wasm32-unknown-unknown`.
    */

    #![allow(dead_code, unused_imports)] // Keeps our cfg's from becoming too convoluted in here

    use super::*;

    #[cfg(all(
        not(feature = "js"),
        not(feature = "rng-getrandom"),
        not(feature = "rng-rand")
    ))]
    compile_error!("to use `uuid` on `wasm32-unknown-unknown`, specify a source of randomness using one of the `js`, `rng-getrandom`, or `rng-rand` features");

    // Using `rand`
    #[cfg(feature = "rng-rand")]
    pub(super) struct RngImp;

    #[cfg(feature = "rng-rand")]
    impl Rng for RngImp {
        fn u128() -> u128 {
            uuid_rng_internal_lib::__private::rand::random()
        }

        fn u64() -> u64 {
            uuid_rng_internal_lib::__private::rand::random()
        }

        fn u16() -> u16 {
            uuid_rng_internal_lib::__private::rand::random()
        }
    }

    // Using `getrandom`
    #[cfg(all(feature = "rng-getrandom", not(feature = "rng-rand")))]
    pub(super) struct RngImp;

    #[cfg(all(feature = "rng-getrandom", not(feature = "rng-rand")))]
    impl Rng for RngImp {
        fn u128() -> u128 {
            let mut bytes = [0u8; 16];

            uuid_rng_internal_lib::__private::getrandom::fill(&mut bytes).unwrap_or_else(|err| {
                // NB: getrandom::Error has no source; this is adequate display
                panic!("could not retrieve random bytes for uuid: {}", err)
            });

            u128::from_ne_bytes(bytes)
        }

        fn u64() -> u64 {
            let mut bytes = [0u8; 8];

            uuid_rng_internal_lib::__private::getrandom::fill(&mut bytes).unwrap_or_else(|err| {
                // NB: getrandom::Error has no source; this is adequate display
                panic!("could not retrieve random bytes for uuid: {}", err)
            });

            u64::from_ne_bytes(bytes)
        }

        fn u16() -> u16 {
            let mut bytes = [0u8; 2];

            uuid_rng_internal_lib::__private::getrandom::fill(&mut bytes).unwrap_or_else(|err| {
                // NB: getrandom::Error has no source; this is adequate display
                panic!("could not retrieve random bytes for uuid: {}", err)
            });

            u16::from_ne_bytes(bytes)
        }
    }

    // Using WebCrypto via `wasm-bindgen`
    #[cfg(all(
        feature = "js",
        not(feature = "rng-rand"),
        not(feature = "rng-getrandom")
    ))]
    pub(super) struct RngImp;

    #[cfg(all(
        feature = "js",
        not(feature = "rng-rand"),
        not(feature = "rng-getrandom")
    ))]
    impl Rng for RngImp {
        fn u128() -> u128 {
            let mut bytes = [0u8; 16];

            if !webcrypto::fill(&mut bytes) {
                panic!("could not retrieve random bytes for uuid")
            }

            u128::from_ne_bytes(bytes)
        }

        fn u64() -> u64 {
            let mut bytes = [0u8; 8];

            if !webcrypto::fill(&mut bytes) {
                panic!("could not retrieve random bytes for uuid")
            }

            u64::from_ne_bytes(bytes)
        }

        fn u16() -> u16 {
            let mut bytes = [0u8; 2];

            if !webcrypto::fill(&mut bytes) {
                panic!("could not retrieve random bytes for uuid")
            }

            u16::from_ne_bytes(bytes)
        }
    }

    #[cfg(feature = "js")]
    mod webcrypto {
        /*
        This module preserves the stabilized behavior of `uuid` that requires the
        `js` feature to enable rng on `wasm32-unknown-unknown`, which it inherited
        from `getrandom` `0.2`.

        Vendored from `getrandom`: https://github.com/rust-random/getrandom/blob/ce3b017fdee0233c6ecd61e68b96a84bf6f911bf/src/backends/wasm_js.rs

        Copyright (c) 2018-2024 The rust-random Project Developers
        Copyright (c) 2014 The Rust Project Developers

        Permission is hereby granted, free of charge, to any
        person obtaining a copy of this software and associated
        documentation files (the "Software"), to deal in the
        Software without restriction, including without
        limitation the rights to use, copy, modify, merge,
        publish, distribute, sublicense, and/or sell copies of
        the Software, and to permit persons to whom the Software
        is furnished to do so, subject to the following
        conditions:

        The above copyright notice and this permission notice
        shall be included in all copies or substantial portions
        of the Software.

        THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
        ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
        TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
        PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
        SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
        CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
        OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
        IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
        DEALINGS IN THE SOFTWARE.
        */

        use wasm_bindgen::{prelude::wasm_bindgen, JsValue};

        #[cfg(target_feature = "atomics")]
        use core::convert::TryInto;

        // Maximum buffer size allowed in `Crypto.getRandomValuesSize` is 65536 bytes.
        // See https://developer.mozilla.org/en-US/docs/Web/API/Crypto/getRandomValues
        const MAX_BUFFER_SIZE: usize = 65536;

        #[cfg(not(target_feature = "atomics"))]
        #[inline]
        pub fn fill(dest: &mut [u8]) -> bool {
            for chunk in dest.chunks_mut(MAX_BUFFER_SIZE) {
                if get_random_values(chunk).is_err() {
                    return false;
                }
            }

            true
        }

        #[cfg(target_feature = "atomics")]
        pub fn fill(dest: &mut [u8]) -> bool {
            // getRandomValues does not work with all types of WASM memory,
            // so we initially write to browser memory to avoid exceptions.
            let buf_len = usize::min(dest.len(), MAX_BUFFER_SIZE);
            let buf_len_u32 = buf_len
                .try_into()
                .expect("buffer length is bounded by MAX_BUFFER_SIZE");
            let buf = js_sys::Uint8Array::new_with_length(buf_len_u32);
            for chunk in dest.chunks_mut(buf_len) {
                let chunk_len = chunk
                    .len()
                    .try_into()
                    .expect("chunk length is bounded by MAX_BUFFER_SIZE");
                // The chunk can be smaller than buf's length, so we call to
                // JS to create a smaller view of buf without allocation.
                let sub_buf = if chunk_len == buf_len_u32 {
                    &buf
                } else {
                    &buf.subarray(0, chunk_len)
                };

                if get_random_values(sub_buf).is_err() {
                    return false;
                }

                sub_buf.copy_to(chunk);
            }

            true
        }

        #[wasm_bindgen]
        extern "C" {
            // Crypto.getRandomValues()
            #[cfg(not(target_feature = "atomics"))]
            #[wasm_bindgen(js_namespace = ["globalThis", "crypto"], js_name = getRandomValues, catch)]
            fn get_random_values(buf: &mut [u8]) -> Result<(), JsValue>;
            #[cfg(target_feature = "atomics")]
            #[wasm_bindgen(js_namespace = ["globalThis", "crypto"], js_name = getRandomValues, catch)]
            fn get_random_values(buf: &js_sys::Uint8Array) -> Result<(), JsValue>;
        }
    }
}
