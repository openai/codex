/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * DH low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <string.h>
#include <openssl/crypto.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/dh.h>
#include <openssl/err.h>
#include <openssl/proverr.h>
#include <openssl/params.h>
#include "prov/providercommon.h"
#include "prov/implementations.h"
#include "prov/provider_ctx.h"
#include "prov/securitycheck.h"
#include "crypto/dh.h"

static OSSL_FUNC_keyexch_newctx_fn dh_newctx;
static OSSL_FUNC_keyexch_init_fn dh_init;
static OSSL_FUNC_keyexch_set_peer_fn dh_set_peer;
static OSSL_FUNC_keyexch_derive_fn dh_derive;
static OSSL_FUNC_keyexch_freectx_fn dh_freectx;
static OSSL_FUNC_keyexch_dupctx_fn dh_dupctx;
static OSSL_FUNC_keyexch_set_ctx_params_fn dh_set_ctx_params;
static OSSL_FUNC_keyexch_settable_ctx_params_fn dh_settable_ctx_params;
static OSSL_FUNC_keyexch_get_ctx_params_fn dh_get_ctx_params;
static OSSL_FUNC_keyexch_gettable_ctx_params_fn dh_gettable_ctx_params;

/*
 * This type is only really used to handle some legacy related functionality.
 * If you need to use other KDF's (such as SSKDF) just use PROV_DH_KDF_NONE
 * here and then create and run a KDF after the key is derived.
 * Note that X942 has 2 variants of key derivation:
 *   (1) DH_KDF_X9_42_ASN1 - which contains an ANS1 encoded object that has
 *   the counter embedded in it.
 *   (2) DH_KDF_X941_CONCAT - which is the same as ECDH_X963_KDF (which can be
 *       done by creating a "X963KDF".
 */
enum kdf_type {
    PROV_DH_KDF_NONE = 0,
    PROV_DH_KDF_X9_42_ASN1
};

/*
 * What's passed as an actual key is defined by the KEYMGMT interface.
 * We happen to know that our KEYMGMT simply passes DH structures, so
 * we use that here too.
 */

typedef struct {
    OSSL_LIB_CTX *libctx;
    DH *dh;
    DH *dhpeer;
    unsigned int pad : 1;

    /* DH KDF */
    /* KDF (if any) to use for DH */
    enum kdf_type kdf_type;
    /* Message digest to use for key derivation */
    EVP_MD *kdf_md;
    /* User key material */
    unsigned char *kdf_ukm;
    size_t kdf_ukmlen;
    /* KDF output length */
    size_t kdf_outlen;
    char *kdf_cekalg;
    OSSL_FIPS_IND_DECLARE
} PROV_DH_CTX;

static void *dh_newctx(void *provctx)
{
    PROV_DH_CTX *pdhctx;

    if (!ossl_prov_is_running())
        return NULL;

    pdhctx = OPENSSL_zalloc(sizeof(PROV_DH_CTX));
    if (pdhctx == NULL)
        return NULL;
    OSSL_FIPS_IND_INIT(pdhctx)
    pdhctx->libctx = PROV_LIBCTX_OF(provctx);
    pdhctx->kdf_type = PROV_DH_KDF_NONE;
    return pdhctx;
}

#ifdef FIPS_MODULE
static int dh_check_key(PROV_DH_CTX *ctx)
{
    int key_approved = ossl_dh_check_key(ctx->dh);

    if (!key_approved) {
        if (!OSSL_FIPS_IND_ON_UNAPPROVED(ctx, OSSL_FIPS_IND_SETTABLE0,
                                         ctx->libctx, "DH Init", "DH Key",
                                         ossl_fips_config_securitycheck_enabled)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
            return 0;
        }
    }
    return 1;
}

static int digest_check(PROV_DH_CTX *ctx, const EVP_MD *md)
{
    return ossl_fips_ind_digest_exch_check(OSSL_FIPS_IND_GET(ctx),
                                           OSSL_FIPS_IND_SETTABLE1, ctx->libctx,
                                           md, "DH Set Ctx");
}
#endif

static int dh_init(void *vpdhctx, void *vdh, const OSSL_PARAM params[])
{
    PROV_DH_CTX *pdhctx = (PROV_DH_CTX *)vpdhctx;

    if (!ossl_prov_is_running()
            || pdhctx == NULL
            || vdh == NULL
            || !DH_up_ref(vdh))
        return 0;
    DH_free(pdhctx->dh);
    pdhctx->dh = vdh;
    pdhctx->kdf_type = PROV_DH_KDF_NONE;

    OSSL_FIPS_IND_SET_APPROVED(pdhctx)
    if (!dh_set_ctx_params(pdhctx, params))
        return 0;
#ifdef FIPS_MODULE
    if (!dh_check_key(pdhctx))
        return 0;
#endif
    return 1;
}

/* The 2 parties must share the same domain parameters */
static int dh_match_params(DH *priv, DH *peer)
{
    int ret;
    FFC_PARAMS *dhparams_priv = ossl_dh_get0_params(priv);
    FFC_PARAMS *dhparams_peer = ossl_dh_get0_params(peer);

    ret = dhparams_priv != NULL
          && dhparams_peer != NULL
          && ossl_ffc_params_cmp(dhparams_priv, dhparams_peer, 1);
    if (!ret)
        ERR_raise(ERR_LIB_PROV, PROV_R_MISMATCHING_DOMAIN_PARAMETERS);
    return ret;
}

static int dh_set_peer(void *vpdhctx, void *vdh)
{
    PROV_DH_CTX *pdhctx = (PROV_DH_CTX *)vpdhctx;

    if (!ossl_prov_is_running()
            || pdhctx == NULL
            || vdh == NULL
            || !dh_match_params(vdh, pdhctx->dh)
            || !DH_up_ref(vdh))
        return 0;
    DH_free(pdhctx->dhpeer);
    pdhctx->dhpeer = vdh;
    return 1;
}

static int dh_plain_derive(void *vpdhctx,
                           unsigned char *secret, size_t *secretlen,
                           size_t outlen, unsigned int pad)
{
    PROV_DH_CTX *pdhctx = (PROV_DH_CTX *)vpdhctx;
    int ret;
    size_t dhsize;
    const BIGNUM *pub_key = NULL;

    if (pdhctx->dh == NULL || pdhctx->dhpeer == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_KEY);
        return 0;
    }

    dhsize = (size_t)DH_size(pdhctx->dh);
    if (secret == NULL) {
        *secretlen = dhsize;
        return 1;
    }
    if (outlen < dhsize) {
        ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
        return 0;
    }

    DH_get0_key(pdhctx->dhpeer, &pub_key, NULL);
    if (pad)
        ret = DH_compute_key_padded(secret, pub_key, pdhctx->dh);
    else
        ret = DH_compute_key(secret, pub_key, pdhctx->dh);
    if (ret <= 0)
        return 0;

    *secretlen = ret;
    return 1;
}

static int dh_X9_42_kdf_derive(void *vpdhctx, unsigned char *secret,
                               size_t *secretlen, size_t outlen)
{
    PROV_DH_CTX *pdhctx = (PROV_DH_CTX *)vpdhctx;
    unsigned char *stmp = NULL;
    size_t stmplen;
    int ret = 0;

    if (secret == NULL) {
        *secretlen = pdhctx->kdf_outlen;
        return 1;
    }

    if (pdhctx->kdf_outlen > outlen) {
        ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
        return 0;
    }
    if (!dh_plain_derive(pdhctx, NULL, &stmplen, 0, 1))
        return 0;
    if ((stmp = OPENSSL_secure_malloc(stmplen)) == NULL)
        return 0;
    if (!dh_plain_derive(pdhctx, stmp, &stmplen, stmplen, 1))
        goto err;

    /* Do KDF stuff */
    if (pdhctx->kdf_type == PROV_DH_KDF_X9_42_ASN1) {
        if (!ossl_dh_kdf_X9_42_asn1(secret, pdhctx->kdf_outlen,
                                    stmp, stmplen,
                                    pdhctx->kdf_cekalg,
                                    pdhctx->kdf_ukm,
                                    pdhctx->kdf_ukmlen,
                                    pdhctx->kdf_md,
                                    pdhctx->libctx, NULL))
            goto err;
    }
    *secretlen = pdhctx->kdf_outlen;
    ret = 1;
err:
    OPENSSL_secure_clear_free(stmp, stmplen);
    return ret;
}

static int dh_derive(void *vpdhctx, unsigned char *secret,
                     size_t *psecretlen, size_t outlen)
{
    PROV_DH_CTX *pdhctx = (PROV_DH_CTX *)vpdhctx;

    if (!ossl_prov_is_running())
        return 0;

    switch (pdhctx->kdf_type) {
        case PROV_DH_KDF_NONE:
            return dh_plain_derive(pdhctx, secret, psecretlen, outlen,
                                   pdhctx->pad);
        case PROV_DH_KDF_X9_42_ASN1:
            return dh_X9_42_kdf_derive(pdhctx, secret, psecretlen, outlen);
        default:
            break;
    }
    return 0;
}

static void dh_freectx(void *vpdhctx)
{
    PROV_DH_CTX *pdhctx = (PROV_DH_CTX *)vpdhctx;

    OPENSSL_free(pdhctx->kdf_cekalg);
    DH_free(pdhctx->dh);
    DH_free(pdhctx->dhpeer);
    EVP_MD_free(pdhctx->kdf_md);
    OPENSSL_clear_free(pdhctx->kdf_ukm, pdhctx->kdf_ukmlen);

    OPENSSL_free(pdhctx);
}

static void *dh_dupctx(void *vpdhctx)
{
    PROV_DH_CTX *srcctx = (PROV_DH_CTX *)vpdhctx;
    PROV_DH_CTX *dstctx;

    if (!ossl_prov_is_running())
        return NULL;

    dstctx = OPENSSL_zalloc(sizeof(*srcctx));
    if (dstctx == NULL)
        return NULL;

    *dstctx = *srcctx;
    dstctx->dh = NULL;
    dstctx->dhpeer = NULL;
    dstctx->kdf_md = NULL;
    dstctx->kdf_ukm = NULL;
    dstctx->kdf_cekalg = NULL;

    if (srcctx->dh != NULL && !DH_up_ref(srcctx->dh))
        goto err;
    else
        dstctx->dh = srcctx->dh;

    if (srcctx->dhpeer != NULL && !DH_up_ref(srcctx->dhpeer))
        goto err;
    else
        dstctx->dhpeer = srcctx->dhpeer;

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

    if (srcctx->kdf_cekalg != NULL) {
        dstctx->kdf_cekalg = OPENSSL_strdup(srcctx->kdf_cekalg);
        if (dstctx->kdf_cekalg == NULL)
            goto err;
    }

    return dstctx;
err:
    dh_freectx(dstctx);
    return NULL;
}

static int dh_set_ctx_params(void *vpdhctx, const OSSL_PARAM params[])
{
    PROV_DH_CTX *pdhctx = (PROV_DH_CTX *)vpdhctx;
    const OSSL_PARAM *p;
    unsigned int pad;
    char name[80] = { '\0' }; /* should be big enough */
    char *str = NULL;

    if (pdhctx == NULL)
        return 0;
    if (ossl_param_is_empty(params))
        return 1;

    if (!OSSL_FIPS_IND_SET_CTX_PARAM(pdhctx, OSSL_FIPS_IND_SETTABLE0, params,
                                     OSSL_EXCHANGE_PARAM_FIPS_KEY_CHECK))
        return  0;
    if (!OSSL_FIPS_IND_SET_CTX_PARAM(pdhctx, OSSL_FIPS_IND_SETTABLE1, params,
                                     OSSL_EXCHANGE_PARAM_FIPS_DIGEST_CHECK))
        return  0;

    p = OSSL_PARAM_locate_const(params, OSSL_EXCHANGE_PARAM_KDF_TYPE);
    if (p != NULL) {
        str = name;
        if (!OSSL_PARAM_get_utf8_string(p, &str, sizeof(name)))
            return 0;

        if (name[0] == '\0')
            pdhctx->kdf_type = PROV_DH_KDF_NONE;
        else if (strcmp(name, OSSL_KDF_NAME_X942KDF_ASN1) == 0)
            pdhctx->kdf_type = PROV_DH_KDF_X9_42_ASN1;
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

        EVP_MD_free(pdhctx->kdf_md);
        pdhctx->kdf_md = EVP_MD_fetch(pdhctx->libctx, name, mdprops);
        if (pdhctx->kdf_md == NULL)
            return 0;
        /* XOF digests are not allowed */
        if (EVP_MD_xof(pdhctx->kdf_md)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_XOF_DIGESTS_NOT_ALLOWED);
            return 0;
        }
#ifdef FIPS_MODULE
        if (!digest_check(pdhctx, pdhctx->kdf_md)) {
            EVP_MD_free(pdhctx->kdf_md);
            pdhctx->kdf_md = NULL;
            return 0;
        }
#endif
    }

    p = OSSL_PARAM_locate_const(params, OSSL_EXCHANGE_PARAM_KDF_OUTLEN);
    if (p != NULL) {
        size_t outlen;

        if (!OSSL_PARAM_get_size_t(p, &outlen))
            return 0;
        pdhctx->kdf_outlen = outlen;
    }

    p = OSSL_PARAM_locate_const(params, OSSL_EXCHANGE_PARAM_KDF_UKM);
    if (p != NULL) {
        void *tmp_ukm = NULL;
        size_t tmp_ukmlen;

        OPENSSL_free(pdhctx->kdf_ukm);
        pdhctx->kdf_ukm = NULL;
        pdhctx->kdf_ukmlen = 0;
        /* ukm is an optional field so it can be NULL */
        if (p->data != NULL && p->data_size != 0) {
            if (!OSSL_PARAM_get_octet_string(p, &tmp_ukm, 0, &tmp_ukmlen))
                return 0;
            pdhctx->kdf_ukm = tmp_ukm;
            pdhctx->kdf_ukmlen = tmp_ukmlen;
        }
    }

    p = OSSL_PARAM_locate_const(params, OSSL_EXCHANGE_PARAM_PAD);
    if (p != NULL) {
        if (!OSSL_PARAM_get_uint(p, &pad))
            return 0;
        pdhctx->pad = pad ? 1 : 0;
    }

    p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_CEK_ALG);
    if (p != NULL) {
        str = name;

        OPENSSL_free(pdhctx->kdf_cekalg);
        pdhctx->kdf_cekalg = NULL;
        if (p->data != NULL && p->data_size != 0) {
            if (!OSSL_PARAM_get_utf8_string(p, &str, sizeof(name)))
                return 0;
            pdhctx->kdf_cekalg = OPENSSL_strdup(name);
            if (pdhctx->kdf_cekalg == NULL)
                return 0;
        }
    }
    return 1;
}

static const OSSL_PARAM known_settable_ctx_params[] = {
    OSSL_PARAM_int(OSSL_EXCHANGE_PARAM_PAD, NULL),
    OSSL_PARAM_utf8_string(OSSL_EXCHANGE_PARAM_KDF_TYPE, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_EXCHANGE_PARAM_KDF_DIGEST, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_EXCHANGE_PARAM_KDF_DIGEST_PROPS, NULL, 0),
    OSSL_PARAM_size_t(OSSL_EXCHANGE_PARAM_KDF_OUTLEN, NULL),
    OSSL_PARAM_octet_string(OSSL_EXCHANGE_PARAM_KDF_UKM, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_KDF_PARAM_CEK_ALG, NULL, 0),
    OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_EXCHANGE_PARAM_FIPS_KEY_CHECK)
    OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_EXCHANGE_PARAM_FIPS_DIGEST_CHECK)
    OSSL_PARAM_END
};

static const OSSL_PARAM *dh_settable_ctx_params(ossl_unused void *vpdhctx,
                                                ossl_unused void *provctx)
{
    return known_settable_ctx_params;
}

static const OSSL_PARAM known_gettable_ctx_params[] = {
    OSSL_PARAM_utf8_string(OSSL_EXCHANGE_PARAM_KDF_TYPE, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_EXCHANGE_PARAM_KDF_DIGEST, NULL, 0),
    OSSL_PARAM_size_t(OSSL_EXCHANGE_PARAM_KDF_OUTLEN, NULL),
    OSSL_PARAM_DEFN(OSSL_EXCHANGE_PARAM_KDF_UKM, OSSL_PARAM_OCTET_PTR,
                    NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_KDF_PARAM_CEK_ALG, NULL, 0),
    OSSL_FIPS_IND_GETTABLE_CTX_PARAM()
    OSSL_PARAM_END
};

static const OSSL_PARAM *dh_gettable_ctx_params(ossl_unused void *vpdhctx,
                                                ossl_unused void *provctx)
{
    return known_gettable_ctx_params;
}

static int dh_get_ctx_params(void *vpdhctx, OSSL_PARAM params[])
{
    PROV_DH_CTX *pdhctx = (PROV_DH_CTX *)vpdhctx;
    OSSL_PARAM *p;

    if (pdhctx == NULL)
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_EXCHANGE_PARAM_KDF_TYPE);
    if (p != NULL) {
        const char *kdf_type = NULL;

        switch (pdhctx->kdf_type) {
            case PROV_DH_KDF_NONE:
                kdf_type = "";
                break;
            case PROV_DH_KDF_X9_42_ASN1:
                kdf_type = OSSL_KDF_NAME_X942KDF_ASN1;
                break;
            default:
                return 0;
        }

        if (!OSSL_PARAM_set_utf8_string(p, kdf_type))
            return 0;
    }

    p = OSSL_PARAM_locate(params, OSSL_EXCHANGE_PARAM_KDF_DIGEST);
    if (p != NULL
            && !OSSL_PARAM_set_utf8_string(p, pdhctx->kdf_md == NULL
                                           ? ""
                                           : EVP_MD_get0_name(pdhctx->kdf_md))) {
        return 0;
    }

    p = OSSL_PARAM_locate(params, OSSL_EXCHANGE_PARAM_KDF_OUTLEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, pdhctx->kdf_outlen))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_EXCHANGE_PARAM_KDF_UKM);
    if (p != NULL
        && !OSSL_PARAM_set_octet_ptr(p, pdhctx->kdf_ukm, pdhctx->kdf_ukmlen))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_KDF_PARAM_CEK_ALG);
    if (p != NULL
            && !OSSL_PARAM_set_utf8_string(p, pdhctx->kdf_cekalg == NULL
                                           ? "" :  pdhctx->kdf_cekalg))
        return 0;
    if (!OSSL_FIPS_IND_GET_CTX_PARAM(pdhctx, params))
        return 0;
    return 1;
}

const OSSL_DISPATCH ossl_dh_keyexch_functions[] = {
    { OSSL_FUNC_KEYEXCH_NEWCTX, (void (*)(void))dh_newctx },
    { OSSL_FUNC_KEYEXCH_INIT, (void (*)(void))dh_init },
    { OSSL_FUNC_KEYEXCH_DERIVE, (void (*)(void))dh_derive },
    { OSSL_FUNC_KEYEXCH_SET_PEER, (void (*)(void))dh_set_peer },
    { OSSL_FUNC_KEYEXCH_FREECTX, (void (*)(void))dh_freectx },
    { OSSL_FUNC_KEYEXCH_DUPCTX, (void (*)(void))dh_dupctx },
    { OSSL_FUNC_KEYEXCH_SET_CTX_PARAMS, (void (*)(void))dh_set_ctx_params },
    { OSSL_FUNC_KEYEXCH_SETTABLE_CTX_PARAMS,
      (void (*)(void))dh_settable_ctx_params },
    { OSSL_FUNC_KEYEXCH_GET_CTX_PARAMS, (void (*)(void))dh_get_ctx_params },
    { OSSL_FUNC_KEYEXCH_GETTABLE_CTX_PARAMS,
      (void (*)(void))dh_gettable_ctx_params },
    OSSL_DISPATCH_END
};
