// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

//! Build and version information.

use cfg_if::cfg_if;
use openssl_macros::corresponds;
use std::ffi::CStr;

cfg_if! {
    if #[cfg(any(ossl110, libressl271))] {
        use ffi::{
            OPENSSL_VERSION, OPENSSL_CFLAGS, OPENSSL_BUILT_ON, OPENSSL_PLATFORM, OPENSSL_DIR,
            OpenSSL_version_num, OpenSSL_version,
        };
    } else {
        use ffi::{
            SSLEAY_VERSION as OPENSSL_VERSION, SSLEAY_CFLAGS as OPENSSL_CFLAGS,
            SSLEAY_BUILT_ON as OPENSSL_BUILT_ON, SSLEAY_PLATFORM as OPENSSL_PLATFORM,
            SSLEAY_DIR as OPENSSL_DIR, SSLeay as OpenSSL_version_num,
            SSLeay_version as OpenSSL_version,
        };
    }
}

/// OPENSSL_VERSION_NUMBER is a numeric release version identifier:
///
/// `MNNFFPPS: major minor fix patch status`
///
/// The status nibble has one of the values 0 for development, 1 to e for betas 1 to 14, and f for release.
///
/// for example
///
/// `0x000906000 == 0.9.6 dev`
/// `0x000906023 == 0.9.6b beta 3`
/// `0x00090605f == 0.9.6e release`
#[corresponds(OpenSSL_version_num)]
pub fn number() -> i64 {
    unsafe { OpenSSL_version_num() as i64 }
}

/// The text variant of the version number and the release date. For example, "OpenSSL 0.9.5a 1 Apr 2000".
#[corresponds(OpenSSL_version)]
pub fn version() -> &'static str {
    unsafe {
        CStr::from_ptr(OpenSSL_version(OPENSSL_VERSION))
            .to_str()
            .unwrap()
    }
}

/// The compiler flags set for the compilation process in the form "compiler: ..." if available or
/// "compiler: information not available" otherwise.
#[corresponds(OpenSSL_version)]
pub fn c_flags() -> &'static str {
    unsafe {
        CStr::from_ptr(OpenSSL_version(OPENSSL_CFLAGS))
            .to_str()
            .unwrap()
    }
}

/// The date of the build process in the form "built on: ..." if available or "built on: date not available" otherwise.
#[corresponds(OpenSSL_version)]
pub fn built_on() -> &'static str {
    unsafe {
        CStr::from_ptr(OpenSSL_version(OPENSSL_BUILT_ON))
            .to_str()
            .unwrap()
    }
}

/// The "Configure" target of the library build in the form "platform: ..." if available or "platform: information not available" otherwise.
#[corresponds(OpenSSL_version)]
pub fn platform() -> &'static str {
    unsafe {
        CStr::from_ptr(OpenSSL_version(OPENSSL_PLATFORM))
            .to_str()
            .unwrap()
    }
}

/// The "OPENSSLDIR" setting of the library build in the form "OPENSSLDIR: "..."" if available or "OPENSSLDIR: N/A" otherwise.
#[corresponds(OpenSSL_version)]
pub fn dir() -> &'static str {
    unsafe {
        CStr::from_ptr(OpenSSL_version(OPENSSL_DIR))
            .to_str()
            .unwrap()
    }
}

/// This test ensures that we do not segfault when calling the functions of this module
/// and that the strings respect a reasonable format.
#[test]
fn test_versions() {
    println!("Number: '{}'", number());
    println!("Version: '{}'", version());
    println!("C flags: '{}'", c_flags());
    println!("Built on: '{}'", built_on());
    println!("Platform: '{}'", platform());
    println!("Dir: '{}'", dir());

    #[cfg(not(any(libressl, boringssl, awslc)))]
    fn expected_name() -> &'static str {
        "OpenSSL"
    }
    #[cfg(libressl)]
    fn expected_name() -> &'static str {
        "LibreSSL"
    }
    #[cfg(boringssl)]
    fn expected_name() -> &'static str {
        "BoringSSL"
    }
    #[cfg(awslc)]
    fn expected_name() -> &'static str {
        "AWS-LC"
    }

    assert!(number() > 0);
    assert!(version().starts_with(expected_name()));
    assert!(c_flags().starts_with("compiler:"));
    // some distributions patch out dates out of openssl so that the builds are reproducible
    if !built_on().is_empty() {
        assert!(built_on().starts_with("built on:"));
    }
}
