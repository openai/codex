/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <stdio.h>
#include <openssl/opensslconf.h>
#include <openssl/core.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include "prov/bio.h"
#include "prov/provider_ctx.h"
#include "prov/providercommon.h"
#include "prov/implementations.h"
#include "prov/provider_util.h"
#include "prov/names.h"

/*
 * Forward declarations to ensure that interface functions are correctly
 * defined.
 */
static OSSL_FUNC_provider_gettable_params_fn base_gettable_params;
static OSSL_FUNC_provider_get_params_fn base_get_params;
static OSSL_FUNC_provider_query_operation_fn base_query;

/* Functions provided by the core */
static OSSL_FUNC_core_gettable_params_fn *c_gettable_params = NULL;
static OSSL_FUNC_core_get_params_fn *c_get_params = NULL;

/* Parameters we provide to the core */
static const OSSL_PARAM base_param_types[] = {
    OSSL_PARAM_DEFN(OSSL_PROV_PARAM_NAME, OSSL_PARAM_UTF8_PTR, NULL, 0),
    OSSL_PARAM_DEFN(OSSL_PROV_PARAM_VERSION, OSSL_PARAM_UTF8_PTR, NULL, 0),
    OSSL_PARAM_DEFN(OSSL_PROV_PARAM_BUILDINFO, OSSL_PARAM_UTF8_PTR, NULL, 0),
    OSSL_PARAM_DEFN(OSSL_PROV_PARAM_STATUS, OSSL_PARAM_INTEGER, NULL, 0),
    OSSL_PARAM_END
};

static const OSSL_PARAM *base_gettable_params(void *provctx)
{
    return base_param_types;
}

static int base_get_params(void *provctx, OSSL_PARAM params[])
{
    OSSL_PARAM *p;

    p = OSSL_PARAM_locate(params, OSSL_PROV_PARAM_NAME);
    if (p != NULL
            && !OSSL_PARAM_set_utf8_ptr(p, "OpenSSL Base Provider"))
        return 0;
    p = OSSL_PARAM_locate(params, OSSL_PROV_PARAM_VERSION);
    if (p != NULL && !OSSL_PARAM_set_utf8_ptr(p, OPENSSL_VERSION_STR))
        return 0;
    p = OSSL_PARAM_locate(params, OSSL_PROV_PARAM_BUILDINFO);
    if (p != NULL && !OSSL_PARAM_set_utf8_ptr(p, OPENSSL_FULL_VERSION_STR))
        return 0;
    p = OSSL_PARAM_locate(params, OSSL_PROV_PARAM_STATUS);
    if (p != NULL && !OSSL_PARAM_set_int(p, ossl_prov_is_running()))
        return 0;

    return 1;
}

static const OSSL_ALGORITHM base_encoder[] = {
#define ENCODER_PROVIDER "base"
#include "encoders.inc"
    { NULL, NULL, NULL }
#undef ENCODER_PROVIDER
};

static const OSSL_ALGORITHM base_decoder[] = {
#define DECODER_PROVIDER "base"
#include "decoders.inc"
    { NULL, NULL, NULL }
#undef DECODER_PROVIDER
};

static const OSSL_ALGORITHM base_store[] = {
#define STORE(name, _fips, func_table)                           \
    { name, "provider=base,fips=" _fips, (func_table) },

#include "stores.inc"
    { NULL, NULL, NULL }
#undef STORE
};

static const OSSL_ALGORITHM base_rands[] = {
    { PROV_NAMES_SEED_SRC, "provider=base", ossl_seed_src_functions },
#ifndef OPENSSL_NO_JITTER
    { PROV_NAMES_JITTER, "provider=base", ossl_jitter_functions },
#endif
    { NULL, NULL, NULL }
};

static const OSSL_ALGORITHM *base_query(void *provctx, int operation_id,
                                         int *no_cache)
{
    *no_cache = 0;
    switch (operation_id) {
    case OSSL_OP_ENCODER:
        return base_encoder;
    case OSSL_OP_DECODER:
        return base_decoder;
    case OSSL_OP_STORE:
        return base_store;
    case OSSL_OP_RAND:
        return base_rands;
    }
    return NULL;
}

static void base_teardown(void *provctx)
{
    BIO_meth_free(ossl_prov_ctx_get0_core_bio_method(provctx));
    ossl_prov_ctx_free(provctx);
}

/* Functions we provide to the core */
static const OSSL_DISPATCH base_dispatch_table[] = {
    { OSSL_FUNC_PROVIDER_TEARDOWN, (void (*)(void))base_teardown },
    { OSSL_FUNC_PROVIDER_GETTABLE_PARAMS,
      (void (*)(void))base_gettable_params },
    { OSSL_FUNC_PROVIDER_GET_PARAMS, (void (*)(void))base_get_params },
    { OSSL_FUNC_PROVIDER_QUERY_OPERATION, (void (*)(void))base_query },
    OSSL_DISPATCH_END
};

OSSL_provider_init_fn ossl_base_provider_init;

int ossl_base_provider_init(const OSSL_CORE_HANDLE *handle,
                            const OSSL_DISPATCH *in, const OSSL_DISPATCH **out,
                            void **provctx)
{
    OSSL_FUNC_core_get_libctx_fn *c_get_libctx = NULL;
    BIO_METHOD *corebiometh;

    if (!ossl_prov_bio_from_dispatch(in))
        return 0;
    for (; in->function_id != 0; in++) {
        switch (in->function_id) {
        case OSSL_FUNC_CORE_GETTABLE_PARAMS:
            c_gettable_params = OSSL_FUNC_core_gettable_params(in);
            break;
        case OSSL_FUNC_CORE_GET_PARAMS:
            c_get_params = OSSL_FUNC_core_get_params(in);
            break;
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
        return 0;
    }
    ossl_prov_ctx_set0_libctx(*provctx,
                                       (OSSL_LIB_CTX *)c_get_libctx(handle));
    ossl_prov_ctx_set0_handle(*provctx, handle);
    ossl_prov_ctx_set0_core_bio_method(*provctx, corebiometh);
    ossl_prov_ctx_set0_core_get_params(*provctx, c_get_params);

    *out = base_dispatch_table;

    return 1;
}
