#import "macos_provider.h"

#import <Foundation/Foundation.h>
#import <LocalAuthentication/LocalAuthentication.h>
#import <Security/Security.h>

#include <stdlib.h>
#include <string.h>

static NSTimeInterval const CodexDeviceKeyTouchIdReuseDurationSeconds = 300.0;
static OSStatus const CodexDeviceKeyErrSecMissingEntitlement = -34018;

static CodexDeviceKeyMacBytesResult CodexDeviceKeyMacResultMake(
    CodexDeviceKeyMacStatus status,
    NSData *data,
    NSString *errorMessage) {
    CodexDeviceKeyMacBytesResult result = {
        .status = status,
        .data = NULL,
        .len = 0,
        .error_message = NULL,
    };

    if (data.length > 0) {
        result.data = malloc(data.length);
        if (result.data == NULL) {
            result.status = CodexDeviceKeyMacStatusPlatformError;
            errorMessage = @"failed to allocate result bytes";
        } else {
            memcpy(result.data, data.bytes, data.length);
            result.len = data.length;
        }
    }

    if (errorMessage.length > 0) {
        const char *utf8 = errorMessage.UTF8String;
        if (utf8 != NULL) {
            size_t len = strlen(utf8);
            result.error_message = malloc(len + 1);
            if (result.error_message != NULL) {
                memcpy(result.error_message, utf8, len + 1);
            }
        }
    }

    return result;
}

static CodexDeviceKeyMacBytesResult CodexDeviceKeyMacError(
    CodexDeviceKeyMacStatus status,
    NSString *message) {
    return CodexDeviceKeyMacResultMake(status, nil, message);
}

static NSString *CodexDeviceKeyMacCopySecurityError(OSStatus status) {
    NSString *message = CFBridgingRelease(SecCopyErrorMessageString(status, NULL));
    if (message.length > 0) {
        return message;
    }
    return [NSString stringWithFormat:@"Security.framework error code %d", status];
}

static NSString *CodexDeviceKeyMacCopyCFError(CFErrorRef error) {
    if (error == NULL) {
        return @"Security.framework returned an unknown error";
    }
    NSError *nsError = CFBridgingRelease(error);
    if (nsError.localizedDescription.length > 0) {
        return nsError.localizedDescription;
    }
    return [nsError description];
}

static BOOL CodexDeviceKeyMacClassIsValid(int32_t keyClass) {
    return keyClass == CodexDeviceKeyMacKeyClassSecureEnclave ||
        keyClass == CodexDeviceKeyMacKeyClassOsProtectedNonextractable;
}

static BOOL CodexDeviceKeyMacSecureEnclaveUnavailableStatus(OSStatus status) {
    return status == errSecUnimplemented ||
        status == errSecParam ||
        status == CodexDeviceKeyErrSecMissingEntitlement;
}

static NSData *CodexDeviceKeyMacTagData(NSString *keyTag) {
    return [keyTag dataUsingEncoding:NSUTF8StringEncoding];
}

static NSMutableDictionary *CodexDeviceKeyMacPrivateKeyQuery(
    NSString *keyTag,
    int32_t keyClass,
    LAContext *authenticationContext) {
    NSMutableDictionary *query = [@{
        (__bridge id)kSecClass: (__bridge id)kSecClassKey,
        (__bridge id)kSecAttrKeyClass: (__bridge id)kSecAttrKeyClassPrivate,
        (__bridge id)kSecAttrApplicationTag: CodexDeviceKeyMacTagData(keyTag),
        (__bridge id)kSecReturnRef: @YES,
    } mutableCopy];

    if (keyClass == CodexDeviceKeyMacKeyClassSecureEnclave) {
        query[(__bridge id)kSecAttrTokenID] = (__bridge id)kSecAttrTokenIDSecureEnclave;
    } else {
        query[(__bridge id)kSecAttrIsExtractable] = @NO;
    }

    if (authenticationContext != nil) {
        query[(__bridge id)kSecUseAuthenticationContext] = authenticationContext;
    }

    return query;
}

static SecKeyRef CodexDeviceKeyMacCopyPrivateKey(
    NSString *keyTag,
    int32_t keyClass,
    LAContext *authenticationContext,
    CodexDeviceKeyMacStatus *status,
    NSString **errorMessage) {
    CFTypeRef item = NULL;
    OSStatus secStatus = SecItemCopyMatching(
        (__bridge CFDictionaryRef)CodexDeviceKeyMacPrivateKeyQuery(
            keyTag, keyClass, authenticationContext),
        &item);
    if (secStatus == errSecItemNotFound) {
        *status = CodexDeviceKeyMacStatusNotFound;
        return NULL;
    }
    if (secStatus != errSecSuccess) {
        *status = CodexDeviceKeyMacStatusPlatformError;
        *errorMessage = CodexDeviceKeyMacCopySecurityError(secStatus);
        return NULL;
    }
    if (item == NULL) {
        *status = CodexDeviceKeyMacStatusPlatformError;
        *errorMessage = @"Security.framework returned an empty key reference";
        return NULL;
    }
    *status = CodexDeviceKeyMacStatusOk;
    return (SecKeyRef)item;
}

static SecKeyRef CodexDeviceKeyMacCreatePrivateKey(
    NSString *keyTag,
    int32_t keyClass,
    CodexDeviceKeyMacStatus *status,
    NSString **errorMessage) {
    CFErrorRef accessControlError = NULL;
    SecAccessControlRef accessControl = SecAccessControlCreateWithFlags(
        kCFAllocatorDefault,
        kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
        kSecAccessControlPrivateKeyUsage | kSecAccessControlUserPresence,
        &accessControlError);
    if (accessControl == NULL) {
        *status = CodexDeviceKeyMacStatusPlatformError;
        *errorMessage = CodexDeviceKeyMacCopyCFError(accessControlError);
        return NULL;
    }

    NSMutableDictionary *privateAttributes = [@{
        (__bridge id)kSecAttrIsPermanent: @YES,
        (__bridge id)kSecAttrAccessControl: (__bridge id)accessControl,
        (__bridge id)kSecAttrApplicationTag: CodexDeviceKeyMacTagData(keyTag),
        (__bridge id)kSecAttrLabel: keyTag,
    } mutableCopy];
    if (keyClass == CodexDeviceKeyMacKeyClassOsProtectedNonextractable) {
        privateAttributes[(__bridge id)kSecAttrIsExtractable] = @NO;
    }

    NSMutableDictionary *attributes = [@{
        (__bridge id)kSecAttrKeyType: (__bridge id)kSecAttrKeyTypeECSECPrimeRandom,
        (__bridge id)kSecAttrKeySizeInBits: @256,
        (__bridge id)kSecAttrLabel: keyTag,
        (__bridge id)kSecPrivateKeyAttrs: privateAttributes,
    } mutableCopy];
    if (keyClass == CodexDeviceKeyMacKeyClassSecureEnclave) {
        attributes[(__bridge id)kSecAttrTokenID] = (__bridge id)kSecAttrTokenIDSecureEnclave;
    }

    CFErrorRef createError = NULL;
    SecKeyRef key = SecKeyCreateRandomKey((__bridge CFDictionaryRef)attributes, &createError);
    CFRelease(accessControl);
    if (key != NULL) {
        *status = CodexDeviceKeyMacStatusOk;
        return key;
    }

    NSError *nsError = createError == NULL ? nil : CFBridgingRelease(createError);
    OSStatus code = nsError == nil ? 0 : (OSStatus)nsError.code;
    if (keyClass == CodexDeviceKeyMacKeyClassSecureEnclave &&
        CodexDeviceKeyMacSecureEnclaveUnavailableStatus(code)) {
        *status = CodexDeviceKeyMacStatusHardwareUnavailable;
        return NULL;
    }

    *status = CodexDeviceKeyMacStatusPlatformError;
    *errorMessage = nsError.localizedDescription.length > 0
        ? nsError.localizedDescription
        : @"Security.framework failed to create a private key";
    return NULL;
}

static CodexDeviceKeyMacBytesResult CodexDeviceKeyMacCopyPublicKeyResult(SecKeyRef privateKey) {
    SecKeyRef publicKey = SecKeyCopyPublicKey(privateKey);
    if (publicKey == NULL) {
        return CodexDeviceKeyMacError(
            CodexDeviceKeyMacStatusPlatformError,
            @"Security.framework did not return a public key");
    }

    CFErrorRef error = NULL;
    CFDataRef publicKeyData = SecKeyCopyExternalRepresentation(publicKey, &error);
    CFRelease(publicKey);
    if (publicKeyData == NULL) {
        return CodexDeviceKeyMacError(
            CodexDeviceKeyMacStatusPlatformError,
            CodexDeviceKeyMacCopyCFError(error));
    }

    NSData *data = CFBridgingRelease(publicKeyData);
    return CodexDeviceKeyMacResultMake(CodexDeviceKeyMacStatusOk, data, nil);
}

static LAContext *CodexDeviceKeyMacReusableAuthenticationContext(void) {
    static LAContext *context = nil;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        context = [[LAContext alloc] init];
        context.touchIDAuthenticationAllowableReuseDuration =
            CodexDeviceKeyTouchIdReuseDurationSeconds;
    });
    return context;
}

CodexDeviceKeyMacBytesResult codex_device_key_macos_create_or_load_public_key(
    const char *keyTag,
    int32_t keyClass) {
    @autoreleasepool {
        if (keyTag == NULL || !CodexDeviceKeyMacClassIsValid(keyClass)) {
            return CodexDeviceKeyMacError(
                CodexDeviceKeyMacStatusPlatformError,
                @"invalid macOS device-key provider argument");
        }

        NSString *tag = [NSString stringWithUTF8String:keyTag];
        CodexDeviceKeyMacStatus status = CodexDeviceKeyMacStatusOk;
        NSString *errorMessage = nil;
        SecKeyRef key = CodexDeviceKeyMacCreatePrivateKey(tag, keyClass, &status, &errorMessage);
        if (key == NULL) {
            if (status == CodexDeviceKeyMacStatusHardwareUnavailable) {
                return CodexDeviceKeyMacError(status, nil);
            }

            CodexDeviceKeyMacStatus loadStatus = CodexDeviceKeyMacStatusOk;
            NSString *loadErrorMessage = nil;
            key = CodexDeviceKeyMacCopyPrivateKey(
                tag, keyClass, nil, &loadStatus, &loadErrorMessage);
            if (key == NULL) {
                if (loadStatus == CodexDeviceKeyMacStatusNotFound) {
                    return CodexDeviceKeyMacError(status, errorMessage);
                }
                return CodexDeviceKeyMacError(
                    CodexDeviceKeyMacStatusPlatformError,
                    [NSString stringWithFormat:
                        @"key creation failed (%@); reload failed (%@)",
                        errorMessage ?: @"unknown error",
                        loadErrorMessage ?: @"unknown error"]);
            }
        }

        CodexDeviceKeyMacBytesResult result = CodexDeviceKeyMacCopyPublicKeyResult(key);
        CFRelease(key);
        return result;
    }
}

CodexDeviceKeyMacBytesResult codex_device_key_macos_load_public_key(
    const char *keyTag,
    int32_t keyClass) {
    @autoreleasepool {
        if (keyTag == NULL || !CodexDeviceKeyMacClassIsValid(keyClass)) {
            return CodexDeviceKeyMacError(
                CodexDeviceKeyMacStatusPlatformError,
                @"invalid macOS device-key provider argument");
        }

        NSString *tag = [NSString stringWithUTF8String:keyTag];
        CodexDeviceKeyMacStatus status = CodexDeviceKeyMacStatusOk;
        NSString *errorMessage = nil;
        SecKeyRef key = CodexDeviceKeyMacCopyPrivateKey(tag, keyClass, nil, &status, &errorMessage);
        if (key == NULL) {
            return CodexDeviceKeyMacError(status, errorMessage);
        }

        CodexDeviceKeyMacBytesResult result = CodexDeviceKeyMacCopyPublicKeyResult(key);
        CFRelease(key);
        return result;
    }
}

CodexDeviceKeyMacBytesResult codex_device_key_macos_delete(
    const char *keyTag,
    int32_t keyClass) {
    @autoreleasepool {
        if (keyTag == NULL || !CodexDeviceKeyMacClassIsValid(keyClass)) {
            return CodexDeviceKeyMacError(
                CodexDeviceKeyMacStatusPlatformError,
                @"invalid macOS device-key provider argument");
        }

        NSString *tag = [NSString stringWithUTF8String:keyTag];
        NSMutableDictionary *query = CodexDeviceKeyMacPrivateKeyQuery(tag, keyClass, nil);
        [query removeObjectForKey:(__bridge id)kSecReturnRef];
        OSStatus status = SecItemDelete((__bridge CFDictionaryRef)query);
        if (status == errSecSuccess || status == errSecItemNotFound) {
            return CodexDeviceKeyMacResultMake(CodexDeviceKeyMacStatusOk, nil, nil);
        }

        return CodexDeviceKeyMacError(
            CodexDeviceKeyMacStatusPlatformError,
            CodexDeviceKeyMacCopySecurityError(status));
    }
}

CodexDeviceKeyMacBytesResult codex_device_key_macos_sign(
    const char *keyTag,
    int32_t keyClass,
    const uint8_t *payload,
    size_t payloadLen) {
    @autoreleasepool {
        if (keyTag == NULL || payload == NULL || !CodexDeviceKeyMacClassIsValid(keyClass)) {
            return CodexDeviceKeyMacError(
                CodexDeviceKeyMacStatusPlatformError,
                @"invalid macOS device-key provider argument");
        }

        NSString *tag = [NSString stringWithUTF8String:keyTag];
        CodexDeviceKeyMacStatus status = CodexDeviceKeyMacStatusOk;
        NSString *errorMessage = nil;
        SecKeyRef key = CodexDeviceKeyMacCopyPrivateKey(
            tag,
            keyClass,
            CodexDeviceKeyMacReusableAuthenticationContext(),
            &status,
            &errorMessage);
        if (key == NULL) {
            return CodexDeviceKeyMacError(status, errorMessage);
        }

        NSData *payloadData = [NSData dataWithBytes:payload length:payloadLen];
        CFErrorRef error = NULL;
        CFDataRef signature = SecKeyCreateSignature(
            key,
            kSecKeyAlgorithmECDSASignatureMessageX962SHA256,
            (__bridge CFDataRef)payloadData,
            &error);
        CFRelease(key);
        if (signature == NULL) {
            return CodexDeviceKeyMacError(
                CodexDeviceKeyMacStatusPlatformError,
                CodexDeviceKeyMacCopyCFError(error));
        }

        NSData *signatureData = CFBridgingRelease(signature);
        return CodexDeviceKeyMacResultMake(CodexDeviceKeyMacStatusOk, signatureData, nil);
    }
}

void codex_device_key_macos_free_bytes_result(CodexDeviceKeyMacBytesResult *result) {
    if (result == NULL) {
        return;
    }
    free(result->data);
    free(result->error_message);
    result->data = NULL;
    result->len = 0;
    result->error_message = NULL;
}
