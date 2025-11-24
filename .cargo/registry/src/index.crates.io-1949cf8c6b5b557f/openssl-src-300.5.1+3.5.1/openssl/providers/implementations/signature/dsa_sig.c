/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * DSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <string.h>

#include <openssl/crypto.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/err.h>
#include <openssl/dsa.h>
#include <openssl/params.h>
#include <openssl/evp.h>
#include <openssl/proverr.h>
#include "internal/nelem.h"
#include "internal/sizes.h"
#include "internal/cryptlib.h"
#include "prov/providercommon.h"
#include "prov/implementations.h"
#include "prov/provider_ctx.h"
#include "prov/securitycheck.h"
#include "prov/der_dsa.h"
#include "crypto/dsa.h"

static OSSL_FUNC_signature_newctx_fn dsa_newctx;
static OSSL_FUNC_signature_sign_init_fn dsa_sign_init;
static OSSL_FUNC_signature_verify_init_fn dsa_verify_init;
static OSSL_FUNC_signature_sign_fn dsa_sign;
static OSSL_FUNC_signature_sign_message_update_fn dsa_signverify_message_update;
static OSSL_FUNC_signature_sign_message_final_fn dsa_sign_message_final;
static OSSL_FUNC_signature_verify_fn dsa_verify;
static OSSL_FUNC_signature_verify_message_update_fn dsa_signverify_message_update;
static OSSL_FUNC_signature_verify_message_final_fn dsa_verify_message_final;
static OSSL_FUNC_signature_digest_sign_init_fn dsa_digest_sign_init;
static OSSL_FUNC_signature_digest_sign_update_fn dsa_digest_signverify_update;
static OSSL_FUNC_signature_digest_sign_final_fn dsa_digest_sign_final;
static OSSL_FUNC_signature_digest_verify_init_fn dsa_digest_verify_init;
static OSSL_FUNC_signature_digest_verify_update_fn dsa_digest_signverify_update;
static OSSL_FUNC_signature_digest_verify_final_fn dsa_digest_verify_final;
static OSSL_FUNC_signature_freectx_fn dsa_freectx;
static OSSL_FUNC_signature_dupctx_fn dsa_dupctx;
static OSSL_FUNC_signature_query_key_types_fn dsa_sigalg_query_key_types;
static OSSL_FUNC_signature_get_ctx_params_fn dsa_get_ctx_params;
static OSSL_FUNC_signature_gettable_ctx_params_fn dsa_gettable_ctx_params;
static OSSL_FUNC_signature_set_ctx_params_fn dsa_set_ctx_params;
static OSSL_FUNC_signature_settable_ctx_params_fn dsa_settable_ctx_params;
static OSSL_FUNC_signature_get_ctx_md_params_fn dsa_get_ctx_md_params;
static OSSL_FUNC_signature_gettable_ctx_md_params_fn dsa_gettable_ctx_md_params;
static OSSL_FUNC_signature_set_ctx_md_params_fn dsa_set_ctx_md_params;
static OSSL_FUNC_signature_settable_ctx_md_params_fn dsa_settable_ctx_md_params;
static OSSL_FUNC_signature_set_ctx_params_fn dsa_sigalg_set_ctx_params;
static OSSL_FUNC_signature_settable_ctx_params_fn dsa_sigalg_settable_ctx_params;

/*
 * What's passed as an actual key is defined by the KEYMGMT interface.
 * We happen to know that our KEYMGMT simply passes DSA structures, so
 * we use that here too.
 */

typedef struct {
    OSSL_LIB_CTX *libctx;
    char *propq;
    DSA *dsa;
    /* |operation| reuses EVP's operation bitfield */
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
     */
    unsigned int flag_allow_md : 1;

    /* If this is set to 1 then the generated k is not random */
    unsigned int nonce_type;

    /* The Algorithm Identifier of the combined signature algorithm */
    unsigned char aid_buf[OSSL_MAX_ALGORITHM_ID_SIZE];
    size_t  aid_len;

    /* main digest */
    char mdname[OSSL_MAX_NAME_SIZE];
    EVP_MD *md;
    EVP_MD_CTX *mdctx;

    /* Signature, for verification */
    unsigned char *sig;
    size_t siglen;

    OSSL_FIPS_IND_DECLARE
} PROV_DSA_CTX;

static size_t dsa_get_md_size(const PROV_DSA_CTX *pdsactx)
{
    int md_size;

    if (pdsactx->md != NULL) {
        md_size = EVP_MD_get_size(pdsactx->md);
        if (md_size <= 0)
            return 0;
        return (size_t)md_size;
    }
    return 0;
}

static void *dsa_newctx(void *provctx, const char *propq)
{
    PROV_DSA_CTX *pdsactx;

    if (!ossl_prov_is_running())
        return NULL;

    pdsactx = OPENSSL_zalloc(sizeof(PROV_DSA_CTX));
    if (pdsactx == NULL)
        return NULL;

    pdsactx->libctx = PROV_LIBCTX_OF(provctx);
    pdsactx->flag_allow_md = 1;
    OSSL_FIPS_IND_INIT(pdsactx)
    if (propq != NULL && (pdsactx->propq = OPENSSL_strdup(propq)) == NULL) {
        OPENSSL_free(pdsactx);
        pdsactx = NULL;
    }
    return pdsactx;
}

static int dsa_setup_md(PROV_DSA_CTX *ctx,
                        const char *mdname, const char *mdprops,
                        const char *desc)
{
    EVP_MD *md = NULL;

    if (mdprops == NULL)
        mdprops = ctx->propq;

    if (mdname != NULL) {
        WPACKET pkt;
        int md_nid;
        size_t mdname_len = strlen(mdname);
        unsigned char *aid = NULL;

        md = EVP_MD_fetch(ctx->libctx, mdname, mdprops);
        md_nid = ossl_digest_get_approved_nid(md);

        if (md == NULL) {
            ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_DIGEST,
                           "%s could not be fetched", mdname);
            goto err;
        }
        if (md_nid == NID_undef) {
            ERR_raise_data(ERR_LIB_PROV, PROV_R_DIGEST_NOT_ALLOWED,
                           "digest=%s", mdname);
            goto err;
        }
        if (mdname_len >= sizeof(ctx->mdname)) {
            ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_DIGEST,
                           "%s exceeds name buffer length", mdname);
            goto err;
        }
        /* XOF digests don't work */
        if (EVP_MD_xof(md)) {
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

        if (!ctx->flag_allow_md) {
            if (ctx->mdname[0] != '\0'
                && !EVP_MD_is_a(md, ctx->mdname)) {
                ERR_raise_data(ERR_LIB_PROV, PROV_R_DIGEST_NOT_ALLOWED,
                               "digest %s != %s", mdname, ctx->mdname);
                goto err;
            }
            EVP_MD_free(md);
            return 1;
        }

        EVP_MD_CTX_free(ctx->mdctx);
        EVP_MD_free(ctx->md);

        /*
         * We do not care about DER writing errors.
         * All it really means is that for some reason, there's no
         * AlgorithmIdentifier to be had, but the operation itself is
         * still valid, just as long as it's not used to construct
         * anything that needs an AlgorithmIdentifier.
         */
        ctx->aid_len = 0;
        if (WPACKET_init_der(&pkt, ctx->aid_buf, sizeof(ctx->aid_buf))
            && ossl_DER_w_algorithmIdentifier_DSA_with_MD(&pkt, -1, ctx->dsa,
                                                          md_nid)
            && WPACKET_finish(&pkt)) {
            WPACKET_get_total_written(&pkt, &ctx->aid_len);
            aid = WPACKET_get_curr(&pkt);
        }
        WPACKET_cleanup(&pkt);
        if (aid != NULL && ctx->aid_len != 0)
            memmove(ctx->aid_buf, aid, ctx->aid_len);

        ctx->mdctx = NULL;
        ctx->md = md;
        OPENSSL_strlcpy(ctx->mdname, mdname, sizeof(ctx->mdname));
    }

    return 1;
 err:
    EVP_MD_free(md);
    return 0;
}

#ifdef FIPS_MODULE

static int dsa_sign_check_approved(PROV_DSA_CTX *ctx, int signing,
                                   const char *desc)
{
    /* DSA Signing is not approved in FIPS 140-3 */
    if (signing
        && !OSSL_FIPS_IND_ON_UNAPPROVED(ctx, OSSL_FIPS_IND_SETTABLE2,
                                        ctx->libctx, desc, "DSA",
                                        ossl_fips_config_dsa_sign_disallowed))
        return 0;
    return 1;
}

static int dsa_check_key(PROV_DSA_CTX *ctx, int sign, const char *desc)
{
    int approved = ossl_dsa_check_key(ctx->dsa, sign);

    if (!approved) {
        if (!OSSL_FIPS_IND_ON_UNAPPROVED(ctx, OSSL_FIPS_IND_SETTABLE0,
                                         ctx->libctx, desc, "DSA Key",
                                         ossl_fips_config_signature_digest_check)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
            return 0;
        }
    }
    return 1;
}
#endif

static int
dsa_signverify_init(void *vpdsactx, void *vdsa,
                    OSSL_FUNC_signature_set_ctx_params_fn *set_ctx_params,
                    const OSSL_PARAM params[], int operation,
                    const char *desc)
{
    PROV_DSA_CTX *pdsactx = (PROV_DSA_CTX *)vpdsactx;

    if (!ossl_prov_is_running()
            || pdsactx == NULL)
        return 0;

    if (vdsa == NULL && pdsactx->dsa == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_NO_KEY_SET);
        return 0;
    }

    if (vdsa != NULL) {
        if (!DSA_up_ref(vdsa))
            return 0;
        DSA_free(pdsactx->dsa);
        pdsactx->dsa = vdsa;
    }

    pdsactx->operation = operation;

    OSSL_FIPS_IND_SET_APPROVED(pdsactx)
    if (!set_ctx_params(pdsactx, params))
        return 0;
#ifdef FIPS_MODULE
    {
        int operation_is_sign
            = (operation & (EVP_PKEY_OP_SIGN | EVP_PKEY_OP_SIGNMSG)) != 0;

        if (!dsa_sign_check_approved(pdsactx, operation_is_sign, desc))
            return 0;
        if (!dsa_check_key(pdsactx, operation_is_sign, desc))
            return 0;
    }
#endif
    return 1;
}

static int dsa_sign_init(void *vpdsactx, void *vdsa, const OSSL_PARAM params[])
{
    return dsa_signverify_init(vpdsactx, vdsa, dsa_set_ctx_params, params,
                               EVP_PKEY_OP_SIGN, "DSA Sign Init");
}

/*
 * Sign tbs without digesting it first.  This is suitable for "primitive"
 * signing and signing the digest of a message, i.e. should be used with
 * implementations of the keytype related algorithms.
 */
static int dsa_sign_directly(void *vpdsactx,
                             unsigned char *sig, size_t *siglen, size_t sigsize,
                             const unsigned char *tbs, size_t tbslen)
{
    PROV_DSA_CTX *pdsactx = (PROV_DSA_CTX *)vpdsactx;
    int ret;
    unsigned int sltmp;
    size_t dsasize = DSA_size(pdsactx->dsa);
    size_t mdsize = dsa_get_md_size(pdsactx);

    if (!ossl_prov_is_running())
        return 0;

#ifdef FIPS_MODULE
    if (!dsa_sign_check_approved(pdsactx, 1, "Sign"))
        return 0;
#endif

    if (sig == NULL) {
        *siglen = dsasize;
        return 1;
    }

    if (sigsize < dsasize)
        return 0;

    if (mdsize != 0 && tbslen != mdsize)
        return 0;

    ret = ossl_dsa_sign_int(0, tbs, tbslen, sig, &sltmp, pdsactx->dsa,
                            pdsactx->nonce_type, pdsactx->mdname,
                            pdsactx->libctx, pdsactx->propq);
    if (ret <= 0)
        return 0;

    *siglen = sltmp;
    return 1;
}

static int dsa_signverify_message_update(void *vpdsactx,
                                         const unsigned char *data,
                                         size_t datalen)
{
    PROV_DSA_CTX *pdsactx = (PROV_DSA_CTX *)vpdsactx;

    if (pdsactx == NULL)
        return 0;

    return EVP_DigestUpdate(pdsactx->mdctx, data, datalen);
}

static int dsa_sign_message_final(void *vpdsactx, unsigned char *sig,
                                  size_t *siglen, size_t sigsize)
{
    PROV_DSA_CTX *pdsactx = (PROV_DSA_CTX *)vpdsactx;
    unsigned char digest[EVP_MAX_MD_SIZE];
    unsigned int dlen = 0;

    if (!ossl_prov_is_running() || pdsactx == NULL || pdsactx->mdctx == NULL)
        return 0;
    /*
     * If sig is NULL then we're just finding out the sig size. Other fields
     * are ignored. Defer to dsa_sign.
     */
    if (sig != NULL) {
        /*
         * When this function is used through dsa_digest_sign_final(),
         * there is the possibility that some externally provided digests
         * exceed EVP_MAX_MD_SIZE. We should probably handle that
         * somehow but that problem is much larger than just in DSA.
         */
        if (!EVP_DigestFinal_ex(pdsactx->mdctx, digest, &dlen))
            return 0;
    }

    return dsa_sign_directly(vpdsactx, sig, siglen, sigsize, digest, dlen);
}

/*
 * If signing a message, digest tbs and sign the result.
 * Otherwise, sign tbs directly.
 */
static int dsa_sign(void *vpdsactx, unsigned char *sig, size_t *siglen,
                    size_t sigsize, const unsigned char *tbs, size_t tbslen)
{
    PROV_DSA_CTX *pdsactx = (PROV_DSA_CTX *)vpdsactx;

    if (pdsactx->operation == EVP_PKEY_OP_SIGNMSG) {
        /*
         * If |sig| is NULL, the caller is only looking for the sig length.
         * DO NOT update the input in this case.
         */
        if (sig == NULL)
            return dsa_sign_message_final(pdsactx, sig, siglen, sigsize);

        if (dsa_signverify_message_update(pdsactx, tbs, tbslen) <= 0)
            return 0;
        return dsa_sign_message_final(pdsactx, sig, siglen, sigsize);
    }
    return dsa_sign_directly(pdsactx, sig, siglen, sigsize, tbs, tbslen);
}

static int dsa_verify_init(void *vpdsactx, void *vdsa,
                           const OSSL_PARAM params[])
{
    return dsa_signverify_init(vpdsactx, vdsa, dsa_set_ctx_params, params,
                               EVP_PKEY_OP_VERIFY, "DSA Verify Init");
}

static int dsa_verify_directly(void *vpdsactx,
                               const unsigned char *sig, size_t siglen,
                               const unsigned char *tbs, size_t tbslen)
{
    PROV_DSA_CTX *pdsactx = (PROV_DSA_CTX *)vpdsactx;
    size_t mdsize = dsa_get_md_size(pdsactx);

    if (!ossl_prov_is_running() || (mdsize != 0 && tbslen != mdsize))
        return 0;

    return DSA_verify(0, tbs, tbslen, sig, siglen, pdsactx->dsa);
}

static int dsa_verify_set_sig(void *vpdsactx,
                              const unsigned char *sig, size_t siglen)
{
    PROV_DSA_CTX *pdsactx = (PROV_DSA_CTX *)vpdsactx;
    OSSL_PARAM params[2];

    params[0] =
        OSSL_PARAM_construct_octet_string(OSSL_SIGNATURE_PARAM_SIGNATURE,
                                          (unsigned char *)sig, siglen);
    params[1] = OSSL_PARAM_construct_end();
    return dsa_sigalg_set_ctx_params(pdsactx, params);
}

static int dsa_verify_message_final(void *vpdsactx)
{
    PROV_DSA_CTX *pdsactx = (PROV_DSA_CTX *)vpdsactx;
    unsigned char digest[EVP_MAX_MD_SIZE];
    unsigned int dlen = 0;

    if (!ossl_prov_is_running())
        return 0;

    if (pdsactx == NULL || pdsactx->mdctx == NULL)
        return 0;

    /*
     * The digests used here are all known (see dsa_get_md_nid()), so they
     * should not exceed the internal buffer size of EVP_MAX_MD_SIZE.
     */
    if (!EVP_DigestFinal_ex(pdsactx->mdctx, digest, &dlen))
        return 0;

    return dsa_verify_directly(vpdsactx, pdsactx->sig, pdsactx->siglen,
                               digest, dlen);
}

/*
 * If verifying a message, digest tbs and verify the result.
 * Otherwise, verify tbs directly.
 */
static int dsa_verify(void *vpdsactx,
                      const unsigned char *sig, size_t siglen,
                      const unsigned char *tbs, size_t tbslen)
{
    PROV_DSA_CTX *pdsactx = (PROV_DSA_CTX *)vpdsactx;

    if (pdsactx->operation == EVP_PKEY_OP_VERIFYMSG) {
        if (dsa_verify_set_sig(pdsactx, sig, siglen) <= 0)
            return 0;
        if (dsa_signverify_message_update(pdsactx, tbs, tbslen) <= 0)
            return 0;
        return dsa_verify_message_final(pdsactx);
    }
    return dsa_verify_directly(pdsactx, sig, siglen, tbs, tbslen);
}

/* DigestSign/DigestVerify wrappers */

static int dsa_digest_signverify_init(void *vpdsactx, const char *mdname,
                                      void *vdsa, const OSSL_PARAM params[],
                                      int operation, const char *desc)
{
    PROV_DSA_CTX *pdsactx = (PROV_DSA_CTX *)vpdsactx;

    if (!ossl_prov_is_running())
        return 0;

    if (!dsa_signverify_init(vpdsactx, vdsa, dsa_set_ctx_params, params,
                             operation, desc))
        return 0;

    if (mdname != NULL
        /* was dsa_setup_md already called in dsa_signverify_init()? */
        && (mdname[0] == '\0' || OPENSSL_strcasecmp(pdsactx->mdname, mdname) != 0)
        && !dsa_setup_md(pdsactx, mdname, NULL, desc))
        return 0;

    pdsactx->flag_allow_md = 0;

    if (pdsactx->mdctx == NULL) {
        pdsactx->mdctx = EVP_MD_CTX_new();
        if (pdsactx->mdctx == NULL)
            goto error;
    }

    if (!EVP_DigestInit_ex2(pdsactx->mdctx, pdsactx->md, params))
        goto error;

    return 1;

 error:
    EVP_MD_CTX_free(pdsactx->mdctx);
    pdsactx->mdctx = NULL;
    return 0;
}

static int dsa_digest_sign_init(void *vpdsactx, const char *mdname,
                                void *vdsa, const OSSL_PARAM params[])
{
    return dsa_digest_signverify_init(vpdsactx, mdname, vdsa, params,
                                      EVP_PKEY_OP_SIGNMSG,
                                      "DSA Digest Sign Init");
}

static int dsa_digest_signverify_update(void *vpdsactx, const unsigned char *data,
                                        size_t datalen)
{
    PROV_DSA_CTX *pdsactx = (PROV_DSA_CTX *)vpdsactx;

    if (pdsactx == NULL)
        return 0;
    /* Sigalg implementations shouldn't do digest_sign */
    if (pdsactx->flag_sigalg)
        return 0;

    return dsa_signverify_message_update(vpdsactx, data, datalen);
}

static int dsa_digest_sign_final(void *vpdsactx, unsigned char *sig,
                                 size_t *siglen, size_t sigsize)
{
    PROV_DSA_CTX *pdsactx = (PROV_DSA_CTX *)vpdsactx;
    int ok = 0;

    if (pdsactx == NULL)
        return 0;
    /* Sigalg implementations shouldn't do digest_sign */
    if (pdsactx->flag_sigalg)
        return 0;

    ok = dsa_sign_message_final(pdsactx, sig, siglen, sigsize);

    pdsactx->flag_allow_md = 1;

    return ok;
}

static int dsa_digest_verify_init(void *vpdsactx, const char *mdname,
                                  void *vdsa, const OSSL_PARAM params[])
{
    return dsa_digest_signverify_init(vpdsactx, mdname, vdsa, params,
                                      EVP_PKEY_OP_VERIFYMSG,
                                      "DSA Digest Verify Init");
}

int dsa_digest_verify_final(void *vpdsactx, const unsigned char *sig,
                            size_t siglen)
{
    PROV_DSA_CTX *pdsactx = (PROV_DSA_CTX *)vpdsactx;
    int ok = 0;

    if (pdsactx == NULL)
        return 0;
    /* Sigalg implementations shouldn't do digest_verify */
    if (pdsactx->flag_sigalg)
        return 0;

    if (dsa_verify_set_sig(pdsactx, sig, siglen))
        ok = dsa_verify_message_final(vpdsactx);

    pdsactx->flag_allow_md = 1;

    return ok;
}

static void dsa_freectx(void *vpdsactx)
{
    PROV_DSA_CTX *ctx = (PROV_DSA_CTX *)vpdsactx;

    EVP_MD_CTX_free(ctx->mdctx);
    EVP_MD_free(ctx->md);
    OPENSSL_free(ctx->sig);
    OPENSSL_free(ctx->propq);
    DSA_free(ctx->dsa);
    OPENSSL_free(ctx);
}

static void *dsa_dupctx(void *vpdsactx)
{
    PROV_DSA_CTX *srcctx = (PROV_DSA_CTX *)vpdsactx;
    PROV_DSA_CTX *dstctx;

    if (!ossl_prov_is_running())
        return NULL;

    dstctx = OPENSSL_zalloc(sizeof(*srcctx));
    if (dstctx == NULL)
        return NULL;

    *dstctx = *srcctx;
    dstctx->dsa = NULL;
    dstctx->propq = NULL;

    if (srcctx->dsa != NULL && !DSA_up_ref(srcctx->dsa))
        goto err;
    dstctx->dsa = srcctx->dsa;

    if (srcctx->md != NULL && !EVP_MD_up_ref(srcctx->md))
        goto err;
    dstctx->md = srcctx->md;

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
    dsa_freectx(dstctx);
    return NULL;
}

static int dsa_get_ctx_params(void *vpdsactx, OSSL_PARAM *params)
{
    PROV_DSA_CTX *pdsactx = (PROV_DSA_CTX *)vpdsactx;
    OSSL_PARAM *p;

    if (pdsactx == NULL)
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_SIGNATURE_PARAM_ALGORITHM_ID);
    if (p != NULL
        && !OSSL_PARAM_set_octet_string(p,
                                        pdsactx->aid_len == 0 ? NULL : pdsactx->aid_buf,
                                        pdsactx->aid_len))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_SIGNATURE_PARAM_DIGEST);
    if (p != NULL && !OSSL_PARAM_set_utf8_string(p, pdsactx->mdname))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_SIGNATURE_PARAM_NONCE_TYPE);
    if (p != NULL && !OSSL_PARAM_set_uint(p, pdsactx->nonce_type))
        return 0;
    if (!OSSL_FIPS_IND_GET_CTX_PARAM(pdsactx, params))
        return 0;

    return 1;
}

static const OSSL_PARAM known_gettable_ctx_params[] = {
    OSSL_PARAM_octet_string(OSSL_SIGNATURE_PARAM_ALGORITHM_ID, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_SIGNATURE_PARAM_DIGEST, NULL, 0),
    OSSL_PARAM_uint(OSSL_SIGNATURE_PARAM_NONCE_TYPE, NULL),
    OSSL_FIPS_IND_GETTABLE_CTX_PARAM()
    OSSL_PARAM_END
};

static const OSSL_PARAM *dsa_gettable_ctx_params(ossl_unused void *ctx,
                                                 ossl_unused void *provctx)
{
    return known_gettable_ctx_params;
}

/**
 * @brief Setup common params for dsa_set_ctx_params and dsa_sigalg_set_ctx_params
 * The caller is responsible for checking |vpdsactx| is not NULL and |params|
 * is not empty.
 */
static int dsa_common_set_ctx_params(void *vpdsactx, const OSSL_PARAM params[])
{
    PROV_DSA_CTX *pdsactx = (PROV_DSA_CTX *)vpdsactx;
    const OSSL_PARAM *p;

    if (!OSSL_FIPS_IND_SET_CTX_PARAM(pdsactx, OSSL_FIPS_IND_SETTABLE0, params,
                                     OSSL_SIGNATURE_PARAM_FIPS_KEY_CHECK))
        return 0;
    if (!OSSL_FIPS_IND_SET_CTX_PARAM(pdsactx, OSSL_FIPS_IND_SETTABLE1, params,
                                     OSSL_SIGNATURE_PARAM_FIPS_DIGEST_CHECK))
        return 0;
    if (!OSSL_FIPS_IND_SET_CTX_PARAM(pdsactx, OSSL_FIPS_IND_SETTABLE2, params,
                                     OSSL_SIGNATURE_PARAM_FIPS_SIGN_CHECK))
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_SIGNATURE_PARAM_NONCE_TYPE);
    if (p != NULL
        && !OSSL_PARAM_get_uint(p, &pdsactx->nonce_type))
        return 0;
    return 1;
}

#define DSA_COMMON_SETTABLE_CTX_PARAMS                                        \
    OSSL_PARAM_uint(OSSL_SIGNATURE_PARAM_NONCE_TYPE, NULL),                   \
    OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_SIGNATURE_PARAM_FIPS_KEY_CHECK)     \
    OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_SIGNATURE_PARAM_FIPS_DIGEST_CHECK)  \
    OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_SIGNATURE_PARAM_FIPS_SIGN_CHECK)    \
    OSSL_PARAM_END

static int dsa_set_ctx_params(void *vpdsactx, const OSSL_PARAM params[])
{
    PROV_DSA_CTX *pdsactx = (PROV_DSA_CTX *)vpdsactx;
    const OSSL_PARAM *p;
    int ret;

    if (pdsactx == NULL)
        return 0;
    if (ossl_param_is_empty(params))
        return 1;

    if ((ret = dsa_common_set_ctx_params(pdsactx, params)) <= 0)
        return ret;

    p = OSSL_PARAM_locate_const(params, OSSL_SIGNATURE_PARAM_DIGEST);
    if (p != NULL) {
        char mdname[OSSL_MAX_NAME_SIZE] = "", *pmdname = mdname;
        char mdprops[OSSL_MAX_PROPQUERY_SIZE] = "", *pmdprops = mdprops;
        const OSSL_PARAM *propsp =
            OSSL_PARAM_locate_const(params,
                                    OSSL_SIGNATURE_PARAM_PROPERTIES);

        if (!OSSL_PARAM_get_utf8_string(p, &pmdname, sizeof(mdname)))
            return 0;
        if (propsp != NULL
            && !OSSL_PARAM_get_utf8_string(propsp, &pmdprops, sizeof(mdprops)))
            return 0;
        if (!dsa_setup_md(pdsactx, mdname, mdprops, "DSA Set Ctx"))
            return 0;
    }
    return 1;
}

static const OSSL_PARAM settable_ctx_params[] = {
    OSSL_PARAM_utf8_string(OSSL_SIGNATURE_PARAM_DIGEST, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_SIGNATURE_PARAM_PROPERTIES, NULL, 0),
    DSA_COMMON_SETTABLE_CTX_PARAMS
};

static const OSSL_PARAM settable_ctx_params_no_digest[] = {
    OSSL_PARAM_END
};

static const OSSL_PARAM *dsa_settable_ctx_params(void *vpdsactx,
                                                 ossl_unused void *provctx)
{
    PROV_DSA_CTX *pdsactx = (PROV_DSA_CTX *)vpdsactx;

    if (pdsactx != NULL && !pdsactx->flag_allow_md)
        return settable_ctx_params_no_digest;
    return settable_ctx_params;
}

static int dsa_get_ctx_md_params(void *vpdsactx, OSSL_PARAM *params)
{
    PROV_DSA_CTX *pdsactx = (PROV_DSA_CTX *)vpdsactx;

    if (pdsactx->mdctx == NULL)
        return 0;

    return EVP_MD_CTX_get_params(pdsactx->mdctx, params);
}

static const OSSL_PARAM *dsa_gettable_ctx_md_params(void *vpdsactx)
{
    PROV_DSA_CTX *pdsactx = (PROV_DSA_CTX *)vpdsactx;

    if (pdsactx->md == NULL)
        return 0;

    return EVP_MD_gettable_ctx_params(pdsactx->md);
}

static int dsa_set_ctx_md_params(void *vpdsactx, const OSSL_PARAM params[])
{
    PROV_DSA_CTX *pdsactx = (PROV_DSA_CTX *)vpdsactx;

    if (pdsactx->mdctx == NULL)
        return 0;

    return EVP_MD_CTX_set_params(pdsactx->mdctx, params);
}

static const OSSL_PARAM *dsa_settable_ctx_md_params(void *vpdsactx)
{
    PROV_DSA_CTX *pdsactx = (PROV_DSA_CTX *)vpdsactx;

    if (pdsactx->md == NULL)
        return 0;

    return EVP_MD_settable_ctx_params(pdsactx->md);
}

const OSSL_DISPATCH ossl_dsa_signature_functions[] = {
    { OSSL_FUNC_SIGNATURE_NEWCTX, (void (*)(void))dsa_newctx },
    { OSSL_FUNC_SIGNATURE_SIGN_INIT, (void (*)(void))dsa_sign_init },
    { OSSL_FUNC_SIGNATURE_SIGN, (void (*)(void))dsa_sign },
    { OSSL_FUNC_SIGNATURE_VERIFY_INIT, (void (*)(void))dsa_verify_init },
    { OSSL_FUNC_SIGNATURE_VERIFY, (void (*)(void))dsa_verify },
    { OSSL_FUNC_SIGNATURE_DIGEST_SIGN_INIT,
      (void (*)(void))dsa_digest_sign_init },
    { OSSL_FUNC_SIGNATURE_DIGEST_SIGN_UPDATE,
      (void (*)(void))dsa_digest_signverify_update },
    { OSSL_FUNC_SIGNATURE_DIGEST_SIGN_FINAL,
      (void (*)(void))dsa_digest_sign_final },
    { OSSL_FUNC_SIGNATURE_DIGEST_VERIFY_INIT,
      (void (*)(void))dsa_digest_verify_init },
    { OSSL_FUNC_SIGNATURE_DIGEST_VERIFY_UPDATE,
      (void (*)(void))dsa_digest_signverify_update },
    { OSSL_FUNC_SIGNATURE_DIGEST_VERIFY_FINAL,
      (void (*)(void))dsa_digest_verify_final },
    { OSSL_FUNC_SIGNATURE_FREECTX, (void (*)(void))dsa_freectx },
    { OSSL_FUNC_SIGNATURE_DUPCTX, (void (*)(void))dsa_dupctx },
    { OSSL_FUNC_SIGNATURE_GET_CTX_PARAMS, (void (*)(void))dsa_get_ctx_params },
    { OSSL_FUNC_SIGNATURE_GETTABLE_CTX_PARAMS,
      (void (*)(void))dsa_gettable_ctx_params },
    { OSSL_FUNC_SIGNATURE_SET_CTX_PARAMS, (void (*)(void))dsa_set_ctx_params },
    { OSSL_FUNC_SIGNATURE_SETTABLE_CTX_PARAMS,
      (void (*)(void))dsa_settable_ctx_params },
    { OSSL_FUNC_SIGNATURE_GET_CTX_MD_PARAMS,
      (void (*)(void))dsa_get_ctx_md_params },
    { OSSL_FUNC_SIGNATURE_GETTABLE_CTX_MD_PARAMS,
      (void (*)(void))dsa_gettable_ctx_md_params },
    { OSSL_FUNC_SIGNATURE_SET_CTX_MD_PARAMS,
      (void (*)(void))dsa_set_ctx_md_params },
    { OSSL_FUNC_SIGNATURE_SETTABLE_CTX_MD_PARAMS,
      (void (*)(void))dsa_settable_ctx_md_params },
    OSSL_DISPATCH_END
};

/* ------------------------------------------------------------------ */

/*
 * So called sigalgs (composite DSA+hash) implemented below.  They
 * are pretty much hard coded.
 */

static OSSL_FUNC_signature_query_key_types_fn dsa_sigalg_query_key_types;
static OSSL_FUNC_signature_settable_ctx_params_fn dsa_sigalg_settable_ctx_params;
static OSSL_FUNC_signature_set_ctx_params_fn dsa_sigalg_set_ctx_params;

/*
 * dsa_sigalg_signverify_init() is almost like dsa_digest_signverify_init(),
 * just doesn't allow fetching an MD from whatever the user chooses.
 */
static int dsa_sigalg_signverify_init(void *vpdsactx, void *vdsa,
                                      OSSL_FUNC_signature_set_ctx_params_fn *set_ctx_params,
                                      const OSSL_PARAM params[],
                                      const char *mdname,
                                      int operation, const char *desc)
{
    PROV_DSA_CTX *pdsactx = (PROV_DSA_CTX *)vpdsactx;

    if (!ossl_prov_is_running())
        return 0;

    if (!dsa_signverify_init(vpdsactx, vdsa, set_ctx_params, params, operation,
                             desc))
        return 0;

    if (!dsa_setup_md(pdsactx, mdname, NULL, desc))
        return 0;

    pdsactx->flag_sigalg = 1;
    pdsactx->flag_allow_md = 0;

    if (pdsactx->mdctx == NULL) {
        pdsactx->mdctx = EVP_MD_CTX_new();
        if (pdsactx->mdctx == NULL)
            goto error;
    }

    if (!EVP_DigestInit_ex2(pdsactx->mdctx, pdsactx->md, params))
        goto error;

    return 1;

 error:
    EVP_MD_CTX_free(pdsactx->mdctx);
    pdsactx->mdctx = NULL;
    return 0;
}

static const char **dsa_sigalg_query_key_types(void)
{
    static const char *keytypes[] = { "DSA", NULL };

    return keytypes;
}

static const OSSL_PARAM settable_sigalg_ctx_params[] = {
    OSSL_PARAM_octet_string(OSSL_SIGNATURE_PARAM_SIGNATURE, NULL, 0),
    DSA_COMMON_SETTABLE_CTX_PARAMS
};

static const OSSL_PARAM *dsa_sigalg_settable_ctx_params(void *vpdsactx,
                                                        ossl_unused void *provctx)
{
    PROV_DSA_CTX *pdsactx = (PROV_DSA_CTX *)vpdsactx;

    if (pdsactx != NULL && pdsactx->operation == EVP_PKEY_OP_VERIFYMSG)
        return settable_sigalg_ctx_params;
    return NULL;
}

static int dsa_sigalg_set_ctx_params(void *vpdsactx, const OSSL_PARAM params[])
{
    PROV_DSA_CTX *pdsactx = (PROV_DSA_CTX *)vpdsactx;
    const OSSL_PARAM *p;
    int ret;

    if (pdsactx == NULL)
        return 0;
    if (ossl_param_is_empty(params))
        return 1;

    if ((ret = dsa_common_set_ctx_params(pdsactx, params)) <= 0)
        return ret;

    if (pdsactx->operation == EVP_PKEY_OP_VERIFYMSG) {
        p = OSSL_PARAM_locate_const(params, OSSL_SIGNATURE_PARAM_SIGNATURE);
        if (p != NULL) {
            OPENSSL_free(pdsactx->sig);
            pdsactx->sig = NULL;
            pdsactx->siglen = 0;
            if (!OSSL_PARAM_get_octet_string(p, (void **)&pdsactx->sig,
                                             0, &pdsactx->siglen))
                return 0;
        }
    }
    return 1;
}

#define IMPL_DSA_SIGALG(md, MD)                                         \
    static OSSL_FUNC_signature_sign_init_fn dsa_##md##_sign_init;       \
    static OSSL_FUNC_signature_sign_message_init_fn                     \
        dsa_##md##_sign_message_init;                                   \
    static OSSL_FUNC_signature_verify_init_fn dsa_##md##_verify_init;   \
    static OSSL_FUNC_signature_verify_message_init_fn                   \
        dsa_##md##_verify_message_init;                                 \
                                                                        \
    static int                                                          \
    dsa_##md##_sign_init(void *vpdsactx, void *vdsa,                    \
                         const OSSL_PARAM params[])                     \
    {                                                                   \
        static const char desc[] = "DSA-" #MD " Sign Init";             \
                                                                        \
        return dsa_sigalg_signverify_init(vpdsactx, vdsa,               \
                                          dsa_sigalg_set_ctx_params,    \
                                          params, #MD,                  \
                                          EVP_PKEY_OP_SIGN,             \
                                          desc);                        \
    }                                                                   \
                                                                        \
    static int                                                          \
    dsa_##md##_sign_message_init(void *vpdsactx, void *vdsa,            \
                                 const OSSL_PARAM params[])             \
    {                                                                   \
        static const char desc[] = "DSA-" #MD " Sign Message Init";     \
                                                                        \
        return dsa_sigalg_signverify_init(vpdsactx, vdsa,               \
                                          dsa_sigalg_set_ctx_params,    \
                                          params, #MD,                  \
                                          EVP_PKEY_OP_SIGNMSG,          \
                                          desc);                        \
    }                                                                   \
                                                                        \
    static int                                                          \
    dsa_##md##_verify_init(void *vpdsactx, void *vdsa,                  \
                           const OSSL_PARAM params[])                   \
    {                                                                   \
        static const char desc[] = "DSA-" #MD " Verify Init";           \
                                                                        \
        return dsa_sigalg_signverify_init(vpdsactx, vdsa,               \
                                          dsa_sigalg_set_ctx_params,    \
                                          params, #MD,                  \
                                          EVP_PKEY_OP_VERIFY,           \
                                          desc);                        \
    }                                                                   \
                                                                        \
    static int                                                          \
    dsa_##md##_verify_message_init(void *vpdsactx, void *vdsa,          \
                                   const OSSL_PARAM params[])           \
    {                                                                   \
        static const char desc[] = "DSA-" #MD " Verify Message Init";   \
                                                                        \
        return dsa_sigalg_signverify_init(vpdsactx, vdsa,               \
                                          dsa_sigalg_set_ctx_params,    \
                                          params, #MD,                  \
                                          EVP_PKEY_OP_VERIFYMSG,        \
                                          desc);                        \
    }                                                                   \
                                                                        \
    const OSSL_DISPATCH ossl_dsa_##md##_signature_functions[] = {       \
        { OSSL_FUNC_SIGNATURE_NEWCTX, (void (*)(void))dsa_newctx },     \
        { OSSL_FUNC_SIGNATURE_SIGN_INIT,                                \
          (void (*)(void))dsa_##md##_sign_init },                       \
        { OSSL_FUNC_SIGNATURE_SIGN, (void (*)(void))dsa_sign },         \
        { OSSL_FUNC_SIGNATURE_SIGN_MESSAGE_INIT,                        \
          (void (*)(void))dsa_##md##_sign_message_init },               \
        { OSSL_FUNC_SIGNATURE_SIGN_MESSAGE_UPDATE,                      \
          (void (*)(void))dsa_signverify_message_update },              \
        { OSSL_FUNC_SIGNATURE_SIGN_MESSAGE_FINAL,                       \
          (void (*)(void))dsa_sign_message_final },                     \
        { OSSL_FUNC_SIGNATURE_VERIFY_INIT,                              \
          (void (*)(void))dsa_##md##_verify_init },                     \
        { OSSL_FUNC_SIGNATURE_VERIFY,                                   \
          (void (*)(void))dsa_verify },                                 \
        { OSSL_FUNC_SIGNATURE_VERIFY_MESSAGE_INIT,                      \
          (void (*)(void))dsa_##md##_verify_message_init },             \
        { OSSL_FUNC_SIGNATURE_VERIFY_MESSAGE_UPDATE,                    \
          (void (*)(void))dsa_signverify_message_update },              \
        { OSSL_FUNC_SIGNATURE_VERIFY_MESSAGE_FINAL,                     \
          (void (*)(void))dsa_verify_message_final },                   \
        { OSSL_FUNC_SIGNATURE_FREECTX, (void (*)(void))dsa_freectx },   \
        { OSSL_FUNC_SIGNATURE_DUPCTX, (void (*)(void))dsa_dupctx },     \
        { OSSL_FUNC_SIGNATURE_QUERY_KEY_TYPES,                          \
          (void (*)(void))dsa_sigalg_query_key_types },                 \
        { OSSL_FUNC_SIGNATURE_GET_CTX_PARAMS,                           \
          (void (*)(void))dsa_get_ctx_params },                         \
        { OSSL_FUNC_SIGNATURE_GETTABLE_CTX_PARAMS,                      \
          (void (*)(void))dsa_gettable_ctx_params },                    \
        { OSSL_FUNC_SIGNATURE_SET_CTX_PARAMS,                           \
          (void (*)(void))dsa_sigalg_set_ctx_params },                  \
        { OSSL_FUNC_SIGNATURE_SETTABLE_CTX_PARAMS,                      \
          (void (*)(void))dsa_sigalg_settable_ctx_params },             \
        OSSL_DISPATCH_END                                               \
    }

IMPL_DSA_SIGALG(sha1, SHA1);
IMPL_DSA_SIGALG(sha224, SHA2-224);
IMPL_DSA_SIGALG(sha256, SHA2-256);
IMPL_DSA_SIGALG(sha384, SHA2-384);
IMPL_DSA_SIGALG(sha512, SHA2-512);
IMPL_DSA_SIGALG(sha3_224, SHA3-224);
IMPL_DSA_SIGALG(sha3_256, SHA3-256);
IMPL_DSA_SIGALG(sha3_384, SHA3-384);
IMPL_DSA_SIGALG(sha3_512, SHA3-512);
