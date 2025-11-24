/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * A filtering provider for test purposes. We pass all calls through to the
 * default provider except where we want other behaviour for a test.
 */

#include <string.h>
#include <openssl/core.h>
#include <openssl/core_dispatch.h>
#include <openssl/provider.h>
#include <openssl/crypto.h>
#include "testutil.h"
#include "filterprov.h"
#include "prov/bio.h"

#define MAX_FILTERS     10
#define MAX_ALG_FILTERS 5

struct filter_prov_globals_st {
    OSSL_LIB_CTX *libctx;
    OSSL_PROVIDER *deflt;
    struct {
        int operation;
        OSSL_ALGORITHM alg[MAX_ALG_FILTERS + 1];
    } dispatch[MAX_FILTERS];
    int num_dispatch;
    int no_cache;
    unsigned long int query_count;
    int error;
};

static struct filter_prov_globals_st ourglobals;

static struct filter_prov_globals_st *get_globals(void)
{
    /*
     * Ideally we'd like to store this in the OSSL_LIB_CTX so that we can have
     * more than one instance of the filter provider at a time. But for now we
     * just make it simple.
     */
    return &ourglobals;
}

static OSSL_FUNC_provider_gettable_params_fn filter_gettable_params;
static OSSL_FUNC_provider_get_params_fn filter_get_params;
static OSSL_FUNC_provider_query_operation_fn filter_query;
static OSSL_FUNC_provider_unquery_operation_fn filter_unquery;
static OSSL_FUNC_provider_teardown_fn filter_teardown;

static const OSSL_PARAM *filter_gettable_params(void *provctx)
{
    struct filter_prov_globals_st *globs = get_globals();

    return OSSL_PROVIDER_gettable_params(globs->deflt);
}

static int filter_get_params(void *provctx, OSSL_PARAM params[])
{
    struct filter_prov_globals_st *globs = get_globals();

    return OSSL_PROVIDER_get_params(globs->deflt, params);
}

static int filter_get_capabilities(void *provctx, const char *capability,
                                   OSSL_CALLBACK *cb, void *arg)
{
    struct filter_prov_globals_st *globs = get_globals();

    return OSSL_PROVIDER_get_capabilities(globs->deflt, capability, cb, arg);
}

static const OSSL_ALGORITHM *filter_query(void *provctx,
                                          int operation_id,
                                          int *no_cache)
{
    struct filter_prov_globals_st *globs = get_globals();
    int i;

    globs->query_count++;
    for (i = 0; i < globs->num_dispatch; i++) {
        if (globs->dispatch[i].operation == operation_id) {
            *no_cache = globs->no_cache;
            return globs->dispatch[i].alg;
        }
    }

    /* No filter set, so pass it down to the chained provider */
    return OSSL_PROVIDER_query_operation(globs->deflt, operation_id, no_cache);
}

static void filter_unquery(void *provctx, int operation_id,
                           const OSSL_ALGORITHM *algs)
{
    struct filter_prov_globals_st *globs = get_globals();
    int i;

    if (!TEST_ulong_gt(globs->query_count, 0))
        globs->error = 1;
    else
        globs->query_count--;

    for (i = 0; i < globs->num_dispatch; i++)
        if (globs->dispatch[i].alg == algs)
            return;
    OSSL_PROVIDER_unquery_operation(globs->deflt, operation_id, algs);
}

static void filter_teardown(void *provctx)
{
    struct filter_prov_globals_st *globs = get_globals();

    OSSL_PROVIDER_unload(globs->deflt);
    OSSL_LIB_CTX_free(globs->libctx);
    memset(globs, 0, sizeof(*globs));
    BIO_meth_free(ossl_prov_ctx_get0_core_bio_method(provctx));
    ossl_prov_ctx_free(provctx);
}

/* Functions we provide to the core */
static const OSSL_DISPATCH filter_dispatch_table[] = {
    { OSSL_FUNC_PROVIDER_GETTABLE_PARAMS, (void (*)(void))filter_gettable_params },
    { OSSL_FUNC_PROVIDER_GET_PARAMS, (void (*)(void))filter_get_params },
    { OSSL_FUNC_PROVIDER_QUERY_OPERATION, (void (*)(void))filter_query },
    { OSSL_FUNC_PROVIDER_UNQUERY_OPERATION, (void (*)(void))filter_unquery },
    { OSSL_FUNC_PROVIDER_GET_CAPABILITIES, (void (*)(void))filter_get_capabilities },
    { OSSL_FUNC_PROVIDER_TEARDOWN, (void (*)(void))filter_teardown },
    OSSL_DISPATCH_END
};

int filter_provider_init(const OSSL_CORE_HANDLE *handle,
                         const OSSL_DISPATCH *in,
                         const OSSL_DISPATCH **out,
                         void **provctx)
{
    OSSL_FUNC_core_get_libctx_fn *c_get_libctx = NULL;
    BIO_METHOD *corebiometh;

    if (!ossl_prov_bio_from_dispatch(in))
        return 0;
    for (; in->function_id != 0; in++) {
        switch (in->function_id) {
        case OSSL_FUNC_CORE_GET_LIBCTX:
            c_get_libctx = OSSL_FUNC_core_get_libctx(in);
            break;
        default:
            /* Just ignore anything we don't understand */
            break;
        }
    }

    if (c_get_libctx == NULL)
        return 0;

    memset(&ourglobals, 0, sizeof(ourglobals));
    ourglobals.libctx = OSSL_LIB_CTX_new();
    if (ourglobals.libctx == NULL)
        goto err;

    ourglobals.deflt = OSSL_PROVIDER_load(ourglobals.libctx, "default");
    if (ourglobals.deflt == NULL)
        goto err;

    /*
     * We want to make sure that all calls from this provider that requires
     * a library context use the same context as the one used to call our
     * functions.  We do that by passing it along in the provider context.
     *
     * This only works for built-in providers.  Most providers should
     * create their own library context.
     */
    if ((*provctx = ossl_prov_ctx_new()) == NULL
            || (corebiometh = ossl_bio_prov_init_bio_method()) == NULL) {
        ossl_prov_ctx_free(*provctx);
        *provctx = NULL;
        goto err;
    }
    ossl_prov_ctx_set0_libctx(*provctx, (OSSL_LIB_CTX *)c_get_libctx(handle));
    ossl_prov_ctx_set0_handle(*provctx, handle);
    ossl_prov_ctx_set0_core_bio_method(*provctx, corebiometh);
    *out = filter_dispatch_table;
    return 1;

 err:
    OSSL_PROVIDER_unload(ourglobals.deflt);
    OSSL_LIB_CTX_free(ourglobals.libctx);
    return 0;
}

/*
 * Set a filter for the given operation id. The filter string is a colon
 * separated list of algorithms that will be made available by this provider.
 * Anything not in the filter will be suppressed. If a filter is not set for
 * a given operation id then all algorithms are made available.
 */
int filter_provider_set_filter(int operation, const char *filterstr)
{
    int no_cache = 0;
    int algnum = 0, last = 0, ret = 0;
    struct filter_prov_globals_st *globs = get_globals();
    size_t namelen;
    char *filterstrtmp = OPENSSL_strdup(filterstr);
    char *name, *sep;
    const OSSL_ALGORITHM *provalgs = OSSL_PROVIDER_query_operation(globs->deflt,
                                                                   operation,
                                                                   &no_cache);
    const OSSL_ALGORITHM *algs;

    if (filterstrtmp == NULL)
        goto err;

    /* Nothing to filter */
    if (provalgs == NULL)
        goto err;

    if (globs->num_dispatch >= MAX_FILTERS)
        goto err;

    for (name = filterstrtmp; !last; name = (sep == NULL ? NULL : sep + 1)) {
        sep = strstr(name, ":");
        if (sep != NULL)
            *sep = '\0';
        else
            last = 1;
        namelen = strlen(name);

        for (algs = provalgs; algs->algorithm_names != NULL; algs++) {
            const char *found = strstr(algs->algorithm_names, name);

            if (found == NULL)
                continue;
            if (found[namelen] != '\0' && found[namelen] != ':')
                continue;
            if (found != algs->algorithm_names && found[-1] != ':')
                continue;

            /* We found a match */
            if (algnum >= MAX_ALG_FILTERS)
                goto err;

            globs->dispatch[globs->num_dispatch].alg[algnum++] = *algs;
            break;
        }
        if (algs->algorithm_names == NULL) {
            /* No match found */
            goto err;
        }
    }

    globs->dispatch[globs->num_dispatch].operation = operation;
    globs->no_cache = no_cache;
    globs->num_dispatch++;

    ret = 1;
 err:
    OSSL_PROVIDER_unquery_operation(globs->deflt, operation, provalgs);
    OPENSSL_free(filterstrtmp);
    return ret;
}

/*
 * Test if a filter provider is in a clean finishing state.
 * If it is return 1, otherwise return 0.
 */
int filter_provider_check_clean_finish(void)
{
    struct filter_prov_globals_st *globs = get_globals();

    return TEST_ulong_eq(globs->query_count, 0) && !globs->error;
}
