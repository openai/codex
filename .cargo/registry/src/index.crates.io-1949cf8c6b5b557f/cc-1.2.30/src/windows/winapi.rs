// Copyright © 2015-2017 winapi-rs developers
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// All files in the project carrying such notice may not be copied, modified, or distributed
// except according to those terms.

#![allow(bad_style, clippy::upper_case_acronyms)]

use std::os::raw;

pub type wchar_t = u16;

pub use crate::windows::windows_sys::{FILETIME, GUID, HRESULT, SAFEARRAY};

pub type REFIID = *const IID;
pub type IID = GUID;
pub type ULONG = raw::c_ulong;
pub type DWORD = u32;
pub type LPFILETIME = *mut FILETIME;
pub type OLECHAR = WCHAR;
pub type WCHAR = wchar_t;
pub type LPCOLESTR = *const OLECHAR;
pub type LCID = DWORD;
pub type LPCWSTR = *const WCHAR;
pub type PULONGLONG = *mut ULONGLONG;
pub type ULONGLONG = u64;

pub trait Interface {
    fn uuidof() -> GUID;
}

pub type LPSAFEARRAY = *mut SAFEARRAY;

macro_rules! DEFINE_GUID {
    (
        $name:ident, $l:expr, $w1:expr, $w2:expr,
        $b1:expr, $b2:expr, $b3:expr, $b4:expr, $b5:expr, $b6:expr, $b7:expr, $b8:expr
    ) => {
        pub const $name: $crate::windows::winapi::GUID = $crate::windows::winapi::GUID {
            data1: $l,
            data2: $w1,
            data3: $w2,
            data4: [$b1, $b2, $b3, $b4, $b5, $b6, $b7, $b8],
        };
    };
}

macro_rules! RIDL {
    (#[uuid($($uuid:expr),+)]
    interface $interface:ident ($vtbl:ident) {$(
        fn $method:ident($($p:ident : $t:ty,)*) -> $rtr:ty,
    )+}) => (
        #[repr(C)]
        pub struct $vtbl {
            $(pub $method: unsafe extern "system" fn(
                This: *mut $interface,
                $($p: $t),*
            ) -> $rtr,)+
        }
        #[repr(C)]
        pub struct $interface {
            pub lpVtbl: *const $vtbl,
        }
        RIDL!{@impl $interface {$(fn $method($($p: $t,)*) -> $rtr,)+}}
        RIDL!{@uuid $interface $($uuid),+}
    );
    (#[uuid($($uuid:expr),+)]
    interface $interface:ident ($vtbl:ident) : $pinterface:ident ($pvtbl:ident) {
    }) => (
        #[repr(C)]
        pub struct $vtbl {
            pub parent: $pvtbl,
        }
        #[repr(C)]
        pub struct $interface {
            pub lpVtbl: *const $vtbl,
        }
        RIDL!{@deref $interface $pinterface}
        RIDL!{@uuid $interface $($uuid),+}
    );
    (#[uuid($($uuid:expr),+)]
    interface $interface:ident ($vtbl:ident) : $pinterface:ident ($pvtbl:ident) {$(
        fn $method:ident($($p:ident : $t:ty,)*) -> $rtr:ty,
    )+}) => (
        #[repr(C)]
        pub struct $vtbl {
            pub parent: $pvtbl,
            $(pub $method: unsafe extern "system" fn(
                This: *mut $interface,
                $($p: $t,)*
            ) -> $rtr,)+
        }
        #[repr(C)]
        pub struct $interface {
            pub lpVtbl: *const $vtbl,
        }
        RIDL!{@impl $interface {$(fn $method($($p: $t,)*) -> $rtr,)+}}
        RIDL!{@deref $interface $pinterface}
        RIDL!{@uuid $interface $($uuid),+}
    );
    (@deref $interface:ident $pinterface:ident) => (
        impl ::std::ops::Deref for $interface {
            type Target = $pinterface;
            #[inline]
            fn deref(&self) -> &$pinterface {
                unsafe { &*(self as *const $interface as *const $pinterface) }
            }
        }
    );
    (@impl $interface:ident {$(
        fn $method:ident($($p:ident : $t:ty,)*) -> $rtr:ty,
    )+}) => (
        impl $interface {
            $(#[inline] pub unsafe fn $method(&self, $($p: $t,)*) -> $rtr {
                ((*self.lpVtbl).$method)(self as *const _ as *mut _, $($p,)*)
            })+
        }
    );
    (@uuid $interface:ident
        $l:expr, $w1:expr, $w2:expr,
        $b1:expr, $b2:expr, $b3:expr, $b4:expr, $b5:expr, $b6:expr, $b7:expr, $b8:expr
    ) => (
        impl $crate::windows::winapi::Interface for $interface {
            #[inline]
            fn uuidof() -> $crate::windows::winapi::GUID {
                $crate::windows::winapi::GUID {
                    data1: $l,
                    data2: $w1,
                    data3: $w2,
                    data4: [$b1, $b2, $b3, $b4, $b5, $b6, $b7, $b8],
                }
            }
        }
    );
}

RIDL! {#[uuid(0x00000000, 0x0000, 0x0000, 0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46)]
interface IUnknown(IUnknownVtbl) {
    fn QueryInterface(
        riid: REFIID,
        ppvObject: *mut *mut raw::c_void,
    ) -> HRESULT,
    fn AddRef() -> ULONG,
    fn Release() -> ULONG,
}}
