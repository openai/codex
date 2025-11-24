/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core.h>
#include <openssl/core_dispatch.h>
#include <openssl/decoder.h>
#include <openssl/ui.h>
#include "internal/core.h"
#include "internal/namemap.h"
#include "internal/property.h"
#include "internal/provider.h"
#include "crypto/decoder.h"
#include "encoder_local.h"
#include "crypto/context.h"

/*
 * Decoder can have multiple names, separated with colons in a name string
 */
#define NAME_SEPARATOR ':'

static void ossl_decoder_free(void *data)
{
    OSSL_DECODER_free(data);
}

static int ossl_decoder_up_ref(void *data)
{
    return OSSL_DECODER_up_ref(data);
}

/* Simple method structure constructor and destructor */
static OSSL_DECODER *ossl_decoder_new(void)
{
    OSSL_DECODER *decoder = NULL;

    if ((decoder = OPENSSL_zalloc(sizeof(*decoder))) == NULL)
        return NULL;
    if (!CRYPTO_NEW_REF(&decoder->base.refcnt, 1)) {
        OSSL_DECODER_free(decoder);
        return NULL;
    }

    return decoder;
}

int OSSL_DECODER_up_ref(OSSL_DECODER *decoder)
{
    int ref = 0;

    CRYPTO_UP_REF(&decoder->base.refcnt, &ref);
    return 1;
}

void OSSL_DECODER_free(OSSL_DECODER *decoder)
{
    int ref = 0;

    if (decoder == NULL)
        return;

    CRYPTO_DOWN_REF(&decoder->base.refcnt, &ref);
    if (ref > 0)
        return;
    OPENSSL_free(decoder->base.name);
    ossl_property_free(decoder->base.parsed_propdef);
    ossl_provider_free(decoder->base.prov);
    CRYPTO_FREE_REF(&decoder->base.refcnt);
    OPENSSL_free(decoder);
}

/* Data to be passed through ossl_method_construct() */
struct decoder_data_st {
    OSSL_LIB_CTX *libctx;
    int id;                      /* For get_decoder_from_store() */
    const char *names;           /* For get_decoder_from_store() */
    const char *propquery;       /* For get_decoder_from_store() */

    OSSL_METHOD_STORE *tmp_store; /* For get_tmp_decoder_store() */

    unsigned int flag_construct_error_occurred : 1;
};

/*
 * Generic routines to fetch / create DECODER methods with
 * ossl_method_construct()
 */

/* Temporary decoder method store, constructor and destructor */
static void *get_tmp_decoder_store(void *data)
{
    struct decoder_data_st *methdata = data;

    if (methdata->tmp_store == NULL)
        methdata->tmp_store = ossl_method_store_new(methdata->libctx);
    return methdata->tmp_store;
}

static void dealloc_tmp_decoder_store(void *store)
{
    if (store != NULL)
        ossl_method_store_free(store);
}

/* Get the permanent decoder store */
static OSSL_METHOD_STORE *get_decoder_store(OSSL_LIB_CTX *libctx)
{
    return ossl_lib_ctx_get_data(libctx, OSSL_LIB_CTX_DECODER_STORE_INDEX);
}

static int reserve_decoder_store(void *store, void *data)
{
    struct decoder_data_st *methdata = data;

    if (store == NULL
        && (store = get_decoder_store(methdata->libctx)) == NULL)
        return 0;

    return ossl_method_lock_store(store);
}

static int unreserve_decoder_store(void *store, void *data)
{
    struct decoder_data_st *methdata = data;

    if (store == NULL
        && (store = get_decoder_store(methdata->libctx)) == NULL)
        return 0;

    return ossl_method_unlock_store(store);
}

/* Get decoder methods from a store, or put one in */
static void *get_decoder_from_store(void *store, const OSSL_PROVIDER **prov,
                                    void *data)
{
    struct decoder_data_st *methdata = data;
    void *method = NULL;
    int id;

    /*
     * get_decoder_from_store() is only called to try and get the method
     * that OSSL_DECODER_fetch() is asking for, and the name or name id are
     * passed via methdata.
     */
    if ((id = methdata->id) == 0 && methdata->names != NULL) {
        OSSL_NAMEMAP *namemap = ossl_namemap_stored(methdata->libctx);
        const char *names = methdata->names;
        const char *q = strchr(names, NAME_SEPARATOR);
        size_t l = (q == NULL ? strlen(names) : (size_t)(q - names));

        if (namemap == 0)
            return NULL;
        id = ossl_namemap_name2num_n(namemap, names, l);
    }

    if (id == 0)
        return NULL;

    if (store == NULL
        && (store = get_decoder_store(methdata->libctx)) == NULL)
        return NULL;

    if (!ossl_method_store_fetch(store, id, methdata->propquery, prov, &method))
        return NULL;
    return method;
}

static int put_decoder_in_store(void *store, void *method,
                                const OSSL_PROVIDER *prov,
                                const char *names, const char *propdef,
                                void *data)
{
    struct decoder_data_st *methdata = data;
    OSSL_NAMEMAP *namemap;
    int id;
    size_t l = 0;

    /*
     * put_decoder_in_store() is only called with an OSSL_DECODER method that
     * was successfully created by construct_decoder() below, which means that
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

    if (store == NULL && (store = get_decoder_store(methdata->libctx)) == NULL)
        return 0;

    return ossl_method_store_add(store, prov, id, propdef, method,
                                 ossl_decoder_up_ref,
                                 ossl_decoder_free);
}

/* Create and populate a decoder method */
void *ossl_decoder_from_algorithm(int id, const OSSL_ALGORITHM *algodef,
                                  OSSL_PROVIDER *prov)
{
    OSSL_DECODER *decoder = NULL;
    const OSSL_DISPATCH *fns = algodef->implementation;
    OSSL_LIB_CTX *libctx = ossl_provider_libctx(prov);

    if ((decoder = ossl_decoder_new()) == NULL)
        return NULL;
    decoder->base.id = id;
    if ((decoder->base.name = ossl_algorithm_get1_first_name(algodef)) == NULL) {
        OSSL_DECODER_free(decoder);
        return NULL;
    }
    decoder->base.algodef = algodef;
    if ((decoder->base.parsed_propdef
         = ossl_parse_property(libctx, algodef->property_definition)) == NULL) {
        OSSL_DECODER_free(decoder);
        return NULL;
    }

    for (; fns->function_id != 0; fns++) {
        switch (fns->function_id) {
        case OSSL_FUNC_DECODER_NEWCTX:
            if (decoder->newctx == NULL)
                decoder->newctx = OSSL_FUNC_decoder_newctx(fns);
            break;
        case OSSL_FUNC_DECODER_FREECTX:
            if (decoder->freectx == NULL)
                decoder->freectx = OSSL_FUNC_decoder_freectx(fns);
            break;
        case OSSL_FUNC_DECODER_GET_PARAMS:
            if (decoder->get_params == NULL)
                decoder->get_params =
                    OSSL_FUNC_decoder_get_params(fns);
            break;
        case OSSL_FUNC_DECODER_GETTABLE_PARAMS:
            if (decoder->gettable_params == NULL)
                decoder->gettable_params =
                    OSSL_FUNC_decoder_gettable_params(fns);
            break;
        case OSSL_FUNC_DECODER_SET_CTX_PARAMS:
            if (decoder->set_ctx_params == NULL)
                decoder->set_ctx_params =
                    OSSL_FUNC_decoder_set_ctx_params(fns);
            break;
        case OSSL_FUNC_DECODER_SETTABLE_CTX_PARAMS:
            if (decoder->settable_ctx_params == NULL)
                decoder->settable_ctx_params =
                    OSSL_FUNC_decoder_settable_ctx_params(fns);
            break;
        case OSSL_FUNC_DECODER_DOES_SELECTION:
            if (decoder->does_selection == NULL)
                decoder->does_selection =
                    OSSL_FUNC_decoder_does_selection(fns);
            break;
        case OSSL_FUNC_DECODER_DECODE:
            if (decoder->decode == NULL)
                decoder->decode = OSSL_FUNC_decoder_decode(fns);
            break;
        case OSSL_FUNC_DECODER_EXPORT_OBJECT:
            if (decoder->export_object == NULL)
                decoder->export_object = OSSL_FUNC_decoder_export_object(fns);
            break;
        }
    }
    /*
     * Try to check that the method is sensible.
     * If you have a constructor, you must have a destructor and vice versa.
     * You must have at least one of the encoding driver functions.
     */
    if (!((decoder->newctx == NULL && decoder->freectx == NULL)
          || (decoder->newctx != NULL && decoder->freectx != NULL))
        || decoder->decode == NULL) {
        OSSL_DECODER_free(decoder);
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_INVALID_PROVIDER_FUNCTIONS);
        return NULL;
    }

    if (prov != NULL && !ossl_provider_up_ref(prov)) {
        OSSL_DECODER_free(decoder);
        return NULL;
    }

    decoder->base.prov = prov;
    return decoder;
}


/*
 * The core fetching functionality passes the names of the implementation.
 * This function is responsible to getting an identity number for them,
 * then call ossl_decoder_from_algorithm() with that identity number.
 */
static void *construct_decoder(const OSSL_ALGORITHM *algodef,
                               OSSL_PROVIDER *prov, void *data)
{
    /*
     * This function is only called if get_decoder_from_store() returned
     * NULL, so it's safe to say that of all the spots to create a new
     * namemap entry, this is it.  Should the name already exist there, we
     * know that ossl_namemap_add() will return its corresponding number.
     */
    struct decoder_data_st *methdata = data;
    OSSL_LIB_CTX *libctx = ossl_provider_libctx(prov);
    OSSL_NAMEMAP *namemap = ossl_namemap_stored(libctx);
    const char *names = algodef->algorithm_names;
    int id = ossl_namemap_add_names(namemap, 0, names, NAME_SEPARATOR);
    void *method = NULL;

    if (id != 0)
        method = ossl_decoder_from_algorithm(id, algodef, prov);

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
static void destruct_decoder(void *method, void *data)
{
    OSSL_DECODER_free(method);
}

static int up_ref_decoder(void *method)
{
    return OSSL_DECODER_up_ref(method);
}

static void free_decoder(void *method)
{
    OSSL_DECODER_free(method);
}

/* Fetching support.  Can fetch by numeric identity or by name */
static OSSL_DECODER *
inner_ossl_decoder_fetch(struct decoder_data_st *methdata,
                         const char *name, const char *properties)
{
    OSSL_METHOD_STORE *store = get_decoder_store(methdata->libctx);
    OSSL_NAMEMAP *namemap = ossl_namemap_stored(methdata->libctx);
    const char *const propq = properties != NULL ? properties : "";
    void *method = NULL;
    int unsupported, id;

    if (store == NULL || namemap == NULL) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_PASSED_INVALID_ARGUMENT);
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
            get_tmp_decoder_store,
            reserve_decoder_store,
            unreserve_decoder_store,
            get_decoder_from_store,
            put_decoder_in_store,
            construct_decoder,
            destruct_decoder
        };
        OSSL_PROVIDER *prov = NULL;

        methdata->id = id;
        methdata->names = name;
        methdata->propquery = propq;
        methdata->flag_construct_error_occurred = 0;
        if ((method = ossl_method_construct(methdata->libctx, OSSL_OP_DECODER,
                                            &prov, 0 /* !force_cache */,
                                            &mcm, methdata)) != NULL) {
            /*
             * If construction did create a method for us, we know that
             * there is a correct name_id and meth_id, since those have
             * already been calculated in get_decoder_from_store() and
             * put_decoder_in_store() above.
             */
            if (id == 0 && name != NULL)
                id = ossl_namemap_name2num(namemap, name);
            if (id != 0)
                ossl_method_store_cache_set(store, prov, id, propq, method,
                                            up_ref_decoder, free_decoder);
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
        ERR_raise_data(ERR_LIB_OSSL_DECODER, code,
                       "%s, Name (%s : %d), Properties (%s)",
                       ossl_lib_ctx_get_descriptor(methdata->libctx),
                       name == NULL ? "<null>" : name, id,
                       properties == NULL ? "<null>" : properties);
    }

    return method;
}

OSSL_DECODER *OSSL_DECODER_fetch(OSSL_LIB_CTX *libctx, const char *name,
                                 const char *properties)
{
    struct decoder_data_st methdata;
    void *method;

    methdata.libctx = libctx;
    methdata.tmp_store = NULL;
    method = inner_ossl_decoder_fetch(&methdata, name, properties);
    dealloc_tmp_decoder_store(methdata.tmp_store);
    return method;
}

int ossl_decoder_store_cache_flush(OSSL_LIB_CTX *libctx)
{
    OSSL_METHOD_STORE *store = get_decoder_store(libctx);

    if (store != NULL)
        return ossl_method_store_cache_flush_all(store);
    return 1;
}

int ossl_decoder_store_remove_all_provided(const OSSL_PROVIDER *prov)
{
    OSSL_LIB_CTX *libctx = ossl_provider_libctx(prov);
    OSSL_METHOD_STORE *store = get_decoder_store(libctx);

    if (store != NULL)
        return ossl_method_store_remove_all_provided(store, prov);
    return 1;
}

/*
 * Library of basic method functions
 */

const OSSL_PROVIDER *OSSL_DECODER_get0_provider(const OSSL_DECODER *decoder)
{
    if (!ossl_assert(decoder != NULL)) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    return decoder->base.prov;
}

const char *OSSL_DECODER_get0_properties(const OSSL_DECODER *decoder)
{
    if (!ossl_assert(decoder != NULL)) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    return decoder->base.algodef->property_definition;
}

const OSSL_PROPERTY_LIST *
ossl_decoder_parsed_properties(const OSSL_DECODER *decoder)
{
    if (!ossl_assert(decoder != NULL)) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    return decoder->base.parsed_propdef;
}

int ossl_decoder_get_number(const OSSL_DECODER *decoder)
{
    if (!ossl_assert(decoder != NULL)) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    return decoder->base.id;
}

const char *OSSL_DECODER_get0_name(const OSSL_DECODER *decoder)
{
    return decoder->base.name;
}

const char *OSSL_DECODER_get0_description(const OSSL_DECODER *decoder)
{
    return decoder->base.algodef->algorithm_description;
}

int OSSL_DECODER_is_a(const OSSL_DECODER *decoder, const char *name)
{
    if (decoder->base.prov != NULL) {
        OSSL_LIB_CTX *libctx = ossl_provider_libctx(decoder->base.prov);
        OSSL_NAMEMAP *namemap = ossl_namemap_stored(libctx);

        return ossl_namemap_name2num(namemap, name) == decoder->base.id;
    }
    return 0;
}

static int resolve_name(OSSL_DECODER *decoder, const char *name)
{
    OSSL_LIB_CTX *libctx = ossl_provider_libctx(decoder->base.prov);
    OSSL_NAMEMAP *namemap = ossl_namemap_stored(libctx);

    return ossl_namemap_name2num(namemap, name);
}

int ossl_decoder_fast_is_a(OSSL_DECODER *decoder, const char *name, int *id_cache)
{
    int id = *id_cache;

    if (id <= 0)
        *id_cache = id = resolve_name(decoder, name);

    return id > 0 && ossl_decoder_get_number(decoder) == id;
}

struct do_one_data_st {
    void (*user_fn)(OSSL_DECODER *decoder, void *arg);
    void *user_arg;
};

static void do_one(ossl_unused int id, void *method, void *arg)
{
    struct do_one_data_st *data = arg;

    data->user_fn(method, data->user_arg);
}

void OSSL_DECODER_do_all_provided(OSSL_LIB_CTX *libctx,
                                  void (*user_fn)(OSSL_DECODER *decoder,
                                                  void *arg),
                                  void *user_arg)
{
    struct decoder_data_st methdata;
    struct do_one_data_st data;

    methdata.libctx = libctx;
    methdata.tmp_store = NULL;
    (void)inner_ossl_decoder_fetch(&methdata, NULL, NULL /* properties */);

    data.user_fn = user_fn;
    data.user_arg = user_arg;
    if (methdata.tmp_store != NULL)
        ossl_method_store_do_all(methdata.tmp_store, &do_one, &data);
    ossl_method_store_do_all(get_decoder_store(libctx), &do_one, &data);
    dealloc_tmp_decoder_store(methdata.tmp_store);
}

int OSSL_DECODER_names_do_all(const OSSL_DECODER *decoder,
                              void (*fn)(const char *name, void *data),
                              void *data)
{
    if (decoder == NULL)
        return 0;

    if (decoder->base.prov != NULL) {
        OSSL_LIB_CTX *libctx = ossl_provider_libctx(decoder->base.prov);
        OSSL_NAMEMAP *namemap = ossl_namemap_stored(libctx);

        return ossl_namemap_doall_names(namemap, decoder->base.id, fn, data);
    }

    return 1;
}

const OSSL_PARAM *
OSSL_DECODER_gettable_params(OSSL_DECODER *decoder)
{
    if (decoder != NULL && decoder->gettable_params != NULL) {
        void *provctx = ossl_provider_ctx(OSSL_DECODER_get0_provider(decoder));

        return decoder->gettable_params(provctx);
    }
    return NULL;
}

int OSSL_DECODER_get_params(OSSL_DECODER *decoder, OSSL_PARAM params[])
{
    if (decoder != NULL && decoder->get_params != NULL)
        return decoder->get_params(params);
    return 0;
}

const OSSL_PARAM *
OSSL_DECODER_settable_ctx_params(OSSL_DECODER *decoder)
{
    if (decoder != NULL && decoder->settable_ctx_params != NULL) {
        void *provctx = ossl_provider_ctx(OSSL_DECODER_get0_provider(decoder));

        return decoder->settable_ctx_params(provctx);
    }
    return NULL;
}

/*
 * Decoder context support
 */

/*
 * |encoder| value NULL is valid, and signifies that there is no decoder.
 * This is useful to provide fallback mechanisms.
 *  Functions that want to verify if there is a decoder can do so with
 * OSSL_DECODER_CTX_get_decoder()
 */
OSSL_DECODER_CTX *OSSL_DECODER_CTX_new(void)
{
    OSSL_DECODER_CTX *ctx;

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    return ctx;
}

int OSSL_DECODER_CTX_set_params(OSSL_DECODER_CTX *ctx,
                                const OSSL_PARAM params[])
{
    int ok = 1;
    size_t i;
    size_t l;

    if (!ossl_assert(ctx != NULL)) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if (ctx->decoder_insts == NULL)
        return 1;

    l = OSSL_DECODER_CTX_get_num_decoders(ctx);
    for (i = 0; i < l; i++) {
        OSSL_DECODER_INSTANCE *decoder_inst =
            sk_OSSL_DECODER_INSTANCE_value(ctx->decoder_insts, i);
        OSSL_DECODER *decoder =
            OSSL_DECODER_INSTANCE_get_decoder(decoder_inst);
        OSSL_DECODER *decoderctx =
            OSSL_DECODER_INSTANCE_get_decoder_ctx(decoder_inst);

        if (decoderctx == NULL || decoder->set_ctx_params == NULL)
            continue;
        if (!decoder->set_ctx_params(decoderctx, params))
            ok = 0;
    }
    return ok;
}

void OSSL_DECODER_CTX_free(OSSL_DECODER_CTX *ctx)
{
    if (ctx != NULL) {
        if (ctx->cleanup != NULL)
            ctx->cleanup(ctx->construct_data);
        sk_OSSL_DECODER_INSTANCE_pop_free(ctx->decoder_insts,
                                          ossl_decoder_instance_free);
        ossl_pw_clear_passphrase_data(&ctx->pwdata);
        OPENSSL_free(ctx);
    }
}
