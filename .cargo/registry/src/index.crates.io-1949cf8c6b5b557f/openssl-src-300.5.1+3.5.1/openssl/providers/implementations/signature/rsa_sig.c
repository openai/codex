/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * RSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <string.h>
#include <openssl/crypto.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/err.h>
#include <openssl/obj_mac.h>
#include <openssl/rsa.h>
#include <openssl/params.h>
#include <openssl/evp.h>
#include <openssl/proverr.h>
#include "internal/cryptlib.h"
#include "internal/nelem.h"
#include "internal/sizes.h"
#include "crypto/rsa.h"
#include "prov/providercommon.h"
#include "prov/implementations.h"
#include "prov/provider_ctx.h"
#include "prov/der_rsa.h"
#include "prov/securitycheck.h"

#define RSA_DEFAULT_DIGEST_NAME OSSL_DIGEST_NAME_SHA1

static OSSL_FUNC_signature_newctx_fn rsa_newctx;
static OSSL_FUNC_signature_sign_init_fn rsa_sign_init;
static OSSL_FUNC_signature_verify_init_fn rsa_verify_init;
static OSSL_FUNC_signature_verify_recover_init_fn rsa_verify_recover_init;
static OSSL_FUNC_signature_sign_fn rsa_sign;
static OSSL_FUNC_signature_sign_message_update_fn rsa_signverify_message_update;
static OSSL_FUNC_signature_sign_message_final_fn rsa_sign_message_final;
static OSSL_FUNC_signature_verify_fn rsa_verify;
static OSSL_FUNC_signature_verify_recover_fn rsa_verify_recover;
static OSSL_FUNC_signature_verify_message_update_fn rsa_signverify_message_update;
static OSSL_FUNC_signature_verify_message_final_fn rsa_verify_message_final;
static OSSL_FUNC_signature_digest_sign_init_fn rsa_digest_sign_init;
static OSSL_FUNC_signature_digest_sign_update_fn rsa_digest_sign_update;
static OSSL_FUNC_signature_digest_sign_final_fn rsa_digest_sign_final;
static OSSL_FUNC_signature_digest_verify_init_fn rsa_digest_verify_init;
static OSSL_FUNC_signature_digest_verify_update_fn rsa_digest_verify_update;
static OSSL_FUNC_signature_digest_verify_final_fn rsa_digest_verify_final;
static OSSL_FUNC_signature_freectx_fn rsa_freectx;
static OSSL_FUNC_signature_dupctx_fn rsa_dupctx;
static OSSL_FUNC_signature_query_key_types_fn rsa_sigalg_query_key_types;
static OSSL_FUNC_signature_get_ctx_params_fn rsa_get_ctx_params;
static OSSL_FUNC_signature_gettable_ctx_params_fn rsa_gettable_ctx_params;
static OSSL_FUNC_signature_set_ctx_params_fn rsa_set_ctx_params;
static OSSL_FUNC_signature_settable_ctx_params_fn rsa_settable_ctx_params;
static OSSL_FUNC_signature_get_ctx_md_params_fn rsa_get_ctx_md_params;
static OSSL_FUNC_signature_gettable_ctx_md_params_fn rsa_gettable_ctx_md_params;
static OSSL_FUNC_signature_set_ctx_md_params_fn rsa_set_ctx_md_params;
static OSSL_FUNC_signature_settable_ctx_md_params_fn rsa_settable_ctx_md_params;
static OSSL_FUNC_signature_set_ctx_params_fn rsa_sigalg_set_ctx_params;
static OSSL_FUNC_signature_settable_ctx_params_fn rsa_sigalg_settable_ctx_params;

static OSSL_ITEM padding_item[] = {
    { RSA_PKCS1_PADDING,        OSSL_PKEY_RSA_PAD_MODE_PKCSV15 },
    { RSA_NO_PADDING,           OSSL_PKEY_RSA_PAD_MODE_NONE },
    { RSA_X931_PADDING,         OSSL_PKEY_RSA_PAD_MODE_X931 },
    { RSA_PKCS1_PSS_PADDING,    OSSL_PKEY_RSA_PAD_MODE_PSS },
    { 0,                        NULL     }
};

/*
 * What's passed as an actual key is defined by the KEYMGMT interface.
 * We happen to know that our KEYMGMT simply passes RSA structures, so
 * we use that here too.
 */

typedef struct {
    OSSL_LIB_CTX *libctx;
    char *propq;
    RSA *rsa;
    int operation;

    /*
     * Flag to determine if a full sigalg is run (1) or if a composable
     * signature algorithm is run (0).
     *
     * When a full sigalg is run (1), this currently affects the following
     * other flags, which are to remain untouched after their initialization:
     *
     * - flag_allow_md (initialized to 0)
     */
    unsigned int flag_sigalg : 1;
    /*
     * Flag to determine if the hash function can be changed (1) or not (0)
     * Because it's dangerous to change during a DigestSign or DigestVerify
     * operation, this flag is cleared by their Init function, and set again
     * by their Final function.
     * Implementations of full sigalgs (such as RSA-SHA256) hard-code this
     * flag to not allow changes (0).
     */
    unsigned int flag_allow_md : 1;
    unsigned int mgf1_md_set : 1;
    /*
     * Flags to say what are the possible next external calls in what
     * consitutes the life cycle of an algorithm.  The relevant calls are:
     * - init
     * - update
     * - final
     * - oneshot
     * All other external calls are regarded as utilitarian and are allowed
     * at any time (they may be affected by other flags, like flag_allow_md,
     * though).
     */
    unsigned int flag_allow_update : 1;
    unsigned int flag_allow_final : 1;
    unsigned int flag_allow_oneshot : 1;

    /* main digest */
    EVP_MD *md;
    EVP_MD_CTX *mdctx;
    int mdnid;
    char mdname[OSSL_MAX_NAME_SIZE]; /* Purely informational */

    /* RSA padding mode */
    int pad_mode;
    /* message digest for MGF1 */
    EVP_MD *mgf1_md;
    int mgf1_mdnid;
    char mgf1_mdname[OSSL_MAX_NAME_SIZE]; /* Purely informational */
    /* PSS salt length */
    int saltlen;
    /* Minimum salt length or -1 if no PSS parameter restriction */
    int min_saltlen;

    /* Signature, for verification */
    unsigned char *sig;
    size_t siglen;

#ifdef FIPS_MODULE
    /*
     * FIPS 140-3 IG 2.4.B mandates that verification based on a digest of a
     * message is not permitted.  However, signing based on a digest is still
     * permitted.
     */
    int verify_message;
#endif

    /* Temp buffer */
    unsigned char *tbuf;

    OSSL_FIPS_IND_DECLARE
} PROV_RSA_CTX;

/* True if PSS parameters are restricted */
#define rsa_pss_restricted(prsactx) (prsactx->min_saltlen != -1)

static size_t rsa_get_md_size(const PROV_RSA_CTX *prsactx)
{
    int md_size;

    if (prsactx->md != NULL) {
        md_size = EVP_MD_get_size(prsactx->md);
        if (md_size <= 0)
            return 0;
        return md_size;
    }
    return 0;
}

static int rsa_check_padding(const PROV_RSA_CTX *prsactx,
                             const char *mdname, const char *mgf1_mdname,
                             int mdnid)
{
    switch (prsactx->pad_mode) {
    case RSA_NO_PADDING:
        if (mdname != NULL || mdnid != NID_undef) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_PADDING_MODE);
            return 0;
        }
        break;
    case RSA_X931_PADDING:
        if (RSA_X931_hash_id(mdnid) == -1) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_X931_DIGEST);
            return 0;
        }
        break;
    case RSA_PKCS1_PSS_PADDING:
        if (rsa_pss_restricted(prsactx))
            if ((mdname != NULL && !EVP_MD_is_a(prsactx->md, mdname))
                || (mgf1_mdname != NULL
                    && !EVP_MD_is_a(prsactx->mgf1_md, mgf1_mdname))) {
                ERR_raise(ERR_LIB_PROV, PROV_R_DIGEST_NOT_ALLOWED);
                return 0;
            }
        break;
    default:
        break;
    }

    return 1;
}

static int rsa_check_parameters(PROV_RSA_CTX *prsactx, int min_saltlen)
{
    if (prsactx->pad_mode == RSA_PKCS1_PSS_PADDING) {
        int max_saltlen;

        /* See if minimum salt length exceeds maximum possible */
        max_saltlen = RSA_size(prsactx->rsa) - EVP_MD_get_size(prsactx->md);
        if ((RSA_bits(prsactx->rsa) & 0x7) == 1)
            max_saltlen--;
        if (min_saltlen < 0 || min_saltlen > max_saltlen) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_SALT_LENGTH);
            return 0;
        }
        prsactx->min_saltlen = min_saltlen;
    }
    return 1;
}

static void *rsa_newctx(void *provctx, const char *propq)
{
    PROV_RSA_CTX *prsactx = NULL;
    char *propq_copy = NULL;

    if (!ossl_prov_is_running())
        return NULL;

    if ((prsactx = OPENSSL_zalloc(sizeof(PROV_RSA_CTX))) == NULL
        || (propq != NULL
            && (propq_copy = OPENSSL_strdup(propq)) == NULL)) {
        OPENSSL_free(prsactx);
        return NULL;
    }

    OSSL_FIPS_IND_INIT(prsactx)
    prsactx->libctx = PROV_LIBCTX_OF(provctx);
    prsactx->flag_allow_md = 1;
#ifdef FIPS_MODULE
    prsactx->verify_message = 1;
#endif
    prsactx->propq = propq_copy;
    /* Maximum up to digest length for sign, auto for verify */
    prsactx->saltlen = RSA_PSS_SALTLEN_AUTO_DIGEST_MAX;
    prsactx->min_saltlen = -1;
    return prsactx;
}

static int rsa_pss_compute_saltlen(PROV_RSA_CTX *ctx)
{
    int saltlen = ctx->saltlen;
    int saltlenMax = -1;

    /* FIPS 186-4 section 5 "The RSA Digital Signature Algorithm", subsection
     * 5.5 "PKCS #1" says: "For RSASSA-PSS […] the length (in bytes) of the
     * salt (sLen) shall satisfy 0 <= sLen <= hLen, where hLen is the length of
     * the hash function output block (in bytes)."
     *
     * Provide a way to use at most the digest length, so that the default does
     * not violate FIPS 186-4. */
    if (saltlen == RSA_PSS_SALTLEN_DIGEST) {
        if ((saltlen = EVP_MD_get_size(ctx->md)) <= 0) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_DIGEST);
            return -1;
        }
    } else if (saltlen == RSA_PSS_SALTLEN_AUTO_DIGEST_MAX) {
        saltlen = RSA_PSS_SALTLEN_MAX;
        if ((saltlenMax = EVP_MD_get_size(ctx->md)) <= 0) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_DIGEST);
            return -1;
        }
    }
    if (saltlen == RSA_PSS_SALTLEN_MAX || saltlen == RSA_PSS_SALTLEN_AUTO) {
        int mdsize, rsasize;

        if ((mdsize = EVP_MD_get_size(ctx->md)) <= 0) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_DIGEST);
            return -1;
        }
        if ((rsasize = RSA_size(ctx->rsa)) <= 2 || rsasize - 2 < mdsize) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY);
            return -1;
        }
        saltlen = rsasize - mdsize - 2;
        if ((RSA_bits(ctx->rsa) & 0x7) == 1)
            saltlen--;
        if (saltlenMax >= 0 && saltlen > saltlenMax)
            saltlen = saltlenMax;
    }
    if (saltlen < 0) {
        ERR_raise(ERR_LIB_PROV, ERR_R_INTERNAL_ERROR);
        return -1;
    } else if (saltlen < ctx->min_saltlen) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_PSS_SALTLEN_TOO_SMALL,
                       "minimum salt length: %d, actual salt length: %d",
                       ctx->min_saltlen, saltlen);
        return -1;
    }
    return saltlen;
}

static unsigned char *rsa_generate_signature_aid(PROV_RSA_CTX *ctx,
                                                 unsigned char *aid_buf,
                                                 size_t buf_len,
                                                 size_t *aid_len)
{
    WPACKET pkt;
    unsigned char *aid = NULL;
    int saltlen;
    RSA_PSS_PARAMS_30 pss_params;
    int ret;

    if (!WPACKET_init_der(&pkt, aid_buf, buf_len)) {
        ERR_raise(ERR_LIB_PROV, ERR_R_CRYPTO_LIB);
        return NULL;
    }

    switch (ctx->pad_mode) {
    case RSA_PKCS1_PADDING:
        ret = ossl_DER_w_algorithmIdentifier_MDWithRSAEncryption(&pkt, -1,
                                                                 ctx->mdnid);

        if (ret > 0) {
            break;
        } else if (ret == 0) {
            ERR_raise(ERR_LIB_PROV, ERR_R_INTERNAL_ERROR);
            goto cleanup;
        }
        ERR_raise_data(ERR_LIB_PROV, ERR_R_UNSUPPORTED,
                       "Algorithm ID generation - md NID: %d",
                       ctx->mdnid);
        goto cleanup;
    case RSA_PKCS1_PSS_PADDING:
        saltlen = rsa_pss_compute_saltlen(ctx);
        if (saltlen < 0)
            goto cleanup;
        if (!ossl_rsa_pss_params_30_set_defaults(&pss_params)
            || !ossl_rsa_pss_params_30_set_hashalg(&pss_params, ctx->mdnid)
            || !ossl_rsa_pss_params_30_set_maskgenhashalg(&pss_params,
                                                          ctx->mgf1_mdnid)
            || !ossl_rsa_pss_params_30_set_saltlen(&pss_params, saltlen)
            || !ossl_DER_w_algorithmIdentifier_RSA_PSS(&pkt, -1,
                                                       RSA_FLAG_TYPE_RSASSAPSS,
                                                       &pss_params)) {
            ERR_raise(ERR_LIB_PROV, ERR_R_INTERNAL_ERROR);
            goto cleanup;
        }
        break;
    default:
        ERR_raise_data(ERR_LIB_PROV, ERR_R_UNSUPPORTED,
                       "Algorithm ID generation - pad mode: %d",
                       ctx->pad_mode);
        goto cleanup;
    }
    if (WPACKET_finish(&pkt)) {
        WPACKET_get_total_written(&pkt, aid_len);
        aid = WPACKET_get_curr(&pkt);
    }
 cleanup:
    WPACKET_cleanup(&pkt);
    return aid;
}

static int rsa_setup_md(PROV_RSA_CTX *ctx, const char *mdname,
                        const char *mdprops, const char *desc)
{
    EVP_MD *md = NULL;

    if (mdprops == NULL)
        mdprops = ctx->propq;

    if (mdname != NULL) {
        int md_nid;
        size_t mdname_len = strlen(mdname);

        md = EVP_MD_fetch(ctx->libctx, mdname, mdprops);

        if (md == NULL) {
            ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_DIGEST,
                           "%s could not be fetched", mdname);
            goto err;
        }
        md_nid = ossl_digest_rsa_sign_get_md_nid(md);
        if (md_nid == NID_undef) {
            ERR_raise_data(ERR_LIB_PROV, PROV_R_DIGEST_NOT_ALLOWED,
                           "digest=%s", mdname);
            goto err;
        }
        /*
         * XOF digests are not allowed except for RSA PSS.
         * We don't support XOF digests with RSA PSS (yet), so just fail.
         * When we do support them, uncomment the second clause.
         */
        if (EVP_MD_xof(md)
                /* && ctx->pad_mode != RSA_PKCS1_PSS_PADDING */) {
            ERR_raise(ERR_LIB_PROV, PROV_R_XOF_DIGESTS_NOT_ALLOWED);
            goto err;
        }
#ifdef FIPS_MODULE
        {
            int sha1_allowed
                = ((ctx->operation
                    & (EVP_PKEY_OP_SIGN | EVP_PKEY_OP_SIGNMSG)) == 0);

            if (!ossl_fips_ind_digest_sign_check(OSSL_FIPS_IND_GET(ctx),
                                                 OSSL_FIPS_IND_SETTABLE1,
                                                 ctx->libctx,
                                                 md_nid, sha1_allowed, desc,
                                                 ossl_fips_config_signature_digest_check))
                goto err;
        }
#endif

        if (!rsa_check_padding(ctx, mdname, NULL, md_nid))
            goto err;
        if (mdname_len >= sizeof(ctx->mdname)) {
            ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_DIGEST,
                           "%s exceeds name buffer length", mdname);
            goto err;
        }

        if (!ctx->flag_allow_md) {
            if (ctx->mdname[0] != '\0' && !EVP_MD_is_a(md, ctx->mdname)) {
                ERR_raise_data(ERR_LIB_PROV, PROV_R_DIGEST_NOT_ALLOWED,
                               "digest %s != %s", mdname, ctx->mdname);
                goto err;
            }
            EVP_MD_free(md);
            return 1;
        }

        if (!ctx->mgf1_md_set) {
            if (!EVP_MD_up_ref(md)) {
                goto err;
            }
            EVP_MD_free(ctx->mgf1_md);
            ctx->mgf1_md = md;
            ctx->mgf1_mdnid = md_nid;
            OPENSSL_strlcpy(ctx->mgf1_mdname, mdname, sizeof(ctx->mgf1_mdname));
        }

        EVP_MD_CTX_free(ctx->mdctx);
        EVP_MD_free(ctx->md);

        ctx->mdctx = NULL;
        ctx->md = md;
        ctx->mdnid = md_nid;
        OPENSSL_strlcpy(ctx->mdname, mdname, sizeof(ctx->mdname));
    }

    return 1;
err:
    EVP_MD_free(md);
    return 0;
}

static int rsa_setup_mgf1_md(PROV_RSA_CTX *ctx, const char *mdname,
                             const char *mdprops)
{
    size_t len;
    EVP_MD *md = NULL;
    int mdnid;

    if (mdprops == NULL)
        mdprops = ctx->propq;

    if ((md = EVP_MD_fetch(ctx->libctx, mdname, mdprops)) == NULL) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_DIGEST,
                       "%s could not be fetched", mdname);
        return 0;
    }
    /* The default for mgf1 is SHA1 - so allow SHA1 */
    if ((mdnid = ossl_digest_rsa_sign_get_md_nid(md)) <= 0
        || !rsa_check_padding(ctx, NULL, mdname, mdnid)) {
        if (mdnid <= 0)
            ERR_raise_data(ERR_LIB_PROV, PROV_R_DIGEST_NOT_ALLOWED,
                           "digest=%s", mdname);
        EVP_MD_free(md);
        return 0;
    }
    len = OPENSSL_strlcpy(ctx->mgf1_mdname, mdname, sizeof(ctx->mgf1_mdname));
    if (len >= sizeof(ctx->mgf1_mdname)) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_DIGEST,
                       "%s exceeds name buffer length", mdname);
        EVP_MD_free(md);
        return 0;
    }

    EVP_MD_free(ctx->mgf1_md);
    ctx->mgf1_md = md;
    ctx->mgf1_mdnid = mdnid;
    ctx->mgf1_md_set = 1;
    return 1;
}

static int
rsa_signverify_init(PROV_RSA_CTX *prsactx, void *vrsa,
                    OSSL_FUNC_signature_set_ctx_params_fn *set_ctx_params,
                    const OSSL_PARAM params[], int operation,
                    const char *desc)
{
    int protect;

    if (!ossl_prov_is_running() || prsactx == NULL)
        return 0;

    if (vrsa == NULL && prsactx->rsa == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_NO_KEY_SET);
        return 0;
    }

    if (vrsa != NULL) {
        if (!RSA_up_ref(vrsa))
            return 0;
        RSA_free(prsactx->rsa);
        prsactx->rsa = vrsa;
    }
    if (!ossl_rsa_key_op_get_protect(prsactx->rsa, operation, &protect))
        return 0;

    prsactx->operation = operation;
    prsactx->flag_allow_update = 1;
    prsactx->flag_allow_final = 1;
    prsactx->flag_allow_oneshot = 1;

    /* Maximize up to digest length for sign, auto for verify */
    prsactx->saltlen = RSA_PSS_SALTLEN_AUTO_DIGEST_MAX;
    prsactx->min_saltlen = -1;

    switch (RSA_test_flags(prsactx->rsa, RSA_FLAG_TYPE_MASK)) {
    case RSA_FLAG_TYPE_RSA:
        prsactx->pad_mode = RSA_PKCS1_PADDING;
        break;
    case RSA_FLAG_TYPE_RSASSAPSS:
        prsactx->pad_mode = RSA_PKCS1_PSS_PADDING;

        {
            const RSA_PSS_PARAMS_30 *pss =
                ossl_rsa_get0_pss_params_30(prsactx->rsa);

            if (!ossl_rsa_pss_params_30_is_unrestricted(pss)) {
                int md_nid = ossl_rsa_pss_params_30_hashalg(pss);
                int mgf1md_nid = ossl_rsa_pss_params_30_maskgenhashalg(pss);
                int min_saltlen = ossl_rsa_pss_params_30_saltlen(pss);
                const char *mdname, *mgf1mdname;
                size_t len;

                mdname = ossl_rsa_oaeppss_nid2name(md_nid);
                mgf1mdname = ossl_rsa_oaeppss_nid2name(mgf1md_nid);

                if (mdname == NULL) {
                    ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_DIGEST,
                                   "PSS restrictions lack hash algorithm");
                    return 0;
                }
                if (mgf1mdname == NULL) {
                    ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_DIGEST,
                                   "PSS restrictions lack MGF1 hash algorithm");
                    return 0;
                }

                len = OPENSSL_strlcpy(prsactx->mdname, mdname,
                                      sizeof(prsactx->mdname));
                if (len >= sizeof(prsactx->mdname)) {
                    ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_DIGEST,
                                   "hash algorithm name too long");
                    return 0;
                }
                len = OPENSSL_strlcpy(prsactx->mgf1_mdname, mgf1mdname,
                                      sizeof(prsactx->mgf1_mdname));
                if (len >= sizeof(prsactx->mgf1_mdname)) {
                    ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_DIGEST,
                                   "MGF1 hash algorithm name too long");
                    return 0;
                }
                prsactx->saltlen = min_saltlen;

                /* call rsa_setup_mgf1_md before rsa_setup_md to avoid duplication */
                if (!rsa_setup_mgf1_md(prsactx, mgf1mdname, prsactx->propq)
                    || !rsa_setup_md(prsactx, mdname, prsactx->propq, desc)
                    || !rsa_check_parameters(prsactx, min_saltlen))
                    return 0;
            }
        }

        break;
    default:
        ERR_raise(ERR_LIB_RSA, PROV_R_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE);
        return 0;
    }

    OSSL_FIPS_IND_SET_APPROVED(prsactx)
    if (!set_ctx_params(prsactx, params))
        return 0;
#ifdef FIPS_MODULE
    if (!ossl_fips_ind_rsa_key_check(OSSL_FIPS_IND_GET(prsactx),
                                     OSSL_FIPS_IND_SETTABLE0, prsactx->libctx,
                                     prsactx->rsa, desc, protect))
        return 0;
#endif
    return 1;
}

static int setup_tbuf(PROV_RSA_CTX *ctx)
{
    if (ctx->tbuf != NULL)
        return 1;
    if ((ctx->tbuf = OPENSSL_malloc(RSA_size(ctx->rsa))) == NULL)
        return 0;
    return 1;
}

static void clean_tbuf(PROV_RSA_CTX *ctx)
{
    if (ctx->tbuf != NULL)
        OPENSSL_cleanse(ctx->tbuf, RSA_size(ctx->rsa));
}

static void free_tbuf(PROV_RSA_CTX *ctx)
{
    clean_tbuf(ctx);
    OPENSSL_free(ctx->tbuf);
    ctx->tbuf = NULL;
}

#ifdef FIPS_MODULE
static int rsa_pss_saltlen_check_passed(PROV_RSA_CTX *ctx, const char *algoname, int saltlen)
{
    int mdsize = rsa_get_md_size(ctx);
    /*
     * Perform the check if the salt length is compliant to FIPS 186-5.
     *
     * According to FIPS 186-5 5.4 (g), the salt length shall be between zero
     * and the output block length of the digest function (inclusive).
     */
    int approved = (saltlen >= 0 && saltlen <= mdsize);

    if (!approved) {
        if (!OSSL_FIPS_IND_ON_UNAPPROVED(ctx, OSSL_FIPS_IND_SETTABLE3,
                                         ctx->libctx,
                                         algoname, "PSS Salt Length",
                                         ossl_fips_config_rsa_pss_saltlen_check)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_SALT_LENGTH);
            return 0;
        }
    }

    return 1;
}
#endif

static int rsa_sign_init(void *vprsactx, void *vrsa, const OSSL_PARAM params[])
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;

#ifdef FIPS_MODULE
    if (prsactx != NULL)
        prsactx->verify_message = 1;
#endif

    return rsa_signverify_init(prsactx, vrsa, rsa_set_ctx_params, params,
                               EVP_PKEY_OP_SIGN, "RSA Sign Init");
}

/*
 * Sign tbs without digesting it first.  This is suitable for "primitive"
 * signing and signing the digest of a message, i.e. should be used with
 * implementations of the keytype related algorithms.
 */
static int rsa_sign_directly(PROV_RSA_CTX *prsactx,
                             unsigned char *sig, size_t *siglen, size_t sigsize,
                             const unsigned char *tbs, size_t tbslen)
{
    int ret;
    size_t rsasize = RSA_size(prsactx->rsa);
    size_t mdsize = rsa_get_md_size(prsactx);

    if (!ossl_prov_is_running())
        return 0;

    if (sig == NULL) {
        *siglen = rsasize;
        return 1;
    }

    if (sigsize < rsasize) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_SIGNATURE_SIZE,
                       "is %zu, should be at least %zu", sigsize, rsasize);
        return 0;
    }

    if (mdsize != 0) {
        if (tbslen != mdsize) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_DIGEST_LENGTH);
            return 0;
        }

#ifndef FIPS_MODULE
        if (EVP_MD_is_a(prsactx->md, OSSL_DIGEST_NAME_MDC2)) {
            unsigned int sltmp;

            if (prsactx->pad_mode != RSA_PKCS1_PADDING) {
                ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_PADDING_MODE,
                               "only PKCS#1 padding supported with MDC2");
                return 0;
            }
            ret = RSA_sign_ASN1_OCTET_STRING(0, tbs, tbslen, sig, &sltmp,
                                             prsactx->rsa);

            if (ret <= 0) {
                ERR_raise(ERR_LIB_PROV, ERR_R_RSA_LIB);
                return 0;
            }
            ret = sltmp;
            goto end;
        }
#endif
        switch (prsactx->pad_mode) {
        case RSA_X931_PADDING:
            if ((size_t)RSA_size(prsactx->rsa) < tbslen + 1) {
                ERR_raise_data(ERR_LIB_PROV, PROV_R_KEY_SIZE_TOO_SMALL,
                               "RSA key size = %d, expected minimum = %d",
                               RSA_size(prsactx->rsa), tbslen + 1);
                return 0;
            }
            if (!setup_tbuf(prsactx)) {
                ERR_raise(ERR_LIB_PROV, ERR_R_PROV_LIB);
                return 0;
            }
            memcpy(prsactx->tbuf, tbs, tbslen);
            prsactx->tbuf[tbslen] = RSA_X931_hash_id(prsactx->mdnid);
            ret = RSA_private_encrypt(tbslen + 1, prsactx->tbuf,
                                      sig, prsactx->rsa, RSA_X931_PADDING);
            clean_tbuf(prsactx);
            break;
        case RSA_PKCS1_PADDING:
            {
                unsigned int sltmp;

                ret = RSA_sign(prsactx->mdnid, tbs, tbslen, sig, &sltmp,
                               prsactx->rsa);
                if (ret <= 0) {
                    ERR_raise(ERR_LIB_PROV, ERR_R_RSA_LIB);
                    return 0;
                }
                ret = sltmp;
            }
            break;

        case RSA_PKCS1_PSS_PADDING:
            {
                int saltlen;

                /* Check PSS restrictions */
                if (rsa_pss_restricted(prsactx)) {
                    switch (prsactx->saltlen) {
                    case RSA_PSS_SALTLEN_DIGEST:
                        if (prsactx->min_saltlen > EVP_MD_get_size(prsactx->md)) {
                            ERR_raise_data(ERR_LIB_PROV,
                                           PROV_R_PSS_SALTLEN_TOO_SMALL,
                                           "minimum salt length set to %d, "
                                           "but the digest only gives %d",
                                           prsactx->min_saltlen,
                                           EVP_MD_get_size(prsactx->md));
                            return 0;
                        }
                        /* FALLTHRU */
                    default:
                        if (prsactx->saltlen >= 0
                            && prsactx->saltlen < prsactx->min_saltlen) {
                            ERR_raise_data(ERR_LIB_PROV,
                                           PROV_R_PSS_SALTLEN_TOO_SMALL,
                                           "minimum salt length set to %d, but the"
                                           "actual salt length is only set to %d",
                                           prsactx->min_saltlen,
                                           prsactx->saltlen);
                            return 0;
                        }
                        break;
                    }
                }
                if (!setup_tbuf(prsactx))
                    return 0;
                saltlen = prsactx->saltlen;
                if (!ossl_rsa_padding_add_PKCS1_PSS_mgf1(prsactx->rsa,
                                                         prsactx->tbuf, tbs,
                                                         prsactx->md, prsactx->mgf1_md,
                                                         &saltlen)) {
                    ERR_raise(ERR_LIB_PROV, ERR_R_RSA_LIB);
                    return 0;
                }
#ifdef FIPS_MODULE
                if (!rsa_pss_saltlen_check_passed(prsactx, "RSA Sign", saltlen))
                    return 0;
#endif
                ret = RSA_private_encrypt(RSA_size(prsactx->rsa), prsactx->tbuf,
                                          sig, prsactx->rsa, RSA_NO_PADDING);
                clean_tbuf(prsactx);
            }
            break;

        default:
            ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_PADDING_MODE,
                           "Only X.931, PKCS#1 v1.5 or PSS padding allowed");
            return 0;
        }
    } else {
        ret = RSA_private_encrypt(tbslen, tbs, sig, prsactx->rsa,
                                  prsactx->pad_mode);
    }

#ifndef FIPS_MODULE
 end:
#endif
    if (ret <= 0) {
        ERR_raise(ERR_LIB_PROV, ERR_R_RSA_LIB);
        return 0;
    }

    *siglen = ret;
    return 1;
}

static int rsa_signverify_message_update(void *vprsactx,
                                         const unsigned char *data,
                                         size_t datalen)
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;

    if (prsactx == NULL || prsactx->mdctx == NULL)
        return 0;

    if (!prsactx->flag_allow_update) {
        ERR_raise(ERR_LIB_PROV, PROV_R_UPDATE_CALL_OUT_OF_ORDER);
        return 0;
    }
    prsactx->flag_allow_oneshot = 0;

    return EVP_DigestUpdate(prsactx->mdctx, data, datalen);
}

static int rsa_sign_message_final(void *vprsactx, unsigned char *sig,
                                  size_t *siglen, size_t sigsize)
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;
    unsigned char digest[EVP_MAX_MD_SIZE];
    unsigned int dlen = 0;

    if (!ossl_prov_is_running() || prsactx == NULL)
        return 0;
    if (prsactx->mdctx == NULL)
        return 0;
    if (!prsactx->flag_allow_final) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FINAL_CALL_OUT_OF_ORDER);
        return 0;
    }

    /*
     * If sig is NULL then we're just finding out the sig size. Other fields
     * are ignored. Defer to rsa_sign.
     */
    if (sig != NULL) {
        /*
         * The digests used here are all known (see rsa_get_md_nid()), so they
         * should not exceed the internal buffer size of EVP_MAX_MD_SIZE.
         */
        if (!EVP_DigestFinal_ex(prsactx->mdctx, digest, &dlen))
            return 0;

        prsactx->flag_allow_update = 0;
        prsactx->flag_allow_oneshot = 0;
        prsactx->flag_allow_final = 0;
    }

    return rsa_sign_directly(prsactx, sig, siglen, sigsize, digest, dlen);
}

/*
 * If signing a message, digest tbs and sign the result.
 * Otherwise, sign tbs directly.
 */
static int rsa_sign(void *vprsactx, unsigned char *sig, size_t *siglen,
                    size_t sigsize, const unsigned char *tbs, size_t tbslen)
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;

    if (!ossl_prov_is_running() || prsactx == NULL)
        return 0;
    if (!prsactx->flag_allow_oneshot) {
        ERR_raise(ERR_LIB_PROV, PROV_R_ONESHOT_CALL_OUT_OF_ORDER);
        return 0;
    }

    if (prsactx->operation == EVP_PKEY_OP_SIGNMSG) {
        /*
         * If |sig| is NULL, the caller is only looking for the sig length.
         * DO NOT update the input in this case.
         */
        if (sig == NULL)
            return rsa_sign_message_final(prsactx, sig, siglen, sigsize);

        return rsa_signverify_message_update(prsactx, tbs, tbslen)
            && rsa_sign_message_final(prsactx, sig, siglen, sigsize);
    }
    return rsa_sign_directly(prsactx, sig, siglen, sigsize, tbs, tbslen);
}

static int rsa_verify_recover_init(void *vprsactx, void *vrsa,
                                   const OSSL_PARAM params[])
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;

#ifdef FIPS_MODULE
    if (prsactx != NULL)
        prsactx->verify_message = 0;
#endif

    return rsa_signverify_init(prsactx, vrsa, rsa_set_ctx_params, params,
                               EVP_PKEY_OP_VERIFYRECOVER, "RSA VerifyRecover Init");
}

/*
 * There is no message variant of verify recover, so no need for
 * 'rsa_verify_recover_directly', just use this function, er, directly.
 */
static int rsa_verify_recover(void *vprsactx,
                              unsigned char *rout, size_t *routlen,
                              size_t routsize,
                              const unsigned char *sig, size_t siglen)
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;
    int ret;

    if (!ossl_prov_is_running())
        return 0;

    if (rout == NULL) {
        *routlen = RSA_size(prsactx->rsa);
        return 1;
    }

    if (prsactx->md != NULL) {
        switch (prsactx->pad_mode) {
        case RSA_X931_PADDING:
            if (!setup_tbuf(prsactx))
                return 0;
            ret = RSA_public_decrypt(siglen, sig, prsactx->tbuf, prsactx->rsa,
                                     RSA_X931_PADDING);
            if (ret < 1) {
                ERR_raise(ERR_LIB_PROV, ERR_R_RSA_LIB);
                return 0;
            }
            ret--;
            if (prsactx->tbuf[ret] != RSA_X931_hash_id(prsactx->mdnid)) {
                ERR_raise(ERR_LIB_PROV, PROV_R_ALGORITHM_MISMATCH);
                return 0;
            }
            if (ret != EVP_MD_get_size(prsactx->md)) {
                ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_DIGEST_LENGTH,
                               "Should be %d, but got %d",
                               EVP_MD_get_size(prsactx->md), ret);
                return 0;
            }

            *routlen = ret;
            if (rout != prsactx->tbuf) {
                if (routsize < (size_t)ret) {
                    ERR_raise_data(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL,
                                   "buffer size is %d, should be %d",
                                   routsize, ret);
                    return 0;
                }
                memcpy(rout, prsactx->tbuf, ret);
            }
            break;

        case RSA_PKCS1_PADDING:
            {
                size_t sltmp;

                ret = ossl_rsa_verify(prsactx->mdnid, NULL, 0, rout, &sltmp,
                                      sig, siglen, prsactx->rsa);
                if (ret <= 0) {
                    ERR_raise(ERR_LIB_PROV, ERR_R_RSA_LIB);
                    return 0;
                }
                ret = sltmp;
            }
            break;

        default:
            ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_PADDING_MODE,
                           "Only X.931 or PKCS#1 v1.5 padding allowed");
            return 0;
        }
    } else {
        ret = RSA_public_decrypt(siglen, sig, rout, prsactx->rsa,
                                 prsactx->pad_mode);
        if (ret < 0) {
            ERR_raise(ERR_LIB_PROV, ERR_R_RSA_LIB);
            return 0;
        }
    }
    *routlen = ret;
    return 1;
}

static int rsa_verify_init(void *vprsactx, void *vrsa,
                           const OSSL_PARAM params[])
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;

#ifdef FIPS_MODULE
    if (prsactx != NULL)
        prsactx->verify_message = 0;
#endif

    return rsa_signverify_init(prsactx, vrsa, rsa_set_ctx_params, params,
                               EVP_PKEY_OP_VERIFY, "RSA Verify Init");
}

static int rsa_verify_directly(PROV_RSA_CTX *prsactx,
                               const unsigned char *sig, size_t siglen,
                               const unsigned char *tbs, size_t tbslen)
{
    size_t rslen;

    if (!ossl_prov_is_running())
        return 0;
    if (prsactx->md != NULL) {
        switch (prsactx->pad_mode) {
        case RSA_PKCS1_PADDING:
            if (!RSA_verify(prsactx->mdnid, tbs, tbslen, sig, siglen,
                            prsactx->rsa)) {
                ERR_raise(ERR_LIB_PROV, ERR_R_RSA_LIB);
                return 0;
            }
            return 1;
        case RSA_X931_PADDING:
            if (!setup_tbuf(prsactx))
                return 0;
            if (rsa_verify_recover(prsactx, prsactx->tbuf, &rslen, 0,
                                   sig, siglen) <= 0)
                return 0;
            break;
        case RSA_PKCS1_PSS_PADDING:
            {
                int ret;
                int saltlen;
                size_t mdsize;

                /*
                 * We need to check this for the RSA_verify_PKCS1_PSS_mgf1()
                 * call
                 */
                mdsize = rsa_get_md_size(prsactx);
                if (tbslen != mdsize) {
                    ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_DIGEST_LENGTH,
                                   "Should be %d, but got %d",
                                   mdsize, tbslen);
                    return 0;
                }

                if (!setup_tbuf(prsactx))
                    return 0;
                ret = RSA_public_decrypt(siglen, sig, prsactx->tbuf,
                                         prsactx->rsa, RSA_NO_PADDING);
                if (ret <= 0) {
                    ERR_raise(ERR_LIB_PROV, ERR_R_RSA_LIB);
                    return 0;
                }
                saltlen = prsactx->saltlen;
                ret = ossl_rsa_verify_PKCS1_PSS_mgf1(prsactx->rsa, tbs,
                                                     prsactx->md, prsactx->mgf1_md,
                                                     prsactx->tbuf,
                                                     &saltlen);
                if (ret <= 0) {
                    ERR_raise(ERR_LIB_PROV, ERR_R_RSA_LIB);
                    return 0;
                }
#ifdef FIPS_MODULE
                if (!rsa_pss_saltlen_check_passed(prsactx, "RSA Verify", saltlen))
                    return 0;
#endif
                return 1;
            }
        default:
            ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_PADDING_MODE,
                           "Only X.931, PKCS#1 v1.5 or PSS padding allowed");
            return 0;
        }
    } else {
        int ret;

        if (!setup_tbuf(prsactx))
            return 0;
        ret = RSA_public_decrypt(siglen, sig, prsactx->tbuf, prsactx->rsa,
                                 prsactx->pad_mode);
        if (ret <= 0) {
            ERR_raise(ERR_LIB_PROV, ERR_R_RSA_LIB);
            return 0;
        }
        rslen = (size_t)ret;
    }

    if ((rslen != tbslen) || memcmp(tbs, prsactx->tbuf, rslen))
        return 0;

    return 1;
}

static int rsa_verify_set_sig(void *vprsactx,
                              const unsigned char *sig, size_t siglen)
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;
    OSSL_PARAM params[2];

    params[0] =
        OSSL_PARAM_construct_octet_string(OSSL_SIGNATURE_PARAM_SIGNATURE,
                                          (unsigned char *)sig, siglen);
    params[1] = OSSL_PARAM_construct_end();
    return rsa_sigalg_set_ctx_params(prsactx, params);
}

static int rsa_verify_message_final(void *vprsactx)
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;
    unsigned char digest[EVP_MAX_MD_SIZE];
    unsigned int dlen = 0;

    if (!ossl_prov_is_running() || prsactx == NULL)
        return 0;
    if (prsactx->mdctx == NULL)
        return 0;
    if (!prsactx->flag_allow_final) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FINAL_CALL_OUT_OF_ORDER);
        return 0;
    }

    /*
     * The digests used here are all known (see rsa_get_md_nid()), so they
     * should not exceed the internal buffer size of EVP_MAX_MD_SIZE.
     */
    if (!EVP_DigestFinal_ex(prsactx->mdctx, digest, &dlen))
        return 0;

    prsactx->flag_allow_update = 0;
    prsactx->flag_allow_final = 0;
    prsactx->flag_allow_oneshot = 0;

    return rsa_verify_directly(prsactx, prsactx->sig, prsactx->siglen,
                               digest, dlen);
}

/*
 * If verifying a message, digest tbs and verify the result.
 * Otherwise, verify tbs directly.
 */
static int rsa_verify(void *vprsactx,
                      const unsigned char *sig, size_t siglen,
                      const unsigned char *tbs, size_t tbslen)
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;

    if (!ossl_prov_is_running() || prsactx == NULL)
        return 0;
    if (!prsactx->flag_allow_oneshot) {
        ERR_raise(ERR_LIB_PROV, PROV_R_ONESHOT_CALL_OUT_OF_ORDER);
        return 0;
    }

    if (prsactx->operation == EVP_PKEY_OP_VERIFYMSG)
        return rsa_verify_set_sig(prsactx, sig, siglen)
            && rsa_signverify_message_update(prsactx, tbs, tbslen)
            && rsa_verify_message_final(prsactx);
    return rsa_verify_directly(prsactx, sig, siglen, tbs, tbslen);
}

/* DigestSign/DigestVerify wrappers */

static int rsa_digest_signverify_init(void *vprsactx, const char *mdname,
                                      void *vrsa, const OSSL_PARAM params[],
                                      int operation, const char *desc)
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;

#ifdef FIPS_MODULE
    if (prsactx != NULL)
        prsactx->verify_message = 1;
#endif

    if (!rsa_signverify_init(prsactx, vrsa, rsa_set_ctx_params, params,
                             operation, desc))
        return 0;

    if (mdname != NULL
        /* was rsa_setup_md already called in rsa_signverify_init()? */
        && (mdname[0] == '\0' || OPENSSL_strcasecmp(prsactx->mdname, mdname) != 0)
        && !rsa_setup_md(prsactx, mdname, prsactx->propq, desc))
        return 0;

    prsactx->flag_allow_md = 0;

    if (prsactx->mdctx == NULL) {
        prsactx->mdctx = EVP_MD_CTX_new();
        if (prsactx->mdctx == NULL)
            goto error;
    }

    if (!EVP_DigestInit_ex2(prsactx->mdctx, prsactx->md, params))
        goto error;

    return 1;

 error:
    EVP_MD_CTX_free(prsactx->mdctx);
    prsactx->mdctx = NULL;
    return 0;
}

static int rsa_digest_sign_init(void *vprsactx, const char *mdname,
                                void *vrsa, const OSSL_PARAM params[])
{
    if (!ossl_prov_is_running())
        return 0;
    return rsa_digest_signverify_init(vprsactx, mdname, vrsa,
                                      params, EVP_PKEY_OP_SIGNMSG,
                                      "RSA Digest Sign Init");
}

static int rsa_digest_sign_update(void *vprsactx, const unsigned char *data,
                                  size_t datalen)
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;

    if (prsactx == NULL)
        return 0;
    /* Sigalg implementations shouldn't do digest_sign */
    if (prsactx->flag_sigalg)
        return 0;

    return rsa_signverify_message_update(prsactx, data, datalen);
}

static int rsa_digest_sign_final(void *vprsactx, unsigned char *sig,
                                 size_t *siglen, size_t sigsize)
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;
    int ok = 0;

    if (prsactx == NULL)
        return 0;
    /* Sigalg implementations shouldn't do digest_sign */
    if (prsactx->flag_sigalg)
        return 0;

    if (rsa_sign_message_final(prsactx, sig, siglen, sigsize))
        ok = 1;

    prsactx->flag_allow_md = 1;

    return ok;
}

static int rsa_digest_verify_init(void *vprsactx, const char *mdname,
                                  void *vrsa, const OSSL_PARAM params[])
{
    if (!ossl_prov_is_running())
        return 0;
    return rsa_digest_signverify_init(vprsactx, mdname, vrsa,
                                      params, EVP_PKEY_OP_VERIFYMSG,
                                      "RSA Digest Verify Init");
}

static int rsa_digest_verify_update(void *vprsactx, const unsigned char *data,
                                    size_t datalen)
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;

    if (prsactx == NULL)
        return 0;
    /* Sigalg implementations shouldn't do digest_sign */
    if (prsactx->flag_sigalg)
        return 0;

    return rsa_signverify_message_update(prsactx, data, datalen);
}

int rsa_digest_verify_final(void *vprsactx, const unsigned char *sig,
                            size_t siglen)
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;
    int ok = 0;

    if (prsactx == NULL)
        return 0;
    /* Sigalg implementations shouldn't do digest_verify */
    if (prsactx->flag_sigalg)
        return 0;

    if (rsa_verify_set_sig(prsactx, sig, siglen)
        && rsa_verify_message_final(vprsactx))
        ok = 1;

    prsactx->flag_allow_md = 1;

    return ok;
}

static void rsa_freectx(void *vprsactx)
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;

    if (prsactx == NULL)
        return;

    EVP_MD_CTX_free(prsactx->mdctx);
    EVP_MD_free(prsactx->md);
    EVP_MD_free(prsactx->mgf1_md);
    OPENSSL_free(prsactx->sig);
    OPENSSL_free(prsactx->propq);
    free_tbuf(prsactx);
    RSA_free(prsactx->rsa);

    OPENSSL_clear_free(prsactx, sizeof(*prsactx));
}

static void *rsa_dupctx(void *vprsactx)
{
    PROV_RSA_CTX *srcctx = (PROV_RSA_CTX *)vprsactx;
    PROV_RSA_CTX *dstctx;

    if (!ossl_prov_is_running())
        return NULL;

    dstctx = OPENSSL_zalloc(sizeof(*srcctx));
    if (dstctx == NULL)
        return NULL;

    *dstctx = *srcctx;
    dstctx->rsa = NULL;
    dstctx->md = NULL;
    dstctx->mgf1_md = NULL;
    dstctx->mdctx = NULL;
    dstctx->tbuf = NULL;
    dstctx->propq = NULL;

    if (srcctx->rsa != NULL && !RSA_up_ref(srcctx->rsa))
        goto err;
    dstctx->rsa = srcctx->rsa;

    if (srcctx->md != NULL && !EVP_MD_up_ref(srcctx->md))
        goto err;
    dstctx->md = srcctx->md;

    if (srcctx->mgf1_md != NULL && !EVP_MD_up_ref(srcctx->mgf1_md))
        goto err;
    dstctx->mgf1_md = srcctx->mgf1_md;

    if (srcctx->mdctx != NULL) {
        dstctx->mdctx = EVP_MD_CTX_new();
        if (dstctx->mdctx == NULL
                || !EVP_MD_CTX_copy_ex(dstctx->mdctx, srcctx->mdctx))
            goto err;
    }

    if (srcctx->propq != NULL) {
        dstctx->propq = OPENSSL_strdup(srcctx->propq);
        if (dstctx->propq == NULL)
            goto err;
    }

    return dstctx;
 err:
    rsa_freectx(dstctx);
    return NULL;
}

static int rsa_get_ctx_params(void *vprsactx, OSSL_PARAM *params)
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;
    OSSL_PARAM *p;

    if (prsactx == NULL)
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_SIGNATURE_PARAM_ALGORITHM_ID);
    if (p != NULL) {
        /* The Algorithm Identifier of the combined signature algorithm */
        unsigned char aid_buf[128];
        unsigned char *aid;
        size_t  aid_len;

        aid = rsa_generate_signature_aid(prsactx, aid_buf,
                                         sizeof(aid_buf), &aid_len);
        if (aid == NULL || !OSSL_PARAM_set_octet_string(p, aid, aid_len))
            return 0;
    }

    p = OSSL_PARAM_locate(params, OSSL_SIGNATURE_PARAM_PAD_MODE);
    if (p != NULL)
        switch (p->data_type) {
        case OSSL_PARAM_INTEGER:
            if (!OSSL_PARAM_set_int(p, prsactx->pad_mode))
                return 0;
            break;
        case OSSL_PARAM_UTF8_STRING:
            {
                int i;
                const char *word = NULL;

                for (i = 0; padding_item[i].id != 0; i++) {
                    if (prsactx->pad_mode == (int)padding_item[i].id) {
                        word = padding_item[i].ptr;
                        break;
                    }
                }

                if (word != NULL) {
                    if (!OSSL_PARAM_set_utf8_string(p, word))
                        return 0;
                } else {
                    ERR_raise(ERR_LIB_PROV, ERR_R_INTERNAL_ERROR);
                }
            }
            break;
        default:
            return 0;
        }

    p = OSSL_PARAM_locate(params, OSSL_SIGNATURE_PARAM_DIGEST);
    if (p != NULL && !OSSL_PARAM_set_utf8_string(p, prsactx->mdname))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_SIGNATURE_PARAM_MGF1_DIGEST);
    if (p != NULL && !OSSL_PARAM_set_utf8_string(p, prsactx->mgf1_mdname))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_SIGNATURE_PARAM_PSS_SALTLEN);
    if (p != NULL) {
        if (p->data_type == OSSL_PARAM_INTEGER) {
            if (!OSSL_PARAM_set_int(p, prsactx->saltlen))
                return 0;
        } else if (p->data_type == OSSL_PARAM_UTF8_STRING) {
            const char *value = NULL;

            switch (prsactx->saltlen) {
            case RSA_PSS_SALTLEN_DIGEST:
                value = OSSL_PKEY_RSA_PSS_SALT_LEN_DIGEST;
                break;
            case RSA_PSS_SALTLEN_MAX:
                value = OSSL_PKEY_RSA_PSS_SALT_LEN_MAX;
                break;
            case RSA_PSS_SALTLEN_AUTO:
                value = OSSL_PKEY_RSA_PSS_SALT_LEN_AUTO;
                break;
            case RSA_PSS_SALTLEN_AUTO_DIGEST_MAX:
                value = OSSL_PKEY_RSA_PSS_SALT_LEN_AUTO_DIGEST_MAX;
                break;
            default:
                {
                    int len = BIO_snprintf(p->data, p->data_size, "%d",
                                           prsactx->saltlen);

                    if (len <= 0)
                        return 0;
                    p->return_size = len;
                    break;
                }
            }
            if (value != NULL
                && !OSSL_PARAM_set_utf8_string(p, value))
                return 0;
        }
    }

#ifdef FIPS_MODULE
    p = OSSL_PARAM_locate(params, OSSL_SIGNATURE_PARAM_FIPS_VERIFY_MESSAGE);
    if (p != NULL && !OSSL_PARAM_set_uint(p, prsactx->verify_message))
        return 0;
#endif

    if (!OSSL_FIPS_IND_GET_CTX_PARAM(prsactx, params))
        return 0;
    return 1;
}

static const OSSL_PARAM known_gettable_ctx_params[] = {
    OSSL_PARAM_octet_string(OSSL_SIGNATURE_PARAM_ALGORITHM_ID, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_SIGNATURE_PARAM_PAD_MODE, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_SIGNATURE_PARAM_DIGEST, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_SIGNATURE_PARAM_MGF1_DIGEST, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_SIGNATURE_PARAM_PSS_SALTLEN, NULL, 0),
#ifdef FIPS_MODULE
    OSSL_PARAM_uint(OSSL_SIGNATURE_PARAM_FIPS_VERIFY_MESSAGE, NULL),
#endif
    OSSL_FIPS_IND_GETTABLE_CTX_PARAM()
    OSSL_PARAM_END
};

static const OSSL_PARAM *rsa_gettable_ctx_params(ossl_unused void *vprsactx,
                                                 ossl_unused void *provctx)
{
    return known_gettable_ctx_params;
}

#ifdef FIPS_MODULE
static int rsa_x931_padding_allowed(PROV_RSA_CTX *ctx)
{
    int approved = ((ctx->operation & EVP_PKEY_OP_SIGN) == 0);

    if (!approved) {
        if (!OSSL_FIPS_IND_ON_UNAPPROVED(ctx, OSSL_FIPS_IND_SETTABLE2,
                                         ctx->libctx,
                                         "RSA Sign set ctx", "X931 Padding",
                                         ossl_fips_config_rsa_sign_x931_disallowed)) {
            ERR_raise(ERR_LIB_PROV,
                      PROV_R_ILLEGAL_OR_UNSUPPORTED_PADDING_MODE);
            return 0;
        }
    }
    return 1;
}
#endif

static int rsa_set_ctx_params(void *vprsactx, const OSSL_PARAM params[])
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;
    const OSSL_PARAM *p;
    int pad_mode;
    int saltlen;
    char mdname[OSSL_MAX_NAME_SIZE] = "", *pmdname = NULL;
    char mdprops[OSSL_MAX_PROPQUERY_SIZE] = "", *pmdprops = NULL;
    char mgf1mdname[OSSL_MAX_NAME_SIZE] = "", *pmgf1mdname = NULL;
    char mgf1mdprops[OSSL_MAX_PROPQUERY_SIZE] = "", *pmgf1mdprops = NULL;

    if (prsactx == NULL)
        return 0;
    if (ossl_param_is_empty(params))
        return 1;

    if (!OSSL_FIPS_IND_SET_CTX_PARAM(prsactx, OSSL_FIPS_IND_SETTABLE0, params,
                                     OSSL_SIGNATURE_PARAM_FIPS_KEY_CHECK))
        return 0;

    if (!OSSL_FIPS_IND_SET_CTX_PARAM(prsactx, OSSL_FIPS_IND_SETTABLE1, params,
                                     OSSL_SIGNATURE_PARAM_FIPS_DIGEST_CHECK))
        return 0;

    if (!OSSL_FIPS_IND_SET_CTX_PARAM(prsactx, OSSL_FIPS_IND_SETTABLE2, params,
                                     OSSL_SIGNATURE_PARAM_FIPS_SIGN_X931_PAD_CHECK))
        return 0;

    if (!OSSL_FIPS_IND_SET_CTX_PARAM(prsactx, OSSL_FIPS_IND_SETTABLE3, params,
                                     OSSL_SIGNATURE_PARAM_FIPS_RSA_PSS_SALTLEN_CHECK))
        return 0;

    pad_mode = prsactx->pad_mode;
    saltlen = prsactx->saltlen;

    p = OSSL_PARAM_locate_const(params, OSSL_SIGNATURE_PARAM_DIGEST);
    if (p != NULL) {
        const OSSL_PARAM *propsp =
            OSSL_PARAM_locate_const(params,
                                    OSSL_SIGNATURE_PARAM_PROPERTIES);

        pmdname = mdname;
        if (!OSSL_PARAM_get_utf8_string(p, &pmdname, sizeof(mdname)))
            return 0;

        if (propsp != NULL) {
            pmdprops = mdprops;
            if (!OSSL_PARAM_get_utf8_string(propsp,
                                            &pmdprops, sizeof(mdprops)))
                return 0;
        }
    }

    p = OSSL_PARAM_locate_const(params, OSSL_SIGNATURE_PARAM_PAD_MODE);
    if (p != NULL) {
        const char *err_extra_text = NULL;

        switch (p->data_type) {
        case OSSL_PARAM_INTEGER: /* Support for legacy pad mode number */
            if (!OSSL_PARAM_get_int(p, &pad_mode))
                return 0;
            break;
        case OSSL_PARAM_UTF8_STRING:
            {
                int i;

                if (p->data == NULL)
                    return 0;

                for (i = 0; padding_item[i].id != 0; i++) {
                    if (strcmp(p->data, padding_item[i].ptr) == 0) {
                        pad_mode = padding_item[i].id;
                        break;
                    }
                }
            }
            break;
        default:
            return 0;
        }

        switch (pad_mode) {
        case RSA_PKCS1_OAEP_PADDING:
            /*
             * OAEP padding is for asymmetric cipher only so is not compatible
             * with signature use.
             */
            err_extra_text = "OAEP padding not allowed for signing / verifying";
            goto bad_pad;
        case RSA_PKCS1_PSS_PADDING:
            if ((prsactx->operation
                 & (EVP_PKEY_OP_SIGN | EVP_PKEY_OP_SIGNMSG
                    | EVP_PKEY_OP_VERIFY | EVP_PKEY_OP_VERIFYMSG)) == 0) {
                err_extra_text =
                    "PSS padding only allowed for sign and verify operations";
                goto bad_pad;
            }
            break;
        case RSA_PKCS1_PADDING:
            err_extra_text = "PKCS#1 padding not allowed with RSA-PSS";
            goto cont;
        case RSA_NO_PADDING:
            err_extra_text = "No padding not allowed with RSA-PSS";
            goto cont;
        case RSA_X931_PADDING:
#ifdef FIPS_MODULE
            /* X9.31 only allows sizes of 1024 + 256 * s (bits) */
            if ((RSA_bits(prsactx->rsa) & 0xFF) != 0) {
                ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
                return 0;
            }
            /* RSA Signing with X9.31 padding is not allowed in FIPS 140-3 */
            if (!rsa_x931_padding_allowed(prsactx))
                return 0;
#endif
            err_extra_text = "X.931 padding not allowed with RSA-PSS";
        cont:
            if (RSA_test_flags(prsactx->rsa,
                               RSA_FLAG_TYPE_MASK) == RSA_FLAG_TYPE_RSA)
                break;
            /* FALLTHRU */
        default:
        bad_pad:
            if (err_extra_text == NULL)
                ERR_raise(ERR_LIB_PROV,
                          PROV_R_ILLEGAL_OR_UNSUPPORTED_PADDING_MODE);
            else
                ERR_raise_data(ERR_LIB_PROV,
                               PROV_R_ILLEGAL_OR_UNSUPPORTED_PADDING_MODE,
                               err_extra_text);
            return 0;
        }
    }

    p = OSSL_PARAM_locate_const(params, OSSL_SIGNATURE_PARAM_PSS_SALTLEN);
    if (p != NULL) {
        if (pad_mode != RSA_PKCS1_PSS_PADDING) {
            ERR_raise_data(ERR_LIB_PROV, PROV_R_NOT_SUPPORTED,
                           "PSS saltlen can only be specified if "
                           "PSS padding has been specified first");
            return 0;
        }

        switch (p->data_type) {
        case OSSL_PARAM_INTEGER: /* Support for legacy pad mode number */
            if (!OSSL_PARAM_get_int(p, &saltlen))
                return 0;
            break;
        case OSSL_PARAM_UTF8_STRING:
            if (strcmp(p->data, OSSL_PKEY_RSA_PSS_SALT_LEN_DIGEST) == 0)
                saltlen = RSA_PSS_SALTLEN_DIGEST;
            else if (strcmp(p->data, OSSL_PKEY_RSA_PSS_SALT_LEN_MAX) == 0)
                saltlen = RSA_PSS_SALTLEN_MAX;
            else if (strcmp(p->data, OSSL_PKEY_RSA_PSS_SALT_LEN_AUTO) == 0)
                saltlen = RSA_PSS_SALTLEN_AUTO;
            else if (strcmp(p->data, OSSL_PKEY_RSA_PSS_SALT_LEN_AUTO_DIGEST_MAX) == 0)
                saltlen = RSA_PSS_SALTLEN_AUTO_DIGEST_MAX;
            else
                saltlen = atoi(p->data);
            break;
        default:
            return 0;
        }

        /*
         * RSA_PSS_SALTLEN_AUTO_DIGEST_MAX seems curiously named in this check.
         * Contrary to what it's name suggests, it's the currently lowest
         * saltlen number possible.
         */
        if (saltlen < RSA_PSS_SALTLEN_AUTO_DIGEST_MAX) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_SALT_LENGTH);
            return 0;
        }

        if (rsa_pss_restricted(prsactx)) {
            switch (saltlen) {
            case RSA_PSS_SALTLEN_AUTO:
            case RSA_PSS_SALTLEN_AUTO_DIGEST_MAX:
                if ((prsactx->operation
                     & (EVP_PKEY_OP_VERIFY | EVP_PKEY_OP_VERIFYMSG)) == 0) {
                    ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_SALT_LENGTH,
                                   "Cannot use autodetected salt length");
                    return 0;
                }
                break;
            case RSA_PSS_SALTLEN_DIGEST:
                if (prsactx->min_saltlen > EVP_MD_get_size(prsactx->md)) {
                    ERR_raise_data(ERR_LIB_PROV,
                                   PROV_R_PSS_SALTLEN_TOO_SMALL,
                                   "Should be more than %d, but would be "
                                   "set to match digest size (%d)",
                                   prsactx->min_saltlen,
                                   EVP_MD_get_size(prsactx->md));
                    return 0;
                }
                break;
            default:
                if (saltlen >= 0 && saltlen < prsactx->min_saltlen) {
                    ERR_raise_data(ERR_LIB_PROV,
                                   PROV_R_PSS_SALTLEN_TOO_SMALL,
                                   "Should be more than %d, "
                                   "but would be set to %d",
                                   prsactx->min_saltlen, saltlen);
                    return 0;
                }
            }
        }
    }

    p = OSSL_PARAM_locate_const(params, OSSL_SIGNATURE_PARAM_MGF1_DIGEST);
    if (p != NULL) {
        const OSSL_PARAM *propsp =
            OSSL_PARAM_locate_const(params,
                                    OSSL_SIGNATURE_PARAM_MGF1_PROPERTIES);

        pmgf1mdname = mgf1mdname;
        if (!OSSL_PARAM_get_utf8_string(p, &pmgf1mdname, sizeof(mgf1mdname)))
            return 0;

        if (propsp != NULL) {
            pmgf1mdprops = mgf1mdprops;
            if (!OSSL_PARAM_get_utf8_string(propsp,
                                            &pmgf1mdprops, sizeof(mgf1mdprops)))
                return 0;
        }

        if (pad_mode != RSA_PKCS1_PSS_PADDING) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_MGF1_MD);
            return  0;
        }
    }

    prsactx->saltlen = saltlen;
    prsactx->pad_mode = pad_mode;

    if (prsactx->md == NULL && pmdname == NULL
        && pad_mode == RSA_PKCS1_PSS_PADDING)
        pmdname = RSA_DEFAULT_DIGEST_NAME;

    if (pmgf1mdname != NULL
        && !rsa_setup_mgf1_md(prsactx, pmgf1mdname, pmgf1mdprops))
        return 0;

    if (pmdname != NULL) {
        if (!rsa_setup_md(prsactx, pmdname, pmdprops, "RSA Sign Set Ctx"))
            return 0;
    } else {
        if (!rsa_check_padding(prsactx, NULL, NULL, prsactx->mdnid))
            return 0;
    }
    return 1;
}

static const OSSL_PARAM settable_ctx_params[] = {
    OSSL_PARAM_utf8_string(OSSL_SIGNATURE_PARAM_DIGEST, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_SIGNATURE_PARAM_PROPERTIES, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_SIGNATURE_PARAM_PAD_MODE, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_SIGNATURE_PARAM_MGF1_DIGEST, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_SIGNATURE_PARAM_MGF1_PROPERTIES, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_SIGNATURE_PARAM_PSS_SALTLEN, NULL, 0),
    OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_SIGNATURE_PARAM_FIPS_KEY_CHECK)
    OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_SIGNATURE_PARAM_FIPS_DIGEST_CHECK)
    OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_SIGNATURE_PARAM_FIPS_RSA_PSS_SALTLEN_CHECK)
    OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_SIGNATURE_PARAM_FIPS_SIGN_X931_PAD_CHECK)
    OSSL_PARAM_END
};

static const OSSL_PARAM settable_ctx_params_no_digest[] = {
    OSSL_PARAM_utf8_string(OSSL_SIGNATURE_PARAM_PAD_MODE, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_SIGNATURE_PARAM_MGF1_DIGEST, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_SIGNATURE_PARAM_MGF1_PROPERTIES, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_SIGNATURE_PARAM_PSS_SALTLEN, NULL, 0),
    OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_SIGNATURE_PARAM_FIPS_KEY_CHECK)
    OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_SIGNATURE_PARAM_FIPS_DIGEST_CHECK)
    OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_SIGNATURE_PARAM_FIPS_RSA_PSS_SALTLEN_CHECK)
    OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_SIGNATURE_PARAM_FIPS_SIGN_X931_PAD_CHECK)
    OSSL_PARAM_END
};

static const OSSL_PARAM *rsa_settable_ctx_params(void *vprsactx,
                                                 ossl_unused void *provctx)
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;

    if (prsactx != NULL && !prsactx->flag_allow_md)
        return settable_ctx_params_no_digest;
    return settable_ctx_params;
}

static int rsa_get_ctx_md_params(void *vprsactx, OSSL_PARAM *params)
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;

    if (prsactx->mdctx == NULL)
        return 0;

    return EVP_MD_CTX_get_params(prsactx->mdctx, params);
}

static const OSSL_PARAM *rsa_gettable_ctx_md_params(void *vprsactx)
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;

    if (prsactx->md == NULL)
        return 0;

    return EVP_MD_gettable_ctx_params(prsactx->md);
}

static int rsa_set_ctx_md_params(void *vprsactx, const OSSL_PARAM params[])
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;

    if (prsactx->mdctx == NULL)
        return 0;

    return EVP_MD_CTX_set_params(prsactx->mdctx, params);
}

static const OSSL_PARAM *rsa_settable_ctx_md_params(void *vprsactx)
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;

    if (prsactx->md == NULL)
        return 0;

    return EVP_MD_settable_ctx_params(prsactx->md);
}

const OSSL_DISPATCH ossl_rsa_signature_functions[] = {
    { OSSL_FUNC_SIGNATURE_NEWCTX, (void (*)(void))rsa_newctx },
    { OSSL_FUNC_SIGNATURE_SIGN_INIT, (void (*)(void))rsa_sign_init },
    { OSSL_FUNC_SIGNATURE_SIGN, (void (*)(void))rsa_sign },
    { OSSL_FUNC_SIGNATURE_VERIFY_INIT, (void (*)(void))rsa_verify_init },
    { OSSL_FUNC_SIGNATURE_VERIFY, (void (*)(void))rsa_verify },
    { OSSL_FUNC_SIGNATURE_VERIFY_RECOVER_INIT,
      (void (*)(void))rsa_verify_recover_init },
    { OSSL_FUNC_SIGNATURE_VERIFY_RECOVER,
      (void (*)(void))rsa_verify_recover },
    { OSSL_FUNC_SIGNATURE_DIGEST_SIGN_INIT,
      (void (*)(void))rsa_digest_sign_init },
    { OSSL_FUNC_SIGNATURE_DIGEST_SIGN_UPDATE,
      (void (*)(void))rsa_digest_sign_update },
    { OSSL_FUNC_SIGNATURE_DIGEST_SIGN_FINAL,
      (void (*)(void))rsa_digest_sign_final },
    { OSSL_FUNC_SIGNATURE_DIGEST_VERIFY_INIT,
      (void (*)(void))rsa_digest_verify_init },
    { OSSL_FUNC_SIGNATURE_DIGEST_VERIFY_UPDATE,
      (void (*)(void))rsa_digest_verify_update },
    { OSSL_FUNC_SIGNATURE_DIGEST_VERIFY_FINAL,
      (void (*)(void))rsa_digest_verify_final },
    { OSSL_FUNC_SIGNATURE_FREECTX, (void (*)(void))rsa_freectx },
    { OSSL_FUNC_SIGNATURE_DUPCTX, (void (*)(void))rsa_dupctx },
    { OSSL_FUNC_SIGNATURE_GET_CTX_PARAMS, (void (*)(void))rsa_get_ctx_params },
    { OSSL_FUNC_SIGNATURE_GETTABLE_CTX_PARAMS,
      (void (*)(void))rsa_gettable_ctx_params },
    { OSSL_FUNC_SIGNATURE_SET_CTX_PARAMS, (void (*)(void))rsa_set_ctx_params },
    { OSSL_FUNC_SIGNATURE_SETTABLE_CTX_PARAMS,
      (void (*)(void))rsa_settable_ctx_params },
    { OSSL_FUNC_SIGNATURE_GET_CTX_MD_PARAMS,
      (void (*)(void))rsa_get_ctx_md_params },
    { OSSL_FUNC_SIGNATURE_GETTABLE_CTX_MD_PARAMS,
      (void (*)(void))rsa_gettable_ctx_md_params },
    { OSSL_FUNC_SIGNATURE_SET_CTX_MD_PARAMS,
      (void (*)(void))rsa_set_ctx_md_params },
    { OSSL_FUNC_SIGNATURE_SETTABLE_CTX_MD_PARAMS,
      (void (*)(void))rsa_settable_ctx_md_params },
    OSSL_DISPATCH_END
};

/* ------------------------------------------------------------------ */

/*
 * So called sigalgs (composite RSA+hash) implemented below.  They
 * are pretty much hard coded, and rely on the hash implementation
 * being available as per what OPENSSL_NO_ macros allow.
 */

static OSSL_FUNC_signature_query_key_types_fn rsa_sigalg_query_key_types;
static OSSL_FUNC_signature_settable_ctx_params_fn rsa_sigalg_settable_ctx_params;
static OSSL_FUNC_signature_set_ctx_params_fn rsa_sigalg_set_ctx_params;

/*
 * rsa_sigalg_signverify_init() is almost like rsa_digest_signverify_init(),
 * just doesn't allow fetching an MD from whatever the user chooses.
 */
static int rsa_sigalg_signverify_init(void *vprsactx, void *vrsa,
                                      OSSL_FUNC_signature_set_ctx_params_fn *set_ctx_params,
                                      const OSSL_PARAM params[],
                                      const char *mdname,
                                      int operation, int pad_mode,
                                      const char *desc)
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;

    if (!ossl_prov_is_running())
        return 0;

    if (!rsa_signverify_init(prsactx, vrsa, set_ctx_params, params, operation,
                             desc))
        return 0;

    /* PSS is currently not supported as a sigalg */
    if (prsactx->pad_mode == RSA_PKCS1_PSS_PADDING) {
        ERR_raise(ERR_LIB_RSA, PROV_R_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE);
        return 0;
    }

    if (!rsa_setup_md(prsactx, mdname, NULL, desc))
        return 0;

    prsactx->pad_mode = pad_mode;
    prsactx->flag_sigalg = 1;
    prsactx->flag_allow_md = 0;

    if (prsactx->mdctx == NULL) {
        prsactx->mdctx = EVP_MD_CTX_new();
        if (prsactx->mdctx == NULL)
            goto error;
    }

    if (!EVP_DigestInit_ex2(prsactx->mdctx, prsactx->md, params))
        goto error;

    return 1;

 error:
    EVP_MD_CTX_free(prsactx->mdctx);
    prsactx->mdctx = NULL;
    return 0;
}

static const char **rsa_sigalg_query_key_types(void)
{
    static const char *keytypes[] = { "RSA", NULL };

    return keytypes;
}

static const OSSL_PARAM settable_sigalg_ctx_params[] = {
    OSSL_PARAM_octet_string(OSSL_SIGNATURE_PARAM_SIGNATURE, NULL, 0),
    OSSL_PARAM_END
};

static const OSSL_PARAM *rsa_sigalg_settable_ctx_params(void *vprsactx,
                                                        ossl_unused void *provctx)
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;

    if (prsactx != NULL && prsactx->operation == EVP_PKEY_OP_VERIFYMSG)
        return settable_sigalg_ctx_params;
    return NULL;
}

static int rsa_sigalg_set_ctx_params(void *vprsactx, const OSSL_PARAM params[])
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;
    const OSSL_PARAM *p;

    if (prsactx == NULL)
        return 0;
    if (ossl_param_is_empty(params))
        return 1;

    if (prsactx->operation == EVP_PKEY_OP_VERIFYMSG) {
        p = OSSL_PARAM_locate_const(params, OSSL_SIGNATURE_PARAM_SIGNATURE);
        if (p != NULL) {
            OPENSSL_free(prsactx->sig);
            prsactx->sig = NULL;
            prsactx->siglen = 0;
            if (!OSSL_PARAM_get_octet_string(p, (void **)&prsactx->sig,
                                             0, &prsactx->siglen))
                return 0;
        }
    }
    return 1;
}

#define IMPL_RSA_SIGALG(md, MD)                                         \
    static OSSL_FUNC_signature_sign_init_fn rsa_##md##_sign_init;       \
    static OSSL_FUNC_signature_sign_message_init_fn                     \
        rsa_##md##_sign_message_init;                                   \
    static OSSL_FUNC_signature_verify_init_fn rsa_##md##_verify_init;   \
    static OSSL_FUNC_signature_verify_message_init_fn                   \
        rsa_##md##_verify_message_init;                                 \
                                                                        \
    static int                                                          \
    rsa_##md##_sign_init(void *vprsactx, void *vrsa,                    \
                         const OSSL_PARAM params[])                     \
    {                                                                   \
        static const char desc[] = "RSA Sigalg Sign Init";              \
                                                                        \
        return rsa_sigalg_signverify_init(vprsactx, vrsa,               \
                                          rsa_sigalg_set_ctx_params,    \
                                          params, #MD,                  \
                                          EVP_PKEY_OP_SIGN,             \
                                          RSA_PKCS1_PADDING,            \
                                          desc);                        \
    }                                                                   \
                                                                        \
    static int                                                          \
    rsa_##md##_sign_message_init(void *vprsactx, void *vrsa,            \
                                 const OSSL_PARAM params[])             \
    {                                                                   \
        static const char desc[] = "RSA Sigalg Sign Message Init";      \
                                                                        \
        return rsa_sigalg_signverify_init(vprsactx, vrsa,               \
                                          rsa_sigalg_set_ctx_params,    \
                                          params, #MD,                  \
                                          EVP_PKEY_OP_SIGNMSG,          \
                                          RSA_PKCS1_PADDING,            \
                                          desc);                        \
    }                                                                   \
                                                                        \
    static int                                                          \
    rsa_##md##_verify_init(void *vprsactx, void *vrsa,                  \
                           const OSSL_PARAM params[])                   \
    {                                                                   \
        static const char desc[] = "RSA Sigalg Verify Init";            \
                                                                        \
        return rsa_sigalg_signverify_init(vprsactx, vrsa,               \
                                          rsa_sigalg_set_ctx_params,    \
                                          params, #MD,                  \
                                          EVP_PKEY_OP_VERIFY,           \
                                          RSA_PKCS1_PADDING,            \
                                          desc);                        \
    }                                                                   \
                                                                        \
    static int                                                          \
    rsa_##md##_verify_recover_init(void *vprsactx, void *vrsa,          \
                                   const OSSL_PARAM params[])           \
    {                                                                   \
        static const char desc[] = "RSA Sigalg Verify Recover Init";    \
                                                                        \
        return rsa_sigalg_signverify_init(vprsactx, vrsa,               \
                                          rsa_sigalg_set_ctx_params,    \
                                          params, #MD,                  \
                                          EVP_PKEY_OP_VERIFYRECOVER,    \
                                          RSA_PKCS1_PADDING,            \
                                          desc);                        \
    }                                                                   \
                                                                        \
    static int                                                          \
    rsa_##md##_verify_message_init(void *vprsactx, void *vrsa,          \
                                   const OSSL_PARAM params[])           \
    {                                                                   \
        static const char desc[] = "RSA Sigalg Verify Message Init";    \
                                                                        \
        return rsa_sigalg_signverify_init(vprsactx, vrsa,               \
                                          rsa_sigalg_set_ctx_params,    \
                                          params, #MD,                  \
                                          EVP_PKEY_OP_VERIFYMSG,        \
                                          RSA_PKCS1_PADDING,            \
                                          desc);                        \
    }                                                                   \
                                                                        \
    const OSSL_DISPATCH ossl_rsa_##md##_signature_functions[] = {       \
        { OSSL_FUNC_SIGNATURE_NEWCTX, (void (*)(void))rsa_newctx },     \
        { OSSL_FUNC_SIGNATURE_SIGN_INIT,                                \
          (void (*)(void))rsa_##md##_sign_init },                       \
        { OSSL_FUNC_SIGNATURE_SIGN, (void (*)(void))rsa_sign },         \
        { OSSL_FUNC_SIGNATURE_SIGN_MESSAGE_INIT,                        \
          (void (*)(void))rsa_##md##_sign_message_init },               \
        { OSSL_FUNC_SIGNATURE_SIGN_MESSAGE_UPDATE,                      \
          (void (*)(void))rsa_signverify_message_update },              \
        { OSSL_FUNC_SIGNATURE_SIGN_MESSAGE_FINAL,                       \
          (void (*)(void))rsa_sign_message_final },                     \
        { OSSL_FUNC_SIGNATURE_VERIFY_INIT,                              \
          (void (*)(void))rsa_##md##_verify_init },                     \
        { OSSL_FUNC_SIGNATURE_VERIFY,                                   \
          (void (*)(void))rsa_verify },                                 \
        { OSSL_FUNC_SIGNATURE_VERIFY_MESSAGE_INIT,                      \
          (void (*)(void))rsa_##md##_verify_message_init },             \
        { OSSL_FUNC_SIGNATURE_VERIFY_MESSAGE_UPDATE,                    \
          (void (*)(void))rsa_signverify_message_update },              \
        { OSSL_FUNC_SIGNATURE_VERIFY_MESSAGE_FINAL,                     \
          (void (*)(void))rsa_verify_message_final },                   \
        { OSSL_FUNC_SIGNATURE_VERIFY_RECOVER_INIT,                      \
          (void (*)(void))rsa_##md##_verify_recover_init },             \
        { OSSL_FUNC_SIGNATURE_VERIFY_RECOVER,                           \
          (void (*)(void))rsa_verify_recover },                         \
        { OSSL_FUNC_SIGNATURE_FREECTX, (void (*)(void))rsa_freectx },   \
        { OSSL_FUNC_SIGNATURE_DUPCTX, (void (*)(void))rsa_dupctx },     \
        { OSSL_FUNC_SIGNATURE_QUERY_KEY_TYPES,                          \
          (void (*)(void))rsa_sigalg_query_key_types },                 \
        { OSSL_FUNC_SIGNATURE_GET_CTX_PARAMS,                           \
          (void (*)(void))rsa_get_ctx_params },                         \
        { OSSL_FUNC_SIGNATURE_GETTABLE_CTX_PARAMS,                      \
          (void (*)(void))rsa_gettable_ctx_params },                    \
        { OSSL_FUNC_SIGNATURE_SET_CTX_PARAMS,                           \
          (void (*)(void))rsa_sigalg_set_ctx_params },                  \
        { OSSL_FUNC_SIGNATURE_SETTABLE_CTX_PARAMS,                      \
          (void (*)(void))rsa_sigalg_settable_ctx_params },             \
        OSSL_DISPATCH_END                                               \
    }

#if !defined(OPENSSL_NO_RMD160) && !defined(FIPS_MODULE)
IMPL_RSA_SIGALG(ripemd160, RIPEMD160);
#endif
IMPL_RSA_SIGALG(sha1, SHA1);
IMPL_RSA_SIGALG(sha224, SHA2-224);
IMPL_RSA_SIGALG(sha256, SHA2-256);
IMPL_RSA_SIGALG(sha384, SHA2-384);
IMPL_RSA_SIGALG(sha512, SHA2-512);
IMPL_RSA_SIGALG(sha512_224, SHA2-512/224);
IMPL_RSA_SIGALG(sha512_256, SHA2-512/256);
IMPL_RSA_SIGALG(sha3_224, SHA3-224);
IMPL_RSA_SIGALG(sha3_256, SHA3-256);
IMPL_RSA_SIGALG(sha3_384, SHA3-384);
IMPL_RSA_SIGALG(sha3_512, SHA3-512);
#if !defined(OPENSSL_NO_SM3) && !defined(FIPS_MODULE)
IMPL_RSA_SIGALG(sm3, SM3);
#endif
