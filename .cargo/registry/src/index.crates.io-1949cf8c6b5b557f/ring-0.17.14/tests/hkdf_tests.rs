// Copyright 2015 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
// SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
// OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
// CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

#![allow(missing_docs)]

use ring::{digest, error, hkdf};
#[allow(deprecated)]
use ring::{test, test_file};

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::{wasm_bindgen_test as test, wasm_bindgen_test_configure};

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
wasm_bindgen_test_configure!(run_in_browser);

#[test]
fn hkdf_tests() {
    test::run(test_file!("hkdf_tests.txt"), |section, test_case| {
        assert_eq!(section, "");
        let alg = {
            let digest_alg = test_case
                .consume_digest_alg("Hash")
                .ok_or(error::Unspecified)?;
            if digest_alg == &digest::SHA256 {
                hkdf::HKDF_SHA256
            } else {
                // TODO: add test vectors for other algorithms
                panic!("unsupported algorithm: {:?}", digest_alg);
            }
        };
        let secret = test_case.consume_bytes("IKM");
        let salt = test_case.consume_bytes("salt");
        let info = test_case.consume_bytes("info");
        let _ = test_case.consume_bytes("PRK");
        let expected_out = test_case.consume_bytes("OKM");

        let salt = hkdf::Salt::new(alg, &salt);

        // TODO: test multi-part info, especially with empty parts.
        let My(out) = salt
            .extract(&secret)
            .expand(&[&info], My(expected_out.len()))
            .unwrap()
            .into();
        assert_eq!(out, expected_out);

        Ok(())
    });
}

#[test]
fn hkdf_output_len_tests() {
    for &alg in &[hkdf::HKDF_SHA256, hkdf::HKDF_SHA384, hkdf::HKDF_SHA512] {
        const MAX_BLOCKS: usize = 255;

        let salt = hkdf::Salt::new(alg, &[]);
        let prk = salt.extract(&[]); // TODO: enforce minimum length.

        {
            // Test zero length.
            let okm = prk.expand(&[b"info"], My(0)).unwrap();
            let result: My<Vec<u8>> = okm.into();
            assert_eq!(&result.0, &[]);
        }

        let max_out_len = MAX_BLOCKS * alg.hmac_algorithm().digest_algorithm().output_len();

        {
            // Test maximum length output succeeds.
            let okm = prk.expand(&[b"info"], My(max_out_len)).unwrap();
            let result: My<Vec<u8>> = okm.into();
            assert_eq!(result.0.len(), max_out_len);
        }

        {
            // Test too-large output fails.
            assert!(prk.expand(&[b"info"], My(max_out_len + 1)).is_err());
        }

        {
            // Test length mismatch (smaller).
            let okm = prk.expand(&[b"info"], My(2)).unwrap();
            let mut buf = [0u8; 1];
            assert_eq!(okm.fill(&mut buf), Err(error::Unspecified));
        }

        {
            // Test length mismatch (larger).
            let okm = prk.expand(&[b"info"], My(2)).unwrap();
            let mut buf = [0u8; 3];
            assert_eq!(okm.fill(&mut buf), Err(error::Unspecified));
        }

        {
            // Control for above two tests.
            let okm = prk.expand(&[b"info"], My(2)).unwrap();
            let mut buf = [0u8; 2];
            assert_eq!(okm.fill(&mut buf), Ok(()));
        }
    }
}

/// Generic newtype wrapper that lets us implement traits for externally-defined
/// types.
#[derive(Debug, PartialEq)]
struct My<T: core::fmt::Debug + PartialEq>(T);

impl hkdf::KeyType for My<usize> {
    fn len(&self) -> usize {
        self.0
    }
}

impl From<hkdf::Okm<'_, My<usize>>> for My<Vec<u8>> {
    fn from(okm: hkdf::Okm<My<usize>>) -> Self {
        let mut r = vec![0u8; okm.len().0];
        okm.fill(&mut r).unwrap();
        Self(r)
    }
}
