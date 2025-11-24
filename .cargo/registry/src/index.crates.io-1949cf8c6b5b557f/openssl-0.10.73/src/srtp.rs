use crate::stack::Stackable;
use foreign_types::ForeignTypeRef;
use libc::c_ulong;
use std::ffi::CStr;
use std::str;

/// fake free method, since SRTP_PROTECTION_PROFILE is static
unsafe fn free(_profile: *mut ffi::SRTP_PROTECTION_PROFILE) {}

foreign_type_and_impl_send_sync! {
    type CType = ffi::SRTP_PROTECTION_PROFILE;
    fn drop = free;

    pub struct SrtpProtectionProfile;
    /// Reference to `SrtpProtectionProfile`.
    pub struct SrtpProtectionProfileRef;
}

impl Stackable for SrtpProtectionProfile {
    type StackType = ffi::stack_st_SRTP_PROTECTION_PROFILE;
}

impl SrtpProtectionProfileRef {
    pub fn id(&self) -> SrtpProfileId {
        SrtpProfileId::from_raw(unsafe { (*self.as_ptr()).id })
    }
    pub fn name(&self) -> &'static str {
        unsafe { CStr::from_ptr((*self.as_ptr()).name as *const _) }
            .to_str()
            .expect("should be UTF-8")
    }
}

/// An identifier of an SRTP protection profile.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SrtpProfileId(c_ulong);

impl SrtpProfileId {
    pub const SRTP_AES128_CM_SHA1_80: SrtpProfileId =
        SrtpProfileId(ffi::SRTP_AES128_CM_SHA1_80 as c_ulong);
    pub const SRTP_AES128_CM_SHA1_32: SrtpProfileId =
        SrtpProfileId(ffi::SRTP_AES128_CM_SHA1_32 as c_ulong);
    pub const SRTP_AES128_F8_SHA1_80: SrtpProfileId =
        SrtpProfileId(ffi::SRTP_AES128_F8_SHA1_80 as c_ulong);
    pub const SRTP_AES128_F8_SHA1_32: SrtpProfileId =
        SrtpProfileId(ffi::SRTP_AES128_F8_SHA1_32 as c_ulong);
    pub const SRTP_NULL_SHA1_80: SrtpProfileId = SrtpProfileId(ffi::SRTP_NULL_SHA1_80 as c_ulong);
    pub const SRTP_NULL_SHA1_32: SrtpProfileId = SrtpProfileId(ffi::SRTP_NULL_SHA1_32 as c_ulong);
    #[cfg(any(boringssl, ossl110, awslc))]
    pub const SRTP_AEAD_AES_128_GCM: SrtpProfileId =
        SrtpProfileId(ffi::SRTP_AEAD_AES_128_GCM as c_ulong);
    #[cfg(any(boringssl, ossl110, awslc))]
    pub const SRTP_AEAD_AES_256_GCM: SrtpProfileId =
        SrtpProfileId(ffi::SRTP_AEAD_AES_256_GCM as c_ulong);

    /// Creates a `SrtpProfileId` from an integer representation.
    pub fn from_raw(value: c_ulong) -> SrtpProfileId {
        SrtpProfileId(value)
    }

    /// Returns the integer representation of `SrtpProfileId`.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn as_raw(&self) -> c_ulong {
        self.0
    }
}
