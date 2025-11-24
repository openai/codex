/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "prov/ciphercommon.h"

/*-
 * The generic cipher functions for cipher modes cbc, ecb, ofb, cfb and ctr.
 * Used if there is no special hardware implementations.
 */
int ossl_cipher_hw_generic_cbc(PROV_CIPHER_CTX *dat, unsigned char *out,
                               const unsigned char *in, size_t len)
{
    if (dat->stream.cbc)
        (*dat->stream.cbc) (in, out, len, dat->ks, dat->iv, dat->enc);
    else if (dat->enc)
        CRYPTO_cbc128_encrypt(in, out, len, dat->ks, dat->iv, dat->block);
    else
        CRYPTO_cbc128_decrypt(in, out, len, dat->ks, dat->iv, dat->block);

    return 1;
}

int ossl_cipher_hw_generic_ecb(PROV_CIPHER_CTX *dat, unsigned char *out,
                               const unsigned char *in, size_t len)
{
    size_t i, bl = dat->blocksize;

    if (len < bl)
        return 1;

    if (dat->stream.ecb) {
        (*dat->stream.ecb) (in, out, len, dat->ks, dat->enc);
    }
    else {
        for (i = 0, len -= bl; i <= len; i += bl)
            (*dat->block) (in + i, out + i, dat->ks);
    }

    return 1;
}

int ossl_cipher_hw_generic_ofb128(PROV_CIPHER_CTX *dat, unsigned char *out,
                                  const unsigned char *in, size_t len)
{
    int num = dat->num;

    CRYPTO_ofb128_encrypt(in, out, len, dat->ks, dat->iv, &num, dat->block);
    dat->num = num;

    return 1;
}

int ossl_cipher_hw_generic_cfb128(PROV_CIPHER_CTX *dat, unsigned char *out,
                                  const unsigned char *in, size_t len)
{
    int num = dat->num;

    CRYPTO_cfb128_encrypt(in, out, len, dat->ks, dat->iv, &num, dat->enc,
                          dat->block);
    dat->num = num;

    return 1;
}

int ossl_cipher_hw_generic_cfb8(PROV_CIPHER_CTX *dat, unsigned char *out,
                                const unsigned char *in, size_t len)
{
    int num = dat->num;

    CRYPTO_cfb128_8_encrypt(in, out, len, dat->ks, dat->iv, &num, dat->enc,
                            dat->block);
    dat->num = num;

    return 1;
}

int ossl_cipher_hw_generic_cfb1(PROV_CIPHER_CTX *dat, unsigned char *out,
                                const unsigned char *in, size_t len)
{
    int num = dat->num;

    if (dat->use_bits) {
        CRYPTO_cfb128_1_encrypt(in, out, len, dat->ks, dat->iv, &num,
                                dat->enc, dat->block);
        dat->num = num;
        return 1;
    }

    while (len >= MAXBITCHUNK) {
        CRYPTO_cfb128_1_encrypt(in, out, MAXBITCHUNK * 8, dat->ks,
                                dat->iv, &num, dat->enc, dat->block);
        len -= MAXBITCHUNK;
        out += MAXBITCHUNK;
        in  += MAXBITCHUNK;
    }
    if (len)
        CRYPTO_cfb128_1_encrypt(in, out, len * 8, dat->ks, dat->iv, &num,
                                dat->enc, dat->block);

    dat->num = num;

    return 1;
}

int ossl_cipher_hw_generic_ctr(PROV_CIPHER_CTX *dat, unsigned char *out,
                               const unsigned char *in, size_t len)
{
    unsigned int num = dat->num;

    if (dat->stream.ctr)
        CRYPTO_ctr128_encrypt_ctr32(in, out, len, dat->ks, dat->iv, dat->buf,
                                    &num, dat->stream.ctr);
    else
        CRYPTO_ctr128_encrypt(in, out, len, dat->ks, dat->iv, dat->buf,
                              &num, dat->block);
    dat->num = num;

    return 1;
}

/*-
 * The chunked cipher functions for cipher modes cbc, ecb, ofb, cfb and ctr.
 * Used if there is no special hardware implementations.
 */

int ossl_cipher_hw_chunked_cbc(PROV_CIPHER_CTX *ctx, unsigned char *out,
                               const unsigned char *in, size_t inl)
{
    while (inl >= MAXCHUNK) {
        ossl_cipher_hw_generic_cbc(ctx, out, in, MAXCHUNK);
        inl -= MAXCHUNK;
        in  += MAXCHUNK;
        out += MAXCHUNK;
    }
    if (inl > 0)
        ossl_cipher_hw_generic_cbc(ctx, out, in, inl);
    return 1;
}

int ossl_cipher_hw_chunked_cfb8(PROV_CIPHER_CTX *ctx, unsigned char *out,
                                const unsigned char *in, size_t inl)
{
    size_t chunk = MAXCHUNK;

    if (inl < chunk)
        chunk = inl;
    while (inl > 0 && inl >= chunk) {
        ossl_cipher_hw_generic_cfb8(ctx, out, in, inl);
        inl -= chunk;
        in += chunk;
        out += chunk;
        if (inl < chunk)
            chunk = inl;
    }
    return 1;
}

int ossl_cipher_hw_chunked_cfb128(PROV_CIPHER_CTX *ctx, unsigned char *out,
                                  const unsigned char *in, size_t inl)
{
    size_t chunk = MAXCHUNK;

    if (inl < chunk)
        chunk = inl;
    while (inl > 0 && inl >= chunk) {
        ossl_cipher_hw_generic_cfb128(ctx, out, in, inl);
        inl -= chunk;
        in += chunk;
        out += chunk;
        if (inl < chunk)
            chunk = inl;
    }
    return 1;
}

int ossl_cipher_hw_chunked_ofb128(PROV_CIPHER_CTX *ctx, unsigned char *out,
                                  const unsigned char *in, size_t inl)
{
    while (inl >= MAXCHUNK) {
        ossl_cipher_hw_generic_ofb128(ctx, out, in, MAXCHUNK);
        inl -= MAXCHUNK;
        in  += MAXCHUNK;
        out += MAXCHUNK;
    }
    if (inl > 0)
        ossl_cipher_hw_generic_ofb128(ctx, out, in, inl);
    return 1;
}
