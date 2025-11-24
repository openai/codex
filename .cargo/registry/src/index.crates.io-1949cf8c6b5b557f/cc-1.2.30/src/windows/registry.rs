// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::windows::windows_sys::{
    RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, ERROR_NO_MORE_ITEMS,
    ERROR_SUCCESS, HKEY, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, REG_SZ,
};
use std::{
    ffi::{OsStr, OsString},
    io,
    ops::RangeFrom,
    os::windows::prelude::*,
    ptr::null_mut,
};

/// Must never be `HKEY_PERFORMANCE_DATA`.
pub(crate) struct RegistryKey(Repr);

#[allow(clippy::upper_case_acronyms)]
type DWORD = u32;

struct OwnedKey(HKEY);

/// Note: must not encode `HKEY_PERFORMANCE_DATA` or one of its subkeys.
enum Repr {
    /// `HKEY_LOCAL_MACHINE`.
    LocalMachine,
    /// A subkey of `HKEY_LOCAL_MACHINE`.
    Owned(OwnedKey),
}

pub struct Iter<'a> {
    idx: RangeFrom<DWORD>,
    key: &'a RegistryKey,
}

unsafe impl Sync for Repr {}
unsafe impl Send for Repr {}

pub(crate) const LOCAL_MACHINE: RegistryKey = RegistryKey(Repr::LocalMachine);

impl RegistryKey {
    fn raw(&self) -> HKEY {
        match self.0 {
            Repr::LocalMachine => HKEY_LOCAL_MACHINE,
            Repr::Owned(ref val) => val.0,
        }
    }

    /// Open a sub-key of `self`.
    pub fn open(&self, key: &OsStr) -> io::Result<RegistryKey> {
        let key = key.encode_wide().chain(Some(0)).collect::<Vec<_>>();
        let mut ret = null_mut();
        let err = unsafe {
            RegOpenKeyExW(
                self.raw(),
                key.as_ptr(),
                0,
                KEY_READ | KEY_WOW64_32KEY,
                &mut ret,
            )
        };
        if err == ERROR_SUCCESS {
            Ok(RegistryKey(Repr::Owned(OwnedKey(ret))))
        } else {
            Err(io::Error::from_raw_os_error(err as i32))
        }
    }

    pub fn iter(&self) -> Iter {
        Iter {
            idx: 0..,
            key: self,
        }
    }

    pub fn query_str(&self, name: &str) -> io::Result<OsString> {
        let name: &OsStr = name.as_ref();
        let name = name.encode_wide().chain(Some(0)).collect::<Vec<_>>();
        let mut len = 0;
        let mut kind = 0;
        unsafe {
            let err = RegQueryValueExW(
                self.raw(),
                name.as_ptr(),
                null_mut(),
                &mut kind,
                null_mut(),
                &mut len,
            );
            if err != ERROR_SUCCESS {
                return Err(io::Error::from_raw_os_error(err as i32));
            }
            if kind != REG_SZ {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "registry key wasn't a string",
                ));
            }

            // The length here is the length in bytes, but we're using wide
            // characters so we need to be sure to halve it for the length
            // passed in.
            assert!(len % 2 == 0, "impossible wide string size: {} bytes", len);
            let vlen = len as usize / 2;
            // Defensively initialized, see comment about
            // `HKEY_PERFORMANCE_DATA` below.
            let mut v = vec![0u16; vlen];
            let err = RegQueryValueExW(
                self.raw(),
                name.as_ptr(),
                null_mut(),
                null_mut(),
                v.as_mut_ptr() as *mut _,
                &mut len,
            );
            // We don't check for `ERROR_MORE_DATA` (which would if the value
            // grew between the first and second call to `RegQueryValueExW`),
            // both because it's extremely unlikely, and this is a bit more
            // defensive more defensive against weird types of registry keys.
            if err != ERROR_SUCCESS {
                return Err(io::Error::from_raw_os_error(err as i32));
            }
            // The length is allowed to change, but should still be even, as
            // well as smaller.
            assert!(len % 2 == 0, "impossible wide string size: {} bytes", len);
            // If the length grew but returned a success code, it *probably*
            // indicates we're `HKEY_PERFORMANCE_DATA` or a subkey(?). We
            // consider this UB, since those keys write "undefined" or
            // "unpredictable" values to len, and need to use a completely
            // different loop structure. This should be impossible (and enforce
            // it in the API to the best of our ability), but to mitigate the
            // damage we do some smoke-checks on the len, and ensure `v` has
            // been fully initialized (rather than trusting the result of
            // `RegQueryValueExW`).
            let actual_len = len as usize / 2;
            assert!(actual_len <= v.len());
            v.truncate(actual_len);
            // Some registry keys may have a terminating nul character, but
            // we're not interested in that, so chop it off if it's there.
            if !v.is_empty() && v[v.len() - 1] == 0 {
                v.pop();
            }
            Ok(OsString::from_wide(&v))
        }
    }
}

impl Drop for OwnedKey {
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.0);
        }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = io::Result<OsString>;

    fn next(&mut self) -> Option<io::Result<OsString>> {
        self.idx.next().and_then(|i| unsafe {
            let mut v = Vec::with_capacity(256);
            let mut len = v.capacity() as DWORD;
            let ret = RegEnumKeyExW(
                self.key.raw(),
                i,
                v.as_mut_ptr(),
                &mut len,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
            );
            if ret == ERROR_NO_MORE_ITEMS {
                None
            } else if ret != ERROR_SUCCESS {
                Some(Err(io::Error::from_raw_os_error(ret as i32)))
            } else {
                v.set_len(len as usize);
                Some(Ok(OsString::from_wide(&v)))
            }
        })
    }
}
