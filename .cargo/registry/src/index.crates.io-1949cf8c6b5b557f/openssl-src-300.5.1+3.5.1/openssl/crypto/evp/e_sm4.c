/*
 * Copyright 2017-2022 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright 2017 Ribose Inc. All Rights Reserved.
 * Ported from Ribose contributions from Botan.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/deprecated.h"

#include "internal/cryptlib.h"
#ifndef OPENSSL_NO_SM4
# include <openssl/evp.h>
# include <openssl/modes.h>
# include "crypto/sm4.h"
# include "crypto/evp.h"
# include "crypto/sm4_platform.h"
# include "evp_local.h"

typedef struct {
    union {
        OSSL_UNION_ALIGN;
        SM4_KEY ks;
    } ks;
    block128_f block;
    union {
        ecb128_f ecb;
        cbc128_f cbc;
        ctr128_f ctr;
    } stream;
} EVP_SM4_KEY;

# define BLOCK_CIPHER_generic(nid,blocksize,ivlen,nmode,mode,MODE,flags) \
static const EVP_CIPHER sm4_##mode = { \
        nid##_##nmode,blocksize,128/8,ivlen, \
        flags|EVP_CIPH_##MODE##_MODE,   \
        EVP_ORIG_GLOBAL,                \
        sm4_init_key,                   \
        sm4_##mode##_cipher,            \
        NULL,                           \
        sizeof(EVP_SM4_KEY),            \
        NULL,NULL,NULL,NULL }; \
const EVP_CIPHER *EVP_sm4_##mode(void) \
{ return &sm4_##mode; }

#define DEFINE_BLOCK_CIPHERS(nid,flags)             \
        BLOCK_CIPHER_generic(nid,16,16,cbc,cbc,CBC,flags|EVP_CIPH_FLAG_DEFAULT_ASN1)     \
        BLOCK_CIPHER_generic(nid,16,0,ecb,ecb,ECB,flags|EVP_CIPH_FLAG_DEFAULT_ASN1)      \
        BLOCK_CIPHER_generic(nid,1,16,ofb128,ofb,OFB,flags|EVP_CIPH_FLAG_DEFAULT_ASN1)   \
        BLOCK_CIPHER_generic(nid,1,16,cfb128,cfb,CFB,flags|EVP_CIPH_FLAG_DEFAULT_ASN1)   \
        BLOCK_CIPHER_generic(nid,1,16,ctr,ctr,CTR,flags)

static int sm4_init_key(EVP_CIPHER_CTX *ctx, const unsigned char *key,
                        const unsigned char *iv, int enc)
{
    int mode;
    EVP_SM4_KEY *dat = EVP_C_DATA(EVP_SM4_KEY,ctx);

    mode = EVP_CIPHER_CTX_get_mode(ctx);
    if ((mode == EVP_CIPH_ECB_MODE || mode == EVP_CIPH_CBC_MODE)
        && !enc) {
#ifdef HWSM4_CAPABLE
        if (HWSM4_CAPABLE) {
            HWSM4_set_decrypt_key(key, &dat->ks.ks);
            dat->block = (block128_f) HWSM4_decrypt;
            dat->stream.cbc = NULL;
# ifdef HWSM4_cbc_encrypt
            if (mode == EVP_CIPH_CBC_MODE)
                dat->stream.cbc = (cbc128_f) HWSM4_cbc_encrypt;
# endif
# ifdef HWSM4_ecb_encrypt
            if (mode == EVP_CIPH_ECB_MODE)
                dat->stream.ecb = (ecb128_f) HWSM4_ecb_encrypt;
# endif
        } else
#endif
#ifdef VPSM4_CAPABLE
        if (VPSM4_CAPABLE) {
            vpsm4_set_decrypt_key(key, &dat->ks.ks);
            dat->block = (block128_f) vpsm4_decrypt;
            dat->stream.cbc = NULL;
            if (mode == EVP_CIPH_CBC_MODE)
                dat->stream.cbc = (cbc128_f) vpsm4_cbc_encrypt;
            else if (mode == EVP_CIPH_ECB_MODE)
                dat->stream.ecb = (ecb128_f) vpsm4_ecb_encrypt;
        } else
#endif
        {
            dat->block = (block128_f) ossl_sm4_decrypt;
            ossl_sm4_set_key(key, EVP_CIPHER_CTX_get_cipher_data(ctx));
        }
    } else
#ifdef HWSM4_CAPABLE
    if (HWSM4_CAPABLE) {
        HWSM4_set_encrypt_key(key, &dat->ks.ks);
        dat->block = (block128_f) HWSM4_encrypt;
        dat->stream.cbc = NULL;
# ifdef HWSM4_cbc_encrypt
        if (mode == EVP_CIPH_CBC_MODE)
            dat->stream.cbc = (cbc128_f) HWSM4_cbc_encrypt;
        else
# endif
# ifdef HWSM4_ecb_encrypt
        if (mode == EVP_CIPH_ECB_MODE)
            dat->stream.ecb = (ecb128_f) HWSM4_ecb_encrypt;
        else
# endif
# ifdef HWSM4_ctr32_encrypt_blocks
        if (mode == EVP_CIPH_CTR_MODE)
            dat->stream.ctr = (ctr128_f) HWSM4_ctr32_encrypt_blocks;
        else
# endif
            (void)0;            /* terminate potentially open 'else' */
    } else
#endif
#ifdef VPSM4_CAPABLE
    if (VPSM4_CAPABLE) {
        vpsm4_set_encrypt_key(key, &dat->ks.ks);
        dat->block = (block128_f) vpsm4_encrypt;
        dat->stream.cbc = NULL;
        if (mode == EVP_CIPH_CBC_MODE)
            dat->stream.cbc = (cbc128_f) vpsm4_cbc_encrypt;
        else if (mode == EVP_CIPH_ECB_MODE)
            dat->stream.ecb = (ecb128_f) vpsm4_ecb_encrypt;
        else if (mode == EVP_CIPH_CTR_MODE)
            dat->stream.ctr = (ctr128_f) vpsm4_ctr32_encrypt_blocks;
    } else
#endif
    {
        dat->block = (block128_f) ossl_sm4_encrypt;
        ossl_sm4_set_key(key, EVP_CIPHER_CTX_get_cipher_data(ctx));
    }
    return 1;
}

static int sm4_cbc_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                          const unsigned char *in, size_t len)
{
    EVP_SM4_KEY *dat = EVP_C_DATA(EVP_SM4_KEY,ctx);

    if (dat->stream.cbc)
        (*dat->stream.cbc) (in, out, len, &dat->ks.ks, ctx->iv,
                            EVP_CIPHER_CTX_is_encrypting(ctx));
    else if (EVP_CIPHER_CTX_is_encrypting(ctx))
        CRYPTO_cbc128_encrypt(in, out, len, &dat->ks, ctx->iv,
                              dat->block);
    else
        CRYPTO_cbc128_decrypt(in, out, len, &dat->ks,
                              ctx->iv, dat->block);
    return 1;
}

static int sm4_cfb_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                          const unsigned char *in, size_t len)
{
    EVP_SM4_KEY *dat = EVP_C_DATA(EVP_SM4_KEY,ctx);
    int num = EVP_CIPHER_CTX_get_num(ctx);

    CRYPTO_cfb128_encrypt(in, out, len, &dat->ks,
                          ctx->iv, &num,
                          EVP_CIPHER_CTX_is_encrypting(ctx), dat->block);
    EVP_CIPHER_CTX_set_num(ctx, num);
    return 1;
}

static int sm4_ecb_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                          const unsigned char *in, size_t len)
{
    size_t bl = EVP_CIPHER_CTX_get_block_size(ctx);
    size_t i;
    EVP_SM4_KEY *dat = EVP_C_DATA(EVP_SM4_KEY,ctx);

    if (len < bl)
        return 1;

    if (dat->stream.ecb != NULL)
        (*dat->stream.ecb) (in, out, len, &dat->ks.ks,
                            EVP_CIPHER_CTX_is_encrypting(ctx));
    else
        for (i = 0, len -= bl; i <= len; i += bl)
            (*dat->block) (in + i, out + i, &dat->ks);

    return 1;
}

static int sm4_ofb_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                          const unsigned char *in, size_t len)
{
    EVP_SM4_KEY *dat = EVP_C_DATA(EVP_SM4_KEY,ctx);
    int num = EVP_CIPHER_CTX_get_num(ctx);

    CRYPTO_ofb128_encrypt(in, out, len, &dat->ks,
                          ctx->iv, &num, dat->block);
    EVP_CIPHER_CTX_set_num(ctx, num);
    return 1;
}

static int sm4_ctr_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                          const unsigned char *in, size_t len)
{
    int n = EVP_CIPHER_CTX_get_num(ctx);
    unsigned int num;
    EVP_SM4_KEY *dat = EVP_C_DATA(EVP_SM4_KEY,ctx);

    if (n < 0)
        return 0;
    num = (unsigned int)n;

    if (dat->stream.ctr)
        CRYPTO_ctr128_encrypt_ctr32(in, out, len, &dat->ks,
                                    ctx->iv,
                                    EVP_CIPHER_CTX_buf_noconst(ctx),
                                    &num, dat->stream.ctr);
    else
        CRYPTO_ctr128_encrypt(in, out, len, &dat->ks,
                              ctx->iv,
                              EVP_CIPHER_CTX_buf_noconst(ctx), &num,
                              dat->block);
    EVP_CIPHER_CTX_set_num(ctx, num);
    return 1;
}

DEFINE_BLOCK_CIPHERS(NID_sm4, 0)
#endif
