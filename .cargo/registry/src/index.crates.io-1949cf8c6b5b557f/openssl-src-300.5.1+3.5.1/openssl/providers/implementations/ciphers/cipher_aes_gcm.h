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
#include "prov/ciphercommon_gcm.h"
#include "crypto/aes_platform.h"

typedef struct prov_aes_gcm_ctx_st {
    PROV_GCM_CTX base;          /* must be first entry in struct */
    union {
        OSSL_UNION_ALIGN;
        AES_KEY ks;
    } ks;                       /* AES key schedule to use */

    /* Platform specific data */
    union {
        int dummy;
#if defined(OPENSSL_CPUID_OBJ) && defined(__s390__)
        struct {
            union {
                OSSL_UNION_ALIGN;
                S390X_KMA_PARAMS kma;
            } param;
            unsigned int fc;
            unsigned int hsflag;    /* hash subkey set flag */
            unsigned char ares[16];
            unsigned char mres[16];
            unsigned char kres[16];
            int areslen;
            int mreslen;
            int kreslen;
            int res;
        } s390x;
#endif /* defined(OPENSSL_CPUID_OBJ) && defined(__s390__) */
    } plat;
} PROV_AES_GCM_CTX;

const PROV_GCM_HW *ossl_prov_aes_hw_gcm(size_t keybits);
