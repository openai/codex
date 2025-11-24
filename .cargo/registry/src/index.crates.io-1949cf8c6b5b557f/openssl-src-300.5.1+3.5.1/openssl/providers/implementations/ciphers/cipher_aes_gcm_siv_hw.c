/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * AES low level APIs are deprecated for public use, but still ok for internal
 * use where we're using them to implement the higher level EVP interface, as is
 * the case here.
 */
#include "internal/deprecated.h"

#include <openssl/evp.h>
#include <internal/endian.h>
#include <prov/implementations.h>
#include "cipher_aes_gcm_siv.h"

static int aes_gcm_siv_ctr32(PROV_AES_GCM_SIV_CTX *ctx, const unsigned char *init_counter,
                             unsigned char *out, const unsigned char *in, size_t len);

static int aes_gcm_siv_initkey(void *vctx)
{
    PROV_AES_GCM_SIV_CTX *ctx = (PROV_AES_GCM_SIV_CTX *)vctx;
    uint8_t output[BLOCK_SIZE];
    uint32_t counter = 0x0;
    size_t i;
    union {
        uint32_t counter;
        uint8_t block[BLOCK_SIZE];
    } data;
    int out_len;
    EVP_CIPHER *ecb = NULL;
    DECLARE_IS_ENDIAN;

    switch (ctx->key_len) {
    case 16:
        ecb = EVP_CIPHER_fetch(ctx->libctx, "AES-128-ECB", NULL);
        break;
    case 24:
        ecb = EVP_CIPHER_fetch(ctx->libctx, "AES-192-ECB", NULL);
        break;
    case 32:
        ecb = EVP_CIPHER_fetch(ctx->libctx, "AES-256-ECB", NULL);
        break;
    default:
        goto err;
    }

    if (ctx->ecb_ctx == NULL && (ctx->ecb_ctx = EVP_CIPHER_CTX_new()) == NULL)
        goto err;
    if (!EVP_EncryptInit_ex2(ctx->ecb_ctx, ecb, ctx->key_gen_key, NULL, NULL))
        goto err;

    memset(&data, 0, sizeof(data));
    memcpy(&data.block[sizeof(data.counter)], ctx->nonce, NONCE_SIZE);

    /* msg_auth_key is always 16 bytes in size, regardless of AES128/AES256 */
    /* counter is stored little-endian */
    for (i = 0; i < BLOCK_SIZE; i += 8) {
        if (IS_LITTLE_ENDIAN) {
            data.counter = counter;
        } else {
            data.counter = GSWAP4(counter);
        }
        /* Block size is 16 (128 bits), but only 8 bytes are used */
        out_len = BLOCK_SIZE;
        if (!EVP_EncryptUpdate(ctx->ecb_ctx, output, &out_len, data.block, BLOCK_SIZE))
            goto err;
        memcpy(&ctx->msg_auth_key[i], output, 8);
        counter++;
    }

    /* msg_enc_key length is directly tied to key length AES128/AES256 */
    for (i = 0; i < ctx->key_len; i += 8) {
        if (IS_LITTLE_ENDIAN) {
            data.counter = counter;
        } else {
            data.counter = GSWAP4(counter);
        }
        /* Block size is 16 bytes (128 bits), but only 8 bytes are used */
        out_len = BLOCK_SIZE;
        if (!EVP_EncryptUpdate(ctx->ecb_ctx, output, &out_len, data.block, BLOCK_SIZE))
            goto err;
        memcpy(&ctx->msg_enc_key[i], output, 8);
        counter++;
    }

    if (!EVP_EncryptInit_ex2(ctx->ecb_ctx, ecb, ctx->msg_enc_key, NULL, NULL))
        goto err;

    /* Freshen up the state */
    ctx->used_enc = 0;
    ctx->used_dec = 0;
    EVP_CIPHER_free(ecb);
    return 1;
 err:
    EVP_CIPHER_CTX_free(ctx->ecb_ctx);
    EVP_CIPHER_free(ecb);
    ctx->ecb_ctx = NULL;
    return 0;
}

static int aes_gcm_siv_aad(PROV_AES_GCM_SIV_CTX *ctx,
                           const unsigned char *aad, size_t len)
{
    size_t to_alloc;
    uint8_t *ptr;
    uint64_t len64;

    /* length of 0 resets the AAD */
    if (len == 0) {
        OPENSSL_free(ctx->aad);
        ctx->aad = NULL;
        ctx->aad_len = 0;
        return 1;
    }
    to_alloc = UP16(ctx->aad_len + len);
    /* need to check the size of the AAD per RFC8452 */
    len64 = to_alloc;
    if (len64 > ((uint64_t)1 << 36))
        return 0;
    ptr = OPENSSL_realloc(ctx->aad, to_alloc);
    if (ptr == NULL)
        return 0;
    ctx->aad = ptr;
    memcpy(&ctx->aad[ctx->aad_len], aad, len);
    ctx->aad_len += len;
    if (to_alloc > ctx->aad_len)
        memset(&ctx->aad[ctx->aad_len], 0, to_alloc - ctx->aad_len);
    return 1;
}

static int aes_gcm_siv_finish(PROV_AES_GCM_SIV_CTX *ctx)
{
    int ret = 0;

    if (ctx->enc)
        return ctx->generated_tag;
    ret = !CRYPTO_memcmp(ctx->tag, ctx->user_tag, sizeof(ctx->tag));
    ret &= ctx->have_user_tag;
    return ret;
}

static int aes_gcm_siv_encrypt(PROV_AES_GCM_SIV_CTX *ctx, const unsigned char *in,
                               unsigned char *out, size_t len)
{
    uint64_t len_blk[2];
    uint8_t S_s[TAG_SIZE];
    uint8_t counter_block[TAG_SIZE];
    uint8_t padding[BLOCK_SIZE];
    size_t i;
    int64_t len64 = len;
    int out_len;
    int error = 0;
    DECLARE_IS_ENDIAN;

    ctx->generated_tag = 0;
    if (!ctx->speed && ctx->used_enc)
        return 0;
    /* need to check the size of the input! */
    if (len64 > ((int64_t)1 << 36))
        return 0;

    if (IS_LITTLE_ENDIAN) {
        len_blk[0] = (uint64_t)ctx->aad_len * 8;
        len_blk[1] = (uint64_t)len * 8;
    } else {
        len_blk[0] = GSWAP8((uint64_t)ctx->aad_len * 8);
        len_blk[1] = GSWAP8((uint64_t)len * 8);
    }
    memset(S_s, 0, TAG_SIZE);
    ossl_polyval_ghash_init(ctx->Htable, (const uint64_t*)ctx->msg_auth_key);

    if (ctx->aad != NULL) {
        /* AAD is allocated with padding, but need to adjust length */
        ossl_polyval_ghash_hash(ctx->Htable, S_s, ctx->aad, UP16(ctx->aad_len));
    }
    if (DOWN16(len) > 0)
        ossl_polyval_ghash_hash(ctx->Htable, S_s, (uint8_t *) in, DOWN16(len));
    if (!IS16(len)) {
        /* deal with padding - probably easier to memset the padding first rather than calculate */
        memset(padding, 0, sizeof(padding));
        memcpy(padding, &in[DOWN16(len)], REMAINDER16(len));
        ossl_polyval_ghash_hash(ctx->Htable, S_s, padding, sizeof(padding));
    }
    ossl_polyval_ghash_hash(ctx->Htable, S_s, (uint8_t *) len_blk, sizeof(len_blk));

    for (i = 0; i < NONCE_SIZE; i++)
        S_s[i] ^= ctx->nonce[i];

    S_s[TAG_SIZE - 1] &= 0x7f;
    out_len = sizeof(ctx->tag);
    error |= !EVP_EncryptUpdate(ctx->ecb_ctx, ctx->tag, &out_len, S_s, sizeof(S_s));
    memcpy(counter_block, ctx->tag, TAG_SIZE);
    counter_block[TAG_SIZE - 1] |= 0x80;

    error |= !aes_gcm_siv_ctr32(ctx, counter_block, out, in, len);

    ctx->generated_tag = !error;
    /* Regardless of error */
    ctx->used_enc = 1;
    return !error;
}

static int aes_gcm_siv_decrypt(PROV_AES_GCM_SIV_CTX *ctx, const unsigned char *in,
                               unsigned char *out, size_t len)
{
    uint8_t counter_block[TAG_SIZE];
    uint64_t len_blk[2];
    uint8_t S_s[TAG_SIZE];
    size_t i;
    uint64_t padding[2];
    int64_t len64 = len;
    int out_len;
    int error = 0;
    DECLARE_IS_ENDIAN;

    ctx->generated_tag = 0;
    if (!ctx->speed && ctx->used_dec)
        return 0;
    /* need to check the size of the input! */
    if (len64 > ((int64_t)1 << 36))
        return 0;

    memcpy(counter_block, ctx->user_tag, sizeof(counter_block));
    counter_block[TAG_SIZE - 1] |= 0x80;

    error |= !aes_gcm_siv_ctr32(ctx, counter_block, out, in, len);

    if (IS_LITTLE_ENDIAN) {
        len_blk[0] = (uint64_t)ctx->aad_len * 8;
        len_blk[1] = (uint64_t)len * 8;
    } else {
        len_blk[0] = GSWAP8((uint64_t)ctx->aad_len * 8);
        len_blk[1] = GSWAP8((uint64_t)len * 8);
    }
    memset(S_s, 0, TAG_SIZE);
    ossl_polyval_ghash_init(ctx->Htable, (const uint64_t*)ctx->msg_auth_key);
    if (ctx->aad != NULL) {
        /* AAD allocated with padding, but need to adjust length */
        ossl_polyval_ghash_hash(ctx->Htable, S_s, ctx->aad, UP16(ctx->aad_len));
    }
    if (DOWN16(len) > 0)
        ossl_polyval_ghash_hash(ctx->Htable, S_s, out, DOWN16(len));
    if (!IS16(len)) {
        /* deal with padding - probably easier to "memset" the padding first rather than calculate */
        padding[0] = padding[1] = 0;
        memcpy(padding, &out[DOWN16(len)], REMAINDER16(len));
        ossl_polyval_ghash_hash(ctx->Htable, S_s, (uint8_t *)padding, sizeof(padding));
    }
    ossl_polyval_ghash_hash(ctx->Htable, S_s, (uint8_t *)len_blk, TAG_SIZE);

    for (i = 0; i < NONCE_SIZE; i++)
        S_s[i] ^= ctx->nonce[i];

    S_s[TAG_SIZE - 1] &= 0x7f;

    /*
     * In the ctx, user_tag is the one received/set by the user,
     * and tag is generated from the input
     */
    out_len = sizeof(ctx->tag);
    error |= !EVP_EncryptUpdate(ctx->ecb_ctx, ctx->tag, &out_len, S_s, sizeof(S_s));
    ctx->generated_tag = !error;
    /* Regardless of error */
    ctx->used_dec = 1;
    return !error;
}

static int aes_gcm_siv_cipher(void *vctx, unsigned char *out,
                              const unsigned char *in, size_t len)
{
    PROV_AES_GCM_SIV_CTX *ctx = (PROV_AES_GCM_SIV_CTX *)vctx;

    /* EncryptFinal or DecryptFinal */
    if (in == NULL)
        return aes_gcm_siv_finish(ctx);

    /* Deal with associated data */
    if (out == NULL)
        return aes_gcm_siv_aad(ctx, in, len);

    if (ctx->enc)
        return aes_gcm_siv_encrypt(ctx, in, out, len);

    return aes_gcm_siv_decrypt(ctx, in, out, len);
}

static void aes_gcm_siv_clean_ctx(void *vctx)
{
    PROV_AES_GCM_SIV_CTX *ctx = (PROV_AES_GCM_SIV_CTX *)vctx;

    EVP_CIPHER_CTX_free(ctx->ecb_ctx);
    ctx->ecb_ctx = NULL;
}

static int aes_gcm_siv_dup_ctx(void *vdst, void *vsrc)
{
    PROV_AES_GCM_SIV_CTX *dst = (PROV_AES_GCM_SIV_CTX *)vdst;
    PROV_AES_GCM_SIV_CTX *src = (PROV_AES_GCM_SIV_CTX *)vsrc;

    dst->ecb_ctx = NULL;
    if (src->ecb_ctx != NULL) {
        if ((dst->ecb_ctx = EVP_CIPHER_CTX_new()) == NULL)
            goto err;
        if (!EVP_CIPHER_CTX_copy(dst->ecb_ctx, src->ecb_ctx))
            goto err;
    }
    return 1;

 err:
    EVP_CIPHER_CTX_free(dst->ecb_ctx);
    dst->ecb_ctx = NULL;
    return 0;
}

static const PROV_CIPHER_HW_AES_GCM_SIV aes_gcm_siv_hw = {
    aes_gcm_siv_initkey,
    aes_gcm_siv_cipher,
    aes_gcm_siv_dup_ctx,
    aes_gcm_siv_clean_ctx,
};

const PROV_CIPHER_HW_AES_GCM_SIV *ossl_prov_cipher_hw_aes_gcm_siv(size_t keybits)
{
    return &aes_gcm_siv_hw;
}

/* AES-GCM-SIV needs AES-CTR32, which is different than the AES-CTR implementation */
static int aes_gcm_siv_ctr32(PROV_AES_GCM_SIV_CTX *ctx, const unsigned char *init_counter,
                             unsigned char *out, const unsigned char *in, size_t len)
{
    uint8_t keystream[BLOCK_SIZE];
    int out_len;
    size_t i;
    size_t j;
    size_t todo;
    uint32_t counter;
    int error = 0;
    union {
        uint32_t x32[BLOCK_SIZE / sizeof(uint32_t)];
        uint8_t x8[BLOCK_SIZE];
    } block;
    DECLARE_IS_ENDIAN;

    memcpy(&block, init_counter, sizeof(block));
    if (IS_BIG_ENDIAN) {
        counter = GSWAP4(block.x32[0]);
    }

    for (i = 0; i < len; i += sizeof(block)) {
        out_len = BLOCK_SIZE;
        error |= !EVP_EncryptUpdate(ctx->ecb_ctx, keystream, &out_len, (uint8_t*)&block, sizeof(block));
        if (IS_LITTLE_ENDIAN) {
            block.x32[0]++;
        } else {
            counter++;
            block.x32[0] = GSWAP4(counter);
        }
        todo = len - i;
        if (todo > sizeof(keystream))
            todo = sizeof(keystream);
        /* Non optimal, but avoids alignment issues */
        for (j = 0; j < todo; j++)
            out[i + j] = in[i + j] ^ keystream[j];
    }
    return !error;
}
