/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stddef.h>

#include <openssl/core.h>
#include <openssl/trace.h>
#include "internal/cryptlib.h"
#include "internal/core.h"
#include "internal/property.h"
#include "internal/provider.h"

struct construct_data_st {
    OSSL_LIB_CTX *libctx;
    OSSL_METHOD_STORE *store;
    int operation_id;
    int force_store;
    OSSL_METHOD_CONSTRUCT_METHOD *mcm;
    void *mcm_data;
};

static int is_temporary_method_store(int no_store, void *cbdata)
{
    struct construct_data_st *data = cbdata;

    return no_store && !data->force_store;
}

static int ossl_method_construct_reserve_store(int no_store, void *cbdata)
{
    struct construct_data_st *data = cbdata;

    if (is_temporary_method_store(no_store, data) && data->store == NULL) {
        /*
         * If we have been told not to store the method "permanently", we
         * ask for a temporary store, and store the method there.
         * The owner of |data->mcm| is completely responsible for managing
         * that temporary store.
         */
        if ((data->store = data->mcm->get_tmp_store(data->mcm_data)) == NULL)
            return 0;
    }

    return data->mcm->lock_store(data->store, data->mcm_data);
}

static int ossl_method_construct_unreserve_store(void *cbdata)
{
    struct construct_data_st *data = cbdata;

    return data->mcm->unlock_store(data->store, data->mcm_data);
}

static int ossl_method_construct_precondition(OSSL_PROVIDER *provider,
                                              int operation_id, int no_store,
                                              void *cbdata, int *result)
{
    if (!ossl_assert(result != NULL)) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    /* Assume that no bits are set */
    *result = 0;

    /* No flag bits for temporary stores */
    if (!is_temporary_method_store(no_store, cbdata)
        && !ossl_provider_test_operation_bit(provider, operation_id, result))
        return 0;

    /*
     * The result we get tells if methods have already been constructed.
     * However, we want to tell whether construction should happen (true)
     * or not (false), which is the opposite of what we got.
     */
    *result = !*result;

    return 1;
}

static int ossl_method_construct_postcondition(OSSL_PROVIDER *provider,
                                               int operation_id, int no_store,
                                               void *cbdata, int *result)
{
    if (!ossl_assert(result != NULL)) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    *result = 1;

    /* No flag bits for temporary stores */
    return is_temporary_method_store(no_store, cbdata)
        || ossl_provider_set_operation_bit(provider, operation_id);
}

static void ossl_method_construct_this(OSSL_PROVIDER *provider,
                                       const OSSL_ALGORITHM *algo,
                                       int no_store, void *cbdata)
{
    struct construct_data_st *data = cbdata;
    void *method = NULL;

    if ((method = data->mcm->construct(algo, provider, data->mcm_data))
        == NULL)
        return;

    OSSL_TRACE2(QUERY,
                "ossl_method_construct_this: putting an algo to the store %p with no_store %d\n",
                (void *)data->store, no_store);
    /*
     * Note regarding putting the method in stores:
     *
     * we don't need to care if it actually got in or not here.
     * If it didn't get in, it will simply not be available when
     * ossl_method_construct() tries to get it from the store.
     *
     * It is *expected* that the put function increments the refcnt
     * of the passed method.
     */
    data->mcm->put(no_store ? data->store : NULL, method, provider, algo->algorithm_names,
                   algo->property_definition, data->mcm_data);

    /* refcnt-- because we're dropping the reference */
    data->mcm->destruct(method, data->mcm_data);
}

void *ossl_method_construct(OSSL_LIB_CTX *libctx, int operation_id,
                            OSSL_PROVIDER **provider_rw, int force_store,
                            OSSL_METHOD_CONSTRUCT_METHOD *mcm, void *mcm_data)
{
    void *method = NULL;
    OSSL_PROVIDER *provider = provider_rw != NULL ? *provider_rw : NULL;
    struct construct_data_st cbdata;

    /*
     * We might be tempted to try to look into the method store without
     * constructing to see if we can find our method there already.
     * Unfortunately that does not work well if the query contains
     * optional properties as newly loaded providers can match them better.
     * We trust that ossl_method_construct_precondition() and
     * ossl_method_construct_postcondition() make sure that the
     * ossl_algorithm_do_all() does very little when methods from
     * a provider have already been constructed.
     */

    cbdata.store = NULL;
    cbdata.force_store = force_store;
    cbdata.mcm = mcm;
    cbdata.mcm_data = mcm_data;
    ossl_algorithm_do_all(libctx, operation_id, provider,
                          ossl_method_construct_precondition,
                          ossl_method_construct_reserve_store,
                          ossl_method_construct_this,
                          ossl_method_construct_unreserve_store,
                          ossl_method_construct_postcondition,
                          &cbdata);

    /* If there is a temporary store, try there first */
    if (cbdata.store != NULL)
        method = mcm->get(cbdata.store, (const OSSL_PROVIDER **)provider_rw,
                          mcm_data);

    /* If no method was found yet, try the global store */
    if (method == NULL)
        method = mcm->get(NULL, (const OSSL_PROVIDER **)provider_rw, mcm_data);

    return method;
}
