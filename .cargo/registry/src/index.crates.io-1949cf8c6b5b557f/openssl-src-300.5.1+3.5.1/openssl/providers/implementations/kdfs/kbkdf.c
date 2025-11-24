/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright 2019 Red Hat, Inc.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * This implements https://csrc.nist.gov/publications/detail/sp/800-108/final
 * section 5.1 ("counter mode") and section 5.2 ("feedback mode") in both HMAC
 * and CMAC.  That document does not name the KDFs it defines; the name is
 * derived from
 * https://csrc.nist.gov/Projects/Cryptographic-Algorithm-Validation-Program/Key-Derivation
 *
 * Note that section 5.3 ("double-pipeline mode") is not implemented, though
 * it would be possible to do so in the future.
 *
 * These versions all assume the counter is used.  It would be relatively
 * straightforward to expose a configuration handle should the need arise.
 *
 * Variable names attempt to match those of SP800-108.
 */

#include <stdarg.h>
#include <stdlib.h>
#include <string.h>

#include <openssl/core_names.h>
#include <openssl/evp.h>
#include <openssl/hmac.h>
#include <openssl/kdf.h>
#include <openssl/params.h>
#include <openssl/proverr.h>

#include "internal/cryptlib.h"
#include "crypto/evp.h"
#include "internal/numbers.h"
#include "internal/endian.h"
#include "prov/implementations.h"
#include "prov/provider_ctx.h"
#include "prov/provider_util.h"
#include "prov/providercommon.h"
#include "prov/securitycheck.h"
#include "internal/e_os.h"
#include "internal/params.h"

#define ossl_min(a, b) ((a) < (b)) ? (a) : (b)

typedef enum {
    COUNTER = 0,
    FEEDBACK
} kbkdf_mode;

/* Our context structure. */
typedef struct {
    void *provctx;
    kbkdf_mode mode;
    EVP_MAC_CTX *ctx_init;

    /* Names are lowercased versions of those found in SP800-108. */
    int r;
    unsigned char *ki;
    size_t ki_len;
    unsigned char *label;
    size_t label_len;
    unsigned char *context;
    size_t context_len;
    unsigned char *iv;
    size_t iv_len;
    int use_l;
    int is_kmac;
    int use_separator;
    OSSL_FIPS_IND_DECLARE
} KBKDF;

/* Definitions needed for typechecking. */
static OSSL_FUNC_kdf_newctx_fn kbkdf_new;
static OSSL_FUNC_kdf_dupctx_fn kbkdf_dup;
static OSSL_FUNC_kdf_freectx_fn kbkdf_free;
static OSSL_FUNC_kdf_reset_fn kbkdf_reset;
static OSSL_FUNC_kdf_derive_fn kbkdf_derive;
static OSSL_FUNC_kdf_settable_ctx_params_fn kbkdf_settable_ctx_params;
static OSSL_FUNC_kdf_set_ctx_params_fn kbkdf_set_ctx_params;
static OSSL_FUNC_kdf_gettable_ctx_params_fn kbkdf_gettable_ctx_params;
static OSSL_FUNC_kdf_get_ctx_params_fn kbkdf_get_ctx_params;

/* Not all platforms have htobe32(). */
static uint32_t be32(uint32_t host)
{
    uint32_t big = 0;
    DECLARE_IS_ENDIAN;

    if (!IS_LITTLE_ENDIAN)
        return host;

    big |= (host & 0xff000000) >> 24;
    big |= (host & 0x00ff0000) >> 8;
    big |= (host & 0x0000ff00) << 8;
    big |= (host & 0x000000ff) << 24;
    return big;
}

static void init(KBKDF *ctx)
{
    ctx->r = 32;
    ctx->use_l = 1;
    ctx->use_separator = 1;
    ctx->is_kmac = 0;
}

static void *kbkdf_new(void *provctx)
{
    KBKDF *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx == NULL)
        return NULL;

    ctx->provctx = provctx;
    OSSL_FIPS_IND_INIT(ctx)
    init(ctx);
    return ctx;
}

static void kbkdf_free(void *vctx)
{
    KBKDF *ctx = (KBKDF *)vctx;

    if (ctx != NULL) {
        kbkdf_reset(ctx);
        OPENSSL_free(ctx);
    }
}

static void kbkdf_reset(void *vctx)
{
    KBKDF *ctx = (KBKDF *)vctx;
    void *provctx = ctx->provctx;

    EVP_MAC_CTX_free(ctx->ctx_init);
    OPENSSL_clear_free(ctx->context, ctx->context_len);
    OPENSSL_clear_free(ctx->label, ctx->label_len);
    OPENSSL_clear_free(ctx->ki, ctx->ki_len);
    OPENSSL_clear_free(ctx->iv, ctx->iv_len);
    memset(ctx, 0, sizeof(*ctx));
    ctx->provctx = provctx;
    init(ctx);
}

static void *kbkdf_dup(void *vctx)
{
    const KBKDF *src = (const KBKDF *)vctx;
    KBKDF *dest;

    dest = kbkdf_new(src->provctx);
    if (dest != NULL) {
        dest->ctx_init = EVP_MAC_CTX_dup(src->ctx_init);
        if (dest->ctx_init == NULL
                || !ossl_prov_memdup(src->ki, src->ki_len,
                                     &dest->ki, &dest->ki_len)
                || !ossl_prov_memdup(src->label, src->label_len,
                                     &dest->label, &dest->label_len)
                || !ossl_prov_memdup(src->context, src->context_len,
                                     &dest->context, &dest->context_len)
                || !ossl_prov_memdup(src->iv, src->iv_len,
                                     &dest->iv, &dest->iv_len))
            goto err;
        dest->mode = src->mode;
        dest->r = src->r;
        dest->use_l = src->use_l;
        dest->use_separator = src->use_separator;
        dest->is_kmac = src->is_kmac;
        OSSL_FIPS_IND_COPY(dest, src)
    }
    return dest;

 err:
    kbkdf_free(dest);
    return NULL;
}

#ifdef FIPS_MODULE
static int fips_kbkdf_key_check_passed(KBKDF *ctx)
{
    OSSL_LIB_CTX *libctx = PROV_LIBCTX_OF(ctx->provctx);
    int key_approved = ossl_kdf_check_key_size(ctx->ki_len);

    if (!key_approved) {
        if (!OSSL_FIPS_IND_ON_UNAPPROVED(ctx, OSSL_FIPS_IND_SETTABLE0,
                                         libctx, "KBKDF", "Key size",
                                         ossl_fips_config_kbkdf_key_check)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
            return 0;
        }
    }
    return 1;
}
#endif

/* SP800-108 section 5.1 or section 5.2 depending on mode. */
static int derive(EVP_MAC_CTX *ctx_init, kbkdf_mode mode, unsigned char *iv,
                  size_t iv_len, unsigned char *label, size_t label_len,
                  unsigned char *context, size_t context_len,
                  unsigned char *k_i, size_t h, uint32_t l, int has_separator,
                  unsigned char *ko, size_t ko_len, int r)
{
    int ret = 0;
    EVP_MAC_CTX *ctx = NULL;
    size_t written = 0, to_write, k_i_len = iv_len;
    const unsigned char zero = 0;
    uint32_t counter, i;
    /*
     * From SP800-108:
     * The fixed input data is a concatenation of a Label,
     * a separation indicator 0x00, the Context, and L.
     * One or more of these fixed input data fields may be omitted.
     *
     * has_separator == 0 means that the separator is omitted.
     * Passing a value of l == 0 means that L is omitted.
     * The Context and L are omitted automatically if a NULL buffer is passed.
     */
    int has_l = (l != 0);

    /* Setup K(0) for feedback mode. */
    if (iv_len > 0)
        memcpy(k_i, iv, iv_len);

    for (counter = 1; written < ko_len; counter++) {
        i = be32(counter);

        ctx = EVP_MAC_CTX_dup(ctx_init);
        if (ctx == NULL)
            goto done;

        /* Perform feedback, if appropriate. */
        if (mode == FEEDBACK && !EVP_MAC_update(ctx, k_i, k_i_len))
            goto done;

        if (!EVP_MAC_update(ctx, 4 - (r / 8) + (unsigned char *)&i, r / 8)
            || !EVP_MAC_update(ctx, label, label_len)
            || (has_separator && !EVP_MAC_update(ctx, &zero, 1))
            || !EVP_MAC_update(ctx, context, context_len)
            || (has_l && !EVP_MAC_update(ctx, (unsigned char *)&l, 4))
            || !EVP_MAC_final(ctx, k_i, NULL, h))
            goto done;

        to_write = ko_len - written;
        memcpy(ko + written, k_i, ossl_min(to_write, h));
        written += h;

        k_i_len = h;
        EVP_MAC_CTX_free(ctx);
        ctx = NULL;
    }

    ret = 1;
done:
    EVP_MAC_CTX_free(ctx);
    return ret;
}

/* This must be run before the key is set */
static int kmac_init(EVP_MAC_CTX *ctx, const unsigned char *custom, size_t customlen)
{
    OSSL_PARAM params[2];

    if (custom == NULL || customlen == 0)
        return 1;
    params[0] = OSSL_PARAM_construct_octet_string(OSSL_MAC_PARAM_CUSTOM,
                                                  (void *)custom, customlen);
    params[1] = OSSL_PARAM_construct_end();
    return EVP_MAC_CTX_set_params(ctx, params) > 0;
}

static int kmac_derive(EVP_MAC_CTX *ctx, unsigned char *out, size_t outlen,
                       const unsigned char *context, size_t contextlen)
{
    OSSL_PARAM params[2];

    params[0] = OSSL_PARAM_construct_size_t(OSSL_MAC_PARAM_SIZE, &outlen);
    params[1] = OSSL_PARAM_construct_end();
    return EVP_MAC_CTX_set_params(ctx, params) > 0
           && EVP_MAC_update(ctx, context, contextlen)
           && EVP_MAC_final(ctx, out, NULL, outlen);
}

static int kbkdf_derive(void *vctx, unsigned char *key, size_t keylen,
                        const OSSL_PARAM params[])
{
    KBKDF *ctx = (KBKDF *)vctx;
    int ret = 0;
    unsigned char *k_i = NULL;
    uint32_t l = 0;
    size_t h = 0;
    uint64_t counter_max;

    if (!ossl_prov_is_running() || !kbkdf_set_ctx_params(ctx, params))
        return 0;

    /* label, context, and iv are permitted to be empty.  Check everything
     * else. */
    if (ctx->ctx_init == NULL) {
        if (ctx->ki_len == 0 || ctx->ki == NULL) {
            ERR_raise(ERR_LIB_PROV, PROV_R_NO_KEY_SET);
            return 0;
        }
        /* Could either be missing MAC or missing message digest or missing
         * cipher - arbitrarily, I pick this one. */
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_MAC);
        return 0;
    }

    /* Fail if the output length is zero */
    if (keylen == 0) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
        return 0;
    }

    if (ctx->is_kmac) {
        ret = kmac_derive(ctx->ctx_init, key, keylen,
                          ctx->context, ctx->context_len);
        goto done;
    }

    h = EVP_MAC_CTX_get_mac_size(ctx->ctx_init);
    if (h == 0)
        goto done;

    if (ctx->iv_len != 0 && ctx->iv_len != h) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_SEED_LENGTH);
        goto done;
    }

    if (ctx->mode == COUNTER) {
        /* Fail if keylen is too large for r */
        counter_max = (uint64_t)1 << (uint64_t)ctx->r;
        if ((uint64_t)(keylen / h) >= counter_max) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
            goto done;
        }
    }

    if (ctx->use_l != 0)
        l = be32(keylen * 8);

    k_i = OPENSSL_zalloc(h);
    if (k_i == NULL)
        goto done;

    ret = derive(ctx->ctx_init, ctx->mode, ctx->iv, ctx->iv_len, ctx->label,
                 ctx->label_len, ctx->context, ctx->context_len, k_i, h, l,
                 ctx->use_separator, key, keylen, ctx->r);
done:
    if (ret != 1)
        OPENSSL_cleanse(key, keylen);
    OPENSSL_clear_free(k_i, h);
    return ret;
}

static int kbkdf_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    KBKDF *ctx = (KBKDF *)vctx;
    OSSL_LIB_CTX *libctx = PROV_LIBCTX_OF(ctx->provctx);
    const OSSL_PARAM *p;

    if (ossl_param_is_empty(params))
        return 1;

    if (!OSSL_FIPS_IND_SET_CTX_PARAM(ctx, OSSL_FIPS_IND_SETTABLE0, params,
                                     OSSL_KDF_PARAM_FIPS_KEY_CHECK))
        return 0;

    if (!ossl_prov_macctx_load_from_params(&ctx->ctx_init, params, NULL,
                                           NULL, NULL, libctx))
        return 0;
    if (ctx->ctx_init != NULL) {
        ctx->is_kmac = 0;
        if (EVP_MAC_is_a(EVP_MAC_CTX_get0_mac(ctx->ctx_init),
                         OSSL_MAC_NAME_KMAC128)
            || EVP_MAC_is_a(EVP_MAC_CTX_get0_mac(ctx->ctx_init),
                            OSSL_MAC_NAME_KMAC256)) {
            ctx->is_kmac = 1;
        } else if (!EVP_MAC_is_a(EVP_MAC_CTX_get0_mac(ctx->ctx_init),
                                 OSSL_MAC_NAME_HMAC)
                   && !EVP_MAC_is_a(EVP_MAC_CTX_get0_mac(ctx->ctx_init),
                                    OSSL_MAC_NAME_CMAC)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_MAC);
            return 0;
        }
    }

    p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_MODE);
    if (p != NULL
        && OPENSSL_strncasecmp("counter", p->data, p->data_size) == 0) {
        ctx->mode = COUNTER;
    } else if (p != NULL
               && OPENSSL_strncasecmp("feedback", p->data, p->data_size) == 0) {
        ctx->mode = FEEDBACK;
    } else if (p != NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_MODE);
        return 0;
    }

    p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_KEY);
    if (p != NULL) {
        if (ossl_param_get1_octet_string(p, OSSL_KDF_PARAM_KEY,
                                         &ctx->ki, &ctx->ki_len) == 0)
            return 0;
#ifdef FIPS_MODULE
        if (!fips_kbkdf_key_check_passed(ctx))
            return 0;
#endif
    }

    if (ossl_param_get1_octet_string(params, OSSL_KDF_PARAM_SALT,
                                     &ctx->label, &ctx->label_len) == 0)
            return 0;

    if (ossl_param_get1_concat_octet_string(params, OSSL_KDF_PARAM_INFO,
                                            &ctx->context, &ctx->context_len,
                                            0) == 0)
        return 0;

    if (ossl_param_get1_octet_string(params, OSSL_KDF_PARAM_SEED,
                                     &ctx->iv, &ctx->iv_len) == 0)
            return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_KBKDF_USE_L);
    if (p != NULL && !OSSL_PARAM_get_int(p, &ctx->use_l))
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_KBKDF_R);
    if (p != NULL) {
        int new_r = 0;

        if (!OSSL_PARAM_get_int(p, &new_r))
            return 0;
        if (new_r != 8 && new_r != 16 && new_r != 24 && new_r != 32)
            return 0;
        ctx->r = new_r;
    }

    p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_KBKDF_USE_SEPARATOR);
    if (p != NULL && !OSSL_PARAM_get_int(p, &ctx->use_separator))
        return 0;

    /* Set up digest context, if we can. */
    if (ctx->ctx_init != NULL && ctx->ki_len != 0) {
        if ((ctx->is_kmac && !kmac_init(ctx->ctx_init, ctx->label, ctx->label_len))
            || !EVP_MAC_init(ctx->ctx_init, ctx->ki, ctx->ki_len, NULL))
            return 0;
    }
    return 1;
}

static const OSSL_PARAM *kbkdf_settable_ctx_params(ossl_unused void *ctx,
                                                   ossl_unused void *provctx)
{
    static const OSSL_PARAM known_settable_ctx_params[] = {
        OSSL_PARAM_octet_string(OSSL_KDF_PARAM_INFO, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_KDF_PARAM_SALT, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_KDF_PARAM_KEY, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_KDF_PARAM_SEED, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_KDF_PARAM_DIGEST, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_KDF_PARAM_CIPHER, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_KDF_PARAM_MAC, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_KDF_PARAM_MODE, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_KDF_PARAM_PROPERTIES, NULL, 0),
        OSSL_PARAM_int(OSSL_KDF_PARAM_KBKDF_USE_L, NULL),
        OSSL_PARAM_int(OSSL_KDF_PARAM_KBKDF_USE_SEPARATOR, NULL),
        OSSL_PARAM_int(OSSL_KDF_PARAM_KBKDF_R, NULL),
        OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_KDF_PARAM_FIPS_KEY_CHECK)
        OSSL_PARAM_END,
    };
    return known_settable_ctx_params;
}

static int kbkdf_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
#ifdef FIPS_MODULE
    KBKDF *ctx = (KBKDF *)vctx;
#endif
    OSSL_PARAM *p;

    /* KBKDF can produce results as large as you like. */
    p = OSSL_PARAM_locate(params, OSSL_KDF_PARAM_SIZE);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, SIZE_MAX))
        return 0;

    if (!OSSL_FIPS_IND_GET_CTX_PARAM(ctx, params))
        return 0;
    return 1;
}

static const OSSL_PARAM *kbkdf_gettable_ctx_params(ossl_unused void *ctx,
                                                   ossl_unused void *provctx)
{
    static const OSSL_PARAM known_gettable_ctx_params[] = {
        OSSL_PARAM_size_t(OSSL_KDF_PARAM_SIZE, NULL),
        OSSL_FIPS_IND_GETTABLE_CTX_PARAM()
        OSSL_PARAM_END
    };
    return known_gettable_ctx_params;
}

const OSSL_DISPATCH ossl_kdf_kbkdf_functions[] = {
    { OSSL_FUNC_KDF_NEWCTX, (void(*)(void))kbkdf_new },
    { OSSL_FUNC_KDF_DUPCTX, (void(*)(void))kbkdf_dup },
    { OSSL_FUNC_KDF_FREECTX, (void(*)(void))kbkdf_free },
    { OSSL_FUNC_KDF_RESET, (void(*)(void))kbkdf_reset },
    { OSSL_FUNC_KDF_DERIVE, (void(*)(void))kbkdf_derive },
    { OSSL_FUNC_KDF_SETTABLE_CTX_PARAMS,
      (void(*)(void))kbkdf_settable_ctx_params },
    { OSSL_FUNC_KDF_SET_CTX_PARAMS, (void(*)(void))kbkdf_set_ctx_params },
    { OSSL_FUNC_KDF_GETTABLE_CTX_PARAMS,
      (void(*)(void))kbkdf_gettable_ctx_params },
    { OSSL_FUNC_KDF_GET_CTX_PARAMS, (void(*)(void))kbkdf_get_ctx_params },
    OSSL_DISPATCH_END,
};
