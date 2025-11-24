/*
 * Copyright 2015-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * MDC2 low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <openssl/mdc2.h>
#include "crypto/evp.h"
#include "legacy_meth.h"

IMPLEMENT_LEGACY_EVP_MD_METH(mdc2, MDC2)

static const EVP_MD mdc2_md = {
    NID_mdc2,
    NID_mdc2WithRSA,
    MDC2_DIGEST_LENGTH,
    0,
    EVP_ORIG_GLOBAL,
    LEGACY_EVP_MD_METH_TABLE(mdc2_init, mdc2_update, mdc2_final, NULL,
                             MDC2_BLOCK),
};

const EVP_MD *EVP_mdc2(void)
{
    return &mdc2_md;
}
