/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * DES and SHA-1 low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <openssl/sha.h>
#include <openssl/rand.h>
#include <openssl/proverr.h>
#include "cipher_tdes_default.h"
#include "crypto/evp.h"
#include "crypto/sha.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"

#define TDES_WRAP_FLAGS PROV_CIPHER_FLAG_CUSTOM_IV | PROV_CIPHER_FLAG_RAND_KEY

static OSSL_FUNC_cipher_update_fn tdes_wrap_update;
static OSSL_FUNC_cipher_cipher_fn tdes_wrap_cipher;

static const unsigned char wrap_iv[8] = {
    0x4a, 0xdd, 0xa2, 0x2c, 0x79, 0xe8, 0x21, 0x05
};

static int des_ede3_unwrap(PROV_CIPHER_CTX *ctx, unsigned char *out,
                           const unsigned char *in, size_t inl)
{
    unsigned char icv[8], iv[TDES_IVLEN], sha1tmp[SHA_DIGEST_LENGTH];
    int rv = -1;

    if (inl < 24)
        return -1;
    if (out == NULL)
        return inl - 16;

    memcpy(ctx->iv, wrap_iv, 8);
    /* Decrypt first block which will end up as icv */
    ctx->hw->cipher(ctx, icv, in, 8);
    /* Decrypt central blocks */
    /*
     * If decrypting in place move whole output along a block so the next
     * des_ede_cbc_cipher is in place.
     */
    if (out == in) {
        memmove(out, out + 8, inl - 8);
        in -= 8;
    }
    ctx->hw->cipher(ctx, out, in + 8, inl - 16);
    /* Decrypt final block which will be IV */
    ctx->hw->cipher(ctx, iv, in + inl - 8, 8);
    /* Reverse order of everything */
    BUF_reverse(icv, NULL, 8);
    BUF_reverse(out, NULL, inl - 16);
    BUF_reverse(ctx->iv, iv, 8);
    /* Decrypt again using new IV */
    ctx->hw->cipher(ctx, out, out, inl - 16);
    ctx->hw->cipher(ctx, icv, icv, 8);
    if (ossl_sha1(out, inl - 16, sha1tmp) /* Work out hash of first portion */
            && CRYPTO_memcmp(sha1tmp, icv, 8) == 0)
        rv = inl - 16;
    OPENSSL_cleanse(icv, 8);
    OPENSSL_cleanse(sha1tmp, SHA_DIGEST_LENGTH);
    OPENSSL_cleanse(iv, 8);
    OPENSSL_cleanse(ctx->iv, sizeof(ctx->iv));
    if (rv == -1)
        OPENSSL_cleanse(out, inl - 16);

    return rv;
}

static int des_ede3_wrap(PROV_CIPHER_CTX *ctx, unsigned char *out,
                         const unsigned char *in, size_t inl)
{
    unsigned char sha1tmp[SHA_DIGEST_LENGTH];
    size_t ivlen = TDES_IVLEN;
    size_t icvlen = TDES_IVLEN;
    size_t len = inl + ivlen + icvlen;

    if (out == NULL)
        return len;

    /* Copy input to output buffer + 8 so we have space for IV */
    memmove(out + ivlen, in, inl);
    /* Work out ICV */
    if (!ossl_sha1(in, inl, sha1tmp))
        return 0;
    memcpy(out + inl + ivlen, sha1tmp, icvlen);
    OPENSSL_cleanse(sha1tmp, SHA_DIGEST_LENGTH);
    /* Generate random IV */
    if (RAND_bytes_ex(ctx->libctx, ctx->iv, ivlen, 0) <= 0)
        return 0;
    memcpy(out, ctx->iv, ivlen);
    /* Encrypt everything after IV in place */
    ctx->hw->cipher(ctx, out + ivlen, out + ivlen, inl + ivlen);
    BUF_reverse(out, NULL, len);
    memcpy(ctx->iv, wrap_iv, ivlen);
    ctx->hw->cipher(ctx, out, out, len);
    return len;
}

static int tdes_wrap_cipher_internal(PROV_CIPHER_CTX *ctx, unsigned char *out,
                                     const unsigned char *in, size_t inl)
{
    /*
     * Sanity check input length: we typically only wrap keys so EVP_MAXCHUNK
     * is more than will ever be needed. Also input length must be a multiple
     * of 8 bits.
     */
    if (inl >= EVP_MAXCHUNK || inl % 8)
        return -1;
    if (ctx->enc)
        return des_ede3_wrap(ctx, out, in, inl);
    else
        return des_ede3_unwrap(ctx, out, in, inl);
}

static int tdes_wrap_cipher(void *vctx,
                            unsigned char *out, size_t *outl, size_t outsize,
                            const unsigned char *in, size_t inl)
{
    PROV_CIPHER_CTX *ctx = (PROV_CIPHER_CTX *)vctx;
    int ret;

    *outl = 0;
    if (!ossl_prov_is_running())
        return 0;

    if (outsize < inl) {
        ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
        return 0;
    }

    ret = tdes_wrap_cipher_internal(ctx, out, in, inl);
    if (ret <= 0)
        return 0;

    *outl = ret;
    return 1;
}

static int tdes_wrap_update(void *vctx, unsigned char *out, size_t *outl,
                            size_t outsize, const unsigned char *in,
                            size_t inl)
{
    *outl = 0;
    if (inl == 0)
        return 1;
    if (outsize < inl) {
        ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
        return 0;
    }

    if (!tdes_wrap_cipher(vctx, out, outl, outsize, in, inl)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_CIPHER_OPERATION_FAILED);
        return 0;
    }
    return 1;
}


# define IMPLEMENT_WRAP_CIPHER(flags, kbits, blkbits, ivbits)                  \
static OSSL_FUNC_cipher_newctx_fn tdes_wrap_newctx;                            \
static void *tdes_wrap_newctx(void *provctx)                                   \
{                                                                              \
    return ossl_tdes_newctx(provctx, EVP_CIPH_WRAP_MODE, kbits, blkbits,       \
                            ivbits, flags,                                     \
                            ossl_prov_cipher_hw_tdes_wrap_cbc());              \
}                                                                              \
static OSSL_FUNC_cipher_get_params_fn tdes_wrap_get_params;                    \
static int tdes_wrap_get_params(OSSL_PARAM params[])                           \
{                                                                              \
    return ossl_cipher_generic_get_params(params, EVP_CIPH_WRAP_MODE, flags,   \
                                          kbits, blkbits, ivbits);             \
}                                                                              \
const OSSL_DISPATCH ossl_tdes_wrap_cbc_functions[] =                           \
{                                                                              \
    { OSSL_FUNC_CIPHER_ENCRYPT_INIT, (void (*)(void)) ossl_tdes_einit },       \
    { OSSL_FUNC_CIPHER_DECRYPT_INIT, (void (*)(void)) ossl_tdes_dinit },       \
    { OSSL_FUNC_CIPHER_CIPHER, (void (*)(void))tdes_wrap_cipher },             \
    { OSSL_FUNC_CIPHER_NEWCTX, (void (*)(void))tdes_wrap_newctx },             \
    { OSSL_FUNC_CIPHER_FREECTX, (void (*)(void))ossl_tdes_freectx },           \
    { OSSL_FUNC_CIPHER_UPDATE, (void (*)(void))tdes_wrap_update },             \
    { OSSL_FUNC_CIPHER_FINAL,                                                  \
      (void (*)(void))ossl_cipher_generic_stream_final },                      \
    { OSSL_FUNC_CIPHER_GET_PARAMS, (void (*)(void))tdes_wrap_get_params },     \
    { OSSL_FUNC_CIPHER_GETTABLE_PARAMS,                                        \
      (void (*)(void))ossl_cipher_generic_gettable_params },                   \
    { OSSL_FUNC_CIPHER_GET_CTX_PARAMS,                                         \
      (void (*)(void))ossl_tdes_get_ctx_params },                              \
    { OSSL_FUNC_CIPHER_GETTABLE_CTX_PARAMS,                                    \
      (void (*)(void))ossl_tdes_gettable_ctx_params },                         \
    { OSSL_FUNC_CIPHER_SET_CTX_PARAMS,                                         \
      (void (*)(void))ossl_cipher_generic_set_ctx_params },                    \
    { OSSL_FUNC_CIPHER_SETTABLE_CTX_PARAMS,                                    \
      (void (*)(void))ossl_cipher_generic_settable_ctx_params },               \
    OSSL_DISPATCH_END                                                          \
}

/* ossl_tdes_wrap_cbc_functions */
IMPLEMENT_WRAP_CIPHER(TDES_WRAP_FLAGS, 64*3, 64, 0);
