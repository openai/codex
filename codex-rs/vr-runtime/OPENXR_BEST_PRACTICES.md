# OpenXR Best Practices Implementation Guide

**Based on**: [Best Practices for OpenXR API Layers on Windows](https://fredemmott.com/blog/2024/11/25/best-practices-for-openxr-api-layers.html)  
**Last Updated**: 2025-11-06

---

## Overview

This document outlines the best practices for OpenXR implementation in Codex, based on industry standards and recommendations from the OpenXR community.

---

## Key Best Practices

### 1. Use HKLM (HKEY_LOCAL_MACHINE) for Registry

**Requirement**: Register OpenXR layers/runtimes in `HKEY_LOCAL_MACHINE\SOFTWARE\Khronos\OpenXR\<major_api_version>`

**Implementation**:
- ✅ Use HKLM for OpenXR registration
- ❌ DO NOT use HKCU for OpenXR registry locations
- ✅ Use HKCU for application-specific settings only

**Reason**: Layer ordering can only be controlled within a single registry hive. Most existing layers use HKLM, so we should too.

### 2. Sign All DLLs

**Requirement**: All DLLs loaded into third-party processes must be signed with a trusted code-signing certificate.

**Implementation**:
- ✅ Sign all OpenXR DLLs with code-signing certificate
- ✅ Use timestamp server when signing
- ✅ Ensure certificate is trusted by Windows

**Reason**: Required by most anti-cheat software. Unsigned DLLs will prevent users from playing games with anti-cheat.

### 3. Use Timestamp Server for Signing

**Requirement**: Use a timestamp server when signing DLLs to extend signature validity.

**Implementation**:
```powershell
signtool ... /t http://timestamp.example.com
```

**Reason**: Without timestamp server, signatures expire when certificate expires (max 39 months). With timestamp server, signatures remain valid for up to 135 months.

### 4. Add VERSIONINFO Resource

**Requirement**: Add VERSIONINFO resource to all binaries with vendor, name, and version information.

**Implementation**:
- ✅ Populate VERSIONINFO with vendor, name, version
- ✅ Auto-update version information
- ✅ Include in all binaries

**Reason**: Windows includes this in minidumps, saving time for crash investigation.

### 5. Set ACLs for Sandboxed Applications

**Requirement**: Set ACLs for 'All Packages' and 'All Restricted Packages' identities.

**Implementation**:
- ✅ Install into Program Files (default ACL allows access)
- ✅ Explicitly set ACLs for shared resources
- ✅ Support sandboxed applications (WebXR in Chrome, Microsoft Store apps)

**Reason**: Sandboxed applications (including "OpenXR Tools for Windows Mixed Reality") need access to OpenXR resources.

### 6. Use XR_KHR_vulkan_enable2 for Vulkan Support

**Requirement**: Use `XR_KHR_vulkan_enable2` instead of `XR_KHR_vulkan_enable` where practical.

**Implementation**:
- ✅ Prefer `XR_KHR_vulkan_enable2`
- ✅ Do NOT mix `XR_KHR_vulkan_enable` and `XR_KHR_vulkan_enable2`
- ✅ Handle `XR_ERROR_EXTENSION_NOT_PRESENT` gracefully

**Reason**: `XR_KHR_vulkan_enable2` supports modern Vulkan features that `XR_KHR_vulkan_enable` cannot handle.

### 7. Implement Required Functions

**Requirement**: Implement `xrEnumerateApiLayerProperties` and `xrEnumerateInstanceExtensionProperties`.

**Implementation**:
- ✅ Implement both functions
- ✅ Do NOT depend on their availability
- ✅ Use `XrApiLayerCreateInfo` for layer information instead

**Reason**: These functions are invocable by API layers, even though they appear to be dead code when called by applications.

### 8. Test on Multiple Runtimes

**Requirement**: Test with as many OpenXR runtimes as possible.

**Implementation**:
- ✅ Test with SteamVR
- ✅ Test with Windows Mixed Reality
- ✅ Test with vendor-specific runtimes
- ✅ Use simulators when hardware unavailable

**Reason**: Easy to accidentally depend on non-specification behavior. Testing catches these issues early.

### 9. Run OpenXR Conformance Test Suite

**Requirement**: Run the full OpenXR Conformance Test Suite.

**Implementation**:
- ✅ Run full test suite
- ✅ Test with multiple runtimes
- ✅ Ensure conformant behavior

**Reason**: API layers must not make conformant runtimes exhibit non-conformant behavior.

### 10. Graceful Degradation

**Requirement**: If requiring a specific runtime, test for it and gracefully degrade on others.

**Implementation**:
- ✅ Detect runtime type
- ✅ Gracefully handle unsupported runtimes
- ✅ Return appropriate errors instead of crashing

**Reason**: Prevents issues when users switch runtimes, upgrade hardware, or use multiple headsets.

---

## Implementation Checklist

### Registry Setup
- [ ] Register in `HKEY_LOCAL_MACHINE\SOFTWARE\Khronos\OpenXR\1`
- [ ] Use JSON manifest files
- [ ] Set proper layer ordering

### Code Signing
- [ ] Obtain code-signing certificate
- [ ] Sign all DLLs
- [ ] Use timestamp server
- [ ] Verify signature validity

### Binary Resources
- [ ] Add VERSIONINFO resource
- [ ] Include vendor information
- [ ] Include product name
- [ ] Include version number
- [ ] Auto-update version

### Security & Compatibility
- [ ] Set ACLs for 'All Packages'
- [ ] Set ACLs for 'All Restricted Packages'
- [ ] Test with sandboxed applications
- [ ] Support WebXR in Chrome

### Vulkan Support
- [ ] Use `XR_KHR_vulkan_enable2` where possible
- [ ] Handle extension errors gracefully
- [ ] Do not mix enable/enable2

### Testing
- [ ] Test on multiple runtimes
- [ ] Run OpenXR Conformance Test Suite
- [ ] Test graceful degradation
- [ ] Test with simulators

---

## References

- [Best Practices for OpenXR API Layers on Windows](https://fredemmott.com/blog/2024/11/25/best-practices-for-openxr-api-layers.html)
- [OpenXR Specification](https://www.khronos.org/openxr/)
- [OpenXR Loader Developer Documentation](https://github.com/KhronosGroup/OpenXR-SDK-Source)
- [OpenXR Conformance Test Suite](https://github.com/KhronosGroup/OpenXR-CTS)

---

## Notes

- These practices apply to OpenXR API layers, runtimes, games, and engines
- Most practices require developer implementation, not end-user configuration
- Following these practices maximizes compatibility and reduces support burden











