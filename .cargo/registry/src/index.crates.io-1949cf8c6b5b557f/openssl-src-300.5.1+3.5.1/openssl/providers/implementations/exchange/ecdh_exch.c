/*
 * Copyright 2020-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * ECDH low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <string.h>
#include <openssl/crypto.h>
#include <openssl/evp.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/ec.h>
#include <openssl/params.h>
#include <openssl/err.h>
#include <openssl/proverr.h>
#include "prov/provider_ctx.h"
#include "prov/providercommon.h"
#include "prov/implementations.h"
#include "prov/securitycheck.h"
#include "crypto/ec.h" /* ossl_ecdh_kdf_X9_63() */

static OSSL_FUNC_keyexch_newctx_fn ecdh_newctx;
static OSSL_FUNC_keyexch_init_fn ecdh_init;
static OSSL_FUNC_keyexch_set_peer_fn ecdh_set_peer;
static OSSL_FUNC_keyexch_derive_fn ecdh_derive;
static OSSL_FUNC_keyexch_freectx_fn ecdh_freectx;
static OSSL_FUNC_keyexch_dupctx_fn ecdh_dupctx;
static OSSL_FUNC_keyexch_set_ctx_params_fn ecdh_set_ctx_params;
static OSSL_FUNC_keyexch_settable_ctx_params_fn ecdh_settable_ctx_params;
static OSSL_FUNC_keyexch_get_ctx_params_fn ecdh_get_ctx_params;
static OSSL_FUNC_keyexch_gettable_ctx_params_fn ecdh_gettable_ctx_params;

enum kdf_type {
    PROV_ECDH_KDF_NONE = 0,
    PROV_ECDH_KDF_X9_63
};

/*
 * What's passed as an actual key is defined by the KEYMGMT interface.
 * We happen to know that our KEYMGMT simply passes EC_KEY structures, so
 * we use that here too.
 */

typedef struct {
    OSSL_LIB_CTX *libctx;

    EC_KEY *k;
    EC_KEY *peerk;

    /*
     * ECDH cofactor mode:
     *
     *  . 0  disabled
     *  . 1  enabled
     *  . -1 use cofactor mode set for k
     */
    int cofactor_mode;

    /************
     * ECDH KDF *
     ************/
    /* KDF (if any) to use for ECDH */
    enum kdf_type kdf_type;
    /* Message digest to use for key derivation */
    EVP_MD *kdf_md;
    /* User key material */
    unsigned char *kdf_ukm;
    size_t kdf_ukmlen;
    /* KDF output length */
    size_t kdf_outlen;
    OSSL_FIPS_IND_DECLARE
} PROV_ECDH_CTX;

static
void *ecdh_newctx(void *provctx)
{
    PROV_ECDH_CTX *pectx;

    if (!ossl_prov_is_running())
        return NULL;

    pectx = OPENSSL_zalloc(sizeof(*pectx));
    if (pectx == NULL)
        return NULL;

    pectx->libctx = PROV_LIBCTX_OF(provctx);
    pectx->cofactor_mode = -1;
    pectx->kdf_type = PROV_ECDH_KDF_NONE;
    OSSL_FIPS_IND_INIT(pectx)

    return (void *)pectx;
}

static
int ecdh_init(void *vpecdhctx, void *vecdh, const OSSL_PARAM params[])
{
    PROV_ECDH_CTX *pecdhctx = (PROV_ECDH_CTX *)vpecdhctx;

    if (!ossl_prov_is_running()
            || pecdhctx == NULL
            || vecdh == NULL
            || (EC_KEY_get0_group(vecdh) == NULL)
            || !EC_KEY_up_ref(vecdh))
        return 0;
    EC_KEY_free(pecdhctx->k);
    pecdhctx->k = vecdh;
    pecdhctx->cofactor_mode = -1;
    pecdhctx->kdf_type = PROV_ECDH_KDF_NONE;

    OSSL_FIPS_IND_SET_APPROVED(pecdhctx)
    if (!ecdh_set_ctx_params(pecdhctx, params))
        return 0;
#ifdef FIPS_MODULE
    if (!ossl_fips_ind_ec_key_check(OSSL_FIPS_IND_GET(pecdhctx),
                                    OSSL_FIPS_IND_SETTABLE0, pecdhctx->libctx,
                                    EC_KEY_get0_group(vecdh), "ECDH Init", 1))
        return 0;
#endif
    return 1;
}

static
int ecdh_match_params(const EC_KEY *priv, const EC_KEY *peer)
{
    int ret;
    BN_CTX *ctx = NULL;
    const EC_GROUP *group_priv = EC_KEY_get0_group(priv);
    const EC_GROUP *group_peer = EC_KEY_get0_group(peer);

    ctx = BN_CTX_new_ex(ossl_ec_key_get_libctx(priv));
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_BN_LIB);
        return 0;
    }
    ret = group_priv != NULL
          && group_peer != NULL
          && EC_GROUP_cmp(group_priv, group_peer, ctx) == 0;
    if (!ret)
        ERR_raise(ERR_LIB_PROV, PROV_R_MISMATCHING_DOMAIN_PARAMETERS);
    BN_CTX_free(ctx);
    return ret;
}

static
int ecdh_set_peer(void *vpecdhctx, void *vecdh)
{
    PROV_ECDH_CTX *pecdhctx = (PROV_ECDH_CTX *)vpecdhctx;

    if (!ossl_prov_is_running()
            || pecdhctx == NULL
            || vecdh == NULL
            || !ecdh_match_params(pecdhctx->k, vecdh))
        return 0;
#ifdef FIPS_MODULE
    if (!ossl_fips_ind_ec_key_check(OSSL_FIPS_IND_GET(pecdhctx),
                                    OSSL_FIPS_IND_SETTABLE0, pecdhctx->libctx,
                                    EC_KEY_get0_group(vecdh), "ECDH Set Peer",
                                    1))
        return 0;
#endif
    if (!EC_KEY_up_ref(vecdh))
        return 0;

    EC_KEY_free(pecdhctx->peerk);
    pecdhctx->peerk = vecdh;
    return 1;
}

static
void ecdh_freectx(void *vpecdhctx)
{
    PROV_ECDH_CTX *pecdhctx = (PROV_ECDH_CTX *)vpecdhctx;

    EC_KEY_free(pecdhctx->k);
    EC_KEY_free(pecdhctx->peerk);

    EVP_MD_free(pecdhctx->kdf_md);
    OPENSSL_clear_free(pecdhctx->kdf_ukm, pecdhctx->kdf_ukmlen);

    OPENSSL_free(pecdhctx);
}

static
void *ecdh_dupctx(void *vpecdhctx)
{
    PROV_ECDH_CTX *srcctx = (PROV_ECDH_CTX *)vpecdhctx;
    PROV_ECDH_CTX *dstctx;

    if (!ossl_prov_is_running())
        return NULL;

    dstctx = OPENSSL_zalloc(sizeof(*srcctx));
    if (dstctx == NULL)
        return NULL;

    *dstctx = *srcctx;

    /* clear all pointers */

    dstctx->k= NULL;
    dstctx->peerk = NULL;
    dstctx->kdf_md = NULL;
    dstctx->kdf_ukm = NULL;

    /* up-ref all ref-counted objects referenced in dstctx */

    if (srcctx->k != NULL && !EC_KEY_up_ref(srcctx->k))
        goto err;
    else
        dstctx->k = srcctx->k;

    if (srcctx->peerk != NULL && !EC_KEY_up_ref(srcctx->peerk))
        goto err;
    else
        dstctx->peerk = srcctx->peerk;

    if (srcctx->kdf_md != NULL && !EVP_MD_up_ref(srcctx->kdf_md))
        goto err;
    else
        dstctx->kdf_md = srcctx->kdf_md;

    /* Duplicate UKM data if present */
    if (srcctx->kdf_ukm != NULL && srcctx->kdf_ukmlen > 0) {
        dstctx->kdf_ukm = OPENSSL_memdup(srcctx->kdf_ukm,
                                         srcctx->kdf_ukmlen);
        if (dstctx->kdf_ukm == NULL)
            goto err;
    }

    return dstctx;

 err:
    ecdh_freectx(dstctx);
    return NULL;
}

static
int ecdh_set_ctx_params(void *vpecdhctx, const OSSL_PARAM params[])
{
    char name[80] = { '\0' }; /* should be big enough */
    char *str = NULL;
    PROV_ECDH_CTX *pectx = (PROV_ECDH_CTX *)vpecdhctx;
    const OSSL_PARAM *p;

    if (pectx == NULL)
        return 0;
    if (ossl_param_is_empty(params))
        return 1;

    if (!OSSL_FIPS_IND_SET_CTX_PARAM(pectx, OSSL_FIPS_IND_SETTABLE0, params,
                                     OSSL_EXCHANGE_PARAM_FIPS_KEY_CHECK))
        return 0;
    if (!OSSL_FIPS_IND_SET_CTX_PARAM(pectx, OSSL_FIPS_IND_SETTABLE1, params,
                                     OSSL_EXCHANGE_PARAM_FIPS_DIGEST_CHECK))
        return 0;
    if (!OSSL_FIPS_IND_SET_CTX_PARAM(pectx, OSSL_FIPS_IND_SETTABLE2, params,
                                     OSSL_EXCHANGE_PARAM_FIPS_ECDH_COFACTOR_CHECK))
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_EXCHANGE_PARAM_EC_ECDH_COFACTOR_MODE);
    if (p != NULL) {
        int mode;

        if (!OSSL_PARAM_get_int(p, &mode))
            return 0;

        if (mode < -1 || mode > 1)
            return 0;

        pectx->cofactor_mode = mode;
    }

    p = OSSL_PARAM_locate_const(params, OSSL_EXCHANGE_PARAM_KDF_TYPE);
    if (p != NULL) {
        str = name;
        if (!OSSL_PARAM_get_utf8_string(p, &str, sizeof(name)))
            return 0;

        if (name[0] == '\0')
            pectx->kdf_type = PROV_ECDH_KDF_NONE;
        else if (strcmp(name, OSSL_KDF_NAME_X963KDF) == 0)
            pectx->kdf_type = PROV_ECDH_KDF_X9_63;
        else
            return 0;
    }

    p = OSSL_PARAM_locate_const(params, OSSL_EXCHANGE_PARAM_KDF_DIGEST);
    if (p != NULL) {
        char mdprops[80] = { '\0' }; /* should be big enough */

        str = name;
        if (!OSSL_PARAM_get_utf8_string(p, &str, sizeof(name)))
            return 0;

        str = mdprops;
        p = OSSL_PARAM_locate_const(params,
                                    OSSL_EXCHANGE_PARAM_KDF_DIGEST_PROPS);

        if (p != NULL) {
            if (!OSSL_PARAM_get_utf8_string(p, &str, sizeof(mdprops)))
                return 0;
        }

        EVP_MD_free(pectx->kdf_md);
        pectx->kdf_md = EVP_MD_fetch(pectx->libctx, name, mdprops);
        if (pectx->kdf_md == NULL)
            return 0;
        /* XOF digests are not allowed */
        if (EVP_MD_xof(pectx->kdf_md)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_XOF_DIGESTS_NOT_ALLOWED);
            return 0;
        }
#ifdef FIPS_MODULE
        if (!ossl_fips_ind_digest_exch_check(OSSL_FIPS_IND_GET(pectx),
                                             OSSL_FIPS_IND_SETTABLE1, pectx->libctx,
                                             pectx->kdf_md, "ECDH Set Ctx")) {
            EVP_MD_free(pectx->kdf_md);
            pectx->kdf_md = NULL;
            return 0;
        }
#endif
    }

    p = OSSL_PARAM_locate_const(params, OSSL_EXCHANGE_PARAM_KDF_OUTLEN);
    if (p != NULL) {
        size_t outlen;

        if (!OSSL_PARAM_get_size_t(p, &outlen))
            return 0;
        pectx->kdf_outlen = outlen;
    }

    p = OSSL_PARAM_locate_const(params, OSSL_EXCHANGE_PARAM_KDF_UKM);
    if (p != NULL) {
        void *tmp_ukm = NULL;
        size_t tmp_ukmlen;

        if (!OSSL_PARAM_get_octet_string(p, &tmp_ukm, 0, &tmp_ukmlen))
            return 0;
        OPENSSL_free(pectx->kdf_ukm);
        pectx->kdf_ukm = tmp_ukm;
        pectx->kdf_ukmlen = tmp_ukmlen;
    }

    return 1;
}

static const OSSL_PARAM known_settable_ctx_params[] = {
    OSSL_PARAM_int(OSSL_EXCHANGE_PARAM_EC_ECDH_COFACTOR_MODE, NULL),
    OSSL_PARAM_utf8_string(OSSL_EXCHANGE_PARAM_KDF_TYPE, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_EXCHANGE_PARAM_KDF_DIGEST, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_EXCHANGE_PARAM_KDF_DIGEST_PROPS, NULL, 0),
    OSSL_PARAM_size_t(OSSL_EXCHANGE_PARAM_KDF_OUTLEN, NULL),
    OSSL_PARAM_octet_string(OSSL_EXCHANGE_PARAM_KDF_UKM, NULL, 0),
    OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_EXCHANGE_PARAM_FIPS_KEY_CHECK)
    OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_EXCHANGE_PARAM_FIPS_DIGEST_CHECK)
    OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_EXCHANGE_PARAM_FIPS_ECDH_COFACTOR_CHECK)
    OSSL_PARAM_END
};

static
const OSSL_PARAM *ecdh_settable_ctx_params(ossl_unused void *vpecdhctx,
                                           ossl_unused void *provctx)
{
    return known_settable_ctx_params;
}

static
int ecdh_get_ctx_params(void *vpecdhctx, OSSL_PARAM params[])
{
    PROV_ECDH_CTX *pectx = (PROV_ECDH_CTX *)vpecdhctx;
    OSSL_PARAM *p;

    if (pectx == NULL)
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_EXCHANGE_PARAM_EC_ECDH_COFACTOR_MODE);
    if (p != NULL) {
        int mode = pectx->cofactor_mode;

        if (mode == -1) {
            /* check what is the default for pecdhctx->k */
            mode = EC_KEY_get_flags(pectx->k) & EC_FLAG_COFACTOR_ECDH ? 1 : 0;
        }

        if (!OSSL_PARAM_set_int(p, mode))
            return 0;
    }

    p = OSSL_PARAM_locate(params, OSSL_EXCHANGE_PARAM_KDF_TYPE);
    if (p != NULL) {
        const char *kdf_type = NULL;

        switch (pectx->kdf_type) {
            case PROV_ECDH_KDF_NONE:
                kdf_type = "";
                break;
            case PROV_ECDH_KDF_X9_63:
                kdf_type = OSSL_KDF_NAME_X963KDF;
                break;
            default:
                return 0;
        }

        if (!OSSL_PARAM_set_utf8_string(p, kdf_type))
            return 0;
    }

    p = OSSL_PARAM_locate(params, OSSL_EXCHANGE_PARAM_KDF_DIGEST);
    if (p != NULL
            && !OSSL_PARAM_set_utf8_string(p, pectx->kdf_md == NULL
                                           ? ""
                                           : EVP_MD_get0_name(pectx->kdf_md))) {
        return 0;
    }

    p = OSSL_PARAM_locate(params, OSSL_EXCHANGE_PARAM_KDF_OUTLEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, pectx->kdf_outlen))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_EXCHANGE_PARAM_KDF_UKM);
    if (p != NULL &&
        !OSSL_PARAM_set_octet_ptr(p, pectx->kdf_ukm, pectx->kdf_ukmlen))
        return 0;
    if (!OSSL_FIPS_IND_GET_CTX_PARAM(pectx, params))
        return 0;
    return 1;
}

static const OSSL_PARAM known_gettable_ctx_params[] = {
    OSSL_PARAM_int(OSSL_EXCHANGE_PARAM_EC_ECDH_COFACTOR_MODE, NULL),
    OSSL_PARAM_utf8_string(OSSL_EXCHANGE_PARAM_KDF_TYPE, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_EXCHANGE_PARAM_KDF_DIGEST, NULL, 0),
    OSSL_PARAM_size_t(OSSL_EXCHANGE_PARAM_KDF_OUTLEN, NULL),
    OSSL_PARAM_DEFN(OSSL_EXCHANGE_PARAM_KDF_UKM, OSSL_PARAM_OCTET_PTR,
                    NULL, 0),
    OSSL_FIPS_IND_GETTABLE_CTX_PARAM()
    OSSL_PARAM_END
};

static
const OSSL_PARAM *ecdh_gettable_ctx_params(ossl_unused void *vpecdhctx,
                                           ossl_unused void *provctx)
{
    return known_gettable_ctx_params;
}

static ossl_inline
size_t ecdh_size(const EC_KEY *k)
{
    size_t degree = 0;
    const EC_GROUP *group;

    if (k == NULL
            || (group = EC_KEY_get0_group(k)) == NULL)
        return 0;

    degree = EC_GROUP_get_degree(group);

    return (degree + 7) / 8;
}

static ossl_inline
int ecdh_plain_derive(void *vpecdhctx, unsigned char *secret,
                      size_t *psecretlen, size_t outlen)
{
    PROV_ECDH_CTX *pecdhctx = (PROV_ECDH_CTX *)vpecdhctx;
    int retlen, ret = 0;
    size_t ecdhsize, size;
    const EC_POINT *ppubkey = NULL;
    EC_KEY *privk = NULL;
    const EC_GROUP *group;
    const BIGNUM *cofactor;
    int key_cofactor_mode;
    int has_cofactor;
#ifdef FIPS_MODULE
    int cofactor_approved = 0;
#endif

    if (pecdhctx->k == NULL || pecdhctx->peerk == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_KEY);
        return 0;
    }

    ecdhsize = ecdh_size(pecdhctx->k);
    if (secret == NULL) {
        *psecretlen = ecdhsize;
        return 1;
    }

    if ((group = EC_KEY_get0_group(pecdhctx->k)) == NULL
            || (cofactor = EC_GROUP_get0_cofactor(group)) == NULL)
        return 0;

    has_cofactor = !BN_is_one(cofactor);

    /*
     * NB: unlike PKCS#3 DH, if outlen is less than maximum size this is not
     * an error, the result is truncated.
     */
    size = outlen < ecdhsize ? outlen : ecdhsize;

    /*
     * The ctx->cofactor_mode flag has precedence over the
     * cofactor_mode flag set on ctx->k.
     *
     * - if ctx->cofactor_mode == -1, use ctx->k directly
     * - if ctx->cofactor_mode == key_cofactor_mode, use ctx->k directly
     * - if ctx->cofactor_mode != key_cofactor_mode:
     *     - if ctx->k->cofactor == 1, the cofactor_mode flag is irrelevant, use
     *          ctx->k directly
     *     - if ctx->k->cofactor != 1, use a duplicate of ctx->k with the flag
     *          set to ctx->cofactor_mode
     */
    key_cofactor_mode =
        (EC_KEY_get_flags(pecdhctx->k) & EC_FLAG_COFACTOR_ECDH) ? 1 : 0;
    if (pecdhctx->cofactor_mode != -1
            && pecdhctx->cofactor_mode != key_cofactor_mode
            && has_cofactor) {
        if ((privk = EC_KEY_dup(pecdhctx->k)) == NULL)
            return 0;

        if (pecdhctx->cofactor_mode == 1) {
            EC_KEY_set_flags(privk, EC_FLAG_COFACTOR_ECDH);
#ifdef FIPS_MODULE
            cofactor_approved = 1;
#endif
        } else {
            EC_KEY_clear_flags(privk, EC_FLAG_COFACTOR_ECDH);
        }
    } else {
        privk = pecdhctx->k;
#ifdef FIPS_MODULE
        cofactor_approved = key_cofactor_mode;
#endif
    }

#ifdef FIPS_MODULE
    /*
     * SP800-56A r3 Section 5.7.1.2 requires ECC Cofactor DH to be used.
     * This applies to the 'B' and 'K' curves that have cofactors that are not 1.
     */
    if (has_cofactor && !cofactor_approved) {
        if (!OSSL_FIPS_IND_ON_UNAPPROVED(pecdhctx, OSSL_FIPS_IND_SETTABLE2,
                                         pecdhctx->libctx, "ECDH", "Cofactor",
                                         ossl_fips_config_ecdh_cofactor_check)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_COFACTOR_REQUIRED);
            goto end;
        }
    }
#endif

    ppubkey = EC_KEY_get0_public_key(pecdhctx->peerk);

    retlen = ECDH_compute_key(secret, size, ppubkey, privk, NULL);

    if (retlen <= 0)
        goto end;

    *psecretlen = retlen;
    ret = 1;

 end:
    if (privk != pecdhctx->k)
        EC_KEY_free(privk);
    return ret;
}

static ossl_inline
int ecdh_X9_63_kdf_derive(void *vpecdhctx, unsigned char *secret,
                          size_t *psecretlen, size_t outlen)
{
    PROV_ECDH_CTX *pecdhctx = (PROV_ECDH_CTX *)vpecdhctx;
    unsigned char *stmp = NULL;
    size_t stmplen;
    int ret = 0;

    if (secret == NULL) {
        *psecretlen = pecdhctx->kdf_outlen;
        return 1;
    }

    if (pecdhctx->kdf_outlen > outlen) {
        ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
        return 0;
    }
    if (!ecdh_plain_derive(vpecdhctx, NULL, &stmplen, 0))
        return 0;
    if ((stmp = OPENSSL_secure_malloc(stmplen)) == NULL)
        return 0;
    if (!ecdh_plain_derive(vpecdhctx, stmp, &stmplen, stmplen))
        goto err;

    /* Do KDF stuff */
    if (!ossl_ecdh_kdf_X9_63(secret, pecdhctx->kdf_outlen,
                             stmp, stmplen,
                             pecdhctx->kdf_ukm,
                             pecdhctx->kdf_ukmlen,
                             pecdhctx->kdf_md,
                             pecdhctx->libctx, NULL))
        goto err;
    *psecretlen = pecdhctx->kdf_outlen;
    ret = 1;

 err:
    OPENSSL_secure_clear_free(stmp, stmplen);
    return ret;
}

static
int ecdh_derive(void *vpecdhctx, unsigned char *secret,
                size_t *psecretlen, size_t outlen)
{
    PROV_ECDH_CTX *pecdhctx = (PROV_ECDH_CTX *)vpecdhctx;

    switch (pecdhctx->kdf_type) {
        case PROV_ECDH_KDF_NONE:
            return ecdh_plain_derive(vpecdhctx, secret, psecretlen, outlen);
        case PROV_ECDH_KDF_X9_63:
            return ecdh_X9_63_kdf_derive(vpecdhctx, secret, psecretlen, outlen);
        default:
            break;
    }
    return 0;
}

const OSSL_DISPATCH ossl_ecdh_keyexch_functions[] = {
    { OSSL_FUNC_KEYEXCH_NEWCTX, (void (*)(void))ecdh_newctx },
    { OSSL_FUNC_KEYEXCH_INIT, (void (*)(void))ecdh_init },
    { OSSL_FUNC_KEYEXCH_DERIVE, (void (*)(void))ecdh_derive },
    { OSSL_FUNC_KEYEXCH_SET_PEER, (void (*)(void))ecdh_set_peer },
    { OSSL_FUNC_KEYEXCH_FREECTX, (void (*)(void))ecdh_freectx },
    { OSSL_FUNC_KEYEXCH_DUPCTX, (void (*)(void))ecdh_dupctx },
    { OSSL_FUNC_KEYEXCH_SET_CTX_PARAMS, (void (*)(void))ecdh_set_ctx_params },
    { OSSL_FUNC_KEYEXCH_SETTABLE_CTX_PARAMS,
      (void (*)(void))ecdh_settable_ctx_params },
    { OSSL_FUNC_KEYEXCH_GET_CTX_PARAMS, (void (*)(void))ecdh_get_ctx_params },
    { OSSL_FUNC_KEYEXCH_GETTABLE_CTX_PARAMS,
      (void (*)(void))ecdh_gettable_ctx_params },
    OSSL_DISPATCH_END
};
