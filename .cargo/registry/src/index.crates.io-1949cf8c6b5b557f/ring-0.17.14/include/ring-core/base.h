// Copyright 2001-2016 The OpenSSL Project Authors. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#ifndef OPENSSL_HEADER_BASE_H
#define OPENSSL_HEADER_BASE_H


// This file should be the first included by all BoringSSL headers.

#if defined(_MSC_VER) && !defined(__clang__)
#pragma warning(push, 3)
#endif

#include <stddef.h>
#include <stdint.h>

#if defined(_MSC_VER) && !defined(__clang__)
#pragma warning(pop)
#endif

#if defined(__APPLE__)
#include <TargetConditionals.h>
#endif

#include <ring-core/target.h>  // IWYU pragma: export

#include <ring_core_generated/prefix_symbols.h>

#include <ring-core/type_check.h>

#if defined(__APPLE__)
// Note |TARGET_OS_MAC| is set for all Apple OS variants. |TARGET_OS_OSX|
// targets macOS specifically.
#if defined(TARGET_OS_OSX) && TARGET_OS_OSX
#define OPENSSL_MACOS
#endif
#if defined(TARGET_OS_IPHONE) && TARGET_OS_IPHONE
#define OPENSSL_IOS
#endif
#endif

// *ring* doesn't support the `BORINGSSL_SHARED_LIBRARY` configuration, so
// the default (usually "hidden") visibility is always used, even for exported
// items.
#define OPENSSL_EXPORT

// `ring::c` would need to be customized on any platform where these assertions
// fail. Keep in sync with `ring::c`.
OPENSSL_STATIC_ASSERT(sizeof(int32_t) == sizeof(int), "int isn't 32 bits.");
OPENSSL_STATIC_ASSERT(sizeof(uint32_t) == sizeof(unsigned int), "unsigned int isn't 32 bits.");
OPENSSL_STATIC_ASSERT(sizeof(size_t) == sizeof(uintptr_t), "uintptr_t and size_t differ.");
OPENSSL_STATIC_ASSERT(sizeof(size_t) <= sizeof(uint64_t), "size_t is larger than uint64_t.");
OPENSSL_STATIC_ASSERT(sizeof(size_t) >= sizeof(uint32_t), "size_t is smaller than uint32_t.");

#endif  // OPENSSL_HEADER_BASE_H
