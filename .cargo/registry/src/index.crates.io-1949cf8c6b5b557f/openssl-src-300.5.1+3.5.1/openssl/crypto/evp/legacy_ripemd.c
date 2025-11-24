/*
 * Copyright 1995-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * RIPEMD160 low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <openssl/ripemd.h>
#include "crypto/evp.h"
#include "legacy_meth.h"

IMPLEMENT_LEGACY_EVP_MD_METH(ripe, RIPEMD160)

static const EVP_MD ripemd160_md = {
    NID_ripemd160,
    NID_ripemd160WithRSA,
    RIPEMD160_DIGEST_LENGTH,
    0,
    EVP_ORIG_GLOBAL,
    LEGACY_EVP_MD_METH_TABLE(ripe_init, ripe_update, ripe_final, NULL,
                             RIPEMD160_CBLOCK),
};

const EVP_MD *EVP_ripemd160(void)
{
    return &ripemd160_md;
}
