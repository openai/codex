#ifndef CODEX_DEVICE_KEY_MACOS_PROVIDER_H
#define CODEX_DEVICE_KEY_MACOS_PROVIDER_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef enum CodexDeviceKeyMacStatus {
    CodexDeviceKeyMacStatusOk = 0,
    CodexDeviceKeyMacStatusNotFound = 1,
    CodexDeviceKeyMacStatusHardwareUnavailable = 2,
    CodexDeviceKeyMacStatusPlatformError = 3,
} CodexDeviceKeyMacStatus;

typedef enum CodexDeviceKeyMacKeyClass {
    CodexDeviceKeyMacKeyClassSecureEnclave = 0,
    CodexDeviceKeyMacKeyClassOsProtectedNonextractable = 1,
} CodexDeviceKeyMacKeyClass;

typedef struct CodexDeviceKeyMacBytesResult {
    int32_t status;
    uint8_t *data;
    size_t len;
    char *error_message;
} CodexDeviceKeyMacBytesResult;

CodexDeviceKeyMacBytesResult codex_device_key_macos_create_or_load_public_key(
    const char *key_tag,
    int32_t key_class);
CodexDeviceKeyMacBytesResult codex_device_key_macos_load_public_key(
    const char *key_tag,
    int32_t key_class);
CodexDeviceKeyMacBytesResult codex_device_key_macos_delete(
    const char *key_tag,
    int32_t key_class);
CodexDeviceKeyMacBytesResult codex_device_key_macos_sign(
    const char *key_tag,
    int32_t key_class,
    const uint8_t *payload,
    size_t payload_len);
void codex_device_key_macos_free_bytes_result(CodexDeviceKeyMacBytesResult *result);

#ifdef __cplusplus
}
#endif

#endif
