/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/aes.h>
#include "prov/ciphercommon.h"
#include "crypto/aes_platform.h"

/*
 * Available in cipher_fips.c, and compiled with different values depending
 * on we're in the FIPS module or not.
 */
extern const int ossl_aes_xts_allow_insecure_decrypt;

PROV_CIPHER_FUNC(void, xts_stream,
                 (const unsigned char *in, unsigned char *out, size_t len,
                  const AES_KEY *key1, const AES_KEY *key2,
                  const unsigned char iv[16]));

#if defined(OPENSSL_CPUID_OBJ) && defined(__s390__)
typedef struct S390X_km_xts_params_st {
    unsigned char key[64];
    unsigned char tweak[16];
    unsigned char nap[16];
} S390X_KM_XTS_PARAMS;
#endif

typedef struct prov_aes_xts_ctx_st {
    PROV_CIPHER_CTX base;      /* Must be first */
    union {
        OSSL_UNION_ALIGN;
        AES_KEY ks;
    } ks1, ks2;                /* AES key schedules to use */
    XTS128_CONTEXT xts;
    OSSL_xts_stream_fn stream;

    /* Platform specific data */
    union {
        int dummy;
#if defined(OPENSSL_CPUID_OBJ) && defined(__s390__)
        struct {
            union {
                OSSL_UNION_ALIGN;
                S390X_KM_XTS_PARAMS km;
            } param;
            size_t offset;
            unsigned int fc;
            unsigned int iv_set : 1;
            unsigned int key_set : 1;
        } s390x;
#endif
    } plat;
} PROV_AES_XTS_CTX;

const PROV_CIPHER_HW *ossl_prov_cipher_hw_aes_xts(size_t keybits);
