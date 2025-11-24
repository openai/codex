/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2019, Oracle and/or its affiliates.  All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Refer to https://csrc.nist.gov/publications/detail/sp/800-56c/rev-1/final
 * Section 4.1.
 *
 * The Single Step KDF algorithm is given by:
 *
 * Result(0) = empty bit string (i.e., the null string).
 * For i = 1 to reps, do the following:
 *   Increment counter by 1.
 *   Result(i) = Result(i - 1) || H(counter || Z || FixedInfo).
 * DKM = LeftmostBits(Result(reps), L))
 *
 * NOTES:
 *   Z is a shared secret required to produce the derived key material.
 *   counter is a 4 byte buffer.
 *   FixedInfo is a bit string containing context specific data.
 *   DKM is the output derived key material.
 *   L is the required size of the DKM.
 *   reps = [L / H_outputBits]
 *   H(x) is the auxiliary function that can be either a hash, HMAC or KMAC.
 *   H_outputBits is the length of the output of the auxiliary function H(x).
 *
 * Currently there is not a comprehensive list of test vectors for this
 * algorithm, especially for H(x) = HMAC and H(x) = KMAC.
 * Test vectors for H(x) = Hash are indirectly used by CAVS KAS tests.
 */
#include <stdlib.h>
#include <stdarg.h>
#include <string.h>
#include <openssl/hmac.h>
#include <openssl/evp.h>
#include <openssl/kdf.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include <openssl/proverr.h>
#include "internal/cryptlib.h"
#include "internal/numbers.h"
#include "crypto/evp.h"
#include "prov/provider_ctx.h"
#include "prov/providercommon.h"
#include "prov/implementations.h"
#include "prov/provider_util.h"
#include "prov/securitycheck.h"
#include "internal/params.h"

typedef struct {
    void *provctx;
    EVP_MAC_CTX *macctx;         /* H(x) = HMAC_hash OR H(x) = KMAC */
    PROV_DIGEST digest;          /* H(x) = hash(x) */
    unsigned char *secret;
    size_t secret_len;
    unsigned char *info;
    size_t info_len;
    unsigned char *salt;
    size_t salt_len;
    size_t out_len; /* optional KMAC parameter */
    int is_kmac;
    OSSL_FIPS_IND_DECLARE
} KDF_SSKDF;

#define SSKDF_MAX_INLEN (1<<30)
#define SSKDF_KMAC128_DEFAULT_SALT_SIZE (168 - 4)
#define SSKDF_KMAC256_DEFAULT_SALT_SIZE (136 - 4)

/* KMAC uses a Customisation string of 'KDF' */
static const unsigned char kmac_custom_str[] = { 0x4B, 0x44, 0x46 };

static OSSL_FUNC_kdf_newctx_fn sskdf_new;
static OSSL_FUNC_kdf_dupctx_fn sskdf_dup;
static OSSL_FUNC_kdf_freectx_fn sskdf_free;
static OSSL_FUNC_kdf_reset_fn sskdf_reset;
static OSSL_FUNC_kdf_derive_fn sskdf_derive;
static OSSL_FUNC_kdf_settable_ctx_params_fn sskdf_settable_ctx_params;
static OSSL_FUNC_kdf_set_ctx_params_fn sskdf_set_ctx_params;
static OSSL_FUNC_kdf_gettable_ctx_params_fn sskdf_gettable_ctx_params;
static OSSL_FUNC_kdf_get_ctx_params_fn sskdf_get_ctx_params;
static OSSL_FUNC_kdf_derive_fn x963kdf_derive;
static OSSL_FUNC_kdf_settable_ctx_params_fn x963kdf_settable_ctx_params;
static OSSL_FUNC_kdf_set_ctx_params_fn x963kdf_set_ctx_params;
static OSSL_FUNC_kdf_gettable_ctx_params_fn x963kdf_gettable_ctx_params;
static OSSL_FUNC_kdf_get_ctx_params_fn x963kdf_get_ctx_params;

/* Settable context parameters that are common across SSKDF and X963 KDF */
#define SSKDF_COMMON_SETTABLES                                      \
    OSSL_PARAM_octet_string(OSSL_KDF_PARAM_SECRET, NULL, 0),        \
    OSSL_PARAM_octet_string(OSSL_KDF_PARAM_KEY, NULL, 0),           \
    OSSL_PARAM_octet_string(OSSL_KDF_PARAM_INFO, NULL, 0),          \
    OSSL_PARAM_utf8_string(OSSL_KDF_PARAM_PROPERTIES, NULL, 0),     \
    OSSL_PARAM_utf8_string(OSSL_KDF_PARAM_DIGEST, NULL, 0),         \
    OSSL_PARAM_utf8_string(OSSL_KDF_PARAM_MAC, NULL, 0),            \
    OSSL_PARAM_octet_string(OSSL_KDF_PARAM_SALT, NULL, 0),          \
    OSSL_PARAM_size_t(OSSL_KDF_PARAM_MAC_SIZE, NULL)

/* Gettable context parameters that are common across SSKDF and X963 KDF */
#define SSKDF_COMMON_GETTABLES                                          \
    OSSL_PARAM_size_t(OSSL_KDF_PARAM_SIZE, NULL)

/*
 * Refer to https://csrc.nist.gov/publications/detail/sp/800-56c/rev-1/final
 * Section 4. One-Step Key Derivation using H(x) = hash(x)
 * Note: X9.63 also uses this code with the only difference being that the
 * counter is appended to the secret 'z'.
 * i.e.
 *   result[i] = Hash(counter || z || info) for One Step OR
 *   result[i] = Hash(z || counter || info) for X9.63.
 */
static int SSKDF_hash_kdm(const EVP_MD *kdf_md,
                          const unsigned char *z, size_t z_len,
                          const unsigned char *info, size_t info_len,
                          unsigned int append_ctr,
                          unsigned char *derived_key, size_t derived_key_len)
{
    int ret = 0, hlen;
    size_t counter, out_len, len = derived_key_len;
    unsigned char c[4];
    unsigned char mac[EVP_MAX_MD_SIZE];
    unsigned char *out = derived_key;
    EVP_MD_CTX *ctx = NULL, *ctx_init = NULL;

    if (z_len > SSKDF_MAX_INLEN || info_len > SSKDF_MAX_INLEN
            || derived_key_len > SSKDF_MAX_INLEN
            || derived_key_len == 0)
        return 0;

    hlen = EVP_MD_get_size(kdf_md);
    if (hlen <= 0)
        return 0;
    out_len = (size_t)hlen;

    ctx = EVP_MD_CTX_create();
    ctx_init = EVP_MD_CTX_create();
    if (ctx == NULL || ctx_init == NULL)
        goto end;

    if (!EVP_DigestInit(ctx_init, kdf_md))
        goto end;

    for (counter = 1;; counter++) {
        c[0] = (unsigned char)((counter >> 24) & 0xff);
        c[1] = (unsigned char)((counter >> 16) & 0xff);
        c[2] = (unsigned char)((counter >> 8) & 0xff);
        c[3] = (unsigned char)(counter & 0xff);

        if (!(EVP_MD_CTX_copy_ex(ctx, ctx_init)
                && (append_ctr || EVP_DigestUpdate(ctx, c, sizeof(c)))
                && EVP_DigestUpdate(ctx, z, z_len)
                && (!append_ctr || EVP_DigestUpdate(ctx, c, sizeof(c)))
                && EVP_DigestUpdate(ctx, info, info_len)))
            goto end;
        if (len >= out_len) {
            if (!EVP_DigestFinal_ex(ctx, out, NULL))
                goto end;
            out += out_len;
            len -= out_len;
            if (len == 0)
                break;
        } else {
            if (!EVP_DigestFinal_ex(ctx, mac, NULL))
                goto end;
            memcpy(out, mac, len);
            break;
        }
    }
    ret = 1;
end:
    EVP_MD_CTX_destroy(ctx);
    EVP_MD_CTX_destroy(ctx_init);
    OPENSSL_cleanse(mac, sizeof(mac));
    return ret;
}

static int kmac_init(EVP_MAC_CTX *ctx, const unsigned char *custom,
                     size_t custom_len, size_t kmac_out_len,
                     size_t derived_key_len, unsigned char **out)
{
    OSSL_PARAM params[2];

    /* Only KMAC has custom data - so return if not KMAC */
    if (custom == NULL)
        return 1;

    params[0] = OSSL_PARAM_construct_octet_string(OSSL_MAC_PARAM_CUSTOM,
                                                  (void *)custom, custom_len);
    params[1] = OSSL_PARAM_construct_end();

    if (!EVP_MAC_CTX_set_params(ctx, params))
        return 0;

    /* By default only do one iteration if kmac_out_len is not specified */
    if (kmac_out_len == 0)
        kmac_out_len = derived_key_len;
    /* otherwise check the size is valid */
    else if (!(kmac_out_len == derived_key_len
            || kmac_out_len == 20
            || kmac_out_len == 28
            || kmac_out_len == 32
            || kmac_out_len == 48
            || kmac_out_len == 64))
        return 0;

    params[0] = OSSL_PARAM_construct_size_t(OSSL_MAC_PARAM_SIZE,
                                            &kmac_out_len);

    if (EVP_MAC_CTX_set_params(ctx, params) <= 0)
        return 0;

    /*
     * For kmac the output buffer can be larger than EVP_MAX_MD_SIZE: so
     * alloc a buffer for this case.
     */
    if (kmac_out_len > EVP_MAX_MD_SIZE) {
        *out = OPENSSL_zalloc(kmac_out_len);
        if (*out == NULL)
            return 0;
    }
    return 1;
}

/*
 * Refer to https://csrc.nist.gov/publications/detail/sp/800-56c/rev-1/final
 * Section 4. One-Step Key Derivation using MAC: i.e either
 *     H(x) = HMAC-hash(salt, x) OR
 *     H(x) = KMAC#(salt, x, outbits, CustomString='KDF')
 */
static int SSKDF_mac_kdm(EVP_MAC_CTX *ctx_init,
                         const unsigned char *kmac_custom,
                         size_t kmac_custom_len, size_t kmac_out_len,
                         const unsigned char *salt, size_t salt_len,
                         const unsigned char *z, size_t z_len,
                         const unsigned char *info, size_t info_len,
                         unsigned char *derived_key, size_t derived_key_len)
{
    int ret = 0;
    size_t counter, out_len, len;
    unsigned char c[4];
    unsigned char mac_buf[EVP_MAX_MD_SIZE];
    unsigned char *out = derived_key;
    EVP_MAC_CTX *ctx = NULL;
    unsigned char *mac = mac_buf, *kmac_buffer = NULL;

    if (z_len > SSKDF_MAX_INLEN || info_len > SSKDF_MAX_INLEN
            || derived_key_len > SSKDF_MAX_INLEN
            || derived_key_len == 0)
        return 0;

    if (!kmac_init(ctx_init, kmac_custom, kmac_custom_len, kmac_out_len,
                   derived_key_len, &kmac_buffer))
        goto end;
    if (kmac_buffer != NULL)
        mac = kmac_buffer;

    if (!EVP_MAC_init(ctx_init, salt, salt_len, NULL))
        goto end;

    out_len = EVP_MAC_CTX_get_mac_size(ctx_init); /* output size */
    if (out_len <= 0 || (mac == mac_buf && out_len > sizeof(mac_buf)))
        goto end;
    len = derived_key_len;

    for (counter = 1;; counter++) {
        c[0] = (unsigned char)((counter >> 24) & 0xff);
        c[1] = (unsigned char)((counter >> 16) & 0xff);
        c[2] = (unsigned char)((counter >> 8) & 0xff);
        c[3] = (unsigned char)(counter & 0xff);

        ctx = EVP_MAC_CTX_dup(ctx_init);
        if (!(ctx != NULL
                && EVP_MAC_update(ctx, c, sizeof(c))
                && EVP_MAC_update(ctx, z, z_len)
                && EVP_MAC_update(ctx, info, info_len)))
            goto end;
        if (len >= out_len) {
            if (!EVP_MAC_final(ctx, out, NULL, len))
                goto end;
            out += out_len;
            len -= out_len;
            if (len == 0)
                break;
        } else {
            if (!EVP_MAC_final(ctx, mac, NULL, out_len))
                goto end;
            memcpy(out, mac, len);
            break;
        }
        EVP_MAC_CTX_free(ctx);
        ctx = NULL;
    }
    ret = 1;
end:
    if (kmac_buffer != NULL)
        OPENSSL_clear_free(kmac_buffer, kmac_out_len);
    else
        OPENSSL_cleanse(mac_buf, sizeof(mac_buf));

    EVP_MAC_CTX_free(ctx);
    return ret;
}

static void *sskdf_new(void *provctx)
{
    KDF_SSKDF *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    if ((ctx = OPENSSL_zalloc(sizeof(*ctx))) != NULL) {
        ctx->provctx = provctx;
        OSSL_FIPS_IND_INIT(ctx)
    }
    return ctx;
}

static void sskdf_reset(void *vctx)
{
    KDF_SSKDF *ctx = (KDF_SSKDF *)vctx;
    void *provctx = ctx->provctx;

    EVP_MAC_CTX_free(ctx->macctx);
    ossl_prov_digest_reset(&ctx->digest);
    OPENSSL_clear_free(ctx->secret, ctx->secret_len);
    OPENSSL_clear_free(ctx->info, ctx->info_len);
    OPENSSL_clear_free(ctx->salt, ctx->salt_len);
    memset(ctx, 0, sizeof(*ctx));
    ctx->provctx = provctx;
}

static void sskdf_free(void *vctx)
{
    KDF_SSKDF *ctx = (KDF_SSKDF *)vctx;

    if (ctx != NULL) {
        sskdf_reset(ctx);
        OPENSSL_free(ctx);
    }
}

static void *sskdf_dup(void *vctx)
{
    const KDF_SSKDF *src = (const KDF_SSKDF *)vctx;
    KDF_SSKDF *dest;

    dest = sskdf_new(src->provctx);
    if (dest != NULL) {
        if (src->macctx != NULL) {
            dest->macctx = EVP_MAC_CTX_dup(src->macctx);
            if (dest->macctx == NULL)
                goto err;
        }
        if (!ossl_prov_memdup(src->info, src->info_len,
                              &dest->info, &dest->info_len)
                || !ossl_prov_memdup(src->salt, src->salt_len,
                                     &dest->salt , &dest->salt_len)
                || !ossl_prov_memdup(src->secret, src->secret_len,
                                     &dest->secret, &dest->secret_len)
                || !ossl_prov_digest_copy(&dest->digest, &src->digest))
            goto err;
        dest->out_len = src->out_len;
        dest->is_kmac = src->is_kmac;
        OSSL_FIPS_IND_COPY(dest, src)
    }
    return dest;

 err:
    sskdf_free(dest);
    return NULL;
}

static size_t sskdf_size(KDF_SSKDF *ctx)
{
    int len;
    const EVP_MD *md = NULL;

    if (ctx->is_kmac)
        return SIZE_MAX;

    md = ossl_prov_digest_md(&ctx->digest);
    if (md == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_MESSAGE_DIGEST);
        return 0;
    }
    len = EVP_MD_get_size(md);
    return (len <= 0) ? 0 : (size_t)len;
}

#ifdef FIPS_MODULE
static int fips_sskdf_key_check_passed(KDF_SSKDF *ctx)
{
    OSSL_LIB_CTX *libctx = PROV_LIBCTX_OF(ctx->provctx);
    int key_approved = ossl_kdf_check_key_size(ctx->secret_len);

    if (!key_approved) {
        if (!OSSL_FIPS_IND_ON_UNAPPROVED(ctx, OSSL_FIPS_IND_SETTABLE0,
                                         libctx, "SSKDF", "Key size",
                                         ossl_fips_config_sskdf_key_check)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
            return 0;
        }
    }
    return 1;
}
#endif

static int sskdf_derive(void *vctx, unsigned char *key, size_t keylen,
                        const OSSL_PARAM params[])
{
    KDF_SSKDF *ctx = (KDF_SSKDF *)vctx;
    const EVP_MD *md;

    if (!ossl_prov_is_running() || !sskdf_set_ctx_params(ctx, params))
        return 0;
    if (ctx->secret == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_SECRET);
        return 0;
    }

    md = ossl_prov_digest_md(&ctx->digest);

    if (ctx->macctx != NULL) {
        /* H(x) = KMAC or H(x) = HMAC */
        int ret;
        const unsigned char *custom = NULL;
        size_t custom_len = 0;
        int default_salt_len;
        EVP_MAC *mac = EVP_MAC_CTX_get0_mac(ctx->macctx);

        if (EVP_MAC_is_a(mac, OSSL_MAC_NAME_HMAC)) {
            /* H(x) = HMAC(x, salt, hash) */
            if (md == NULL) {
                ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_MESSAGE_DIGEST);
                return 0;
            }
            default_salt_len = EVP_MD_get_size(md);
            if (default_salt_len <= 0)
                return 0;
        } else if (ctx->is_kmac) {
            /* H(x) = KMACzzz(x, salt, custom) */
            custom = kmac_custom_str;
            custom_len = sizeof(kmac_custom_str);
            if (EVP_MAC_is_a(mac, OSSL_MAC_NAME_KMAC128))
                default_salt_len = SSKDF_KMAC128_DEFAULT_SALT_SIZE;
            else
                default_salt_len = SSKDF_KMAC256_DEFAULT_SALT_SIZE;
        } else {
            ERR_raise(ERR_LIB_PROV, PROV_R_UNSUPPORTED_MAC_TYPE);
            return 0;
        }
        /* If no salt is set then use a default_salt of zeros */
        if (ctx->salt == NULL || ctx->salt_len <= 0) {
            ctx->salt = OPENSSL_zalloc(default_salt_len);
            if (ctx->salt == NULL)
                return 0;
            ctx->salt_len = default_salt_len;
        }
        ret = SSKDF_mac_kdm(ctx->macctx,
                            custom, custom_len, ctx->out_len,
                            ctx->salt, ctx->salt_len,
                            ctx->secret, ctx->secret_len,
                            ctx->info, ctx->info_len, key, keylen);
        return ret;
    } else {
        /* H(x) = hash */
        if (md == NULL) {
            ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_MESSAGE_DIGEST);
            return 0;
        }
        return SSKDF_hash_kdm(md, ctx->secret, ctx->secret_len,
                              ctx->info, ctx->info_len, 0, key, keylen);
    }
}

#ifdef FIPS_MODULE
static int fips_x963kdf_digest_check_passed(KDF_SSKDF *ctx, const EVP_MD *md)
{
    OSSL_LIB_CTX *libctx = PROV_LIBCTX_OF(ctx->provctx);
    /*
     * Perform digest check
     *
     * X963KDF is a KDF defined in ANSI-X9.63. According to ACVP specification
     * section 7.3.1, only SHA-2 and SHA-3 can be regarded as valid hash
     * functions.
     */
    int digest_unapproved = (ctx->is_kmac != 1) && EVP_MD_is_a(md, SN_sha1);

    if (digest_unapproved) {
        if (!OSSL_FIPS_IND_ON_UNAPPROVED(ctx, OSSL_FIPS_IND_SETTABLE0,
                                         libctx, "X963KDF", "Digest",
                                         ossl_fips_config_x963kdf_digest_check)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_DIGEST_NOT_ALLOWED);
            return 0;
        }
    }
    return 1;
}

static int fips_x963kdf_key_check_passed(KDF_SSKDF *ctx)
{
    OSSL_LIB_CTX *libctx = PROV_LIBCTX_OF(ctx->provctx);
    int key_approved = ossl_kdf_check_key_size(ctx->secret_len);

    if (!key_approved) {
        if (!OSSL_FIPS_IND_ON_UNAPPROVED(ctx, OSSL_FIPS_IND_SETTABLE1,
                                         libctx, "X963KDF", "Key size",
                                         ossl_fips_config_x963kdf_key_check)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
            return 0;
        }
    }
    return 1;
}
#endif

static int x963kdf_derive(void *vctx, unsigned char *key, size_t keylen,
                          const OSSL_PARAM params[])
{
    KDF_SSKDF *ctx = (KDF_SSKDF *)vctx;
    const EVP_MD *md;

    if (!ossl_prov_is_running() || !x963kdf_set_ctx_params(ctx, params))
        return 0;

    if (ctx->secret == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_SECRET);
        return 0;
    }

    if (ctx->macctx != NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_NOT_SUPPORTED);
        return 0;
    }

    /* H(x) = hash */
    md = ossl_prov_digest_md(&ctx->digest);
    if (md == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_MESSAGE_DIGEST);
        return 0;
    }

    return SSKDF_hash_kdm(md, ctx->secret, ctx->secret_len,
                          ctx->info, ctx->info_len, 1, key, keylen);
}

static int sskdf_common_set_ctx_params(KDF_SSKDF *ctx, const OSSL_PARAM params[])
{
    const OSSL_PARAM *p;
    OSSL_LIB_CTX *libctx = PROV_LIBCTX_OF(ctx->provctx);
    const EVP_MD *md = NULL;
    size_t sz;
    int r;

    if (ossl_param_is_empty(params))
        return 1;

    if (!ossl_prov_macctx_load_from_params(&ctx->macctx, params,
                                           NULL, NULL, NULL, libctx))
        return 0;
    if (ctx->macctx != NULL) {
         if (EVP_MAC_is_a(EVP_MAC_CTX_get0_mac(ctx->macctx),
                          OSSL_MAC_NAME_KMAC128)
             || EVP_MAC_is_a(EVP_MAC_CTX_get0_mac(ctx->macctx),
                             OSSL_MAC_NAME_KMAC256)) {
             ctx->is_kmac = 1;
         }
    }

    if (OSSL_PARAM_locate_const(params, OSSL_ALG_PARAM_DIGEST) != NULL) {
        if (!ossl_prov_digest_load_from_params(&ctx->digest, params, libctx))
            return 0;

        md = ossl_prov_digest_md(&ctx->digest);
        if (EVP_MD_xof(md)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_XOF_DIGESTS_NOT_ALLOWED);
            return 0;
        }
    }

    r = ossl_param_get1_octet_string(params, OSSL_KDF_PARAM_SECRET,
                                     &ctx->secret, &ctx->secret_len);
    if (r == -1)
        r = ossl_param_get1_octet_string(params, OSSL_KDF_PARAM_KEY,
                                         &ctx->secret, &ctx->secret_len);
    if (r == 0)
        return 0;

    if (ossl_param_get1_concat_octet_string(params, OSSL_KDF_PARAM_INFO,
                                            &ctx->info, &ctx->info_len, 0) == 0)
        return 0;

    if (ossl_param_get1_octet_string(params, OSSL_KDF_PARAM_SALT,
                                     &ctx->salt, &ctx->salt_len) == 0)
            return 0;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_MAC_SIZE))
        != NULL) {
        if (!OSSL_PARAM_get_size_t(p, &sz) || sz == 0)
            return 0;
        ctx->out_len = sz;
    }
    return 1;
}

static int sskdf_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    KDF_SSKDF *ctx = (KDF_SSKDF *)vctx;

    if (ossl_param_is_empty(params))
        return 1;

    if (!OSSL_FIPS_IND_SET_CTX_PARAM(ctx, OSSL_FIPS_IND_SETTABLE0, params,
                                     OSSL_KDF_PARAM_FIPS_KEY_CHECK))
        return 0;

    if (!sskdf_common_set_ctx_params(ctx, params))
        return 0;

#ifdef FIPS_MODULE
    if ((OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_KEY) != NULL) ||
        (OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_SECRET) != NULL))
        if (!fips_sskdf_key_check_passed(ctx))
            return 0;
#endif

    return 1;
}

static const OSSL_PARAM *sskdf_settable_ctx_params(ossl_unused void *ctx,
                                                   ossl_unused void *provctx)
{
    static const OSSL_PARAM known_settable_ctx_params[] = {
        SSKDF_COMMON_SETTABLES,
        OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_KDF_PARAM_FIPS_KEY_CHECK)
        OSSL_PARAM_END
    };
    return known_settable_ctx_params;
}

static int sskdf_common_get_ctx_params(KDF_SSKDF *ctx, OSSL_PARAM params[])
{
    OSSL_PARAM *p;

    if (ossl_param_is_empty(params))
        return 1;

    if ((p = OSSL_PARAM_locate(params, OSSL_KDF_PARAM_SIZE)) != NULL) {
        if (!OSSL_PARAM_set_size_t(p, sskdf_size(ctx)))
            return 0;
    }

    return 1;
}

static int sskdf_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    KDF_SSKDF *ctx = (KDF_SSKDF *)vctx;

    if (ossl_param_is_empty(params))
        return 1;

    if (!sskdf_common_get_ctx_params(ctx, params))
        return 0;

    if (!OSSL_FIPS_IND_GET_CTX_PARAM(ctx, params))
        return 0;

    return 1;
}

static const OSSL_PARAM *sskdf_gettable_ctx_params(ossl_unused void *ctx,
                                                   ossl_unused void *provctx)
{
    static const OSSL_PARAM known_gettable_ctx_params[] = {
        SSKDF_COMMON_GETTABLES,
        OSSL_FIPS_IND_GETTABLE_CTX_PARAM()
        OSSL_PARAM_END
    };
    return known_gettable_ctx_params;
}

static int x963kdf_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    KDF_SSKDF *ctx = (KDF_SSKDF *)vctx;

    if (ossl_param_is_empty(params))
        return 1;

    if (!OSSL_FIPS_IND_SET_CTX_PARAM(ctx, OSSL_FIPS_IND_SETTABLE0, params,
                                     OSSL_KDF_PARAM_FIPS_DIGEST_CHECK))
        return 0;
    if (!OSSL_FIPS_IND_SET_CTX_PARAM(ctx, OSSL_FIPS_IND_SETTABLE1, params,
                                     OSSL_KDF_PARAM_FIPS_KEY_CHECK))
        return 0;

    if (!sskdf_common_set_ctx_params(ctx, params))
        return 0;

#ifdef FIPS_MODULE
    if (OSSL_PARAM_locate_const(params, OSSL_ALG_PARAM_DIGEST) != NULL) {
        const EVP_MD *md = ossl_prov_digest_md(&ctx->digest);

        if (!fips_x963kdf_digest_check_passed(ctx, md))
            return 0;
    }

    if ((OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_KEY) != NULL) ||
        (OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_SECRET) != NULL))
        if (!fips_x963kdf_key_check_passed(ctx))
            return 0;
#endif

    return 1;
}

static const OSSL_PARAM *x963kdf_settable_ctx_params(ossl_unused void *ctx,
                                                     ossl_unused void *provctx)
{
    static const OSSL_PARAM known_settable_ctx_params[] = {
        SSKDF_COMMON_SETTABLES,
        OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_KDF_PARAM_FIPS_DIGEST_CHECK)
        OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_KDF_PARAM_FIPS_KEY_CHECK)
        OSSL_PARAM_END
    };
    return known_settable_ctx_params;
}

static int x963kdf_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    KDF_SSKDF *ctx = (KDF_SSKDF *)vctx;

    if (!sskdf_common_get_ctx_params(ctx, params))
        return 0;

    if (!OSSL_FIPS_IND_GET_CTX_PARAM(ctx, params))
        return 0;

    return 1;
}

static const OSSL_PARAM *x963kdf_gettable_ctx_params(ossl_unused void *ctx,
                                                     ossl_unused void *provctx)
{
    static const OSSL_PARAM known_gettable_ctx_params[] = {
        SSKDF_COMMON_GETTABLES,
        OSSL_FIPS_IND_GETTABLE_CTX_PARAM()
        OSSL_PARAM_END
    };
    return known_gettable_ctx_params;
}

const OSSL_DISPATCH ossl_kdf_sskdf_functions[] = {
    { OSSL_FUNC_KDF_NEWCTX, (void(*)(void))sskdf_new },
    { OSSL_FUNC_KDF_DUPCTX, (void(*)(void))sskdf_dup },
    { OSSL_FUNC_KDF_FREECTX, (void(*)(void))sskdf_free },
    { OSSL_FUNC_KDF_RESET, (void(*)(void))sskdf_reset },
    { OSSL_FUNC_KDF_DERIVE, (void(*)(void))sskdf_derive },
    { OSSL_FUNC_KDF_SETTABLE_CTX_PARAMS,
      (void(*)(void))sskdf_settable_ctx_params },
    { OSSL_FUNC_KDF_SET_CTX_PARAMS, (void(*)(void))sskdf_set_ctx_params },
    { OSSL_FUNC_KDF_GETTABLE_CTX_PARAMS,
      (void(*)(void))sskdf_gettable_ctx_params },
    { OSSL_FUNC_KDF_GET_CTX_PARAMS, (void(*)(void))sskdf_get_ctx_params },
    OSSL_DISPATCH_END
};

const OSSL_DISPATCH ossl_kdf_x963_kdf_functions[] = {
    { OSSL_FUNC_KDF_NEWCTX, (void(*)(void))sskdf_new },
    { OSSL_FUNC_KDF_DUPCTX, (void(*)(void))sskdf_dup },
    { OSSL_FUNC_KDF_FREECTX, (void(*)(void))sskdf_free },
    { OSSL_FUNC_KDF_RESET, (void(*)(void))sskdf_reset },
    { OSSL_FUNC_KDF_DERIVE, (void(*)(void))x963kdf_derive },
    { OSSL_FUNC_KDF_SETTABLE_CTX_PARAMS,
      (void(*)(void))x963kdf_settable_ctx_params },
    { OSSL_FUNC_KDF_SET_CTX_PARAMS, (void(*)(void))x963kdf_set_ctx_params },
    { OSSL_FUNC_KDF_GETTABLE_CTX_PARAMS,
      (void(*)(void))x963kdf_gettable_ctx_params },
    { OSSL_FUNC_KDF_GET_CTX_PARAMS, (void(*)(void))x963kdf_get_ctx_params },
    OSSL_DISPATCH_END
};
