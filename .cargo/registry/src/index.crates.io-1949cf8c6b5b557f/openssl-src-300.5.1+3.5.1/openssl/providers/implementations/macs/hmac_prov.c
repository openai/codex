/*
 * Copyright 2018-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * HMAC low level APIs are deprecated for public use, but still ok for internal
 * use.
 */
#include "internal/deprecated.h"

#include <string.h>

#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include <openssl/evp.h>
#include <openssl/hmac.h>
#include <openssl/proverr.h>
#include <openssl/err.h>

#include "internal/ssl3_cbc.h"

#include "prov/implementations.h"
#include "prov/provider_ctx.h"
#include "prov/provider_util.h"
#include "prov/providercommon.h"
#include "prov/securitycheck.h"

/*
 * Forward declaration of everything implemented here.  This is not strictly
 * necessary for the compiler, but provides an assurance that the signatures
 * of the functions in the dispatch table are correct.
 */
static OSSL_FUNC_mac_newctx_fn hmac_new;
static OSSL_FUNC_mac_dupctx_fn hmac_dup;
static OSSL_FUNC_mac_freectx_fn hmac_free;
static OSSL_FUNC_mac_gettable_ctx_params_fn hmac_gettable_ctx_params;
static OSSL_FUNC_mac_get_ctx_params_fn hmac_get_ctx_params;
static OSSL_FUNC_mac_settable_ctx_params_fn hmac_settable_ctx_params;
static OSSL_FUNC_mac_set_ctx_params_fn hmac_set_ctx_params;
static OSSL_FUNC_mac_init_fn hmac_init;
static OSSL_FUNC_mac_update_fn hmac_update;
static OSSL_FUNC_mac_final_fn hmac_final;

/* local HMAC context structure */

/* typedef EVP_MAC_IMPL */
struct hmac_data_st {
    void *provctx;
    HMAC_CTX *ctx;               /* HMAC context */
    PROV_DIGEST digest;
    unsigned char *key;
    size_t keylen;
    /* Length of full TLS record including the MAC and any padding */
    size_t tls_data_size;
    unsigned char tls_header[13];
    int tls_header_set;
    unsigned char tls_mac_out[EVP_MAX_MD_SIZE];
    size_t tls_mac_out_size;
#ifdef FIPS_MODULE
    /*
     * 'internal' is set to 1 if HMAC is used inside another algorithm such as a
     * KDF. In this case it is the parent algorithm that is responsible for
     * performing any conditional FIPS indicator related checks for the HMAC.
     */
    int internal;
#endif
    OSSL_FIPS_IND_DECLARE
};

static void *hmac_new(void *provctx)
{
    struct hmac_data_st *macctx;

    if (!ossl_prov_is_running())
        return NULL;

    if ((macctx = OPENSSL_zalloc(sizeof(*macctx))) == NULL
        || (macctx->ctx = HMAC_CTX_new()) == NULL) {
        OPENSSL_free(macctx);
        return NULL;
    }
    macctx->provctx = provctx;
    OSSL_FIPS_IND_INIT(macctx)

    return macctx;
}

static void hmac_free(void *vmacctx)
{
    struct hmac_data_st *macctx = vmacctx;

    if (macctx != NULL) {
        HMAC_CTX_free(macctx->ctx);
        ossl_prov_digest_reset(&macctx->digest);
        OPENSSL_secure_clear_free(macctx->key, macctx->keylen);
        OPENSSL_free(macctx);
    }
}

static void *hmac_dup(void *vsrc)
{
    struct hmac_data_st *src = vsrc;
    struct hmac_data_st *dst;
    HMAC_CTX *ctx;

    if (!ossl_prov_is_running())
        return NULL;
    dst = hmac_new(src->provctx);
    if (dst == NULL)
        return NULL;

    ctx = dst->ctx;
    *dst = *src;
    dst->ctx = ctx;
    dst->key = NULL;
    memset(&dst->digest, 0, sizeof(dst->digest));

    if (!HMAC_CTX_copy(dst->ctx, src->ctx)
        || !ossl_prov_digest_copy(&dst->digest, &src->digest)) {
        hmac_free(dst);
        return NULL;
    }
    if (src->key != NULL) {
        /* There is no "secure" OPENSSL_memdup */
        dst->key = OPENSSL_secure_malloc(src->keylen > 0 ? src->keylen : 1);
        if (dst->key == NULL) {
            hmac_free(dst);
            return 0;
        }
        memcpy(dst->key, src->key, src->keylen);
    }
    return dst;
}

static size_t hmac_size(struct hmac_data_st *macctx)
{
    return HMAC_size(macctx->ctx);
}

static int hmac_block_size(struct hmac_data_st *macctx)
{
    const EVP_MD *md = ossl_prov_digest_md(&macctx->digest);

    if (md == NULL)
        return 0;
    return EVP_MD_block_size(md);
}

static int hmac_setkey(struct hmac_data_st *macctx,
                       const unsigned char *key, size_t keylen)
{
    const EVP_MD *digest;

#ifdef FIPS_MODULE
    /*
     * KDF's pass a salt rather than a key,
     * which is why it skips the key check unless "HMAC" is fetched directly.
     */
    if (!macctx->internal) {
        OSSL_LIB_CTX *libctx = PROV_LIBCTX_OF(macctx->provctx);
        int approved = ossl_mac_check_key_size(keylen);

        if (!approved) {
            if (!OSSL_FIPS_IND_ON_UNAPPROVED(macctx, OSSL_FIPS_IND_SETTABLE0,
                                             libctx, "HMAC", "keysize",
                                             ossl_fips_config_hmac_key_check)) {
                ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
                return 0;
            }
        }
    }
#endif

    if (macctx->key != NULL)
        OPENSSL_secure_clear_free(macctx->key, macctx->keylen);
    /* Keep a copy of the key in case we need it for TLS HMAC */
    macctx->key = OPENSSL_secure_malloc(keylen > 0 ? keylen : 1);
    if (macctx->key == NULL)
        return 0;

    memcpy(macctx->key, key, keylen);
    macctx->keylen = keylen;

    digest = ossl_prov_digest_md(&macctx->digest);
    /* HMAC_Init_ex doesn't tolerate all zero params, so we must be careful */
    if (key != NULL || (macctx->tls_data_size == 0 && digest != NULL))
        return HMAC_Init_ex(macctx->ctx, key, keylen, digest,
                            ossl_prov_digest_engine(&macctx->digest));
    return 1;
}

static int hmac_init(void *vmacctx, const unsigned char *key,
                     size_t keylen, const OSSL_PARAM params[])
{
    struct hmac_data_st *macctx = vmacctx;

    if (!ossl_prov_is_running() || !hmac_set_ctx_params(macctx, params))
        return 0;

    if (key != NULL)
        return hmac_setkey(macctx, key, keylen);

    /* Just reinit the HMAC context */
    return HMAC_Init_ex(macctx->ctx, NULL, 0, NULL, NULL);
}

static int hmac_update(void *vmacctx, const unsigned char *data,
                       size_t datalen)
{
    struct hmac_data_st *macctx = vmacctx;

    if (macctx->tls_data_size > 0) {
        /* We're doing a TLS HMAC */
        if (!macctx->tls_header_set) {
            /* We expect the first update call to contain the TLS header */
            if (datalen != sizeof(macctx->tls_header))
                return 0;
            memcpy(macctx->tls_header, data, datalen);
            macctx->tls_header_set = 1;
            return 1;
        }
        /* macctx->tls_data_size is datalen plus the padding length */
        if (macctx->tls_data_size < datalen)
            return 0;

        return ssl3_cbc_digest_record(ossl_prov_digest_md(&macctx->digest),
                                      macctx->tls_mac_out,
                                      &macctx->tls_mac_out_size,
                                      macctx->tls_header,
                                      data,
                                      datalen,
                                      macctx->tls_data_size,
                                      macctx->key,
                                      macctx->keylen,
                                      0);
    }

    return HMAC_Update(macctx->ctx, data, datalen);
}

static int hmac_final(void *vmacctx, unsigned char *out, size_t *outl,
                      size_t outsize)
{
    unsigned int hlen;
    struct hmac_data_st *macctx = vmacctx;

    if (!ossl_prov_is_running())
        return 0;
    if (macctx->tls_data_size > 0) {
        if (macctx->tls_mac_out_size == 0)
            return 0;
        if (outl != NULL)
            *outl = macctx->tls_mac_out_size;
        memcpy(out, macctx->tls_mac_out, macctx->tls_mac_out_size);
        return 1;
    }
    if (!HMAC_Final(macctx->ctx, out, &hlen))
        return 0;
    *outl = hlen;
    return 1;
}

static const OSSL_PARAM known_gettable_ctx_params[] = {
    OSSL_PARAM_size_t(OSSL_MAC_PARAM_SIZE, NULL),
    OSSL_PARAM_size_t(OSSL_MAC_PARAM_BLOCK_SIZE, NULL),
    OSSL_FIPS_IND_GETTABLE_CTX_PARAM()
    OSSL_PARAM_END
};
static const OSSL_PARAM *hmac_gettable_ctx_params(ossl_unused void *ctx,
                                                  ossl_unused void *provctx)
{
    return known_gettable_ctx_params;
}

static int hmac_get_ctx_params(void *vmacctx, OSSL_PARAM params[])
{
    struct hmac_data_st *macctx = vmacctx;
    OSSL_PARAM *p;

    if ((p = OSSL_PARAM_locate(params, OSSL_MAC_PARAM_SIZE)) != NULL
            && !OSSL_PARAM_set_size_t(p, hmac_size(macctx)))
        return 0;

    if ((p = OSSL_PARAM_locate(params, OSSL_MAC_PARAM_BLOCK_SIZE)) != NULL
            && !OSSL_PARAM_set_int(p, hmac_block_size(macctx)))
        return 0;

#ifdef FIPS_MODULE
    p = OSSL_PARAM_locate(params, OSSL_MAC_PARAM_FIPS_APPROVED_INDICATOR);
    if (p != NULL) {
        int approved = 0;

        if (!macctx->internal)
            approved = OSSL_FIPS_IND_GET(macctx)->approved;
        if (!OSSL_PARAM_set_int(p, approved))
            return 0;
    }
#endif
    return 1;
}

static const OSSL_PARAM known_settable_ctx_params[] = {
    OSSL_PARAM_utf8_string(OSSL_MAC_PARAM_DIGEST, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_MAC_PARAM_PROPERTIES, NULL, 0),
    OSSL_PARAM_octet_string(OSSL_MAC_PARAM_KEY, NULL, 0),
    OSSL_PARAM_int(OSSL_MAC_PARAM_DIGEST_NOINIT, NULL),
    OSSL_PARAM_int(OSSL_MAC_PARAM_DIGEST_ONESHOT, NULL),
    OSSL_PARAM_size_t(OSSL_MAC_PARAM_TLS_DATA_SIZE, NULL),
    OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_MAC_PARAM_FIPS_KEY_CHECK)
    OSSL_PARAM_END
};
static const OSSL_PARAM *hmac_settable_ctx_params(ossl_unused void *ctx,
                                                  ossl_unused void *provctx)
{
    return known_settable_ctx_params;
}

/*
 * ALL parameters should be set before init().
 */
static int hmac_set_ctx_params(void *vmacctx, const OSSL_PARAM params[])
{
    struct hmac_data_st *macctx = vmacctx;
    OSSL_LIB_CTX *ctx = PROV_LIBCTX_OF(macctx->provctx);
    const OSSL_PARAM *p;

    if (ossl_param_is_empty(params))
        return 1;

    if (!OSSL_FIPS_IND_SET_CTX_PARAM(macctx, OSSL_FIPS_IND_SETTABLE0, params,
                                     OSSL_MAC_PARAM_FIPS_KEY_CHECK))
        return 0;

    if (!ossl_prov_digest_load_from_params(&macctx->digest, params, ctx))
        return 0;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_MAC_PARAM_KEY)) != NULL) {
        if (p->data_type != OSSL_PARAM_OCTET_STRING)
            return 0;

        if (!hmac_setkey(macctx, p->data, p->data_size))
            return 0;
    }

    if ((p = OSSL_PARAM_locate_const(params,
                                     OSSL_MAC_PARAM_TLS_DATA_SIZE)) != NULL) {
        if (!OSSL_PARAM_get_size_t(p, &macctx->tls_data_size))
            return 0;
    }
    return 1;
}

const OSSL_DISPATCH ossl_hmac_functions[] = {
    { OSSL_FUNC_MAC_NEWCTX, (void (*)(void))hmac_new },
    { OSSL_FUNC_MAC_DUPCTX, (void (*)(void))hmac_dup },
    { OSSL_FUNC_MAC_FREECTX, (void (*)(void))hmac_free },
    { OSSL_FUNC_MAC_INIT, (void (*)(void))hmac_init },
    { OSSL_FUNC_MAC_UPDATE, (void (*)(void))hmac_update },
    { OSSL_FUNC_MAC_FINAL, (void (*)(void))hmac_final },
    { OSSL_FUNC_MAC_GETTABLE_CTX_PARAMS,
      (void (*)(void))hmac_gettable_ctx_params },
    { OSSL_FUNC_MAC_GET_CTX_PARAMS, (void (*)(void))hmac_get_ctx_params },
    { OSSL_FUNC_MAC_SETTABLE_CTX_PARAMS,
      (void (*)(void))hmac_settable_ctx_params },
    { OSSL_FUNC_MAC_SET_CTX_PARAMS, (void (*)(void))hmac_set_ctx_params },
    OSSL_DISPATCH_END
};

#ifdef FIPS_MODULE
static OSSL_FUNC_mac_newctx_fn hmac_internal_new;

static void *hmac_internal_new(void *provctx)
{
    struct hmac_data_st *macctx = hmac_new(provctx);

    if (macctx != NULL)
        macctx->internal = 1;
    return macctx;
}

const OSSL_DISPATCH ossl_hmac_internal_functions[] = {
    { OSSL_FUNC_MAC_NEWCTX, (void (*)(void))hmac_internal_new },
    { OSSL_FUNC_MAC_DUPCTX, (void (*)(void))hmac_dup },
    { OSSL_FUNC_MAC_FREECTX, (void (*)(void))hmac_free },
    { OSSL_FUNC_MAC_INIT, (void (*)(void))hmac_init },
    { OSSL_FUNC_MAC_UPDATE, (void (*)(void))hmac_update },
    { OSSL_FUNC_MAC_FINAL, (void (*)(void))hmac_final },
    { OSSL_FUNC_MAC_GETTABLE_CTX_PARAMS,
      (void (*)(void))hmac_gettable_ctx_params },
    { OSSL_FUNC_MAC_GET_CTX_PARAMS, (void (*)(void))hmac_get_ctx_params },
    { OSSL_FUNC_MAC_SETTABLE_CTX_PARAMS,
      (void (*)(void))hmac_settable_ctx_params },
    { OSSL_FUNC_MAC_SET_CTX_PARAMS, (void (*)(void))hmac_set_ctx_params },
    OSSL_DISPATCH_END
};

#endif /* FIPS_MODULE */
