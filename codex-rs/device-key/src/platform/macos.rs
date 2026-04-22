use crate::DeviceKeyAlgorithm;
use crate::DeviceKeyError;
use crate::DeviceKeyInfo;
use crate::DeviceKeyProtectionClass;
use crate::DeviceKeyProvider;
use crate::ProviderCreateRequest;
use crate::ProviderSignature;
use crate::sec1_public_key_to_spki_der;
use core_foundation::base::TCFType;
use core_foundation::base::ToVoid;
use core_foundation::boolean::CFBoolean;
use core_foundation::data::CFData;
use core_foundation::dictionary::CFMutableDictionary;
use core_foundation::error::CFError;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_foundation_sys::base::CFTypeRef;
use core_foundation_sys::string::CFStringRef;
use security_framework::access_control::ProtectionMode;
use security_framework::access_control::SecAccessControl;
use security_framework::key::Algorithm;
use security_framework::key::SecKey;
use security_framework_sys::access_control::kSecAccessControlPrivateKeyUsage;
use security_framework_sys::access_control::kSecAccessControlUserPresence;
use security_framework_sys::base::errSecItemNotFound;
use security_framework_sys::base::errSecParam;
use security_framework_sys::base::errSecSuccess;
use security_framework_sys::base::errSecUnimplemented;
use security_framework_sys::item::kSecAttrAccessControl;
use security_framework_sys::item::kSecAttrIsPermanent;
use security_framework_sys::item::kSecAttrKeyClass;
use security_framework_sys::item::kSecAttrKeyClassPrivate;
use security_framework_sys::item::kSecAttrKeySizeInBits;
use security_framework_sys::item::kSecAttrKeyType;
use security_framework_sys::item::kSecAttrKeyTypeECSECPrimeRandom;
use security_framework_sys::item::kSecAttrLabel;
use security_framework_sys::item::kSecAttrTokenID;
use security_framework_sys::item::kSecAttrTokenIDSecureEnclave;
use security_framework_sys::item::kSecClass;
use security_framework_sys::item::kSecClassKey;
use security_framework_sys::item::kSecPrivateKeyAttrs;
use security_framework_sys::item::kSecReturnRef;
use security_framework_sys::keychain_item::SecItemCopyMatching;
use std::ffi::c_char;
use std::ffi::c_double;
use std::ffi::c_void;
use std::ptr;
use std::sync::Mutex;
use std::sync::OnceLock;

#[allow(non_upper_case_globals)]
unsafe extern "C" {
    static kSecAttrApplicationTag: CFStringRef;
    static kSecAttrIsExtractable: CFStringRef;
    static kSecUseAuthenticationContext: CFStringRef;
}

unsafe extern "C" {
    fn dlopen(path: *const c_char, mode: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

#[link(name = "objc")]
unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> ObjcId;
    fn sel_registerName(name: *const c_char) -> ObjcSel;
}

type ObjcId = *mut c_void;
type ObjcSel = *mut c_void;

const LOCAL_AUTHENTICATION_FRAMEWORK_PATH: &[u8] =
    b"/System/Library/Frameworks/LocalAuthentication.framework/LocalAuthentication\0";
const LA_CONTEXT_CLASS: &[u8] = b"LAContext\0";
const OBJC_MSG_SEND_SYMBOL: &[u8] = b"objc_msgSend\0";
const OBJC_ALLOC_SELECTOR: &[u8] = b"alloc\0";
const OBJC_INIT_SELECTOR: &[u8] = b"init\0";
const OBJC_RELEASE_SELECTOR: &[u8] = b"release\0";
const SET_TOUCH_ID_AUTHENTICATION_REUSE_DURATION_SELECTOR: &[u8] =
    b"setTouchIDAuthenticationAllowableReuseDuration:\0";
const TOUCH_ID_AUTHENTICATION_REUSE_DURATION_SECONDS: c_double = 300.0;
const RTLD_LAZY: i32 = 0x1;
const RTLD_LOCAL: i32 = 0x4;

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

    fn get_public(
        &self,
        key_id: &str,
        protection_class: DeviceKeyProtectionClass,
    ) -> Result<DeviceKeyInfo, DeviceKeyError> {
        let class = MacKeyClass::from_protection_class(protection_class)
            .ok_or(DeviceKeyError::KeyNotFound)?;
        let key = load_private_key(key_id, class)?.ok_or(DeviceKeyError::KeyNotFound)?;
        key_info(key_id, class, &key)
    }

    fn sign(
        &self,
        key_id: &str,
        protection_class: DeviceKeyProtectionClass,
        payload: &[u8],
    ) -> Result<ProviderSignature, DeviceKeyError> {
        let class = MacKeyClass::from_protection_class(protection_class)
            .ok_or(DeviceKeyError::KeyNotFound)?;
        let context = reusable_authentication_context()?;
        let context = context
            .lock()
            .map_err(|err| DeviceKeyError::Platform(format!("LAContext mutex poisoned: {err}")))?;
        let key = load_private_key_with_authentication_context(key_id, class, &context)?
            .ok_or(DeviceKeyError::KeyNotFound)?;
        let signature_der = key
            .create_signature(Algorithm::ECDSASignatureMessageX962SHA256, payload)
            .map_err(|err| DeviceKeyError::Platform(err.to_string()))?;
        Ok(ProviderSignature {
            signature_der,
            algorithm: DeviceKeyAlgorithm::EcdsaP256Sha256,
        })
    }
}

struct LocalAuthenticationContext {
    context: ObjcId,
}

unsafe impl Send for LocalAuthenticationContext {}

impl LocalAuthenticationContext {
    fn new() -> Result<Self, DeviceKeyError> {
        load_local_authentication_framework()?;
        let class = unsafe { objc_getClass(LA_CONTEXT_CLASS.as_ptr().cast::<c_char>()) };
        if class.is_null() {
            return Err(DeviceKeyError::Platform(
                "LocalAuthentication.framework did not provide LAContext".to_string(),
            ));
        }

        let allocated = unsafe { objc_msg_send_id(class, sel(OBJC_ALLOC_SELECTOR))? };
        if allocated.is_null() {
            return Err(DeviceKeyError::Platform(
                "LAContext allocation returned null".to_string(),
            ));
        }

        let context = unsafe { objc_msg_send_id(allocated, sel(OBJC_INIT_SELECTOR))? };
        if context.is_null() {
            return Err(DeviceKeyError::Platform(
                "LAContext initialization returned null".to_string(),
            ));
        }

        unsafe {
            objc_msg_send_void_f64(
                context,
                sel(SET_TOUCH_ID_AUTHENTICATION_REUSE_DURATION_SELECTOR),
                TOUCH_ID_AUTHENTICATION_REUSE_DURATION_SECONDS,
            )?;
        }
        Ok(Self { context })
    }

    fn as_void(&self) -> *const c_void {
        self.context.cast_const()
    }
}

impl Drop for LocalAuthenticationContext {
    fn drop(&mut self) {
        unsafe {
            let _ = objc_msg_send_void(self.context, sel(OBJC_RELEASE_SELECTOR));
        }
    }
}

fn reusable_authentication_context()
-> Result<&'static Mutex<LocalAuthenticationContext>, DeviceKeyError> {
    static AUTHENTICATION_CONTEXT: OnceLock<Mutex<LocalAuthenticationContext>> = OnceLock::new();

    if let Some(context) = AUTHENTICATION_CONTEXT.get() {
        return Ok(context);
    }

    let context = LocalAuthenticationContext::new()?;
    if AUTHENTICATION_CONTEXT.set(Mutex::new(context)).is_err() {
        return AUTHENTICATION_CONTEXT.get().ok_or_else(|| {
            DeviceKeyError::Platform(
                "LAContext initialization raced but no context won".to_string(),
            )
        });
    }
    AUTHENTICATION_CONTEXT.get().ok_or_else(|| {
        DeviceKeyError::Platform("LAContext was not stored after initialization".to_string())
    })
}

fn sel(name: &'static [u8]) -> ObjcSel {
    unsafe { sel_registerName(name.as_ptr().cast::<c_char>()) }
}

fn load_local_authentication_framework() -> Result<(), DeviceKeyError> {
    let handle = unsafe {
        dlopen(
            LOCAL_AUTHENTICATION_FRAMEWORK_PATH
                .as_ptr()
                .cast::<c_char>(),
            RTLD_LAZY | RTLD_LOCAL,
        )
    };
    if handle.is_null() {
        Err(DeviceKeyError::Platform(
            "failed to load LocalAuthentication.framework".to_string(),
        ))
    } else {
        Ok(())
    }
}

unsafe fn objc_msg_send_id(receiver: ObjcId, selector: ObjcSel) -> Result<ObjcId, DeviceKeyError> {
    let msg_send: unsafe extern "C" fn(ObjcId, ObjcSel) -> ObjcId =
        unsafe { std::mem::transmute(objc_msg_send_symbol()?) };
    Ok(unsafe { msg_send(receiver, selector) })
}

unsafe fn objc_msg_send_void(receiver: ObjcId, selector: ObjcSel) -> Result<(), DeviceKeyError> {
    let msg_send: unsafe extern "C" fn(ObjcId, ObjcSel) =
        unsafe { std::mem::transmute(objc_msg_send_symbol()?) };
    unsafe { msg_send(receiver, selector) };
    Ok(())
}

unsafe fn objc_msg_send_void_f64(
    receiver: ObjcId,
    selector: ObjcSel,
    value: c_double,
) -> Result<(), DeviceKeyError> {
    let msg_send: unsafe extern "C" fn(ObjcId, ObjcSel, c_double) =
        unsafe { std::mem::transmute(objc_msg_send_symbol()?) };
    unsafe { msg_send(receiver, selector, value) };
    Ok(())
}

fn objc_msg_send_symbol() -> Result<*mut c_void, DeviceKeyError> {
    let symbol = unsafe {
        dlsym(
            rtld_default(),
            OBJC_MSG_SEND_SYMBOL.as_ptr().cast::<c_char>(),
        )
    };
    if symbol.is_null() {
        Err(DeviceKeyError::Platform(
            "objc_msgSend lookup returned null".to_string(),
        ))
    } else {
        Ok(symbol)
    }
}

fn rtld_default() -> *mut c_void {
    (-2_isize) as *mut c_void
}

#[derive(Debug, Clone, Copy)]
enum MacKeyClass {
    SecureEnclave,
    OsProtectedNonextractable,
}

impl MacKeyClass {
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

fn load_private_key(key_id: &str, class: MacKeyClass) -> Result<Option<SecKey>, DeviceKeyError> {
    load_private_key_with_optional_authentication_context(
        key_id, class, /*authentication_context*/ None,
    )
}

fn load_private_key_with_authentication_context(
    key_id: &str,
    class: MacKeyClass,
    authentication_context: &LocalAuthenticationContext,
) -> Result<Option<SecKey>, DeviceKeyError> {
    load_private_key_with_optional_authentication_context(
        key_id,
        class,
        Some(authentication_context),
    )
}

fn load_private_key_with_optional_authentication_context(
    key_id: &str,
    class: MacKeyClass,
    authentication_context: Option<&LocalAuthenticationContext>,
) -> Result<Option<SecKey>, DeviceKeyError> {
    let tag = key_tag(key_id, class);
    let tag = CFData::from_buffer(tag.as_bytes());
    let mut query = unsafe {
        CFMutableDictionary::from_CFType_pairs(&[
            (kSecClass.to_void(), kSecClassKey.to_void()),
            (
                kSecAttrKeyClass.to_void(),
                kSecAttrKeyClassPrivate.to_void(),
            ),
            (kSecAttrApplicationTag.to_void(), tag.to_void()),
            (kSecReturnRef.to_void(), CFBoolean::true_value().to_void()),
        ])
    };
    if matches!(class, MacKeyClass::SecureEnclave) {
        unsafe {
            query.add(
                &kSecAttrTokenID.to_void(),
                &kSecAttrTokenIDSecureEnclave.to_void(),
            );
        }
    }
    if matches!(class, MacKeyClass::OsProtectedNonextractable) {
        unsafe {
            query.add(
                &kSecAttrIsExtractable.to_void(),
                &CFBoolean::false_value().to_void(),
            );
        }
    }
    if let Some(authentication_context) = authentication_context {
        unsafe {
            query.add(
                &kSecUseAuthenticationContext.to_void(),
                &authentication_context.as_void(),
            );
        }
    }

    let mut result: CFTypeRef = ptr::null();
    let status = unsafe { SecItemCopyMatching(query.as_concrete_TypeRef(), &mut result) };
    if status == errSecItemNotFound {
        return Ok(None);
    }
    if status != errSecSuccess {
        return Err(DeviceKeyError::Platform(security_error(status)));
    }
    if result.is_null() {
        return Err(DeviceKeyError::Platform(
            "Security.framework returned an empty key reference".to_string(),
        ));
    }
    Ok(Some(unsafe {
        SecKey::wrap_under_create_rule(result as *mut _)
    }))
}

fn create_or_load_key_info(
    key_id: &str,
    class: MacKeyClass,
) -> Result<DeviceKeyInfo, DeviceKeyError> {
    let key = create_or_load_private_key(key_id, class)?;
    key_info(key_id, class, &key)
}

fn create_or_load_private_key(key_id: &str, class: MacKeyClass) -> Result<SecKey, DeviceKeyError> {
    match create_private_key(key_id, class) {
        Ok(key) => Ok(key),
        Err(create_error) => match load_private_key(key_id, class) {
            Ok(Some(key)) => Ok(key),
            Ok(None) => Err(create_error),
            Err(load_error) => Err(DeviceKeyError::Platform(format!(
                "key creation failed ({create_error}); reload failed ({load_error})"
            ))),
        },
    }
}

/// Creates a macOS this-device-only P-256 signing key.
///
/// The access-control flags below keep the private key local to this device and require
/// Security.framework to prove user presence before private-key use. The signing path also passes a
/// process-local `LAContext` so successful biometric/password authentication can be reused for later
/// signatures when macOS policy allows it.
#[allow(deprecated)]
fn create_private_key(key_id: &str, class: MacKeyClass) -> Result<SecKey, DeviceKeyError> {
    let access_control = SecAccessControl::create_with_protection(
        Some(ProtectionMode::AccessibleWhenUnlockedThisDeviceOnly),
        kSecAccessControlPrivateKeyUsage | kSecAccessControlUserPresence,
    )
    .map_err(|err| DeviceKeyError::Platform(err.to_string()))?;
    let tag = key_tag(key_id, class);
    let tag_data = CFData::from_buffer(tag.as_bytes());
    let label = CFString::new(&tag);
    let key_size = CFNumber::from(256);
    let mut private_attrs = unsafe {
        CFMutableDictionary::from_CFType_pairs(&[
            (
                kSecAttrIsPermanent.to_void(),
                CFBoolean::true_value().to_void(),
            ),
            (kSecAttrAccessControl.to_void(), access_control.to_void()),
            (kSecAttrApplicationTag.to_void(), tag_data.to_void()),
            (kSecAttrLabel.to_void(), label.to_void()),
        ])
    };
    if matches!(class, MacKeyClass::OsProtectedNonextractable) {
        unsafe {
            private_attrs.add(
                &kSecAttrIsExtractable.to_void(),
                &CFBoolean::false_value().to_void(),
            );
        }
    }

    let mut attributes = unsafe {
        CFMutableDictionary::from_CFType_pairs(&[
            (
                kSecAttrKeyType.to_void(),
                kSecAttrKeyTypeECSECPrimeRandom.to_void(),
            ),
            (kSecAttrKeySizeInBits.to_void(), key_size.to_void()),
            (kSecAttrLabel.to_void(), label.to_void()),
            (kSecPrivateKeyAttrs.to_void(), private_attrs.to_void()),
        ])
    };
    if matches!(class, MacKeyClass::SecureEnclave) {
        unsafe {
            attributes.add(
                &kSecAttrTokenID.to_void(),
                &kSecAttrTokenIDSecureEnclave.to_void(),
            );
        }
    }

    SecKey::generate(attributes.to_immutable()).map_err(|err| create_key_error(class, err))
}

fn create_key_error(class: MacKeyClass, error: CFError) -> DeviceKeyError {
    let code = error.code() as i32;
    if matches!(class, MacKeyClass::SecureEnclave)
        && (code == errSecUnimplemented || code == errSecParam)
    {
        return DeviceKeyError::HardwareBackedKeysUnavailable;
    }

    DeviceKeyError::Platform(error.description().to_string())
}

fn key_info(
    key_id: &str,
    class: MacKeyClass,
    private_key: &SecKey,
) -> Result<DeviceKeyInfo, DeviceKeyError> {
    let public_key = private_key.public_key().ok_or_else(|| {
        DeviceKeyError::Platform("Security.framework did not return a public key".to_string())
    })?;
    let public_key = public_key.external_representation().ok_or_else(|| {
        DeviceKeyError::Platform(
            "Security.framework did not return an exportable public key".to_string(),
        )
    })?;
    Ok(DeviceKeyInfo {
        key_id: key_id.to_string(),
        public_key_spki_der: sec1_public_key_to_spki_der(&public_key)?,
        algorithm: DeviceKeyAlgorithm::EcdsaP256Sha256,
        protection_class: class.protection_class(),
    })
}

fn key_tag(key_id: &str, class: MacKeyClass) -> String {
    format!(
        "com.openai.codex.device-key.{}.{}",
        class.tag_prefix(),
        key_id
    )
}

fn security_error(status: i32) -> String {
    security_framework::base::Error::from_code(status)
        .message()
        .unwrap_or_else(|| format!("Security.framework error code {status}"))
}
