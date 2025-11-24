/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <crypto/sm4.h>
#include "prov/ciphercommon.h"
#include "crypto/sm4_platform.h"

PROV_CIPHER_FUNC(void, xts_stream,
                 (const unsigned char *in, unsigned char *out, size_t len,
                  const SM4_KEY *key1, const SM4_KEY *key2,
                  const unsigned char iv[16], const int enc));

typedef struct prov_sm4_xts_ctx_st {
    /* Must be first */
    PROV_CIPHER_CTX base;

    /* SM4 key schedules to use */
    union {
        OSSL_UNION_ALIGN;
        SM4_KEY ks;
    } ks1, ks2;

    /*-
     * XTS standard to use with SM4-XTS algorithm
     *
     * Must be 0 or 1,
     * 0 for XTS mode specified by GB/T 17964-2021
     * 1 for XTS mode specified by IEEE Std 1619-2007
     */
    int xts_standard;

    XTS128_CONTEXT xts;

    /* Stream function for XTS mode specified by GB/T 17964-2021 */
    OSSL_xts_stream_fn stream_gb;
    /* Stream function for XTS mode specified by IEEE Std 1619-2007 */
    OSSL_xts_stream_fn stream;
} PROV_SM4_XTS_CTX;

const PROV_CIPHER_HW *ossl_prov_cipher_hw_sm4_xts(size_t keybits);
