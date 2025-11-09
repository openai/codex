//! OpenXR Registry Management
//!
//! Best Practice: Use HKLM (HKEY_LOCAL_MACHINE) for OpenXR registry locations
//! DO NOT use HKCU (HKEY_CURRENT_USER) for OpenXR registry

#[cfg(target_os = "windows")]
use anyhow::{Context, Result};
#[cfg(target_os = "windows")]
use tracing::{debug, info, warn};
#[cfg(target_os = "windows")]
use windows::Win32::System::Registry::*;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::*;

/// OpenXR registry path (HKLM)
#[cfg(target_os = "windows")]
const OPENXR_REGISTRY_PATH: &str = "SOFTWARE\\Khronos\\OpenXR\\1";

/// Register OpenXR API layer in registry
///
/// Best Practice: Use HKLM, not HKCU
#[cfg(target_os = "windows")]
pub fn register_api_layer(layer_name: &str, layer_path: &str) -> Result<()> {
    info!("Registering OpenXR API layer: {layer_name}");
    
    unsafe {
        let mut hkey = HKEY::default();
        let api_layers_path = format!("{OPENXR_REGISTRY_PATH}\\ApiLayers");
        
        // Open or create API layers key
        let result = RegCreateKeyExW(
            HKEY_LOCAL_MACHINE,
            &windows::core::w!(api_layers_path),
            0,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hkey,
            None,
        );

        if result.is_err() {
            anyhow::bail!("Failed to create OpenXR API layers registry key. Administrator privileges required.");
        }

        // Create layer-specific key
        let layer_key_path = format!("{api_layers_path}\\{layer_name}");
        let mut layer_key = HKEY::default();
        
        let result = RegCreateKeyExW(
            HKEY_LOCAL_MACHINE,
            &windows::core::w!(layer_key_path),
            0,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut layer_key,
            None,
        );

        if result.is_err() {
            RegCloseKey(hkey);
            anyhow::bail!("Failed to create layer registry key");
        }

        // Set layer path
        let path_bytes: Vec<u16> = layer_path.encode_utf16().chain(std::iter::once(0)).collect();
        let result = RegSetValueExW(
            layer_key,
            &windows::core::w!("Path"),
            0,
            REG_SZ,
            Some(&path_bytes),
        );

        RegCloseKey(layer_key);
        RegCloseKey(hkey);

        if result.is_err() {
            anyhow::bail!("Failed to set layer path in registry");
        }

        info!("OpenXR API layer registered successfully in HKLM");
        debug!("Best Practice: Using HKLM for OpenXR registry (not HKCU)");
        
        Ok(())
    }
}

/// Unregister OpenXR API layer from registry
#[cfg(target_os = "windows")]
pub fn unregister_api_layer(layer_name: &str) -> Result<()> {
    info!("Unregistering OpenXR API layer: {layer_name}");
    
    unsafe {
        let layer_key_path = format!("{OPENXR_REGISTRY_PATH}\\ApiLayers\\{layer_name}");
        
        let result = RegDeleteKeyW(
            HKEY_LOCAL_MACHINE,
            &windows::core::w!(layer_key_path),
        );

        if result.is_err() {
            anyhow::bail!("Failed to delete layer registry key. Administrator privileges may be required.");
        }

        info!("OpenXR API layer unregistered successfully");
        Ok(())
    }
}

/// Get OpenXR API layer order from registry
///
/// Best Practice: Layer order is important for dependencies
#[cfg(target_os = "windows")]
pub fn get_api_layer_order() -> Result<Vec<String>> {
    debug!("Reading OpenXR API layer order from registry");
    
    unsafe {
        let api_layers_path = format!("{OPENXR_REGISTRY_PATH}\\ApiLayers");
        let mut hkey = HKEY::default();
        
        let result = RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            &windows::core::w!(api_layers_path),
            0,
            KEY_READ,
            &mut hkey,
        );

        if result.is_err() {
            return Ok(Vec::new()); // No layers registered
        }

        // TODO: Enumerate subkeys to get layer order
        // Best Practice: Order matters for layer dependencies
        
        RegCloseKey(hkey);
        Ok(Vec::new())
    }
}

/// Stub for non-Windows platforms
#[cfg(not(target_os = "windows"))]
pub fn register_api_layer(_layer_name: &str, _layer_path: &str) -> Result<()> {
    anyhow::bail!("OpenXR registry management is Windows-only")
}

#[cfg(not(target_os = "windows"))]
pub fn unregister_api_layer(_layer_name: &str) -> Result<()> {
    anyhow::bail!("OpenXR registry management is Windows-only")
}

#[cfg(not(target_os = "windows"))]
pub fn get_api_layer_order() -> Result<Vec<String>> {
    Ok(Vec::new())
}











