/*
 * Copyright 2018-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/safestack.h>
#include <openssl/store.h>
#include "internal/cryptlib.h"
#include "crypto/x509.h"
#include "x509_local.h"

typedef struct cached_store_st {
    char *uri;
    OSSL_LIB_CTX *libctx;
    char *propq;
    OSSL_STORE_CTX *ctx;
} CACHED_STORE;

DEFINE_STACK_OF(CACHED_STORE)

/* Generic object loader, given expected type and criterion */
static int cache_objects(X509_LOOKUP *lctx, CACHED_STORE *store,
                         const OSSL_STORE_SEARCH *criterion, int depth)
{
    int ok = 0;
    OSSL_STORE_CTX *ctx = store->ctx;
    X509_STORE *xstore = X509_LOOKUP_get_store(lctx);

    if (ctx == NULL
        && (ctx = OSSL_STORE_open_ex(store->uri, store->libctx, store->propq,
                                     NULL, NULL, NULL, NULL, NULL)) == NULL)
        return 0;
    store->ctx = ctx;

    /*
     * We try to set the criterion, but don't care if it was valid or not.
     * For an OSSL_STORE, it merely serves as an optimization, the expectation
     * being that if the criterion couldn't be used, we will get *everything*
     * from the container that the URI represents rather than the subset that
     * the criterion indicates, so the biggest harm is that we cache more
     * objects certs and CRLs than we may expect, but that's ok.
     *
     * Specifically for OpenSSL's own file: scheme, the only workable
     * criterion is the BY_NAME one, which it can only apply on directories,
     * but it's possible that the URI is a single file rather than a directory,
     * and in that case, the BY_NAME criterion is pointless.
     *
     * We could very simply not apply any criterion at all here, and just let
     * the code that selects certs and CRLs from the cached objects do its job,
     * but it's a nice optimization when it can be applied (such as on an
     * actual directory with a thousand CA certs).
     */
    if (criterion != NULL)
        OSSL_STORE_find(ctx, criterion);

    for (;;) {
        OSSL_STORE_INFO *info = OSSL_STORE_load(ctx);
        int infotype;

        /* NULL means error or "end of file".  Either way, we break. */
        if (info == NULL)
            break;

        infotype = OSSL_STORE_INFO_get_type(info);
        ok = 0;

        if (infotype == OSSL_STORE_INFO_NAME) {
            /*
             * This is an entry in the "directory" represented by the current
             * uri.  if |depth| allows, dive into it.
             */
            if (depth > 0) {
                CACHED_STORE substore;

                substore.uri = (char *)OSSL_STORE_INFO_get0_NAME(info);
                substore.libctx = store->libctx;
                substore.propq = store->propq;
                substore.ctx = NULL;
                ok = cache_objects(lctx, &substore, criterion, depth - 1);
            }
        } else {
            /*
             * We know that X509_STORE_add_{cert|crl} increments the object's
             * refcount, so we can safely use OSSL_STORE_INFO_get0_{cert,crl}
             * to get them.
             */
            switch (infotype) {
            case OSSL_STORE_INFO_CERT:
                ok = X509_STORE_add_cert(xstore,
                                         OSSL_STORE_INFO_get0_CERT(info));
                break;
            case OSSL_STORE_INFO_CRL:
                ok = X509_STORE_add_crl(xstore,
                                        OSSL_STORE_INFO_get0_CRL(info));
                break;
            }
        }

        OSSL_STORE_INFO_free(info);
        if (!ok)
            break;
    }
    OSSL_STORE_close(ctx);
    store->ctx = NULL;

    return ok;
}


static void free_store(CACHED_STORE *store)
{
    if (store != NULL) {
        OSSL_STORE_close(store->ctx);
        OPENSSL_free(store->uri);
        OPENSSL_free(store->propq);
        OPENSSL_free(store);
    }
}

static void by_store_free(X509_LOOKUP *ctx)
{
    STACK_OF(CACHED_STORE) *stores = X509_LOOKUP_get_method_data(ctx);
    sk_CACHED_STORE_pop_free(stores, free_store);
}

static int by_store_ctrl_ex(X509_LOOKUP *ctx, int cmd, const char *argp,
                            long argl, char **retp, OSSL_LIB_CTX *libctx,
                            const char *propq)
{
    switch (cmd) {
    case X509_L_ADD_STORE:
        if (argp != NULL) {
            STACK_OF(CACHED_STORE) *stores = X509_LOOKUP_get_method_data(ctx);
            CACHED_STORE *store = OPENSSL_zalloc(sizeof(*store));

            if (store == NULL) {
                return 0;
            }

            store->uri = OPENSSL_strdup(argp);
            store->libctx = libctx;
            if (propq != NULL)
                store->propq = OPENSSL_strdup(propq);
            store->ctx = OSSL_STORE_open_ex(argp, libctx, propq, NULL, NULL,
                                           NULL, NULL, NULL);
            if (store->ctx == NULL
                || (propq != NULL && store->propq == NULL)
                || store->uri == NULL) {
                free_store(store);
                return 0;
            }

            if (stores == NULL) {
                stores = sk_CACHED_STORE_new_null();
                if (stores != NULL)
                    X509_LOOKUP_set_method_data(ctx, stores);
            }
            if (stores == NULL || sk_CACHED_STORE_push(stores, store) <= 0) {
                free_store(store);
                return 0;
            }
            return 1;
        }
        /* NOP if no URI is given. */
        return 1;
    case X509_L_LOAD_STORE: {
        /* This is a shortcut for quick loading of specific containers */
        CACHED_STORE store;

        store.uri = (char *)argp;
        store.libctx = libctx;
        store.propq = (char *)propq;
        store.ctx = NULL;
        return cache_objects(ctx, &store, NULL, 0);
    }
    default:
        /* Unsupported command */
        return 0;
    }
}

static int by_store_ctrl(X509_LOOKUP *ctx, int cmd,
                         const char *argp, long argl, char **retp)
{
    return by_store_ctrl_ex(ctx, cmd, argp, argl, retp, NULL, NULL);
}

static int by_store(X509_LOOKUP *ctx, X509_LOOKUP_TYPE type,
                    const OSSL_STORE_SEARCH *criterion, X509_OBJECT *ret)
{
    STACK_OF(CACHED_STORE) *stores = X509_LOOKUP_get_method_data(ctx);
    int i;
    int ok = 0;

    for (i = 0; i < sk_CACHED_STORE_num(stores); i++) {
        ok = cache_objects(ctx, sk_CACHED_STORE_value(stores, i), criterion,
                           1 /* depth */);

        if (ok)
            break;
    }
    return ok;
}

static int by_store_subject(X509_LOOKUP *ctx, X509_LOOKUP_TYPE type,
                            const X509_NAME *name, X509_OBJECT *ret)
{
    OSSL_STORE_SEARCH *criterion =
        OSSL_STORE_SEARCH_by_name((X509_NAME *)name); /* won't modify it */
    int ok = by_store(ctx, type, criterion, ret);
    STACK_OF(X509_OBJECT) *store_objects =
        X509_STORE_get0_objects(X509_LOOKUP_get_store(ctx));
    X509_OBJECT *tmp = NULL;

    OSSL_STORE_SEARCH_free(criterion);

    if (ok)
        tmp = X509_OBJECT_retrieve_by_subject(store_objects, type, name);

    ok = 0;
    if (tmp != NULL) {
        /*
         * This could also be done like this:
         *
         *     if (tmp != NULL) {
         *         *ret = *tmp;
         *         ok = 1;
         *     }
         *
         * However, we want to exercise the documented API to the max, so
         * we do it the hard way.
         *
         * To be noted is that X509_OBJECT_set1_* increment the refcount,
         * but so does X509_STORE_CTX_get_by_subject upon return of this
         * function, so we must ensure the refcount is decremented
         * before we return, or we will get a refcount leak.  We cannot do
         * this with X509_OBJECT_free(), though, as that will free a bit
         * too much.
         */
        switch (type) {
        case X509_LU_X509:
            ok = X509_OBJECT_set1_X509(ret, tmp->data.x509);
            if (ok)
                X509_free(tmp->data.x509);
            break;
        case X509_LU_CRL:
            ok = X509_OBJECT_set1_X509_CRL(ret, tmp->data.crl);
            if (ok)
                X509_CRL_free(tmp->data.crl);
            break;
        case X509_LU_NONE:
            break;
        }
    }
    return ok;
}

/*
 * We lack the implementations for get_by_issuer_serial, get_by_fingerprint
 * and get_by_alias.  There's simply not enough support in the X509_LOOKUP
 * or X509_STORE APIs.
 */

static X509_LOOKUP_METHOD x509_store_lookup = {
    "Load certs from STORE URIs",
    NULL,                        /* new_item */
    by_store_free,               /* free */
    NULL,                        /* init */
    NULL,                        /* shutdown */
    by_store_ctrl,               /* ctrl */
    by_store_subject,            /* get_by_subject */
    NULL,                        /* get_by_issuer_serial */
    NULL,                        /* get_by_fingerprint */
    NULL,                        /* get_by_alias */
    NULL,                        /* get_by_subject_ex */
    by_store_ctrl_ex
};

X509_LOOKUP_METHOD *X509_LOOKUP_store(void)
{
    return &x509_store_lookup;
}
