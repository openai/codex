/*
 * Copyright 2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core_dispatch.h>
#include "crypto/types.h"
#include "skeymgmt_lcl.h"
#include "internal/skey.h"
#include "prov/implementations.h"

static OSSL_FUNC_skeymgmt_import_fn aes_import;
static OSSL_FUNC_skeymgmt_export_fn aes_export;

static void *aes_import(void *provctx, int selection,
                        const OSSL_PARAM params[])
{
    PROV_SKEY *aes = generic_import(provctx, selection, params);

    if (aes == NULL)
        return NULL;

    if (aes->length != 16 && aes->length != 24 && aes->length != 32) {
        generic_free(aes);
        return NULL;
    }
    aes->type = SKEY_TYPE_AES;

    return aes;
}

static int aes_export(void *keydata, int selection,
                      OSSL_CALLBACK *param_callback, void *cbarg)
{
    PROV_SKEY *aes = keydata;

    if (aes->type != SKEY_TYPE_AES)
        return 0;

    return generic_export(keydata, selection, param_callback, cbarg);
}

const OSSL_DISPATCH ossl_aes_skeymgmt_functions[] = {
    { OSSL_FUNC_SKEYMGMT_FREE, (void (*)(void))generic_free },
    { OSSL_FUNC_SKEYMGMT_IMPORT, (void (*)(void))aes_import },
    { OSSL_FUNC_SKEYMGMT_EXPORT, (void (*)(void))aes_export },
    OSSL_DISPATCH_END
};
