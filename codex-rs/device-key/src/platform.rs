use crate::DeviceKeyBinding;
use crate::DeviceKeyError;
use crate::DeviceKeyInfo;
use crate::DeviceKeyProtectionClass;
use crate::DeviceKeyProvider;
use crate::ProviderCreateRequest;
use crate::ProviderSignature;
use std::sync::Arc;

pub(crate) fn default_provider() -> Arc<dyn DeviceKeyProvider> {
    Arc::new(UnsupportedDeviceKeyProvider)
}

#[derive(Debug)]
pub(crate) struct UnsupportedDeviceKeyProvider;

impl DeviceKeyProvider for UnsupportedDeviceKeyProvider {
    fn create(&self, request: ProviderCreateRequest<'_>) -> Result<DeviceKeyInfo, DeviceKeyError> {
        let _ = request.key_id_for(DeviceKeyProtectionClass::HardwareTpm);
        let _ = request
            .protection_policy
            .allows(DeviceKeyProtectionClass::HardwareTpm);
        let _ = request.binding;
        Err(DeviceKeyError::HardwareBackedKeysUnavailable)
    }

    fn get_public(
        &self,
        _key_id: &str,
        _protection_class: DeviceKeyProtectionClass,
    ) -> Result<DeviceKeyInfo, DeviceKeyError> {
        Err(DeviceKeyError::KeyNotFound)
    }

    fn binding(
        &self,
        _key_id: &str,
        _protection_class: DeviceKeyProtectionClass,
    ) -> Result<DeviceKeyBinding, DeviceKeyError> {
        Err(DeviceKeyError::KeyNotFound)
    }

    fn sign(
        &self,
        _key_id: &str,
        _protection_class: DeviceKeyProtectionClass,
        _payload: &[u8],
    ) -> Result<ProviderSignature, DeviceKeyError> {
        Err(DeviceKeyError::KeyNotFound)
    }
}
