/*
 * Copyright 2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "crypto/sm4.h"
#include "prov/ciphercommon.h"
#include "prov/ciphercommon_ccm.h"
#include "crypto/sm4_platform.h"

typedef struct prov_sm4_ccm_ctx_st {
    PROV_CCM_CTX base; /* Must be first */
    union {
        OSSL_UNION_ALIGN;
        SM4_KEY ks;
    } ks;                       /* SM4 key schedule to use */
} PROV_SM4_CCM_CTX;

const PROV_CCM_HW *ossl_prov_sm4_hw_ccm(size_t keylen);
