/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* We need to use some engine deprecated APIs */
#define OPENSSL_SUPPRESS_DEPRECATED

#include <stdio.h>
#include <openssl/objects.h>
#include <openssl/evp.h>
#include <openssl/ec.h>
#ifndef FIPS_MODULE
# include <openssl/engine.h>
#endif
#include <openssl/params.h>
#include <openssl/core_names.h>
#include "internal/cryptlib.h"
#include "internal/nelem.h"
#include "internal/provider.h"
#include "internal/core.h"
#include "crypto/evp.h"
#include "evp_local.h"

static void cleanup_old_md_data(EVP_MD_CTX *ctx, int force)
{
    if (ctx->digest != NULL) {
        if (ctx->digest->cleanup != NULL
                && !EVP_MD_CTX_test_flags(ctx, EVP_MD_CTX_FLAG_CLEANED))
            ctx->digest->cleanup(ctx);
        if (ctx->md_data != NULL && ctx->digest->ctx_size > 0
                && (!EVP_MD_CTX_test_flags(ctx, EVP_MD_CTX_FLAG_REUSE)
                    || force)) {
            OPENSSL_clear_free(ctx->md_data, ctx->digest->ctx_size);
            ctx->md_data = NULL;
        }
    }
}

void evp_md_ctx_clear_digest(EVP_MD_CTX *ctx, int force, int keep_fetched)
{
    if (ctx->algctx != NULL) {
        if (ctx->digest != NULL && ctx->digest->freectx != NULL)
            ctx->digest->freectx(ctx->algctx);
        ctx->algctx = NULL;
        EVP_MD_CTX_set_flags(ctx, EVP_MD_CTX_FLAG_CLEANED);
    }

    /* Code below to be removed when legacy support is dropped. */

    /*
     * Don't assume ctx->md_data was cleaned in EVP_Digest_Final, because
     * sometimes only copies of the context are ever finalised.
     */
    cleanup_old_md_data(ctx, force);
    if (force)
        ctx->digest = NULL;

#if !defined(FIPS_MODULE) && !defined(OPENSSL_NO_ENGINE)
    ENGINE_finish(ctx->engine);
    ctx->engine = NULL;
#endif

    /* Non legacy code, this has to be later than the ctx->digest cleaning */
    if (!keep_fetched) {
        EVP_MD_free(ctx->fetched_digest);
        ctx->fetched_digest = NULL;
        ctx->reqdigest = NULL;
    }
}

static int evp_md_ctx_reset_ex(EVP_MD_CTX *ctx, int keep_fetched)
{
    if (ctx == NULL)
        return 1;

    /*
     * pctx should be freed by the user of EVP_MD_CTX
     * if EVP_MD_CTX_FLAG_KEEP_PKEY_CTX is set
     */
    if (!EVP_MD_CTX_test_flags(ctx, EVP_MD_CTX_FLAG_KEEP_PKEY_CTX)) {
        EVP_PKEY_CTX_free(ctx->pctx);
        ctx->pctx = NULL;
    }

    evp_md_ctx_clear_digest(ctx, 0, keep_fetched);
    if (!keep_fetched)
        OPENSSL_cleanse(ctx, sizeof(*ctx));

    return 1;
}

/* This call frees resources associated with the context */
int EVP_MD_CTX_reset(EVP_MD_CTX *ctx)
{
    return evp_md_ctx_reset_ex(ctx, 0);
}

#ifndef FIPS_MODULE
EVP_MD_CTX *evp_md_ctx_new_ex(EVP_PKEY *pkey, const ASN1_OCTET_STRING *id,
                              OSSL_LIB_CTX *libctx, const char *propq)
{
    EVP_MD_CTX *ctx;
    EVP_PKEY_CTX *pctx = NULL;

    if ((ctx = EVP_MD_CTX_new()) == NULL
        || (pctx = EVP_PKEY_CTX_new_from_pkey(libctx, pkey, propq)) == NULL) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_EVP_LIB);
        goto err;
    }

    if (id != NULL && EVP_PKEY_CTX_set1_id(pctx, id->data, id->length) <= 0)
        goto err;

    EVP_MD_CTX_set_pkey_ctx(ctx, pctx);
    return ctx;

 err:
    EVP_PKEY_CTX_free(pctx);
    EVP_MD_CTX_free(ctx);
    return NULL;
}
#endif

EVP_MD_CTX *EVP_MD_CTX_new(void)
{
    return OPENSSL_zalloc(sizeof(EVP_MD_CTX));
}

void EVP_MD_CTX_free(EVP_MD_CTX *ctx)
{
    if (ctx == NULL)
        return;

    EVP_MD_CTX_reset(ctx);
    OPENSSL_free(ctx);
}

int evp_md_ctx_free_algctx(EVP_MD_CTX *ctx)
{
    if (ctx->algctx != NULL) {
        if (!ossl_assert(ctx->digest != NULL)) {
            ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
            return 0;
        }
        if (ctx->digest->freectx != NULL)
            ctx->digest->freectx(ctx->algctx);
        ctx->algctx = NULL;
    }
    return 1;
}

static int evp_md_init_internal(EVP_MD_CTX *ctx, const EVP_MD *type,
                                const OSSL_PARAM params[], ENGINE *impl)
{
#if !defined(OPENSSL_NO_ENGINE) && !defined(FIPS_MODULE)
    ENGINE *tmpimpl = NULL;
#endif

#if !defined(FIPS_MODULE)
    if (ctx->pctx != NULL
            && EVP_PKEY_CTX_IS_SIGNATURE_OP(ctx->pctx)
            && ctx->pctx->op.sig.algctx != NULL) {
        /*
         * Prior to OpenSSL 3.0 calling EVP_DigestInit_ex() on an mdctx
         * previously initialised with EVP_DigestSignInit() would retain
         * information about the key, and re-initialise for another sign
         * operation. So in that case we redirect to EVP_DigestSignInit()
         */
        if (ctx->pctx->operation == EVP_PKEY_OP_SIGNCTX)
            return EVP_DigestSignInit(ctx, NULL, type, impl, NULL);
        if (ctx->pctx->operation == EVP_PKEY_OP_VERIFYCTX)
            return EVP_DigestVerifyInit(ctx, NULL, type, impl, NULL);
        ERR_raise(ERR_LIB_EVP, EVP_R_UPDATE_ERROR);
        return 0;
    }
#endif

    EVP_MD_CTX_clear_flags(ctx, EVP_MD_CTX_FLAG_CLEANED
                                | EVP_MD_CTX_FLAG_FINALISED);

    if (type != NULL) {
        ctx->reqdigest = type;
    } else {
        if (ctx->digest == NULL) {
            ERR_raise(ERR_LIB_EVP, EVP_R_NO_DIGEST_SET);
            return 0;
        }
        type = ctx->digest;
    }

    /* Code below to be removed when legacy support is dropped. */
#if !defined(OPENSSL_NO_ENGINE) && !defined(FIPS_MODULE)
    /*
     * Whether it's nice or not, "Inits" can be used on "Final"'d contexts so
     * this context may already have an ENGINE! Try to avoid releasing the
     * previous handle, re-querying for an ENGINE, and having a
     * reinitialisation, when it may all be unnecessary.
     */
    if (ctx->engine != NULL
            && ctx->digest != NULL
            && type->type == ctx->digest->type)
        goto skip_to_init;

    /*
     * Ensure an ENGINE left lying around from last time is cleared (the
     * previous check attempted to avoid this if the same ENGINE and
     * EVP_MD could be used).
     */
    ENGINE_finish(ctx->engine);
    ctx->engine = NULL;

    if (impl == NULL)
        tmpimpl = ENGINE_get_digest_engine(type->type);
#endif

    /*
     * If there are engines involved or EVP_MD_CTX_FLAG_NO_INIT is set then we
     * should use legacy handling for now.
     */
    if (impl != NULL
#if !defined(OPENSSL_NO_ENGINE)
            || ctx->engine != NULL
# if !defined(FIPS_MODULE)
            || tmpimpl != NULL
# endif
#endif
            || (ctx->flags & EVP_MD_CTX_FLAG_NO_INIT) != 0
            || (type != NULL && type->origin == EVP_ORIG_METH)
            || (type == NULL && ctx->digest != NULL
                             && ctx->digest->origin == EVP_ORIG_METH)) {
        /* If we were using provided hash before, cleanup algctx */
        if (!evp_md_ctx_free_algctx(ctx))
            return 0;
        if (ctx->digest == ctx->fetched_digest)
            ctx->digest = NULL;
        EVP_MD_free(ctx->fetched_digest);
        ctx->fetched_digest = NULL;
        goto legacy;
    }

    cleanup_old_md_data(ctx, 1);

    /* Start of non-legacy code below */
    if (ctx->digest == type) {
        if (!ossl_assert(type->prov != NULL)) {
            ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
            return 0;
        }
    } else {
        if (!evp_md_ctx_free_algctx(ctx))
            return 0;
    }

    if (type->prov == NULL) {
#ifdef FIPS_MODULE
        /* We only do explicit fetches inside the FIPS module */
        ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
        return 0;
#else
        /* The NULL digest is a special case */
        EVP_MD *provmd = EVP_MD_fetch(NULL,
                                      type->type != NID_undef ? OBJ_nid2sn(type->type)
                                                              : "NULL", "");

        if (provmd == NULL) {
            ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
            return 0;
        }
        type = provmd;
        EVP_MD_free(ctx->fetched_digest);
        ctx->fetched_digest = provmd;
#endif
    }

    if (type->prov != NULL && ctx->fetched_digest != type) {
        if (!EVP_MD_up_ref((EVP_MD *)type)) {
            ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
            return 0;
        }
        EVP_MD_free(ctx->fetched_digest);
        ctx->fetched_digest = (EVP_MD *)type;
    }
    ctx->digest = type;
    if (ctx->algctx == NULL) {
        ctx->algctx = ctx->digest->newctx(ossl_provider_ctx(type->prov));
        if (ctx->algctx == NULL) {
            ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
            return 0;
        }
    }

    if (ctx->digest->dinit == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
        return 0;
    }

    return ctx->digest->dinit(ctx->algctx, params);

    /* Code below to be removed when legacy support is dropped. */
 legacy:

#if !defined(OPENSSL_NO_ENGINE) && !defined(FIPS_MODULE)
    if (type) {
        if (impl != NULL) {
            if (!ENGINE_init(impl)) {
                ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
                return 0;
            }
        } else {
            /* Ask if an ENGINE is reserved for this job */
            impl = tmpimpl;
        }
        if (impl != NULL) {
            /* There's an ENGINE for this job ... (apparently) */
            const EVP_MD *d = ENGINE_get_digest(impl, type->type);

            if (d == NULL) {
                ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
                ENGINE_finish(impl);
                return 0;
            }
            /* We'll use the ENGINE's private digest definition */
            type = d;
            /*
             * Store the ENGINE functional reference so we know 'type' came
             * from an ENGINE and we need to release it when done.
             */
            ctx->engine = impl;
        } else
            ctx->engine = NULL;
    }
#endif
    if (ctx->digest != type) {
        cleanup_old_md_data(ctx, 1);

        ctx->digest = type;
        if (!(ctx->flags & EVP_MD_CTX_FLAG_NO_INIT) && type->ctx_size) {
            ctx->update = type->update;
            ctx->md_data = OPENSSL_zalloc(type->ctx_size);
            if (ctx->md_data == NULL)
                return 0;
        }
    }
#if !defined(OPENSSL_NO_ENGINE) && !defined(FIPS_MODULE)
 skip_to_init:
#endif
#ifndef FIPS_MODULE
    if (ctx->pctx != NULL
            && (!EVP_PKEY_CTX_IS_SIGNATURE_OP(ctx->pctx)
                 || ctx->pctx->op.sig.signature == NULL)) {
        int r;
        r = EVP_PKEY_CTX_ctrl(ctx->pctx, -1, EVP_PKEY_OP_TYPE_SIG,
                              EVP_PKEY_CTRL_DIGESTINIT, 0, ctx);
        if (r <= 0 && (r != -2))
            return 0;
    }
#endif
    if (ctx->flags & EVP_MD_CTX_FLAG_NO_INIT)
        return 1;
    return ctx->digest->init(ctx);
}

int EVP_DigestInit_ex2(EVP_MD_CTX *ctx, const EVP_MD *type,
                       const OSSL_PARAM params[])
{
    return evp_md_init_internal(ctx, type, params, NULL);
}

int EVP_DigestInit(EVP_MD_CTX *ctx, const EVP_MD *type)
{
    EVP_MD_CTX_reset(ctx);
    return evp_md_init_internal(ctx, type, NULL, NULL);
}

int EVP_DigestInit_ex(EVP_MD_CTX *ctx, const EVP_MD *type, ENGINE *impl)
{
    return evp_md_init_internal(ctx, type, NULL, impl);
}

int EVP_DigestUpdate(EVP_MD_CTX *ctx, const void *data, size_t count)
{
    if (count == 0)
        return 1;

    if ((ctx->flags & EVP_MD_CTX_FLAG_FINALISED) != 0) {
        ERR_raise(ERR_LIB_EVP, EVP_R_UPDATE_ERROR);
        return 0;
    }

    if (ctx->pctx != NULL
            && EVP_PKEY_CTX_IS_SIGNATURE_OP(ctx->pctx)
            && ctx->pctx->op.sig.algctx != NULL) {
#ifndef FIPS_MODULE
        /*
         * Prior to OpenSSL 3.0 EVP_DigestSignUpdate() and
         * EVP_DigestVerifyUpdate() were just macros for EVP_DigestUpdate().
         * Some code calls EVP_DigestUpdate() directly even when initialised
         * with EVP_DigestSignInit_ex() or
         * EVP_DigestVerifyInit_ex(), so we detect that and redirect to
         * the correct EVP_Digest*Update() function
         */
        if (ctx->pctx->operation == EVP_PKEY_OP_SIGNCTX)
            return EVP_DigestSignUpdate(ctx, data, count);
        if (ctx->pctx->operation == EVP_PKEY_OP_VERIFYCTX)
            return EVP_DigestVerifyUpdate(ctx, data, count);
#endif
        ERR_raise(ERR_LIB_EVP, EVP_R_UPDATE_ERROR);
        return 0;
    }

    if (ctx->digest == NULL
            || ctx->digest->prov == NULL
            || (ctx->flags & EVP_MD_CTX_FLAG_NO_INIT) != 0)
        goto legacy;

    if (ctx->digest->dupdate == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_UPDATE_ERROR);
        return 0;
    }
    return ctx->digest->dupdate(ctx->algctx, data, count);

    /* Code below to be removed when legacy support is dropped. */
 legacy:
    return ctx->update != NULL ? ctx->update(ctx, data, count) : 0;
}

/* The caller can assume that this removes any secret data from the context */
int EVP_DigestFinal(EVP_MD_CTX *ctx, unsigned char *md, unsigned int *size)
{
    int ret;
    ret = EVP_DigestFinal_ex(ctx, md, size);
    EVP_MD_CTX_reset(ctx);
    return ret;
}

/* The caller can assume that this removes any secret data from the context */
int EVP_DigestFinal_ex(EVP_MD_CTX *ctx, unsigned char *md, unsigned int *isize)
{
    int ret, sz;
    size_t size = 0;
    size_t mdsize = 0;

    if (ctx->digest == NULL)
        return 0;

    sz = EVP_MD_CTX_get_size(ctx);
    if (sz < 0)
        return 0;
    mdsize = sz;
    if (ctx->digest->prov == NULL)
        goto legacy;

    if (ctx->digest->dfinal == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_FINAL_ERROR);
        return 0;
    }

    if ((ctx->flags & EVP_MD_CTX_FLAG_FINALISED) != 0) {
        ERR_raise(ERR_LIB_EVP, EVP_R_FINAL_ERROR);
        return 0;
    }

    ret = ctx->digest->dfinal(ctx->algctx, md, &size, mdsize);

    ctx->flags |= EVP_MD_CTX_FLAG_FINALISED;

    if (isize != NULL) {
        if (size <= UINT_MAX) {
            *isize = (unsigned int)size;
        } else {
            ERR_raise(ERR_LIB_EVP, EVP_R_FINAL_ERROR);
            ret = 0;
        }
    }

    return ret;

    /* Code below to be removed when legacy support is dropped. */
 legacy:
    OPENSSL_assert(mdsize <= EVP_MAX_MD_SIZE);
    ret = ctx->digest->final(ctx, md);
    if (isize != NULL)
        *isize = mdsize;
    if (ctx->digest->cleanup) {
        ctx->digest->cleanup(ctx);
        EVP_MD_CTX_set_flags(ctx, EVP_MD_CTX_FLAG_CLEANED);
    }
    OPENSSL_cleanse(ctx->md_data, ctx->digest->ctx_size);
    return ret;
}

/* This is a one shot operation */
int EVP_DigestFinalXOF(EVP_MD_CTX *ctx, unsigned char *md, size_t size)
{
    int ret = 0;
    OSSL_PARAM params[2];
    size_t i = 0;

    if (ctx->digest == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_NULL_ALGORITHM);
        return 0;
    }

    if (ctx->digest->prov == NULL)
        goto legacy;

    if (ctx->digest->dfinal == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_FINAL_ERROR);
        return 0;
    }

    if ((ctx->flags & EVP_MD_CTX_FLAG_FINALISED) != 0) {
        ERR_raise(ERR_LIB_EVP, EVP_R_FINAL_ERROR);
        return 0;
    }

    /*
     * For backward compatibility we pass the XOFLEN via a param here so that
     * older providers can use the supplied value. Ideally we should have just
     * used the size passed into ctx->digest->dfinal().
     */
    params[i++] = OSSL_PARAM_construct_size_t(OSSL_DIGEST_PARAM_XOFLEN, &size);
    params[i++] = OSSL_PARAM_construct_end();

    if (EVP_MD_CTX_set_params(ctx, params) >= 0)
        ret = ctx->digest->dfinal(ctx->algctx, md, &size, size);

    ctx->flags |= EVP_MD_CTX_FLAG_FINALISED;

    return ret;

legacy:
    if (EVP_MD_xof(ctx->digest)
        && size <= INT_MAX
        && ctx->digest->md_ctrl(ctx, EVP_MD_CTRL_XOF_LEN, (int)size, NULL)) {
        ret = ctx->digest->final(ctx, md);
        if (ctx->digest->cleanup != NULL) {
            ctx->digest->cleanup(ctx);
            EVP_MD_CTX_set_flags(ctx, EVP_MD_CTX_FLAG_CLEANED);
        }
        OPENSSL_cleanse(ctx->md_data, ctx->digest->ctx_size);
    } else {
        ERR_raise(ERR_LIB_EVP, EVP_R_NOT_XOF_OR_INVALID_LENGTH);
    }

    return ret;
}

/* EVP_DigestSqueeze() can be called multiple times */
int EVP_DigestSqueeze(EVP_MD_CTX *ctx, unsigned char *md, size_t size)
{
    if (ctx->digest == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_NULL_ALGORITHM);
        return 0;
    }

    if (ctx->digest->prov == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_OPERATION);
        return 0;
    }

    if (ctx->digest->dsqueeze == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_METHOD_NOT_SUPPORTED);
        return 0;
    }

    return ctx->digest->dsqueeze(ctx->algctx, md, &size, size);
}

EVP_MD_CTX *EVP_MD_CTX_dup(const EVP_MD_CTX *in)
{
    EVP_MD_CTX *out = EVP_MD_CTX_new();

    if (out != NULL && !EVP_MD_CTX_copy_ex(out, in)) {
        EVP_MD_CTX_free(out);
        out = NULL;
    }
    return out;
}

int EVP_MD_CTX_copy(EVP_MD_CTX *out, const EVP_MD_CTX *in)
{
    EVP_MD_CTX_reset(out);
    return EVP_MD_CTX_copy_ex(out, in);
}

int EVP_MD_CTX_copy_ex(EVP_MD_CTX *out, const EVP_MD_CTX *in)
{
    int digest_change = 0;
    unsigned char *tmp_buf;

    if (in == NULL) {
        ERR_raise(ERR_LIB_EVP, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if (in->digest == NULL) {
        /* copying uninitialized digest context */
        EVP_MD_CTX_reset(out);
        if (out->fetched_digest != NULL)
            EVP_MD_free(out->fetched_digest);
        *out = *in;
        goto clone_pkey;
    }

    if (in->digest->prov == NULL
            || (in->flags & EVP_MD_CTX_FLAG_NO_INIT) != 0)
        goto legacy;

    if (in->digest->dupctx == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_NOT_ABLE_TO_COPY_CTX);
        return 0;
    }

    if (out->digest == in->digest && in->digest->copyctx != NULL) {

        in->digest->copyctx(out->algctx, in->algctx);

        EVP_PKEY_CTX_free(out->pctx);
        out->pctx = NULL;
        cleanup_old_md_data(out, 0);

        out->flags = in->flags;
        out->update = in->update;
    } else {
        evp_md_ctx_reset_ex(out, 1);
        digest_change = (out->fetched_digest != in->fetched_digest);

        if (digest_change && in->fetched_digest != NULL
            && !EVP_MD_up_ref(in->fetched_digest))
            return 0;
        if (digest_change && out->fetched_digest != NULL)
            EVP_MD_free(out->fetched_digest);
        *out = *in;
        /* NULL out pointers in case of error */
        out->pctx = NULL;
        out->algctx = NULL;

        if (in->algctx != NULL) {
            out->algctx = in->digest->dupctx(in->algctx);
            if (out->algctx == NULL) {
                ERR_raise(ERR_LIB_EVP, EVP_R_NOT_ABLE_TO_COPY_CTX);
                return 0;
            }
        }
    }

 clone_pkey:
    /* copied EVP_MD_CTX should free the copied EVP_PKEY_CTX */
    EVP_MD_CTX_clear_flags(out, EVP_MD_CTX_FLAG_KEEP_PKEY_CTX);
#ifndef FIPS_MODULE
    if (in->pctx != NULL) {
        out->pctx = EVP_PKEY_CTX_dup(in->pctx);
        if (out->pctx == NULL) {
            ERR_raise(ERR_LIB_EVP, EVP_R_NOT_ABLE_TO_COPY_CTX);
            EVP_MD_CTX_reset(out);
            return 0;
        }
    }
#endif

    return 1;

    /* Code below to be removed when legacy support is dropped. */
 legacy:
#if !defined(OPENSSL_NO_ENGINE) && !defined(FIPS_MODULE)
    /* Make sure it's safe to copy a digest context using an ENGINE */
    if (in->engine && !ENGINE_init(in->engine)) {
        ERR_raise(ERR_LIB_EVP, ERR_R_ENGINE_LIB);
        return 0;
    }
#endif

    if (out->digest == in->digest) {
        tmp_buf = out->md_data;
        EVP_MD_CTX_set_flags(out, EVP_MD_CTX_FLAG_REUSE);
    } else
        tmp_buf = NULL;
    EVP_MD_CTX_reset(out);
    memcpy(out, in, sizeof(*out));

    /* copied EVP_MD_CTX should free the copied EVP_PKEY_CTX */
    EVP_MD_CTX_clear_flags(out, EVP_MD_CTX_FLAG_KEEP_PKEY_CTX);

    /* Null these variables, since they are getting fixed up
     * properly below.  Anything else may cause a memleak and/or
     * double free if any of the memory allocations below fail
     */
    out->md_data = NULL;
    out->pctx = NULL;

    if (in->md_data && out->digest->ctx_size) {
        if (tmp_buf)
            out->md_data = tmp_buf;
        else {
            out->md_data = OPENSSL_malloc(out->digest->ctx_size);
            if (out->md_data == NULL)
                return 0;
        }
        memcpy(out->md_data, in->md_data, out->digest->ctx_size);
    }

    out->update = in->update;

#ifndef FIPS_MODULE
    if (in->pctx) {
        out->pctx = EVP_PKEY_CTX_dup(in->pctx);
        if (!out->pctx) {
            EVP_MD_CTX_reset(out);
            return 0;
        }
    }
#endif

    if (out->digest->copy)
        return out->digest->copy(out, in);

    return 1;
}

int EVP_Digest(const void *data, size_t count,
               unsigned char *md, unsigned int *size, const EVP_MD *type,
               ENGINE *impl)
{
    EVP_MD_CTX *ctx = EVP_MD_CTX_new();
    int ret;

    if (ctx == NULL)
        return 0;
    EVP_MD_CTX_set_flags(ctx, EVP_MD_CTX_FLAG_ONESHOT);
    ret = EVP_DigestInit_ex(ctx, type, impl)
        && EVP_DigestUpdate(ctx, data, count)
        && EVP_DigestFinal_ex(ctx, md, size);
    EVP_MD_CTX_free(ctx);

    return ret;
}

int EVP_Q_digest(OSSL_LIB_CTX *libctx, const char *name, const char *propq,
                 const void *data, size_t datalen,
                 unsigned char *md, size_t *mdlen)
{
    EVP_MD *digest = EVP_MD_fetch(libctx, name, propq);
    unsigned int temp = 0;
    int ret = 0;

    if (digest != NULL) {
        ret = EVP_Digest(data, datalen, md, &temp, digest, NULL);
        EVP_MD_free(digest);
    }
    if (mdlen != NULL)
        *mdlen = temp;
    return ret;
}

int EVP_MD_get_params(const EVP_MD *digest, OSSL_PARAM params[])
{
    if (digest != NULL && digest->get_params != NULL)
        return digest->get_params(params);
    return 0;
}

const OSSL_PARAM *EVP_MD_gettable_params(const EVP_MD *digest)
{
    if (digest != NULL && digest->gettable_params != NULL)
        return digest->gettable_params(
                           ossl_provider_ctx(EVP_MD_get0_provider(digest)));
    return NULL;
}

int EVP_MD_CTX_set_params(EVP_MD_CTX *ctx, const OSSL_PARAM params[])
{
    EVP_PKEY_CTX *pctx = ctx->pctx;

    /* If we have a pctx then we should try that first */
    if (pctx != NULL
            && (pctx->operation == EVP_PKEY_OP_VERIFYCTX
                || pctx->operation == EVP_PKEY_OP_SIGNCTX)
            && pctx->op.sig.algctx != NULL
            && pctx->op.sig.signature->set_ctx_md_params != NULL)
        return pctx->op.sig.signature->set_ctx_md_params(pctx->op.sig.algctx,
                                                         params);

    if (ctx->digest != NULL && ctx->digest->set_ctx_params != NULL)
        return ctx->digest->set_ctx_params(ctx->algctx, params);

    return 0;
}

const OSSL_PARAM *EVP_MD_settable_ctx_params(const EVP_MD *md)
{
    void *provctx;

    if (md != NULL && md->settable_ctx_params != NULL) {
        provctx = ossl_provider_ctx(EVP_MD_get0_provider(md));
        return md->settable_ctx_params(NULL, provctx);
    }
    return NULL;
}

const OSSL_PARAM *EVP_MD_CTX_settable_params(EVP_MD_CTX *ctx)
{
    EVP_PKEY_CTX *pctx;
    void *alg;

    if (ctx == NULL)
        return NULL;

    /* If we have a pctx then we should try that first */
    pctx = ctx->pctx;
    if (pctx != NULL
            && (pctx->operation == EVP_PKEY_OP_VERIFYCTX
                || pctx->operation == EVP_PKEY_OP_SIGNCTX)
            && pctx->op.sig.algctx != NULL
            && pctx->op.sig.signature->settable_ctx_md_params != NULL)
        return pctx->op.sig.signature->settable_ctx_md_params(
                   pctx->op.sig.algctx);

    if (ctx->digest != NULL && ctx->digest->settable_ctx_params != NULL) {
        alg = ossl_provider_ctx(EVP_MD_get0_provider(ctx->digest));
        return ctx->digest->settable_ctx_params(ctx->algctx, alg);
    }

    return NULL;
}

int EVP_MD_CTX_get_params(EVP_MD_CTX *ctx, OSSL_PARAM params[])
{
    EVP_PKEY_CTX *pctx = ctx->pctx;

    /* If we have a pctx then we should try that first */
    if (pctx != NULL
            && (pctx->operation == EVP_PKEY_OP_VERIFYCTX
                || pctx->operation == EVP_PKEY_OP_SIGNCTX)
            && pctx->op.sig.algctx != NULL
            && pctx->op.sig.signature->get_ctx_md_params != NULL)
        return pctx->op.sig.signature->get_ctx_md_params(pctx->op.sig.algctx,
                                                         params);

    if (ctx->digest != NULL && ctx->digest->get_ctx_params != NULL)
        return ctx->digest->get_ctx_params(ctx->algctx, params);

    return 0;
}

const OSSL_PARAM *EVP_MD_gettable_ctx_params(const EVP_MD *md)
{
    void *provctx;

    if (md != NULL && md->gettable_ctx_params != NULL) {
        provctx = ossl_provider_ctx(EVP_MD_get0_provider(md));
        return md->gettable_ctx_params(NULL, provctx);
    }
    return NULL;
}

const OSSL_PARAM *EVP_MD_CTX_gettable_params(EVP_MD_CTX *ctx)
{
    EVP_PKEY_CTX *pctx;
    void *provctx;

    if (ctx == NULL)
        return NULL;

    /* If we have a pctx then we should try that first */
    pctx = ctx->pctx;
    if (pctx != NULL
            && (pctx->operation == EVP_PKEY_OP_VERIFYCTX
                || pctx->operation == EVP_PKEY_OP_SIGNCTX)
            && pctx->op.sig.algctx != NULL
            && pctx->op.sig.signature->gettable_ctx_md_params != NULL)
        return pctx->op.sig.signature->gettable_ctx_md_params(
                    pctx->op.sig.algctx);

    if (ctx->digest != NULL && ctx->digest->gettable_ctx_params != NULL) {
        provctx = ossl_provider_ctx(EVP_MD_get0_provider(ctx->digest));
        return ctx->digest->gettable_ctx_params(ctx->algctx, provctx);
    }
    return NULL;
}

int EVP_MD_CTX_ctrl(EVP_MD_CTX *ctx, int cmd, int p1, void *p2)
{
    int ret = EVP_CTRL_RET_UNSUPPORTED;
    int set_params = 1;
    size_t sz;
    OSSL_PARAM params[2] = { OSSL_PARAM_END, OSSL_PARAM_END };

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_EVP, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if (ctx->digest != NULL && ctx->digest->prov == NULL)
        goto legacy;

    switch (cmd) {
    case EVP_MD_CTRL_XOF_LEN:
        sz = (size_t)p1;
        params[0] = OSSL_PARAM_construct_size_t(OSSL_DIGEST_PARAM_XOFLEN, &sz);
        break;
    case EVP_MD_CTRL_MICALG:
        set_params = 0;
        params[0] = OSSL_PARAM_construct_utf8_string(OSSL_DIGEST_PARAM_MICALG,
                                                     p2, p1 ? p1 : 9999);
        break;
    case EVP_CTRL_SSL3_MASTER_SECRET:
        params[0] = OSSL_PARAM_construct_octet_string(OSSL_DIGEST_PARAM_SSL3_MS,
                                                      p2, p1);
        break;
    default:
        goto conclude;
    }

    if (set_params)
        ret = EVP_MD_CTX_set_params(ctx, params);
    else
        ret = EVP_MD_CTX_get_params(ctx, params);
    goto conclude;


    /* Code below to be removed when legacy support is dropped. */
 legacy:
    if (ctx->digest->md_ctrl == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_CTRL_NOT_IMPLEMENTED);
        return 0;
    }

    ret = ctx->digest->md_ctrl(ctx, cmd, p1, p2);
 conclude:
    if (ret <= 0)
        return 0;
    return ret;
}

EVP_MD *evp_md_new(void)
{
    EVP_MD *md = OPENSSL_zalloc(sizeof(*md));

    if (md != NULL && !CRYPTO_NEW_REF(&md->refcnt, 1)) {
        OPENSSL_free(md);
        return NULL;
    }
    return md;
}

/*
 * FIPS module note: since internal fetches will be entirely
 * provider based, we know that none of its code depends on legacy
 * NIDs or any functionality that use them.
 */
#ifndef FIPS_MODULE
static void set_legacy_nid(const char *name, void *vlegacy_nid)
{
    int nid;
    int *legacy_nid = vlegacy_nid;
    /*
     * We use lowest level function to get the associated method, because
     * higher level functions such as EVP_get_digestbyname() have changed
     * to look at providers too.
     */
    const void *legacy_method = OBJ_NAME_get(name, OBJ_NAME_TYPE_MD_METH);

    if (*legacy_nid == -1)       /* We found a clash already */
        return;

    if (legacy_method == NULL)
        return;
    nid = EVP_MD_nid(legacy_method);
    if (*legacy_nid != NID_undef && *legacy_nid != nid) {
        *legacy_nid = -1;
        return;
    }
    *legacy_nid = nid;
}
#endif

static int evp_md_cache_constants(EVP_MD *md)
{
    int ok, xof = 0, algid_absent = 0;
    size_t blksz = 0;
    size_t mdsize = 0;
    OSSL_PARAM params[5];

    /*
     * Note that these parameters are 'constants' that are only set up
     * during the EVP_MD_fetch(). For this reason the XOF functions set the
     * md_size to 0, since the output size is unknown.
     */
    params[0] = OSSL_PARAM_construct_size_t(OSSL_DIGEST_PARAM_BLOCK_SIZE, &blksz);
    params[1] = OSSL_PARAM_construct_size_t(OSSL_DIGEST_PARAM_SIZE, &mdsize);
    params[2] = OSSL_PARAM_construct_int(OSSL_DIGEST_PARAM_XOF, &xof);
    params[3] = OSSL_PARAM_construct_int(OSSL_DIGEST_PARAM_ALGID_ABSENT,
                                         &algid_absent);
    params[4] = OSSL_PARAM_construct_end();
    ok = evp_do_md_getparams(md, params) > 0;
    if (mdsize > INT_MAX || blksz > INT_MAX)
        ok = 0;
    if (ok) {
        md->block_size = (int)blksz;
        md->md_size = (int)mdsize;
        if (xof)
            md->flags |= EVP_MD_FLAG_XOF;
        if (algid_absent)
            md->flags |= EVP_MD_FLAG_DIGALGID_ABSENT;
    }
    return ok;
}

static void *evp_md_from_algorithm(int name_id,
                                   const OSSL_ALGORITHM *algodef,
                                   OSSL_PROVIDER *prov)
{
    const OSSL_DISPATCH *fns = algodef->implementation;
    EVP_MD *md = NULL;
    int fncnt = 0;

    /* EVP_MD_fetch() will set the legacy NID if available */
    if ((md = evp_md_new()) == NULL) {
        ERR_raise(ERR_LIB_EVP, ERR_R_EVP_LIB);
        return NULL;
    }

#ifndef FIPS_MODULE
    md->type = NID_undef;
    if (!evp_names_do_all(prov, name_id, set_legacy_nid, &md->type)
            || md->type == -1) {
        ERR_raise(ERR_LIB_EVP, ERR_R_INTERNAL_ERROR);
        goto err;
    }
#endif

    md->name_id = name_id;
    if ((md->type_name = ossl_algorithm_get1_first_name(algodef)) == NULL)
        goto err;

    md->description = algodef->algorithm_description;

    for (; fns->function_id != 0; fns++) {
        switch (fns->function_id) {
        case OSSL_FUNC_DIGEST_NEWCTX:
            if (md->newctx == NULL) {
                md->newctx = OSSL_FUNC_digest_newctx(fns);
                fncnt++;
            }
            break;
        case OSSL_FUNC_DIGEST_INIT:
            if (md->dinit == NULL) {
                md->dinit = OSSL_FUNC_digest_init(fns);
                fncnt++;
            }
            break;
        case OSSL_FUNC_DIGEST_UPDATE:
            if (md->dupdate == NULL) {
                md->dupdate = OSSL_FUNC_digest_update(fns);
                fncnt++;
            }
            break;
        case OSSL_FUNC_DIGEST_FINAL:
            if (md->dfinal == NULL) {
                md->dfinal = OSSL_FUNC_digest_final(fns);
                fncnt++;
            }
            break;
        case OSSL_FUNC_DIGEST_SQUEEZE:
            if (md->dsqueeze == NULL) {
                md->dsqueeze = OSSL_FUNC_digest_squeeze(fns);
                fncnt++;
            }
            break;
        case OSSL_FUNC_DIGEST_DIGEST:
            if (md->digest == NULL)
                md->digest = OSSL_FUNC_digest_digest(fns);
            /* We don't increment fnct for this as it is stand alone */
            break;
        case OSSL_FUNC_DIGEST_FREECTX:
            if (md->freectx == NULL) {
                md->freectx = OSSL_FUNC_digest_freectx(fns);
                fncnt++;
            }
            break;
        case OSSL_FUNC_DIGEST_DUPCTX:
            if (md->dupctx == NULL)
                md->dupctx = OSSL_FUNC_digest_dupctx(fns);
            break;
        case OSSL_FUNC_DIGEST_GET_PARAMS:
            if (md->get_params == NULL)
                md->get_params = OSSL_FUNC_digest_get_params(fns);
            break;
        case OSSL_FUNC_DIGEST_SET_CTX_PARAMS:
            if (md->set_ctx_params == NULL)
                md->set_ctx_params = OSSL_FUNC_digest_set_ctx_params(fns);
            break;
        case OSSL_FUNC_DIGEST_GET_CTX_PARAMS:
            if (md->get_ctx_params == NULL)
                md->get_ctx_params = OSSL_FUNC_digest_get_ctx_params(fns);
            break;
        case OSSL_FUNC_DIGEST_GETTABLE_PARAMS:
            if (md->gettable_params == NULL)
                md->gettable_params = OSSL_FUNC_digest_gettable_params(fns);
            break;
        case OSSL_FUNC_DIGEST_SETTABLE_CTX_PARAMS:
            if (md->settable_ctx_params == NULL)
                md->settable_ctx_params =
                    OSSL_FUNC_digest_settable_ctx_params(fns);
            break;
        case OSSL_FUNC_DIGEST_GETTABLE_CTX_PARAMS:
            if (md->gettable_ctx_params == NULL)
                md->gettable_ctx_params =
                    OSSL_FUNC_digest_gettable_ctx_params(fns);
            break;
        case OSSL_FUNC_DIGEST_COPYCTX:
            if (md->copyctx == NULL)
                md->copyctx =
                    OSSL_FUNC_digest_copyctx(fns);
            break;
        }
    }
    if ((fncnt != 0 && fncnt != 5 && fncnt != 6)
        || (fncnt == 0 && md->digest == NULL)) {
        /*
         * In order to be a consistent set of functions we either need the
         * whole set of init/update/final etc functions or none of them.
         * The "digest" function can standalone. We at least need one way to
         * generate digests.
         */
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_PROVIDER_FUNCTIONS);
        goto err;
    }
    if (prov != NULL && !ossl_provider_up_ref(prov))
        goto err;

    md->prov = prov;

    if (!evp_md_cache_constants(md)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_CACHE_CONSTANTS_FAILED);
        goto err;
    }

    return md;

err:
    EVP_MD_free(md);
    return NULL;
}

static int evp_md_up_ref(void *md)
{
    return EVP_MD_up_ref(md);
}

static void evp_md_free(void *md)
{
    EVP_MD_free(md);
}

EVP_MD *EVP_MD_fetch(OSSL_LIB_CTX *ctx, const char *algorithm,
                     const char *properties)
{
    EVP_MD *md =
        evp_generic_fetch(ctx, OSSL_OP_DIGEST, algorithm, properties,
                          evp_md_from_algorithm, evp_md_up_ref, evp_md_free);

    return md;
}

int EVP_MD_up_ref(EVP_MD *md)
{
    int ref = 0;

    if (md->origin == EVP_ORIG_DYNAMIC)
        CRYPTO_UP_REF(&md->refcnt, &ref);
    return 1;
}

void EVP_MD_free(EVP_MD *md)
{
    int i;

    if (md == NULL || md->origin != EVP_ORIG_DYNAMIC)
        return;

    CRYPTO_DOWN_REF(&md->refcnt, &i);
    if (i > 0)
        return;
    evp_md_free_int(md);
}

void EVP_MD_do_all_provided(OSSL_LIB_CTX *libctx,
                            void (*fn)(EVP_MD *mac, void *arg),
                            void *arg)
{
    evp_generic_do_all(libctx, OSSL_OP_DIGEST,
                       (void (*)(void *, void *))fn, arg,
                       evp_md_from_algorithm, evp_md_up_ref, evp_md_free);
}

EVP_MD *evp_digest_fetch_from_prov(OSSL_PROVIDER *prov,
                                   const char *algorithm,
                                   const char *properties)
{
    return evp_generic_fetch_from_prov(prov, OSSL_OP_DIGEST,
                                       algorithm, properties,
                                       evp_md_from_algorithm,
                                       evp_md_up_ref,
                                       evp_md_free);
}

typedef struct {
    int md_nid;
    int hmac_nid;
} ossl_hmacmd_pair;

static const ossl_hmacmd_pair ossl_hmacmd_pairs[] = {
    {NID_sha1, NID_hmacWithSHA1},
    {NID_md5, NID_hmacWithMD5},
    {NID_sha224, NID_hmacWithSHA224},
    {NID_sha256, NID_hmacWithSHA256},
    {NID_sha384, NID_hmacWithSHA384},
    {NID_sha512, NID_hmacWithSHA512},
    {NID_id_GostR3411_94, NID_id_HMACGostR3411_94},
    {NID_id_GostR3411_2012_256, NID_id_tc26_hmac_gost_3411_2012_256},
    {NID_id_GostR3411_2012_512, NID_id_tc26_hmac_gost_3411_2012_512},
    {NID_sha3_224, NID_hmac_sha3_224},
    {NID_sha3_256, NID_hmac_sha3_256},
    {NID_sha3_384, NID_hmac_sha3_384},
    {NID_sha3_512, NID_hmac_sha3_512},
    {NID_sha512_224, NID_hmacWithSHA512_224},
    {NID_sha512_256, NID_hmacWithSHA512_256}
};

int ossl_hmac2mdnid(int hmac_nid)
{
    int md_nid = NID_undef;
    size_t i;

    for (i = 0; i < OSSL_NELEM(ossl_hmacmd_pairs); i++) {
        if (ossl_hmacmd_pairs[i].hmac_nid == hmac_nid) {
            md_nid = ossl_hmacmd_pairs[i].md_nid;
            break;
        }
    }

    return md_nid;
}

int ossl_md2hmacnid(int md_nid)
{
    int hmac_nid = NID_undef;
    size_t i;

    for (i = 0; i < OSSL_NELEM(ossl_hmacmd_pairs); i++) {
        if (ossl_hmacmd_pairs[i].md_nid == md_nid) {
            hmac_nid = ossl_hmacmd_pairs[i].hmac_nid;
            break;
        }
    }

    return hmac_nid;
}
