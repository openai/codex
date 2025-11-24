/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * DES low level APIs are deprecated for public use, but still ok for internal
 * use.
 */
#include "internal/deprecated.h"

#include <openssl/des.h>
#include "cipher_tdes_default.h"

/*
 * Note the PROV_TDES_CTX has been used for the DESX cipher, just to reduce
 * code size.
 */
#define ks1 tks.ks[0]
#define ks2 tks.ks[1].ks[0].cblock
#define ks3 tks.ks[2].ks[0].cblock

static int cipher_hw_desx_cbc_initkey(PROV_CIPHER_CTX *ctx,
                                      const unsigned char *key, size_t keylen)
{
    PROV_TDES_CTX *tctx = (PROV_TDES_CTX *)ctx;
    DES_cblock *deskey = (DES_cblock *)key;

    DES_set_key_unchecked(deskey, &tctx->ks1);
    memcpy(&tctx->ks2, &key[8], 8);
    memcpy(&tctx->ks3, &key[16], 8);

    return 1;
}

static void cipher_hw_desx_copyctx(PROV_CIPHER_CTX *dst,
                                   const PROV_CIPHER_CTX *src)
{
    PROV_TDES_CTX *sctx = (PROV_TDES_CTX *)src;
    PROV_TDES_CTX *dctx = (PROV_TDES_CTX *)dst;

    *dctx = *sctx;
    dst->ks = &dctx->tks.ks;
}

static int cipher_hw_desx_cbc(PROV_CIPHER_CTX *ctx, unsigned char *out,
                              const unsigned char *in, size_t inl)
{
    PROV_TDES_CTX *tctx = (PROV_TDES_CTX *)ctx;

    while (inl >= MAXCHUNK) {
        DES_xcbc_encrypt(in, out, (long)MAXCHUNK, &tctx->ks1,
                         (DES_cblock *)ctx->iv, &tctx->ks2, &tctx->ks3,
                         ctx->enc);
        inl -= MAXCHUNK;
        in += MAXCHUNK;
        out += MAXCHUNK;
    }
    if (inl > 0)
        DES_xcbc_encrypt(in, out, (long)inl, &tctx->ks1,
                         (DES_cblock *)ctx->iv, &tctx->ks2, &tctx->ks3,
                         ctx->enc);
    return 1;
}

static const PROV_CIPHER_HW desx_cbc = {
    cipher_hw_desx_cbc_initkey,
    cipher_hw_desx_cbc,
    cipher_hw_desx_copyctx
};

const PROV_CIPHER_HW *ossl_prov_cipher_hw_tdes_desx_cbc(void)
{
    return &desx_cbc;
}
