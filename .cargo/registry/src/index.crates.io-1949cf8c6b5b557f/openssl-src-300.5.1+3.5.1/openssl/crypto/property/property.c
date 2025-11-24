/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2019, Oracle and/or its affiliates.  All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <stdio.h>
#include <stdarg.h>
#include <openssl/crypto.h>
#include "internal/core.h"
#include "internal/property.h"
#include "internal/provider.h"
#include "internal/tsan_assist.h"
#include "crypto/ctype.h"
#include <openssl/lhash.h>
#include <openssl/rand.h>
#include <openssl/trace.h>
#include "internal/thread_once.h"
#include "crypto/lhash.h"
#include "crypto/sparse_array.h"
#include "property_local.h"
#include "crypto/context.h"

/*
 * The number of elements in the query cache before we initiate a flush.
 * If reducing this, also ensure the stochastic test in test/property_test.c
 * isn't likely to fail.
 */
#define IMPL_CACHE_FLUSH_THRESHOLD  500

typedef struct {
    void *method;
    int (*up_ref)(void *);
    void (*free)(void *);
} METHOD;

typedef struct {
    const OSSL_PROVIDER *provider;
    OSSL_PROPERTY_LIST *properties;
    METHOD method;
} IMPLEMENTATION;

DEFINE_STACK_OF(IMPLEMENTATION)

typedef struct {
    const OSSL_PROVIDER *provider;
    const char *query;
    METHOD method;
    char body[1];
} QUERY;

DEFINE_LHASH_OF_EX(QUERY);

typedef struct {
    int nid;
    STACK_OF(IMPLEMENTATION) *impls;
    LHASH_OF(QUERY) *cache;
} ALGORITHM;

struct ossl_method_store_st {
    OSSL_LIB_CTX *ctx;
    SPARSE_ARRAY_OF(ALGORITHM) *algs;
    /*
     * Lock to protect the |algs| array from concurrent writing, when
     * individual implementations or queries are inserted.  This is used
     * by the appropriate functions here.
     */
    CRYPTO_RWLOCK *lock;
    /*
     * Lock to reserve the whole store.  This is used when fetching a set
     * of algorithms, via these functions, found in crypto/core_fetch.c:
     * ossl_method_construct_reserve_store()
     * ossl_method_construct_unreserve_store()
     */
    CRYPTO_RWLOCK *biglock;

    /* query cache specific values */

    /* Count of the query cache entries for all algs */
    size_t cache_nelem;

    /* Flag: 1 if query cache entries for all algs need flushing */
    int cache_need_flush;
};

typedef struct {
    LHASH_OF(QUERY) *cache;
    size_t nelem;
    uint32_t seed;
    unsigned char using_global_seed;
} IMPL_CACHE_FLUSH;

DEFINE_SPARSE_ARRAY_OF(ALGORITHM);

DEFINE_STACK_OF(ALGORITHM)

typedef struct ossl_global_properties_st {
    OSSL_PROPERTY_LIST *list;
#ifndef FIPS_MODULE
    unsigned int no_mirrored : 1;
#endif
} OSSL_GLOBAL_PROPERTIES;

static void ossl_method_cache_flush_alg(OSSL_METHOD_STORE *store,
                                        ALGORITHM *alg);
static void ossl_method_cache_flush(OSSL_METHOD_STORE *store, int nid);

/* Global properties are stored per library context */
void ossl_ctx_global_properties_free(void *vglobp)
{
    OSSL_GLOBAL_PROPERTIES *globp = vglobp;

    if (globp != NULL) {
        ossl_property_free(globp->list);
        OPENSSL_free(globp);
    }
}

void *ossl_ctx_global_properties_new(OSSL_LIB_CTX *ctx)
{
    return OPENSSL_zalloc(sizeof(OSSL_GLOBAL_PROPERTIES));
}

OSSL_PROPERTY_LIST **ossl_ctx_global_properties(OSSL_LIB_CTX *libctx,
                                                ossl_unused int loadconfig)
{
    OSSL_GLOBAL_PROPERTIES *globp;

#if !defined(FIPS_MODULE) && !defined(OPENSSL_NO_AUTOLOAD_CONFIG)
    if (loadconfig && !OPENSSL_init_crypto(OPENSSL_INIT_LOAD_CONFIG, NULL))
        return NULL;
#endif
    globp = ossl_lib_ctx_get_data(libctx, OSSL_LIB_CTX_GLOBAL_PROPERTIES);

    return globp != NULL ? &globp->list : NULL;
}

#ifndef FIPS_MODULE
int ossl_global_properties_no_mirrored(OSSL_LIB_CTX *libctx)
{
    OSSL_GLOBAL_PROPERTIES *globp
        = ossl_lib_ctx_get_data(libctx, OSSL_LIB_CTX_GLOBAL_PROPERTIES);

    return globp != NULL && globp->no_mirrored ? 1 : 0;
}

void ossl_global_properties_stop_mirroring(OSSL_LIB_CTX *libctx)
{
    OSSL_GLOBAL_PROPERTIES *globp
        = ossl_lib_ctx_get_data(libctx, OSSL_LIB_CTX_GLOBAL_PROPERTIES);

    if (globp != NULL)
        globp->no_mirrored = 1;
}
#endif

static int ossl_method_up_ref(METHOD *method)
{
    return (*method->up_ref)(method->method);
}

static void ossl_method_free(METHOD *method)
{
    (*method->free)(method->method);
}

static __owur int ossl_property_read_lock(OSSL_METHOD_STORE *p)
{
    return p != NULL ? CRYPTO_THREAD_read_lock(p->lock) : 0;
}

static __owur int ossl_property_write_lock(OSSL_METHOD_STORE *p)
{
    return p != NULL ? CRYPTO_THREAD_write_lock(p->lock) : 0;
}

static int ossl_property_unlock(OSSL_METHOD_STORE *p)
{
    return p != 0 ? CRYPTO_THREAD_unlock(p->lock) : 0;
}

static unsigned long query_hash(const QUERY *a)
{
    return OPENSSL_LH_strhash(a->query);
}

static int query_cmp(const QUERY *a, const QUERY *b)
{
    int res = strcmp(a->query, b->query);

    if (res == 0 && a->provider != NULL && b->provider != NULL)
        res = b->provider > a->provider ? 1
            : b->provider < a->provider ? -1
            : 0;
    return res;
}

static void impl_free(IMPLEMENTATION *impl)
{
    if (impl != NULL) {
        ossl_method_free(&impl->method);
        OPENSSL_free(impl);
    }
}

static void impl_cache_free(QUERY *elem)
{
    if (elem != NULL) {
        ossl_method_free(&elem->method);
        OPENSSL_free(elem);
    }
}

static void impl_cache_flush_alg(ossl_uintmax_t idx, ALGORITHM *alg)
{
    lh_QUERY_doall(alg->cache, &impl_cache_free);
    lh_QUERY_flush(alg->cache);
}

static void alg_cleanup(ossl_uintmax_t idx, ALGORITHM *a, void *arg)
{
    OSSL_METHOD_STORE *store = arg;

    if (a != NULL) {
        sk_IMPLEMENTATION_pop_free(a->impls, &impl_free);
        lh_QUERY_doall(a->cache, &impl_cache_free);
        lh_QUERY_free(a->cache);
        OPENSSL_free(a);
    }
    if (store != NULL)
        ossl_sa_ALGORITHM_set(store->algs, idx, NULL);
}

/*
 * The OSSL_LIB_CTX param here allows access to underlying property data needed
 * for computation
 */
OSSL_METHOD_STORE *ossl_method_store_new(OSSL_LIB_CTX *ctx)
{
    OSSL_METHOD_STORE *res;

    res = OPENSSL_zalloc(sizeof(*res));
    if (res != NULL) {
        res->ctx = ctx;
        if ((res->algs = ossl_sa_ALGORITHM_new()) == NULL
            || (res->lock = CRYPTO_THREAD_lock_new()) == NULL
            || (res->biglock = CRYPTO_THREAD_lock_new()) == NULL) {
            ossl_method_store_free(res);
            return NULL;
        }
    }
    return res;
}

void ossl_method_store_free(OSSL_METHOD_STORE *store)
{
    if (store != NULL) {
        if (store->algs != NULL)
            ossl_sa_ALGORITHM_doall_arg(store->algs, &alg_cleanup, store);
        ossl_sa_ALGORITHM_free(store->algs);
        CRYPTO_THREAD_lock_free(store->lock);
        CRYPTO_THREAD_lock_free(store->biglock);
        OPENSSL_free(store);
    }
}

int ossl_method_lock_store(OSSL_METHOD_STORE *store)
{
    return store != NULL ? CRYPTO_THREAD_write_lock(store->biglock) : 0;
}

int ossl_method_unlock_store(OSSL_METHOD_STORE *store)
{
    return store != NULL ? CRYPTO_THREAD_unlock(store->biglock) : 0;
}

static ALGORITHM *ossl_method_store_retrieve(OSSL_METHOD_STORE *store, int nid)
{
    return ossl_sa_ALGORITHM_get(store->algs, nid);
}

static int ossl_method_store_insert(OSSL_METHOD_STORE *store, ALGORITHM *alg)
{
    return ossl_sa_ALGORITHM_set(store->algs, alg->nid, alg);
}

/**
 * @brief Adds a method to the specified method store.
 *
 * This function adds a new method to the provided method store, associating it
 * with a specified id, properties, and provider. The method is stored with
 * reference count and destruction callbacks.
 *
 * @param store Pointer to the OSSL_METHOD_STORE where the method will be added.
 *              Must be non-null.
 * @param prov Pointer to the OSSL_PROVIDER for the provider of the method.
 *             Must be non-null.
 * @param nid (identifier) associated with the method, must be > 0
 * @param properties String containing properties of the method.
 * @param method Pointer to the method to be added.
 * @param method_up_ref Function pointer for incrementing the method ref count.
 * @param method_destruct Function pointer for destroying the method.
 *
 * @return 1 if the method is successfully added, 0 on failure.
 *
 * If tracing is enabled, a message is printed indicating that the method is
 * being added to the method store.
 *
 * NOTE: The nid parameter here is _not_ a nid in the sense of the NID_* macros.
 * It is an internal unique identifier.
 */
int ossl_method_store_add(OSSL_METHOD_STORE *store, const OSSL_PROVIDER *prov,
                          int nid, const char *properties, void *method,
                          int (*method_up_ref)(void *),
                          void (*method_destruct)(void *))
{
    ALGORITHM *alg = NULL;
    IMPLEMENTATION *impl;
    int ret = 0;
    int i;

    if (nid <= 0 || method == NULL || store == NULL)
        return 0;

    if (properties == NULL)
        properties = "";

    if (!ossl_assert(prov != NULL))
        return 0;

    /* Create new entry */
    impl = OPENSSL_malloc(sizeof(*impl));
    if (impl == NULL)
        return 0;
    impl->method.method = method;
    impl->method.up_ref = method_up_ref;
    impl->method.free = method_destruct;
    if (!ossl_method_up_ref(&impl->method)) {
        OPENSSL_free(impl);
        return 0;
    }
    impl->provider = prov;

    /* Insert into the hash table if required */
    if (!ossl_property_write_lock(store)) {
        impl_free(impl);
        return 0;
    }

    /*
     * Flush the alg cache of any implementation that already exists
     * for this id.
     * This is done to ensure that on the next lookup we go through the
     * provider comparison in ossl_method_store_fetch.  If we don't do this
     * then this new method won't be given a chance to get selected.
     * NOTE: This doesn't actually remove the method from the backing store
     * It just ensures that we query the backing store when (re)-adding a
     * method to the algorithm cache, in case the one selected by the next
     * query selects a different implementation
     */
    ossl_method_cache_flush(store, nid);

    /*
     * Parse the properties associated with this method, and convert it to a
     * property list stored against the implementation for later comparison
     * during fetch operations
     */
    if ((impl->properties = ossl_prop_defn_get(store->ctx, properties)) == NULL) {
        impl->properties = ossl_parse_property(store->ctx, properties);
        if (impl->properties == NULL)
            goto err;
        if (!ossl_prop_defn_set(store->ctx, properties, &impl->properties)) {
            ossl_property_free(impl->properties);
            impl->properties = NULL;
            goto err;
        }
    }

    /*
     * Check if we have an algorithm cache already for this nid.  If so use
     * it, otherwise, create it, and insert it into the store
     */
    alg = ossl_method_store_retrieve(store, nid);
    if (alg == NULL) {
        if ((alg = OPENSSL_zalloc(sizeof(*alg))) == NULL
                || (alg->impls = sk_IMPLEMENTATION_new_null()) == NULL
                || (alg->cache = lh_QUERY_new(&query_hash, &query_cmp)) == NULL)
            goto err;
        alg->nid = nid;
        if (!ossl_method_store_insert(store, alg))
            goto err;
        OSSL_TRACE2(QUERY, "Inserted an alg with nid %d into the store %p\n", nid, (void *)store);
    }

    /* Push onto stack if there isn't one there already */
    for (i = 0; i < sk_IMPLEMENTATION_num(alg->impls); i++) {
        const IMPLEMENTATION *tmpimpl = sk_IMPLEMENTATION_value(alg->impls, i);

        if (tmpimpl->provider == impl->provider
            && tmpimpl->properties == impl->properties)
            break;
    }

    if (i == sk_IMPLEMENTATION_num(alg->impls)
        && sk_IMPLEMENTATION_push(alg->impls, impl)) {
        ret = 1;
#ifndef FIPS_MODULE
        OSSL_TRACE_BEGIN(QUERY) {
            BIO_printf(trc_out, "Adding to method store "
                       "nid: %d\nproperties: %s\nprovider: %s\n",
                       nid, properties,
                       ossl_provider_name(prov) == NULL ? "none" :
                       ossl_provider_name(prov));
        } OSSL_TRACE_END(QUERY);
#endif
    }
    ossl_property_unlock(store);
    if (ret == 0)
        impl_free(impl);
    return ret;

err:
    ossl_property_unlock(store);
    alg_cleanup(0, alg, NULL);
    impl_free(impl);
    return 0;
}

int ossl_method_store_remove(OSSL_METHOD_STORE *store, int nid,
                             const void *method)
{
    ALGORITHM *alg = NULL;
    int i;

    if (nid <= 0 || method == NULL || store == NULL)
        return 0;

    if (!ossl_property_write_lock(store))
        return 0;
    ossl_method_cache_flush(store, nid);
    alg = ossl_method_store_retrieve(store, nid);
    if (alg == NULL) {
        ossl_property_unlock(store);
        return 0;
    }

    /*
     * A sorting find then a delete could be faster but these stacks should be
     * relatively small, so we avoid the overhead.  Sorting could also surprise
     * users when result orderings change (even though they are not guaranteed).
     */
    for (i = 0; i < sk_IMPLEMENTATION_num(alg->impls); i++) {
        IMPLEMENTATION *impl = sk_IMPLEMENTATION_value(alg->impls, i);

        if (impl->method.method == method) {
            impl_free(impl);
            (void)sk_IMPLEMENTATION_delete(alg->impls, i);
            ossl_property_unlock(store);
            return 1;
        }
    }
    ossl_property_unlock(store);
    return 0;
}

struct alg_cleanup_by_provider_data_st {
    OSSL_METHOD_STORE *store;
    const OSSL_PROVIDER *prov;
};

/**
 * @brief Cleans up implementations of an algorithm associated with a provider.
 *
 * This function removes all implementations of a specified algorithm that are
 * associated with a given provider. The function walks through the stack of
 * implementations backwards to handle deletions without affecting indexing.
 *
 * @param idx Index of the algorithm (unused in this function).
 * @param alg Pointer to the ALGORITHM structure containing the implementations.
 * @param arg Pointer to the data containing the provider information.
 *
 * If tracing is enabled, messages are printed indicating the removal of each
 * implementation and its properties. If any implementation is removed, the
 * associated cache is flushed.
 */
static void
alg_cleanup_by_provider(ossl_uintmax_t idx, ALGORITHM *alg, void *arg)
{
    struct alg_cleanup_by_provider_data_st *data = arg;
    int i, count;

    /*
     * We walk the stack backwards, to avoid having to deal with stack shifts
     * caused by deletion
     */
    for (count = 0, i = sk_IMPLEMENTATION_num(alg->impls); i-- > 0;) {
        IMPLEMENTATION *impl = sk_IMPLEMENTATION_value(alg->impls, i);

        if (impl->provider == data->prov) {
#ifndef FIPS_MODULE
            OSSL_TRACE_BEGIN(QUERY) {
                char buf[512];
                size_t size;

                size = ossl_property_list_to_string(NULL, impl->properties, buf,
                                                    sizeof(buf));
                BIO_printf(trc_out, "Removing implementation from "
                           "query cache\nproperties %s\nprovider %s\n",
                           size == 0 ? "none" : buf,
                           ossl_provider_name(impl->provider) == NULL ? "none" :
                           ossl_provider_name(impl->provider));
            } OSSL_TRACE_END(QUERY);
#endif

            (void)sk_IMPLEMENTATION_delete(alg->impls, i);
            count++;
            impl_free(impl);
        }
    }

    /*
     * If we removed any implementation, we also clear the whole associated
     * cache, 'cause that's the sensible thing to do.
     * There's no point flushing the cache entries where we didn't remove
     * any implementation, though.
     */
    if (count > 0)
        ossl_method_cache_flush_alg(data->store, alg);
}

int ossl_method_store_remove_all_provided(OSSL_METHOD_STORE *store,
                                          const OSSL_PROVIDER *prov)
{
    struct alg_cleanup_by_provider_data_st data;

    if (!ossl_property_write_lock(store))
        return 0;
    data.prov = prov;
    data.store = store;
    ossl_sa_ALGORITHM_doall_arg(store->algs, &alg_cleanup_by_provider, &data);
    ossl_property_unlock(store);
    return 1;
}

static void alg_do_one(ALGORITHM *alg, IMPLEMENTATION *impl,
                       void (*fn)(int id, void *method, void *fnarg),
                       void *fnarg)
{
    fn(alg->nid, impl->method.method, fnarg);
}

static void alg_copy(ossl_uintmax_t idx, ALGORITHM *alg, void *arg)
{
    STACK_OF(ALGORITHM) *newalg = arg;

    (void)sk_ALGORITHM_push(newalg, alg);
}

void ossl_method_store_do_all(OSSL_METHOD_STORE *store,
                              void (*fn)(int id, void *method, void *fnarg),
                              void *fnarg)
{
    int i, j;
    int numalgs, numimps;
    STACK_OF(ALGORITHM) *tmpalgs;
    ALGORITHM *alg;

    if (store != NULL) {

        if (!ossl_property_read_lock(store))
            return;
       
        tmpalgs = sk_ALGORITHM_new_reserve(NULL,
                                           ossl_sa_ALGORITHM_num(store->algs));
        if (tmpalgs == NULL) {
            ossl_property_unlock(store);
            return;
        }

        ossl_sa_ALGORITHM_doall_arg(store->algs, alg_copy, tmpalgs);
        ossl_property_unlock(store);
        numalgs = sk_ALGORITHM_num(tmpalgs);
        for (i = 0; i < numalgs; i++) {
            alg = sk_ALGORITHM_value(tmpalgs, i);
            numimps = sk_IMPLEMENTATION_num(alg->impls);
            for (j = 0; j < numimps; j++)
                alg_do_one(alg, sk_IMPLEMENTATION_value(alg->impls, j), fn, fnarg);
        }
        sk_ALGORITHM_free(tmpalgs);
    }
}

/**
 * @brief Fetches a method from the method store matching the given properties.
 *
 * This function searches the method store for an implementation of a specified
 * method, identified by its id (nid), and matching the given property query. If
 * successful, it returns the method and its associated provider.
 *
 * @param store Pointer to the OSSL_METHOD_STORE from which to fetch the method.
 *              Must be non-null.
 * @param nid (identifier) of the method to be fetched. Must be > 0
 * @param prop_query String containing the property query to match against.
 * @param prov_rw Pointer to the OSSL_PROVIDER to restrict the search to, or
 *                to receive the matched provider.
 * @param method Pointer to receive the fetched method. Must be non-null.
 *
 * @return 1 if the method is successfully fetched, 0 on failure.
 *
 * If tracing is enabled, a message is printed indicating the property query and
 * the resolved provider.
 *
 * NOTE: The nid parameter here is _not_ a NID in the sense of the NID_* macros.
 * It is a unique internal identifier value.
 */
int ossl_method_store_fetch(OSSL_METHOD_STORE *store,
                            int nid, const char *prop_query,
                            const OSSL_PROVIDER **prov_rw, void **method)
{
    OSSL_PROPERTY_LIST **plp;
    ALGORITHM *alg;
    IMPLEMENTATION *impl, *best_impl = NULL;
    OSSL_PROPERTY_LIST *pq = NULL, *p2 = NULL;
    const OSSL_PROVIDER *prov = prov_rw != NULL ? *prov_rw : NULL;
    int ret = 0;
    int j, best = -1, score, optional;

    if (nid <= 0 || method == NULL || store == NULL)
        return 0;

#if !defined(FIPS_MODULE) && !defined(OPENSSL_NO_AUTOLOAD_CONFIG)
    if (ossl_lib_ctx_is_default(store->ctx)
            && !OPENSSL_init_crypto(OPENSSL_INIT_LOAD_CONFIG, NULL))
        return 0;
#endif

    /* This only needs to be a read lock, because the query won't create anything */
    if (!ossl_property_read_lock(store))
        return 0;

    OSSL_TRACE2(QUERY, "Retrieving by nid %d from store %p\n", nid, (void *)store);
    alg = ossl_method_store_retrieve(store, nid);
    if (alg == NULL) {
        ossl_property_unlock(store);
        OSSL_TRACE2(QUERY, "Failed to retrieve by nid %d from store %p\n", nid, (void *)store);
        return 0;
    }
    OSSL_TRACE2(QUERY, "Retrieved by nid %d from store %p\n", nid, (void *)store);

    /*
     * If a property query string is provided, convert it to an
     * OSSL_PROPERTY_LIST structure
     */
    if (prop_query != NULL)
        p2 = pq = ossl_parse_query(store->ctx, prop_query, 0);

    /*
     * If the library context has default properties specified
     * then merge those with the properties passed to this function
     */
    plp = ossl_ctx_global_properties(store->ctx, 0);
    if (plp != NULL && *plp != NULL) {
        if (pq == NULL) {
            pq = *plp;
        } else {
            p2 = ossl_property_merge(pq, *plp);
            ossl_property_free(pq);
            if (p2 == NULL)
                goto fin;
            pq = p2;
        }
    }

    /*
     * Search for a provider that provides this implementation.
     * If the requested provider is NULL, then any provider will do,
     * otherwise we should try to find the one that matches the requested
     * provider.  Note that providers are given implicit preference via the
     * ordering of the implementation stack
     */
    if (pq == NULL) {
        for (j = 0; j < sk_IMPLEMENTATION_num(alg->impls); j++) {
            if ((impl = sk_IMPLEMENTATION_value(alg->impls, j)) != NULL
                && (prov == NULL || impl->provider == prov)) {
                best_impl = impl;
                ret = 1;
                break;
            }
        }
        goto fin;
    }

    /*
     * If there are optional properties specified
     * then run the search again, and select the provider that matches the
     * most options
     */
    optional = ossl_property_has_optional(pq);
    for (j = 0; j < sk_IMPLEMENTATION_num(alg->impls); j++) {
        if ((impl = sk_IMPLEMENTATION_value(alg->impls, j)) != NULL
            && (prov == NULL || impl->provider == prov)) {
            score = ossl_property_match_count(pq, impl->properties);
            if (score > best) {
                best_impl = impl;
                best = score;
                ret = 1;
                if (!optional)
                    goto fin;
            }
        }
    }
fin:
    if (ret && ossl_method_up_ref(&best_impl->method)) {
        *method = best_impl->method.method;
        if (prov_rw != NULL)
            *prov_rw = best_impl->provider;
    } else {
        ret = 0;
    }

#ifndef FIPS_MODULE
    OSSL_TRACE_BEGIN(QUERY) {
        char buf[512];
        int size;

        size = ossl_property_list_to_string(NULL, pq, buf, 512);
        BIO_printf(trc_out, "method store query with properties %s "
                   "resolves to provider %s\n",
                   size == 0 ? "none" : buf,
                   best_impl == NULL ? "none" :
                   ossl_provider_name(best_impl->provider));
    } OSSL_TRACE_END(QUERY);
#endif

    ossl_property_unlock(store);
    ossl_property_free(p2);
    return ret;
}

static void ossl_method_cache_flush_alg(OSSL_METHOD_STORE *store,
                                        ALGORITHM *alg)
{
    store->cache_nelem -= lh_QUERY_num_items(alg->cache);
    impl_cache_flush_alg(0, alg);
}

static void ossl_method_cache_flush(OSSL_METHOD_STORE *store, int nid)
{
    ALGORITHM *alg = ossl_method_store_retrieve(store, nid);

    if (alg != NULL)
        ossl_method_cache_flush_alg(store, alg);
}

int ossl_method_store_cache_flush_all(OSSL_METHOD_STORE *store)
{
    if (!ossl_property_write_lock(store))
        return 0;
    ossl_sa_ALGORITHM_doall(store->algs, &impl_cache_flush_alg);
    store->cache_nelem = 0;
    ossl_property_unlock(store);
    return 1;
}

IMPLEMENT_LHASH_DOALL_ARG(QUERY, IMPL_CACHE_FLUSH);

/*
 * Flush an element from the query cache (perhaps).
 *
 * In order to avoid taking a write lock or using atomic operations
 * to keep accurate least recently used (LRU) or least frequently used
 * (LFU) information, the procedure used here is to stochastically
 * flush approximately half the cache.
 *
 * This procedure isn't ideal, LRU or LFU would be better.  However,
 * in normal operation, reaching a full cache would be unexpected.
 * It means that no steady state of algorithm queries has been reached.
 * That is, it is most likely an attack of some form.  A suboptimal clearance
 * strategy that doesn't degrade performance of the normal case is
 * preferable to a more refined approach that imposes a performance
 * impact.
 */
static void impl_cache_flush_cache(QUERY *c, IMPL_CACHE_FLUSH *state)
{
    uint32_t n;

    /*
     * Implement the 32 bit xorshift as suggested by George Marsaglia in:
     *      https://doi.org/10.18637/jss.v008.i14
     *
     * This is a very fast PRNG so there is no need to extract bits one at a
     * time and use the entire value each time.
     */
    n = state->seed;
    n ^= n << 13;
    n ^= n >> 17;
    n ^= n << 5;
    state->seed = n;

    if ((n & 1) != 0)
        impl_cache_free(lh_QUERY_delete(state->cache, c));
    else
        state->nelem++;
}

static void impl_cache_flush_one_alg(ossl_uintmax_t idx, ALGORITHM *alg,
                                     void *v)
{
    IMPL_CACHE_FLUSH *state = (IMPL_CACHE_FLUSH *)v;
    unsigned long orig_down_load = lh_QUERY_get_down_load(alg->cache);

    state->cache = alg->cache;
    lh_QUERY_set_down_load(alg->cache, 0);
    lh_QUERY_doall_IMPL_CACHE_FLUSH(state->cache, &impl_cache_flush_cache,
                                    state);
    lh_QUERY_set_down_load(alg->cache, orig_down_load);
}

static void ossl_method_cache_flush_some(OSSL_METHOD_STORE *store)
{
    IMPL_CACHE_FLUSH state;
    static TSAN_QUALIFIER uint32_t global_seed = 1;

    state.nelem = 0;
    state.using_global_seed = 0;
    if ((state.seed = OPENSSL_rdtsc()) == 0) {
        /* If there is no timer available, seed another way */
        state.using_global_seed = 1;
        state.seed = tsan_load(&global_seed);
    }
    store->cache_need_flush = 0;
    ossl_sa_ALGORITHM_doall_arg(store->algs, &impl_cache_flush_one_alg, &state);
    store->cache_nelem = state.nelem;
    /* Without a timer, update the global seed */
    if (state.using_global_seed)
        tsan_add(&global_seed, state.seed);
}

int ossl_method_store_cache_get(OSSL_METHOD_STORE *store, OSSL_PROVIDER *prov,
                                int nid, const char *prop_query, void **method)
{
    ALGORITHM *alg;
    QUERY elem, *r;
    int res = 0;

    if (nid <= 0 || store == NULL || prop_query == NULL)
        return 0;

    if (!ossl_property_read_lock(store))
        return 0;
    alg = ossl_method_store_retrieve(store, nid);
    if (alg == NULL)
        goto err;

    elem.query = prop_query;
    elem.provider = prov;
    r = lh_QUERY_retrieve(alg->cache, &elem);
    if (r == NULL)
        goto err;
    if (ossl_method_up_ref(&r->method)) {
        *method = r->method.method;
        res = 1;
    }
err:
    ossl_property_unlock(store);
    return res;
}

int ossl_method_store_cache_set(OSSL_METHOD_STORE *store, OSSL_PROVIDER *prov,
                                int nid, const char *prop_query, void *method,
                                int (*method_up_ref)(void *),
                                void (*method_destruct)(void *))
{
    QUERY elem, *old, *p = NULL;
    ALGORITHM *alg;
    size_t len;
    int res = 1;

    if (nid <= 0 || store == NULL || prop_query == NULL)
        return 0;

    if (!ossl_assert(prov != NULL))
        return 0;

    if (!ossl_property_write_lock(store))
        return 0;
    if (store->cache_need_flush)
        ossl_method_cache_flush_some(store);
    alg = ossl_method_store_retrieve(store, nid);
    if (alg == NULL)
        goto err;

    if (method == NULL) {
        elem.query = prop_query;
        elem.provider = prov;
        if ((old = lh_QUERY_delete(alg->cache, &elem)) != NULL) {
            impl_cache_free(old);
            store->cache_nelem--;
        }
        goto end;
    }
    p = OPENSSL_malloc(sizeof(*p) + (len = strlen(prop_query)));
    if (p != NULL) {
        p->query = p->body;
        p->provider = prov;
        p->method.method = method;
        p->method.up_ref = method_up_ref;
        p->method.free = method_destruct;
        if (!ossl_method_up_ref(&p->method))
            goto err;
        memcpy((char *)p->query, prop_query, len + 1);
        if ((old = lh_QUERY_insert(alg->cache, p)) != NULL) {
            impl_cache_free(old);
            goto end;
        }
        if (!lh_QUERY_error(alg->cache)) {
            if (++store->cache_nelem >= IMPL_CACHE_FLUSH_THRESHOLD)
                store->cache_need_flush = 1;
            goto end;
        }
        ossl_method_free(&p->method);
    }
err:
    res = 0;
    OPENSSL_free(p);
end:
    ossl_property_unlock(store);
    return res;
}
