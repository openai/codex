/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * AES low level APIs are deprecated for public use, but still ok for internal
 * use where we're using them to implement the higher level EVP interface, as is
 * the case here.
 */
#include "internal/deprecated.h"

#include "cipher_aes_xts.h"

#ifdef FIPS_MODULE
const int ossl_aes_xts_allow_insecure_decrypt = 0;
#else
const int ossl_aes_xts_allow_insecure_decrypt = 1;
#endif /* FIPS_MODULE */
