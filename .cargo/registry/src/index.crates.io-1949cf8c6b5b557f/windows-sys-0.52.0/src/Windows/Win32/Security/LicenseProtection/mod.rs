::windows_targets::link!("licenseprotection.dll" "system" fn RegisterLicenseKeyWithExpiration(licensekey : ::windows_sys::core::PCWSTR, validityindays : u32, status : *mut LicenseProtectionStatus) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("licenseprotection.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ValidateLicenseKeyProtection(licensekey : ::windows_sys::core::PCWSTR, notvalidbefore : *mut super::super::Foundation:: FILETIME, notvalidafter : *mut super::super::Foundation:: FILETIME, status : *mut LicenseProtectionStatus) -> ::windows_sys::core::HRESULT);
pub const LicenseKeyAlreadyExists: LicenseProtectionStatus = 4i32;
pub const LicenseKeyCorrupted: LicenseProtectionStatus = 3i32;
pub const LicenseKeyNotFound: LicenseProtectionStatus = 1i32;
pub const LicenseKeyUnprotected: LicenseProtectionStatus = 2i32;
pub const Success: LicenseProtectionStatus = 0i32;
pub type LicenseProtectionStatus = i32;
