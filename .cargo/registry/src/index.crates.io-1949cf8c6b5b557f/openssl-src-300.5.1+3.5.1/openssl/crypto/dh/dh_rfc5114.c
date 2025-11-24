/*
 * Copyright 2011-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * DH low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include "internal/cryptlib.h"
#include "dh_local.h"
#include <openssl/bn.h>
#include "crypto/bn_dh.h"

/*
 * Macro to make a DH structure from BIGNUM data. NB: although just copying
 * the BIGNUM static pointers would be more efficient, we can't do that
 * because they get wiped using BN_clear_free() when DH_free() is called.
 */

#define make_dh(x) \
DH *DH_get_##x(void) \
{ \
    DH *dh = DH_new(); \
\
    if (dh == NULL) \
        return NULL; \
    dh->params.p = BN_dup(&ossl_bignum_dh##x##_p); \
    dh->params.g = BN_dup(&ossl_bignum_dh##x##_g); \
    dh->params.q = BN_dup(&ossl_bignum_dh##x##_q); \
    if (dh->params.p == NULL || dh->params.q == NULL || dh->params.g == NULL) {\
        DH_free(dh); \
        return NULL; \
    } \
    return dh; \
}

make_dh(1024_160)
make_dh(2048_224)
make_dh(2048_256)
