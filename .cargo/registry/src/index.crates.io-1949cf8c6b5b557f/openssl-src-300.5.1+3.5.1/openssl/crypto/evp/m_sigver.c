/*
 * Copyright 2006-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/evp.h>
#include <openssl/objects.h>
#include "crypto/evp.h"
#include "internal/provider.h"
#include "internal/numbers.h"   /* includes SIZE_MAX */
#include "evp_local.h"

static int update(EVP_MD_CTX *ctx, const void *data, size_t datalen)
{
    ERR_raise(ERR_LIB_EVP, EVP_R_ONLY_ONESHOT_SUPPORTED);
    return 0;
}

/*
 * If we get the "NULL" md then the name comes back as "UNDEF". We want to use
 * NULL for this.
 */
static const char *canon_mdname(const char *mdname)
{
    if (mdname != NULL && strcmp(mdname, "UNDEF") == 0)
        return NULL;

    return mdname;
}

static int do_sigver_init(EVP_MD_CTX *ctx, EVP_PKEY_CTX **pctx,
                          const EVP_MD *type, const char *mdname,
                          OSSL_LIB_CTX *libctx, const char *props,
                          ENGINE *e, EVP_PKEY *pkey, int ver,
                          const OSSL_PARAM params[])
{
    EVP_PKEY_CTX *locpctx = NULL;
    EVP_SIGNATURE *signature = NULL;
    const char *desc;
    EVP_KEYMGMT *tmp_keymgmt = NULL;
    const OSSL_PROVIDER *tmp_prov = NULL;
    const char *supported_sig = NULL;
    char locmdname[80] = "";     /* 80 chars should be enough */
    void *provkey = NULL;
    int ret, iter, reinit = 1;

    if (!evp_md_ctx_free_algctx(ctx))
        return 0;

    if (ctx->pctx == NULL) {
        reinit = 0;
        if (e == NULL)
            ctx->pctx = EVP_PKEY_CTX_new_from_pkey(libctx, pkey, props);
        else
            ctx->pctx = EVP_PKEY_CTX_new(pkey, e);
    }
    if (ctx->pctx == NULL)
        return 0;

    EVP_MD_CTX_clear_flags(ctx, EVP_MD_CTX_FLAG_FINALISED);

    locpctx = ctx->pctx;
    ERR_set_mark();

    if (evp_pkey_ctx_is_legacy(locpctx))
        goto legacy;

    /* do not reinitialize if pkey is set or operation is different */
    if (reinit
        && (pkey != NULL
            || locpctx->operation != (ver ? EVP_PKEY_OP_VERIFYCTX
                                          : EVP_PKEY_OP_SIGNCTX)
            || (signature = locpctx->op.sig.signature) == NULL
            || locpctx->op.sig.algctx == NULL))
        reinit = 0;

    if (props == NULL)
        props = locpctx->propquery;

    if (locpctx->pkey == NULL) {
        ERR_clear_last_mark();
        ERR_raise(ERR_LIB_EVP, EVP_R_NO_KEY_SET);
        goto err;
    }

    if (!reinit) {
        evp_pkey_ctx_free_old_ops(locpctx);
    } else {
        if (mdname == NULL && type == NULL)
            mdname = canon_mdname(EVP_MD_get0_name(ctx->reqdigest));
        goto reinitialize;
    }

    /*
     * Try to derive the supported signature from |locpctx->keymgmt|.
     */
    if (!ossl_assert(locpctx->pkey->keymgmt == NULL
                     || locpctx->pkey->keymgmt == locpctx->keymgmt)) {
        ERR_clear_last_mark();
        ERR_raise(ERR_LIB_EVP, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    supported_sig = evp_keymgmt_util_query_operation_name(locpctx->keymgmt,
                                                          OSSL_OP_SIGNATURE);
    if (supported_sig == NULL) {
        ERR_clear_last_mark();
        ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
        goto err;
    }

    /*
     * We perform two iterations:
     *
     * 1.  Do the normal signature fetch, using the fetching data given by
     *     the EVP_PKEY_CTX.
     * 2.  Do the provider specific signature fetch, from the same provider
     *     as |ctx->keymgmt|
     *
     * We then try to fetch the keymgmt from the same provider as the
     * signature, and try to export |ctx->pkey| to that keymgmt (when
     * this keymgmt happens to be the same as |ctx->keymgmt|, the export
     * is a no-op, but we call it anyway to not complicate the code even
     * more).
     * If the export call succeeds (returns a non-NULL provider key pointer),
     * we're done and can perform the operation itself.  If not, we perform
     * the second iteration, or jump to legacy.
     */
    for (iter = 1, provkey = NULL; iter < 3 && provkey == NULL; iter++) {
        EVP_KEYMGMT *tmp_keymgmt_tofree = NULL;

        /*
         * If we're on the second iteration, free the results from the first.
         * They are NULL on the first iteration, so no need to check what
         * iteration we're on.
         */
        EVP_SIGNATURE_free(signature);
        EVP_KEYMGMT_free(tmp_keymgmt);

        switch (iter) {
        case 1:
            signature = EVP_SIGNATURE_fetch(locpctx->libctx, supported_sig,
                                            locpctx->propquery);
            if (signature != NULL)
                tmp_prov = EVP_SIGNATURE_get0_provider(signature);
            break;
        case 2:
            tmp_prov = EVP_KEYMGMT_get0_provider(locpctx->keymgmt);
            signature =
                evp_signature_fetch_from_prov((OSSL_PROVIDER *)tmp_prov,
                                              supported_sig, locpctx->propquery);
            if (signature == NULL)
                goto legacy;
            break;
        }
        if (signature == NULL)
            continue;

        /*
         * Ensure that the key is provided, either natively, or as a cached
         * export.  We start by fetching the keymgmt with the same name as
         * |locpctx->pkey|, but from the provider of the signature method, using
         * the same property query as when fetching the signature method.
         * With the keymgmt we found (if we did), we try to export |locpctx->pkey|
         * to it (evp_pkey_export_to_provider() is smart enough to only actually

         * export it if |tmp_keymgmt| is different from |locpctx->pkey|'s keymgmt)
         */
        tmp_keymgmt_tofree = tmp_keymgmt =
            evp_keymgmt_fetch_from_prov((OSSL_PROVIDER *)tmp_prov,
                                        EVP_KEYMGMT_get0_name(locpctx->keymgmt),
                                        locpctx->propquery);
        if (tmp_keymgmt != NULL)
            provkey = evp_pkey_export_to_provider(locpctx->pkey, locpctx->libctx,
                                                  &tmp_keymgmt, locpctx->propquery);
        if (tmp_keymgmt == NULL)
            EVP_KEYMGMT_free(tmp_keymgmt_tofree);
    }

    if (provkey == NULL) {
        EVP_SIGNATURE_free(signature);
        ERR_clear_last_mark();
        ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
        goto err;
    }

    ERR_pop_to_mark();

    /* No more legacy from here down to legacy: */

    locpctx->op.sig.signature = signature;
    locpctx->operation = ver ? EVP_PKEY_OP_VERIFYCTX
                             : EVP_PKEY_OP_SIGNCTX;
    locpctx->op.sig.algctx
        = signature->newctx(ossl_provider_ctx(signature->prov), props);
    if (locpctx->op.sig.algctx == NULL) {
        ERR_raise(ERR_LIB_EVP,  EVP_R_INITIALIZATION_ERROR);
        goto err;
    }

 reinitialize:
    if (pctx != NULL)
        *pctx = locpctx;

    if (type != NULL) {
        ctx->reqdigest = type;
        if (mdname == NULL)
            mdname = canon_mdname(EVP_MD_get0_name(type));
    } else {
        if (mdname == NULL && !reinit) {
            if (evp_keymgmt_util_get_deflt_digest_name(tmp_keymgmt, provkey,
                                                       locmdname,
                                                       sizeof(locmdname)) > 0) {
                mdname = canon_mdname(locmdname);
            }
        }

        if (mdname != NULL) {
            /*
             * We're about to get a new digest so clear anything associated with
             * an old digest.
             */
            evp_md_ctx_clear_digest(ctx, 1, 0);

            /* legacy code support for engines */
            ERR_set_mark();
            /*
             * This might be requested by a later call to EVP_MD_CTX_get0_md().
             * In that case the "explicit fetch" rules apply for that
             * function (as per man pages), i.e. the ref count is not updated
             * so the EVP_MD should not be used beyond the lifetime of the
             * EVP_MD_CTX.
             */
            ctx->fetched_digest = EVP_MD_fetch(locpctx->libctx, mdname, props);
            if (ctx->fetched_digest != NULL) {
                ctx->digest = ctx->reqdigest = ctx->fetched_digest;
            } else {
                /* legacy engine support : remove the mark when this is deleted */
                ctx->reqdigest = ctx->digest = EVP_get_digestbyname(mdname);
                if (ctx->digest == NULL) {
                    (void)ERR_clear_last_mark();
                    ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
                    goto err;
                }
            }
            (void)ERR_pop_to_mark();
        }
    }

    desc = signature->description != NULL ? signature->description : "";
    if (ver) {
        if (signature->digest_verify_init == NULL) {
            ERR_raise_data(ERR_LIB_EVP, EVP_R_PROVIDER_SIGNATURE_NOT_SUPPORTED,
                           "%s digest_verify_init:%s", signature->type_name, desc);
            goto err;
        }
        ret = signature->digest_verify_init(locpctx->op.sig.algctx,
                                            mdname, provkey, params);
    } else {
        if (signature->digest_sign_init == NULL) {
            ERR_raise_data(ERR_LIB_EVP, EVP_R_PROVIDER_SIGNATURE_NOT_SUPPORTED,
                           "%s digest_sign_init:%s", signature->type_name, desc);
            goto err;
        }
        ret = signature->digest_sign_init(locpctx->op.sig.algctx,
                                          mdname, provkey, params);
    }

    /*
     * If the operation was not a success and no digest was found, an error
     * needs to be raised.
     */
    if (ret > 0 || mdname != NULL)
        goto end;
    if (type == NULL)   /* This check is redundant but clarifies matters */
        ERR_raise(ERR_LIB_EVP, EVP_R_NO_DEFAULT_DIGEST);
    ERR_raise_data(ERR_LIB_EVP, EVP_R_PROVIDER_SIGNATURE_FAILURE,
                   ver ? "%s digest_verify_init:%s" : "%s digest_sign_init:%s",
                   signature->type_name, desc);

 err:
    evp_pkey_ctx_free_old_ops(locpctx);
    locpctx->operation = EVP_PKEY_OP_UNDEFINED;
    EVP_KEYMGMT_free(tmp_keymgmt);
    return 0;

 legacy:
    /*
     * If we don't have the full support we need with provided methods,
     * let's go see if legacy does.
     */
    ERR_pop_to_mark();
    EVP_KEYMGMT_free(tmp_keymgmt);
    tmp_keymgmt = NULL;

    if (type == NULL && mdname != NULL)
        type = evp_get_digestbyname_ex(locpctx->libctx, mdname);

    if (ctx->pctx->pmeth == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE);
        return 0;
    }

    if (!(ctx->pctx->pmeth->flags & EVP_PKEY_FLAG_SIGCTX_CUSTOM)) {

        if (type == NULL) {
            int def_nid;
            if (EVP_PKEY_get_default_digest_nid(pkey, &def_nid) > 0)
                type = EVP_get_digestbynid(def_nid);
        }

        if (type == NULL) {
            ERR_raise(ERR_LIB_EVP, EVP_R_NO_DEFAULT_DIGEST);
            return 0;
        }
    }

    if (ver) {
        if (ctx->pctx->pmeth->verifyctx_init) {
            if (ctx->pctx->pmeth->verifyctx_init(ctx->pctx, ctx) <= 0)
                return 0;
            ctx->pctx->operation = EVP_PKEY_OP_VERIFYCTX;
        } else if (ctx->pctx->pmeth->digestverify != 0) {
            ctx->pctx->operation = EVP_PKEY_OP_VERIFY;
            ctx->update = update;
        } else if (EVP_PKEY_verify_init(ctx->pctx) <= 0) {
            return 0;
        }
    } else {
        if (ctx->pctx->pmeth->signctx_init) {
            if (ctx->pctx->pmeth->signctx_init(ctx->pctx, ctx) <= 0)
                return 0;
            ctx->pctx->operation = EVP_PKEY_OP_SIGNCTX;
        } else if (ctx->pctx->pmeth->digestsign != 0) {
            ctx->pctx->operation = EVP_PKEY_OP_SIGN;
            ctx->update = update;
        } else if (EVP_PKEY_sign_init(ctx->pctx) <= 0) {
            return 0;
        }
    }
    if (EVP_PKEY_CTX_set_signature_md(ctx->pctx, type) <= 0)
        return 0;
    if (pctx)
        *pctx = ctx->pctx;
    if (ctx->pctx->pmeth->flags & EVP_PKEY_FLAG_SIGCTX_CUSTOM)
        return 1;
    if (!EVP_DigestInit_ex(ctx, type, e))
        return 0;
    /*
     * This indicates the current algorithm requires
     * special treatment before hashing the tbs-message.
     */
    ctx->pctx->flag_call_digest_custom = 0;
    if (ctx->pctx->pmeth->digest_custom != NULL)
        ctx->pctx->flag_call_digest_custom = 1;

    ret = 1;
 end:
    if (ret > 0)
        ret = evp_pkey_ctx_use_cached_data(locpctx);

    EVP_KEYMGMT_free(tmp_keymgmt);
    return ret > 0 ? 1 : 0;
}

int EVP_DigestSignInit_ex(EVP_MD_CTX *ctx, EVP_PKEY_CTX **pctx,
                          const char *mdname, OSSL_LIB_CTX *libctx,
                          const char *props, EVP_PKEY *pkey,
                          const OSSL_PARAM params[])
{
    return do_sigver_init(ctx, pctx, NULL, mdname, libctx, props, NULL, pkey, 0,
                          params);
}

int EVP_DigestSignInit(EVP_MD_CTX *ctx, EVP_PKEY_CTX **pctx,
                       const EVP_MD *type, ENGINE *e, EVP_PKEY *pkey)
{
    return do_sigver_init(ctx, pctx, type, NULL, NULL, NULL, e, pkey, 0,
                          NULL);
}

int EVP_DigestVerifyInit_ex(EVP_MD_CTX *ctx, EVP_PKEY_CTX **pctx,
                            const char *mdname, OSSL_LIB_CTX *libctx,
                            const char *props, EVP_PKEY *pkey,
                            const OSSL_PARAM params[])
{
    return do_sigver_init(ctx, pctx, NULL, mdname, libctx, props, NULL, pkey, 1,
                          params);
}

int EVP_DigestVerifyInit(EVP_MD_CTX *ctx, EVP_PKEY_CTX **pctx,
                         const EVP_MD *type, ENGINE *e, EVP_PKEY *pkey)
{
    return do_sigver_init(ctx, pctx, type, NULL, NULL, NULL, e, pkey, 1,
                          NULL);
}

int EVP_DigestSignUpdate(EVP_MD_CTX *ctx, const void *data, size_t dsize)
{
    EVP_SIGNATURE *signature;
    const char *desc;
    EVP_PKEY_CTX *pctx = ctx->pctx;
    int ret;

    if ((ctx->flags & EVP_MD_CTX_FLAG_FINALISED) != 0) {
        ERR_raise(ERR_LIB_EVP, EVP_R_UPDATE_ERROR);
        return 0;
    }

    if (pctx == NULL
            || pctx->operation != EVP_PKEY_OP_SIGNCTX
            || pctx->op.sig.algctx == NULL
            || pctx->op.sig.signature == NULL)
        goto legacy;

    signature = pctx->op.sig.signature;
    desc = signature->description != NULL ? signature->description : "";
    if (signature->digest_sign_update == NULL) {
        ERR_raise_data(ERR_LIB_EVP, EVP_R_PROVIDER_SIGNATURE_NOT_SUPPORTED,
                       "%s digest_sign_update:%s", signature->type_name, desc);
        return 0;
    }

    ret = signature->digest_sign_update(pctx->op.sig.algctx, data, dsize);
    if (ret <= 0)
        ERR_raise_data(ERR_LIB_EVP, EVP_R_PROVIDER_SIGNATURE_FAILURE,
                       "%s digest_sign_update:%s", signature->type_name, desc);
    return ret;

 legacy:
    if (pctx != NULL) {
        /* do_sigver_init() checked that |digest_custom| is non-NULL */
        if (pctx->flag_call_digest_custom
            && !ctx->pctx->pmeth->digest_custom(ctx->pctx, ctx))
            return 0;
        pctx->flag_call_digest_custom = 0;
    }

    return EVP_DigestUpdate(ctx, data, dsize);
}

int EVP_DigestVerifyUpdate(EVP_MD_CTX *ctx, const void *data, size_t dsize)
{
    EVP_SIGNATURE *signature;
    const char *desc;
    EVP_PKEY_CTX *pctx = ctx->pctx;
    int ret;

    if ((ctx->flags & EVP_MD_CTX_FLAG_FINALISED) != 0) {
        ERR_raise(ERR_LIB_EVP, EVP_R_UPDATE_ERROR);
        return 0;
    }

    if (pctx == NULL
            || pctx->operation != EVP_PKEY_OP_VERIFYCTX
            || pctx->op.sig.algctx == NULL
            || pctx->op.sig.signature == NULL)
        goto legacy;

    signature = pctx->op.sig.signature;
    desc = signature->description != NULL ? signature->description : "";
    if (signature->digest_verify_update == NULL) {
        ERR_raise_data(ERR_LIB_EVP, EVP_R_PROVIDER_SIGNATURE_NOT_SUPPORTED,
                       "%s digest_verify_update:%s", signature->type_name, desc);
        return 0;
    }

    ret = signature->digest_verify_update(pctx->op.sig.algctx, data, dsize);
    if (ret <= 0)
        ERR_raise_data(ERR_LIB_EVP, EVP_R_PROVIDER_SIGNATURE_FAILURE,
                       "%s digest_verify_update:%s", signature->type_name, desc);
    return ret;

 legacy:
    if (pctx != NULL) {
        /* do_sigver_init() checked that |digest_custom| is non-NULL */
        if (pctx->flag_call_digest_custom
            && !ctx->pctx->pmeth->digest_custom(ctx->pctx, ctx))
            return 0;
        pctx->flag_call_digest_custom = 0;
    }

    return EVP_DigestUpdate(ctx, data, dsize);
}

int EVP_DigestSignFinal(EVP_MD_CTX *ctx, unsigned char *sigret,
                        size_t *siglen)
{
    EVP_SIGNATURE *signature;
    const char *desc;
    int sctx = 0;
    int r = 0;
    EVP_PKEY_CTX *dctx = NULL, *pctx = ctx->pctx;

    if ((ctx->flags & EVP_MD_CTX_FLAG_FINALISED) != 0) {
        ERR_raise(ERR_LIB_EVP, EVP_R_FINAL_ERROR);
        return 0;
    }

    if (pctx == NULL
            || pctx->operation != EVP_PKEY_OP_SIGNCTX
            || pctx->op.sig.algctx == NULL
            || pctx->op.sig.signature == NULL)
        goto legacy;

    signature = pctx->op.sig.signature;
    desc = signature->description != NULL ? signature->description : "";
    if (signature->digest_sign_final == NULL) {
        ERR_raise_data(ERR_LIB_EVP, EVP_R_PROVIDER_SIGNATURE_NOT_SUPPORTED,
                       "%s digest_sign_final:%s", signature->type_name, desc);
        return 0;
    }

    if (sigret != NULL && (ctx->flags & EVP_MD_CTX_FLAG_FINALISE) == 0) {
        /* try dup */
        dctx = EVP_PKEY_CTX_dup(pctx);
        if (dctx != NULL)
            pctx = dctx;
    }

    r = signature->digest_sign_final(pctx->op.sig.algctx, sigret, siglen,
                                     sigret == NULL ? 0 : *siglen);
    if (!r)
        ERR_raise_data(ERR_LIB_EVP, EVP_R_PROVIDER_SIGNATURE_FAILURE,
                       "%s digest_sign_final:%s", signature->type_name, desc);
    if (dctx == NULL && sigret != NULL)
        ctx->flags |= EVP_MD_CTX_FLAG_FINALISED;
    else
        EVP_PKEY_CTX_free(dctx);
    return r;

 legacy:
    if (pctx == NULL || pctx->pmeth == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
        return 0;
    }

    /* do_sigver_init() checked that |digest_custom| is non-NULL */
    if (pctx->flag_call_digest_custom
        && !ctx->pctx->pmeth->digest_custom(ctx->pctx, ctx))
        return 0;
    pctx->flag_call_digest_custom = 0;

    if (pctx->pmeth->flags & EVP_PKEY_FLAG_SIGCTX_CUSTOM) {
        if (sigret == NULL)
            return pctx->pmeth->signctx(pctx, sigret, siglen, ctx);
        if ((ctx->flags & EVP_MD_CTX_FLAG_FINALISE) != 0) {
            r = pctx->pmeth->signctx(pctx, sigret, siglen, ctx);
            ctx->flags |= EVP_MD_CTX_FLAG_FINALISED;
        } else {
            dctx = EVP_PKEY_CTX_dup(pctx);
            if (dctx == NULL)
                return 0;
            r = dctx->pmeth->signctx(dctx, sigret, siglen, ctx);
            EVP_PKEY_CTX_free(dctx);
        }
        return r;
    }
    if (pctx->pmeth->signctx != NULL)
        sctx = 1;
    else
        sctx = 0;
    if (sigret != NULL) {
        unsigned char md[EVP_MAX_MD_SIZE];
        unsigned int mdlen = 0;

        if (ctx->flags & EVP_MD_CTX_FLAG_FINALISE) {
            if (sctx)
                r = pctx->pmeth->signctx(pctx, sigret, siglen, ctx);
            else
                r = EVP_DigestFinal_ex(ctx, md, &mdlen);
        } else {
            EVP_MD_CTX *tmp_ctx = EVP_MD_CTX_new();

            if (tmp_ctx == NULL)
                return 0;
            if (!EVP_MD_CTX_copy_ex(tmp_ctx, ctx)) {
                EVP_MD_CTX_free(tmp_ctx);
                return 0;
            }
            if (sctx)
                r = tmp_ctx->pctx->pmeth->signctx(tmp_ctx->pctx,
                                                  sigret, siglen, tmp_ctx);
            else
                r = EVP_DigestFinal_ex(tmp_ctx, md, &mdlen);
            EVP_MD_CTX_free(tmp_ctx);
        }
        if (sctx || !r)
            return r;
        if (EVP_PKEY_sign(pctx, sigret, siglen, md, mdlen) <= 0)
            return 0;
    } else {
        if (sctx) {
            if (pctx->pmeth->signctx(pctx, sigret, siglen, ctx) <= 0)
                return 0;
        } else {
            int s = EVP_MD_get_size(ctx->digest);

            if (s <= 0 || EVP_PKEY_sign(pctx, sigret, siglen, NULL, s) <= 0)
                return 0;
        }
    }
    return 1;
}

int EVP_DigestSign(EVP_MD_CTX *ctx, unsigned char *sigret, size_t *siglen,
                   const unsigned char *tbs, size_t tbslen)
{
    EVP_PKEY_CTX *pctx = ctx->pctx;
    int ret;

    if (pctx == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
        return 0;
    }

    if ((ctx->flags & EVP_MD_CTX_FLAG_FINALISED) != 0) {
        ERR_raise(ERR_LIB_EVP, EVP_R_FINAL_ERROR);
        return 0;
    }

    if (pctx->operation == EVP_PKEY_OP_SIGNCTX
            && pctx->op.sig.algctx != NULL
            && pctx->op.sig.signature != NULL) {
        EVP_SIGNATURE *signature = pctx->op.sig.signature;

        if (signature->digest_sign != NULL) {
            const char *desc = signature->description != NULL ? signature->description : "";

            if (sigret != NULL)
                ctx->flags |= EVP_MD_CTX_FLAG_FINALISED;
            ret = signature->digest_sign(pctx->op.sig.algctx, sigret, siglen,
                                         sigret == NULL ? 0 : *siglen, tbs, tbslen);
            if (ret <= 0)
                ERR_raise_data(ERR_LIB_EVP, EVP_R_PROVIDER_SIGNATURE_FAILURE,
                               "%s digest_sign:%s", signature->type_name, desc);
            return ret;
        }
    } else {
        /* legacy */
        if (pctx->pmeth != NULL && pctx->pmeth->digestsign != NULL)
            return pctx->pmeth->digestsign(ctx, sigret, siglen, tbs, tbslen);
    }

    if (sigret != NULL && EVP_DigestSignUpdate(ctx, tbs, tbslen) <= 0)
        return 0;
    return EVP_DigestSignFinal(ctx, sigret, siglen);
}

int EVP_DigestVerifyFinal(EVP_MD_CTX *ctx, const unsigned char *sig,
                          size_t siglen)
{
    EVP_SIGNATURE *signature;
    const char *desc;
    int vctx = 0;
    unsigned int mdlen = 0;
    unsigned char md[EVP_MAX_MD_SIZE];
    int r = 0;
    EVP_PKEY_CTX *dctx = NULL, *pctx = ctx->pctx;

    if ((ctx->flags & EVP_MD_CTX_FLAG_FINALISED) != 0) {
        ERR_raise(ERR_LIB_EVP, EVP_R_FINAL_ERROR);
        return 0;
    }

    if (pctx == NULL
            || pctx->operation != EVP_PKEY_OP_VERIFYCTX
            || pctx->op.sig.algctx == NULL
            || pctx->op.sig.signature == NULL)
        goto legacy;

    signature = pctx->op.sig.signature;
    desc = signature->description != NULL ? signature->description : "";
    if (signature->digest_verify_final == NULL) {
        ERR_raise_data(ERR_LIB_EVP, EVP_R_PROVIDER_SIGNATURE_NOT_SUPPORTED,
                       "%s digest_verify_final:%s", signature->type_name, desc);
        return 0;
    }

    if ((ctx->flags & EVP_MD_CTX_FLAG_FINALISE) == 0) {
        /* try dup */
        dctx = EVP_PKEY_CTX_dup(pctx);
        if (dctx != NULL)
            pctx = dctx;
    }

    r = signature->digest_verify_final(pctx->op.sig.algctx, sig, siglen);
    if (!r)
        ERR_raise_data(ERR_LIB_EVP, EVP_R_PROVIDER_SIGNATURE_FAILURE,
                       "%s digest_verify_final:%s", signature->type_name, desc);
    if (dctx == NULL)
        ctx->flags |= EVP_MD_CTX_FLAG_FINALISED;
    else
        EVP_PKEY_CTX_free(dctx);
    return r;

 legacy:
    if (pctx == NULL || pctx->pmeth == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
        return 0;
    }

    /* do_sigver_init() checked that |digest_custom| is non-NULL */
    if (pctx->flag_call_digest_custom
        && !ctx->pctx->pmeth->digest_custom(ctx->pctx, ctx))
        return 0;
    pctx->flag_call_digest_custom = 0;

    if (pctx->pmeth->verifyctx != NULL)
        vctx = 1;
    else
        vctx = 0;
    if (ctx->flags & EVP_MD_CTX_FLAG_FINALISE) {
        if (vctx) {
            r = pctx->pmeth->verifyctx(pctx, sig, siglen, ctx);
            ctx->flags |= EVP_MD_CTX_FLAG_FINALISED;
        } else
            r = EVP_DigestFinal_ex(ctx, md, &mdlen);
    } else {
        EVP_MD_CTX *tmp_ctx = EVP_MD_CTX_new();
        if (tmp_ctx == NULL)
            return -1;
        if (!EVP_MD_CTX_copy_ex(tmp_ctx, ctx)) {
            EVP_MD_CTX_free(tmp_ctx);
            return -1;
        }
        if (vctx)
            r = tmp_ctx->pctx->pmeth->verifyctx(tmp_ctx->pctx,
                                                sig, siglen, tmp_ctx);
        else
            r = EVP_DigestFinal_ex(tmp_ctx, md, &mdlen);
        EVP_MD_CTX_free(tmp_ctx);
    }
    if (vctx || !r)
        return r;
    return EVP_PKEY_verify(pctx, sig, siglen, md, mdlen);
}

int EVP_DigestVerify(EVP_MD_CTX *ctx, const unsigned char *sigret,
                     size_t siglen, const unsigned char *tbs, size_t tbslen)
{
    EVP_PKEY_CTX *pctx = ctx->pctx;

    if (pctx == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
        return -1;
    }

    if ((ctx->flags & EVP_MD_CTX_FLAG_FINALISED) != 0) {
        ERR_raise(ERR_LIB_EVP, EVP_R_FINAL_ERROR);
        return 0;
    }

    if (pctx->operation == EVP_PKEY_OP_VERIFYCTX
            && pctx->op.sig.algctx != NULL
            && pctx->op.sig.signature != NULL) {
        if (pctx->op.sig.signature->digest_verify != NULL) {
            EVP_SIGNATURE *signature = pctx->op.sig.signature;
            const char *desc = signature->description != NULL ? signature->description : "";
            int ret;

            ctx->flags |= EVP_MD_CTX_FLAG_FINALISED;
            ret = signature->digest_verify(pctx->op.sig.algctx, sigret, siglen, tbs, tbslen);
            if (ret <= 0)
                ERR_raise_data(ERR_LIB_EVP, EVP_R_PROVIDER_SIGNATURE_FAILURE,
                               "%s digest_verify:%s", signature->type_name, desc);
            return ret;
        }
    } else {
        /* legacy */
        if (pctx->pmeth != NULL && pctx->pmeth->digestverify != NULL)
            return pctx->pmeth->digestverify(ctx, sigret, siglen, tbs, tbslen);
    }
    if (EVP_DigestVerifyUpdate(ctx, tbs, tbslen) <= 0)
        return -1;
    return EVP_DigestVerifyFinal(ctx, sigret, siglen);
}
