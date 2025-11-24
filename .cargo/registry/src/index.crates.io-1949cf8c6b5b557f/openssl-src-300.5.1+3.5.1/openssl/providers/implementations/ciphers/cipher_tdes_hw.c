/*
 * Copyright 1995-2020 The OpenSSL Project Authors. All Rights Reserved.
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

#include "prov/ciphercommon.h"
#include "cipher_tdes.h"

#define ks1 tks.ks[0]
#define ks2 tks.ks[1]
#define ks3 tks.ks[2]

int ossl_cipher_hw_tdes_ede3_initkey(PROV_CIPHER_CTX *ctx,
                                     const unsigned char *key, size_t keylen)
{
    PROV_TDES_CTX *tctx = (PROV_TDES_CTX *)ctx;
    DES_cblock *deskey = (DES_cblock *)key;

    tctx->tstream.cbc = NULL;
# if defined(SPARC_DES_CAPABLE)
    if (SPARC_DES_CAPABLE) {
        if (ctx->mode == EVP_CIPH_CBC_MODE) {
            des_t4_key_expand(&deskey[0], &tctx->ks1);
            des_t4_key_expand(&deskey[1], &tctx->ks2);
            des_t4_key_expand(&deskey[2], &tctx->ks3);
            tctx->tstream.cbc = ctx->enc ? des_t4_ede3_cbc_encrypt :
                                           des_t4_ede3_cbc_decrypt;
            return 1;
        }
    }
# endif
    DES_set_key_unchecked(&deskey[0], &tctx->ks1);
    DES_set_key_unchecked(&deskey[1], &tctx->ks2);
    DES_set_key_unchecked(&deskey[2], &tctx->ks3);
    return 1;
}

void ossl_cipher_hw_tdes_copyctx(PROV_CIPHER_CTX *dst,
                                 const PROV_CIPHER_CTX *src)
{
    PROV_TDES_CTX *sctx = (PROV_TDES_CTX *)src;
    PROV_TDES_CTX *dctx = (PROV_TDES_CTX *)dst;

    *dctx = *sctx;
    dst->ks = &dctx->tks.ks;
}

int ossl_cipher_hw_tdes_cbc(PROV_CIPHER_CTX *ctx, unsigned char *out,
                            const unsigned char *in, size_t inl)
{
    PROV_TDES_CTX *tctx = (PROV_TDES_CTX *)ctx;

    if (tctx->tstream.cbc != NULL) {
        (*tctx->tstream.cbc) (in, out, inl, tctx->tks.ks, ctx->iv);
        return 1;
    }

    while (inl >= MAXCHUNK) {
        DES_ede3_cbc_encrypt(in, out, (long)MAXCHUNK, &tctx->ks1, &tctx->ks2,
                             &tctx->ks3, (DES_cblock *)ctx->iv, ctx->enc);
        inl -= MAXCHUNK;
        in += MAXCHUNK;
        out += MAXCHUNK;
    }
    if (inl > 0)
        DES_ede3_cbc_encrypt(in, out, (long)inl, &tctx->ks1, &tctx->ks2,
                             &tctx->ks3, (DES_cblock *)ctx->iv, ctx->enc);
    return 1;
}

int ossl_cipher_hw_tdes_ecb(PROV_CIPHER_CTX *ctx, unsigned char *out,
                            const unsigned char *in, size_t len)
{
    size_t i;
    PROV_TDES_CTX *tctx = (PROV_TDES_CTX *)ctx;

    if (len < DES_BLOCK_SIZE)
        return 1;

    for (i = 0, len -= DES_BLOCK_SIZE; i <= len; i += DES_BLOCK_SIZE) {
        DES_ecb3_encrypt((const_DES_cblock *)(in + i), (DES_cblock *)(out + i),
                         &tctx->ks1, &tctx->ks2, &tctx->ks3, ctx->enc);
    }
    return 1;
}

PROV_CIPHER_HW_tdes_mode(ede3, ecb)
PROV_CIPHER_HW_tdes_mode(ede3, cbc)
