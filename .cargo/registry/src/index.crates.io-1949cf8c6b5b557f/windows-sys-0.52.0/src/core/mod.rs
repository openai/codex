mod literals;

#[doc(hidden)]
pub use literals::*;

pub type HRESULT = i32;
pub type HSTRING = *mut ::core::ffi::c_void;
pub type IUnknown = *mut ::core::ffi::c_void;
pub type IInspectable = *mut ::core::ffi::c_void;
pub type PSTR = *mut u8;
pub type PWSTR = *mut u16;
pub type PCSTR = *const u8;
pub type PCWSTR = *const u16;
pub type BSTR = *const u16;

#[repr(C)]
pub struct GUID {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

impl ::core::marker::Copy for GUID {}

impl ::core::clone::Clone for GUID {
    fn clone(&self) -> Self {
        *self
    }
}

impl GUID {
    pub const fn from_u128(uuid: u128) -> Self {
        Self { data1: (uuid >> 96) as u32, data2: (uuid >> 80 & 0xffff) as u16, data3: (uuid >> 64 & 0xffff) as u16, data4: (uuid as u64).to_be_bytes() }
    }
}
