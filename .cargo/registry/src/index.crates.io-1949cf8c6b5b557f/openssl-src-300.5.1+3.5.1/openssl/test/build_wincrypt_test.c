/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Simple buildtest to check for symbol collisions between wincrypt and
 * OpenSSL headers
 */

#include <openssl/types.h>

#ifdef _WIN32
# ifndef WIN32_LEAN_AND_MEAN
#  define WIN32_LEAN_AND_MEAN
# endif
# include <windows.h>
# include <wincrypt.h>
# ifndef X509_NAME
#  ifndef PEDANTIC
#    ifdef _MSC_VER
#      pragma message("wincrypt.h no longer defining X509_NAME before OpenSSL headers")
#    else
#      warning "wincrypt.h no longer defining X509_NAME before OpenSSL headers"
#    endif
#  endif
# endif
#endif

#include <openssl/opensslconf.h>
#ifndef OPENSSL_NO_STDIO
# include <stdio.h>
#endif

#include <openssl/evp.h>
#include <openssl/x509.h>
#include <openssl/x509v3.h>

int main(void)
{
    return 0;
}
