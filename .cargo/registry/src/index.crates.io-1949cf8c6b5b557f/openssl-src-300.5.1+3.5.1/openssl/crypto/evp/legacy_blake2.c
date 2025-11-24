/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "crypto/evp.h"
#include "prov/blake2.h"        /* diverse BLAKE2 macros */
#include "legacy_meth.h"

/*
 * Local hack to adapt the BLAKE2 init functions to what the
 * legacy function signatures demand.
 */
static int blake2s_init(BLAKE2S_CTX *C)
{
    BLAKE2S_PARAM P;

    ossl_blake2s_param_init(&P);
    return ossl_blake2s_init(C, &P);
}
static int blake2b_init(BLAKE2B_CTX *C)
{
    BLAKE2B_PARAM P;

    ossl_blake2b_param_init(&P);
    return ossl_blake2b_init(C, &P);
}
#define blake2s_update ossl_blake2s_update
#define blake2b_update ossl_blake2b_update
#define blake2s_final ossl_blake2s_final
#define blake2b_final ossl_blake2b_final

IMPLEMENT_LEGACY_EVP_MD_METH_LC(blake2s_int, blake2s)
IMPLEMENT_LEGACY_EVP_MD_METH_LC(blake2b_int, blake2b)

static const EVP_MD blake2b_md = {
    NID_blake2b512,
    0,
    BLAKE2B_DIGEST_LENGTH,
    0,
    EVP_ORIG_GLOBAL,
    LEGACY_EVP_MD_METH_TABLE(blake2b_int_init, blake2b_int_update,
                             blake2b_int_final, NULL, BLAKE2B_BLOCKBYTES),
};

const EVP_MD *EVP_blake2b512(void)
{
    return &blake2b_md;
}

static const EVP_MD blake2s_md = {
    NID_blake2s256,
    0,
    BLAKE2S_DIGEST_LENGTH,
    0,
    EVP_ORIG_GLOBAL,
    LEGACY_EVP_MD_METH_TABLE(blake2s_int_init, blake2s_int_update,
                             blake2s_int_final, NULL, BLAKE2S_BLOCKBYTES),
};

const EVP_MD *EVP_blake2s256(void)
{
    return &blake2s_md;
}
