/*
 * Copyright 2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_E_WINSOCK_H
# define OSSL_E_WINSOCK_H
# pragma once

# ifdef WINDOWS
#  if !defined(_WIN32_WCE) && !defined(_WIN32_WINNT)
      /*
       * Defining _WIN32_WINNT here in e_winsock.h implies certain "discipline."
       * Most notably we ought to check for availability of each specific
       * routine that was introduced after denoted _WIN32_WINNT with
       * GetProcAddress(). Normally newer functions are masked with higher
       * _WIN32_WINNT in SDK headers. So that if you wish to use them in
       * some module, you'd need to override _WIN32_WINNT definition in
       * the target module in order to "reach for" prototypes, but replace
       * calls to new functions with indirect calls. Alternatively it
       * might be possible to achieve the goal by /DELAYLOAD-ing .DLLs
       * and check for current OS version instead.
       */
#   define _WIN32_WINNT 0x0501
#  endif
#  if defined(_WIN32_WINNT) || defined(_WIN32_WCE)
      /*
       * Just like defining _WIN32_WINNT including winsock2.h implies
       * certain "discipline" for maintaining [broad] binary compatibility.
       * As long as structures are invariant among Winsock versions,
       * it's sufficient to check for specific Winsock2 API availability
       * at run-time [DSO_global_lookup is recommended]...
       */
#   include <winsock2.h>
#   include <ws2tcpip.h>
      /*
       * Clang-based C++Builder 10.3.3 toolchains cannot find C inline
       * definitions at link-time.  This header defines WspiapiLoad() as an
       * __inline function.  https://quality.embarcadero.com/browse/RSP-33806
       */
#   if !defined(__BORLANDC__) || !defined(__clang__)
#    include <wspiapi.h>
#   endif
      /* yes, they have to be #included prior to <windows.h> */
#  endif
#  include <windows.h>
# endif
#endif /* !(OSSL_E_WINSOCK_H) */
