/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/crypto.h>
#include <openssl/core_dispatch.h>
#include <openssl/evp.h>
#include <openssl/err.h>
#include "internal/provider.h"
#include "internal/refcount.h"
#include "internal/core.h"
#include "crypto/evp.h"
#include "evp_local.h"

static void evp_keymgmt_free(void *data)
{
    EVP_KEYMGMT_free(data);
}

static int evp_keymgmt_up_ref(void *data)
{
    return EVP_KEYMGMT_up_ref(data);
}

static void *keymgmt_new(void)
{
    EVP_KEYMGMT *keymgmt = NULL;

    if ((keymgmt = OPENSSL_zalloc(sizeof(*keymgmt))) == NULL)
        return NULL;
    if (!CRYPTO_NEW_REF(&keymgmt->refcnt, 1)) {
        EVP_KEYMGMT_free(keymgmt);
        return NULL;
    }
    return keymgmt;
}

#ifndef FIPS_MODULE
static void help_get_legacy_alg_type_from_keymgmt(const char *keytype,
                                                  void *arg)
{
    int *type = arg;

    if (*type == NID_undef)
        *type = evp_pkey_name2type(keytype);
}

static int get_legacy_alg_type_from_keymgmt(const EVP_KEYMGMT *keymgmt)
{
    int type = NID_undef;

    EVP_KEYMGMT_names_do_all(keymgmt, help_get_legacy_alg_type_from_keymgmt,
                             &type);
    return type;
}
#endif

static void *keymgmt_from_algorithm(int name_id,
                                    const OSSL_ALGORITHM *algodef,
                                    OSSL_PROVIDER *prov)
{
    const OSSL_DISPATCH *fns = algodef->implementation;
    EVP_KEYMGMT *keymgmt = NULL;
    int setparamfncnt = 0, getparamfncnt = 0;
    int setgenparamfncnt = 0;
    int importfncnt = 0, exportfncnt = 0;
    int importtypesfncnt = 0, exporttypesfncnt = 0;
    int getgenparamfncnt = 0;

    if ((keymgmt = keymgmt_new()) == NULL)
        return NULL;

    keymgmt->name_id = name_id;
    if ((keymgmt->type_name = ossl_algorithm_get1_first_name(algodef)) == NULL) {
        EVP_KEYMGMT_free(keymgmt);
        return NULL;
    }
    keymgmt->description = algodef->algorithm_description;

    for (; fns->function_id != 0; fns++) {
        switch (fns->function_id) {
        case OSSL_FUNC_KEYMGMT_NEW:
            if (keymgmt->new == NULL)
                keymgmt->new = OSSL_FUNC_keymgmt_new(fns);
            break;
        case OSSL_FUNC_KEYMGMT_GEN_INIT:
            if (keymgmt->gen_init == NULL)
                keymgmt->gen_init = OSSL_FUNC_keymgmt_gen_init(fns);
            break;
        case OSSL_FUNC_KEYMGMT_GEN_SET_TEMPLATE:
            if (keymgmt->gen_set_template == NULL)
                keymgmt->gen_set_template =
                    OSSL_FUNC_keymgmt_gen_set_template(fns);
            break;
        case OSSL_FUNC_KEYMGMT_GEN_SET_PARAMS:
            if (keymgmt->gen_set_params == NULL) {
                setgenparamfncnt++;
                keymgmt->gen_set_params =
                    OSSL_FUNC_keymgmt_gen_set_params(fns);
            }
            break;
        case OSSL_FUNC_KEYMGMT_GEN_SETTABLE_PARAMS:
            if (keymgmt->gen_settable_params == NULL) {
                setgenparamfncnt++;
                keymgmt->gen_settable_params =
                    OSSL_FUNC_keymgmt_gen_settable_params(fns);
            }
            break;
        case OSSL_FUNC_KEYMGMT_GEN_GET_PARAMS:
            if (keymgmt->gen_get_params == NULL) {
                getgenparamfncnt++;
                keymgmt->gen_get_params =
                    OSSL_FUNC_keymgmt_gen_get_params(fns);
            }
            break;
        case OSSL_FUNC_KEYMGMT_GEN_GETTABLE_PARAMS:
            if (keymgmt->gen_gettable_params == NULL) {
                getgenparamfncnt++;
                keymgmt->gen_gettable_params =
                    OSSL_FUNC_keymgmt_gen_gettable_params(fns);
            }
            break;
        case OSSL_FUNC_KEYMGMT_GEN:
            if (keymgmt->gen == NULL)
                keymgmt->gen = OSSL_FUNC_keymgmt_gen(fns);
            break;
        case OSSL_FUNC_KEYMGMT_GEN_CLEANUP:
            if (keymgmt->gen_cleanup == NULL)
                keymgmt->gen_cleanup = OSSL_FUNC_keymgmt_gen_cleanup(fns);
            break;
        case OSSL_FUNC_KEYMGMT_FREE:
            if (keymgmt->free == NULL)
                keymgmt->free = OSSL_FUNC_keymgmt_free(fns);
            break;
        case OSSL_FUNC_KEYMGMT_LOAD:
            if (keymgmt->load == NULL)
                keymgmt->load = OSSL_FUNC_keymgmt_load(fns);
            break;
        case OSSL_FUNC_KEYMGMT_GET_PARAMS:
            if (keymgmt->get_params == NULL) {
                getparamfncnt++;
                keymgmt->get_params = OSSL_FUNC_keymgmt_get_params(fns);
            }
            break;
        case OSSL_FUNC_KEYMGMT_GETTABLE_PARAMS:
            if (keymgmt->gettable_params == NULL) {
                getparamfncnt++;
                keymgmt->gettable_params =
                    OSSL_FUNC_keymgmt_gettable_params(fns);
            }
            break;
         case OSSL_FUNC_KEYMGMT_SET_PARAMS:
            if (keymgmt->set_params == NULL) {
                setparamfncnt++;
                keymgmt->set_params = OSSL_FUNC_keymgmt_set_params(fns);
            }
            break;
        case OSSL_FUNC_KEYMGMT_SETTABLE_PARAMS:
            if (keymgmt->settable_params == NULL) {
                setparamfncnt++;
                keymgmt->settable_params =
                    OSSL_FUNC_keymgmt_settable_params(fns);
            }
            break;
        case OSSL_FUNC_KEYMGMT_QUERY_OPERATION_NAME:
            if (keymgmt->query_operation_name == NULL)
                keymgmt->query_operation_name =
                    OSSL_FUNC_keymgmt_query_operation_name(fns);
            break;
        case OSSL_FUNC_KEYMGMT_HAS:
            if (keymgmt->has == NULL)
                keymgmt->has = OSSL_FUNC_keymgmt_has(fns);
            break;
        case OSSL_FUNC_KEYMGMT_DUP:
            if (keymgmt->dup == NULL)
                keymgmt->dup = OSSL_FUNC_keymgmt_dup(fns);
            break;
        case OSSL_FUNC_KEYMGMT_VALIDATE:
            if (keymgmt->validate == NULL)
                keymgmt->validate = OSSL_FUNC_keymgmt_validate(fns);
            break;
        case OSSL_FUNC_KEYMGMT_MATCH:
            if (keymgmt->match == NULL)
                keymgmt->match = OSSL_FUNC_keymgmt_match(fns);
            break;
        case OSSL_FUNC_KEYMGMT_IMPORT:
            if (keymgmt->import == NULL) {
                importfncnt++;
                keymgmt->import = OSSL_FUNC_keymgmt_import(fns);
            }
            break;
        case OSSL_FUNC_KEYMGMT_IMPORT_TYPES:
            if (keymgmt->import_types == NULL) {
                if (importtypesfncnt == 0)
                    importfncnt++;
                importtypesfncnt++;
                keymgmt->import_types = OSSL_FUNC_keymgmt_import_types(fns);
            }
            break;
        case OSSL_FUNC_KEYMGMT_IMPORT_TYPES_EX:
            if (keymgmt->import_types_ex == NULL) {
                if (importtypesfncnt == 0)
                    importfncnt++;
                importtypesfncnt++;
                keymgmt->import_types_ex = OSSL_FUNC_keymgmt_import_types_ex(fns);
            }
            break;
        case OSSL_FUNC_KEYMGMT_EXPORT:
            if (keymgmt->export == NULL) {
                exportfncnt++;
                keymgmt->export = OSSL_FUNC_keymgmt_export(fns);
            }
            break;
        case OSSL_FUNC_KEYMGMT_EXPORT_TYPES:
            if (keymgmt->export_types == NULL) {
                if (exporttypesfncnt == 0)
                    exportfncnt++;
                exporttypesfncnt++;
                keymgmt->export_types = OSSL_FUNC_keymgmt_export_types(fns);
            }
            break;
        case OSSL_FUNC_KEYMGMT_EXPORT_TYPES_EX:
            if (keymgmt->export_types_ex == NULL) {
                if (exporttypesfncnt == 0)
                    exportfncnt++;
                exporttypesfncnt++;
                keymgmt->export_types_ex = OSSL_FUNC_keymgmt_export_types_ex(fns);
            }
            break;
        }
    }
    /*
     * Try to check that the method is sensible.
     * At least one constructor and the destructor are MANDATORY
     * The functions 'has' is MANDATORY
     * It makes no sense being able to free stuff if you can't create it.
     * It makes no sense providing OSSL_PARAM descriptors for import and
     * export if you can't import or export.
     */
    if (keymgmt->free == NULL
        || (keymgmt->new == NULL
            && keymgmt->gen == NULL
            && keymgmt->load == NULL)
        || keymgmt->has == NULL
        || (getparamfncnt != 0 && getparamfncnt != 2)
        || (setparamfncnt != 0 && setparamfncnt != 2)
        || (setgenparamfncnt != 0 && setgenparamfncnt != 2)
        || (getgenparamfncnt != 0 && getgenparamfncnt != 2)
        || (importfncnt != 0 && importfncnt != 2)
        || (exportfncnt != 0 && exportfncnt != 2)
        || (keymgmt->gen != NULL
            && (keymgmt->gen_init == NULL
                || keymgmt->gen_cleanup == NULL))) {
        EVP_KEYMGMT_free(keymgmt);
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_PROVIDER_FUNCTIONS);
        return NULL;
    }
    keymgmt->prov = prov;
    if (prov != NULL)
        ossl_provider_up_ref(prov);

#ifndef FIPS_MODULE
    keymgmt->legacy_alg = get_legacy_alg_type_from_keymgmt(keymgmt);
#endif

    return keymgmt;
}

EVP_KEYMGMT *evp_keymgmt_fetch_from_prov(OSSL_PROVIDER *prov,
                                         const char *name,
                                         const char *properties)
{
    return evp_generic_fetch_from_prov(prov, OSSL_OP_KEYMGMT,
                                       name, properties,
                                       keymgmt_from_algorithm,
                                       evp_keymgmt_up_ref,
                                       evp_keymgmt_free);
}

EVP_KEYMGMT *EVP_KEYMGMT_fetch(OSSL_LIB_CTX *ctx, const char *algorithm,
                               const char *properties)
{
    return evp_generic_fetch(ctx, OSSL_OP_KEYMGMT, algorithm, properties,
                             keymgmt_from_algorithm,
                             evp_keymgmt_up_ref,
                             evp_keymgmt_free);
}

int EVP_KEYMGMT_up_ref(EVP_KEYMGMT *keymgmt)
{
    int ref = 0;

    CRYPTO_UP_REF(&keymgmt->refcnt, &ref);
    return 1;
}

void EVP_KEYMGMT_free(EVP_KEYMGMT *keymgmt)
{
    int ref = 0;

    if (keymgmt == NULL)
        return;

    CRYPTO_DOWN_REF(&keymgmt->refcnt, &ref);
    if (ref > 0)
        return;
    OPENSSL_free(keymgmt->type_name);
    ossl_provider_free(keymgmt->prov);
    CRYPTO_FREE_REF(&keymgmt->refcnt);
    OPENSSL_free(keymgmt);
}

const OSSL_PROVIDER *EVP_KEYMGMT_get0_provider(const EVP_KEYMGMT *keymgmt)
{
    return keymgmt->prov;
}

int evp_keymgmt_get_number(const EVP_KEYMGMT *keymgmt)
{
    return keymgmt->name_id;
}

int evp_keymgmt_get_legacy_alg(const EVP_KEYMGMT *keymgmt)
{
    return keymgmt->legacy_alg;
}

const char *EVP_KEYMGMT_get0_description(const EVP_KEYMGMT *keymgmt)
{
    return keymgmt->description;
}

const char *EVP_KEYMGMT_get0_name(const EVP_KEYMGMT *keymgmt)
{
    return keymgmt->type_name;
}

int EVP_KEYMGMT_is_a(const EVP_KEYMGMT *keymgmt, const char *name)
{
    return keymgmt != NULL
           && evp_is_a(keymgmt->prov, keymgmt->name_id, NULL, name);
}

void EVP_KEYMGMT_do_all_provided(OSSL_LIB_CTX *libctx,
                                 void (*fn)(EVP_KEYMGMT *keymgmt, void *arg),
                                 void *arg)
{
    evp_generic_do_all(libctx, OSSL_OP_KEYMGMT,
                       (void (*)(void *, void *))fn, arg,
                       keymgmt_from_algorithm,
                       evp_keymgmt_up_ref,
                       evp_keymgmt_free);
}

int EVP_KEYMGMT_names_do_all(const EVP_KEYMGMT *keymgmt,
                             void (*fn)(const char *name, void *data),
                             void *data)
{
    if (keymgmt->prov != NULL)
        return evp_names_do_all(keymgmt->prov, keymgmt->name_id, fn, data);

    return 1;
}

/*
 * Internal API that interfaces with the method function pointers
 */
void *evp_keymgmt_newdata(const EVP_KEYMGMT *keymgmt)
{
    void *provctx = ossl_provider_ctx(EVP_KEYMGMT_get0_provider(keymgmt));

    /*
     * 'new' is currently mandatory on its own, but when new
     * constructors appear, it won't be quite as mandatory,
     * so we have a check for future cases.
     */
    if (keymgmt->new == NULL)
        return NULL;
    return keymgmt->new(provctx);
}

void evp_keymgmt_freedata(const EVP_KEYMGMT *keymgmt, void *keydata)
{
    /* This is mandatory, no need to check for its presence */
    keymgmt->free(keydata);
}

void *evp_keymgmt_gen_init(const EVP_KEYMGMT *keymgmt, int selection,
                           const OSSL_PARAM params[])
{
    void *provctx = ossl_provider_ctx(EVP_KEYMGMT_get0_provider(keymgmt));

    if (keymgmt->gen_init == NULL)
        return NULL;
    return keymgmt->gen_init(provctx, selection, params);
}

int evp_keymgmt_gen_set_template(const EVP_KEYMGMT *keymgmt, void *genctx,
                                 void *templ)
{
    /*
     * It's arguable if we actually should return success in this case, as
     * it allows the caller to set a template key, which is then ignored.
     * However, this is how the legacy methods (EVP_PKEY_METHOD) operate,
     * so we do this in the interest of backward compatibility.
     */
    if (keymgmt->gen_set_template == NULL)
        return 1;
    return keymgmt->gen_set_template(genctx, templ);
}

int evp_keymgmt_gen_set_params(const EVP_KEYMGMT *keymgmt, void *genctx,
                               const OSSL_PARAM params[])
{
    if (keymgmt->gen_set_params == NULL)
        return 0;
    return keymgmt->gen_set_params(genctx, params);
}

const OSSL_PARAM *EVP_KEYMGMT_gen_settable_params(const EVP_KEYMGMT *keymgmt)
{
    void *provctx = ossl_provider_ctx(EVP_KEYMGMT_get0_provider(keymgmt));

    if (keymgmt->gen_settable_params == NULL)
        return NULL;
    return keymgmt->gen_settable_params(NULL, provctx);
}

int evp_keymgmt_gen_get_params(const EVP_KEYMGMT *keymgmt, void *genctx,
                               OSSL_PARAM params[])
{
    if (keymgmt->gen_get_params == NULL)
        return 0;
    return keymgmt->gen_get_params(genctx, params);
}

const OSSL_PARAM *EVP_KEYMGMT_gen_gettable_params(const EVP_KEYMGMT *keymgmt)
{
    void *provctx = ossl_provider_ctx(EVP_KEYMGMT_get0_provider(keymgmt));

    if (keymgmt->gen_gettable_params == NULL)
        return NULL;
    return keymgmt->gen_gettable_params(NULL, provctx);
}

void *evp_keymgmt_gen(const EVP_KEYMGMT *keymgmt, void *genctx,
                      OSSL_CALLBACK *cb, void *cbarg)
{
    void *ret;
    const char *desc = keymgmt->description != NULL ? keymgmt->description : "";

    if (keymgmt->gen == NULL) {
        ERR_raise_data(ERR_LIB_EVP, EVP_R_PROVIDER_KEYMGMT_NOT_SUPPORTED,
                       "%s key generation:%s", keymgmt->type_name, desc);
        return NULL;
    }

    ret = keymgmt->gen(genctx, cb, cbarg);
    if (ret == NULL)
        ERR_raise_data(ERR_LIB_EVP, EVP_R_PROVIDER_KEYMGMT_FAILURE,
                       "%s key generation:%s", keymgmt->type_name, desc);
    return ret;
}

void evp_keymgmt_gen_cleanup(const EVP_KEYMGMT *keymgmt, void *genctx)
{
    if (keymgmt->gen_cleanup != NULL)
        keymgmt->gen_cleanup(genctx);
}

int evp_keymgmt_has_load(const EVP_KEYMGMT *keymgmt)
{
    return keymgmt != NULL && keymgmt->load != NULL;
}

void *evp_keymgmt_load(const EVP_KEYMGMT *keymgmt,
                       const void *objref, size_t objref_sz)
{
    if (evp_keymgmt_has_load(keymgmt))
        return keymgmt->load(objref, objref_sz);
    return NULL;
}

int evp_keymgmt_get_params(const EVP_KEYMGMT *keymgmt, void *keydata,
                           OSSL_PARAM params[])
{
    if (keymgmt->get_params == NULL)
        return 1;
    return keymgmt->get_params(keydata, params);
}

const OSSL_PARAM *EVP_KEYMGMT_gettable_params(const EVP_KEYMGMT *keymgmt)
{
    void *provctx = ossl_provider_ctx(EVP_KEYMGMT_get0_provider(keymgmt));

    if (keymgmt->gettable_params == NULL)
        return NULL;
    return keymgmt->gettable_params(provctx);
}

int evp_keymgmt_set_params(const EVP_KEYMGMT *keymgmt, void *keydata,
                           const OSSL_PARAM params[])
{
    if (keymgmt->set_params == NULL)
        return 1;
    return keymgmt->set_params(keydata, params);
}

const OSSL_PARAM *EVP_KEYMGMT_settable_params(const EVP_KEYMGMT *keymgmt)
{
    void *provctx = ossl_provider_ctx(EVP_KEYMGMT_get0_provider(keymgmt));

    if (keymgmt->settable_params == NULL)
        return NULL;
    return keymgmt->settable_params(provctx);
}

int evp_keymgmt_has(const EVP_KEYMGMT *keymgmt, void *keydata, int selection)
{
    /* This is mandatory, no need to check for its presence */
    return keymgmt->has(keydata, selection);
}

int evp_keymgmt_validate(const EVP_KEYMGMT *keymgmt, void *keydata,
                         int selection, int checktype)
{
    /* We assume valid if the implementation doesn't have a function */
    if (keymgmt->validate == NULL)
        return 1;
    return keymgmt->validate(keydata, selection, checktype);
}

int evp_keymgmt_match(const EVP_KEYMGMT *keymgmt,
                      const void *keydata1, const void *keydata2,
                      int selection)
{
    /* We assume no match if the implementation doesn't have a function */
    if (keymgmt->match == NULL)
        return 0;
    return keymgmt->match(keydata1, keydata2, selection);
}

int evp_keymgmt_import(const EVP_KEYMGMT *keymgmt, void *keydata,
                       int selection, const OSSL_PARAM params[])
{
    if (keymgmt->import == NULL)
        return 0;
    return keymgmt->import(keydata, selection, params);
}

const OSSL_PARAM *evp_keymgmt_import_types(const EVP_KEYMGMT *keymgmt,
                                           int selection)
{
    void *provctx = ossl_provider_ctx(EVP_KEYMGMT_get0_provider(keymgmt));

    if (keymgmt->import_types_ex != NULL)
        return keymgmt->import_types_ex(provctx, selection);
    if (keymgmt->import_types == NULL)
        return NULL;
    return keymgmt->import_types(selection);
}

int evp_keymgmt_export(const EVP_KEYMGMT *keymgmt, void *keydata,
                       int selection, OSSL_CALLBACK *param_cb, void *cbarg)
{
    if (keymgmt->export == NULL)
        return 0;
    return keymgmt->export(keydata, selection, param_cb, cbarg);
}

const OSSL_PARAM *evp_keymgmt_export_types(const EVP_KEYMGMT *keymgmt,
                                           int selection)
{
    void *provctx = ossl_provider_ctx(EVP_KEYMGMT_get0_provider(keymgmt));

    if (keymgmt->export_types_ex != NULL)
        return keymgmt->export_types_ex(provctx, selection);
    if (keymgmt->export_types == NULL)
        return NULL;
    return keymgmt->export_types(selection);
}

void *evp_keymgmt_dup(const EVP_KEYMGMT *keymgmt, const void *keydata_from,
                      int selection)
{
    /* We assume no dup if the implementation doesn't have a function */
    if (keymgmt->dup == NULL)
        return NULL;
    return keymgmt->dup(keydata_from, selection);
}
