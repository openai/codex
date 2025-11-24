/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* This is the C source file where we include this header directly */
#include <openssl/sslerr_legacy.h>
#include <openssl/ssl.h>

#ifndef OPENSSL_NO_DEPRECATED_3_0
int ERR_load_SSL_strings(void)
{
    return OPENSSL_init_crypto(OPENSSL_INIT_LOAD_SSL_STRINGS, 0);
}
#else
NON_EMPTY_TRANSLATION_UNIT
#endif
