/*
 * Copyright 1995-2021 The OpenSSL Project Authors. All Rights Reserved.
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

#include "cipher_tdes_default.h"

#define ks1 tks.ks[0]
#define ks2 tks.ks[1]
#define ks3 tks.ks[2]

static int ossl_cipher_hw_tdes_ede2_initkey(PROV_CIPHER_CTX *ctx,
                                            const unsigned char *key,
                                            size_t keylen)
{
    PROV_TDES_CTX *tctx = (PROV_TDES_CTX *)ctx;
    DES_cblock *deskey = (DES_cblock *)key;

    tctx->tstream.cbc = NULL;
# if defined(SPARC_DES_CAPABLE)
    if (SPARC_DES_CAPABLE) {
        if (ctx->mode == EVP_CIPH_CBC_MODE) {
            des_t4_key_expand(&deskey[0], &tctx->ks1);
            des_t4_key_expand(&deskey[1], &tctx->ks2);
            memcpy(&tctx->ks3, &tctx->ks1, sizeof(tctx->ks1));
            tctx->tstream.cbc = ctx->enc ? des_t4_ede3_cbc_encrypt :
                                           des_t4_ede3_cbc_decrypt;
            return 1;
        }
    }
# endif
    DES_set_key_unchecked(&deskey[0], &tctx->ks1);
    DES_set_key_unchecked(&deskey[1], &tctx->ks2);
    memcpy(&tctx->ks3, &tctx->ks1, sizeof(tctx->ks1));
    return 1;
}

static int ossl_cipher_hw_tdes_ofb(PROV_CIPHER_CTX *ctx, unsigned char *out,
                                   const unsigned char *in, size_t inl)
{
    PROV_TDES_CTX *tctx = (PROV_TDES_CTX *)ctx;
    int num = ctx->num;

    while (inl >= MAXCHUNK) {
        DES_ede3_ofb64_encrypt(in, out, (long)MAXCHUNK, &tctx->ks1, &tctx->ks2,
                               &tctx->ks3, (DES_cblock *)ctx->iv, &num);
        inl -= MAXCHUNK;
        in += MAXCHUNK;
        out += MAXCHUNK;
    }
    if (inl > 0) {
        DES_ede3_ofb64_encrypt(in, out, (long)inl, &tctx->ks1, &tctx->ks2,
                               &tctx->ks3, (DES_cblock *)ctx->iv, &num);
    }
    ctx->num = num;
    return 1;
}

static int ossl_cipher_hw_tdes_cfb(PROV_CIPHER_CTX *ctx, unsigned char *out,
                                   const unsigned char *in, size_t inl)
{
    PROV_TDES_CTX *tctx = (PROV_TDES_CTX *)ctx;
    int num = ctx->num;

    while (inl >= MAXCHUNK) {

        DES_ede3_cfb64_encrypt(in, out, (long)MAXCHUNK,
                               &tctx->ks1, &tctx->ks2, &tctx->ks3,
                               (DES_cblock *)ctx->iv, &num, ctx->enc);
        inl -= MAXCHUNK;
        in += MAXCHUNK;
        out += MAXCHUNK;
    }
    if (inl > 0) {
        DES_ede3_cfb64_encrypt(in, out, (long)inl,
                               &tctx->ks1, &tctx->ks2, &tctx->ks3,
                               (DES_cblock *)ctx->iv, &num, ctx->enc);
    }
    ctx->num = num;
    return 1;
}

/*
 * Although we have a CFB-r implementation for 3-DES, it doesn't pack the
 * right way, so wrap it here
 */
static int ossl_cipher_hw_tdes_cfb1(PROV_CIPHER_CTX *ctx, unsigned char *out,
                                    const unsigned char *in, size_t inl)
{
    PROV_TDES_CTX *tctx = (PROV_TDES_CTX *)ctx;
    size_t n;
    unsigned char c[1];
    unsigned char d[1] = { 0 };

    if (ctx->use_bits == 0)
        inl *= 8;
    for (n = 0; n < inl; ++n) {
        c[0] = (in[n / 8] & (1 << (7 - n % 8))) ? 0x80 : 0;
        DES_ede3_cfb_encrypt(c, d, 1, 1,
                             &tctx->ks1, &tctx->ks2, &tctx->ks3,
                             (DES_cblock *)ctx->iv, ctx->enc);
        out[n / 8] = (out[n / 8] & ~(0x80 >> (unsigned int)(n % 8)))
            | ((d[0] & 0x80) >> (unsigned int)(n % 8));
    }

    return 1;
}

static int ossl_cipher_hw_tdes_cfb8(PROV_CIPHER_CTX *ctx, unsigned char *out,
                                    const unsigned char *in, size_t inl)
{
    PROV_TDES_CTX *tctx = (PROV_TDES_CTX *)ctx;

    while (inl >= MAXCHUNK) {
        DES_ede3_cfb_encrypt(in, out, 8, (long)MAXCHUNK,
                             &tctx->ks1, &tctx->ks2, &tctx->ks3,
                             (DES_cblock *)ctx->iv, ctx->enc);
        inl -= MAXCHUNK;
        in += MAXCHUNK;
        out += MAXCHUNK;
    }
    if (inl > 0)
        DES_ede3_cfb_encrypt(in, out, 8, (long)inl,
                             &tctx->ks1, &tctx->ks2, &tctx->ks3,
                             (DES_cblock *)ctx->iv, ctx->enc);
    return 1;
}

PROV_CIPHER_HW_tdes_mode(ede3, ofb)
PROV_CIPHER_HW_tdes_mode(ede3, cfb)
PROV_CIPHER_HW_tdes_mode(ede3, cfb1)
PROV_CIPHER_HW_tdes_mode(ede3, cfb8)

PROV_CIPHER_HW_tdes_mode(ede2, ecb)
PROV_CIPHER_HW_tdes_mode(ede2, cbc)
PROV_CIPHER_HW_tdes_mode(ede2, ofb)
PROV_CIPHER_HW_tdes_mode(ede2, cfb)

