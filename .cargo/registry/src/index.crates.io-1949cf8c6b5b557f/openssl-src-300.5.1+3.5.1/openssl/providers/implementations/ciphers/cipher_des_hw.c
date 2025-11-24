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

#include "prov/ciphercommon.h"
#include "cipher_des.h"

static int cipher_hw_des_initkey(PROV_CIPHER_CTX *ctx,
                                 const unsigned char *key, size_t keylen)
{
    PROV_DES_CTX *dctx = (PROV_DES_CTX *)ctx;
    DES_cblock *deskey = (DES_cblock *)key;
    DES_key_schedule *ks = &dctx->dks.ks;

    dctx->dstream.cbc = NULL;
#if defined(SPARC_DES_CAPABLE)
    if (SPARC_DES_CAPABLE) {
        if (ctx->mode == EVP_CIPH_CBC_MODE) {
            des_t4_key_expand(&deskey[0], ks);
            dctx->dstream.cbc = ctx->enc ? des_t4_cbc_encrypt :
                                           des_t4_cbc_decrypt;
            return 1;
        }
    }
#endif
    DES_set_key_unchecked(deskey, ks);
    return 1;
}

static void cipher_hw_des_copyctx(PROV_CIPHER_CTX *dst,
                                  const PROV_CIPHER_CTX *src)
{
    PROV_DES_CTX *sctx = (PROV_DES_CTX *)src;
    PROV_DES_CTX *dctx = (PROV_DES_CTX *)dst;

    *dctx = *sctx;
    dst->ks = &dctx->dks.ks;
}

static int cipher_hw_des_ecb_cipher(PROV_CIPHER_CTX *ctx, unsigned char *out,
                                    const unsigned char *in, size_t len)
{
    size_t i, bl = ctx->blocksize;
    DES_key_schedule *key = &(((PROV_DES_CTX *)ctx)->dks.ks);

    if (len < bl)
        return 1;
    for (i = 0, len -= bl; i <= len; i += bl)
        DES_ecb_encrypt((const_DES_cblock *)(in + i),
                        (const_DES_cblock *)(out + i), key, ctx->enc);
    return 1;
}

static int cipher_hw_des_cbc_cipher(PROV_CIPHER_CTX *ctx, unsigned char *out,
                                    const unsigned char *in, size_t len)
{
    PROV_DES_CTX *dctx = (PROV_DES_CTX *)ctx;
    DES_key_schedule *key = &(dctx->dks.ks);

    if (dctx->dstream.cbc != NULL) {
        (*dctx->dstream.cbc) (in, out, len, key, ctx->iv);
        return 1;
    }

    while (len >= MAXCHUNK) {
        DES_ncbc_encrypt(in, out, MAXCHUNK, key, (DES_cblock *)ctx->iv,
                         ctx->enc);
        len -= MAXCHUNK;
        in += MAXCHUNK;
        out += MAXCHUNK;
    }
    if (len > 0)
        DES_ncbc_encrypt(in, out, (long)len, key, (DES_cblock *)ctx->iv,
                         ctx->enc);
    return 1;
}

static int cipher_hw_des_ofb64_cipher(PROV_CIPHER_CTX *ctx, unsigned char *out,
                                      const unsigned char *in, size_t len)
{
    int num = ctx->num;
    DES_key_schedule *key = &(((PROV_DES_CTX *)ctx)->dks.ks);

    while (len >= MAXCHUNK) {
        DES_ofb64_encrypt(in, out, MAXCHUNK, key, (DES_cblock *)ctx->iv, &num);
        len -= MAXCHUNK;
        in += MAXCHUNK;
        out += MAXCHUNK;
    }
    if (len > 0) {
        DES_ofb64_encrypt(in, out, (long)len, key, (DES_cblock *)ctx->iv, &num);
    }
    ctx->num = num;
    return 1;
}

static int cipher_hw_des_cfb64_cipher(PROV_CIPHER_CTX *ctx, unsigned char *out,
                                      const unsigned char *in, size_t len)
{
    size_t chunk = MAXCHUNK;
    DES_key_schedule *key = &(((PROV_DES_CTX *)ctx)->dks.ks);
    int num = ctx->num;

    if (len < chunk)
        chunk = len;
    while (len > 0 && len >= chunk) {
        DES_cfb64_encrypt(in, out, (long)chunk, key, (DES_cblock *)ctx->iv,
                          &num, ctx->enc);
        len -= chunk;
        in += chunk;
        out += chunk;
        if (len < chunk)
            chunk = len;
    }
    ctx->num = num;
    return 1;
}

/*
 * Although we have a CFB-r implementation for DES, it doesn't pack the right
 * way, so wrap it here
 */
static int cipher_hw_des_cfb1_cipher(PROV_CIPHER_CTX *ctx, unsigned char *out,
                                     const unsigned char *in, size_t inl)
{
    size_t n, chunk = MAXCHUNK / 8;
    DES_key_schedule *key = &(((PROV_DES_CTX *)ctx)->dks.ks);
    unsigned char c[1];
    unsigned char d[1] = { 0 };

    if (inl < chunk)
        chunk = inl;

    while (inl && inl >= chunk) {
        for (n = 0; n < chunk * 8; ++n) {
            c[0] = (in[n / 8] & (1 << (7 - n % 8))) ? 0x80 : 0;
            DES_cfb_encrypt(c, d, 1, 1, key, (DES_cblock *)ctx->iv, ctx->enc);
            out[n / 8] =
                (out[n / 8] & ~(0x80 >> (unsigned int)(n % 8))) |
                ((d[0] & 0x80) >> (unsigned int)(n % 8));
        }
        inl -= chunk;
        in += chunk;
        out += chunk;
        if (inl < chunk)
            chunk = inl;
    }

    return 1;
}

static int cipher_hw_des_cfb8_cipher(PROV_CIPHER_CTX *ctx, unsigned char *out,
                                     const unsigned char *in, size_t inl)
{
    DES_key_schedule *key = &(((PROV_DES_CTX *)ctx)->dks.ks);

    while (inl >= MAXCHUNK) {
        DES_cfb_encrypt(in, out, 8, (long)MAXCHUNK, key,
                        (DES_cblock *)ctx->iv, ctx->enc);
        inl -= MAXCHUNK;
        in += MAXCHUNK;
        out += MAXCHUNK;
    }
    if (inl > 0)
        DES_cfb_encrypt(in, out, 8, (long)inl, key,
                        (DES_cblock *)ctx->iv, ctx->enc);
    return 1;
}

#define PROV_CIPHER_HW_des_mode(mode)                                          \
static const PROV_CIPHER_HW des_##mode = {                                     \
    cipher_hw_des_initkey,                                                     \
    cipher_hw_des_##mode##_cipher,                                             \
    cipher_hw_des_copyctx                                                      \
};                                                                             \
const PROV_CIPHER_HW *ossl_prov_cipher_hw_des_##mode(void)                     \
{                                                                              \
    return &des_##mode;                                                        \
}

PROV_CIPHER_HW_des_mode(ecb)
PROV_CIPHER_HW_des_mode(cbc)
PROV_CIPHER_HW_des_mode(ofb64)
PROV_CIPHER_HW_des_mode(cfb64)
PROV_CIPHER_HW_des_mode(cfb1)
PROV_CIPHER_HW_des_mode(cfb8)
