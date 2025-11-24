/*
 * Copyright 2015-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * MD5 low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <openssl/md5.h>
#include "crypto/evp.h"
#include "legacy_meth.h"

IMPLEMENT_LEGACY_EVP_MD_METH(md5, MD5)

static const EVP_MD md5_md = {
    NID_md5,
    NID_md5WithRSAEncryption,
    MD5_DIGEST_LENGTH,
    0,
    EVP_ORIG_GLOBAL,
    LEGACY_EVP_MD_METH_TABLE(md5_init, md5_update, md5_final, NULL, MD5_CBLOCK)
};

const EVP_MD *EVP_md5(void)
{
    return &md5_md;
}
