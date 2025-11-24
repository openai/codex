/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core.h>
#include <openssl/core_dispatch.h>
#include <openssl/encoder.h>
#include <openssl/ui.h>
#include "internal/core.h"
#include "internal/namemap.h"
#include "internal/property.h"
#include "internal/provider.h"
#include "crypto/encoder.h"
#include "encoder_local.h"
#include "crypto/context.h"

/*
 * Encoder can have multiple names, separated with colons in a name string
 */
#define NAME_SEPARATOR ':'

static void ossl_encoder_free(void *data)
{
    OSSL_ENCODER_free(data);
}

static int ossl_encoder_up_ref(void *data)
{
    return OSSL_ENCODER_up_ref(data);
}

/* Simple method structure constructor and destructor */
static OSSL_ENCODER *ossl_encoder_new(void)
{
    OSSL_ENCODER *encoder = NULL;

    if ((encoder = OPENSSL_zalloc(sizeof(*encoder))) == NULL)
        return NULL;
    if (!CRYPTO_NEW_REF(&encoder->base.refcnt, 1)) {
        OSSL_ENCODER_free(encoder);
        return NULL;
    }

    return encoder;
}

int OSSL_ENCODER_up_ref(OSSL_ENCODER *encoder)
{
    int ref = 0;

    CRYPTO_UP_REF(&encoder->base.refcnt, &ref);
    return 1;
}

void OSSL_ENCODER_free(OSSL_ENCODER *encoder)
{
    int ref = 0;

    if (encoder == NULL)
        return;

    CRYPTO_DOWN_REF(&encoder->base.refcnt, &ref);
    if (ref > 0)
        return;
    OPENSSL_free(encoder->base.name);
    ossl_property_free(encoder->base.parsed_propdef);
    ossl_provider_free(encoder->base.prov);
    CRYPTO_FREE_REF(&encoder->base.refcnt);
    OPENSSL_free(encoder);
}

/* Data to be passed through ossl_method_construct() */
struct encoder_data_st {
    OSSL_LIB_CTX *libctx;
    int id;                      /* For get_encoder_from_store() */
    const char *names;           /* For get_encoder_from_store() */
    const char *propquery;       /* For get_encoder_from_store() */

    OSSL_METHOD_STORE *tmp_store; /* For get_tmp_encoder_store() */

    unsigned int flag_construct_error_occurred : 1;
};

/*
 * Generic routines to fetch / create ENCODER methods with
 * ossl_method_construct()
 */

/* Temporary encoder method store, constructor and destructor */
static void *get_tmp_encoder_store(void *data)
{
    struct encoder_data_st *methdata = data;

    if (methdata->tmp_store == NULL)
        methdata->tmp_store = ossl_method_store_new(methdata->libctx);
    return methdata->tmp_store;
}

static void dealloc_tmp_encoder_store(void *store)
{
    if (store != NULL)
        ossl_method_store_free(store);
}

/* Get the permanent encoder store */
static OSSL_METHOD_STORE *get_encoder_store(OSSL_LIB_CTX *libctx)
{
    return ossl_lib_ctx_get_data(libctx, OSSL_LIB_CTX_ENCODER_STORE_INDEX);
}

static int reserve_encoder_store(void *store, void *data)
{
    struct encoder_data_st *methdata = data;

    if (store == NULL
        && (store = get_encoder_store(methdata->libctx)) == NULL)
        return 0;

    return ossl_method_lock_store(store);
}

static int unreserve_encoder_store(void *store, void *data)
{
    struct encoder_data_st *methdata = data;

    if (store == NULL
        && (store = get_encoder_store(methdata->libctx)) == NULL)
        return 0;

    return ossl_method_unlock_store(store);
}

/* Get encoder methods from a store, or put one in */
static void *get_encoder_from_store(void *store, const OSSL_PROVIDER **prov,
                                    void *data)
{
    struct encoder_data_st *methdata = data;
    void *method = NULL;
    int id;

    /*
     * get_encoder_from_store() is only called to try and get the method
     * that OSSL_ENCODER_fetch() is asking for, and the name or name id are
     * passed via methdata.
     */
    if ((id = methdata->id) == 0 && methdata->names != NULL) {
        OSSL_NAMEMAP *namemap = ossl_namemap_stored(methdata->libctx);
        const char *names = methdata->names;
        const char *q = strchr(names, NAME_SEPARATOR);
        size_t l = (q == NULL ? strlen(names) : (size_t)(q - names));

        if (namemap == 0)
            return NULL;
        id = ossl_namemap_name2num_n(namemap, methdata->names, l);
    }

    if (id == 0)
        return NULL;

    if (store == NULL
        && (store = get_encoder_store(methdata->libctx)) == NULL)
        return NULL;

    if (!ossl_method_store_fetch(store, id, methdata->propquery, prov, &method))
        return NULL;
    return method;
}

static int put_encoder_in_store(void *store, void *method,
                                const OSSL_PROVIDER *prov,
                                const char *names, const char *propdef,
                                void *data)
{
    struct encoder_data_st *methdata = data;
    OSSL_NAMEMAP *namemap;
    int id;
    size_t l = 0;

    /*
     * put_encoder_in_store() is only called with an OSSL_ENCODER method that
     * was successfully created by construct_encoder() below, which means that
     * all the names should already be stored in the namemap with the same
     * numeric identity, so just use the first to get that identity.
     */
    if (names != NULL) {
        const char *q = strchr(names, NAME_SEPARATOR);

        l = (q == NULL ? strlen(names) : (size_t)(q - names));
    }

    if ((namemap = ossl_namemap_stored(methdata->libctx)) == NULL
        || (id = ossl_namemap_name2num_n(namemap, names, l)) == 0)
        return 0;

    if (store == NULL && (store = get_encoder_store(methdata->libctx)) == NULL)
        return 0;

    return ossl_method_store_add(store, prov, id, propdef, method,
                                 ossl_encoder_up_ref,
                                 ossl_encoder_free);
}

/* Create and populate a encoder method */
static void *encoder_from_algorithm(int id, const OSSL_ALGORITHM *algodef,
                                    OSSL_PROVIDER *prov)
{
    OSSL_ENCODER *encoder = NULL;
    const OSSL_DISPATCH *fns = algodef->implementation;
    OSSL_LIB_CTX *libctx = ossl_provider_libctx(prov);

    if ((encoder = ossl_encoder_new()) == NULL)
        return NULL;
    encoder->base.id = id;
    if ((encoder->base.name = ossl_algorithm_get1_first_name(algodef)) == NULL) {
        OSSL_ENCODER_free(encoder);
        return NULL;
    }
    encoder->base.algodef = algodef;
    if ((encoder->base.parsed_propdef
         = ossl_parse_property(libctx, algodef->property_definition)) == NULL) {
        OSSL_ENCODER_free(encoder);
        return NULL;
    }

    for (; fns->function_id != 0; fns++) {
        switch (fns->function_id) {
        case OSSL_FUNC_ENCODER_NEWCTX:
            if (encoder->newctx == NULL)
                encoder->newctx =
                    OSSL_FUNC_encoder_newctx(fns);
            break;
        case OSSL_FUNC_ENCODER_FREECTX:
            if (encoder->freectx == NULL)
                encoder->freectx =
                    OSSL_FUNC_encoder_freectx(fns);
            break;
        case OSSL_FUNC_ENCODER_GET_PARAMS:
            if (encoder->get_params == NULL)
                encoder->get_params =
                    OSSL_FUNC_encoder_get_params(fns);
            break;
        case OSSL_FUNC_ENCODER_GETTABLE_PARAMS:
            if (encoder->gettable_params == NULL)
                encoder->gettable_params =
                    OSSL_FUNC_encoder_gettable_params(fns);
            break;
        case OSSL_FUNC_ENCODER_SET_CTX_PARAMS:
            if (encoder->set_ctx_params == NULL)
                encoder->set_ctx_params =
                    OSSL_FUNC_encoder_set_ctx_params(fns);
            break;
        case OSSL_FUNC_ENCODER_SETTABLE_CTX_PARAMS:
            if (encoder->settable_ctx_params == NULL)
                encoder->settable_ctx_params =
                    OSSL_FUNC_encoder_settable_ctx_params(fns);
            break;
        case OSSL_FUNC_ENCODER_DOES_SELECTION:
            if (encoder->does_selection == NULL)
                encoder->does_selection =
                    OSSL_FUNC_encoder_does_selection(fns);
            break;
        case OSSL_FUNC_ENCODER_ENCODE:
            if (encoder->encode == NULL)
                encoder->encode = OSSL_FUNC_encoder_encode(fns);
            break;
        case OSSL_FUNC_ENCODER_IMPORT_OBJECT:
            if (encoder->import_object == NULL)
                encoder->import_object =
                    OSSL_FUNC_encoder_import_object(fns);
            break;
        case OSSL_FUNC_ENCODER_FREE_OBJECT:
            if (encoder->free_object == NULL)
                encoder->free_object =
                    OSSL_FUNC_encoder_free_object(fns);
            break;
        }
    }
    /*
     * Try to check that the method is sensible.
     * If you have a constructor, you must have a destructor and vice versa.
     * You must have the encoding driver functions.
     */
    if (!((encoder->newctx == NULL && encoder->freectx == NULL)
          || (encoder->newctx != NULL && encoder->freectx != NULL)
          || (encoder->import_object != NULL && encoder->free_object != NULL)
          || (encoder->import_object == NULL && encoder->free_object == NULL))
        || encoder->encode == NULL) {
        OSSL_ENCODER_free(encoder);
        ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_INVALID_PROVIDER_FUNCTIONS);
        return NULL;
    }

    if (prov != NULL && !ossl_provider_up_ref(prov)) {
        OSSL_ENCODER_free(encoder);
        return NULL;
    }

    encoder->base.prov = prov;
    return encoder;
}


/*
 * The core fetching functionality passes the names of the implementation.
 * This function is responsible to getting an identity number for them,
 * then call encoder_from_algorithm() with that identity number.
 */
static void *construct_encoder(const OSSL_ALGORITHM *algodef,
                               OSSL_PROVIDER *prov, void *data)
{
    /*
     * This function is only called if get_encoder_from_store() returned
     * NULL, so it's safe to say that of all the spots to create a new
     * namemap entry, this is it.  Should the name already exist there, we
     * know that ossl_namemap_add() will return its corresponding number.
     */
    struct encoder_data_st *methdata = data;
    OSSL_LIB_CTX *libctx = ossl_provider_libctx(prov);
    OSSL_NAMEMAP *namemap = ossl_namemap_stored(libctx);
    const char *names = algodef->algorithm_names;
    int id = ossl_namemap_add_names(namemap, 0, names, NAME_SEPARATOR);
    void *method = NULL;

    if (id != 0)
        method = encoder_from_algorithm(id, algodef, prov);

    /*
     * Flag to indicate that there was actual construction errors.  This
     * helps inner_evp_generic_fetch() determine what error it should
     * record on inaccessible algorithms.
     */
    if (method == NULL)
        methdata->flag_construct_error_occurred = 1;

    return method;
}

/* Intermediary function to avoid ugly casts, used below */
static void destruct_encoder(void *method, void *data)
{
    OSSL_ENCODER_free(method);
}

static int up_ref_encoder(void *method)
{
    return OSSL_ENCODER_up_ref(method);
}

static void free_encoder(void *method)
{
    OSSL_ENCODER_free(method);
}

/* Fetching support.  Can fetch by numeric identity or by name */
static OSSL_ENCODER *
inner_ossl_encoder_fetch(struct encoder_data_st *methdata,
                         const char *name, const char *properties)
{
    OSSL_METHOD_STORE *store = get_encoder_store(methdata->libctx);
    OSSL_NAMEMAP *namemap = ossl_namemap_stored(methdata->libctx);
    const char *const propq = properties != NULL ? properties : "";
    void *method = NULL;
    int unsupported, id;

    if (store == NULL || namemap == NULL) {
        ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_PASSED_INVALID_ARGUMENT);
        return NULL;
    }

    id = name != NULL ? ossl_namemap_name2num(namemap, name) : 0;

    /*
     * If we haven't found the name yet, chances are that the algorithm to
     * be fetched is unsupported.
     */
    unsupported = id == 0;

    if (id == 0
        || !ossl_method_store_cache_get(store, NULL, id, propq, &method)) {
        OSSL_METHOD_CONSTRUCT_METHOD mcm = {
            get_tmp_encoder_store,
            reserve_encoder_store,
            unreserve_encoder_store,
            get_encoder_from_store,
            put_encoder_in_store,
            construct_encoder,
            destruct_encoder
        };
        OSSL_PROVIDER *prov = NULL;

        methdata->id = id;
        methdata->names = name;
        methdata->propquery = propq;
        methdata->flag_construct_error_occurred = 0;
        if ((method = ossl_method_construct(methdata->libctx, OSSL_OP_ENCODER,
                                            &prov, 0 /* !force_cache */,
                                            &mcm, methdata)) != NULL) {
            /*
             * If construction did create a method for us, we know that
             * there is a correct name_id and meth_id, since those have
             * already been calculated in get_encoder_from_store() and
             * put_encoder_in_store() above.
             */
            if (id == 0)
                id = ossl_namemap_name2num(namemap, name);
            ossl_method_store_cache_set(store, prov, id, propq, method,
                                        up_ref_encoder, free_encoder);
        }

        /*
         * If we never were in the constructor, the algorithm to be fetched
         * is unsupported.
         */
        unsupported = !methdata->flag_construct_error_occurred;
    }

    if ((id != 0 || name != NULL) && method == NULL) {
        int code = unsupported ? ERR_R_UNSUPPORTED : ERR_R_FETCH_FAILED;

        if (name == NULL)
            name = ossl_namemap_num2name(namemap, id, 0);
        ERR_raise_data(ERR_LIB_OSSL_ENCODER, code,
                       "%s, Name (%s : %d), Properties (%s)",
                       ossl_lib_ctx_get_descriptor(methdata->libctx),
                       name == NULL ? "<null>" : name, id,
                       properties == NULL ? "<null>" : properties);
    }

    return method;
}

OSSL_ENCODER *OSSL_ENCODER_fetch(OSSL_LIB_CTX *libctx, const char *name,
                                 const char *properties)
{
    struct encoder_data_st methdata;
    void *method;

    methdata.libctx = libctx;
    methdata.tmp_store = NULL;
    method = inner_ossl_encoder_fetch(&methdata, name, properties);
    dealloc_tmp_encoder_store(methdata.tmp_store);
    return method;
}

int ossl_encoder_store_cache_flush(OSSL_LIB_CTX *libctx)
{
    OSSL_METHOD_STORE *store = get_encoder_store(libctx);

    if (store != NULL)
        return ossl_method_store_cache_flush_all(store);
    return 1;
}

int ossl_encoder_store_remove_all_provided(const OSSL_PROVIDER *prov)
{
    OSSL_LIB_CTX *libctx = ossl_provider_libctx(prov);
    OSSL_METHOD_STORE *store = get_encoder_store(libctx);

    if (store != NULL)
        return ossl_method_store_remove_all_provided(store, prov);
    return 1;
}

/*
 * Library of basic method functions
 */

const OSSL_PROVIDER *OSSL_ENCODER_get0_provider(const OSSL_ENCODER *encoder)
{
    if (!ossl_assert(encoder != NULL)) {
        ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    return encoder->base.prov;
}

const char *OSSL_ENCODER_get0_properties(const OSSL_ENCODER *encoder)
{
    if (!ossl_assert(encoder != NULL)) {
        ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    return encoder->base.algodef->property_definition;
}

const OSSL_PROPERTY_LIST *
ossl_encoder_parsed_properties(const OSSL_ENCODER *encoder)
{
    if (!ossl_assert(encoder != NULL)) {
        ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    return encoder->base.parsed_propdef;
}

int ossl_encoder_get_number(const OSSL_ENCODER *encoder)
{
    if (!ossl_assert(encoder != NULL)) {
        ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    return encoder->base.id;
}

const char *OSSL_ENCODER_get0_name(const OSSL_ENCODER *encoder)
{
    return encoder->base.name;
}

const char *OSSL_ENCODER_get0_description(const OSSL_ENCODER *encoder)
{
    return encoder->base.algodef->algorithm_description;
}

int OSSL_ENCODER_is_a(const OSSL_ENCODER *encoder, const char *name)
{
    if (encoder->base.prov != NULL) {
        OSSL_LIB_CTX *libctx = ossl_provider_libctx(encoder->base.prov);
        OSSL_NAMEMAP *namemap = ossl_namemap_stored(libctx);

        return ossl_namemap_name2num(namemap, name) == encoder->base.id;
    }
    return 0;
}

struct do_one_data_st {
    void (*user_fn)(OSSL_ENCODER *encoder, void *arg);
    void *user_arg;
};

static void do_one(ossl_unused int id, void *method, void *arg)
{
    struct do_one_data_st *data = arg;

    data->user_fn(method, data->user_arg);
}

void OSSL_ENCODER_do_all_provided(OSSL_LIB_CTX *libctx,
                                  void (*user_fn)(OSSL_ENCODER *encoder,
                                                  void *arg),
                                  void *user_arg)
{
    struct encoder_data_st methdata;
    struct do_one_data_st data;

    methdata.libctx = libctx;
    methdata.tmp_store = NULL;
    (void)inner_ossl_encoder_fetch(&methdata, NULL, NULL /* properties */);

    data.user_fn = user_fn;
    data.user_arg = user_arg;
    if (methdata.tmp_store != NULL)
        ossl_method_store_do_all(methdata.tmp_store, &do_one, &data);
    ossl_method_store_do_all(get_encoder_store(libctx), &do_one, &data);
    dealloc_tmp_encoder_store(methdata.tmp_store);
}

int OSSL_ENCODER_names_do_all(const OSSL_ENCODER *encoder,
                              void (*fn)(const char *name, void *data),
                              void *data)
{
    if (encoder == NULL)
        return 0;

    if (encoder->base.prov != NULL) {
        OSSL_LIB_CTX *libctx = ossl_provider_libctx(encoder->base.prov);
        OSSL_NAMEMAP *namemap = ossl_namemap_stored(libctx);

        return ossl_namemap_doall_names(namemap, encoder->base.id, fn, data);
    }

    return 1;
}

const OSSL_PARAM *
OSSL_ENCODER_gettable_params(OSSL_ENCODER *encoder)
{
    if (encoder != NULL && encoder->gettable_params != NULL) {
        void *provctx = ossl_provider_ctx(OSSL_ENCODER_get0_provider(encoder));

        return encoder->gettable_params(provctx);
    }
    return NULL;
}

int OSSL_ENCODER_get_params(OSSL_ENCODER *encoder, OSSL_PARAM params[])
{
    if (encoder != NULL && encoder->get_params != NULL)
        return encoder->get_params(params);
    return 0;
}

const OSSL_PARAM *OSSL_ENCODER_settable_ctx_params(OSSL_ENCODER *encoder)
{
    if (encoder != NULL && encoder->settable_ctx_params != NULL) {
        void *provctx = ossl_provider_ctx(OSSL_ENCODER_get0_provider(encoder));

        return encoder->settable_ctx_params(provctx);
    }
    return NULL;
}

/*
 * Encoder context support
 */

OSSL_ENCODER_CTX *OSSL_ENCODER_CTX_new(void)
{
    OSSL_ENCODER_CTX *ctx;

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    return ctx;
}

int OSSL_ENCODER_CTX_set_params(OSSL_ENCODER_CTX *ctx,
                                const OSSL_PARAM params[])
{
    int ok = 1;
    size_t i;
    size_t l;

    if (!ossl_assert(ctx != NULL)) {
        ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if (ctx->encoder_insts == NULL)
        return 1;

    l = OSSL_ENCODER_CTX_get_num_encoders(ctx);
    for (i = 0; i < l; i++) {
        OSSL_ENCODER_INSTANCE *encoder_inst =
            sk_OSSL_ENCODER_INSTANCE_value(ctx->encoder_insts, i);
        OSSL_ENCODER *encoder = OSSL_ENCODER_INSTANCE_get_encoder(encoder_inst);
        void *encoderctx = OSSL_ENCODER_INSTANCE_get_encoder_ctx(encoder_inst);

        if (encoderctx == NULL || encoder->set_ctx_params == NULL)
            continue;
        if (!encoder->set_ctx_params(encoderctx, params))
            ok = 0;
    }
    return ok;
}

void OSSL_ENCODER_CTX_free(OSSL_ENCODER_CTX *ctx)
{
    if (ctx != NULL) {
        sk_OSSL_ENCODER_INSTANCE_pop_free(ctx->encoder_insts,
                                          ossl_encoder_instance_free);
        OPENSSL_free(ctx->construct_data);
        ossl_pw_clear_passphrase_data(&ctx->pwdata);
        OPENSSL_free(ctx);
    }
}
