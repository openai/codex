/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stddef.h>
#include <openssl/types.h>
#include <openssl/evp.h>
#include <openssl/core.h>
#include "internal/cryptlib.h"
#include "internal/thread_once.h"
#include "internal/property.h"
#include "internal/core.h"
#include "internal/provider.h"
#include "internal/namemap.h"
#include "crypto/decoder.h"
#include "crypto/evp.h"    /* evp_local.h needs it */
#include "evp_local.h"

#define NAME_SEPARATOR ':'

/* Data to be passed through ossl_method_construct() */
struct evp_method_data_st {
    OSSL_LIB_CTX *libctx;
    int operation_id;            /* For get_evp_method_from_store() */
    int name_id;                 /* For get_evp_method_from_store() */
    const char *names;           /* For get_evp_method_from_store() */
    const char *propquery;       /* For get_evp_method_from_store() */

    OSSL_METHOD_STORE *tmp_store; /* For get_tmp_evp_method_store() */

    unsigned int flag_construct_error_occurred : 1;

    void *(*method_from_algorithm)(int name_id, const OSSL_ALGORITHM *,
                                   OSSL_PROVIDER *);
    int (*refcnt_up_method)(void *method);
    void (*destruct_method)(void *method);
};

/*
 * Generic routines to fetch / create EVP methods with ossl_method_construct()
 */
static void *get_tmp_evp_method_store(void *data)
{
    struct evp_method_data_st *methdata = data;

    if (methdata->tmp_store == NULL) {
        methdata->tmp_store = ossl_method_store_new(methdata->libctx);
        OSSL_TRACE1(QUERY, "Allocating a new tmp_store %p\n", (void *)methdata->tmp_store);
    } else {
        OSSL_TRACE1(QUERY, "Using the existing tmp_store %p\n", (void *)methdata->tmp_store);
    }
    return methdata->tmp_store;
}

 static void dealloc_tmp_evp_method_store(void *store)
{
    OSSL_TRACE1(QUERY, "Deallocating the tmp_store %p\n", store);
    if (store != NULL)
        ossl_method_store_free(store);
}

static OSSL_METHOD_STORE *get_evp_method_store(OSSL_LIB_CTX *libctx)
{
    return ossl_lib_ctx_get_data(libctx, OSSL_LIB_CTX_EVP_METHOD_STORE_INDEX);
}

static int reserve_evp_method_store(void *store, void *data)
{
    struct evp_method_data_st *methdata = data;

    if (store == NULL
        && (store = get_evp_method_store(methdata->libctx)) == NULL)
        return 0;

    return ossl_method_lock_store(store);
}

static int unreserve_evp_method_store(void *store, void *data)
{
    struct evp_method_data_st *methdata = data;

    if (store == NULL
        && (store = get_evp_method_store(methdata->libctx)) == NULL)
        return 0;

    return ossl_method_unlock_store(store);
}

/*
 * To identify the method in the EVP method store, we mix the name identity
 * with the operation identity, under the assumption that we don't have more
 * than 2^23 names or more than 2^8 operation types.
 *
 * The resulting identity is a 31-bit integer, composed like this:
 *
 * +---------23 bits--------+-8 bits-+
 * |      name identity     | op id  |
 * +------------------------+--------+
 *
 * We limit this composite number to 31 bits, thus leaving the top uint32_t
 * bit always zero, to avoid negative sign extension when downshifting after
 * this number happens to be passed to an int (which happens as soon as it's
 * passed to ossl_method_store_cache_set(), and it's in that form that it
 * gets passed along to filter_on_operation_id(), defined further down.
 */
#define METHOD_ID_OPERATION_MASK        0x000000FF
#define METHOD_ID_OPERATION_MAX         ((1 << 8) - 1)
#define METHOD_ID_NAME_MASK             0x7FFFFF00
#define METHOD_ID_NAME_OFFSET           8
#define METHOD_ID_NAME_MAX              ((1 << 23) - 1)
static uint32_t evp_method_id(int name_id, unsigned int operation_id)
{
    if (!ossl_assert(name_id > 0 && name_id <= METHOD_ID_NAME_MAX)
        || !ossl_assert(operation_id > 0
                        && operation_id <= METHOD_ID_OPERATION_MAX))
        return 0;
    return (((name_id << METHOD_ID_NAME_OFFSET) & METHOD_ID_NAME_MASK)
            | (operation_id & METHOD_ID_OPERATION_MASK));
}

static void *get_evp_method_from_store(void *store, const OSSL_PROVIDER **prov,
                                       void *data)
{
    struct evp_method_data_st *methdata = data;
    void *method = NULL;
    int name_id;
    uint32_t meth_id;

    /*
     * get_evp_method_from_store() is only called to try and get the method
     * that evp_generic_fetch() is asking for, and the operation id as well
     * as the name or name id are passed via methdata.
     */
    if ((name_id = methdata->name_id) == 0 && methdata->names != NULL) {
        OSSL_NAMEMAP *namemap = ossl_namemap_stored(methdata->libctx);
        const char *names = methdata->names;
        const char *q = strchr(names, NAME_SEPARATOR);
        size_t l = (q == NULL ? strlen(names) : (size_t)(q - names));

        if (namemap == 0)
            return NULL;
        name_id = ossl_namemap_name2num_n(namemap, names, l);
    }

    if (name_id == 0
        || (meth_id = evp_method_id(name_id, methdata->operation_id)) == 0)
        return NULL;

    if (store == NULL
        && (store = get_evp_method_store(methdata->libctx)) == NULL)
        return NULL;

    if (!ossl_method_store_fetch(store, meth_id, methdata->propquery, prov,
                                 &method))
        return NULL;
    return method;
}

static int put_evp_method_in_store(void *store, void *method,
                                   const OSSL_PROVIDER *prov,
                                   const char *names, const char *propdef,
                                   void *data)
{
    struct evp_method_data_st *methdata = data;
    OSSL_NAMEMAP *namemap;
    int name_id;
    uint32_t meth_id;
    size_t l = 0;

    /*
     * put_evp_method_in_store() is only called with an EVP method that was
     * successfully created by construct_method() below, which means that
     * all the names should already be stored in the namemap with the same
     * numeric identity, so just use the first to get that identity.
     */
    if (names != NULL) {
        const char *q = strchr(names, NAME_SEPARATOR);

        l = (q == NULL ? strlen(names) : (size_t)(q - names));
    }

    if ((namemap = ossl_namemap_stored(methdata->libctx)) == NULL
        || (name_id = ossl_namemap_name2num_n(namemap, names, l)) == 0
        || (meth_id = evp_method_id(name_id, methdata->operation_id)) == 0)
        return 0;

    OSSL_TRACE1(QUERY, "put_evp_method_in_store: original store: %p\n", store);
    if (store == NULL
        && (store = get_evp_method_store(methdata->libctx)) == NULL)
        return 0;

    OSSL_TRACE5(QUERY,
                "put_evp_method_in_store: "
                "store: %p, names: %s, operation_id %d, method_id: %d, properties: %s\n",
                store, names, methdata->operation_id, meth_id, propdef ? propdef : "<null>");
    return ossl_method_store_add(store, prov, meth_id, propdef, method,
                                 methdata->refcnt_up_method,
                                 methdata->destruct_method);
}

/*
 * The core fetching functionality passes the name of the implementation.
 * This function is responsible to getting an identity number for it.
 */
static void *construct_evp_method(const OSSL_ALGORITHM *algodef,
                                  OSSL_PROVIDER *prov, void *data)
{
    /*
     * This function is only called if get_evp_method_from_store() returned
     * NULL, so it's safe to say that of all the spots to create a new
     * namemap entry, this is it.  Should the name already exist there, we
     * know that ossl_namemap_add_name() will return its corresponding
     * number.
     */
    struct evp_method_data_st *methdata = data;
    OSSL_LIB_CTX *libctx = ossl_provider_libctx(prov);
    OSSL_NAMEMAP *namemap = ossl_namemap_stored(libctx);
    const char *names = algodef->algorithm_names;
    int name_id = ossl_namemap_add_names(namemap, 0, names, NAME_SEPARATOR);
    void *method;

    if (name_id == 0)
        return NULL;

    method = methdata->method_from_algorithm(name_id, algodef, prov);

    /*
     * Flag to indicate that there was actual construction errors.  This
     * helps inner_evp_generic_fetch() determine what error it should
     * record on inaccessible algorithms.
     */
    if (method == NULL)
        methdata->flag_construct_error_occurred = 1;

    return method;
}

static void destruct_evp_method(void *method, void *data)
{
    struct evp_method_data_st *methdata = data;

    methdata->destruct_method(method);
}

static void *
inner_evp_generic_fetch(struct evp_method_data_st *methdata,
                        OSSL_PROVIDER *prov, int operation_id,
                        const char *name, ossl_unused const char *properties,
                        void *(*new_method)(int name_id,
                                            const OSSL_ALGORITHM *algodef,
                                            OSSL_PROVIDER *prov),
                        int (*up_ref_method)(void *),
                        void (*free_method)(void *))
{
    OSSL_METHOD_STORE *store = get_evp_method_store(methdata->libctx);
    OSSL_NAMEMAP *namemap = ossl_namemap_stored(methdata->libctx);
#ifdef FIPS_MODULE
    /*
     * The FIPS provider has its own internal library context where only it
     * is loaded.  Consequently, property queries aren't relevant because
     * there is only one fetchable algorithm and it is assumed that the
     * FIPS-ness is handled by the using algorithm.
     */
    const char *const propq = "";
#else
    const char *const propq = properties != NULL ? properties : "";
#endif  /* FIPS_MODULE */
    uint32_t meth_id = 0;
    void *method = NULL;
    int unsupported, name_id;

    if (store == NULL || namemap == NULL) {
        ERR_raise(ERR_LIB_EVP, ERR_R_PASSED_INVALID_ARGUMENT);
        return NULL;
    }

    /*
     * If there's ever an operation_id == 0 passed, we have an internal
     * programming error.
     */
    if (!ossl_assert(operation_id > 0)) {
        ERR_raise(ERR_LIB_EVP, ERR_R_INTERNAL_ERROR);
        return NULL;
    }

    /* If we haven't received a name id yet, try to get one for the name */
    name_id = name != NULL ? ossl_namemap_name2num(namemap, name) : 0;

    /*
     * If we have a name id, calculate a method id with evp_method_id().
     *
     * evp_method_id returns 0 if we have too many operations (more than
     * about 2^8) or too many names (more than about 2^24).  In that case,
     * we can't create any new method.
     * For all intents and purposes, this is an internal error.
     */
    if (name_id != 0 && (meth_id = evp_method_id(name_id, operation_id)) == 0) {
        ERR_raise(ERR_LIB_EVP, ERR_R_INTERNAL_ERROR);
        return NULL;
    }

    /*
     * If we haven't found the name yet, chances are that the algorithm to
     * be fetched is unsupported.
     */
    unsupported = name_id == 0;

    if (meth_id == 0
        || !ossl_method_store_cache_get(store, prov, meth_id, propq, &method)) {
        OSSL_METHOD_CONSTRUCT_METHOD mcm = {
            get_tmp_evp_method_store,
            reserve_evp_method_store,
            unreserve_evp_method_store,
            get_evp_method_from_store,
            put_evp_method_in_store,
            construct_evp_method,
            destruct_evp_method
        };

        methdata->operation_id = operation_id;
        methdata->name_id = name_id;
        methdata->names = name;
        methdata->propquery = propq;
        methdata->method_from_algorithm = new_method;
        methdata->refcnt_up_method = up_ref_method;
        methdata->destruct_method = free_method;
        methdata->flag_construct_error_occurred = 0;
        if ((method = ossl_method_construct(methdata->libctx, operation_id,
                                            &prov, 0 /* !force_cache */,
                                            &mcm, methdata)) != NULL) {
            /*
             * If construction did create a method for us, we know that
             * there is a correct name_id and meth_id, since those have
             * already been calculated in get_evp_method_from_store() and
             * put_evp_method_in_store() above.
             * Note that there is a corner case here, in which, if a user
             * passes a name of the form name1:name2:..., then the construction
             * will create a method against all names, but the lookup will fail
             * as ossl_namemap_name2num treats the name string as a single name
             * rather than introducing new features where in the EVP_<obj>_fetch
             * parses the string and queries for each, return an error.
             */
            if (name_id == 0)
                name_id = ossl_namemap_name2num(namemap, name);
            if (name_id == 0) {
                ERR_raise_data(ERR_LIB_EVP, ERR_R_FETCH_FAILED,
                               "Algorithm %s cannot be found", name);
                free_method(method);
                method = NULL;
            } else {
                meth_id = evp_method_id(name_id, operation_id);
                if (meth_id != 0)
                    ossl_method_store_cache_set(store, prov, meth_id, propq,
                                                method, up_ref_method, free_method);
            }
        }

        /*
         * If we never were in the constructor, the algorithm to be fetched
         * is unsupported.
         */
        unsupported = !methdata->flag_construct_error_occurred;
    }

    if ((name_id != 0 || name != NULL) && method == NULL) {
        int code = unsupported ? ERR_R_UNSUPPORTED : ERR_R_FETCH_FAILED;

        if (name == NULL)
            name = ossl_namemap_num2name(namemap, name_id, 0);
        ERR_raise_data(ERR_LIB_EVP, code,
                       "%s, Algorithm (%s : %d), Properties (%s)",
                       ossl_lib_ctx_get_descriptor(methdata->libctx),
                       name == NULL ? "<null>" : name, name_id,
                       properties == NULL ? "<null>" : properties);
    } else {
        OSSL_TRACE4(QUERY, "%s, Algorithm (%s : %d), Properties (%s)\n",
                    ossl_lib_ctx_get_descriptor(methdata->libctx),
                    name == NULL ? "<null>" : name, name_id,
                    properties == NULL ? "<null>" : properties);
    }

    return method;
}

void *evp_generic_fetch(OSSL_LIB_CTX *libctx, int operation_id,
                        const char *name, const char *properties,
                        void *(*new_method)(int name_id,
                                            const OSSL_ALGORITHM *algodef,
                                            OSSL_PROVIDER *prov),
                        int (*up_ref_method)(void *),
                        void (*free_method)(void *))
{
    struct evp_method_data_st methdata;
    void *method;

    methdata.libctx = libctx;
    methdata.tmp_store = NULL;
    method = inner_evp_generic_fetch(&methdata, NULL, operation_id,
                                     name, properties,
                                     new_method, up_ref_method, free_method);
    dealloc_tmp_evp_method_store(methdata.tmp_store);
    return method;
}

/*
 * evp_generic_fetch_from_prov() is special, and only returns methods from
 * the given provider.
 * This is meant to be used when one method needs to fetch an associated
 * method.
 */
void *evp_generic_fetch_from_prov(OSSL_PROVIDER *prov, int operation_id,
                                  const char *name, const char *properties,
                                  void *(*new_method)(int name_id,
                                                      const OSSL_ALGORITHM *algodef,
                                                      OSSL_PROVIDER *prov),
                                  int (*up_ref_method)(void *),
                                  void (*free_method)(void *))
{
    struct evp_method_data_st methdata;
    void *method;

    methdata.libctx = ossl_provider_libctx(prov);
    methdata.tmp_store = NULL;
    method = inner_evp_generic_fetch(&methdata, prov, operation_id,
                                     name, properties,
                                     new_method, up_ref_method, free_method);
    dealloc_tmp_evp_method_store(methdata.tmp_store);
    return method;
}

int evp_method_store_cache_flush(OSSL_LIB_CTX *libctx)
{
    OSSL_METHOD_STORE *store = get_evp_method_store(libctx);

    if (store != NULL)
        return ossl_method_store_cache_flush_all(store);
    return 1;
}

int evp_method_store_remove_all_provided(const OSSL_PROVIDER *prov)
{
    OSSL_LIB_CTX *libctx = ossl_provider_libctx(prov);
    OSSL_METHOD_STORE *store = get_evp_method_store(libctx);

    if (store != NULL)
        return ossl_method_store_remove_all_provided(store, prov);
    return 1;
}

static int evp_set_parsed_default_properties(OSSL_LIB_CTX *libctx,
                                             OSSL_PROPERTY_LIST *def_prop,
                                             int loadconfig,
                                             int mirrored)
{
    OSSL_METHOD_STORE *store = get_evp_method_store(libctx);
    OSSL_PROPERTY_LIST **plp = ossl_ctx_global_properties(libctx, loadconfig);

    if (plp != NULL && store != NULL) {
        int ret;
#ifndef FIPS_MODULE
        char *propstr = NULL;
        size_t strsz;

        if (mirrored) {
            if (ossl_global_properties_no_mirrored(libctx))
                return 0;
        } else {
            /*
             * These properties have been explicitly set on this libctx, so
             * don't allow any mirroring from a parent libctx.
             */
            ossl_global_properties_stop_mirroring(libctx);
        }

        strsz = ossl_property_list_to_string(libctx, def_prop, NULL, 0);
        if (strsz > 0)
            propstr = OPENSSL_malloc(strsz);
        if (propstr == NULL) {
            ERR_raise(ERR_LIB_EVP, ERR_R_INTERNAL_ERROR);
            return 0;
        }
        if (ossl_property_list_to_string(libctx, def_prop, propstr,
                                         strsz) == 0) {
            OPENSSL_free(propstr);
            ERR_raise(ERR_LIB_EVP, ERR_R_INTERNAL_ERROR);
            return 0;
        }
        ossl_provider_default_props_update(libctx, propstr);
        OPENSSL_free(propstr);
#endif
        ossl_property_free(*plp);
        *plp = def_prop;

        ret = ossl_method_store_cache_flush_all(store);
#ifndef FIPS_MODULE
        ossl_decoder_cache_flush(libctx);
#endif
        return ret;
    }
    ERR_raise(ERR_LIB_EVP, ERR_R_INTERNAL_ERROR);
    return 0;
}

int evp_set_default_properties_int(OSSL_LIB_CTX *libctx, const char *propq,
                                   int loadconfig, int mirrored)
{
    OSSL_PROPERTY_LIST *pl = NULL;

    if (propq != NULL && (pl = ossl_parse_query(libctx, propq, 1)) == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_DEFAULT_QUERY_PARSE_ERROR);
        return 0;
    }
    if (!evp_set_parsed_default_properties(libctx, pl, loadconfig, mirrored)) {
        ossl_property_free(pl);
        return 0;
    }
    return 1;
}

int EVP_set_default_properties(OSSL_LIB_CTX *libctx, const char *propq)
{
    return evp_set_default_properties_int(libctx, propq, 1, 0);
}

static int evp_default_properties_merge(OSSL_LIB_CTX *libctx, const char *propq,
                                        int loadconfig)
{
    OSSL_PROPERTY_LIST **plp = ossl_ctx_global_properties(libctx, loadconfig);
    OSSL_PROPERTY_LIST *pl1, *pl2;

    if (propq == NULL)
        return 1;
    if (plp == NULL || *plp == NULL)
        return evp_set_default_properties_int(libctx, propq, 0, 0);
    if ((pl1 = ossl_parse_query(libctx, propq, 1)) == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_DEFAULT_QUERY_PARSE_ERROR);
        return 0;
    }
    pl2 = ossl_property_merge(pl1, *plp);
    ossl_property_free(pl1);
    if (pl2 == NULL) {
        ERR_raise(ERR_LIB_EVP, ERR_R_CRYPTO_LIB);
        return 0;
    }
    if (!evp_set_parsed_default_properties(libctx, pl2, 0, 0)) {
        ossl_property_free(pl2);
        return 0;
    }
    return 1;
}

static int evp_default_property_is_enabled(OSSL_LIB_CTX *libctx,
                                           const char *prop_name)
{
    OSSL_PROPERTY_LIST **plp = ossl_ctx_global_properties(libctx, 1);

    return plp != NULL && ossl_property_is_enabled(libctx, prop_name, *plp);
}

int EVP_default_properties_is_fips_enabled(OSSL_LIB_CTX *libctx)
{
    return evp_default_property_is_enabled(libctx, "fips");
}

int evp_default_properties_enable_fips_int(OSSL_LIB_CTX *libctx, int enable,
                                           int loadconfig)
{
    const char *query = (enable != 0) ? "fips=yes" : "-fips";

    return evp_default_properties_merge(libctx, query, loadconfig);
}

int EVP_default_properties_enable_fips(OSSL_LIB_CTX *libctx, int enable)
{
    return evp_default_properties_enable_fips_int(libctx, enable, 1);
}

char *evp_get_global_properties_str(OSSL_LIB_CTX *libctx, int loadconfig)
{
    OSSL_PROPERTY_LIST **plp = ossl_ctx_global_properties(libctx, loadconfig);
    char *propstr = NULL;
    size_t sz;

    if (plp == NULL)
        return OPENSSL_strdup("");

    sz = ossl_property_list_to_string(libctx, *plp, NULL, 0);
    if (sz == 0) {
        ERR_raise(ERR_LIB_EVP, ERR_R_INTERNAL_ERROR);
        return NULL;
    }

    propstr = OPENSSL_malloc(sz);
    if (propstr == NULL)
        return NULL;
    if (ossl_property_list_to_string(libctx, *plp, propstr, sz) == 0) {
        ERR_raise(ERR_LIB_EVP, ERR_R_INTERNAL_ERROR);
        OPENSSL_free(propstr);
        return NULL;
    }
    return propstr;
}

char *EVP_get1_default_properties(OSSL_LIB_CTX *libctx)
{
    return evp_get_global_properties_str(libctx, ossl_lib_ctx_is_global_default(libctx));
}

struct filter_data_st {
    int operation_id;
    void (*user_fn)(void *method, void *arg);
    void *user_arg;
};

static void filter_on_operation_id(int id, void *method, void *arg)
{
    struct filter_data_st *data = arg;

    if ((id & METHOD_ID_OPERATION_MASK) == data->operation_id)
        data->user_fn(method, data->user_arg);
}

void evp_generic_do_all(OSSL_LIB_CTX *libctx, int operation_id,
                        void (*user_fn)(void *method, void *arg),
                        void *user_arg,
                        void *(*new_method)(int name_id,
                                            const OSSL_ALGORITHM *algodef,
                                            OSSL_PROVIDER *prov),
                        int (*up_ref_method)(void *),
                        void (*free_method)(void *))
{
    struct evp_method_data_st methdata;
    struct filter_data_st data;

    methdata.libctx = libctx;
    methdata.tmp_store = NULL;
    (void)inner_evp_generic_fetch(&methdata, NULL, operation_id, NULL, NULL,
                                  new_method, up_ref_method, free_method);

    data.operation_id = operation_id;
    data.user_fn = user_fn;
    data.user_arg = user_arg;
    if (methdata.tmp_store != NULL)
        ossl_method_store_do_all(methdata.tmp_store, &filter_on_operation_id,
                                 &data);
    ossl_method_store_do_all(get_evp_method_store(libctx),
                             &filter_on_operation_id, &data);
    dealloc_tmp_evp_method_store(methdata.tmp_store);
}

int evp_is_a(OSSL_PROVIDER *prov, int number,
             const char *legacy_name, const char *name)
{
    /*
     * For a |prov| that is NULL, the library context will be NULL
     */
    OSSL_LIB_CTX *libctx = ossl_provider_libctx(prov);
    OSSL_NAMEMAP *namemap = ossl_namemap_stored(libctx);

    if (prov == NULL)
        number = ossl_namemap_name2num(namemap, legacy_name);
    return ossl_namemap_name2num(namemap, name) == number;
}

int evp_names_do_all(OSSL_PROVIDER *prov, int number,
                     void (*fn)(const char *name, void *data),
                     void *data)
{
    OSSL_LIB_CTX *libctx = ossl_provider_libctx(prov);
    OSSL_NAMEMAP *namemap = ossl_namemap_stored(libctx);

    return ossl_namemap_doall_names(namemap, number, fn, data);
}
