/*
 * Copyright 2019-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "crypto/aria.h"
#include "prov/ciphercommon.h"
#include "prov/ciphercommon_ccm.h"

typedef struct prov_aria_ccm_ctx_st {
    PROV_CCM_CTX base; /* Must be first */
    union {
        OSSL_UNION_ALIGN;
        ARIA_KEY ks;
    } ks;                       /* ARIA key schedule to use */
} PROV_ARIA_CCM_CTX;

const PROV_CCM_HW *ossl_prov_aria_hw_ccm(size_t keylen);
