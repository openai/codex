/*
 * Copyright 2006-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Camellia low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <openssl/opensslconf.h>

#include <openssl/evp.h>
#include <openssl/err.h>
#include <string.h>
#include <assert.h>
#include <openssl/camellia.h>
#include "crypto/evp.h"
#include "crypto/modes.h"
#include "crypto/cmll_platform.h"
#include "evp_local.h"

static int camellia_init_key(EVP_CIPHER_CTX *ctx, const unsigned char *key,
                             const unsigned char *iv, int enc);

/* Camellia subkey Structure */
typedef struct {
    CAMELLIA_KEY ks;
    block128_f block;
    union {
        cbc128_f cbc;
        ctr128_f ctr;
    } stream;
} EVP_CAMELLIA_KEY;

#define MAXBITCHUNK     ((size_t)1<<(sizeof(size_t)*8-4))

/* Attribute operation for Camellia */
#define data(ctx)       EVP_C_DATA(EVP_CAMELLIA_KEY,ctx)

#if defined(AES_ASM) && (defined(__sparc) || defined(__sparc__))
/* ---------^^^ this is not a typo, just a way to detect that
 * assembler support was in general requested... */
# include "crypto/sparc_arch.h"

static int cmll_t4_init_key(EVP_CIPHER_CTX *ctx, const unsigned char *key,
                            const unsigned char *iv, int enc)
{
    int ret, mode, bits;
    EVP_CAMELLIA_KEY *dat =
        (EVP_CAMELLIA_KEY *)EVP_CIPHER_CTX_get_cipher_data(ctx);

    mode = EVP_CIPHER_CTX_get_mode(ctx);
    bits = EVP_CIPHER_CTX_get_key_length(ctx) * 8;

    cmll_t4_set_key(key, bits, &dat->ks);

    if ((mode == EVP_CIPH_ECB_MODE || mode == EVP_CIPH_CBC_MODE)
        && !enc) {
        ret = 0;
        dat->block = (block128_f) cmll_t4_decrypt;
        switch (bits) {
        case 128:
            dat->stream.cbc = mode == EVP_CIPH_CBC_MODE ?
                (cbc128_f) cmll128_t4_cbc_decrypt : NULL;
            break;
        case 192:
        case 256:
            dat->stream.cbc = mode == EVP_CIPH_CBC_MODE ?
                (cbc128_f) cmll256_t4_cbc_decrypt : NULL;
            break;
        default:
            ret = -1;
        }
    } else {
        ret = 0;
        dat->block = (block128_f) cmll_t4_encrypt;
        switch (bits) {
        case 128:
            if (mode == EVP_CIPH_CBC_MODE)
                dat->stream.cbc = (cbc128_f) cmll128_t4_cbc_encrypt;
            else if (mode == EVP_CIPH_CTR_MODE)
                dat->stream.ctr = (ctr128_f) cmll128_t4_ctr32_encrypt;
            else
                dat->stream.cbc = NULL;
            break;
        case 192:
        case 256:
            if (mode == EVP_CIPH_CBC_MODE)
                dat->stream.cbc = (cbc128_f) cmll256_t4_cbc_encrypt;
            else if (mode == EVP_CIPH_CTR_MODE)
                dat->stream.ctr = (ctr128_f) cmll256_t4_ctr32_encrypt;
            else
                dat->stream.cbc = NULL;
            break;
        default:
            ret = -1;
        }
    }

    if (ret < 0) {
        ERR_raise(ERR_LIB_EVP, EVP_R_CAMELLIA_KEY_SETUP_FAILED);
        return 0;
    }

    return 1;
}

# define cmll_t4_cbc_cipher camellia_cbc_cipher
static int cmll_t4_cbc_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                              const unsigned char *in, size_t len);

# define cmll_t4_ecb_cipher camellia_ecb_cipher
static int cmll_t4_ecb_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                              const unsigned char *in, size_t len);

# define cmll_t4_ofb_cipher camellia_ofb_cipher
static int cmll_t4_ofb_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                              const unsigned char *in, size_t len);

# define cmll_t4_cfb_cipher camellia_cfb_cipher
static int cmll_t4_cfb_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                              const unsigned char *in, size_t len);

# define cmll_t4_cfb8_cipher camellia_cfb8_cipher
static int cmll_t4_cfb8_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                               const unsigned char *in, size_t len);

# define cmll_t4_cfb1_cipher camellia_cfb1_cipher
static int cmll_t4_cfb1_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                               const unsigned char *in, size_t len);

# define cmll_t4_ctr_cipher camellia_ctr_cipher
static int cmll_t4_ctr_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                              const unsigned char *in, size_t len);

# define BLOCK_CIPHER_generic(nid,keylen,blocksize,ivlen,nmode,mode,MODE,flags) \
static const EVP_CIPHER cmll_t4_##keylen##_##mode = { \
        nid##_##keylen##_##nmode,blocksize,keylen/8,ivlen, \
        flags|EVP_CIPH_##MODE##_MODE,   \
        EVP_ORIG_GLOBAL,                \
        cmll_t4_init_key,               \
        cmll_t4_##mode##_cipher,        \
        NULL,                           \
        sizeof(EVP_CAMELLIA_KEY),       \
        NULL,NULL,NULL,NULL }; \
static const EVP_CIPHER camellia_##keylen##_##mode = { \
        nid##_##keylen##_##nmode,blocksize,     \
        keylen/8,ivlen, \
        flags|EVP_CIPH_##MODE##_MODE,   \
        EVP_ORIG_GLOBAL,                \
        camellia_init_key,              \
        camellia_##mode##_cipher,       \
        NULL,                           \
        sizeof(EVP_CAMELLIA_KEY),       \
        NULL,NULL,NULL,NULL }; \
const EVP_CIPHER *EVP_camellia_##keylen##_##mode(void) \
{ return SPARC_CMLL_CAPABLE?&cmll_t4_##keylen##_##mode:&camellia_##keylen##_##mode; }

#else

# define BLOCK_CIPHER_generic(nid,keylen,blocksize,ivlen,nmode,mode,MODE,flags) \
static const EVP_CIPHER camellia_##keylen##_##mode = { \
        nid##_##keylen##_##nmode,blocksize,keylen/8,ivlen, \
        flags|EVP_CIPH_##MODE##_MODE,   \
        EVP_ORIG_GLOBAL,                \
        camellia_init_key,              \
        camellia_##mode##_cipher,       \
        NULL,                           \
        sizeof(EVP_CAMELLIA_KEY),       \
        NULL,NULL,NULL,NULL }; \
const EVP_CIPHER *EVP_camellia_##keylen##_##mode(void) \
{ return &camellia_##keylen##_##mode; }

#endif

#define BLOCK_CIPHER_generic_pack(nid,keylen,flags)             \
        BLOCK_CIPHER_generic(nid,keylen,16,16,cbc,cbc,CBC,flags|EVP_CIPH_FLAG_DEFAULT_ASN1)     \
        BLOCK_CIPHER_generic(nid,keylen,16,0,ecb,ecb,ECB,flags|EVP_CIPH_FLAG_DEFAULT_ASN1)      \
        BLOCK_CIPHER_generic(nid,keylen,1,16,ofb128,ofb,OFB,flags|EVP_CIPH_FLAG_DEFAULT_ASN1)   \
        BLOCK_CIPHER_generic(nid,keylen,1,16,cfb128,cfb,CFB,flags|EVP_CIPH_FLAG_DEFAULT_ASN1)   \
        BLOCK_CIPHER_generic(nid,keylen,1,16,cfb1,cfb1,CFB,flags)       \
        BLOCK_CIPHER_generic(nid,keylen,1,16,cfb8,cfb8,CFB,flags)       \
        BLOCK_CIPHER_generic(nid, keylen, 1, 16, ctr, ctr, CTR, flags)

/* The subkey for Camellia is generated. */
static int camellia_init_key(EVP_CIPHER_CTX *ctx, const unsigned char *key,
                             const unsigned char *iv, int enc)
{
    int ret, mode;
    EVP_CAMELLIA_KEY *dat = EVP_C_DATA(EVP_CAMELLIA_KEY, ctx);

    ret = Camellia_set_key(key, EVP_CIPHER_CTX_get_key_length(ctx) * 8,
                           &dat->ks);
    if (ret < 0) {
        ERR_raise(ERR_LIB_EVP, EVP_R_CAMELLIA_KEY_SETUP_FAILED);
        return 0;
    }

    mode = EVP_CIPHER_CTX_get_mode(ctx);
    if ((mode == EVP_CIPH_ECB_MODE || mode == EVP_CIPH_CBC_MODE)
        && !enc) {
        dat->block = (block128_f) Camellia_decrypt;
        dat->stream.cbc = mode == EVP_CIPH_CBC_MODE ?
            (cbc128_f) Camellia_cbc_encrypt : NULL;
    } else {
        dat->block = (block128_f) Camellia_encrypt;
        dat->stream.cbc = mode == EVP_CIPH_CBC_MODE ?
            (cbc128_f) Camellia_cbc_encrypt : NULL;
    }

    return 1;
}

static int camellia_cbc_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                               const unsigned char *in, size_t len)
{
    EVP_CAMELLIA_KEY *dat = EVP_C_DATA(EVP_CAMELLIA_KEY, ctx);

    if (dat->stream.cbc)
        (*dat->stream.cbc) (in, out, len, &dat->ks, ctx->iv,
                            EVP_CIPHER_CTX_is_encrypting(ctx));
    else if (EVP_CIPHER_CTX_is_encrypting(ctx))
        CRYPTO_cbc128_encrypt(in, out, len, &dat->ks, ctx->iv, dat->block);
    else
        CRYPTO_cbc128_decrypt(in, out, len, &dat->ks, ctx->iv, dat->block);

    return 1;
}

static int camellia_ecb_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                               const unsigned char *in, size_t len)
{
    size_t bl = EVP_CIPHER_CTX_get_block_size(ctx);
    size_t i;
    EVP_CAMELLIA_KEY *dat = EVP_C_DATA(EVP_CAMELLIA_KEY, ctx);

    if (len < bl)
        return 1;

    for (i = 0, len -= bl; i <= len; i += bl)
        (*dat->block) (in + i, out + i, &dat->ks);

    return 1;
}

static int camellia_ofb_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                               const unsigned char *in, size_t len)
{
    EVP_CAMELLIA_KEY *dat = EVP_C_DATA(EVP_CAMELLIA_KEY, ctx);

    int num = EVP_CIPHER_CTX_get_num(ctx);
    CRYPTO_ofb128_encrypt(in, out, len, &dat->ks, ctx->iv, &num, dat->block);
    EVP_CIPHER_CTX_set_num(ctx, num);
    return 1;
}

static int camellia_cfb_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                               const unsigned char *in, size_t len)
{
    EVP_CAMELLIA_KEY *dat = EVP_C_DATA(EVP_CAMELLIA_KEY, ctx);

    int num = EVP_CIPHER_CTX_get_num(ctx);
    CRYPTO_cfb128_encrypt(in, out, len, &dat->ks, ctx->iv, &num,
                          EVP_CIPHER_CTX_is_encrypting(ctx), dat->block);
    EVP_CIPHER_CTX_set_num(ctx, num);
    return 1;
}

static int camellia_cfb8_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                                const unsigned char *in, size_t len)
{
    EVP_CAMELLIA_KEY *dat = EVP_C_DATA(EVP_CAMELLIA_KEY, ctx);

    int num = EVP_CIPHER_CTX_get_num(ctx);
    CRYPTO_cfb128_8_encrypt(in, out, len, &dat->ks, ctx->iv, &num,
                            EVP_CIPHER_CTX_is_encrypting(ctx), dat->block);
    EVP_CIPHER_CTX_set_num(ctx, num);
    return 1;
}

static int camellia_cfb1_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                                const unsigned char *in, size_t len)
{
    EVP_CAMELLIA_KEY *dat = EVP_C_DATA(EVP_CAMELLIA_KEY, ctx);

    if (EVP_CIPHER_CTX_test_flags(ctx, EVP_CIPH_FLAG_LENGTH_BITS)) {
        int num = EVP_CIPHER_CTX_get_num(ctx);
        CRYPTO_cfb128_1_encrypt(in, out, len, &dat->ks, ctx->iv, &num,
                                EVP_CIPHER_CTX_is_encrypting(ctx),
                                dat->block);
        EVP_CIPHER_CTX_set_num(ctx, num);
        return 1;
    }

    while (len >= MAXBITCHUNK) {
        int num = EVP_CIPHER_CTX_get_num(ctx);
        CRYPTO_cfb128_1_encrypt(in, out, MAXBITCHUNK * 8, &dat->ks,
                                ctx->iv, &num,
                                EVP_CIPHER_CTX_is_encrypting(ctx),
                                dat->block);
        EVP_CIPHER_CTX_set_num(ctx, num);
        len -= MAXBITCHUNK;
        out += MAXBITCHUNK;
        in  += MAXBITCHUNK;
    }
    if (len) {
        int num = EVP_CIPHER_CTX_get_num(ctx);
        CRYPTO_cfb128_1_encrypt(in, out, len * 8, &dat->ks,
                                ctx->iv, &num,
                                EVP_CIPHER_CTX_is_encrypting(ctx),
                                dat->block);
        EVP_CIPHER_CTX_set_num(ctx, num);
    }

    return 1;
}

static int camellia_ctr_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                               const unsigned char *in, size_t len)
{
    int snum = EVP_CIPHER_CTX_get_num(ctx);
    unsigned int num;
    EVP_CAMELLIA_KEY *dat = EVP_C_DATA(EVP_CAMELLIA_KEY, ctx);

    if (snum < 0)
        return 0;
    num = snum;
    if (dat->stream.ctr)
        CRYPTO_ctr128_encrypt_ctr32(in, out, len, &dat->ks, ctx->iv,
                                    EVP_CIPHER_CTX_buf_noconst(ctx),
                                    &num,
                                    dat->stream.ctr);
    else
        CRYPTO_ctr128_encrypt(in, out, len, &dat->ks, ctx->iv,
                              EVP_CIPHER_CTX_buf_noconst(ctx), &num,
                              dat->block);
    EVP_CIPHER_CTX_set_num(ctx, num);
    return 1;
}

BLOCK_CIPHER_generic_pack(NID_camellia, 128, 0)
    BLOCK_CIPHER_generic_pack(NID_camellia, 192, 0)
    BLOCK_CIPHER_generic_pack(NID_camellia, 256, 0)
