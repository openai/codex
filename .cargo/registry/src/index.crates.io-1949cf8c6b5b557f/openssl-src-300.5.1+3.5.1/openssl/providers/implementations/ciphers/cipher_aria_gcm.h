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
#include "prov/ciphercommon_gcm.h"

typedef struct prov_aria_gcm_ctx_st {
    PROV_GCM_CTX base;              /* must be first entry in struct */
    union {
        OSSL_UNION_ALIGN;
        ARIA_KEY ks;
    } ks;
} PROV_ARIA_GCM_CTX;

const PROV_GCM_HW *ossl_prov_aria_hw_gcm(size_t keybits);
