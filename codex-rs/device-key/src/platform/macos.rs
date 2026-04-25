use crate::DeviceKeyAlgorithm;
use crate::DeviceKeyError;
use crate::DeviceKeyInfo;
use crate::DeviceKeyProtectionClass;
use crate::DeviceKeyProvider;
use crate::ProviderCreateRequest;
use crate::ProviderSignature;
use crate::sec1_public_key_to_spki_der;
use std::ffi::CStr;
use std::ffi::CString;
use std::ffi::c_char;
use std::ffi::c_int;
use std::ptr;
use std::slice;

const MAC_STATUS_OK: c_int = 0;
const MAC_STATUS_NOT_FOUND: c_int = 1;
const MAC_STATUS_HARDWARE_UNAVAILABLE: c_int = 2;

const MAC_KEY_CLASS_SECURE_ENCLAVE: c_int = 0;
const MAC_KEY_CLASS_OS_PROTECTED_NONEXTRACTABLE: c_int = 1;

#[repr(C)]
struct MacBytesResult {
    status: c_int,
    data: *mut u8,
    len: usize,
    error_message: *mut c_char,
}

unsafe extern "C" {
    fn codex_device_key_macos_create_or_load_public_key(
        key_tag: *const c_char,
        key_class: c_int,
    ) -> MacBytesResult;
    fn codex_device_key_macos_load_public_key(
        key_tag: *const c_char,
        key_class: c_int,
    ) -> MacBytesResult;
    fn codex_device_key_macos_delete(key_tag: *const c_char, key_class: c_int) -> MacBytesResult;
    fn codex_device_key_macos_sign(
        key_tag: *const c_char,
        key_class: c_int,
        payload: *const u8,
        payload_len: usize,
    ) -> MacBytesResult;
    fn codex_device_key_macos_free_bytes_result(result: *mut MacBytesResult);
}

impl MacBytesResult {
    fn into_bytes(mut self) -> Result<Vec<u8>, DeviceKeyError> {
        let result = match self.status {
            MAC_STATUS_OK => {
                if self.data.is_null() && self.len != 0 {
                    Err(DeviceKeyError::Platform(
                        "macOS device-key provider returned null data".to_string(),
                    ))
                } else {
                    let bytes = if self.len == 0 {
                        Vec::new()
                    } else {
                        unsafe { slice::from_raw_parts(self.data.cast_const(), self.len).to_vec() }
                    };
                    Ok(bytes)
                }
            }
            MAC_STATUS_NOT_FOUND => Err(DeviceKeyError::KeyNotFound),
            MAC_STATUS_HARDWARE_UNAVAILABLE => Err(DeviceKeyError::HardwareBackedKeysUnavailable),
            _ => Err(DeviceKeyError::Platform(self.error_message())),
        };
        unsafe {
            codex_device_key_macos_free_bytes_result(ptr::addr_of_mut!(self));
        }
        result
    }

    fn error_message(&self) -> String {
        if self.error_message.is_null() {
            return "unknown macOS device-key provider error".to_string();
        }
        unsafe { CStr::from_ptr(self.error_message) }
            .to_string_lossy()
            .into_owned()
    }
}

#[derive(Debug)]
pub(crate) struct MacOsDeviceKeyProvider;

impl DeviceKeyProvider for MacOsDeviceKeyProvider {
    fn create(&self, request: ProviderCreateRequest) -> Result<DeviceKeyInfo, DeviceKeyError> {
        let secure_enclave_key_id =
            request.key_id_for(DeviceKeyProtectionClass::HardwareSecureEnclave);
        match create_or_load_key_info(&secure_enclave_key_id, MacKeyClass::SecureEnclave) {
            Ok(info) => Ok(info),
            Err(secure_enclave_error) => {
                if !matches!(
                    secure_enclave_error,
                    DeviceKeyError::HardwareBackedKeysUnavailable
                ) {
                    return Err(secure_enclave_error);
                }
                if !request
                    .protection_policy
                    .allows(DeviceKeyProtectionClass::OsProtectedNonextractable)
                {
                    return Err(DeviceKeyError::DegradedProtectionNotAllowed {
                        available: DeviceKeyProtectionClass::OsProtectedNonextractable,
                    });
                }
                let fallback_key_id =
                    request.key_id_for(DeviceKeyProtectionClass::OsProtectedNonextractable);
                create_or_load_key_info(&fallback_key_id, MacKeyClass::OsProtectedNonextractable)
                    .map_err(|fallback_error| {
                        DeviceKeyError::Platform(format!(
                            "Secure Enclave key creation failed ({secure_enclave_error}); OS-protected fallback failed ({fallback_error})"
                        ))
                    })
            }
        }
    }

    fn delete(
        &self,
        key_id: &str,
        protection_class: DeviceKeyProtectionClass,
    ) -> Result<(), DeviceKeyError> {
        let class = MacKeyClass::from_protection_class(protection_class)
            .ok_or(DeviceKeyError::KeyNotFound)?;
        delete_key(key_id, class)
    }

    fn get_public(
        &self,
        key_id: &str,
        protection_class: DeviceKeyProtectionClass,
    ) -> Result<DeviceKeyInfo, DeviceKeyError> {
        let class = MacKeyClass::from_protection_class(protection_class)
            .ok_or(DeviceKeyError::KeyNotFound)?;
        let public_key = load_public_key(key_id, class)?;
        key_info(key_id, class, public_key.as_slice())
    }

    fn sign(
        &self,
        key_id: &str,
        protection_class: DeviceKeyProtectionClass,
        payload: &[u8],
    ) -> Result<ProviderSignature, DeviceKeyError> {
        let class = MacKeyClass::from_protection_class(protection_class)
            .ok_or(DeviceKeyError::KeyNotFound)?;
        let signature_der = sign(key_id, class, payload)?;
        Ok(ProviderSignature {
            signature_der,
            algorithm: DeviceKeyAlgorithm::EcdsaP256Sha256,
        })
    }
}

#[derive(Debug, Clone, Copy)]
enum MacKeyClass {
    SecureEnclave,
    OsProtectedNonextractable,
}

impl MacKeyClass {
    fn native(self) -> c_int {
        match self {
            Self::SecureEnclave => MAC_KEY_CLASS_SECURE_ENCLAVE,
            Self::OsProtectedNonextractable => MAC_KEY_CLASS_OS_PROTECTED_NONEXTRACTABLE,
        }
    }

    fn protection_class(self) -> DeviceKeyProtectionClass {
        match self {
            Self::SecureEnclave => DeviceKeyProtectionClass::HardwareSecureEnclave,
            Self::OsProtectedNonextractable => DeviceKeyProtectionClass::OsProtectedNonextractable,
        }
    }

    fn tag_prefix(self) -> &'static str {
        match self {
            Self::SecureEnclave => "secure-enclave",
            Self::OsProtectedNonextractable => "os-protected-nonextractable",
        }
    }

    fn from_protection_class(protection_class: DeviceKeyProtectionClass) -> Option<Self> {
        match protection_class {
            DeviceKeyProtectionClass::HardwareSecureEnclave => Some(Self::SecureEnclave),
            DeviceKeyProtectionClass::OsProtectedNonextractable => {
                Some(Self::OsProtectedNonextractable)
            }
            DeviceKeyProtectionClass::HardwareTpm => None,
        }
    }
}

fn create_or_load_key_info(
    key_id: &str,
    class: MacKeyClass,
) -> Result<DeviceKeyInfo, DeviceKeyError> {
    let public_key = create_or_load_public_key(key_id, class)?;
    key_info(key_id, class, public_key.as_slice())
}

fn create_or_load_public_key(key_id: &str, class: MacKeyClass) -> Result<Vec<u8>, DeviceKeyError> {
    let tag = key_tag_cstring(key_id, class)?;
    unsafe { codex_device_key_macos_create_or_load_public_key(tag.as_ptr(), class.native()) }
        .into_bytes()
}

fn load_public_key(key_id: &str, class: MacKeyClass) -> Result<Vec<u8>, DeviceKeyError> {
    let tag = key_tag_cstring(key_id, class)?;
    unsafe { codex_device_key_macos_load_public_key(tag.as_ptr(), class.native()) }.into_bytes()
}

fn delete_key(key_id: &str, class: MacKeyClass) -> Result<(), DeviceKeyError> {
    let tag = key_tag_cstring(key_id, class)?;
    unsafe { codex_device_key_macos_delete(tag.as_ptr(), class.native()) }
        .into_bytes()
        .map(|_| ())
}

fn sign(key_id: &str, class: MacKeyClass, payload: &[u8]) -> Result<Vec<u8>, DeviceKeyError> {
    let tag = key_tag_cstring(key_id, class)?;
    unsafe {
        codex_device_key_macos_sign(
            tag.as_ptr(),
            class.native(),
            payload.as_ptr(),
            payload.len(),
        )
    }
    .into_bytes()
}

fn key_info(
    key_id: &str,
    class: MacKeyClass,
    sec1_public_key: &[u8],
) -> Result<DeviceKeyInfo, DeviceKeyError> {
    Ok(DeviceKeyInfo {
        key_id: key_id.to_string(),
        public_key_spki_der: sec1_public_key_to_spki_der(sec1_public_key)?,
        algorithm: DeviceKeyAlgorithm::EcdsaP256Sha256,
        protection_class: class.protection_class(),
    })
}

fn key_tag_cstring(key_id: &str, class: MacKeyClass) -> Result<CString, DeviceKeyError> {
    CString::new(key_tag(key_id, class)).map_err(|err| DeviceKeyError::Platform(err.to_string()))
}

fn key_tag(key_id: &str, class: MacKeyClass) -> String {
    format!(
        "com.openai.codex.device-key.{}.{}",
        class.tag_prefix(),
        key_id
    )
}
