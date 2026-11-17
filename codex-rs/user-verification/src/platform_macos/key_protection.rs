//! Create biometric signing policies and validate Secure Enclave key attributes.

use crate::UserVerificationError;
use crate::UserVerificationUnavailableReason;
use core_foundation::base::CFEqual;
use core_foundation::base::TCFType as _;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use security_framework::access_control::ProtectionMode;
use security_framework::access_control::SecAccessControl;
use security_framework::base::Error as SecurityError;
use security_framework::key::SecKey;
use security_framework_sys::access_control::kSecAccessControlBiometryAny;
use security_framework_sys::access_control::kSecAccessControlPrivateKeyUsage;
use security_framework_sys::item::kSecAttrKeyClass;
use security_framework_sys::item::kSecAttrKeyClassPrivate;
use security_framework_sys::item::kSecAttrKeySizeInBits;
use security_framework_sys::item::kSecAttrKeyType;
use security_framework_sys::item::kSecAttrKeyTypeECSECPrimeRandom;
use security_framework_sys::item::kSecAttrTokenID;
use security_framework_sys::item::kSecAttrTokenIDSecureEnclave;

pub(super) fn access_control() -> Result<SecAccessControl, SecurityError> {
    SecAccessControl::create_with_protection(
        Some(ProtectionMode::AccessibleWhenUnlockedThisDeviceOnly),
        kSecAccessControlBiometryAny | kSecAccessControlPrivateKeyUsage,
    )
}

pub(super) fn validate(key: &SecKey) -> Result<(), UserVerificationError> {
    validate_attributes(&key.attributes())
}

fn validate_attributes(attributes: &CFDictionary) -> Result<(), UserVerificationError> {
    let size = CFNumber::from(/*value*/ 256);
    // Do not compare opaque SecAccessControl objects: macOS can change their persisted
    // representation. Biometric signing is enforced by the policy set at key creation.
    // Reuse trusts that keys in this namespace, within the process's entitled keychain
    // access groups, were created with that policy; these checks do not revalidate it.
    let expected = unsafe {
        [
            (kSecAttrTokenID, kSecAttrTokenIDSecureEnclave.cast()),
            (kSecAttrKeyClass, kSecAttrKeyClassPrivate.cast()),
            (kSecAttrKeyType, kSecAttrKeyTypeECSECPrimeRandom.cast()),
            (kSecAttrKeySizeInBits, size.as_CFTypeRef()),
        ]
    };
    if expected.into_iter().all(|(name, expected)| {
        attributes
            .find(name.cast())
            .is_some_and(|value| unsafe { CFEqual(*value, expected) != 0 })
    }) {
        Ok(())
    } else {
        Err(UserVerificationError::Unavailable {
            reason: UserVerificationUnavailableReason::ProviderUnavailable,
            message: "the local credential has incompatible key protection; delete it before enrolling again"
                .to_string(),
        })
    }
}

#[cfg(test)]
#[path = "key_protection_tests.rs"]
mod tests;
