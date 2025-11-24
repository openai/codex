// Copyright 1999-2016 The OpenSSL Project Authors. All Rights Reserved.
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

#ifndef OPENSSL_HEADER_TYPE_CHECK_H
#define OPENSSL_HEADER_TYPE_CHECK_H

#include <ring-core/base.h>


#if defined(__cplusplus) || (defined(_MSC_VER) && !defined(__clang__))
// In C++ and non-clang MSVC, |static_assert| is a keyword.
#define OPENSSL_STATIC_ASSERT(cond, msg) static_assert(cond, msg)
#else
// C11 defines the |_Static_assert| keyword and the |static_assert| macro in
// assert.h. While the former is available at all versions in Clang and GCC, the
// later depends on libc and, in glibc, depends on being built in C11 mode. We
// do not require this, for now, so use |_Static_assert| directly.
#define OPENSSL_STATIC_ASSERT(cond, msg) _Static_assert(cond, msg)
#endif

#endif  // OPENSSL_HEADER_TYPE_CHECK_H
