/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 *
 * RFC 9106 Argon2 (see https://www.rfc-editor.org/rfc/rfc9106.txt)
 *
 */

#include <stdlib.h>
#include <stddef.h>
#include <stdarg.h>
#include <string.h>
#include <openssl/e_os2.h>
#include <openssl/evp.h>
#include <openssl/objects.h>
#include <openssl/crypto.h>
#include <openssl/kdf.h>
#include <openssl/err.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include <openssl/thread.h>
#include <openssl/proverr.h>
#include "internal/thread.h"
#include "internal/numbers.h"
#include "internal/endian.h"
#include "crypto/evp.h"
#include "prov/implementations.h"
#include "prov/provider_ctx.h"
#include "prov/providercommon.h"
#include "prov/blake2.h"

#if defined(OPENSSL_NO_DEFAULT_THREAD_POOL) && defined(OPENSSL_NO_THREAD_POOL)
# define ARGON2_NO_THREADS
#endif

#if !defined(OPENSSL_THREADS)
# define ARGON2_NO_THREADS
#endif

#ifndef OPENSSL_NO_ARGON2

# define ARGON2_MIN_LANES 1u
# define ARGON2_MAX_LANES 0xFFFFFFu
# define ARGON2_MIN_THREADS 1u
# define ARGON2_MAX_THREADS 0xFFFFFFu
# define ARGON2_SYNC_POINTS 4u
# define ARGON2_MIN_OUT_LENGTH 4u
# define ARGON2_MAX_OUT_LENGTH 0xFFFFFFFFu
# define ARGON2_MIN_MEMORY (2 * ARGON2_SYNC_POINTS)
# define ARGON2_MIN(a, b) ((a) < (b) ? (a) : (b))
# define ARGON2_MAX_MEMORY 0xFFFFFFFFu
# define ARGON2_MIN_TIME 1u
# define ARGON2_MAX_TIME 0xFFFFFFFFu
# define ARGON2_MIN_PWD_LENGTH 0u
# define ARGON2_MAX_PWD_LENGTH 0xFFFFFFFFu
# define ARGON2_MIN_AD_LENGTH 0u
# define ARGON2_MAX_AD_LENGTH 0xFFFFFFFFu
# define ARGON2_MIN_SALT_LENGTH 8u
# define ARGON2_MAX_SALT_LENGTH 0xFFFFFFFFu
# define ARGON2_MIN_SECRET 0u
# define ARGON2_MAX_SECRET 0xFFFFFFFFu
# define ARGON2_BLOCK_SIZE 1024
# define ARGON2_QWORDS_IN_BLOCK ((ARGON2_BLOCK_SIZE) / 8)
# define ARGON2_OWORDS_IN_BLOCK ((ARGON2_BLOCK_SIZE) / 16)
# define ARGON2_HWORDS_IN_BLOCK ((ARGON2_BLOCK_SIZE) / 32)
# define ARGON2_512BIT_WORDS_IN_BLOCK ((ARGON2_BLOCK_SIZE) / 64)
# define ARGON2_ADDRESSES_IN_BLOCK 128
# define ARGON2_PREHASH_DIGEST_LENGTH 64
# define ARGON2_PREHASH_SEED_LENGTH \
    (ARGON2_PREHASH_DIGEST_LENGTH + (2 * sizeof(uint32_t)))

# define ARGON2_DEFAULT_OUTLEN 64u
# define ARGON2_DEFAULT_T_COST 3u
# define ARGON2_DEFAULT_M_COST ARGON2_MIN_MEMORY
# define ARGON2_DEFAULT_LANES  1u
# define ARGON2_DEFAULT_THREADS 1u
# define ARGON2_DEFAULT_VERSION ARGON2_VERSION_NUMBER

# undef G
# define G(a, b, c, d)                                                        \
    do {                                                                      \
        a = a + b + 2 * mul_lower(a, b);                                      \
        d = rotr64(d ^ a, 32);                                                \
        c = c + d + 2 * mul_lower(c, d);                                      \
        b = rotr64(b ^ c, 24);                                                \
        a = a + b + 2 * mul_lower(a, b);                                      \
        d = rotr64(d ^ a, 16);                                                \
        c = c + d + 2 * mul_lower(c, d);                                      \
        b = rotr64(b ^ c, 63);                                                \
    } while ((void)0, 0)

# undef PERMUTATION_P
# define PERMUTATION_P(v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11,      \
                       v12, v13, v14, v15)                                    \
    do {                                                                      \
        G(v0, v4, v8, v12);                                                   \
        G(v1, v5, v9, v13);                                                   \
        G(v2, v6, v10, v14);                                                  \
        G(v3, v7, v11, v15);                                                  \
        G(v0, v5, v10, v15);                                                  \
        G(v1, v6, v11, v12);                                                  \
        G(v2, v7, v8, v13);                                                   \
        G(v3, v4, v9, v14);                                                   \
    } while ((void)0, 0)

# undef PERMUTATION_P_COLUMN
# define PERMUTATION_P_COLUMN(x, i)                                           \
    do {                                                                      \
        uint64_t *base = &x[16 * i];                                          \
        PERMUTATION_P(                                                        \
            *base,        *(base + 1),  *(base + 2),  *(base + 3),            \
            *(base + 4),  *(base + 5),  *(base + 6),  *(base + 7),            \
            *(base + 8),  *(base + 9),  *(base + 10), *(base + 11),           \
            *(base + 12), *(base + 13), *(base + 14), *(base + 15)            \
        );                                                                    \
    } while ((void)0, 0)

# undef PERMUTATION_P_ROW
# define PERMUTATION_P_ROW(x, i)                                              \
    do {                                                                      \
        uint64_t *base = &x[2 * i];                                           \
        PERMUTATION_P(                                                        \
            *base,        *(base + 1),  *(base + 16),  *(base + 17),          \
            *(base + 32), *(base + 33), *(base + 48),  *(base + 49),          \
            *(base + 64), *(base + 65), *(base + 80),  *(base + 81),          \
            *(base + 96), *(base + 97), *(base + 112), *(base + 113)          \
        );                                                                    \
    } while ((void)0, 0)

typedef struct {
    uint64_t v[ARGON2_QWORDS_IN_BLOCK];
} BLOCK;

typedef enum {
    ARGON2_VERSION_10 = 0x10,
    ARGON2_VERSION_13 = 0x13,
    ARGON2_VERSION_NUMBER = ARGON2_VERSION_13
} ARGON2_VERSION;

typedef enum {
    ARGON2_D  = 0,
    ARGON2_I  = 1,
    ARGON2_ID = 2
} ARGON2_TYPE;

typedef struct {
    uint32_t pass;
    uint32_t lane;
    uint8_t slice;
    uint32_t index;
} ARGON2_POS;

typedef struct {
    void *provctx;
    uint32_t outlen;
    uint8_t *pwd;
    uint32_t pwdlen;
    uint8_t *salt;
    uint32_t saltlen;
    uint8_t *secret;
    uint32_t secretlen;
    uint8_t *ad;
    uint32_t adlen;
    uint32_t t_cost;
    uint32_t m_cost;
    uint32_t lanes;
    uint32_t threads;
    uint32_t version;
    uint32_t early_clean;
    ARGON2_TYPE type;
    BLOCK *memory;
    uint32_t passes;
    uint32_t memory_blocks;
    uint32_t segment_length;
    uint32_t lane_length;
    OSSL_LIB_CTX *libctx;
    EVP_MD *md;
    EVP_MAC *mac;
    char *propq;
} KDF_ARGON2;

typedef struct {
    ARGON2_POS pos;
    KDF_ARGON2 *ctx;
} ARGON2_THREAD_DATA;

static OSSL_FUNC_kdf_newctx_fn kdf_argon2i_new;
static OSSL_FUNC_kdf_newctx_fn kdf_argon2d_new;
static OSSL_FUNC_kdf_newctx_fn kdf_argon2id_new;
static OSSL_FUNC_kdf_freectx_fn kdf_argon2_free;
static OSSL_FUNC_kdf_reset_fn kdf_argon2_reset;
static OSSL_FUNC_kdf_derive_fn kdf_argon2_derive;
static OSSL_FUNC_kdf_settable_ctx_params_fn kdf_argon2_settable_ctx_params;
static OSSL_FUNC_kdf_set_ctx_params_fn kdf_argon2_set_ctx_params;

static void kdf_argon2_init(KDF_ARGON2 *ctx, ARGON2_TYPE t);
static void *kdf_argon2d_new(void *provctx);
static void *kdf_argon2i_new(void *provctx);
static void *kdf_argon2id_new(void *provctx);
static void kdf_argon2_free(void *vctx);
static int kdf_argon2_derive(void *vctx, unsigned char *out, size_t outlen,
                             const OSSL_PARAM params[]);
static void kdf_argon2_reset(void *vctx);
static int kdf_argon2_ctx_set_threads(KDF_ARGON2 *ctx, uint32_t threads);
static int kdf_argon2_ctx_set_lanes(KDF_ARGON2 *ctx, uint32_t lanes);
static int kdf_argon2_ctx_set_t_cost(KDF_ARGON2 *ctx, uint32_t t_cost);
static int kdf_argon2_ctx_set_m_cost(KDF_ARGON2 *ctx, uint32_t m_cost);
static int kdf_argon2_ctx_set_out_length(KDF_ARGON2 *ctx, uint32_t outlen);
static int kdf_argon2_ctx_set_secret(KDF_ARGON2 *ctx, const OSSL_PARAM *p);
static int kdf_argon2_ctx_set_pwd(KDF_ARGON2 *ctx, const OSSL_PARAM *p);
static int kdf_argon2_ctx_set_salt(KDF_ARGON2 *ctx, const OSSL_PARAM *p);
static int kdf_argon2_ctx_set_ad(KDF_ARGON2 *ctx, const OSSL_PARAM *p);
static int kdf_argon2_set_ctx_params(void *vctx, const OSSL_PARAM params[]);
static int kdf_argon2_get_ctx_params(void *vctx, OSSL_PARAM params[]);
static int kdf_argon2_ctx_set_version(KDF_ARGON2 *ctx, uint32_t version);
static const OSSL_PARAM *kdf_argon2_settable_ctx_params(ossl_unused void *ctx,
                                                        ossl_unused void *p_ctx);
static const OSSL_PARAM *kdf_argon2_gettable_ctx_params(ossl_unused void *ctx,
                                                        ossl_unused void *p_ctx);

static ossl_inline uint64_t load64(const uint8_t *src);
static ossl_inline void store32(uint8_t *dst, uint32_t w);
static ossl_inline void store64(uint8_t *dst, uint64_t w);
static ossl_inline uint64_t rotr64(const uint64_t w, const unsigned int c);
static ossl_inline uint64_t mul_lower(uint64_t x, uint64_t y);

static void init_block_value(BLOCK *b, uint8_t in);
static void copy_block(BLOCK *dst, const BLOCK *src);
static void xor_block(BLOCK *dst, const BLOCK *src);
static void load_block(BLOCK *dst, const void *input);
static void store_block(void *output, const BLOCK *src);
static void fill_first_blocks(uint8_t *blockhash, const KDF_ARGON2 *ctx);
static void fill_block(const BLOCK *prev, const BLOCK *ref, BLOCK *next,
                       int with_xor);

static void next_addresses(BLOCK *address_block, BLOCK *input_block,
                           const BLOCK *zero_block);
static int data_indep_addressing(const KDF_ARGON2 *ctx, uint32_t pass,
                                 uint8_t slice);
static uint32_t index_alpha(const KDF_ARGON2 *ctx, uint32_t pass,
                            uint8_t slice, uint32_t index,
                            uint32_t pseudo_rand, int same_lane);

static void fill_segment(const KDF_ARGON2 *ctx, uint32_t pass, uint32_t lane,
                         uint8_t slice);

# if !defined(ARGON2_NO_THREADS)
static uint32_t fill_segment_thr(void *thread_data);
static int fill_mem_blocks_mt(KDF_ARGON2 *ctx);
# endif

static int fill_mem_blocks_st(KDF_ARGON2 *ctx);
static ossl_inline int fill_memory_blocks(KDF_ARGON2 *ctx);

static void initial_hash(uint8_t *blockhash, KDF_ARGON2 *ctx);
static int initialize(KDF_ARGON2 *ctx);
static void finalize(const KDF_ARGON2 *ctx, void *out);

static int blake2b(EVP_MD *md, EVP_MAC *mac, void *out, size_t outlen,
                   const void *in, size_t inlen, const void *key,
                   size_t keylen);
static int blake2b_long(EVP_MD *md, EVP_MAC *mac, unsigned char *out,
                        size_t outlen, const void *in, size_t inlen);

static ossl_inline uint64_t load64(const uint8_t *src)
{
    return
      (((uint64_t)src[0]) << 0)
    | (((uint64_t)src[1]) << 8)
    | (((uint64_t)src[2]) << 16)
    | (((uint64_t)src[3]) << 24)
    | (((uint64_t)src[4]) << 32)
    | (((uint64_t)src[5]) << 40)
    | (((uint64_t)src[6]) << 48)
    | (((uint64_t)src[7]) << 56);
}

static ossl_inline void store32(uint8_t *dst, uint32_t w)
{
    dst[0] = (uint8_t)(w >> 0);
    dst[1] = (uint8_t)(w >> 8);
    dst[2] = (uint8_t)(w >> 16);
    dst[3] = (uint8_t)(w >> 24);
}

static ossl_inline void store64(uint8_t *dst, uint64_t w)
{
    dst[0] = (uint8_t)(w >> 0);
    dst[1] = (uint8_t)(w >> 8);
    dst[2] = (uint8_t)(w >> 16);
    dst[3] = (uint8_t)(w >> 24);
    dst[4] = (uint8_t)(w >> 32);
    dst[5] = (uint8_t)(w >> 40);
    dst[6] = (uint8_t)(w >> 48);
    dst[7] = (uint8_t)(w >> 56);
}

static ossl_inline uint64_t rotr64(const uint64_t w, const unsigned int c)
{
    return (w >> c) | (w << (64 - c));
}

static ossl_inline uint64_t mul_lower(uint64_t x, uint64_t y)
{
    const uint64_t m = 0xFFFFFFFFUL;
    return (x & m) * (y & m);
}

static void init_block_value(BLOCK *b, uint8_t in)
{
    memset(b->v, in, sizeof(b->v));
}

static void copy_block(BLOCK *dst, const BLOCK *src)
{
    memcpy(dst->v, src->v, sizeof(uint64_t) * ARGON2_QWORDS_IN_BLOCK);
}

static void xor_block(BLOCK *dst, const BLOCK *src)
{
    int i;

    for (i = 0; i < ARGON2_QWORDS_IN_BLOCK; ++i)
        dst->v[i] ^= src->v[i];
}

static void load_block(BLOCK *dst, const void *input)
{
    unsigned i;

    for (i = 0; i < ARGON2_QWORDS_IN_BLOCK; ++i)
        dst->v[i] = load64((const uint8_t *)input + i * sizeof(dst->v[i]));
}

static void store_block(void *output, const BLOCK *src)
{
    unsigned i;

    for (i = 0; i < ARGON2_QWORDS_IN_BLOCK; ++i)
        store64((uint8_t *)output + i * sizeof(src->v[i]), src->v[i]);
}

static void fill_first_blocks(uint8_t *blockhash, const KDF_ARGON2 *ctx)
{
    uint32_t l;
    uint8_t blockhash_bytes[ARGON2_BLOCK_SIZE];

    /*
     * Make the first and second block in each lane as G(H0||0||i)
     * or G(H0||1||i).
     */
    for (l = 0; l < ctx->lanes; ++l) {
        store32(blockhash + ARGON2_PREHASH_DIGEST_LENGTH, 0);
        store32(blockhash + ARGON2_PREHASH_DIGEST_LENGTH + 4, l);
        blake2b_long(ctx->md, ctx->mac, blockhash_bytes, ARGON2_BLOCK_SIZE,
                     blockhash, ARGON2_PREHASH_SEED_LENGTH);
        load_block(&ctx->memory[l * ctx->lane_length + 0],
                   blockhash_bytes);
        store32(blockhash + ARGON2_PREHASH_DIGEST_LENGTH, 1);
        blake2b_long(ctx->md, ctx->mac, blockhash_bytes, ARGON2_BLOCK_SIZE,
                     blockhash, ARGON2_PREHASH_SEED_LENGTH);
        load_block(&ctx->memory[l * ctx->lane_length + 1],
                   blockhash_bytes);
    }
    OPENSSL_cleanse(blockhash_bytes, ARGON2_BLOCK_SIZE);
}

static void fill_block(const BLOCK *prev, const BLOCK *ref,
                       BLOCK *next, int with_xor)
{
    BLOCK blockR, tmp;
    unsigned i;

    copy_block(&blockR, ref);
    xor_block(&blockR, prev);
    copy_block(&tmp, &blockR);

    if (with_xor)
        xor_block(&tmp, next);

    for (i = 0; i < 8; ++i)
        PERMUTATION_P_COLUMN(blockR.v, i);

    for (i = 0; i < 8; ++i)
        PERMUTATION_P_ROW(blockR.v, i);

    copy_block(next, &tmp);
    xor_block(next, &blockR);
}

static void next_addresses(BLOCK *address_block, BLOCK *input_block,
                           const BLOCK *zero_block)
{
    input_block->v[6]++;
    fill_block(zero_block, input_block, address_block, 0);
    fill_block(zero_block, address_block, address_block, 0);
}

static int data_indep_addressing(const KDF_ARGON2 *ctx, uint32_t pass,
                                 uint8_t slice)
{
    switch (ctx->type) {
    case ARGON2_I:
        return 1;
    case ARGON2_ID:
        return (pass == 0) && (slice < ARGON2_SYNC_POINTS / 2);
    case ARGON2_D:
    default:
        return 0;
    }
}

/*
 * Pass 0 (pass = 0):
 * This lane: all already finished segments plus already constructed blocks
 *            in this segment
 * Other lanes: all already finished segments
 *
 * Pass 1+:
 * This lane: (SYNC_POINTS - 1) last segments plus already constructed
 *            blocks in this segment
 * Other lanes: (SYNC_POINTS - 1) last segments
 */
static uint32_t index_alpha(const KDF_ARGON2 *ctx, uint32_t pass,
                            uint8_t slice, uint32_t index,
                            uint32_t pseudo_rand, int same_lane)
{
    uint32_t ref_area_sz;
    uint64_t rel_pos;
    uint32_t start_pos, abs_pos;

    start_pos = 0;
    switch (pass) {
    case 0:
        if (slice == 0)
            ref_area_sz = index - 1;
        else if (same_lane)
            ref_area_sz = slice * ctx->segment_length + index - 1;
        else
            ref_area_sz = slice * ctx->segment_length +
                ((index == 0) ? (-1) : 0);
        break;
    default:
        if (same_lane)
            ref_area_sz = ctx->lane_length - ctx->segment_length + index - 1;
        else
            ref_area_sz = ctx->lane_length - ctx->segment_length +
                ((index == 0) ? (-1) : 0);
        if (slice != ARGON2_SYNC_POINTS - 1)
            start_pos = (slice + 1) * ctx->segment_length;
        break;
    }

    rel_pos = pseudo_rand;
    rel_pos = rel_pos * rel_pos >> 32;
    rel_pos = ref_area_sz - 1 - (ref_area_sz * rel_pos >> 32);
    abs_pos = (start_pos + rel_pos) % ctx->lane_length;

    return abs_pos;
}

static void fill_segment(const KDF_ARGON2 *ctx, uint32_t pass, uint32_t lane,
                         uint8_t slice)
{
    BLOCK *ref_block = NULL, *curr_block = NULL;
    BLOCK address_block, input_block, zero_block;
    uint64_t rnd, ref_index, ref_lane;
    uint32_t prev_offset;
    uint32_t start_idx;
    uint32_t j;
    uint32_t curr_offset; /* Offset of the current block */

    memset(&input_block, 0, sizeof(BLOCK));

    if (ctx == NULL)
        return;

    if (data_indep_addressing(ctx, pass, slice)) {
        init_block_value(&zero_block, 0);
        init_block_value(&input_block, 0);

        input_block.v[0] = pass;
        input_block.v[1] = lane;
        input_block.v[2] = slice;
        input_block.v[3] = ctx->memory_blocks;
        input_block.v[4] = ctx->passes;
        input_block.v[5] = ctx->type;
    }

    start_idx = 0;

    /* We've generated the first two blocks. Generate the 1st block of addrs. */
    if ((pass == 0) && (slice == 0)) {
        start_idx = 2;
        if (data_indep_addressing(ctx, pass, slice))
            next_addresses(&address_block, &input_block, &zero_block);
    }

    curr_offset = lane * ctx->lane_length + slice * ctx->segment_length
        + start_idx;

    if ((curr_offset % ctx->lane_length) == 0)
        prev_offset = curr_offset + ctx->lane_length - 1;
    else
        prev_offset = curr_offset - 1;

    for (j = start_idx; j < ctx->segment_length; ++j, ++curr_offset, ++prev_offset) {
        if (curr_offset % ctx->lane_length == 1)
            prev_offset = curr_offset - 1;

        /* Taking pseudo-random value from the previous block. */
        if (data_indep_addressing(ctx, pass, slice)) {
            if (j % ARGON2_ADDRESSES_IN_BLOCK == 0)
                next_addresses(&address_block, &input_block, &zero_block);
            rnd = address_block.v[j % ARGON2_ADDRESSES_IN_BLOCK];
        } else {
            rnd = ctx->memory[prev_offset].v[0];
        }

        /* Computing the lane of the reference block */
        ref_lane = ((rnd >> 32)) % ctx->lanes;
        /* Can not reference other lanes yet */
        if ((pass == 0) && (slice == 0))
            ref_lane = lane;

        /* Computing the number of possible reference block within the lane. */
        ref_index = index_alpha(ctx, pass, slice, j, rnd & 0xFFFFFFFF,
                                ref_lane == lane);

        /* Creating a new block */
        ref_block = ctx->memory + ctx->lane_length * ref_lane + ref_index;
        curr_block = ctx->memory + curr_offset;
        if (ARGON2_VERSION_10 == ctx->version) {
            /* Version 1.2.1 and earlier: overwrite, not XOR */
            fill_block(ctx->memory + prev_offset, ref_block, curr_block, 0);
            continue;
        }

        fill_block(ctx->memory + prev_offset, ref_block, curr_block,
                   pass == 0 ? 0 : 1);
    }
}

# if !defined(ARGON2_NO_THREADS)

static uint32_t fill_segment_thr(void *thread_data)
{
    ARGON2_THREAD_DATA *my_data;

    my_data = (ARGON2_THREAD_DATA *) thread_data;
    fill_segment(my_data->ctx, my_data->pos.pass, my_data->pos.lane,
                 my_data->pos.slice);

    return 0;
}

static int fill_mem_blocks_mt(KDF_ARGON2 *ctx)
{
    uint32_t r, s, l, ll;
    void **t;
    ARGON2_THREAD_DATA *t_data;

    t = OPENSSL_zalloc(sizeof(void *)*ctx->lanes);
    t_data = OPENSSL_zalloc(ctx->lanes * sizeof(ARGON2_THREAD_DATA));

    if (t == NULL || t_data == NULL)
        goto fail;

    for (r = 0; r < ctx->passes; ++r) {
        for (s = 0; s < ARGON2_SYNC_POINTS; ++s) {
            for (l = 0; l < ctx->lanes; ++l) {
                ARGON2_POS p;
                if (l >= ctx->threads) {
                    if (ossl_crypto_thread_join(t[l - ctx->threads], NULL) == 0)
                        goto fail;
                    if (ossl_crypto_thread_clean(t[l - ctx->threads]) == 0)
                        goto fail;
                    t[l] = NULL;
                }

                p.pass = r;
                p.lane = l;
                p.slice = (uint8_t)s;
                p.index = 0;

                t_data[l].ctx = ctx;
                memcpy(&(t_data[l].pos), &p, sizeof(ARGON2_POS));
                t[l] = ossl_crypto_thread_start(ctx->libctx, &fill_segment_thr,
                                                (void *) &t_data[l]);
                if (t[l] == NULL) {
                    for (ll = 0; ll < l; ++ll) {
                        if (ossl_crypto_thread_join(t[ll], NULL) == 0)
                            goto fail;
                        if (ossl_crypto_thread_clean(t[ll]) == 0)
                            goto fail;
                        t[ll] = NULL;
                    }
                    goto fail;
                }
            }
            for (l = ctx->lanes - ctx->threads; l < ctx->lanes; ++l) {
                if (ossl_crypto_thread_join(t[l], NULL) == 0)
                    goto fail;
                if (ossl_crypto_thread_clean(t[l]) == 0)
                    goto fail;
                t[l] = NULL;
            }
        }
    }

    OPENSSL_free(t_data);
    OPENSSL_free(t);

    return 1;

fail:
    if (t_data != NULL)
        OPENSSL_free(t_data);
    if (t != NULL)
        OPENSSL_free(t);
    return 0;
}

# endif /* !defined(ARGON2_NO_THREADS) */

static int fill_mem_blocks_st(KDF_ARGON2 *ctx)
{
    uint32_t r, s, l;

    for (r = 0; r < ctx->passes; ++r)
        for (s = 0; s < ARGON2_SYNC_POINTS; ++s)
            for (l = 0; l < ctx->lanes; ++l)
                fill_segment(ctx, r, l, s);
    return 1;
}

static ossl_inline int fill_memory_blocks(KDF_ARGON2 *ctx)
{
# if !defined(ARGON2_NO_THREADS)
    return ctx->threads == 1 ? fill_mem_blocks_st(ctx) : fill_mem_blocks_mt(ctx);
# else
    return ctx->threads == 1 ? fill_mem_blocks_st(ctx) : 0;
# endif
}

static void initial_hash(uint8_t *blockhash, KDF_ARGON2 *ctx)
{
    EVP_MD_CTX *mdctx;
    uint8_t value[sizeof(uint32_t)];
    unsigned int tmp;
    uint32_t args[7];

    if (ctx == NULL || blockhash == NULL)
        return;

    args[0] = ctx->lanes;
    args[1] = ctx->outlen;
    args[2] = ctx->m_cost;
    args[3] = ctx->t_cost;
    args[4] = ctx->version;
    args[5] = (uint32_t) ctx->type;
    args[6] = ctx->pwdlen;

    mdctx = EVP_MD_CTX_create();
    if (mdctx == NULL || EVP_DigestInit_ex(mdctx, ctx->md, NULL) != 1)
        goto fail;

    for (tmp = 0; tmp < sizeof(args) / sizeof(uint32_t); ++tmp) {
        store32((uint8_t *) &value, args[tmp]);
        if (EVP_DigestUpdate(mdctx, &value, sizeof(value)) != 1)
            goto fail;
    }

    if (ctx->pwd != NULL) {
        if (EVP_DigestUpdate(mdctx, ctx->pwd, ctx->pwdlen) != 1)
            goto fail;
        if (ctx->early_clean) {
            OPENSSL_cleanse(ctx->pwd, ctx->pwdlen);
            ctx->pwdlen = 0;
        }
    }

    store32((uint8_t *) &value, ctx->saltlen);

    if (EVP_DigestUpdate(mdctx, &value, sizeof(value)) != 1)
        goto fail;

    if (ctx->salt != NULL)
        if (EVP_DigestUpdate(mdctx, ctx->salt, ctx->saltlen) != 1)
            goto fail;

    store32((uint8_t *) &value, ctx->secretlen);
    if (EVP_DigestUpdate(mdctx, &value, sizeof(value)) != 1)
        goto fail;

    if (ctx->secret != NULL) {
        if (EVP_DigestUpdate(mdctx, ctx->secret, ctx->secretlen) != 1)
            goto fail;
        if (ctx->early_clean) {
            OPENSSL_cleanse(ctx->secret, ctx->secretlen);
            ctx->secretlen = 0;
        }
    }

    store32((uint8_t *) &value, ctx->adlen);
    if (EVP_DigestUpdate(mdctx, &value, sizeof(value)) != 1)
        goto fail;

    if (ctx->ad != NULL)
        if (EVP_DigestUpdate(mdctx, ctx->ad, ctx->adlen) != 1)
            goto fail;

    tmp = ARGON2_PREHASH_DIGEST_LENGTH;
    if (EVP_DigestFinal_ex(mdctx, blockhash, &tmp) != 1)
        goto fail;

fail:
    EVP_MD_CTX_destroy(mdctx);
}

static int initialize(KDF_ARGON2 *ctx)
{
    uint8_t blockhash[ARGON2_PREHASH_SEED_LENGTH];

    if (ctx == NULL)
        return 0;

    if (ctx->memory_blocks * sizeof(BLOCK) / sizeof(BLOCK) != ctx->memory_blocks)
        return 0;

    if (ctx->type != ARGON2_D)
        ctx->memory = OPENSSL_secure_zalloc(ctx->memory_blocks *
                                            sizeof(BLOCK));
    else
        ctx->memory = OPENSSL_zalloc(ctx->memory_blocks *
                                     sizeof(BLOCK));

    if (ctx->memory == NULL) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_MEMORY_SIZE,
                       "cannot allocate required memory");
        return 0;
    }

    initial_hash(blockhash, ctx);
    OPENSSL_cleanse(blockhash + ARGON2_PREHASH_DIGEST_LENGTH,
                    ARGON2_PREHASH_SEED_LENGTH - ARGON2_PREHASH_DIGEST_LENGTH);
    fill_first_blocks(blockhash, ctx);
    OPENSSL_cleanse(blockhash, ARGON2_PREHASH_SEED_LENGTH);

    return 1;
}

static void finalize(const KDF_ARGON2 *ctx, void *out)
{
    BLOCK blockhash;
    uint8_t blockhash_bytes[ARGON2_BLOCK_SIZE];
    uint32_t last_block_in_lane;
    uint32_t l;

    if (ctx == NULL)
        return;

    copy_block(&blockhash, ctx->memory + ctx->lane_length - 1);

    /* XOR the last blocks */
    for (l = 1; l < ctx->lanes; ++l) {
        last_block_in_lane = l * ctx->lane_length + (ctx->lane_length - 1);
        xor_block(&blockhash, ctx->memory + last_block_in_lane);
    }

    /* Hash the result */
    store_block(blockhash_bytes, &blockhash);
    blake2b_long(ctx->md, ctx->mac, out, ctx->outlen, blockhash_bytes,
                 ARGON2_BLOCK_SIZE);
    OPENSSL_cleanse(blockhash.v, ARGON2_BLOCK_SIZE);
    OPENSSL_cleanse(blockhash_bytes, ARGON2_BLOCK_SIZE);

    if (ctx->type != ARGON2_D)
        OPENSSL_secure_clear_free(ctx->memory,
                                  ctx->memory_blocks * sizeof(BLOCK));
    else
        OPENSSL_clear_free(ctx->memory,
                           ctx->memory_blocks * sizeof(BLOCK));
}

static int blake2b_mac(EVP_MAC *mac, void *out, size_t outlen, const void *in,
                       size_t inlen, const void *key, size_t keylen)
{
    int ret = 0;
    size_t par_n = 0, out_written;
    EVP_MAC_CTX *ctx = NULL;
    OSSL_PARAM par[3];

    if ((ctx = EVP_MAC_CTX_new(mac)) == NULL)
        goto fail;

    par[par_n++] = OSSL_PARAM_construct_octet_string(OSSL_MAC_PARAM_KEY,
                                                     (void *) key, keylen);
    par[par_n++] = OSSL_PARAM_construct_size_t(OSSL_MAC_PARAM_SIZE, &outlen);
    par[par_n++] = OSSL_PARAM_construct_end();

    ret = EVP_MAC_CTX_set_params(ctx, par) == 1
        && EVP_MAC_init(ctx, NULL, 0, NULL) == 1
        && EVP_MAC_update(ctx, in, inlen) == 1
        && EVP_MAC_final(ctx, out, (size_t *) &out_written, outlen) == 1;

fail:
    EVP_MAC_CTX_free(ctx);
    return ret;
}

static int blake2b_md(EVP_MD *md, void *out, size_t outlen, const void *in,
                      size_t inlen)
{
    int ret = 0;
    EVP_MD_CTX *ctx = NULL;
    OSSL_PARAM par[2];

    if ((ctx = EVP_MD_CTX_create()) == NULL)
        return 0;

    par[0] = OSSL_PARAM_construct_size_t(OSSL_DIGEST_PARAM_SIZE, &outlen);
    par[1] = OSSL_PARAM_construct_end();

    ret = EVP_DigestInit_ex2(ctx, md, par) == 1
        && EVP_DigestUpdate(ctx, in, inlen) == 1
        && EVP_DigestFinal_ex(ctx, out, NULL) == 1;

    EVP_MD_CTX_free(ctx);
    return ret;
}

static int blake2b(EVP_MD *md, EVP_MAC *mac, void *out, size_t outlen,
                   const void *in, size_t inlen, const void *key, size_t keylen)
{
    if (out == NULL || outlen == 0)
        return 0;

    if (key == NULL || keylen == 0)
        return blake2b_md(md, out, outlen, in, inlen);

    return blake2b_mac(mac, out, outlen, in, inlen, key, keylen);
}

static int blake2b_long(EVP_MD *md, EVP_MAC *mac, unsigned char *out,
                        size_t outlen, const void *in, size_t inlen)
{
    int ret = 0;
    EVP_MD_CTX *ctx = NULL;
    uint32_t outlen_curr;
    uint8_t outbuf[BLAKE2B_OUTBYTES];
    uint8_t inbuf[BLAKE2B_OUTBYTES];
    uint8_t outlen_bytes[sizeof(uint32_t)] = {0};
    OSSL_PARAM par[2];
    size_t outlen_md;

    if (out == NULL || outlen == 0)
        return 0;

    /* Ensure little-endian byte order */
    store32(outlen_bytes, (uint32_t)outlen);

    if ((ctx = EVP_MD_CTX_create()) == NULL)
        return 0;

    outlen_md = (outlen <= BLAKE2B_OUTBYTES) ? outlen : BLAKE2B_OUTBYTES;
    par[0] = OSSL_PARAM_construct_size_t(OSSL_DIGEST_PARAM_SIZE, &outlen_md);
    par[1] = OSSL_PARAM_construct_end();

    ret = EVP_DigestInit_ex2(ctx, md, par) == 1
        && EVP_DigestUpdate(ctx, outlen_bytes, sizeof(outlen_bytes)) == 1
        && EVP_DigestUpdate(ctx, in, inlen) == 1
        && EVP_DigestFinal_ex(ctx, (outlen > BLAKE2B_OUTBYTES) ? outbuf : out,
                              NULL) == 1;

    if (ret == 0)
        goto fail;

    if (outlen > BLAKE2B_OUTBYTES) {
        memcpy(out, outbuf, BLAKE2B_OUTBYTES / 2);
        out += BLAKE2B_OUTBYTES / 2;
        outlen_curr = (uint32_t) outlen - BLAKE2B_OUTBYTES / 2;

        while (outlen_curr > BLAKE2B_OUTBYTES) {
            memcpy(inbuf, outbuf, BLAKE2B_OUTBYTES);
            if (blake2b(md, mac, outbuf, BLAKE2B_OUTBYTES, inbuf,
                        BLAKE2B_OUTBYTES, NULL, 0) != 1)
                goto fail;
            memcpy(out, outbuf, BLAKE2B_OUTBYTES / 2);
            out += BLAKE2B_OUTBYTES / 2;
            outlen_curr -= BLAKE2B_OUTBYTES / 2;
        }

        memcpy(inbuf, outbuf, BLAKE2B_OUTBYTES);
        if (blake2b(md, mac, outbuf, outlen_curr, inbuf, BLAKE2B_OUTBYTES,
                    NULL, 0) != 1)
            goto fail;
        memcpy(out, outbuf, outlen_curr);
    }
    ret = 1;

fail:
    EVP_MD_CTX_free(ctx);
    return ret;
}

static void kdf_argon2_init(KDF_ARGON2 *c, ARGON2_TYPE type)
{
    OSSL_LIB_CTX *libctx;

    libctx = c->libctx;
    memset(c, 0, sizeof(*c));

    c->libctx = libctx;
    c->outlen = ARGON2_DEFAULT_OUTLEN;
    c->t_cost = ARGON2_DEFAULT_T_COST;
    c->m_cost = ARGON2_DEFAULT_M_COST;
    c->lanes = ARGON2_DEFAULT_LANES;
    c->threads = ARGON2_DEFAULT_THREADS;
    c->version = ARGON2_DEFAULT_VERSION;
    c->type = type;
}

static void *kdf_argon2d_new(void *provctx)
{
    KDF_ARGON2 *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_MALLOC_FAILURE);
        return NULL;
    }

    ctx->libctx = PROV_LIBCTX_OF(provctx);

    kdf_argon2_init(ctx, ARGON2_D);
    return ctx;
}

static void *kdf_argon2i_new(void *provctx)
{
    KDF_ARGON2 *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_MALLOC_FAILURE);
        return NULL;
    }

    ctx->libctx = PROV_LIBCTX_OF(provctx);

    kdf_argon2_init(ctx, ARGON2_I);
    return ctx;
}

static void *kdf_argon2id_new(void *provctx)
{
    KDF_ARGON2 *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_MALLOC_FAILURE);
        return NULL;
    }

    ctx->libctx = PROV_LIBCTX_OF(provctx);

    kdf_argon2_init(ctx, ARGON2_ID);
    return ctx;
}

static void kdf_argon2_free(void *vctx)
{
    KDF_ARGON2 *ctx = (KDF_ARGON2 *)vctx;

    if (ctx == NULL)
        return;

    if (ctx->pwd != NULL)
        OPENSSL_clear_free(ctx->pwd, ctx->pwdlen);

    if (ctx->salt != NULL)
        OPENSSL_clear_free(ctx->salt, ctx->saltlen);

    if (ctx->secret != NULL)
        OPENSSL_clear_free(ctx->secret, ctx->secretlen);

    if (ctx->ad != NULL)
        OPENSSL_clear_free(ctx->ad, ctx->adlen);

    EVP_MD_free(ctx->md);
    EVP_MAC_free(ctx->mac);

    OPENSSL_free(ctx->propq);

    memset(ctx, 0, sizeof(*ctx));

    OPENSSL_free(ctx);
}

static int kdf_argon2_derive(void *vctx, unsigned char *out, size_t outlen,
                             const OSSL_PARAM params[])
{
    KDF_ARGON2 *ctx;
    uint32_t memory_blocks, segment_length;

    ctx = (KDF_ARGON2 *)vctx;

    if (!ossl_prov_is_running() || !kdf_argon2_set_ctx_params(vctx, params))
        return 0;

    if (ctx->mac == NULL)
        ctx->mac = EVP_MAC_fetch(ctx->libctx, "blake2bmac", ctx->propq);
    if (ctx->mac == NULL) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_MISSING_MAC,
                       "cannot fetch blake2bmac");
        return 0;
    }

    if (ctx->md == NULL)
        ctx->md = EVP_MD_fetch(ctx->libctx, "blake2b512", ctx->propq);
    if (ctx->md == NULL) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_MISSING_MESSAGE_DIGEST,
                       "cannot fetch blake2b512");
        return 0;
    }

    if (ctx->salt == NULL || ctx->saltlen == 0) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_SALT);
        return 0;
    }

    if (outlen != ctx->outlen) {
        if (OSSL_PARAM_locate((OSSL_PARAM *)params, "size") != NULL) {
            ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
            return 0;
        }
        if (!kdf_argon2_ctx_set_out_length(ctx, (uint32_t) outlen))
            return 0;
    }

    switch (ctx->type) {
    case ARGON2_D:
    case ARGON2_I:
    case ARGON2_ID:
        break;
    default:
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_MODE, "invalid Argon2 type");
        return 0;
    }

    if (ctx->threads > 1) {
# ifdef ARGON2_NO_THREADS
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_THREAD_POOL_SIZE,
                       "requested %u threads, single-threaded mode supported only",
                       ctx->threads);
        return 0;
# else
        if (ctx->threads > ossl_get_avail_threads(ctx->libctx)) {
            ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_THREAD_POOL_SIZE,
                           "requested %u threads, available: %u",
                           ctx->threads, ossl_get_avail_threads(ctx->libctx));
            return 0;
        }
# endif
        if (ctx->threads > ctx->lanes) {
            ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_THREAD_POOL_SIZE,
                           "requested more threads (%u) than lanes (%u)",
                           ctx->threads, ctx->lanes);
            return 0;
        }
    }

    if (ctx->m_cost < 8 * ctx->lanes) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_MEMORY_SIZE,
                       "m_cost must be greater or equal than 8 times the number of lanes");
        return 0;
    }

    memory_blocks = ctx->m_cost;
    if (memory_blocks < 2 * ARGON2_SYNC_POINTS * ctx->lanes)
        memory_blocks = 2 * ARGON2_SYNC_POINTS * ctx->lanes;

    /* Ensure that all segments have equal length */
    segment_length = memory_blocks / (ctx->lanes * ARGON2_SYNC_POINTS);
    memory_blocks = segment_length * (ctx->lanes * ARGON2_SYNC_POINTS);

    ctx->memory = NULL;
    ctx->memory_blocks = memory_blocks;
    ctx->segment_length = segment_length;
    ctx->passes = ctx->t_cost;
    ctx->lane_length = segment_length * ARGON2_SYNC_POINTS;

    if (initialize(ctx) != 1)
        return 0;

    if (fill_memory_blocks(ctx) != 1)
        return 0;

    finalize(ctx, out);

    return 1;
}

static void kdf_argon2_reset(void *vctx)
{
    OSSL_LIB_CTX *libctx;
    KDF_ARGON2 *ctx;
    ARGON2_TYPE type;

    ctx = (KDF_ARGON2 *) vctx;
    type = ctx->type;
    libctx = ctx->libctx;

    EVP_MD_free(ctx->md);
    EVP_MAC_free(ctx->mac);

    OPENSSL_free(ctx->propq);

    if (ctx->pwd != NULL)
        OPENSSL_clear_free(ctx->pwd, ctx->pwdlen);

    if (ctx->salt != NULL)
        OPENSSL_clear_free(ctx->salt, ctx->saltlen);

    if (ctx->secret != NULL)
        OPENSSL_clear_free(ctx->secret, ctx->secretlen);

    if (ctx->ad != NULL)
        OPENSSL_clear_free(ctx->ad, ctx->adlen);

    memset(ctx, 0, sizeof(*ctx));
    ctx->libctx = libctx;
    kdf_argon2_init(ctx, type);
}

static int kdf_argon2_ctx_set_threads(KDF_ARGON2 *ctx, uint32_t threads)
{
    if (threads < ARGON2_MIN_THREADS) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_THREAD_POOL_SIZE,
                       "min threads: %u", ARGON2_MIN_THREADS);
        return 0;
    }

    if (threads > ARGON2_MAX_THREADS) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_THREAD_POOL_SIZE,
                       "max threads: %u", ARGON2_MAX_THREADS);
        return 0;
    }

    ctx->threads = threads;
    return 1;
}

static int kdf_argon2_ctx_set_lanes(KDF_ARGON2 *ctx, uint32_t lanes)
{
    if (lanes > ARGON2_MAX_LANES) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER,
                       "max lanes: %u", ARGON2_MAX_LANES);
        return 0;
    }

    if (lanes < ARGON2_MIN_LANES) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER,
                       "min lanes: %u", ARGON2_MIN_LANES);
        return 0;
    }

    ctx->lanes = lanes;
    return 1;
}

static int kdf_argon2_ctx_set_t_cost(KDF_ARGON2 *ctx, uint32_t t_cost)
{
    /* ARGON2_MAX_MEMORY == max m_cost value, so skip check  */

    if (t_cost < ARGON2_MIN_TIME) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_ITERATION_COUNT,
                       "min: %u", ARGON2_MIN_TIME);
        return 0;
    }

    ctx->t_cost = t_cost;
    return 1;
}

static int kdf_argon2_ctx_set_m_cost(KDF_ARGON2 *ctx, uint32_t m_cost)
{
    /* ARGON2_MAX_MEMORY == max m_cost value, so skip check */

    if (m_cost < ARGON2_MIN_MEMORY) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_MEMORY_SIZE, "min: %u",
                       ARGON2_MIN_MEMORY);
        return 0;
    }

    ctx->m_cost = m_cost;
    return 1;
}

static int kdf_argon2_ctx_set_out_length(KDF_ARGON2 *ctx, uint32_t outlen)
{
    /*
     * ARGON2_MAX_OUT_LENGTH == max outlen value, so upper bounds checks
     * are always satisfied; to suppress compiler if statement tautology
     * warnings, these checks are skipped.
     */

    if (outlen < ARGON2_MIN_OUT_LENGTH) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_OUTPUT_LENGTH, "min: %u",
                       ARGON2_MIN_OUT_LENGTH);
        return 0;
    }

    ctx->outlen = outlen;
    return 1;
}

static int kdf_argon2_ctx_set_secret(KDF_ARGON2 *ctx, const OSSL_PARAM *p)
{
    size_t buflen;

    if (p->data == NULL)
        return 0;

    if (ctx->secret != NULL) {
        OPENSSL_clear_free(ctx->secret, ctx->secretlen);
        ctx->secret = NULL;
        ctx->secretlen = 0U;
    }

    if (!OSSL_PARAM_get_octet_string(p, (void **)&ctx->secret, 0, &buflen))
        return 0;

    if (buflen > ARGON2_MAX_SECRET) {
        OPENSSL_free(ctx->secret);
        ctx->secret = NULL;
        ctx->secretlen = 0U;
        return 0;
    }

    ctx->secretlen = (uint32_t) buflen;
    return 1;
}

static int kdf_argon2_ctx_set_pwd(KDF_ARGON2 *ctx, const OSSL_PARAM *p)
{
    size_t buflen;

    if (p->data == NULL)
        return 0;

    if (ctx->pwd != NULL) {
        OPENSSL_clear_free(ctx->pwd, ctx->pwdlen);
        ctx->pwd = NULL;
        ctx->pwdlen = 0U;
    }

    if (!OSSL_PARAM_get_octet_string(p, (void **)&ctx->pwd, 0, &buflen))
        return 0;

    if (buflen > ARGON2_MAX_PWD_LENGTH) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_SALT_LENGTH, "max: %u",
                       ARGON2_MAX_PWD_LENGTH);
        goto fail;
    }

    ctx->pwdlen = (uint32_t) buflen;
    return 1;

fail:
    OPENSSL_free(ctx->pwd);
    ctx->pwd = NULL;
    ctx->pwdlen = 0U;
    return 0;
}

static int kdf_argon2_ctx_set_salt(KDF_ARGON2 *ctx, const OSSL_PARAM *p)
{
    size_t buflen;

    if (p->data == NULL)
        return 0;

    if (ctx->salt != NULL) {
        OPENSSL_clear_free(ctx->salt, ctx->saltlen);
        ctx->salt = NULL;
        ctx->saltlen = 0U;
    }

    if (!OSSL_PARAM_get_octet_string(p, (void **)&ctx->salt, 0, &buflen))
        return 0;

    if (buflen < ARGON2_MIN_SALT_LENGTH) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_SALT_LENGTH, "min: %u",
                       ARGON2_MIN_SALT_LENGTH);
        goto fail;
    }

    if (buflen > ARGON2_MAX_SALT_LENGTH) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_SALT_LENGTH, "max: %u",
                       ARGON2_MAX_SALT_LENGTH);
        goto fail;
    }

    ctx->saltlen = (uint32_t) buflen;
    return 1;

fail:
    OPENSSL_free(ctx->salt);
    ctx->salt = NULL;
    ctx->saltlen = 0U;
    return 0;
}

static int kdf_argon2_ctx_set_ad(KDF_ARGON2 *ctx, const OSSL_PARAM *p)
{
    size_t buflen;

    if (p->data == NULL)
        return 0;

    if (ctx->ad != NULL) {
        OPENSSL_clear_free(ctx->ad, ctx->adlen);
        ctx->ad = NULL;
        ctx->adlen = 0U;
    }

    if (!OSSL_PARAM_get_octet_string(p, (void **)&ctx->ad, 0, &buflen))
        return 0;

    if (buflen > ARGON2_MAX_AD_LENGTH) {
        OPENSSL_free(ctx->ad);
        ctx->ad = NULL;
        ctx->adlen = 0U;
        return 0;
    }

    ctx->adlen = (uint32_t) buflen;
    return 1;
}

static void kdf_argon2_ctx_set_flag_early_clean(KDF_ARGON2 *ctx, uint32_t f)
{
    ctx->early_clean = !!(f);
}

static int kdf_argon2_ctx_set_version(KDF_ARGON2 *ctx, uint32_t version)
{
    switch (version) {
    case ARGON2_VERSION_10:
    case ARGON2_VERSION_13:
        ctx->version = version;
        return 1;
    default:
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_MODE,
                       "invalid Argon2 version");
        return 0;
    }
}

static int set_property_query(KDF_ARGON2 *ctx, const char *propq)
{
    OPENSSL_free(ctx->propq);
    ctx->propq = NULL;
    if (propq != NULL) {
        ctx->propq = OPENSSL_strdup(propq);
        if (ctx->propq == NULL)
            return 0;
    }
    EVP_MD_free(ctx->md);
    ctx->md = NULL;
    EVP_MAC_free(ctx->mac);
    ctx->mac = NULL;
    return 1;
}

static int kdf_argon2_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    const OSSL_PARAM *p;
    KDF_ARGON2 *ctx;
    uint32_t u32_value;

    if (ossl_param_is_empty(params))
        return 1;

    ctx = (KDF_ARGON2 *) vctx;
    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_PASSWORD)) != NULL)
        if (!kdf_argon2_ctx_set_pwd(ctx, p))
            return 0;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_SALT)) != NULL)
        if (!kdf_argon2_ctx_set_salt(ctx, p))
            return 0;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_SECRET)) != NULL)
        if (!kdf_argon2_ctx_set_secret(ctx, p))
            return 0;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_ARGON2_AD)) != NULL)
        if (!kdf_argon2_ctx_set_ad(ctx, p))
            return 0;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_SIZE)) != NULL) {
        if (!OSSL_PARAM_get_uint32(p, &u32_value))
            return 0;
        if (!kdf_argon2_ctx_set_out_length(ctx, u32_value))
            return 0;
    }

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_ITER)) != NULL) {
        if (!OSSL_PARAM_get_uint32(p, &u32_value))
            return 0;
        if (!kdf_argon2_ctx_set_t_cost(ctx, u32_value))
            return 0;
    }

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_THREADS)) != NULL) {
        if (!OSSL_PARAM_get_uint32(p, &u32_value))
            return 0;
        if (!kdf_argon2_ctx_set_threads(ctx, u32_value))
            return 0;
    }

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_ARGON2_LANES)) != NULL) {
        if (!OSSL_PARAM_get_uint32(p, &u32_value))
            return 0;
        if (!kdf_argon2_ctx_set_lanes(ctx, u32_value))
            return 0;
    }

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_ARGON2_MEMCOST)) != NULL) {
        if (!OSSL_PARAM_get_uint32(p, &u32_value))
            return 0;
        if (!kdf_argon2_ctx_set_m_cost(ctx, u32_value))
            return 0;
    }

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_EARLY_CLEAN)) != NULL) {
        if (!OSSL_PARAM_get_uint32(p, &u32_value))
            return 0;
        kdf_argon2_ctx_set_flag_early_clean(ctx, u32_value);
    }

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_ARGON2_VERSION)) != NULL) {
        if (!OSSL_PARAM_get_uint32(p, &u32_value))
            return 0;
        if (!kdf_argon2_ctx_set_version(ctx, u32_value))
            return 0;
    }

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_PROPERTIES)) != NULL) {
        if (p->data_type != OSSL_PARAM_UTF8_STRING
            || !set_property_query(ctx, p->data))
            return 0;
    }

    return 1;
}

static const OSSL_PARAM *kdf_argon2_settable_ctx_params(ossl_unused void *ctx,
                                                        ossl_unused void *p_ctx)
{
    static const OSSL_PARAM known_settable_ctx_params[] = {
        OSSL_PARAM_octet_string(OSSL_KDF_PARAM_PASSWORD, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_KDF_PARAM_SALT, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_KDF_PARAM_SECRET, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_KDF_PARAM_ARGON2_AD, NULL, 0),
        OSSL_PARAM_uint32(OSSL_KDF_PARAM_SIZE, NULL),
        OSSL_PARAM_uint32(OSSL_KDF_PARAM_ITER, NULL),
        OSSL_PARAM_uint32(OSSL_KDF_PARAM_THREADS, NULL),
        OSSL_PARAM_uint32(OSSL_KDF_PARAM_ARGON2_LANES, NULL),
        OSSL_PARAM_uint32(OSSL_KDF_PARAM_ARGON2_MEMCOST, NULL),
        OSSL_PARAM_uint32(OSSL_KDF_PARAM_EARLY_CLEAN, NULL),
        OSSL_PARAM_uint32(OSSL_KDF_PARAM_ARGON2_VERSION, NULL),
        OSSL_PARAM_utf8_string(OSSL_KDF_PARAM_PROPERTIES, NULL, 0),
        OSSL_PARAM_END
    };

    return known_settable_ctx_params;
}

static int kdf_argon2_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    OSSL_PARAM *p;

    (void) vctx;
    if ((p = OSSL_PARAM_locate(params, OSSL_KDF_PARAM_SIZE)) != NULL)
        return OSSL_PARAM_set_size_t(p, SIZE_MAX);

    return -2;
}

static const OSSL_PARAM *kdf_argon2_gettable_ctx_params(ossl_unused void *ctx,
                                                        ossl_unused void *p_ctx)
{
    static const OSSL_PARAM known_gettable_ctx_params[] = {
        OSSL_PARAM_size_t(OSSL_KDF_PARAM_SIZE, NULL),
        OSSL_PARAM_END
    };

    return known_gettable_ctx_params;
}

const OSSL_DISPATCH ossl_kdf_argon2i_functions[] = {
    { OSSL_FUNC_KDF_NEWCTX, (void(*)(void))kdf_argon2i_new },
    { OSSL_FUNC_KDF_FREECTX, (void(*)(void))kdf_argon2_free },
    { OSSL_FUNC_KDF_RESET, (void(*)(void))kdf_argon2_reset },
    { OSSL_FUNC_KDF_DERIVE, (void(*)(void))kdf_argon2_derive },
    { OSSL_FUNC_KDF_SETTABLE_CTX_PARAMS,
      (void(*)(void))kdf_argon2_settable_ctx_params },
    { OSSL_FUNC_KDF_SET_CTX_PARAMS, (void(*)(void))kdf_argon2_set_ctx_params },
    { OSSL_FUNC_KDF_GETTABLE_CTX_PARAMS,
      (void(*)(void))kdf_argon2_gettable_ctx_params },
    { OSSL_FUNC_KDF_GET_CTX_PARAMS, (void(*)(void))kdf_argon2_get_ctx_params },
    OSSL_DISPATCH_END
};

const OSSL_DISPATCH ossl_kdf_argon2d_functions[] = {
    { OSSL_FUNC_KDF_NEWCTX, (void(*)(void))kdf_argon2d_new },
    { OSSL_FUNC_KDF_FREECTX, (void(*)(void))kdf_argon2_free },
    { OSSL_FUNC_KDF_RESET, (void(*)(void))kdf_argon2_reset },
    { OSSL_FUNC_KDF_DERIVE, (void(*)(void))kdf_argon2_derive },
    { OSSL_FUNC_KDF_SETTABLE_CTX_PARAMS,
      (void(*)(void))kdf_argon2_settable_ctx_params },
    { OSSL_FUNC_KDF_SET_CTX_PARAMS, (void(*)(void))kdf_argon2_set_ctx_params },
    { OSSL_FUNC_KDF_GETTABLE_CTX_PARAMS,
      (void(*)(void))kdf_argon2_gettable_ctx_params },
    { OSSL_FUNC_KDF_GET_CTX_PARAMS, (void(*)(void))kdf_argon2_get_ctx_params },
    OSSL_DISPATCH_END
};

const OSSL_DISPATCH ossl_kdf_argon2id_functions[] = {
    { OSSL_FUNC_KDF_NEWCTX, (void(*)(void))kdf_argon2id_new },
    { OSSL_FUNC_KDF_FREECTX, (void(*)(void))kdf_argon2_free },
    { OSSL_FUNC_KDF_RESET, (void(*)(void))kdf_argon2_reset },
    { OSSL_FUNC_KDF_DERIVE, (void(*)(void))kdf_argon2_derive },
    { OSSL_FUNC_KDF_SETTABLE_CTX_PARAMS,
      (void(*)(void))kdf_argon2_settable_ctx_params },
    { OSSL_FUNC_KDF_SET_CTX_PARAMS, (void(*)(void))kdf_argon2_set_ctx_params },
    { OSSL_FUNC_KDF_GETTABLE_CTX_PARAMS,
      (void(*)(void))kdf_argon2_gettable_ctx_params },
    { OSSL_FUNC_KDF_GET_CTX_PARAMS, (void(*)(void))kdf_argon2_get_ctx_params },
    OSSL_DISPATCH_END
};

#endif
