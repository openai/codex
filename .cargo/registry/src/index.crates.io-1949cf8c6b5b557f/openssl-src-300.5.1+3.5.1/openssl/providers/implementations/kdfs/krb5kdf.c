/*
 * Copyright 2018-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * DES low level APIs are deprecated for public use, but still ok for internal
 * use.  We access the DES_set_odd_parity(3) function here.
 */
#include "internal/deprecated.h"

#include <stdlib.h>
#include <stdarg.h>
#include <string.h>

#include <openssl/core_names.h>
#include <openssl/des.h>
#include <openssl/evp.h>
#include <openssl/kdf.h>
#include <openssl/proverr.h>

#include "internal/cryptlib.h"
#include "crypto/evp.h"
#include "internal/numbers.h"
#include "prov/implementations.h"
#include "prov/provider_ctx.h"
#include "prov/provider_util.h"
#include "prov/providercommon.h"

/* KRB5 KDF defined in RFC 3961, Section 5.1 */

static OSSL_FUNC_kdf_newctx_fn krb5kdf_new;
static OSSL_FUNC_kdf_dupctx_fn krb5kdf_dup;
static OSSL_FUNC_kdf_freectx_fn krb5kdf_free;
static OSSL_FUNC_kdf_reset_fn krb5kdf_reset;
static OSSL_FUNC_kdf_derive_fn krb5kdf_derive;
static OSSL_FUNC_kdf_settable_ctx_params_fn krb5kdf_settable_ctx_params;
static OSSL_FUNC_kdf_set_ctx_params_fn krb5kdf_set_ctx_params;
static OSSL_FUNC_kdf_gettable_ctx_params_fn krb5kdf_gettable_ctx_params;
static OSSL_FUNC_kdf_get_ctx_params_fn krb5kdf_get_ctx_params;

static int KRB5KDF(const EVP_CIPHER *cipher, ENGINE *engine,
                   const unsigned char *key, size_t key_len,
                   const unsigned char *constant, size_t constant_len,
                   unsigned char *okey, size_t okey_len);

typedef struct {
    void *provctx;
    PROV_CIPHER cipher;
    unsigned char *key;
    size_t key_len;
    unsigned char *constant;
    size_t constant_len;
} KRB5KDF_CTX;

static void *krb5kdf_new(void *provctx)
{
    KRB5KDF_CTX *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    if ((ctx = OPENSSL_zalloc(sizeof(*ctx))) == NULL)
        return NULL;
    ctx->provctx = provctx;
    return ctx;
}

static void krb5kdf_free(void *vctx)
{
    KRB5KDF_CTX *ctx = (KRB5KDF_CTX *)vctx;

    if (ctx != NULL) {
        krb5kdf_reset(ctx);
        OPENSSL_free(ctx);
    }
}

static void krb5kdf_reset(void *vctx)
{
    KRB5KDF_CTX *ctx = (KRB5KDF_CTX *)vctx;
    void *provctx = ctx->provctx;

    ossl_prov_cipher_reset(&ctx->cipher);
    OPENSSL_clear_free(ctx->key, ctx->key_len);
    OPENSSL_clear_free(ctx->constant, ctx->constant_len);
    memset(ctx, 0, sizeof(*ctx));
    ctx->provctx = provctx;
}

static int krb5kdf_set_membuf(unsigned char **dst, size_t *dst_len,
                              const OSSL_PARAM *p)
{
    OPENSSL_clear_free(*dst, *dst_len);
    *dst = NULL;
    *dst_len = 0;
    return OSSL_PARAM_get_octet_string(p, (void **)dst, 0, dst_len);
}

static void *krb5kdf_dup(void *vctx)
{
    const KRB5KDF_CTX *src = (const KRB5KDF_CTX *)vctx;
    KRB5KDF_CTX *dest;

    dest = krb5kdf_new(src->provctx);
    if (dest != NULL) {
        if (!ossl_prov_memdup(src->key, src->key_len,
                              &dest->key, &dest->key_len)
                || !ossl_prov_memdup(src->constant, src->constant_len,
                                     &dest->constant , &dest->constant_len)
                || !ossl_prov_cipher_copy(&dest->cipher, &src->cipher))
            goto err;
    }
    return dest;

 err:
    krb5kdf_free(dest);
    return NULL;
}

static int krb5kdf_derive(void *vctx, unsigned char *key, size_t keylen,
                          const OSSL_PARAM params[])
{
    KRB5KDF_CTX *ctx = (KRB5KDF_CTX *)vctx;
    const EVP_CIPHER *cipher;
    ENGINE *engine;

    if (!ossl_prov_is_running() || !krb5kdf_set_ctx_params(ctx, params))
        return 0;

    cipher = ossl_prov_cipher_cipher(&ctx->cipher);
    if (cipher == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_CIPHER);
        return 0;
    }
    if (ctx->key == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_KEY);
        return 0;
    }
    if (ctx->constant == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_CONSTANT);
        return 0;
    }
    engine = ossl_prov_cipher_engine(&ctx->cipher);
    return KRB5KDF(cipher, engine, ctx->key, ctx->key_len,
                   ctx->constant, ctx->constant_len,
                   key, keylen);
}

static int krb5kdf_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    const OSSL_PARAM *p;
    KRB5KDF_CTX *ctx = vctx;
    OSSL_LIB_CTX *provctx = PROV_LIBCTX_OF(ctx->provctx);

    if (ossl_param_is_empty(params))
        return 1;

    if (!ossl_prov_cipher_load_from_params(&ctx->cipher, params, provctx))
        return 0;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_KEY)) != NULL)
        if (!krb5kdf_set_membuf(&ctx->key, &ctx->key_len, p))
            return 0;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_CONSTANT))
        != NULL)
        if (!krb5kdf_set_membuf(&ctx->constant, &ctx->constant_len, p))
            return 0;

    return 1;
}

static const OSSL_PARAM *krb5kdf_settable_ctx_params(ossl_unused void *ctx,
                                                     ossl_unused void *provctx)
{
    static const OSSL_PARAM known_settable_ctx_params[] = {
        OSSL_PARAM_utf8_string(OSSL_KDF_PARAM_PROPERTIES, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_KDF_PARAM_CIPHER, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_KDF_PARAM_KEY, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_KDF_PARAM_CONSTANT, NULL, 0),
        OSSL_PARAM_END
    };
    return known_settable_ctx_params;
}

static int krb5kdf_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    KRB5KDF_CTX *ctx = (KRB5KDF_CTX *)vctx;
    const EVP_CIPHER *cipher;
    size_t len;
    OSSL_PARAM *p;

    cipher = ossl_prov_cipher_cipher(&ctx->cipher);
    if (cipher)
        len = EVP_CIPHER_get_key_length(cipher);
    else
        len = EVP_MAX_KEY_LENGTH;

    if ((p = OSSL_PARAM_locate(params, OSSL_KDF_PARAM_SIZE)) != NULL)
        return OSSL_PARAM_set_size_t(p, len);
    return -2;
}

static const OSSL_PARAM *krb5kdf_gettable_ctx_params(ossl_unused void *ctx,
                                                     ossl_unused void *provctx)
{
    static const OSSL_PARAM known_gettable_ctx_params[] = {
        OSSL_PARAM_size_t(OSSL_KDF_PARAM_SIZE, NULL),
        OSSL_PARAM_END
    };
    return known_gettable_ctx_params;
}

const OSSL_DISPATCH ossl_kdf_krb5kdf_functions[] = {
    { OSSL_FUNC_KDF_NEWCTX, (void(*)(void))krb5kdf_new },
    { OSSL_FUNC_KDF_DUPCTX, (void(*)(void))krb5kdf_dup },
    { OSSL_FUNC_KDF_FREECTX, (void(*)(void))krb5kdf_free },
    { OSSL_FUNC_KDF_RESET, (void(*)(void))krb5kdf_reset },
    { OSSL_FUNC_KDF_DERIVE, (void(*)(void))krb5kdf_derive },
    { OSSL_FUNC_KDF_SETTABLE_CTX_PARAMS,
      (void(*)(void))krb5kdf_settable_ctx_params },
    { OSSL_FUNC_KDF_SET_CTX_PARAMS,
      (void(*)(void))krb5kdf_set_ctx_params },
    { OSSL_FUNC_KDF_GETTABLE_CTX_PARAMS,
      (void(*)(void))krb5kdf_gettable_ctx_params },
    { OSSL_FUNC_KDF_GET_CTX_PARAMS,
      (void(*)(void))krb5kdf_get_ctx_params },
    OSSL_DISPATCH_END
};

#ifndef OPENSSL_NO_DES
/*
 * DES3 is a special case, it requires a random-to-key function and its
 * input truncated to 21 bytes of the 24 produced by the cipher.
 * See RFC3961 6.3.1
 */
static int fixup_des3_key(unsigned char *key)
{
    unsigned char *cblock;
    int i, j;

    for (i = 2; i >= 0; i--) {
        cblock = &key[i * 8];
        memmove(cblock, &key[i * 7], 7);
        cblock[7] = 0;
        for (j = 0; j < 7; j++)
            cblock[7] |= (cblock[j] & 1) << (j + 1);
        DES_set_odd_parity((DES_cblock *)cblock);
    }

    /* fail if keys are such that triple des degrades to single des */
    if (CRYPTO_memcmp(&key[0], &key[8], 8) == 0 ||
        CRYPTO_memcmp(&key[8], &key[16], 8) == 0) {
        return 0;
    }

    return 1;
}
#endif

/*
 * N-fold(K) where blocksize is N, and constant_len is K
 * Note: Here |= denotes concatenation
 *
 * L = lcm(N,K)
 * R = L/K
 *
 * for r: 1 -> R
 *   s |= constant rot 13*(r-1))
 *
 * block = 0
 * for k: 1 -> K
 *   block += s[N(k-1)..(N-1)k] (ones'-complement addition)
 *
 * Optimizing for space we compute:
 * for each l in L-1 -> 0:
 *   s[l] = (constant rot 13*(l/K))[l%k]
 *   block[l % N] += s[l] (with carry)
 * finally add carry if any
 */
static void n_fold(unsigned char *block, unsigned int blocksize,
                   const unsigned char *constant, size_t constant_len)
{
    unsigned int tmp, gcd, remainder, lcm, carry;
    int b, l;

    if (constant_len == blocksize) {
        memcpy(block, constant, constant_len);
        return;
    }

    /* Least Common Multiple of lengths: LCM(a,b)*/
    gcd = blocksize;
    remainder = constant_len;
    /* Calculate Great Common Divisor first GCD(a,b) */
    while (remainder != 0) {
        tmp = gcd % remainder;
        gcd = remainder;
        remainder = tmp;
    }
    /* resulting a is the GCD, LCM(a,b) = |a*b|/GCD(a,b) */
    lcm = blocksize * constant_len / gcd;

    /* now spread out the bits */
    memset(block, 0, blocksize);

    /* last to first to be able to bring carry forward */
    carry = 0;
    for (l = lcm - 1; l >= 0; l--) {
        unsigned int rotbits, rshift, rbyte;

        /* destination byte in block is l % N */
        b = l % blocksize;
        /* Our virtual s buffer is R = L/K long (K = constant_len) */
        /* So we rotate backwards from R-1 to 0 (none) rotations */
        rotbits = 13 * (l / constant_len);
        /* find the byte on s where rotbits falls onto */
        rbyte = l - (rotbits / 8);
        /* calculate how much shift on that byte */
        rshift = rotbits & 0x07;
        /* rbyte % constant_len gives us the unrotated byte in the
         * constant buffer, get also the previous byte then
         * appropriately shift them to get the rotated byte we need */
        tmp = (constant[(rbyte-1) % constant_len] << (8 - rshift)
               | constant[rbyte % constant_len] >> rshift)
              & 0xff;
        /* add with carry to any value placed by previous passes */
        tmp += carry + block[b];
        block[b] = tmp & 0xff;
        /* save any carry that may be left */
        carry = tmp >> 8;
    }

    /* if any carry is left at the end, add it through the number */
    for (b = blocksize - 1; b >= 0 && carry != 0; b--) {
        carry += block[b];
        block[b] = carry & 0xff;
        carry >>= 8;
    }
}

static int cipher_init(EVP_CIPHER_CTX *ctx,
                       const EVP_CIPHER *cipher, ENGINE *engine,
                       const unsigned char *key, size_t key_len)
{
    int klen, ret;

    ret = EVP_EncryptInit_ex(ctx, cipher, engine, key, NULL);
    if (!ret)
        goto out;
    /* set the key len for the odd variable key len cipher */
    klen = EVP_CIPHER_CTX_get_key_length(ctx);
    if (key_len != (size_t)klen) {
        ret = EVP_CIPHER_CTX_set_key_length(ctx, key_len);
        if (ret <= 0) {
            ret = 0;
            goto out;
        }
    }
    /* we never want padding, either the length requested is a multiple of
     * the cipher block size or we are passed a cipher that can cope with
     * partial blocks via techniques like cipher text stealing */
    ret = EVP_CIPHER_CTX_set_padding(ctx, 0);
    if (!ret)
        goto out;

out:
    return ret;
}

static int KRB5KDF(const EVP_CIPHER *cipher, ENGINE *engine,
                   const unsigned char *key, size_t key_len,
                   const unsigned char *constant, size_t constant_len,
                   unsigned char *okey, size_t okey_len)
{
    EVP_CIPHER_CTX *ctx = NULL;
    unsigned char block[EVP_MAX_BLOCK_LENGTH * 2];
    unsigned char *plainblock, *cipherblock;
    size_t blocksize;
    size_t cipherlen;
    size_t osize;
#ifndef OPENSSL_NO_DES
    int des3_no_fixup = 0;
#endif
    int ret;

    if (key_len != okey_len) {
#ifndef OPENSSL_NO_DES
        /* special case for 3des, where the caller may be requesting
         * the random raw key, instead of the fixed up key  */
        if (EVP_CIPHER_get_nid(cipher) == NID_des_ede3_cbc &&
            key_len == 24 && okey_len == 21) {
                des3_no_fixup = 1;
        } else {
#endif
            ERR_raise(ERR_LIB_PROV, PROV_R_WRONG_OUTPUT_BUFFER_SIZE);
            return 0;
#ifndef OPENSSL_NO_DES
        }
#endif
    }

    ctx = EVP_CIPHER_CTX_new();
    if (ctx == NULL)
        return 0;

    ret = cipher_init(ctx, cipher, engine, key, key_len);
    if (!ret)
        goto out;

    /* Initialize input block */
    blocksize = EVP_CIPHER_CTX_get_block_size(ctx);

    if (blocksize == 0) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_CIPHER);
        ret = 0;
        goto out;
    }

    if (constant_len > blocksize) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_CONSTANT_LENGTH);
        ret = 0;
        goto out;
    }

    n_fold(block, blocksize, constant, constant_len);
    plainblock = block;
    cipherblock = block + EVP_MAX_BLOCK_LENGTH;

    for (osize = 0; osize < okey_len; osize += cipherlen) {
        int olen;

        ret = EVP_EncryptUpdate(ctx, cipherblock, &olen,
                                plainblock, blocksize);
        if (!ret)
            goto out;
        cipherlen = olen;
        ret = EVP_EncryptFinal_ex(ctx, cipherblock, &olen);
        if (!ret)
            goto out;
        if (olen != 0) {
            ERR_raise(ERR_LIB_PROV, PROV_R_WRONG_FINAL_BLOCK_LENGTH);
            ret = 0;
            goto out;
        }

        /* write cipherblock out */
        if (cipherlen > okey_len - osize)
            cipherlen = okey_len - osize;
        memcpy(okey + osize, cipherblock, cipherlen);

        if (okey_len > osize + cipherlen) {
            /* we need to reinitialize cipher context per spec */
            ret = EVP_CIPHER_CTX_reset(ctx);
            if (!ret)
                goto out;
            ret = cipher_init(ctx, cipher, engine, key, key_len);
            if (!ret)
                goto out;

            /* also swap block offsets so last ciphertext becomes new
             * plaintext */
            plainblock = cipherblock;
            if (cipherblock == block) {
                cipherblock += EVP_MAX_BLOCK_LENGTH;
            } else {
                cipherblock = block;
            }
        }
    }

#ifndef OPENSSL_NO_DES
    if (EVP_CIPHER_get_nid(cipher) == NID_des_ede3_cbc && !des3_no_fixup) {
        ret = fixup_des3_key(okey);
        if (!ret) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GENERATE_KEY);
            goto out;
        }
    }
#endif

    ret = 1;

out:
    EVP_CIPHER_CTX_free(ctx);
    OPENSSL_cleanse(block, EVP_MAX_BLOCK_LENGTH * 2);
    return ret;
}

