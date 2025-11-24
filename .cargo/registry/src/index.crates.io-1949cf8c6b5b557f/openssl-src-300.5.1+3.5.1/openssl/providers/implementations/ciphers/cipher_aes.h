/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/aes.h>
#include "prov/ciphercommon.h"
#include "crypto/aes_platform.h"

typedef struct prov_aes_ctx_st {
    PROV_CIPHER_CTX base;      /* Must be first */
    union {
        OSSL_UNION_ALIGN;
        AES_KEY ks;
    } ks;

    /* Platform specific data */
    union {
        int dummy;
#if defined(OPENSSL_CPUID_OBJ) && defined(__s390__)
        struct {
            union {
                OSSL_UNION_ALIGN;
                /*-
                 * KM-AES parameter block - begin
                 * (see z/Architecture Principles of Operation >= SA22-7832-06)
                 */
                struct {
                    unsigned char k[32];
                } km;
                /* KM-AES parameter block - end */
                /*-
                 * KMO-AES/KMF-AES parameter block - begin
                 * (see z/Architecture Principles of Operation >= SA22-7832-08)
                 */
                struct {
                    unsigned char cv[16];
                    unsigned char k[32];
                } kmo_kmf;
                /* KMO-AES/KMF-AES parameter block - end */
            } param;
            unsigned int fc;
        } s390x;
#endif /* defined(OPENSSL_CPUID_OBJ) && defined(__s390__) */
    } plat;

} PROV_AES_CTX;

#define ossl_prov_cipher_hw_aes_ofb ossl_prov_cipher_hw_aes_ofb128
#define ossl_prov_cipher_hw_aes_cfb ossl_prov_cipher_hw_aes_cfb128
const PROV_CIPHER_HW *ossl_prov_cipher_hw_aes_ecb(size_t keybits);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_aes_cbc(size_t keybits);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_aes_ofb128(size_t keybits);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_aes_cfb128(size_t keybits);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_aes_cfb1(size_t keybits);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_aes_cfb8(size_t keybits);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_aes_ctr(size_t keybits);
