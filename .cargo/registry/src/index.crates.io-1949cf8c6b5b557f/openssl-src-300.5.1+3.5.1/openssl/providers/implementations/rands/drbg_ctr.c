/*
 * Copyright 2011-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdlib.h>
#include <string.h>
#include <openssl/crypto.h>
#include <openssl/err.h>
#include <openssl/rand.h>
#include <openssl/aes.h>
#include <openssl/proverr.h>
#include "crypto/modes.h"
#include "internal/thread_once.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"
#include "prov/provider_ctx.h"
#include "drbg_local.h"
#include "crypto/evp.h"
#include "crypto/evp/evp_local.h"
#include "internal/provider.h"
#include "internal/common.h"

static OSSL_FUNC_rand_newctx_fn drbg_ctr_new_wrapper;
static OSSL_FUNC_rand_freectx_fn drbg_ctr_free;
static OSSL_FUNC_rand_instantiate_fn drbg_ctr_instantiate_wrapper;
static OSSL_FUNC_rand_uninstantiate_fn drbg_ctr_uninstantiate_wrapper;
static OSSL_FUNC_rand_generate_fn drbg_ctr_generate_wrapper;
static OSSL_FUNC_rand_reseed_fn drbg_ctr_reseed_wrapper;
static OSSL_FUNC_rand_settable_ctx_params_fn drbg_ctr_settable_ctx_params;
static OSSL_FUNC_rand_set_ctx_params_fn drbg_ctr_set_ctx_params;
static OSSL_FUNC_rand_gettable_ctx_params_fn drbg_ctr_gettable_ctx_params;
static OSSL_FUNC_rand_get_ctx_params_fn drbg_ctr_get_ctx_params;
static OSSL_FUNC_rand_verify_zeroization_fn drbg_ctr_verify_zeroization;

static int drbg_ctr_set_ctx_params_locked(void *vctx, const OSSL_PARAM params[]);

/*
 * The state of a DRBG AES-CTR.
 */
typedef struct rand_drbg_ctr_st {
    EVP_CIPHER_CTX *ctx_ecb;
    EVP_CIPHER_CTX *ctx_ctr;
    EVP_CIPHER_CTX *ctx_df;
    EVP_CIPHER *cipher_ecb;
    EVP_CIPHER *cipher_ctr;
    size_t keylen;
    int use_df;
    unsigned char K[32];
    unsigned char V[16];
    /* Temporary block storage used by ctr_df */
    unsigned char bltmp[16];
    size_t bltmp_pos;
    unsigned char KX[48];
} PROV_DRBG_CTR;

/*
 * Implementation of NIST SP 800-90A CTR DRBG.
 */
static void inc_128(PROV_DRBG_CTR *ctr)
{
    unsigned char *p = &ctr->V[0];
    u32 n = 16, c = 1;

    do {
        --n;
        c += p[n];
        p[n] = (u8)c;
        c >>= 8;
    } while (n);
}

static void ctr_XOR(PROV_DRBG_CTR *ctr, const unsigned char *in, size_t inlen)
{
    size_t i, n;

    if (in == NULL || inlen == 0)
        return;

    /*
     * Any zero padding will have no effect on the result as we
     * are XORing. So just process however much input we have.
     */
    n = inlen < ctr->keylen ? inlen : ctr->keylen;
    if (!ossl_assert(n <= sizeof(ctr->K)))
        return;
    for (i = 0; i < n; i++)
        ctr->K[i] ^= in[i];
    if (inlen <= ctr->keylen)
        return;

    n = inlen - ctr->keylen;
    if (n > 16) {
        /* Should never happen */
        n = 16;
    }
    for (i = 0; i < n; i++)
        ctr->V[i] ^= in[i + ctr->keylen];
}

/*
 * Process a complete block using BCC algorithm of SP 800-90A 10.3.3
 */
__owur static int ctr_BCC_block(PROV_DRBG_CTR *ctr, unsigned char *out,
                                const unsigned char *in, int len)
{
    int i, outlen = AES_BLOCK_SIZE;

    for (i = 0; i < len; i++)
        out[i] ^= in[i];

    if (!EVP_CipherUpdate(ctr->ctx_df, out, &outlen, out, len)
        || outlen != len)
        return 0;
    return 1;
}


/*
 * Handle several BCC operations for as much data as we need for K and X
 */
__owur static int ctr_BCC_blocks(PROV_DRBG_CTR *ctr, const unsigned char *in)
{
    unsigned char in_tmp[48];
    unsigned char num_of_blk = 2;

    memcpy(in_tmp, in, 16);
    memcpy(in_tmp + 16, in, 16);
    if (ctr->keylen != 16) {
        memcpy(in_tmp + 32, in, 16);
        num_of_blk = 3;
    }
    return ctr_BCC_block(ctr, ctr->KX, in_tmp, AES_BLOCK_SIZE * num_of_blk);
}

/*
 * Initialise BCC blocks: these have the value 0,1,2 in leftmost positions:
 * see 10.3.1 stage 7.
 */
__owur static int ctr_BCC_init(PROV_DRBG_CTR *ctr)
{
    unsigned char bltmp[48] = {0};
    unsigned char num_of_blk;

    memset(ctr->KX, 0, 48);
    num_of_blk = ctr->keylen == 16 ? 2 : 3;
    bltmp[(AES_BLOCK_SIZE * 1) + 3] = 1;
    bltmp[(AES_BLOCK_SIZE * 2) + 3] = 2;
    return ctr_BCC_block(ctr, ctr->KX, bltmp, num_of_blk * AES_BLOCK_SIZE);
}

/*
 * Process several blocks into BCC algorithm, some possibly partial
 */
__owur static int ctr_BCC_update(PROV_DRBG_CTR *ctr,
                                 const unsigned char *in, size_t inlen)
{
    if (in == NULL || inlen == 0)
        return 1;

    /* If we have partial block handle it first */
    if (ctr->bltmp_pos) {
        size_t left = 16 - ctr->bltmp_pos;

        /* If we now have a complete block process it */
        if (inlen >= left) {
            memcpy(ctr->bltmp + ctr->bltmp_pos, in, left);
            if (!ctr_BCC_blocks(ctr, ctr->bltmp))
                return 0;
            ctr->bltmp_pos = 0;
            inlen -= left;
            in += left;
        }
    }

    /* Process zero or more complete blocks */
    for (; inlen >= 16; in += 16, inlen -= 16) {
        if (!ctr_BCC_blocks(ctr, in))
            return 0;
    }

    /* Copy any remaining partial block to the temporary buffer */
    if (inlen > 0) {
        memcpy(ctr->bltmp + ctr->bltmp_pos, in, inlen);
        ctr->bltmp_pos += inlen;
    }
    return 1;
}

__owur static int ctr_BCC_final(PROV_DRBG_CTR *ctr)
{
    if (ctr->bltmp_pos) {
        memset(ctr->bltmp + ctr->bltmp_pos, 0, 16 - ctr->bltmp_pos);
        if (!ctr_BCC_blocks(ctr, ctr->bltmp))
            return 0;
    }
    return 1;
}

__owur static int ctr_df(PROV_DRBG_CTR *ctr,
                         const unsigned char *in1, size_t in1len,
                         const unsigned char *in2, size_t in2len,
                         const unsigned char *in3, size_t in3len)
{
    static unsigned char c80 = 0x80;
    size_t inlen;
    unsigned char *p = ctr->bltmp;
    int outlen = AES_BLOCK_SIZE;

    if (!ctr_BCC_init(ctr))
        return 0;
    if (in1 == NULL)
        in1len = 0;
    if (in2 == NULL)
        in2len = 0;
    if (in3 == NULL)
        in3len = 0;
    inlen = in1len + in2len + in3len;
    /* Initialise L||N in temporary block */
    *p++ = (inlen >> 24) & 0xff;
    *p++ = (inlen >> 16) & 0xff;
    *p++ = (inlen >> 8) & 0xff;
    *p++ = inlen & 0xff;

    /* NB keylen is at most 32 bytes */
    *p++ = 0;
    *p++ = 0;
    *p++ = 0;
    *p = (unsigned char)((ctr->keylen + 16) & 0xff);
    ctr->bltmp_pos = 8;
    if (!ctr_BCC_update(ctr, in1, in1len)
        || !ctr_BCC_update(ctr, in2, in2len)
        || !ctr_BCC_update(ctr, in3, in3len)
        || !ctr_BCC_update(ctr, &c80, 1)
        || !ctr_BCC_final(ctr))
        return 0;
    /* Set up key K */
    if (!EVP_CipherInit_ex(ctr->ctx_ecb, NULL, NULL, ctr->KX, NULL, -1))
        return 0;
    /* X follows key K */
    if (!EVP_CipherUpdate(ctr->ctx_ecb, ctr->KX, &outlen, ctr->KX + ctr->keylen,
                          AES_BLOCK_SIZE)
        || outlen != AES_BLOCK_SIZE)
        return 0;
    if (!EVP_CipherUpdate(ctr->ctx_ecb, ctr->KX + 16, &outlen, ctr->KX,
                          AES_BLOCK_SIZE)
        || outlen != AES_BLOCK_SIZE)
        return 0;
    if (ctr->keylen != 16)
        if (!EVP_CipherUpdate(ctr->ctx_ecb, ctr->KX + 32, &outlen,
                              ctr->KX + 16, AES_BLOCK_SIZE)
            || outlen != AES_BLOCK_SIZE)
            return 0;
    return 1;
}

/*
 * NB the no-df Update in SP800-90A specifies a constant input length
 * of seedlen, however other uses of this algorithm pad the input with
 * zeroes if necessary and have up to two parameters XORed together,
 * so we handle both cases in this function instead.
 */
__owur static int ctr_update(PROV_DRBG *drbg,
                             const unsigned char *in1, size_t in1len,
                             const unsigned char *in2, size_t in2len,
                             const unsigned char *nonce, size_t noncelen)
{
    PROV_DRBG_CTR *ctr = (PROV_DRBG_CTR *)drbg->data;
    int outlen = AES_BLOCK_SIZE;
    unsigned char V_tmp[48], out[48];
    unsigned char len;

    /* correct key is already set up. */
    memcpy(V_tmp, ctr->V, 16);
    inc_128(ctr);
    memcpy(V_tmp + 16, ctr->V, 16);
    if (ctr->keylen == 16) {
        len = 32;
    } else {
        inc_128(ctr);
        memcpy(V_tmp + 32, ctr->V, 16);
        len = 48;
    }
    if (!EVP_CipherUpdate(ctr->ctx_ecb, out, &outlen, V_tmp, len)
            || outlen != len)
        return 0;
    memcpy(ctr->K, out, ctr->keylen);
    memcpy(ctr->V, out + ctr->keylen, 16);

    if (ctr->use_df) {
        /* If no input reuse existing derived value */
        if (in1 != NULL || nonce != NULL || in2 != NULL)
            if (!ctr_df(ctr, in1, in1len, nonce, noncelen, in2, in2len))
                return 0;
        /* If this a reuse input in1len != 0 */
        if (in1len)
            ctr_XOR(ctr, ctr->KX, drbg->seedlen);
    } else {
        ctr_XOR(ctr, in1, in1len);
        ctr_XOR(ctr, in2, in2len);
    }

    if (!EVP_CipherInit_ex(ctr->ctx_ecb, NULL, NULL, ctr->K, NULL, -1)
        || !EVP_CipherInit_ex(ctr->ctx_ctr, NULL, NULL, ctr->K, NULL, -1))
        return 0;
    return 1;
}

static int drbg_ctr_instantiate(PROV_DRBG *drbg,
                                const unsigned char *entropy, size_t entropylen,
                                const unsigned char *nonce, size_t noncelen,
                                const unsigned char *pers, size_t perslen)
{
    PROV_DRBG_CTR *ctr = (PROV_DRBG_CTR *)drbg->data;

    if (entropy == NULL)
        return 0;

    memset(ctr->K, 0, sizeof(ctr->K));
    memset(ctr->V, 0, sizeof(ctr->V));
    if (!EVP_CipherInit_ex(ctr->ctx_ecb, NULL, NULL, ctr->K, NULL, -1))
        return 0;

    inc_128(ctr);
    if (!ctr_update(drbg, entropy, entropylen, pers, perslen, nonce, noncelen))
        return 0;
    return 1;
}

static int drbg_ctr_instantiate_wrapper(void *vdrbg, unsigned int strength,
                                        int prediction_resistance,
                                        const unsigned char *pstr,
                                        size_t pstr_len,
                                        const OSSL_PARAM params[])
{
    PROV_DRBG *drbg = (PROV_DRBG *)vdrbg;
    int ret = 0;

    if (drbg->lock != NULL && !CRYPTO_THREAD_write_lock(drbg->lock))
        return 0;

    if (!ossl_prov_is_running()
            || !drbg_ctr_set_ctx_params_locked(drbg, params))
        goto err;
    ret = ossl_prov_drbg_instantiate(drbg, strength, prediction_resistance,
                                     pstr, pstr_len);
 err:
    if (drbg->lock != NULL)
        CRYPTO_THREAD_unlock(drbg->lock);
    return ret;
}

static int drbg_ctr_reseed(PROV_DRBG *drbg,
                           const unsigned char *entropy, size_t entropylen,
                           const unsigned char *adin, size_t adinlen)
{
    PROV_DRBG_CTR *ctr = (PROV_DRBG_CTR *)drbg->data;

    if (entropy == NULL)
        return 0;

    inc_128(ctr);
    if (!ctr_update(drbg, entropy, entropylen, adin, adinlen, NULL, 0))
        return 0;
    return 1;
}

static int drbg_ctr_reseed_wrapper(void *vdrbg, int prediction_resistance,
                                   const unsigned char *ent, size_t ent_len,
                                   const unsigned char *adin, size_t adin_len)
{
    PROV_DRBG *drbg = (PROV_DRBG *)vdrbg;

    return ossl_prov_drbg_reseed(drbg, prediction_resistance, ent, ent_len,
                                 adin, adin_len);
}

static void ctr96_inc(unsigned char *counter)
{
    u32 n = 12, c = 1;

    do {
        --n;
        c += counter[n];
        counter[n] = (u8)c;
        c >>= 8;
    } while (n);
}

static int drbg_ctr_generate(PROV_DRBG *drbg,
                             unsigned char *out, size_t outlen,
                             const unsigned char *adin, size_t adinlen)
{
    PROV_DRBG_CTR *ctr = (PROV_DRBG_CTR *)drbg->data;
    unsigned int ctr32, blocks;
    int outl, buflen;

    if (adin != NULL && adinlen != 0) {
        inc_128(ctr);

        if (!ctr_update(drbg, adin, adinlen, NULL, 0, NULL, 0))
            return 0;
        /* This means we reuse derived value */
        if (ctr->use_df) {
            adin = NULL;
            adinlen = 1;
        }
    } else {
        adinlen = 0;
    }

    inc_128(ctr);

    if (outlen == 0) {
        inc_128(ctr);

        if (!ctr_update(drbg, adin, adinlen, NULL, 0, NULL, 0))
            return 0;
        return 1;
    }

    memset(out, 0, outlen);

    do {
        if (!EVP_CipherInit_ex(ctr->ctx_ctr,
                               NULL, NULL, NULL, ctr->V, -1))
            return 0;

        /*-
         * outlen has type size_t while EVP_CipherUpdate takes an
         * int argument and thus cannot be guaranteed to process more
         * than 2^31-1 bytes at a time. We process such huge generate
         * requests in 2^30 byte chunks, which is the greatest multiple
         * of AES block size lower than or equal to 2^31-1.
         */
        buflen = outlen > (1U << 30) ? (1U << 30) : outlen;
        blocks = (buflen + 15) / 16;

        ctr32 = GETU32(ctr->V + 12) + blocks;
        if (ctr32 < blocks) {
            /* 32-bit counter overflow into V. */
            if (ctr32 != 0) {
                blocks -= ctr32;
                buflen = blocks * 16;
                ctr32 = 0;
            }
            ctr96_inc(ctr->V);
        }
        PUTU32(ctr->V + 12, ctr32);

        if (!EVP_CipherUpdate(ctr->ctx_ctr, out, &outl, out, buflen)
            || outl != buflen)
            return 0;

        out += buflen;
        outlen -= buflen;
    } while (outlen);

    if (!ctr_update(drbg, adin, adinlen, NULL, 0, NULL, 0))
        return 0;
    return 1;
}

static int drbg_ctr_generate_wrapper
    (void *vdrbg, unsigned char *out, size_t outlen,
     unsigned int strength, int prediction_resistance,
     const unsigned char *adin, size_t adin_len)
{
    PROV_DRBG *drbg = (PROV_DRBG *)vdrbg;

    return ossl_prov_drbg_generate(drbg, out, outlen, strength,
                                   prediction_resistance, adin, adin_len);
}

static int drbg_ctr_uninstantiate(PROV_DRBG *drbg)
{
    PROV_DRBG_CTR *ctr = (PROV_DRBG_CTR *)drbg->data;

    OPENSSL_cleanse(ctr->K, sizeof(ctr->K));
    OPENSSL_cleanse(ctr->V, sizeof(ctr->V));
    OPENSSL_cleanse(ctr->bltmp, sizeof(ctr->bltmp));
    OPENSSL_cleanse(ctr->KX, sizeof(ctr->KX));
    ctr->bltmp_pos = 0;
    return ossl_prov_drbg_uninstantiate(drbg);
}

static int drbg_ctr_uninstantiate_wrapper(void *vdrbg)
{
    PROV_DRBG *drbg = (PROV_DRBG *)vdrbg;
    int ret;

    if (drbg->lock != NULL && !CRYPTO_THREAD_write_lock(drbg->lock))
        return 0;

    ret = drbg_ctr_uninstantiate(drbg);

    if (drbg->lock != NULL)
        CRYPTO_THREAD_unlock(drbg->lock);

    return ret;
}

static int drbg_ctr_verify_zeroization(void *vdrbg)
{
    PROV_DRBG *drbg = (PROV_DRBG *)vdrbg;
    PROV_DRBG_CTR *ctr = (PROV_DRBG_CTR *)drbg->data;
    int ret = 0;

    if (drbg->lock != NULL && !CRYPTO_THREAD_read_lock(drbg->lock))
        return 0;

    PROV_DRBG_VERIFY_ZEROIZATION(ctr->K);
    PROV_DRBG_VERIFY_ZEROIZATION(ctr->V);
    PROV_DRBG_VERIFY_ZEROIZATION(ctr->bltmp);
    PROV_DRBG_VERIFY_ZEROIZATION(ctr->KX);
    if (ctr->bltmp_pos != 0)
        goto err;

    ret = 1;
 err:
    if (drbg->lock != NULL)
        CRYPTO_THREAD_unlock(drbg->lock);
    return ret;
}

static int drbg_ctr_init_lengths(PROV_DRBG *drbg)
{
    PROV_DRBG_CTR *ctr = (PROV_DRBG_CTR *)drbg->data;
    int res = 1;

    /* Maximum number of bits per request = 2^19  = 2^16 bytes */
    drbg->max_request = 1 << 16;
    if (ctr->use_df) {
        drbg->min_entropylen = 0;
        drbg->max_entropylen = DRBG_MAX_LENGTH;
        drbg->min_noncelen = 0;
        drbg->max_noncelen = DRBG_MAX_LENGTH;
        drbg->max_perslen = DRBG_MAX_LENGTH;
        drbg->max_adinlen = DRBG_MAX_LENGTH;

        if (ctr->keylen > 0) {
            drbg->min_entropylen = ctr->keylen;
            drbg->min_noncelen = drbg->min_entropylen / 2;
        }
    } else {
        const size_t len = ctr->keylen > 0 ? drbg->seedlen : DRBG_MAX_LENGTH;

        drbg->min_entropylen = len;
        drbg->max_entropylen = len;
        /* Nonce not used */
        drbg->min_noncelen = 0;
        drbg->max_noncelen = 0;
        drbg->max_perslen = len;
        drbg->max_adinlen = len;
    }
    return res;
}

static int drbg_ctr_init(PROV_DRBG *drbg)
{
    PROV_DRBG_CTR *ctr = (PROV_DRBG_CTR *)drbg->data;
    size_t keylen;

    if (ctr->cipher_ctr == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_CIPHER);
        return 0;
    }
    ctr->keylen = keylen = EVP_CIPHER_get_key_length(ctr->cipher_ctr);
    if (ctr->ctx_ecb == NULL)
        ctr->ctx_ecb = EVP_CIPHER_CTX_new();
    if (ctr->ctx_ctr == NULL)
        ctr->ctx_ctr = EVP_CIPHER_CTX_new();
    if (ctr->ctx_ecb == NULL || ctr->ctx_ctr == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_EVP_LIB);
        goto err;
    }

    if (!EVP_CipherInit_ex(ctr->ctx_ecb,
                           ctr->cipher_ecb, NULL, NULL, NULL, 1)
        || !EVP_CipherInit_ex(ctr->ctx_ctr,
                              ctr->cipher_ctr, NULL, NULL, NULL, 1)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_UNABLE_TO_INITIALISE_CIPHERS);
        goto err;
    }

    drbg->strength = keylen * 8;
    drbg->seedlen = keylen + 16;

    if (ctr->use_df) {
        /* df initialisation */
        static const unsigned char df_key[32] = {
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
            0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
            0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17,
            0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f
        };

        if (ctr->ctx_df == NULL)
            ctr->ctx_df = EVP_CIPHER_CTX_new();
        if (ctr->ctx_df == NULL) {
            ERR_raise(ERR_LIB_PROV, ERR_R_EVP_LIB);
            goto err;
        }
        /* Set key schedule for df_key */
        if (!EVP_CipherInit_ex(ctr->ctx_df,
                               ctr->cipher_ecb, NULL, df_key, NULL, 1)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_DERIVATION_FUNCTION_INIT_FAILED);
            goto err;
        }
    }
    return drbg_ctr_init_lengths(drbg);

err:
    EVP_CIPHER_CTX_free(ctr->ctx_ecb);
    EVP_CIPHER_CTX_free(ctr->ctx_ctr);
    ctr->ctx_ecb = ctr->ctx_ctr = NULL;
    return 0;
}

static int drbg_ctr_new(PROV_DRBG *drbg)
{
    PROV_DRBG_CTR *ctr;

    ctr = OPENSSL_secure_zalloc(sizeof(*ctr));
    if (ctr == NULL)
        return 0;

    ctr->use_df = 1;
    drbg->data = ctr;
    OSSL_FIPS_IND_INIT(drbg)
    return drbg_ctr_init_lengths(drbg);
}

static void *drbg_ctr_new_wrapper(void *provctx, void *parent,
                                   const OSSL_DISPATCH *parent_dispatch)
{
    return ossl_rand_drbg_new(provctx, parent, parent_dispatch,
                              &drbg_ctr_new, &drbg_ctr_free,
                              &drbg_ctr_instantiate, &drbg_ctr_uninstantiate,
                              &drbg_ctr_reseed, &drbg_ctr_generate);
}

static void drbg_ctr_free(void *vdrbg)
{
    PROV_DRBG *drbg = (PROV_DRBG *)vdrbg;
    PROV_DRBG_CTR *ctr;

    if (drbg != NULL && (ctr = (PROV_DRBG_CTR *)drbg->data) != NULL) {
        EVP_CIPHER_CTX_free(ctr->ctx_ecb);
        EVP_CIPHER_CTX_free(ctr->ctx_ctr);
        EVP_CIPHER_CTX_free(ctr->ctx_df);
        EVP_CIPHER_free(ctr->cipher_ecb);
        EVP_CIPHER_free(ctr->cipher_ctr);

        OPENSSL_secure_clear_free(ctr, sizeof(*ctr));
    }
    ossl_rand_drbg_free(drbg);
}

static int drbg_ctr_get_ctx_params(void *vdrbg, OSSL_PARAM params[])
{
    PROV_DRBG *drbg = (PROV_DRBG *)vdrbg;
    PROV_DRBG_CTR *ctr = (PROV_DRBG_CTR *)drbg->data;
    OSSL_PARAM *p;
    int ret = 0, complete = 0;

    if (!ossl_drbg_get_ctx_params_no_lock(drbg, params, &complete))
        return 0;

    if (complete)
        return 1;

    if (drbg->lock != NULL && !CRYPTO_THREAD_read_lock(drbg->lock))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_DRBG_PARAM_USE_DF);
    if (p != NULL && !OSSL_PARAM_set_int(p, ctr->use_df))
        goto err;

    p = OSSL_PARAM_locate(params, OSSL_DRBG_PARAM_CIPHER);
    if (p != NULL) {
        if (ctr->cipher_ctr == NULL
            || !OSSL_PARAM_set_utf8_string(p,
                                           EVP_CIPHER_get0_name(ctr->cipher_ctr)))
            goto err;
    }

    ret = ossl_drbg_get_ctx_params(drbg, params);
 err:
    if (drbg->lock != NULL)
        CRYPTO_THREAD_unlock(drbg->lock);

    return ret;
}

static const OSSL_PARAM *drbg_ctr_gettable_ctx_params(ossl_unused void *vctx,
                                                      ossl_unused void *provctx)
{
    static const OSSL_PARAM known_gettable_ctx_params[] = {
        OSSL_PARAM_utf8_string(OSSL_DRBG_PARAM_CIPHER, NULL, 0),
        OSSL_PARAM_int(OSSL_DRBG_PARAM_USE_DF, NULL),
        OSSL_PARAM_DRBG_GETTABLE_CTX_COMMON,
        OSSL_FIPS_IND_GETTABLE_CTX_PARAM()
        OSSL_PARAM_END
    };
    return known_gettable_ctx_params;
}

static int drbg_ctr_set_ctx_params_locked(void *vctx, const OSSL_PARAM params[])
{
    PROV_DRBG *ctx = (PROV_DRBG *)vctx;
    PROV_DRBG_CTR *ctr = (PROV_DRBG_CTR *)ctx->data;
    OSSL_LIB_CTX *libctx = PROV_LIBCTX_OF(ctx->provctx);
    OSSL_PROVIDER *prov = NULL;
    const OSSL_PARAM *p;
    char *ecb;
    const char *propquery = NULL;
    int i, cipher_init = 0;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_DRBG_PARAM_USE_DF)) != NULL
            && OSSL_PARAM_get_int(p, &i)) {
        /* FIPS errors out in the drbg_ctr_init() call later */
        ctr->use_df = i != 0;
        cipher_init = 1;
    }

    if ((p = OSSL_PARAM_locate_const(params,
                                     OSSL_DRBG_PARAM_PROPERTIES)) != NULL) {
        if (p->data_type != OSSL_PARAM_UTF8_STRING)
            return 0;
        propquery = (const char *)p->data;
    }

    if ((p = OSSL_PARAM_locate_const(params,
                                     OSSL_PROV_PARAM_CORE_PROV_NAME)) != NULL) {
        if (p->data_type != OSSL_PARAM_UTF8_STRING)
            return 0;
        if ((prov = ossl_provider_find(libctx,
                                       (const char *)p->data, 1)) == NULL)
            return 0;
    }

    if ((p = OSSL_PARAM_locate_const(params, OSSL_DRBG_PARAM_CIPHER)) != NULL) {
        const char *base = (const char *)p->data;
        size_t ctr_str_len = sizeof("CTR") - 1;
        size_t ecb_str_len = sizeof("ECB") - 1;

        if (p->data_type != OSSL_PARAM_UTF8_STRING
                || p->data_size < ctr_str_len) {
            ossl_provider_free(prov);
            return 0;
        }
        if (OPENSSL_strcasecmp("CTR", base + p->data_size - ctr_str_len) != 0) {
            ERR_raise(ERR_LIB_PROV, PROV_R_REQUIRE_CTR_MODE_CIPHER);
            ossl_provider_free(prov);
            return 0;
        }
        if ((ecb = OPENSSL_strndup(base, p->data_size)) == NULL) {
            ossl_provider_free(prov);
            return 0;
        }
        strcpy(ecb + p->data_size - ecb_str_len, "ECB");
        EVP_CIPHER_free(ctr->cipher_ecb);
        EVP_CIPHER_free(ctr->cipher_ctr);
        /*
         * Try to fetch algorithms from our own provider code, fallback
         * to generic fetch only if that fails
         */
        (void)ERR_set_mark();
        ctr->cipher_ctr = evp_cipher_fetch_from_prov(prov, base, NULL);
        if (ctr->cipher_ctr == NULL) {
            (void)ERR_pop_to_mark();
            ctr->cipher_ctr = EVP_CIPHER_fetch(libctx, base, propquery);
        } else {
            (void)ERR_clear_last_mark();
        }
        (void)ERR_set_mark();
        ctr->cipher_ecb = evp_cipher_fetch_from_prov(prov, ecb, NULL);
        if (ctr->cipher_ecb == NULL) {
            (void)ERR_pop_to_mark();
            ctr->cipher_ecb = EVP_CIPHER_fetch(libctx, ecb, propquery);
        } else {
            (void)ERR_clear_last_mark();
        }
        OPENSSL_free(ecb);
        if (ctr->cipher_ctr == NULL || ctr->cipher_ecb == NULL) {
            ERR_raise(ERR_LIB_PROV, PROV_R_UNABLE_TO_FIND_CIPHERS);
            ossl_provider_free(prov);
            return 0;
        }
        cipher_init = 1;
    }
    ossl_provider_free(prov);

    if (cipher_init && !drbg_ctr_init(ctx))
        return 0;

    return ossl_drbg_set_ctx_params(ctx, params);
}

static int drbg_ctr_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    PROV_DRBG *drbg = (PROV_DRBG *)vctx;
    int ret;

    if (drbg->lock != NULL && !CRYPTO_THREAD_write_lock(drbg->lock))
        return 0;

    ret = drbg_ctr_set_ctx_params_locked(vctx, params);

    if (drbg->lock != NULL)
        CRYPTO_THREAD_unlock(drbg->lock);

    return ret;
}

static const OSSL_PARAM *drbg_ctr_settable_ctx_params(ossl_unused void *vctx,
                                                      ossl_unused void *provctx)
{
    static const OSSL_PARAM known_settable_ctx_params[] = {
        OSSL_PARAM_utf8_string(OSSL_DRBG_PARAM_PROPERTIES, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_DRBG_PARAM_CIPHER, NULL, 0),
        OSSL_PARAM_int(OSSL_DRBG_PARAM_USE_DF, NULL),
        OSSL_PARAM_DRBG_SETTABLE_CTX_COMMON,
        OSSL_PARAM_END
    };
    return known_settable_ctx_params;
}

const OSSL_DISPATCH ossl_drbg_ctr_functions[] = {
    { OSSL_FUNC_RAND_NEWCTX, (void(*)(void))drbg_ctr_new_wrapper },
    { OSSL_FUNC_RAND_FREECTX, (void(*)(void))drbg_ctr_free },
    { OSSL_FUNC_RAND_INSTANTIATE,
      (void(*)(void))drbg_ctr_instantiate_wrapper },
    { OSSL_FUNC_RAND_UNINSTANTIATE,
      (void(*)(void))drbg_ctr_uninstantiate_wrapper },
    { OSSL_FUNC_RAND_GENERATE, (void(*)(void))drbg_ctr_generate_wrapper },
    { OSSL_FUNC_RAND_RESEED, (void(*)(void))drbg_ctr_reseed_wrapper },
    { OSSL_FUNC_RAND_ENABLE_LOCKING, (void(*)(void))ossl_drbg_enable_locking },
    { OSSL_FUNC_RAND_LOCK, (void(*)(void))ossl_drbg_lock },
    { OSSL_FUNC_RAND_UNLOCK, (void(*)(void))ossl_drbg_unlock },
    { OSSL_FUNC_RAND_SETTABLE_CTX_PARAMS,
      (void(*)(void))drbg_ctr_settable_ctx_params },
    { OSSL_FUNC_RAND_SET_CTX_PARAMS, (void(*)(void))drbg_ctr_set_ctx_params },
    { OSSL_FUNC_RAND_GETTABLE_CTX_PARAMS,
      (void(*)(void))drbg_ctr_gettable_ctx_params },
    { OSSL_FUNC_RAND_GET_CTX_PARAMS, (void(*)(void))drbg_ctr_get_ctx_params },
    { OSSL_FUNC_RAND_VERIFY_ZEROIZATION,
      (void(*)(void))drbg_ctr_verify_zeroization },
    { OSSL_FUNC_RAND_GET_SEED, (void(*)(void))ossl_drbg_get_seed },
    { OSSL_FUNC_RAND_CLEAR_SEED, (void(*)(void))ossl_drbg_clear_seed },
    OSSL_DISPATCH_END
};
