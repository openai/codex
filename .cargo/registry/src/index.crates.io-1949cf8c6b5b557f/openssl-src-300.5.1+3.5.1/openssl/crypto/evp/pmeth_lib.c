/*
 * Copyright 2006-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Low level key APIs (DH etc) are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include <stdlib.h>
#ifndef FIPS_MODULE
# include <openssl/engine.h>
#endif
#include <openssl/evp.h>
#include <openssl/core_names.h>
#include <openssl/dh.h>
#include <openssl/rsa.h>
#include <openssl/kdf.h>
#include "internal/cryptlib.h"
#ifndef FIPS_MODULE
# include "crypto/asn1.h"
#endif
#include "crypto/evp.h"
#include "crypto/dh.h"
#include "crypto/ec.h"
#include "internal/ffc.h"
#include "internal/numbers.h"
#include "internal/provider.h"
#include "evp_local.h"

#ifndef FIPS_MODULE

static int evp_pkey_ctx_store_cached_data(EVP_PKEY_CTX *ctx,
                                          int keytype, int optype,
                                          int cmd, const char *name,
                                          const void *data, size_t data_len);
static void evp_pkey_ctx_free_cached_data(EVP_PKEY_CTX *ctx,
                                          int cmd, const char *name);
static void evp_pkey_ctx_free_all_cached_data(EVP_PKEY_CTX *ctx);

typedef const EVP_PKEY_METHOD *(*pmeth_fn)(void);
typedef int sk_cmp_fn_type(const char *const *a, const char *const *b);

static STACK_OF(EVP_PKEY_METHOD) *app_pkey_methods = NULL;

/* This array needs to be in order of NIDs */
static pmeth_fn standard_methods[] = {
    ossl_rsa_pkey_method,
# ifndef OPENSSL_NO_DH
    ossl_dh_pkey_method,
# endif
# ifndef OPENSSL_NO_DSA
    ossl_dsa_pkey_method,
# endif
# ifndef OPENSSL_NO_EC
    ossl_ec_pkey_method,
# endif
    ossl_rsa_pss_pkey_method,
# ifndef OPENSSL_NO_DH
    ossl_dhx_pkey_method,
# endif
# ifndef OPENSSL_NO_ECX
    ossl_ecx25519_pkey_method,
    ossl_ecx448_pkey_method,
    ossl_ed25519_pkey_method,
    ossl_ed448_pkey_method,
# endif
};

DECLARE_OBJ_BSEARCH_CMP_FN(const EVP_PKEY_METHOD *, pmeth_fn, pmeth_func);

static int pmeth_func_cmp(const EVP_PKEY_METHOD *const *a, pmeth_fn const *b)
{
    return ((*a)->pkey_id - ((**b)())->pkey_id);
}

IMPLEMENT_OBJ_BSEARCH_CMP_FN(const EVP_PKEY_METHOD *, pmeth_fn, pmeth_func);

static int pmeth_cmp(const EVP_PKEY_METHOD *const *a,
                     const EVP_PKEY_METHOD *const *b)
{
    return ((*a)->pkey_id - (*b)->pkey_id);
}

static const EVP_PKEY_METHOD *evp_pkey_meth_find_added_by_application(int type)
{
    if (app_pkey_methods != NULL) {
        int idx;
        EVP_PKEY_METHOD tmp;

        tmp.pkey_id = type;
        idx = sk_EVP_PKEY_METHOD_find(app_pkey_methods, &tmp);
        if (idx >= 0)
            return sk_EVP_PKEY_METHOD_value(app_pkey_methods, idx);
    }
    return NULL;
}

const EVP_PKEY_METHOD *EVP_PKEY_meth_find(int type)
{
    pmeth_fn *ret;
    EVP_PKEY_METHOD tmp;
    const EVP_PKEY_METHOD *t;

    if ((t = evp_pkey_meth_find_added_by_application(type)) != NULL)
        return t;

    tmp.pkey_id = type;
    t = &tmp;
    ret = OBJ_bsearch_pmeth_func(&t, standard_methods,
                                 OSSL_NELEM(standard_methods));
    if (ret == NULL || *ret == NULL)
        return NULL;
    return (**ret)();
}

EVP_PKEY_METHOD *EVP_PKEY_meth_new(int id, int flags)
{
    EVP_PKEY_METHOD *pmeth;

    pmeth = OPENSSL_zalloc(sizeof(*pmeth));
    if (pmeth == NULL)
        return NULL;

    pmeth->pkey_id = id;
    pmeth->flags = flags | EVP_PKEY_FLAG_DYNAMIC;
    return pmeth;
}
#endif /* FIPS_MODULE */

int evp_pkey_ctx_state(const EVP_PKEY_CTX *ctx)
{
    if (ctx->operation == EVP_PKEY_OP_UNDEFINED)
        return EVP_PKEY_STATE_UNKNOWN;

    if ((EVP_PKEY_CTX_IS_DERIVE_OP(ctx)
         && ctx->op.kex.algctx != NULL)
        || (EVP_PKEY_CTX_IS_SIGNATURE_OP(ctx)
            && ctx->op.sig.algctx != NULL)
        || (EVP_PKEY_CTX_IS_ASYM_CIPHER_OP(ctx)
            && ctx->op.ciph.algctx != NULL)
        || (EVP_PKEY_CTX_IS_GEN_OP(ctx)
            && ctx->op.keymgmt.genctx != NULL)
        || (EVP_PKEY_CTX_IS_KEM_OP(ctx)
            && ctx->op.encap.algctx != NULL))
        return EVP_PKEY_STATE_PROVIDER;

    return EVP_PKEY_STATE_LEGACY;
}

static EVP_PKEY_CTX *int_ctx_new(OSSL_LIB_CTX *libctx,
                                 EVP_PKEY *pkey, ENGINE *e,
                                 const char *keytype, const char *propquery,
                                 int id)

{
    EVP_PKEY_CTX *ret = NULL;
    const EVP_PKEY_METHOD *pmeth = NULL, *app_pmeth = NULL;
    EVP_KEYMGMT *keymgmt = NULL;

    /* Code below to be removed when legacy support is dropped. */
    /* BEGIN legacy */
    if (id == -1) {
        if (pkey != NULL && !evp_pkey_is_provided(pkey)) {
            id = pkey->type;
        } else {
            if (pkey != NULL) {
                /* Must be provided if we get here */
                keytype = EVP_KEYMGMT_get0_name(pkey->keymgmt);
            }
#ifndef FIPS_MODULE
            if (keytype != NULL) {
                id = evp_pkey_name2type(keytype);
                if (id == NID_undef)
                    id = -1;
            }
#endif
        }
    }
    /* If no ID was found here, we can only resort to find a keymgmt */
    if (id == -1) {
#ifndef FIPS_MODULE
        /* Using engine with a key without id will not work */
        if (e != NULL) {
            ERR_raise(ERR_LIB_EVP, EVP_R_UNSUPPORTED_ALGORITHM);
            return NULL;
        }
#endif
        goto common;
    }

#ifndef FIPS_MODULE
    /*
     * Here, we extract what information we can for the purpose of
     * supporting usage with implementations from providers, to make
     * for a smooth transition from legacy stuff to provider based stuff.
     *
     * If an engine is given, this is entirely legacy, and we should not
     * pretend anything else, so we clear the name.
     */
    if (e != NULL)
        keytype = NULL;
    if (e == NULL && (pkey == NULL || pkey->foreign == 0))
        keytype = OBJ_nid2sn(id);

# ifndef OPENSSL_NO_ENGINE
    if (e == NULL && pkey != NULL)
        e = pkey->pmeth_engine != NULL ? pkey->pmeth_engine : pkey->engine;
    /* Try to find an ENGINE which implements this method */
    if (e != NULL) {
        if (!ENGINE_init(e)) {
            ERR_raise(ERR_LIB_EVP, ERR_R_ENGINE_LIB);
            return NULL;
        }
    } else {
        e = ENGINE_get_pkey_meth_engine(id);
    }

    /*
     * If an ENGINE handled this method look it up. Otherwise use internal
     * tables.
     */
    if (e != NULL)
        pmeth = ENGINE_get_pkey_meth(e, id);
    else
# endif /* OPENSSL_NO_ENGINE */
    if (pkey != NULL && pkey->foreign)
        pmeth = EVP_PKEY_meth_find(id);
    else
        app_pmeth = pmeth = evp_pkey_meth_find_added_by_application(id);

    /* END legacy */
#endif /* FIPS_MODULE */
 common:
    /*
     * If there's no engine and no app supplied pmeth and there's a name, we try
     * fetching a provider implementation.
     */
    if (e == NULL && app_pmeth == NULL && keytype != NULL) {
        /*
         * If |pkey| is given and is provided, we take a reference to its
         * keymgmt.  Otherwise, we fetch one for the keytype we got. This
         * is to ensure that operation init functions can access what they
         * need through this single pointer.
         */
        if (pkey != NULL && pkey->keymgmt != NULL) {
            if (!EVP_KEYMGMT_up_ref(pkey->keymgmt))
                ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
            else
                keymgmt = pkey->keymgmt;
        } else {
            keymgmt = EVP_KEYMGMT_fetch(libctx, keytype, propquery);
        }
        if (keymgmt == NULL)
            return NULL;   /* EVP_KEYMGMT_fetch() recorded an error */

#ifndef FIPS_MODULE
        /*
         * Chase down the legacy NID, as that might be needed for diverse
         * purposes, such as ensure that EVP_PKEY_type() can return sensible
         * values. We go through all keymgmt names, because the keytype
         * that's passed to this function doesn't necessarily translate
         * directly.
         */
        if (keymgmt != NULL) {
            int tmp_id = evp_keymgmt_get_legacy_alg(keymgmt);

            if (tmp_id != NID_undef) {
                if (id == -1) {
                    id = tmp_id;
                } else {
                    /*
                     * It really really shouldn't differ.  If it still does,
                     * something is very wrong.
                     */
                    if (!ossl_assert(id == tmp_id)) {
                        ERR_raise(ERR_LIB_EVP, ERR_R_INTERNAL_ERROR);
                        EVP_KEYMGMT_free(keymgmt);
                        return NULL;
                    }
                }
            }
        }
#endif
    }

    if (pmeth == NULL && keymgmt == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_UNSUPPORTED_ALGORITHM);
    } else {
        ret = OPENSSL_zalloc(sizeof(*ret));
    }

#if !defined(OPENSSL_NO_ENGINE) && !defined(FIPS_MODULE)
    if ((ret == NULL || pmeth == NULL) && e != NULL)
        ENGINE_finish(e);
#endif

    if (ret == NULL) {
        EVP_KEYMGMT_free(keymgmt);
        return NULL;
    }
    if (propquery != NULL) {
        ret->propquery = OPENSSL_strdup(propquery);
        if (ret->propquery == NULL) {
            OPENSSL_free(ret);
            EVP_KEYMGMT_free(keymgmt);
            return NULL;
        }
    }
    ret->libctx = libctx;
    ret->keytype = keytype;
    ret->keymgmt = keymgmt;
    ret->legacy_keytype = id;
    ret->engine = e;
    ret->pmeth = pmeth;
    ret->operation = EVP_PKEY_OP_UNDEFINED;

    if (pkey != NULL && !EVP_PKEY_up_ref(pkey)) {
        EVP_PKEY_CTX_free(ret);
        return NULL;
    }

    ret->pkey = pkey;

    if (pmeth != NULL && pmeth->init != NULL) {
        if (pmeth->init(ret) <= 0) {
            ret->pmeth = NULL;
            EVP_PKEY_CTX_free(ret);
            return NULL;
        }
    }

    return ret;
}

/*- All methods below can also be used in FIPS_MODULE */

EVP_PKEY_CTX *EVP_PKEY_CTX_new_from_name(OSSL_LIB_CTX *libctx,
                                         const char *name,
                                         const char *propquery)
{
    return int_ctx_new(libctx, NULL, NULL, name, propquery, -1);
}

EVP_PKEY_CTX *EVP_PKEY_CTX_new_from_pkey(OSSL_LIB_CTX *libctx, EVP_PKEY *pkey,
                                         const char *propquery)
{
    return int_ctx_new(libctx, pkey, NULL, NULL, propquery, -1);
}

void evp_pkey_ctx_free_old_ops(EVP_PKEY_CTX *ctx)
{
    if (EVP_PKEY_CTX_IS_SIGNATURE_OP(ctx)) {
        if (ctx->op.sig.algctx != NULL && ctx->op.sig.signature != NULL)
            ctx->op.sig.signature->freectx(ctx->op.sig.algctx);
        EVP_SIGNATURE_free(ctx->op.sig.signature);
        ctx->op.sig.algctx = NULL;
        ctx->op.sig.signature = NULL;
    } else if (EVP_PKEY_CTX_IS_DERIVE_OP(ctx)) {
        if (ctx->op.kex.algctx != NULL && ctx->op.kex.exchange != NULL)
            ctx->op.kex.exchange->freectx(ctx->op.kex.algctx);
        EVP_KEYEXCH_free(ctx->op.kex.exchange);
        ctx->op.kex.algctx = NULL;
        ctx->op.kex.exchange = NULL;
    } else if (EVP_PKEY_CTX_IS_KEM_OP(ctx)) {
        if (ctx->op.encap.algctx != NULL && ctx->op.encap.kem != NULL)
            ctx->op.encap.kem->freectx(ctx->op.encap.algctx);
        EVP_KEM_free(ctx->op.encap.kem);
        ctx->op.encap.algctx = NULL;
        ctx->op.encap.kem = NULL;
    }
    else if (EVP_PKEY_CTX_IS_ASYM_CIPHER_OP(ctx)) {
        if (ctx->op.ciph.algctx != NULL && ctx->op.ciph.cipher != NULL)
            ctx->op.ciph.cipher->freectx(ctx->op.ciph.algctx);
        EVP_ASYM_CIPHER_free(ctx->op.ciph.cipher);
        ctx->op.ciph.algctx = NULL;
        ctx->op.ciph.cipher = NULL;
    } else if (EVP_PKEY_CTX_IS_GEN_OP(ctx)) {
        if (ctx->op.keymgmt.genctx != NULL && ctx->keymgmt != NULL)
            evp_keymgmt_gen_cleanup(ctx->keymgmt, ctx->op.keymgmt.genctx);
    }
}

void EVP_PKEY_CTX_free(EVP_PKEY_CTX *ctx)
{
    if (ctx == NULL)
        return;
    if (ctx->pmeth && ctx->pmeth->cleanup)
        ctx->pmeth->cleanup(ctx);

    evp_pkey_ctx_free_old_ops(ctx);
#ifndef FIPS_MODULE
    evp_pkey_ctx_free_all_cached_data(ctx);
#endif
    EVP_KEYMGMT_free(ctx->keymgmt);

    OPENSSL_free(ctx->propquery);
    EVP_PKEY_free(ctx->pkey);
    EVP_PKEY_free(ctx->peerkey);
#if !defined(OPENSSL_NO_ENGINE) && !defined(FIPS_MODULE)
    ENGINE_finish(ctx->engine);
#endif
    BN_free(ctx->rsa_pubexp);
    OPENSSL_free(ctx);
}

#ifndef FIPS_MODULE

void EVP_PKEY_meth_get0_info(int *ppkey_id, int *pflags,
                             const EVP_PKEY_METHOD *meth)
{
    if (ppkey_id)
        *ppkey_id = meth->pkey_id;
    if (pflags)
        *pflags = meth->flags;
}

void EVP_PKEY_meth_copy(EVP_PKEY_METHOD *dst, const EVP_PKEY_METHOD *src)
{
    int pkey_id = dst->pkey_id;
    int flags = dst->flags;

    *dst = *src;

    /* We only copy the function pointers so restore the other values */
    dst->pkey_id = pkey_id;
    dst->flags = flags;
}

void EVP_PKEY_meth_free(EVP_PKEY_METHOD *pmeth)
{
    if (pmeth && (pmeth->flags & EVP_PKEY_FLAG_DYNAMIC))
        OPENSSL_free(pmeth);
}

EVP_PKEY_CTX *EVP_PKEY_CTX_new(EVP_PKEY *pkey, ENGINE *e)
{
    return int_ctx_new(NULL, pkey, e, NULL, NULL, -1);
}

EVP_PKEY_CTX *EVP_PKEY_CTX_new_id(int id, ENGINE *e)
{
    return int_ctx_new(NULL, NULL, e, NULL, NULL, id);
}

EVP_PKEY_CTX *EVP_PKEY_CTX_dup(const EVP_PKEY_CTX *pctx)
{
    EVP_PKEY_CTX *rctx;

# ifndef OPENSSL_NO_ENGINE
    /* Make sure it's safe to copy a pkey context using an ENGINE */
    if (pctx->engine && !ENGINE_init(pctx->engine)) {
        ERR_raise(ERR_LIB_EVP, ERR_R_ENGINE_LIB);
        return 0;
    }
# endif
    rctx = OPENSSL_zalloc(sizeof(*rctx));
    if (rctx == NULL)
        return NULL;

    if (pctx->pkey != NULL && !EVP_PKEY_up_ref(pctx->pkey))
        goto err;

    rctx->pkey = pctx->pkey;
    rctx->operation = pctx->operation;
    rctx->libctx = pctx->libctx;
    rctx->keytype = pctx->keytype;
    rctx->propquery = NULL;
    if (pctx->propquery != NULL) {
        rctx->propquery = OPENSSL_strdup(pctx->propquery);
        if (rctx->propquery == NULL)
            goto err;
    }
    rctx->legacy_keytype = pctx->legacy_keytype;

    if (pctx->keymgmt != NULL) {
        if (!EVP_KEYMGMT_up_ref(pctx->keymgmt))
            goto err;
        rctx->keymgmt = pctx->keymgmt;
    }

    if (EVP_PKEY_CTX_IS_DERIVE_OP(pctx)) {
        if (pctx->op.kex.exchange != NULL) {
            rctx->op.kex.exchange = pctx->op.kex.exchange;
            if (!EVP_KEYEXCH_up_ref(rctx->op.kex.exchange))
                goto err;
        }
        if (pctx->op.kex.algctx != NULL) {
            if (!ossl_assert(pctx->op.kex.exchange != NULL))
                goto err;

            if (pctx->op.kex.exchange->dupctx != NULL)
                rctx->op.kex.algctx
                    = pctx->op.kex.exchange->dupctx(pctx->op.kex.algctx);

            if (rctx->op.kex.algctx == NULL) {
                EVP_KEYEXCH_free(rctx->op.kex.exchange);
                rctx->op.kex.exchange = NULL;
                goto err;
            }
            return rctx;
        }
    } else if (EVP_PKEY_CTX_IS_SIGNATURE_OP(pctx)) {
        if (pctx->op.sig.signature != NULL) {
            rctx->op.sig.signature = pctx->op.sig.signature;
            if (!EVP_SIGNATURE_up_ref(rctx->op.sig.signature))
                goto err;
        }
        if (pctx->op.sig.algctx != NULL) {
            if (!ossl_assert(pctx->op.sig.signature != NULL))
                goto err;

            if (pctx->op.sig.signature->dupctx != NULL)
                rctx->op.sig.algctx
                    = pctx->op.sig.signature->dupctx(pctx->op.sig.algctx);

            if (rctx->op.sig.algctx == NULL) {
                EVP_SIGNATURE_free(rctx->op.sig.signature);
                rctx->op.sig.signature = NULL;
                goto err;
            }
            return rctx;
        }
    } else if (EVP_PKEY_CTX_IS_ASYM_CIPHER_OP(pctx)) {
        if (pctx->op.ciph.cipher != NULL) {
            rctx->op.ciph.cipher = pctx->op.ciph.cipher;
            if (!EVP_ASYM_CIPHER_up_ref(rctx->op.ciph.cipher))
                goto err;
        }
        if (pctx->op.ciph.algctx != NULL) {
            if (!ossl_assert(pctx->op.ciph.cipher != NULL))
                goto err;

            if (pctx->op.ciph.cipher->dupctx != NULL)
                rctx->op.ciph.algctx
                    = pctx->op.ciph.cipher->dupctx(pctx->op.ciph.algctx);

            if (rctx->op.ciph.algctx == NULL) {
                EVP_ASYM_CIPHER_free(rctx->op.ciph.cipher);
                rctx->op.ciph.cipher = NULL;
                goto err;
            }
            return rctx;
        }
    } else if (EVP_PKEY_CTX_IS_KEM_OP(pctx)) {
        if (pctx->op.encap.kem != NULL) {
            rctx->op.encap.kem = pctx->op.encap.kem;
            if (!EVP_KEM_up_ref(rctx->op.encap.kem))
                goto err;
        }
        if (pctx->op.encap.algctx != NULL) {
            if (!ossl_assert(pctx->op.encap.kem != NULL))
                goto err;

            if (pctx->op.encap.kem->dupctx != NULL)
                rctx->op.encap.algctx
                    = pctx->op.encap.kem->dupctx(pctx->op.encap.algctx);

            if (rctx->op.encap.algctx == NULL) {
                EVP_KEM_free(rctx->op.encap.kem);
                rctx->op.encap.kem = NULL;
                goto err;
            }
            return rctx;
        }
    } else if (EVP_PKEY_CTX_IS_GEN_OP(pctx)) {
        /* Not supported - This would need a gen_dupctx() to work */
        goto err;
    }

    rctx->pmeth = pctx->pmeth;
# ifndef OPENSSL_NO_ENGINE
    rctx->engine = pctx->engine;
# endif

    if (pctx->peerkey != NULL && !EVP_PKEY_up_ref(pctx->peerkey))
        goto err;

    rctx->peerkey = pctx->peerkey;

    if (pctx->pmeth == NULL) {
        if (rctx->operation == EVP_PKEY_OP_UNDEFINED) {
            EVP_KEYMGMT *tmp_keymgmt = pctx->keymgmt;
            void *provkey;

            if (pctx->pkey == NULL)
                return rctx;

            provkey = evp_pkey_export_to_provider(pctx->pkey, pctx->libctx,
                                                  &tmp_keymgmt, pctx->propquery);
            if (provkey == NULL)
                goto err;
            if (!EVP_KEYMGMT_up_ref(tmp_keymgmt))
                goto err;
            EVP_KEYMGMT_free(rctx->keymgmt);
            rctx->keymgmt = tmp_keymgmt;
            return rctx;
        }
    } else if (pctx->pmeth->copy(rctx, pctx) > 0) {
        return rctx;
    }
err:
    rctx->pmeth = NULL;
    EVP_PKEY_CTX_free(rctx);
    return NULL;
}

int EVP_PKEY_meth_add0(const EVP_PKEY_METHOD *pmeth)
{
    if (app_pkey_methods == NULL) {
        app_pkey_methods = sk_EVP_PKEY_METHOD_new(pmeth_cmp);
        if (app_pkey_methods == NULL) {
            ERR_raise(ERR_LIB_EVP, ERR_R_CRYPTO_LIB);
            return 0;
        }
    }
    if (!sk_EVP_PKEY_METHOD_push(app_pkey_methods, pmeth)) {
        ERR_raise(ERR_LIB_EVP, ERR_R_CRYPTO_LIB);
        return 0;
    }
    sk_EVP_PKEY_METHOD_sort(app_pkey_methods);
    return 1;
}

void evp_app_cleanup_int(void)
{
    if (app_pkey_methods != NULL)
        sk_EVP_PKEY_METHOD_pop_free(app_pkey_methods, EVP_PKEY_meth_free);
}

int EVP_PKEY_meth_remove(const EVP_PKEY_METHOD *pmeth)
{
    const EVP_PKEY_METHOD *ret;

    ret = sk_EVP_PKEY_METHOD_delete_ptr(app_pkey_methods, pmeth);

    return ret == NULL ? 0 : 1;
}

size_t EVP_PKEY_meth_get_count(void)
{
    size_t rv = OSSL_NELEM(standard_methods);

    if (app_pkey_methods)
        rv += sk_EVP_PKEY_METHOD_num(app_pkey_methods);
    return rv;
}

const EVP_PKEY_METHOD *EVP_PKEY_meth_get0(size_t idx)
{
    if (idx < OSSL_NELEM(standard_methods))
        return (standard_methods[idx])();
    if (app_pkey_methods == NULL)
        return NULL;
    idx -= OSSL_NELEM(standard_methods);
    if (idx >= (size_t)sk_EVP_PKEY_METHOD_num(app_pkey_methods))
        return NULL;
    return sk_EVP_PKEY_METHOD_value(app_pkey_methods, idx);
}
#endif

int EVP_PKEY_CTX_is_a(EVP_PKEY_CTX *ctx, const char *keytype)
{
#ifndef FIPS_MODULE
    if (evp_pkey_ctx_is_legacy(ctx))
        return (ctx->pmeth->pkey_id == evp_pkey_name2type(keytype));
#endif
    return EVP_KEYMGMT_is_a(ctx->keymgmt, keytype);
}

int EVP_PKEY_CTX_set_params(EVP_PKEY_CTX *ctx, const OSSL_PARAM *params)
{
    switch (evp_pkey_ctx_state(ctx)) {
    case EVP_PKEY_STATE_PROVIDER:
        if (EVP_PKEY_CTX_IS_DERIVE_OP(ctx)
            && ctx->op.kex.exchange != NULL
            && ctx->op.kex.exchange->set_ctx_params != NULL)
            return
                ctx->op.kex.exchange->set_ctx_params(ctx->op.kex.algctx,
                                                     params);
        if (EVP_PKEY_CTX_IS_SIGNATURE_OP(ctx)
            && ctx->op.sig.signature != NULL
            && ctx->op.sig.signature->set_ctx_params != NULL)
            return
                ctx->op.sig.signature->set_ctx_params(ctx->op.sig.algctx,
                                                      params);
        if (EVP_PKEY_CTX_IS_ASYM_CIPHER_OP(ctx)
            && ctx->op.ciph.cipher != NULL
            && ctx->op.ciph.cipher->set_ctx_params != NULL)
            return
                ctx->op.ciph.cipher->set_ctx_params(ctx->op.ciph.algctx,
                                                    params);
        if (EVP_PKEY_CTX_IS_GEN_OP(ctx)
            && ctx->keymgmt != NULL
            && ctx->keymgmt->gen_set_params != NULL)
            return
                evp_keymgmt_gen_set_params(ctx->keymgmt, ctx->op.keymgmt.genctx,
                                           params);
        if (EVP_PKEY_CTX_IS_KEM_OP(ctx)
            && ctx->op.encap.kem != NULL
            && ctx->op.encap.kem->set_ctx_params != NULL)
            return
                ctx->op.encap.kem->set_ctx_params(ctx->op.encap.algctx,
                                                  params);
        break;
    case EVP_PKEY_STATE_UNKNOWN:
        break;
#ifndef FIPS_MODULE
    case EVP_PKEY_STATE_LEGACY:
        return evp_pkey_ctx_set_params_to_ctrl(ctx, params);
#endif
    }
    return 0;
}

int EVP_PKEY_CTX_get_params(EVP_PKEY_CTX *ctx, OSSL_PARAM *params)
{
    switch (evp_pkey_ctx_state(ctx)) {
    case EVP_PKEY_STATE_PROVIDER:
        if (EVP_PKEY_CTX_IS_DERIVE_OP(ctx)
            && ctx->op.kex.exchange != NULL
            && ctx->op.kex.exchange->get_ctx_params != NULL)
            return
                ctx->op.kex.exchange->get_ctx_params(ctx->op.kex.algctx,
                                                     params);
        if (EVP_PKEY_CTX_IS_SIGNATURE_OP(ctx)
            && ctx->op.sig.signature != NULL
            && ctx->op.sig.signature->get_ctx_params != NULL)
            return
                ctx->op.sig.signature->get_ctx_params(ctx->op.sig.algctx,
                                                      params);
        if (EVP_PKEY_CTX_IS_ASYM_CIPHER_OP(ctx)
            && ctx->op.ciph.cipher != NULL
            && ctx->op.ciph.cipher->get_ctx_params != NULL)
            return
                ctx->op.ciph.cipher->get_ctx_params(ctx->op.ciph.algctx,
                                                    params);
        if (EVP_PKEY_CTX_IS_KEM_OP(ctx)
            && ctx->op.encap.kem != NULL
            && ctx->op.encap.kem->get_ctx_params != NULL)
            return
                ctx->op.encap.kem->get_ctx_params(ctx->op.encap.algctx,
                                                  params);
        if (EVP_PKEY_CTX_IS_GEN_OP(ctx)
            && ctx->keymgmt != NULL
            && ctx->keymgmt->gen_get_params != NULL)
            return
                evp_keymgmt_gen_get_params(ctx->keymgmt, ctx->op.keymgmt.genctx,
                                           params);
        break;
    case EVP_PKEY_STATE_UNKNOWN:
        break;
#ifndef FIPS_MODULE
    case EVP_PKEY_STATE_LEGACY:
        return evp_pkey_ctx_get_params_to_ctrl(ctx, params);
#endif
    }
    return 0;
}

#ifndef FIPS_MODULE
const OSSL_PARAM *EVP_PKEY_CTX_gettable_params(const EVP_PKEY_CTX *ctx)
{
    void *provctx;

    if (EVP_PKEY_CTX_IS_DERIVE_OP(ctx)
            && ctx->op.kex.exchange != NULL
            && ctx->op.kex.exchange->gettable_ctx_params != NULL) {
        provctx = ossl_provider_ctx(EVP_KEYEXCH_get0_provider(ctx->op.kex.exchange));
        return ctx->op.kex.exchange->gettable_ctx_params(ctx->op.kex.algctx,
                                                         provctx);
    }
    if (EVP_PKEY_CTX_IS_SIGNATURE_OP(ctx)
            && ctx->op.sig.signature != NULL
            && ctx->op.sig.signature->gettable_ctx_params != NULL) {
        provctx = ossl_provider_ctx(
                      EVP_SIGNATURE_get0_provider(ctx->op.sig.signature));
        return ctx->op.sig.signature->gettable_ctx_params(ctx->op.sig.algctx,
                                                          provctx);
    }
    if (EVP_PKEY_CTX_IS_ASYM_CIPHER_OP(ctx)
            && ctx->op.ciph.cipher != NULL
            && ctx->op.ciph.cipher->gettable_ctx_params != NULL) {
        provctx = ossl_provider_ctx(
                      EVP_ASYM_CIPHER_get0_provider(ctx->op.ciph.cipher));
        return ctx->op.ciph.cipher->gettable_ctx_params(ctx->op.ciph.algctx,
                                                        provctx);
    }
    if (EVP_PKEY_CTX_IS_KEM_OP(ctx)
        && ctx->op.encap.kem != NULL
        && ctx->op.encap.kem->gettable_ctx_params != NULL) {
        provctx = ossl_provider_ctx(EVP_KEM_get0_provider(ctx->op.encap.kem));
        return ctx->op.encap.kem->gettable_ctx_params(ctx->op.encap.algctx,
                                                      provctx);
    }
    if (EVP_PKEY_CTX_IS_GEN_OP(ctx)
            && ctx->keymgmt != NULL
            && ctx->keymgmt->gen_gettable_params != NULL) {
        provctx = ossl_provider_ctx(EVP_KEYMGMT_get0_provider(ctx->keymgmt));
        return ctx->keymgmt->gen_gettable_params(ctx->op.keymgmt.genctx,
                                                 provctx);
    }
    return NULL;
}

const OSSL_PARAM *EVP_PKEY_CTX_settable_params(const EVP_PKEY_CTX *ctx)
{
    void *provctx;

    if (EVP_PKEY_CTX_IS_DERIVE_OP(ctx)
            && ctx->op.kex.exchange != NULL
            && ctx->op.kex.exchange->settable_ctx_params != NULL) {
        provctx = ossl_provider_ctx(EVP_KEYEXCH_get0_provider(ctx->op.kex.exchange));
        return ctx->op.kex.exchange->settable_ctx_params(ctx->op.kex.algctx,
                                                         provctx);
    }
    if (EVP_PKEY_CTX_IS_SIGNATURE_OP(ctx)
            && ctx->op.sig.signature != NULL
            && ctx->op.sig.signature->settable_ctx_params != NULL) {
        provctx = ossl_provider_ctx(
                      EVP_SIGNATURE_get0_provider(ctx->op.sig.signature));
        return ctx->op.sig.signature->settable_ctx_params(ctx->op.sig.algctx,
                                                          provctx);
    }
    if (EVP_PKEY_CTX_IS_ASYM_CIPHER_OP(ctx)
            && ctx->op.ciph.cipher != NULL
            && ctx->op.ciph.cipher->settable_ctx_params != NULL) {
        provctx = ossl_provider_ctx(
                      EVP_ASYM_CIPHER_get0_provider(ctx->op.ciph.cipher));
        return ctx->op.ciph.cipher->settable_ctx_params(ctx->op.ciph.algctx,
                                                        provctx);
    }
    if (EVP_PKEY_CTX_IS_GEN_OP(ctx)
            && ctx->keymgmt != NULL
            && ctx->keymgmt->gen_settable_params != NULL) {
        provctx = ossl_provider_ctx(EVP_KEYMGMT_get0_provider(ctx->keymgmt));
        return ctx->keymgmt->gen_settable_params(ctx->op.keymgmt.genctx,
                                                 provctx);
    }
    if (EVP_PKEY_CTX_IS_KEM_OP(ctx)
        && ctx->op.encap.kem != NULL
        && ctx->op.encap.kem->settable_ctx_params != NULL) {
        provctx = ossl_provider_ctx(EVP_KEM_get0_provider(ctx->op.encap.kem));
        return ctx->op.encap.kem->settable_ctx_params(ctx->op.encap.algctx,
                                                      provctx);
    }
    return NULL;
}

/*
 * Internal helpers for stricter EVP_PKEY_CTX_{set,get}_params().
 *
 * Return 1 on success, 0 or negative for errors.
 *
 * In particular they return -2 if any of the params is not supported.
 *
 * They are not available in FIPS_MODULE as they depend on
 *      - EVP_PKEY_CTX_{get,set}_params()
 *      - EVP_PKEY_CTX_{gettable,settable}_params()
 *
 */
int evp_pkey_ctx_set_params_strict(EVP_PKEY_CTX *ctx, OSSL_PARAM *params)
{
    if (ctx == NULL || params == NULL)
        return 0;

    /*
     * We only check for provider side EVP_PKEY_CTX.  For #legacy, we
     * depend on the translation that happens in EVP_PKEY_CTX_set_params()
     * call, and that the resulting ctrl call will return -2 if it doesn't
     * known the ctrl command number.
     */
    if (evp_pkey_ctx_is_provided(ctx)) {
        const OSSL_PARAM *settable = EVP_PKEY_CTX_settable_params(ctx);
        const OSSL_PARAM *p;

        for (p = params; p->key != NULL; p++) {
            /* Check the ctx actually understands this parameter */
            if (OSSL_PARAM_locate_const(settable, p->key) == NULL)
                return -2;
        }
    }

    return EVP_PKEY_CTX_set_params(ctx, params);
}

int evp_pkey_ctx_get_params_strict(EVP_PKEY_CTX *ctx, OSSL_PARAM *params)
{
    if (ctx == NULL || params == NULL)
        return 0;

    /*
     * We only check for provider side EVP_PKEY_CTX.  For #legacy, we
     * depend on the translation that happens in EVP_PKEY_CTX_get_params()
     * call, and that the resulting ctrl call will return -2 if it doesn't
     * known the ctrl command number.
     */
    if (evp_pkey_ctx_is_provided(ctx)) {
        const OSSL_PARAM *gettable = EVP_PKEY_CTX_gettable_params(ctx);
        const OSSL_PARAM *p;

        for (p = params; p->key != NULL; p++) {
            /* Check the ctx actually understands this parameter */
            if (OSSL_PARAM_locate_const(gettable, p->key) == NULL)
                return -2;
        }
    }

    return EVP_PKEY_CTX_get_params(ctx, params);
}

int EVP_PKEY_CTX_get_signature_md(EVP_PKEY_CTX *ctx, const EVP_MD **md)
{
    OSSL_PARAM sig_md_params[2], *p = sig_md_params;
    /* 80 should be big enough */
    char name[80] = "";
    const EVP_MD *tmp;

    if (ctx == NULL || !EVP_PKEY_CTX_IS_SIGNATURE_OP(ctx)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
        /* Uses the same return values as EVP_PKEY_CTX_ctrl */
        return -2;
    }

    if (ctx->op.sig.algctx == NULL)
        return EVP_PKEY_CTX_ctrl(ctx, -1, EVP_PKEY_OP_TYPE_SIG,
                                 EVP_PKEY_CTRL_GET_MD, 0, (void *)(md));

    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_SIGNATURE_PARAM_DIGEST,
                                            name,
                                            sizeof(name));
    *p = OSSL_PARAM_construct_end();

    if (!EVP_PKEY_CTX_get_params(ctx, sig_md_params))
        return 0;

    tmp = evp_get_digestbyname_ex(ctx->libctx, name);
    if (tmp == NULL)
        return 0;

    *md = tmp;

    return 1;
}

static int evp_pkey_ctx_set_md(EVP_PKEY_CTX *ctx, const EVP_MD *md,
                               int fallback, const char *param, int op,
                               int ctrl)
{
    OSSL_PARAM md_params[2], *p = md_params;
    const char *name;

    if (ctx == NULL || (ctx->operation & op) == 0) {
        ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
        /* Uses the same return values as EVP_PKEY_CTX_ctrl */
        return -2;
    }

    if (fallback)
        return EVP_PKEY_CTX_ctrl(ctx, -1, op, ctrl, 0, (void *)(md));

    if (md == NULL) {
        name = "";
    } else {
        name = EVP_MD_get0_name(md);
    }

    *p++ = OSSL_PARAM_construct_utf8_string(param,
                                            /*
                                             * Cast away the const. This is read
                                             * only so should be safe
                                             */
                                            (char *)name, 0);
    *p = OSSL_PARAM_construct_end();

    return EVP_PKEY_CTX_set_params(ctx, md_params);
}

int EVP_PKEY_CTX_set_signature_md(EVP_PKEY_CTX *ctx, const EVP_MD *md)
{
    return evp_pkey_ctx_set_md(ctx, md, ctx->op.sig.algctx == NULL,
                               OSSL_SIGNATURE_PARAM_DIGEST,
                               EVP_PKEY_OP_TYPE_SIG, EVP_PKEY_CTRL_MD);
}

int EVP_PKEY_CTX_set_tls1_prf_md(EVP_PKEY_CTX *ctx, const EVP_MD *md)
{
    return evp_pkey_ctx_set_md(ctx, md, ctx->op.kex.algctx == NULL,
                               OSSL_KDF_PARAM_DIGEST,
                               EVP_PKEY_OP_DERIVE, EVP_PKEY_CTRL_TLS_MD);
}

static int evp_pkey_ctx_set1_octet_string(EVP_PKEY_CTX *ctx, int fallback,
                                          const char *param, int op, int ctrl,
                                          const unsigned char *data,
                                          int datalen)
{
    OSSL_PARAM octet_string_params[2], *p = octet_string_params;

    if (ctx == NULL || (ctx->operation & op) == 0) {
        ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
        /* Uses the same return values as EVP_PKEY_CTX_ctrl */
        return -2;
    }

    /* Code below to be removed when legacy support is dropped. */
    if (fallback)
        return EVP_PKEY_CTX_ctrl(ctx, -1, op, ctrl, datalen, (void *)(data));
    /* end of legacy support */

    if (datalen < 0) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_LENGTH);
        return 0;
    }

    *p++ = OSSL_PARAM_construct_octet_string(param,
                                            /*
                                             * Cast away the const. This is read
                                             * only so should be safe
                                             */
                                            (unsigned char *)data,
                                            (size_t)datalen);
    *p = OSSL_PARAM_construct_end();

    return EVP_PKEY_CTX_set_params(ctx, octet_string_params);
}

static int evp_pkey_ctx_add1_octet_string(EVP_PKEY_CTX *ctx, int fallback,
                                          const char *param, int op, int ctrl,
                                          const unsigned char *data,
                                          int datalen)
{
    OSSL_PARAM os_params[2];
    const OSSL_PARAM *gettables;
    unsigned char *info = NULL;
    size_t info_len = 0;
    size_t info_alloc = 0;
    int ret = 0;

    if (ctx == NULL || (ctx->operation & op) == 0) {
        ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
        /* Uses the same return values as EVP_PKEY_CTX_ctrl */
        return -2;
    }

    /* Code below to be removed when legacy support is dropped. */
    if (fallback)
        return EVP_PKEY_CTX_ctrl(ctx, -1, op, ctrl, datalen, (void *)(data));
    /* end of legacy support */

    if (datalen < 0) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_LENGTH);
        return 0;
    } else if (datalen == 0) {
        return 1;
    }

    /* Check for older provider that doesn't support getting this parameter */
    gettables = EVP_PKEY_CTX_gettable_params(ctx);
    if (gettables == NULL || OSSL_PARAM_locate_const(gettables, param) == NULL)
        return evp_pkey_ctx_set1_octet_string(ctx, fallback, param, op, ctrl,
                                              data, datalen);

    /* Get the original value length */
    os_params[0] = OSSL_PARAM_construct_octet_string(param, NULL, 0);
    os_params[1] = OSSL_PARAM_construct_end();

    if (!EVP_PKEY_CTX_get_params(ctx, os_params))
        return 0;

    /* This should not happen but check to be sure. */
    if (os_params[0].return_size == OSSL_PARAM_UNMODIFIED)
        return 0;

    info_alloc = os_params[0].return_size + datalen;
    if (info_alloc == 0)
        return 0;
    info = OPENSSL_zalloc(info_alloc);
    if (info == NULL)
        return 0;
    info_len = os_params[0].return_size;

    os_params[0] = OSSL_PARAM_construct_octet_string(param, info, info_alloc);

    /* if we have data, then go get it */
    if (info_len > 0) {
        if (!EVP_PKEY_CTX_get_params(ctx, os_params))
            goto error;
    }

    /* Copy the input data */
    memcpy(&info[info_len], data, datalen);
    ret = EVP_PKEY_CTX_set_params(ctx, os_params);

 error:
    OPENSSL_clear_free(info, info_alloc);
    return ret;
}

int EVP_PKEY_CTX_set1_tls1_prf_secret(EVP_PKEY_CTX *ctx,
                                      const unsigned char *sec, int seclen)
{
    return evp_pkey_ctx_set1_octet_string(ctx, ctx->op.kex.algctx == NULL,
                                          OSSL_KDF_PARAM_SECRET,
                                          EVP_PKEY_OP_DERIVE,
                                          EVP_PKEY_CTRL_TLS_SECRET,
                                          sec, seclen);
}

int EVP_PKEY_CTX_add1_tls1_prf_seed(EVP_PKEY_CTX *ctx,
                                    const unsigned char *seed, int seedlen)
{
    return evp_pkey_ctx_set1_octet_string(ctx, ctx->op.kex.algctx == NULL,
                                          OSSL_KDF_PARAM_SEED,
                                          EVP_PKEY_OP_DERIVE,
                                          EVP_PKEY_CTRL_TLS_SEED,
                                          seed, seedlen);
}

int EVP_PKEY_CTX_set_hkdf_md(EVP_PKEY_CTX *ctx, const EVP_MD *md)
{
    return evp_pkey_ctx_set_md(ctx, md, ctx->op.kex.algctx == NULL,
                               OSSL_KDF_PARAM_DIGEST,
                               EVP_PKEY_OP_DERIVE, EVP_PKEY_CTRL_HKDF_MD);
}

int EVP_PKEY_CTX_set1_hkdf_salt(EVP_PKEY_CTX *ctx,
                                const unsigned char *salt, int saltlen)
{
    return evp_pkey_ctx_set1_octet_string(ctx, ctx->op.kex.algctx == NULL,
                                          OSSL_KDF_PARAM_SALT,
                                          EVP_PKEY_OP_DERIVE,
                                          EVP_PKEY_CTRL_HKDF_SALT,
                                          salt, saltlen);
}

int EVP_PKEY_CTX_set1_hkdf_key(EVP_PKEY_CTX *ctx,
                                      const unsigned char *key, int keylen)
{
    return evp_pkey_ctx_set1_octet_string(ctx, ctx->op.kex.algctx == NULL,
                                          OSSL_KDF_PARAM_KEY,
                                          EVP_PKEY_OP_DERIVE,
                                          EVP_PKEY_CTRL_HKDF_KEY,
                                          key, keylen);
}

int EVP_PKEY_CTX_add1_hkdf_info(EVP_PKEY_CTX *ctx,
                                      const unsigned char *info, int infolen)
{
    return evp_pkey_ctx_add1_octet_string(ctx, ctx->op.kex.algctx == NULL,
                                          OSSL_KDF_PARAM_INFO,
                                          EVP_PKEY_OP_DERIVE,
                                          EVP_PKEY_CTRL_HKDF_INFO,
                                          info, infolen);
}

int EVP_PKEY_CTX_set_hkdf_mode(EVP_PKEY_CTX *ctx, int mode)
{
    OSSL_PARAM int_params[2], *p = int_params;

    if (ctx == NULL || !EVP_PKEY_CTX_IS_DERIVE_OP(ctx)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
        /* Uses the same return values as EVP_PKEY_CTX_ctrl */
        return -2;
    }

    /* Code below to be removed when legacy support is dropped. */
    if (ctx->op.kex.algctx == NULL)
        return EVP_PKEY_CTX_ctrl(ctx, -1, EVP_PKEY_OP_DERIVE,
                                 EVP_PKEY_CTRL_HKDF_MODE, mode, NULL);
    /* end of legacy support */

    if (mode < 0) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_VALUE);
        return 0;
    }

    *p++ = OSSL_PARAM_construct_int(OSSL_KDF_PARAM_MODE, &mode);
    *p = OSSL_PARAM_construct_end();

    return EVP_PKEY_CTX_set_params(ctx, int_params);
}

int EVP_PKEY_CTX_set1_pbe_pass(EVP_PKEY_CTX *ctx, const char *pass,
                               int passlen)
{
    return evp_pkey_ctx_set1_octet_string(ctx, ctx->op.kex.algctx == NULL,
                                          OSSL_KDF_PARAM_PASSWORD,
                                          EVP_PKEY_OP_DERIVE,
                                          EVP_PKEY_CTRL_PASS,
                                          (const unsigned char *)pass, passlen);
}

int EVP_PKEY_CTX_set1_scrypt_salt(EVP_PKEY_CTX *ctx,
                                  const unsigned char *salt, int saltlen)
{
    return evp_pkey_ctx_set1_octet_string(ctx, ctx->op.kex.algctx == NULL,
                                          OSSL_KDF_PARAM_SALT,
                                          EVP_PKEY_OP_DERIVE,
                                          EVP_PKEY_CTRL_SCRYPT_SALT,
                                          salt, saltlen);
}

static int evp_pkey_ctx_set_uint64(EVP_PKEY_CTX *ctx, const char *param,
                                   int op, int ctrl, uint64_t val)
{
    OSSL_PARAM uint64_params[2], *p = uint64_params;

    if (ctx == NULL || !EVP_PKEY_CTX_IS_DERIVE_OP(ctx)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
        /* Uses the same return values as EVP_PKEY_CTX_ctrl */
        return -2;
    }

    /* Code below to be removed when legacy support is dropped. */
    if (ctx->op.kex.algctx == NULL)
        return EVP_PKEY_CTX_ctrl_uint64(ctx, -1, op, ctrl, val);
    /* end of legacy support */

    *p++ = OSSL_PARAM_construct_uint64(param, &val);
    *p = OSSL_PARAM_construct_end();

    return EVP_PKEY_CTX_set_params(ctx, uint64_params);
}

int EVP_PKEY_CTX_set_scrypt_N(EVP_PKEY_CTX *ctx, uint64_t n)
{
    return evp_pkey_ctx_set_uint64(ctx, OSSL_KDF_PARAM_SCRYPT_N,
                                   EVP_PKEY_OP_DERIVE, EVP_PKEY_CTRL_SCRYPT_N,
                                   n);
}

int EVP_PKEY_CTX_set_scrypt_r(EVP_PKEY_CTX *ctx, uint64_t r)
{
    return evp_pkey_ctx_set_uint64(ctx, OSSL_KDF_PARAM_SCRYPT_R,
                                   EVP_PKEY_OP_DERIVE, EVP_PKEY_CTRL_SCRYPT_R,
                                   r);
}

int EVP_PKEY_CTX_set_scrypt_p(EVP_PKEY_CTX *ctx, uint64_t p)
{
    return evp_pkey_ctx_set_uint64(ctx, OSSL_KDF_PARAM_SCRYPT_P,
                                   EVP_PKEY_OP_DERIVE, EVP_PKEY_CTRL_SCRYPT_P,
                                   p);
}

int EVP_PKEY_CTX_set_scrypt_maxmem_bytes(EVP_PKEY_CTX *ctx,
                                         uint64_t maxmem_bytes)
{
    return evp_pkey_ctx_set_uint64(ctx, OSSL_KDF_PARAM_SCRYPT_MAXMEM,
                                   EVP_PKEY_OP_DERIVE,
                                   EVP_PKEY_CTRL_SCRYPT_MAXMEM_BYTES,
                                   maxmem_bytes);
}

int EVP_PKEY_CTX_set_mac_key(EVP_PKEY_CTX *ctx, const unsigned char *key,
                             int keylen)
{
    return evp_pkey_ctx_set1_octet_string(ctx, ctx->op.keymgmt.genctx == NULL,
                                          OSSL_PKEY_PARAM_PRIV_KEY,
                                          EVP_PKEY_OP_KEYGEN,
                                          EVP_PKEY_CTRL_SET_MAC_KEY,
                                          key, keylen);
}

int EVP_PKEY_CTX_set_kem_op(EVP_PKEY_CTX *ctx, const char *op)
{
    OSSL_PARAM params[2], *p = params;

    if (ctx == NULL || op == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_VALUE);
        return 0;
    }
    if (!EVP_PKEY_CTX_IS_KEM_OP(ctx)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
        return -2;
    }
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KEM_PARAM_OPERATION,
                                            (char *)op, 0);
    *p = OSSL_PARAM_construct_end();
    return EVP_PKEY_CTX_set_params(ctx, params);
}

int EVP_PKEY_CTX_set1_id(EVP_PKEY_CTX *ctx, const void *id, int len)
{
    return EVP_PKEY_CTX_ctrl(ctx, -1, -1,
                             EVP_PKEY_CTRL_SET1_ID, (int)len, (void*)(id));
}

int EVP_PKEY_CTX_get1_id(EVP_PKEY_CTX *ctx, void *id)
{
    return EVP_PKEY_CTX_ctrl(ctx, -1, -1, EVP_PKEY_CTRL_GET1_ID, 0, (void*)id);
}

int EVP_PKEY_CTX_get1_id_len(EVP_PKEY_CTX *ctx, size_t *id_len)
{
    return EVP_PKEY_CTX_ctrl(ctx, -1, -1,
                             EVP_PKEY_CTRL_GET1_ID_LEN, 0, (void*)id_len);
}

static int evp_pkey_ctx_ctrl_int(EVP_PKEY_CTX *ctx, int keytype, int optype,
                                 int cmd, int p1, void *p2)
{
    int ret = 0;

    /*
     * If the method has a |digest_custom| function, we can relax the
     * operation type check, since this can be called before the operation
     * is initialized.
     */
    if (ctx->pmeth == NULL || ctx->pmeth->digest_custom == NULL) {
        if (ctx->operation == EVP_PKEY_OP_UNDEFINED) {
            ERR_raise(ERR_LIB_EVP, EVP_R_NO_OPERATION_SET);
            return -1;
        }

        if ((optype != -1) && !(ctx->operation & optype)) {
            ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_OPERATION);
            return -1;
        }
    }

    switch (evp_pkey_ctx_state(ctx)) {
    case EVP_PKEY_STATE_PROVIDER:
        return evp_pkey_ctx_ctrl_to_param(ctx, keytype, optype, cmd, p1, p2);
    case EVP_PKEY_STATE_UNKNOWN:
    case EVP_PKEY_STATE_LEGACY:
        if (ctx->pmeth == NULL || ctx->pmeth->ctrl == NULL) {
            ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
            return -2;
        }
        if ((keytype != -1) && (ctx->pmeth->pkey_id != keytype))
            return -1;

        ret = ctx->pmeth->ctrl(ctx, cmd, p1, p2);

        if (ret == -2)
            ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
        break;
    }
    return ret;
}

int EVP_PKEY_CTX_ctrl(EVP_PKEY_CTX *ctx, int keytype, int optype,
                      int cmd, int p1, void *p2)
{
    int ret = 0;

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
        return -2;
    }
    /* If unsupported, we don't want that reported here */
    ERR_set_mark();
    ret = evp_pkey_ctx_store_cached_data(ctx, keytype, optype,
                                         cmd, NULL, p2, p1);
    if (ret == -2) {
        ERR_pop_to_mark();
    } else {
        ERR_clear_last_mark();
        /*
         * If there was an error, there was an error.
         * If the operation isn't initialized yet, we also return, as
         * the saved values will be used then anyway.
         */
        if (ret < 1 || ctx->operation == EVP_PKEY_OP_UNDEFINED)
            return ret;
    }
    return evp_pkey_ctx_ctrl_int(ctx, keytype, optype, cmd, p1, p2);
}

int EVP_PKEY_CTX_ctrl_uint64(EVP_PKEY_CTX *ctx, int keytype, int optype,
                             int cmd, uint64_t value)
{
    return EVP_PKEY_CTX_ctrl(ctx, keytype, optype, cmd, 0, &value);
}


static int evp_pkey_ctx_ctrl_str_int(EVP_PKEY_CTX *ctx,
                                     const char *name, const char *value)
{
    int ret = 0;

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
        return -2;
    }

    switch (evp_pkey_ctx_state(ctx)) {
    case EVP_PKEY_STATE_PROVIDER:
        return evp_pkey_ctx_ctrl_str_to_param(ctx, name, value);
    case EVP_PKEY_STATE_UNKNOWN:
    case EVP_PKEY_STATE_LEGACY:
        if (ctx == NULL || ctx->pmeth == NULL || ctx->pmeth->ctrl_str == NULL) {
            ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
            return -2;
        }
        if (strcmp(name, "digest") == 0)
            ret = EVP_PKEY_CTX_md(ctx,
                                  EVP_PKEY_OP_TYPE_SIG | EVP_PKEY_OP_TYPE_CRYPT,
                                  EVP_PKEY_CTRL_MD, value);
        else
            ret = ctx->pmeth->ctrl_str(ctx, name, value);
        break;
    }

    return ret;
}

int EVP_PKEY_CTX_ctrl_str(EVP_PKEY_CTX *ctx,
                          const char *name, const char *value)
{
    int ret = 0;

    /* If unsupported, we don't want that reported here */
    ERR_set_mark();
    ret = evp_pkey_ctx_store_cached_data(ctx, -1, -1, -1,
                                         name, value, strlen(value) + 1);
    if (ret == -2) {
        ERR_pop_to_mark();
    } else {
        ERR_clear_last_mark();
        /*
         * If there was an error, there was an error.
         * If the operation isn't initialized yet, we also return, as
         * the saved values will be used then anyway.
         */
        if (ret < 1 || ctx->operation == EVP_PKEY_OP_UNDEFINED)
            return ret;
    }

    return evp_pkey_ctx_ctrl_str_int(ctx, name, value);
}

static int decode_cmd(int cmd, const char *name)
{
    if (cmd == -1) {
        /*
         * The consequence of the assertion not being true is that this
         * function will return -1, which will cause the calling functions
         * to signal that the command is unsupported...  in non-debug mode.
         */
        if (ossl_assert(name != NULL))
            if (strcmp(name, "distid") == 0 || strcmp(name, "hexdistid") == 0)
                cmd = EVP_PKEY_CTRL_SET1_ID;
    }

    return cmd;
}

static int evp_pkey_ctx_store_cached_data(EVP_PKEY_CTX *ctx,
                                          int keytype, int optype,
                                          int cmd, const char *name,
                                          const void *data, size_t data_len)
{
    /*
     * Check that it's one of the supported commands.  The ctrl commands
     * number cases here must correspond to the cases in the bottom switch
     * in this function.
     */
    switch (cmd = decode_cmd(cmd, name)) {
    case EVP_PKEY_CTRL_SET1_ID:
        break;
    default:
        ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
        return -2;
    }

    if (keytype != -1) {
        switch (evp_pkey_ctx_state(ctx)) {
        case EVP_PKEY_STATE_PROVIDER:
            if (ctx->keymgmt == NULL) {
                ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
                return -2;
            }
            if (!EVP_KEYMGMT_is_a(ctx->keymgmt,
                                  evp_pkey_type2name(keytype))) {
                ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_OPERATION);
                return -1;
            }
            break;
        case EVP_PKEY_STATE_UNKNOWN:
        case EVP_PKEY_STATE_LEGACY:
            if (ctx->pmeth == NULL) {
                ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
                return -2;
            }
            if (EVP_PKEY_type(ctx->pmeth->pkey_id) != EVP_PKEY_type(keytype)) {
                ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_OPERATION);
                return -1;
            }
            break;
        }
    }
    if (optype != -1 && (ctx->operation & optype) == 0) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_OPERATION);
        return -1;
    }

    switch (cmd) {
    case EVP_PKEY_CTRL_SET1_ID:
        evp_pkey_ctx_free_cached_data(ctx, cmd, name);
        if (name != NULL) {
            ctx->cached_parameters.dist_id_name = OPENSSL_strdup(name);
            if (ctx->cached_parameters.dist_id_name == NULL)
                return 0;
        }
        if (data_len > 0) {
            ctx->cached_parameters.dist_id = OPENSSL_memdup(data, data_len);
            if (ctx->cached_parameters.dist_id == NULL)
                return 0;
        }
        ctx->cached_parameters.dist_id_set = 1;
        ctx->cached_parameters.dist_id_len = data_len;
        break;
    }
    return 1;
}

static void evp_pkey_ctx_free_cached_data(EVP_PKEY_CTX *ctx,
                                          int cmd, const char *name)
{
    cmd = decode_cmd(cmd, name);
    switch (cmd) {
    case EVP_PKEY_CTRL_SET1_ID:
        OPENSSL_free(ctx->cached_parameters.dist_id);
        OPENSSL_free(ctx->cached_parameters.dist_id_name);
        ctx->cached_parameters.dist_id = NULL;
        ctx->cached_parameters.dist_id_name = NULL;
        break;
    }
}

static void evp_pkey_ctx_free_all_cached_data(EVP_PKEY_CTX *ctx)
{
    evp_pkey_ctx_free_cached_data(ctx, EVP_PKEY_CTRL_SET1_ID, NULL);
}

int evp_pkey_ctx_use_cached_data(EVP_PKEY_CTX *ctx)
{
    int ret = 1;

    if (ret && ctx->cached_parameters.dist_id_set) {
        const char *name = ctx->cached_parameters.dist_id_name;
        const void *val = ctx->cached_parameters.dist_id;
        size_t len = ctx->cached_parameters.dist_id_len;

        if (name != NULL)
            ret = evp_pkey_ctx_ctrl_str_int(ctx, name, val);
        else
            ret = evp_pkey_ctx_ctrl_int(ctx, -1, ctx->operation,
                                        EVP_PKEY_CTRL_SET1_ID,
                                        (int)len, (void *)val);
    }

    return ret;
}

OSSL_LIB_CTX *EVP_PKEY_CTX_get0_libctx(EVP_PKEY_CTX *ctx)
{
    return ctx->libctx;
}

const char *EVP_PKEY_CTX_get0_propq(const EVP_PKEY_CTX *ctx)
{
    return ctx->propquery;
}

const OSSL_PROVIDER *EVP_PKEY_CTX_get0_provider(const EVP_PKEY_CTX *ctx)
{
    if (EVP_PKEY_CTX_IS_SIGNATURE_OP(ctx)) {
        if (ctx->op.sig.signature != NULL)
            return EVP_SIGNATURE_get0_provider(ctx->op.sig.signature);
    } else if (EVP_PKEY_CTX_IS_DERIVE_OP(ctx)) {
        if (ctx->op.kex.exchange != NULL)
            return EVP_KEYEXCH_get0_provider(ctx->op.kex.exchange);
    } else if (EVP_PKEY_CTX_IS_KEM_OP(ctx)) {
        if (ctx->op.encap.kem != NULL)
            return EVP_KEM_get0_provider(ctx->op.encap.kem);
    } else if (EVP_PKEY_CTX_IS_ASYM_CIPHER_OP(ctx)) {
        if (ctx->op.ciph.cipher != NULL)
            return EVP_ASYM_CIPHER_get0_provider(ctx->op.ciph.cipher);
    } else if (EVP_PKEY_CTX_IS_GEN_OP(ctx)) {
        if (ctx->keymgmt != NULL)
            return EVP_KEYMGMT_get0_provider(ctx->keymgmt);
    }

    return NULL;
}

/* Utility functions to send a string of hex string to a ctrl */

int EVP_PKEY_CTX_str2ctrl(EVP_PKEY_CTX *ctx, int cmd, const char *str)
{
    size_t len;

    len = strlen(str);
    if (len > INT_MAX)
        return -1;
    return ctx->pmeth->ctrl(ctx, cmd, len, (void *)str);
}

int EVP_PKEY_CTX_hex2ctrl(EVP_PKEY_CTX *ctx, int cmd, const char *hex)
{
    unsigned char *bin;
    long binlen;
    int rv = -1;

    bin = OPENSSL_hexstr2buf(hex, &binlen);
    if (bin == NULL)
        return 0;
    if (binlen <= INT_MAX)
        rv = ctx->pmeth->ctrl(ctx, cmd, binlen, bin);
    OPENSSL_free(bin);
    return rv;
}

/* Pass a message digest to a ctrl */
int EVP_PKEY_CTX_md(EVP_PKEY_CTX *ctx, int optype, int cmd, const char *md)
{
    const EVP_MD *m;

    if (md == NULL || (m = EVP_get_digestbyname(md)) == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_DIGEST);
        return 0;
    }
    return EVP_PKEY_CTX_ctrl(ctx, -1, optype, cmd, 0, (void *)m);
}

int EVP_PKEY_CTX_get_operation(EVP_PKEY_CTX *ctx)
{
    return ctx->operation;
}

void EVP_PKEY_CTX_set0_keygen_info(EVP_PKEY_CTX *ctx, int *dat, int datlen)
{
    ctx->keygen_info = dat;
    ctx->keygen_info_count = datlen;
}

void EVP_PKEY_CTX_set_data(EVP_PKEY_CTX *ctx, void *data)
{
    ctx->data = data;
}

void *EVP_PKEY_CTX_get_data(const EVP_PKEY_CTX *ctx)
{
    return ctx->data;
}

EVP_PKEY *EVP_PKEY_CTX_get0_pkey(EVP_PKEY_CTX *ctx)
{
    return ctx->pkey;
}

EVP_PKEY *EVP_PKEY_CTX_get0_peerkey(EVP_PKEY_CTX *ctx)
{
    return ctx->peerkey;
}

void EVP_PKEY_CTX_set_app_data(EVP_PKEY_CTX *ctx, void *data)
{
    ctx->app_data = data;
}

void *EVP_PKEY_CTX_get_app_data(EVP_PKEY_CTX *ctx)
{
    return ctx->app_data;
}

void EVP_PKEY_meth_set_init(EVP_PKEY_METHOD *pmeth,
                            int (*init) (EVP_PKEY_CTX *ctx))
{
    pmeth->init = init;
}

void EVP_PKEY_meth_set_copy(EVP_PKEY_METHOD *pmeth,
                            int (*copy) (EVP_PKEY_CTX *dst,
                                         const EVP_PKEY_CTX *src))
{
    pmeth->copy = copy;
}

void EVP_PKEY_meth_set_cleanup(EVP_PKEY_METHOD *pmeth,
                               void (*cleanup) (EVP_PKEY_CTX *ctx))
{
    pmeth->cleanup = cleanup;
}

void EVP_PKEY_meth_set_paramgen(EVP_PKEY_METHOD *pmeth,
                                int (*paramgen_init) (EVP_PKEY_CTX *ctx),
                                int (*paramgen) (EVP_PKEY_CTX *ctx,
                                                 EVP_PKEY *pkey))
{
    pmeth->paramgen_init = paramgen_init;
    pmeth->paramgen = paramgen;
}

void EVP_PKEY_meth_set_keygen(EVP_PKEY_METHOD *pmeth,
                              int (*keygen_init) (EVP_PKEY_CTX *ctx),
                              int (*keygen) (EVP_PKEY_CTX *ctx,
                                             EVP_PKEY *pkey))
{
    pmeth->keygen_init = keygen_init;
    pmeth->keygen = keygen;
}

void EVP_PKEY_meth_set_sign(EVP_PKEY_METHOD *pmeth,
                            int (*sign_init) (EVP_PKEY_CTX *ctx),
                            int (*sign) (EVP_PKEY_CTX *ctx,
                                         unsigned char *sig, size_t *siglen,
                                         const unsigned char *tbs,
                                         size_t tbslen))
{
    pmeth->sign_init = sign_init;
    pmeth->sign = sign;
}

void EVP_PKEY_meth_set_verify(EVP_PKEY_METHOD *pmeth,
                              int (*verify_init) (EVP_PKEY_CTX *ctx),
                              int (*verify) (EVP_PKEY_CTX *ctx,
                                             const unsigned char *sig,
                                             size_t siglen,
                                             const unsigned char *tbs,
                                             size_t tbslen))
{
    pmeth->verify_init = verify_init;
    pmeth->verify = verify;
}

void EVP_PKEY_meth_set_verify_recover(EVP_PKEY_METHOD *pmeth,
                                      int (*verify_recover_init) (EVP_PKEY_CTX
                                                                  *ctx),
                                      int (*verify_recover) (EVP_PKEY_CTX
                                                             *ctx,
                                                             unsigned char
                                                             *sig,
                                                             size_t *siglen,
                                                             const unsigned
                                                             char *tbs,
                                                             size_t tbslen))
{
    pmeth->verify_recover_init = verify_recover_init;
    pmeth->verify_recover = verify_recover;
}

void EVP_PKEY_meth_set_signctx(EVP_PKEY_METHOD *pmeth,
                               int (*signctx_init) (EVP_PKEY_CTX *ctx,
                                                    EVP_MD_CTX *mctx),
                               int (*signctx) (EVP_PKEY_CTX *ctx,
                                               unsigned char *sig,
                                               size_t *siglen,
                                               EVP_MD_CTX *mctx))
{
    pmeth->signctx_init = signctx_init;
    pmeth->signctx = signctx;
}

void EVP_PKEY_meth_set_verifyctx(EVP_PKEY_METHOD *pmeth,
                                 int (*verifyctx_init) (EVP_PKEY_CTX *ctx,
                                                        EVP_MD_CTX *mctx),
                                 int (*verifyctx) (EVP_PKEY_CTX *ctx,
                                                   const unsigned char *sig,
                                                   int siglen,
                                                   EVP_MD_CTX *mctx))
{
    pmeth->verifyctx_init = verifyctx_init;
    pmeth->verifyctx = verifyctx;
}

void EVP_PKEY_meth_set_encrypt(EVP_PKEY_METHOD *pmeth,
                               int (*encrypt_init) (EVP_PKEY_CTX *ctx),
                               int (*encryptfn) (EVP_PKEY_CTX *ctx,
                                                 unsigned char *out,
                                                 size_t *outlen,
                                                 const unsigned char *in,
                                                 size_t inlen))
{
    pmeth->encrypt_init = encrypt_init;
    pmeth->encrypt = encryptfn;
}

void EVP_PKEY_meth_set_decrypt(EVP_PKEY_METHOD *pmeth,
                               int (*decrypt_init) (EVP_PKEY_CTX *ctx),
                               int (*decrypt) (EVP_PKEY_CTX *ctx,
                                               unsigned char *out,
                                               size_t *outlen,
                                               const unsigned char *in,
                                               size_t inlen))
{
    pmeth->decrypt_init = decrypt_init;
    pmeth->decrypt = decrypt;
}

void EVP_PKEY_meth_set_derive(EVP_PKEY_METHOD *pmeth,
                              int (*derive_init) (EVP_PKEY_CTX *ctx),
                              int (*derive) (EVP_PKEY_CTX *ctx,
                                             unsigned char *key,
                                             size_t *keylen))
{
    pmeth->derive_init = derive_init;
    pmeth->derive = derive;
}

void EVP_PKEY_meth_set_ctrl(EVP_PKEY_METHOD *pmeth,
                            int (*ctrl) (EVP_PKEY_CTX *ctx, int type, int p1,
                                         void *p2),
                            int (*ctrl_str) (EVP_PKEY_CTX *ctx,
                                             const char *type,
                                             const char *value))
{
    pmeth->ctrl = ctrl;
    pmeth->ctrl_str = ctrl_str;
}

void EVP_PKEY_meth_set_digestsign(EVP_PKEY_METHOD *pmeth,
    int (*digestsign) (EVP_MD_CTX *ctx, unsigned char *sig, size_t *siglen,
                       const unsigned char *tbs, size_t tbslen))
{
    pmeth->digestsign = digestsign;
}

void EVP_PKEY_meth_set_digestverify(EVP_PKEY_METHOD *pmeth,
    int (*digestverify) (EVP_MD_CTX *ctx, const unsigned char *sig,
                         size_t siglen, const unsigned char *tbs,
                         size_t tbslen))
{
    pmeth->digestverify = digestverify;
}

void EVP_PKEY_meth_set_check(EVP_PKEY_METHOD *pmeth,
                             int (*check) (EVP_PKEY *pkey))
{
    pmeth->check = check;
}

void EVP_PKEY_meth_set_public_check(EVP_PKEY_METHOD *pmeth,
                                    int (*check) (EVP_PKEY *pkey))
{
    pmeth->public_check = check;
}

void EVP_PKEY_meth_set_param_check(EVP_PKEY_METHOD *pmeth,
                                   int (*check) (EVP_PKEY *pkey))
{
    pmeth->param_check = check;
}

void EVP_PKEY_meth_set_digest_custom(EVP_PKEY_METHOD *pmeth,
                                     int (*digest_custom) (EVP_PKEY_CTX *ctx,
                                                           EVP_MD_CTX *mctx))
{
    pmeth->digest_custom = digest_custom;
}

void EVP_PKEY_meth_get_init(const EVP_PKEY_METHOD *pmeth,
                            int (**pinit) (EVP_PKEY_CTX *ctx))
{
    *pinit = pmeth->init;
}

void EVP_PKEY_meth_get_copy(const EVP_PKEY_METHOD *pmeth,
                            int (**pcopy) (EVP_PKEY_CTX *dst,
                                           const EVP_PKEY_CTX *src))
{
    *pcopy = pmeth->copy;
}

void EVP_PKEY_meth_get_cleanup(const EVP_PKEY_METHOD *pmeth,
                               void (**pcleanup) (EVP_PKEY_CTX *ctx))
{
    *pcleanup = pmeth->cleanup;
}

void EVP_PKEY_meth_get_paramgen(const EVP_PKEY_METHOD *pmeth,
                                int (**pparamgen_init) (EVP_PKEY_CTX *ctx),
                                int (**pparamgen) (EVP_PKEY_CTX *ctx,
                                                   EVP_PKEY *pkey))
{
    if (pparamgen_init)
        *pparamgen_init = pmeth->paramgen_init;
    if (pparamgen)
        *pparamgen = pmeth->paramgen;
}

void EVP_PKEY_meth_get_keygen(const EVP_PKEY_METHOD *pmeth,
                              int (**pkeygen_init) (EVP_PKEY_CTX *ctx),
                              int (**pkeygen) (EVP_PKEY_CTX *ctx,
                                               EVP_PKEY *pkey))
{
    if (pkeygen_init)
        *pkeygen_init = pmeth->keygen_init;
    if (pkeygen)
        *pkeygen = pmeth->keygen;
}

void EVP_PKEY_meth_get_sign(const EVP_PKEY_METHOD *pmeth,
                            int (**psign_init) (EVP_PKEY_CTX *ctx),
                            int (**psign) (EVP_PKEY_CTX *ctx,
                                           unsigned char *sig, size_t *siglen,
                                           const unsigned char *tbs,
                                           size_t tbslen))
{
    if (psign_init)
        *psign_init = pmeth->sign_init;
    if (psign)
        *psign = pmeth->sign;
}

void EVP_PKEY_meth_get_verify(const EVP_PKEY_METHOD *pmeth,
                              int (**pverify_init) (EVP_PKEY_CTX *ctx),
                              int (**pverify) (EVP_PKEY_CTX *ctx,
                                               const unsigned char *sig,
                                               size_t siglen,
                                               const unsigned char *tbs,
                                               size_t tbslen))
{
    if (pverify_init)
        *pverify_init = pmeth->verify_init;
    if (pverify)
        *pverify = pmeth->verify;
}

void EVP_PKEY_meth_get_verify_recover(const EVP_PKEY_METHOD *pmeth,
                                      int (**pverify_recover_init) (EVP_PKEY_CTX
                                                                    *ctx),
                                      int (**pverify_recover) (EVP_PKEY_CTX
                                                               *ctx,
                                                               unsigned char
                                                               *sig,
                                                               size_t *siglen,
                                                               const unsigned
                                                               char *tbs,
                                                               size_t tbslen))
{
    if (pverify_recover_init)
        *pverify_recover_init = pmeth->verify_recover_init;
    if (pverify_recover)
        *pverify_recover = pmeth->verify_recover;
}

void EVP_PKEY_meth_get_signctx(const EVP_PKEY_METHOD *pmeth,
                               int (**psignctx_init) (EVP_PKEY_CTX *ctx,
                                                      EVP_MD_CTX *mctx),
                               int (**psignctx) (EVP_PKEY_CTX *ctx,
                                                 unsigned char *sig,
                                                 size_t *siglen,
                                                 EVP_MD_CTX *mctx))
{
    if (psignctx_init)
        *psignctx_init = pmeth->signctx_init;
    if (psignctx)
        *psignctx = pmeth->signctx;
}

void EVP_PKEY_meth_get_verifyctx(const EVP_PKEY_METHOD *pmeth,
                                 int (**pverifyctx_init) (EVP_PKEY_CTX *ctx,
                                                          EVP_MD_CTX *mctx),
                                 int (**pverifyctx) (EVP_PKEY_CTX *ctx,
                                                     const unsigned char *sig,
                                                     int siglen,
                                                     EVP_MD_CTX *mctx))
{
    if (pverifyctx_init)
        *pverifyctx_init = pmeth->verifyctx_init;
    if (pverifyctx)
        *pverifyctx = pmeth->verifyctx;
}

void EVP_PKEY_meth_get_encrypt(const EVP_PKEY_METHOD *pmeth,
                               int (**pencrypt_init) (EVP_PKEY_CTX *ctx),
                               int (**pencryptfn) (EVP_PKEY_CTX *ctx,
                                                   unsigned char *out,
                                                   size_t *outlen,
                                                   const unsigned char *in,
                                                   size_t inlen))
{
    if (pencrypt_init)
        *pencrypt_init = pmeth->encrypt_init;
    if (pencryptfn)
        *pencryptfn = pmeth->encrypt;
}

void EVP_PKEY_meth_get_decrypt(const EVP_PKEY_METHOD *pmeth,
                               int (**pdecrypt_init) (EVP_PKEY_CTX *ctx),
                               int (**pdecrypt) (EVP_PKEY_CTX *ctx,
                                                 unsigned char *out,
                                                 size_t *outlen,
                                                 const unsigned char *in,
                                                 size_t inlen))
{
    if (pdecrypt_init)
        *pdecrypt_init = pmeth->decrypt_init;
    if (pdecrypt)
        *pdecrypt = pmeth->decrypt;
}

void EVP_PKEY_meth_get_derive(const EVP_PKEY_METHOD *pmeth,
                              int (**pderive_init) (EVP_PKEY_CTX *ctx),
                              int (**pderive) (EVP_PKEY_CTX *ctx,
                                               unsigned char *key,
                                               size_t *keylen))
{
    if (pderive_init)
        *pderive_init = pmeth->derive_init;
    if (pderive)
        *pderive = pmeth->derive;
}

void EVP_PKEY_meth_get_ctrl(const EVP_PKEY_METHOD *pmeth,
                            int (**pctrl) (EVP_PKEY_CTX *ctx, int type, int p1,
                                           void *p2),
                            int (**pctrl_str) (EVP_PKEY_CTX *ctx,
                                               const char *type,
                                               const char *value))
{
    if (pctrl)
        *pctrl = pmeth->ctrl;
    if (pctrl_str)
        *pctrl_str = pmeth->ctrl_str;
}

void EVP_PKEY_meth_get_digestsign(const EVP_PKEY_METHOD *pmeth,
    int (**digestsign) (EVP_MD_CTX *ctx, unsigned char *sig, size_t *siglen,
                        const unsigned char *tbs, size_t tbslen))
{
    if (digestsign)
        *digestsign = pmeth->digestsign;
}

void EVP_PKEY_meth_get_digestverify(const EVP_PKEY_METHOD *pmeth,
    int (**digestverify) (EVP_MD_CTX *ctx, const unsigned char *sig,
                          size_t siglen, const unsigned char *tbs,
                          size_t tbslen))
{
    if (digestverify)
        *digestverify = pmeth->digestverify;
}

void EVP_PKEY_meth_get_check(const EVP_PKEY_METHOD *pmeth,
                             int (**pcheck) (EVP_PKEY *pkey))
{
    if (pcheck != NULL)
        *pcheck = pmeth->check;
}

void EVP_PKEY_meth_get_public_check(const EVP_PKEY_METHOD *pmeth,
                                    int (**pcheck) (EVP_PKEY *pkey))
{
    if (pcheck != NULL)
        *pcheck = pmeth->public_check;
}

void EVP_PKEY_meth_get_param_check(const EVP_PKEY_METHOD *pmeth,
                                   int (**pcheck) (EVP_PKEY *pkey))
{
    if (pcheck != NULL)
        *pcheck = pmeth->param_check;
}

void EVP_PKEY_meth_get_digest_custom(const EVP_PKEY_METHOD *pmeth,
                                     int (**pdigest_custom) (EVP_PKEY_CTX *ctx,
                                                             EVP_MD_CTX *mctx))
{
    if (pdigest_custom != NULL)
        *pdigest_custom = pmeth->digest_custom;
}

#endif /* FIPS_MODULE */
