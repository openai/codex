/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * ECDSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <string.h> /* memcpy */
#include <openssl/crypto.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/dsa.h>
#include <openssl/params.h>
#include <openssl/evp.h>
#include <openssl/err.h>
#include <openssl/proverr.h>
#include "internal/nelem.h"
#include "internal/sizes.h"
#include "internal/cryptlib.h"
#include "internal/deterministic_nonce.h"
#include "prov/providercommon.h"
#include "prov/implementations.h"
#include "prov/provider_ctx.h"
#include "prov/securitycheck.h"
#include "prov/der_ec.h"
#include "crypto/ec.h"

static OSSL_FUNC_signature_newctx_fn ecdsa_newctx;
static OSSL_FUNC_signature_sign_init_fn ecdsa_sign_init;
static OSSL_FUNC_signature_verify_init_fn ecdsa_verify_init;
static OSSL_FUNC_signature_sign_fn ecdsa_sign;
static OSSL_FUNC_signature_sign_message_update_fn ecdsa_signverify_message_update;
static OSSL_FUNC_signature_sign_message_final_fn ecdsa_sign_message_final;
static OSSL_FUNC_signature_verify_fn ecdsa_verify;
static OSSL_FUNC_signature_verify_message_update_fn ecdsa_signverify_message_update;
static OSSL_FUNC_signature_verify_message_final_fn ecdsa_verify_message_final;
static OSSL_FUNC_signature_digest_sign_init_fn ecdsa_digest_sign_init;
static OSSL_FUNC_signature_digest_sign_update_fn ecdsa_digest_signverify_update;
static OSSL_FUNC_signature_digest_sign_final_fn ecdsa_digest_sign_final;
static OSSL_FUNC_signature_digest_verify_init_fn ecdsa_digest_verify_init;
static OSSL_FUNC_signature_digest_verify_update_fn ecdsa_digest_signverify_update;
static OSSL_FUNC_signature_digest_verify_final_fn ecdsa_digest_verify_final;
static OSSL_FUNC_signature_freectx_fn ecdsa_freectx;
static OSSL_FUNC_signature_dupctx_fn ecdsa_dupctx;
static OSSL_FUNC_signature_query_key_types_fn ecdsa_sigalg_query_key_types;
static OSSL_FUNC_signature_get_ctx_params_fn ecdsa_get_ctx_params;
static OSSL_FUNC_signature_gettable_ctx_params_fn ecdsa_gettable_ctx_params;
static OSSL_FUNC_signature_set_ctx_params_fn ecdsa_set_ctx_params;
static OSSL_FUNC_signature_settable_ctx_params_fn ecdsa_settable_ctx_params;
static OSSL_FUNC_signature_get_ctx_md_params_fn ecdsa_get_ctx_md_params;
static OSSL_FUNC_signature_gettable_ctx_md_params_fn ecdsa_gettable_ctx_md_params;
static OSSL_FUNC_signature_set_ctx_md_params_fn ecdsa_set_ctx_md_params;
static OSSL_FUNC_signature_settable_ctx_md_params_fn ecdsa_settable_ctx_md_params;
static OSSL_FUNC_signature_set_ctx_params_fn ecdsa_sigalg_set_ctx_params;
static OSSL_FUNC_signature_settable_ctx_params_fn ecdsa_sigalg_settable_ctx_params;

/*
 * What's passed as an actual key is defined by the KEYMGMT interface.
 * We happen to know that our KEYMGMT simply passes DSA structures, so
 * we use that here too.
 */

typedef struct {
    OSSL_LIB_CTX *libctx;
    char *propq;
    EC_KEY *ec;
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

    /* The Algorithm Identifier of the combined signature algorithm */
    unsigned char aid_buf[OSSL_MAX_ALGORITHM_ID_SIZE];
    size_t  aid_len;

    /* main digest */
    char mdname[OSSL_MAX_NAME_SIZE];
    EVP_MD *md;
    EVP_MD_CTX *mdctx;
    size_t mdsize;

    /* Signature, for verification */
    unsigned char *sig;
    size_t siglen;

    /*
     * Internally used to cache the results of calling the EC group
     * sign_setup() methods which are then passed to the sign operation.
     * This is used by CAVS failure tests to terminate a loop if the signature
     * is not valid.
     * This could of also been done with a simple flag.
     */
    BIGNUM *kinv;
    BIGNUM *r;
#if !defined(OPENSSL_NO_ACVP_TESTS)
    /*
     * This indicates that KAT (CAVS) test is running. Externally an app will
     * override the random callback such that the generated private key and k
     * are known.
     * Normal operation will loop to choose a new k if the signature is not
     * valid - but for this mode of operation it forces a failure instead.
     */
    unsigned int kattest;
#endif
#ifdef FIPS_MODULE
    /*
     * FIPS 140-3 IG 2.4.B mandates that verification based on a digest of a
     * message is not permitted.  However, signing based on a digest is still
     * permitted.
     */
    int verify_message;
#endif
    /* If this is set then the generated k is not random */
    unsigned int nonce_type;
    OSSL_FIPS_IND_DECLARE
} PROV_ECDSA_CTX;

static void *ecdsa_newctx(void *provctx, const char *propq)
{
    PROV_ECDSA_CTX *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(PROV_ECDSA_CTX));
    if (ctx == NULL)
        return NULL;

    OSSL_FIPS_IND_INIT(ctx)
    ctx->flag_allow_md = 1;
#ifdef FIPS_MODULE
    ctx->verify_message = 1;
#endif
    ctx->libctx = PROV_LIBCTX_OF(provctx);
    if (propq != NULL && (ctx->propq = OPENSSL_strdup(propq)) == NULL) {
        OPENSSL_free(ctx);
        ctx = NULL;
    }
    return ctx;
}

static int ecdsa_setup_md(PROV_ECDSA_CTX *ctx,
                          const char *mdname, const char *mdprops,
                          const char *desc)
{
    EVP_MD *md = NULL;
    size_t mdname_len;
    int md_nid, md_size;
    WPACKET pkt;
    unsigned char *aid = NULL;

    if (mdname == NULL)
        return 1;

    mdname_len = strlen(mdname);
    if (mdname_len >= sizeof(ctx->mdname)) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_DIGEST,
                       "%s exceeds name buffer length", mdname);
        return 0;
    }
    if (mdprops == NULL)
        mdprops = ctx->propq;
    md = EVP_MD_fetch(ctx->libctx, mdname, mdprops);
    if (md == NULL) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_DIGEST,
                       "%s could not be fetched", mdname);
        return 0;
    }
    md_size = EVP_MD_get_size(md);
    if (md_size <= 0) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_DIGEST,
                       "%s has invalid md size %d", mdname, md_size);
        goto err;
    }
    md_nid = ossl_digest_get_approved_nid(md);
#ifdef FIPS_MODULE
    if (md_nid == NID_undef) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_DIGEST_NOT_ALLOWED,
                       "digest=%s", mdname);
        goto err;
    }
#endif
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
        if (ctx->mdname[0] != '\0' && !EVP_MD_is_a(md, ctx->mdname)) {
            ERR_raise_data(ERR_LIB_PROV, PROV_R_DIGEST_NOT_ALLOWED,
                           "digest %s != %s", mdname, ctx->mdname);
            goto err;
        }
        EVP_MD_free(md);
        return 1;
    }

    EVP_MD_CTX_free(ctx->mdctx);
    EVP_MD_free(ctx->md);

    ctx->aid_len = 0;
#ifndef FIPS_MODULE
    if (md_nid != NID_undef) {
#else
    {
#endif
        if (WPACKET_init_der(&pkt, ctx->aid_buf, sizeof(ctx->aid_buf))
            && ossl_DER_w_algorithmIdentifier_ECDSA_with_MD(&pkt, -1, ctx->ec,
                                                            md_nid)
            && WPACKET_finish(&pkt)) {
            WPACKET_get_total_written(&pkt, &ctx->aid_len);
            aid = WPACKET_get_curr(&pkt);
        }
        WPACKET_cleanup(&pkt);
        if (aid != NULL && ctx->aid_len != 0)
            memmove(ctx->aid_buf, aid, ctx->aid_len);
    }

    ctx->mdctx = NULL;
    ctx->md = md;
    ctx->mdsize = (size_t)md_size;
    OPENSSL_strlcpy(ctx->mdname, mdname, sizeof(ctx->mdname));

    return 1;
 err:
    EVP_MD_free(md);
    return 0;
}

static int
ecdsa_signverify_init(PROV_ECDSA_CTX *ctx, void *ec,
                      OSSL_FUNC_signature_set_ctx_params_fn *set_ctx_params,
                      const OSSL_PARAM params[], int operation,
                      const char *desc)
{
    if (!ossl_prov_is_running()
            || ctx == NULL)
        return 0;

    if (ec == NULL && ctx->ec == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_NO_KEY_SET);
        return 0;
    }

    if (ec != NULL) {
        if (!EC_KEY_up_ref(ec))
            return 0;
        EC_KEY_free(ctx->ec);
        ctx->ec = ec;
    }

    ctx->operation = operation;

    OSSL_FIPS_IND_SET_APPROVED(ctx)
    if (!set_ctx_params(ctx, params))
        return 0;
#ifdef FIPS_MODULE
    if (!ossl_fips_ind_ec_key_check(OSSL_FIPS_IND_GET(ctx),
                                    OSSL_FIPS_IND_SETTABLE0, ctx->libctx,
                                    EC_KEY_get0_group(ctx->ec), desc,
                                    (operation & (EVP_PKEY_OP_SIGN
                                                  | EVP_PKEY_OP_SIGNMSG)) != 0))
        return 0;
#endif
    return 1;
}

static int ecdsa_sign_init(void *vctx, void *ec, const OSSL_PARAM params[])
{
    PROV_ECDSA_CTX *ctx = (PROV_ECDSA_CTX *)vctx;

#ifdef FIPS_MODULE
    ctx->verify_message = 1;
#endif
    return ecdsa_signverify_init(ctx, ec, ecdsa_set_ctx_params, params,
                                 EVP_PKEY_OP_SIGN, "ECDSA Sign Init");
}

/*
 * Sign tbs without digesting it first.  This is suitable for "primitive"
 * signing and signing the digest of a message.
 */
static int ecdsa_sign_directly(void *vctx,
                               unsigned char *sig, size_t *siglen, size_t sigsize,
                               const unsigned char *tbs, size_t tbslen)
{
    PROV_ECDSA_CTX *ctx = (PROV_ECDSA_CTX *)vctx;
    int ret;
    unsigned int sltmp;
    size_t ecsize = ECDSA_size(ctx->ec);

    if (!ossl_prov_is_running())
        return 0;

    if (sig == NULL) {
        *siglen = ecsize;
        return 1;
    }

#if !defined(OPENSSL_NO_ACVP_TESTS)
    if (ctx->kattest && !ECDSA_sign_setup(ctx->ec, NULL, &ctx->kinv, &ctx->r))
        return 0;
#endif

    if (sigsize < (size_t)ecsize)
        return 0;

    if (ctx->mdsize != 0 && tbslen != ctx->mdsize)
        return 0;

    if (ctx->nonce_type != 0) {
        const char *mdname = NULL;

        if (ctx->mdname[0] != '\0')
            mdname = ctx->mdname;
        ret = ossl_ecdsa_deterministic_sign(tbs, tbslen, sig, &sltmp,
                                            ctx->ec, ctx->nonce_type,
                                            mdname,
                                            ctx->libctx, ctx->propq);
    } else {
        ret = ECDSA_sign_ex(0, tbs, tbslen, sig, &sltmp, ctx->kinv, ctx->r,
                            ctx->ec);
    }
    if (ret <= 0)
        return 0;

    *siglen = sltmp;
    return 1;
}

static int ecdsa_signverify_message_update(void *vctx,
                                         const unsigned char *data,
                                         size_t datalen)
{
    PROV_ECDSA_CTX *ctx = (PROV_ECDSA_CTX *)vctx;

    if (ctx == NULL)
        return 0;

    return EVP_DigestUpdate(ctx->mdctx, data, datalen);
}

static int ecdsa_sign_message_final(void *vctx, unsigned char *sig,
                                  size_t *siglen, size_t sigsize)
{
    PROV_ECDSA_CTX *ctx = (PROV_ECDSA_CTX *)vctx;
    unsigned char digest[EVP_MAX_MD_SIZE];
    unsigned int dlen = 0;

    if (!ossl_prov_is_running() || ctx == NULL)
        return 0;
    if (ctx->mdctx == NULL)
        return 0;
    /*
     * If sig is NULL then we're just finding out the sig size. Other fields
     * are ignored. Defer to ecdsa_sign.
     */
    if (sig != NULL
        && !EVP_DigestFinal_ex(ctx->mdctx, digest, &dlen))
        return 0;
    return ecdsa_sign_directly(vctx, sig, siglen, sigsize, digest, dlen);
}

/*
 * If signing a message, digest tbs and sign the result.
 * Otherwise, sign tbs directly.
 */
static int ecdsa_sign(void *vctx, unsigned char *sig, size_t *siglen,
                    size_t sigsize, const unsigned char *tbs, size_t tbslen)
{
    PROV_ECDSA_CTX *ctx = (PROV_ECDSA_CTX *)vctx;

    if (ctx->operation == EVP_PKEY_OP_SIGNMSG) {
        /*
         * If |sig| is NULL, the caller is only looking for the sig length.
         * DO NOT update the input in this case.
         */
        if (sig == NULL)
            return ecdsa_sign_message_final(ctx, sig, siglen, sigsize);

        if (ecdsa_signverify_message_update(ctx, tbs, tbslen) <= 0)
            return 0;
        return ecdsa_sign_message_final(ctx, sig, siglen, sigsize);
    }
    return ecdsa_sign_directly(ctx, sig, siglen, sigsize, tbs, tbslen);
}

static int ecdsa_verify_init(void *vctx, void *ec, const OSSL_PARAM params[])
{
    PROV_ECDSA_CTX *ctx = (PROV_ECDSA_CTX *)vctx;

#ifdef FIPS_MODULE
    ctx->verify_message = 0;
#endif
    return ecdsa_signverify_init(ctx, ec, ecdsa_set_ctx_params, params,
                                 EVP_PKEY_OP_VERIFY, "ECDSA Verify Init");
}

static int ecdsa_verify_directly(void *vctx,
                                 const unsigned char *sig, size_t siglen,
                                 const unsigned char *tbs, size_t tbslen)
{
    PROV_ECDSA_CTX *ctx = (PROV_ECDSA_CTX *)vctx;

    if (!ossl_prov_is_running() || (ctx->mdsize != 0 && tbslen != ctx->mdsize))
        return 0;

    return ECDSA_verify(0, tbs, tbslen, sig, siglen, ctx->ec);
}

static int ecdsa_verify_set_sig(void *vctx,
                                const unsigned char *sig, size_t siglen)
{
    PROV_ECDSA_CTX *ctx = (PROV_ECDSA_CTX *)vctx;
    OSSL_PARAM params[2];

    params[0] =
        OSSL_PARAM_construct_octet_string(OSSL_SIGNATURE_PARAM_SIGNATURE,
                                          (unsigned char *)sig, siglen);
    params[1] = OSSL_PARAM_construct_end();
    return ecdsa_sigalg_set_ctx_params(ctx, params);
}

static int ecdsa_verify_message_final(void *vctx)
{
    PROV_ECDSA_CTX *ctx = (PROV_ECDSA_CTX *)vctx;
    unsigned char digest[EVP_MAX_MD_SIZE];
    unsigned int dlen = 0;

    if (!ossl_prov_is_running() || ctx == NULL || ctx->mdctx == NULL)
        return 0;

    /*
     * The digests used here are all known (see ecdsa_get_md_nid()), so they
     * should not exceed the internal buffer size of EVP_MAX_MD_SIZE.
     */
    if (!EVP_DigestFinal_ex(ctx->mdctx, digest, &dlen))
        return 0;

    return ecdsa_verify_directly(vctx, ctx->sig, ctx->siglen,
                               digest, dlen);
}

/*
 * If verifying a message, digest tbs and verify the result.
 * Otherwise, verify tbs directly.
 */
static int ecdsa_verify(void *vctx,
                      const unsigned char *sig, size_t siglen,
                      const unsigned char *tbs, size_t tbslen)
{
    PROV_ECDSA_CTX *ctx = (PROV_ECDSA_CTX *)vctx;

    if (ctx->operation == EVP_PKEY_OP_VERIFYMSG) {
        if (ecdsa_verify_set_sig(ctx, sig, siglen) <= 0)
            return 0;
        if (ecdsa_signverify_message_update(ctx, tbs, tbslen) <= 0)
            return 0;
        return ecdsa_verify_message_final(ctx);
    }
    return ecdsa_verify_directly(ctx, sig, siglen, tbs, tbslen);
}

/* DigestSign/DigestVerify wrappers */

static int ecdsa_digest_signverify_init(void *vctx, const char *mdname,
                                        void *ec, const OSSL_PARAM params[],
                                        int operation, const char *desc)
{
    PROV_ECDSA_CTX *ctx = (PROV_ECDSA_CTX *)vctx;

    if (!ossl_prov_is_running())
        return 0;

#ifdef FIPS_MODULE
    ctx->verify_message = 1;
#endif
    if (!ecdsa_signverify_init(vctx, ec, ecdsa_set_ctx_params, params,
                               operation, desc))
        return 0;

    if (mdname != NULL
        /* was ecdsa_setup_md already called in ecdsa_signverify_init()? */
        && (mdname[0] == '\0' || OPENSSL_strcasecmp(ctx->mdname, mdname) != 0)
        && !ecdsa_setup_md(ctx, mdname, NULL, desc))
        return 0;

    ctx->flag_allow_md = 0;

    if (ctx->mdctx == NULL) {
        ctx->mdctx = EVP_MD_CTX_new();
        if (ctx->mdctx == NULL)
            goto error;
    }

    if (!EVP_DigestInit_ex2(ctx->mdctx, ctx->md, params))
        goto error;
    return 1;
error:
    EVP_MD_CTX_free(ctx->mdctx);
    ctx->mdctx = NULL;
    return 0;
}

static int ecdsa_digest_sign_init(void *vctx, const char *mdname, void *ec,
                                  const OSSL_PARAM params[])
{
    return ecdsa_digest_signverify_init(vctx, mdname, ec, params,
                                        EVP_PKEY_OP_SIGNMSG,
                                        "ECDSA Digest Sign Init");
}

static int ecdsa_digest_signverify_update(void *vctx, const unsigned char *data,
                                          size_t datalen)
{
    PROV_ECDSA_CTX *ctx = (PROV_ECDSA_CTX *)vctx;

    if (ctx == NULL || ctx->mdctx == NULL)
        return 0;
    /* Sigalg implementations shouldn't do digest_sign */
    if (ctx->flag_sigalg)
        return 0;

    return ecdsa_signverify_message_update(vctx, data, datalen);
}

int ecdsa_digest_sign_final(void *vctx, unsigned char *sig, size_t *siglen,
                            size_t sigsize)
{
    PROV_ECDSA_CTX *ctx = (PROV_ECDSA_CTX *)vctx;
    int ok = 0;

    if (ctx == NULL)
        return 0;
    /* Sigalg implementations shouldn't do digest_sign */
    if (ctx->flag_sigalg)
        return 0;

    ok = ecdsa_sign_message_final(ctx, sig, siglen, sigsize);

    ctx->flag_allow_md = 1;

    return ok;
}

static int ecdsa_digest_verify_init(void *vctx, const char *mdname, void *ec,
                                    const OSSL_PARAM params[])
{
    return ecdsa_digest_signverify_init(vctx, mdname, ec, params,
                                        EVP_PKEY_OP_VERIFYMSG,
                                        "ECDSA Digest Verify Init");
}

int ecdsa_digest_verify_final(void *vctx, const unsigned char *sig,
                              size_t siglen)
{
    PROV_ECDSA_CTX *ctx = (PROV_ECDSA_CTX *)vctx;
    int ok = 0;

    if (!ossl_prov_is_running() || ctx == NULL || ctx->mdctx == NULL)
        return 0;

    /* Sigalg implementations shouldn't do digest_verify */
    if (ctx->flag_sigalg)
        return 0;

    if (ecdsa_verify_set_sig(ctx, sig, siglen))
        ok = ecdsa_verify_message_final(ctx);

    ctx->flag_allow_md = 1;

    return ok;
}

static void ecdsa_freectx(void *vctx)
{
    PROV_ECDSA_CTX *ctx = (PROV_ECDSA_CTX *)vctx;

    EVP_MD_CTX_free(ctx->mdctx);
    EVP_MD_free(ctx->md);
    OPENSSL_free(ctx->propq);
    OPENSSL_free(ctx->sig);
    EC_KEY_free(ctx->ec);
    BN_clear_free(ctx->kinv);
    BN_clear_free(ctx->r);
    OPENSSL_free(ctx);
}

static void *ecdsa_dupctx(void *vctx)
{
    PROV_ECDSA_CTX *srcctx = (PROV_ECDSA_CTX *)vctx;
    PROV_ECDSA_CTX *dstctx;

    if (!ossl_prov_is_running())
        return NULL;

    dstctx = OPENSSL_zalloc(sizeof(*srcctx));
    if (dstctx == NULL)
        return NULL;

    *dstctx = *srcctx;
    dstctx->ec = NULL;
    dstctx->propq = NULL;

    if (srcctx->ec != NULL && !EC_KEY_up_ref(srcctx->ec))
        goto err;
    /* Test KATS should not need to be supported */
    if (srcctx->kinv != NULL || srcctx->r != NULL)
        goto err;
    dstctx->ec = srcctx->ec;

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
    ecdsa_freectx(dstctx);
    return NULL;
}

static int ecdsa_get_ctx_params(void *vctx, OSSL_PARAM *params)
{
    PROV_ECDSA_CTX *ctx = (PROV_ECDSA_CTX *)vctx;
    OSSL_PARAM *p;

    if (ctx == NULL)
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_SIGNATURE_PARAM_ALGORITHM_ID);
    if (p != NULL && !OSSL_PARAM_set_octet_string(p,
                                                  ctx->aid_len == 0 ? NULL : ctx->aid_buf,
                                                  ctx->aid_len))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_SIGNATURE_PARAM_DIGEST_SIZE);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, ctx->mdsize))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_SIGNATURE_PARAM_DIGEST);
    if (p != NULL && !OSSL_PARAM_set_utf8_string(p, ctx->md == NULL
                                                    ? ctx->mdname
                                                    : EVP_MD_get0_name(ctx->md)))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_SIGNATURE_PARAM_NONCE_TYPE);
    if (p != NULL && !OSSL_PARAM_set_uint(p, ctx->nonce_type))
        return 0;

#ifdef FIPS_MODULE
    p = OSSL_PARAM_locate(params, OSSL_SIGNATURE_PARAM_FIPS_VERIFY_MESSAGE);
    if (p != NULL && !OSSL_PARAM_set_uint(p, ctx->verify_message))
        return 0;
#endif

    if (!OSSL_FIPS_IND_GET_CTX_PARAM(ctx, params))
        return 0;
    return 1;
}

static const OSSL_PARAM known_gettable_ctx_params[] = {
    OSSL_PARAM_octet_string(OSSL_SIGNATURE_PARAM_ALGORITHM_ID, NULL, 0),
    OSSL_PARAM_size_t(OSSL_SIGNATURE_PARAM_DIGEST_SIZE, NULL),
    OSSL_PARAM_utf8_string(OSSL_SIGNATURE_PARAM_DIGEST, NULL, 0),
    OSSL_PARAM_uint(OSSL_SIGNATURE_PARAM_NONCE_TYPE, NULL),
#ifdef FIPS_MODULE
    OSSL_PARAM_uint(OSSL_SIGNATURE_PARAM_FIPS_VERIFY_MESSAGE, NULL),
#endif
    OSSL_FIPS_IND_GETTABLE_CTX_PARAM()
    OSSL_PARAM_END
};

static const OSSL_PARAM *ecdsa_gettable_ctx_params(ossl_unused void *vctx,
                                                   ossl_unused void *provctx)
{
    return known_gettable_ctx_params;
}

/**
 * @brief Set up common params for ecdsa_set_ctx_params and
 * ecdsa_sigalg_set_ctx_params. The caller is responsible for checking |vctx| is
 * not NULL and |params| is not empty.
 */
static int ecdsa_common_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    PROV_ECDSA_CTX *ctx = (PROV_ECDSA_CTX *)vctx;
    const OSSL_PARAM *p;

    if (!OSSL_FIPS_IND_SET_CTX_PARAM(ctx, OSSL_FIPS_IND_SETTABLE0, params,
                                     OSSL_SIGNATURE_PARAM_FIPS_KEY_CHECK))
        return 0;
    if (!OSSL_FIPS_IND_SET_CTX_PARAM(ctx, OSSL_FIPS_IND_SETTABLE1, params,
                                     OSSL_SIGNATURE_PARAM_FIPS_DIGEST_CHECK))
        return 0;

#if !defined(OPENSSL_NO_ACVP_TESTS)
    p = OSSL_PARAM_locate_const(params, OSSL_SIGNATURE_PARAM_KAT);
    if (p != NULL && !OSSL_PARAM_get_uint(p, &ctx->kattest))
        return 0;
#endif

    p = OSSL_PARAM_locate_const(params, OSSL_SIGNATURE_PARAM_NONCE_TYPE);
    if (p != NULL
        && !OSSL_PARAM_get_uint(p, &ctx->nonce_type))
        return 0;
    return 1;
}

#define ECDSA_COMMON_SETTABLE_CTX_PARAMS                                      \
    OSSL_PARAM_uint(OSSL_SIGNATURE_PARAM_KAT, NULL),                          \
    OSSL_PARAM_uint(OSSL_SIGNATURE_PARAM_NONCE_TYPE, NULL),                   \
    OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_SIGNATURE_PARAM_FIPS_KEY_CHECK)     \
    OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_SIGNATURE_PARAM_FIPS_DIGEST_CHECK)  \
    OSSL_PARAM_END

static int ecdsa_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    PROV_ECDSA_CTX *ctx = (PROV_ECDSA_CTX *)vctx;
    const OSSL_PARAM *p;
    size_t mdsize = 0;
    int ret;

    if (ctx == NULL)
        return 0;
    if (ossl_param_is_empty(params))
        return 1;

    if ((ret = ecdsa_common_set_ctx_params(ctx, params)) <= 0)
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
        if (!ecdsa_setup_md(ctx, mdname, mdprops, "ECDSA Set Ctx"))
            return 0;
    }

    p = OSSL_PARAM_locate_const(params, OSSL_SIGNATURE_PARAM_DIGEST_SIZE);
    if (p != NULL) {
        if (!OSSL_PARAM_get_size_t(p, &mdsize)
            || (!ctx->flag_allow_md && mdsize != ctx->mdsize))
            return 0;
        ctx->mdsize = mdsize;
    }
    return 1;
}

static const OSSL_PARAM settable_ctx_params[] = {
    OSSL_PARAM_utf8_string(OSSL_SIGNATURE_PARAM_DIGEST, NULL, 0),
    OSSL_PARAM_size_t(OSSL_SIGNATURE_PARAM_DIGEST_SIZE, NULL),
    OSSL_PARAM_utf8_string(OSSL_SIGNATURE_PARAM_PROPERTIES, NULL, 0),
    ECDSA_COMMON_SETTABLE_CTX_PARAMS
};

static const OSSL_PARAM *ecdsa_settable_ctx_params(void *vctx,
                                                   ossl_unused void *provctx)
{
    return settable_ctx_params;
}

static int ecdsa_get_ctx_md_params(void *vctx, OSSL_PARAM *params)
{
    PROV_ECDSA_CTX *ctx = (PROV_ECDSA_CTX *)vctx;

    if (ctx->mdctx == NULL)
        return 0;

    return EVP_MD_CTX_get_params(ctx->mdctx, params);
}

static const OSSL_PARAM *ecdsa_gettable_ctx_md_params(void *vctx)
{
    PROV_ECDSA_CTX *ctx = (PROV_ECDSA_CTX *)vctx;

    if (ctx->md == NULL)
        return 0;

    return EVP_MD_gettable_ctx_params(ctx->md);
}

static int ecdsa_set_ctx_md_params(void *vctx, const OSSL_PARAM params[])
{
    PROV_ECDSA_CTX *ctx = (PROV_ECDSA_CTX *)vctx;

    if (ctx->mdctx == NULL)
        return 0;

    return EVP_MD_CTX_set_params(ctx->mdctx, params);
}

static const OSSL_PARAM *ecdsa_settable_ctx_md_params(void *vctx)
{
    PROV_ECDSA_CTX *ctx = (PROV_ECDSA_CTX *)vctx;

    if (ctx->md == NULL)
        return 0;

    return EVP_MD_settable_ctx_params(ctx->md);
}

const OSSL_DISPATCH ossl_ecdsa_signature_functions[] = {
    { OSSL_FUNC_SIGNATURE_NEWCTX, (void (*)(void))ecdsa_newctx },
    { OSSL_FUNC_SIGNATURE_SIGN_INIT, (void (*)(void))ecdsa_sign_init },
    { OSSL_FUNC_SIGNATURE_SIGN, (void (*)(void))ecdsa_sign },
    { OSSL_FUNC_SIGNATURE_VERIFY_INIT, (void (*)(void))ecdsa_verify_init },
    { OSSL_FUNC_SIGNATURE_VERIFY, (void (*)(void))ecdsa_verify },
    { OSSL_FUNC_SIGNATURE_DIGEST_SIGN_INIT,
      (void (*)(void))ecdsa_digest_sign_init },
    { OSSL_FUNC_SIGNATURE_DIGEST_SIGN_UPDATE,
      (void (*)(void))ecdsa_digest_signverify_update },
    { OSSL_FUNC_SIGNATURE_DIGEST_SIGN_FINAL,
      (void (*)(void))ecdsa_digest_sign_final },
    { OSSL_FUNC_SIGNATURE_DIGEST_VERIFY_INIT,
      (void (*)(void))ecdsa_digest_verify_init },
    { OSSL_FUNC_SIGNATURE_DIGEST_VERIFY_UPDATE,
      (void (*)(void))ecdsa_digest_signverify_update },
    { OSSL_FUNC_SIGNATURE_DIGEST_VERIFY_FINAL,
      (void (*)(void))ecdsa_digest_verify_final },
    { OSSL_FUNC_SIGNATURE_FREECTX, (void (*)(void))ecdsa_freectx },
    { OSSL_FUNC_SIGNATURE_DUPCTX, (void (*)(void))ecdsa_dupctx },
    { OSSL_FUNC_SIGNATURE_GET_CTX_PARAMS, (void (*)(void))ecdsa_get_ctx_params },
    { OSSL_FUNC_SIGNATURE_GETTABLE_CTX_PARAMS,
      (void (*)(void))ecdsa_gettable_ctx_params },
    { OSSL_FUNC_SIGNATURE_SET_CTX_PARAMS, (void (*)(void))ecdsa_set_ctx_params },
    { OSSL_FUNC_SIGNATURE_SETTABLE_CTX_PARAMS,
      (void (*)(void))ecdsa_settable_ctx_params },
    { OSSL_FUNC_SIGNATURE_GET_CTX_MD_PARAMS,
      (void (*)(void))ecdsa_get_ctx_md_params },
    { OSSL_FUNC_SIGNATURE_GETTABLE_CTX_MD_PARAMS,
      (void (*)(void))ecdsa_gettable_ctx_md_params },
    { OSSL_FUNC_SIGNATURE_SET_CTX_MD_PARAMS,
      (void (*)(void))ecdsa_set_ctx_md_params },
    { OSSL_FUNC_SIGNATURE_SETTABLE_CTX_MD_PARAMS,
      (void (*)(void))ecdsa_settable_ctx_md_params },
    OSSL_DISPATCH_END
};

/* ------------------------------------------------------------------ */

/*
 * So called sigalgs (composite ECDSA+hash) implemented below.  They
 * are pretty much hard coded.
 */

static OSSL_FUNC_signature_query_key_types_fn ecdsa_sigalg_query_key_types;
static OSSL_FUNC_signature_settable_ctx_params_fn ecdsa_sigalg_settable_ctx_params;
static OSSL_FUNC_signature_set_ctx_params_fn ecdsa_sigalg_set_ctx_params;

/*
 * ecdsa_sigalg_signverify_init() is almost like ecdsa_digest_signverify_init(),
 * just doesn't allow fetching an MD from whatever the user chooses.
 */
static int ecdsa_sigalg_signverify_init(void *vctx, void *vec,
                                      OSSL_FUNC_signature_set_ctx_params_fn *set_ctx_params,
                                      const OSSL_PARAM params[],
                                      const char *mdname,
                                      int operation, const char *desc)
{
    PROV_ECDSA_CTX *ctx = (PROV_ECDSA_CTX *)vctx;

    if (!ossl_prov_is_running())
        return 0;

    if (!ecdsa_signverify_init(vctx, vec, set_ctx_params, params, operation,
                               desc))
        return 0;

    if (!ecdsa_setup_md(ctx, mdname, NULL, desc))
        return 0;

    ctx->flag_sigalg = 1;
    ctx->flag_allow_md = 0;

    if (ctx->mdctx == NULL) {
        ctx->mdctx = EVP_MD_CTX_new();
        if (ctx->mdctx == NULL)
            goto error;
    }

    if (!EVP_DigestInit_ex2(ctx->mdctx, ctx->md, params))
        goto error;

    return 1;

 error:
    EVP_MD_CTX_free(ctx->mdctx);
    ctx->mdctx = NULL;
    return 0;
}

static const char **ecdsa_sigalg_query_key_types(void)
{
    static const char *keytypes[] = { "EC", NULL };

    return keytypes;
}

static const OSSL_PARAM settable_sigalg_ctx_params[] = {
    OSSL_PARAM_octet_string(OSSL_SIGNATURE_PARAM_SIGNATURE, NULL, 0),
    ECDSA_COMMON_SETTABLE_CTX_PARAMS
};

static const OSSL_PARAM *ecdsa_sigalg_settable_ctx_params(void *vctx,
                                                        ossl_unused void *provctx)
{
    PROV_ECDSA_CTX *ctx = (PROV_ECDSA_CTX *)vctx;

    if (ctx != NULL && ctx->operation == EVP_PKEY_OP_VERIFYMSG)
        return settable_sigalg_ctx_params;
    return NULL;
}

static int ecdsa_sigalg_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    PROV_ECDSA_CTX *ctx = (PROV_ECDSA_CTX *)vctx;
    const OSSL_PARAM *p;
    int ret;

    if (ctx == NULL)
        return 0;
    if (ossl_param_is_empty(params))
        return 1;

    if ((ret = ecdsa_common_set_ctx_params(ctx, params)) <= 0)
        return ret;

    if (ctx->operation == EVP_PKEY_OP_VERIFYMSG) {
        p = OSSL_PARAM_locate_const(params, OSSL_SIGNATURE_PARAM_SIGNATURE);
        if (p != NULL) {
            OPENSSL_free(ctx->sig);
            ctx->sig = NULL;
            ctx->siglen = 0;
            if (!OSSL_PARAM_get_octet_string(p, (void **)&ctx->sig,
                                             0, &ctx->siglen))
                return 0;
        }
    }
    return 1;
}

#define IMPL_ECDSA_SIGALG(md, MD)                                       \
    static OSSL_FUNC_signature_sign_init_fn ecdsa_##md##_sign_init;     \
    static OSSL_FUNC_signature_sign_message_init_fn                     \
        ecdsa_##md##_sign_message_init;                                 \
    static OSSL_FUNC_signature_verify_init_fn ecdsa_##md##_verify_init; \
    static OSSL_FUNC_signature_verify_message_init_fn                   \
        ecdsa_##md##_verify_message_init;                               \
                                                                        \
    static int                                                          \
    ecdsa_##md##_sign_init(void *vctx, void *vec,                       \
                         const OSSL_PARAM params[])                     \
    {                                                                   \
        static const char desc[] = "ECDSA-" #MD " Sign Init";           \
                                                                        \
        return ecdsa_sigalg_signverify_init(vctx, vec,                  \
                                            ecdsa_sigalg_set_ctx_params, \
                                            params, #MD,                \
                                            EVP_PKEY_OP_SIGN,           \
                                            desc);                      \
    }                                                                   \
                                                                        \
    static int                                                          \
    ecdsa_##md##_sign_message_init(void *vctx, void *vec,               \
                                   const OSSL_PARAM params[])           \
    {                                                                   \
        static const char desc[] = "ECDSA-" #MD " Sign Message Init";   \
                                                                        \
        return ecdsa_sigalg_signverify_init(vctx, vec,                  \
                                            ecdsa_sigalg_set_ctx_params, \
                                            params, #MD,                \
                                            EVP_PKEY_OP_SIGNMSG,        \
                                            desc);                      \
    }                                                                   \
                                                                        \
    static int                                                          \
    ecdsa_##md##_verify_init(void *vctx, void *vec,                     \
                           const OSSL_PARAM params[])                   \
    {                                                                   \
        static const char desc[] = "ECDSA-" #MD " Verify Init";         \
                                                                        \
        return ecdsa_sigalg_signverify_init(vctx, vec,                  \
                                            ecdsa_sigalg_set_ctx_params, \
                                            params, #MD,                \
                                            EVP_PKEY_OP_VERIFY,         \
                                            desc);                      \
    }                                                                   \
                                                                        \
    static int                                                          \
    ecdsa_##md##_verify_message_init(void *vctx, void *vec,             \
                                     const OSSL_PARAM params[])         \
    {                                                                   \
        static const char desc[] = "ECDSA-" #MD " Verify Message Init"; \
                                                                        \
        return ecdsa_sigalg_signverify_init(vctx, vec,                  \
                                            ecdsa_sigalg_set_ctx_params, \
                                            params, #MD,                \
                                            EVP_PKEY_OP_VERIFYMSG,      \
                                            desc);                      \
    }                                                                   \
                                                                        \
    const OSSL_DISPATCH ossl_ecdsa_##md##_signature_functions[] = {     \
        { OSSL_FUNC_SIGNATURE_NEWCTX, (void (*)(void))ecdsa_newctx },   \
        { OSSL_FUNC_SIGNATURE_SIGN_INIT,                                \
          (void (*)(void))ecdsa_##md##_sign_init },                     \
        { OSSL_FUNC_SIGNATURE_SIGN, (void (*)(void))ecdsa_sign },       \
        { OSSL_FUNC_SIGNATURE_SIGN_MESSAGE_INIT,                        \
          (void (*)(void))ecdsa_##md##_sign_message_init },             \
        { OSSL_FUNC_SIGNATURE_SIGN_MESSAGE_UPDATE,                      \
          (void (*)(void))ecdsa_signverify_message_update },            \
        { OSSL_FUNC_SIGNATURE_SIGN_MESSAGE_FINAL,                       \
          (void (*)(void))ecdsa_sign_message_final },                   \
        { OSSL_FUNC_SIGNATURE_VERIFY_INIT,                              \
          (void (*)(void))ecdsa_##md##_verify_init },                   \
        { OSSL_FUNC_SIGNATURE_VERIFY,                                   \
          (void (*)(void))ecdsa_verify },                               \
        { OSSL_FUNC_SIGNATURE_VERIFY_MESSAGE_INIT,                      \
          (void (*)(void))ecdsa_##md##_verify_message_init },           \
        { OSSL_FUNC_SIGNATURE_VERIFY_MESSAGE_UPDATE,                    \
          (void (*)(void))ecdsa_signverify_message_update },            \
        { OSSL_FUNC_SIGNATURE_VERIFY_MESSAGE_FINAL,                     \
          (void (*)(void))ecdsa_verify_message_final },                 \
        { OSSL_FUNC_SIGNATURE_FREECTX, (void (*)(void))ecdsa_freectx }, \
        { OSSL_FUNC_SIGNATURE_DUPCTX, (void (*)(void))ecdsa_dupctx },   \
        { OSSL_FUNC_SIGNATURE_QUERY_KEY_TYPES,                          \
          (void (*)(void))ecdsa_sigalg_query_key_types },               \
        { OSSL_FUNC_SIGNATURE_GET_CTX_PARAMS,                           \
          (void (*)(void))ecdsa_get_ctx_params },                       \
        { OSSL_FUNC_SIGNATURE_GETTABLE_CTX_PARAMS,                      \
          (void (*)(void))ecdsa_gettable_ctx_params },                  \
        { OSSL_FUNC_SIGNATURE_SET_CTX_PARAMS,                           \
          (void (*)(void))ecdsa_sigalg_set_ctx_params },                \
        { OSSL_FUNC_SIGNATURE_SETTABLE_CTX_PARAMS,                      \
          (void (*)(void))ecdsa_sigalg_settable_ctx_params },           \
        OSSL_DISPATCH_END                                               \
    }

IMPL_ECDSA_SIGALG(sha1, SHA1);
IMPL_ECDSA_SIGALG(sha224, SHA2-224);
IMPL_ECDSA_SIGALG(sha256, SHA2-256);
IMPL_ECDSA_SIGALG(sha384, SHA2-384);
IMPL_ECDSA_SIGALG(sha512, SHA2-512);
IMPL_ECDSA_SIGALG(sha3_224, SHA3-224);
IMPL_ECDSA_SIGALG(sha3_256, SHA3-256);
IMPL_ECDSA_SIGALG(sha3_384, SHA3-384);
IMPL_ECDSA_SIGALG(sha3_512, SHA3-512);
