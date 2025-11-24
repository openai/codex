/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/des.h>
#include "crypto/des_platform.h"

#define TDES_FLAGS 0

typedef struct prov_des_ctx_st {
    PROV_CIPHER_CTX base;      /* Must be first */
    union {
        OSSL_UNION_ALIGN;
        DES_key_schedule ks;
    } dks;
    union {
        void (*cbc) (const void *, void *, size_t,
                     const DES_key_schedule *, unsigned char *);
    } dstream;

} PROV_DES_CTX;

const PROV_CIPHER_HW *ossl_prov_cipher_hw_des_cbc(void);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_des_ecb(void);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_des_ofb64(void);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_des_cfb64(void);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_des_cfb1(void);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_des_cfb8(void);
