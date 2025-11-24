/*
 * Copyright 2020-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core.h>
#include <openssl/buffer.h>
#include "internal/asn1.h"
#include "prov/bio.h"
#include "endecoder_local.h"

OSSL_FUNC_keymgmt_new_fn *
ossl_prov_get_keymgmt_new(const OSSL_DISPATCH *fns)
{
    /* Pilfer the keymgmt dispatch table */
    for (; fns->function_id != 0; fns++)
        if (fns->function_id == OSSL_FUNC_KEYMGMT_NEW)
            return OSSL_FUNC_keymgmt_new(fns);

    return NULL;
}

OSSL_FUNC_keymgmt_free_fn *
ossl_prov_get_keymgmt_free(const OSSL_DISPATCH *fns)
{
    /* Pilfer the keymgmt dispatch table */
    for (; fns->function_id != 0; fns++)
        if (fns->function_id == OSSL_FUNC_KEYMGMT_FREE)
            return OSSL_FUNC_keymgmt_free(fns);

    return NULL;
}

OSSL_FUNC_keymgmt_import_fn *
ossl_prov_get_keymgmt_import(const OSSL_DISPATCH *fns)
{
    /* Pilfer the keymgmt dispatch table */
    for (; fns->function_id != 0; fns++)
        if (fns->function_id == OSSL_FUNC_KEYMGMT_IMPORT)
            return OSSL_FUNC_keymgmt_import(fns);

    return NULL;
}

OSSL_FUNC_keymgmt_export_fn *
ossl_prov_get_keymgmt_export(const OSSL_DISPATCH *fns)
{
    /* Pilfer the keymgmt dispatch table */
    for (; fns->function_id != 0; fns++)
        if (fns->function_id == OSSL_FUNC_KEYMGMT_EXPORT)
            return OSSL_FUNC_keymgmt_export(fns);

    return NULL;
}

void *ossl_prov_import_key(const OSSL_DISPATCH *fns, void *provctx,
                           int selection, const OSSL_PARAM params[])
{
    OSSL_FUNC_keymgmt_new_fn *kmgmt_new = ossl_prov_get_keymgmt_new(fns);
    OSSL_FUNC_keymgmt_free_fn *kmgmt_free = ossl_prov_get_keymgmt_free(fns);
    OSSL_FUNC_keymgmt_import_fn *kmgmt_import =
        ossl_prov_get_keymgmt_import(fns);
    void *key = NULL;

    if (kmgmt_new != NULL && kmgmt_import != NULL && kmgmt_free != NULL) {
        if ((key = kmgmt_new(provctx)) == NULL
            || !kmgmt_import(key, selection, params)) {
            kmgmt_free(key);
            key = NULL;
        }
    }
    return key;
}

void ossl_prov_free_key(const OSSL_DISPATCH *fns, void *key)
{
    OSSL_FUNC_keymgmt_free_fn *kmgmt_free = ossl_prov_get_keymgmt_free(fns);

    if (kmgmt_free != NULL)
        kmgmt_free(key);
}

int ossl_read_der(PROV_CTX *provctx, OSSL_CORE_BIO *cin,  unsigned char **data,
                  long *len)
{
    BUF_MEM *mem = NULL;
    BIO *in = ossl_bio_new_from_core_bio(provctx, cin);
    int ok;

    if (in == NULL)
        return 0;
    ok = (asn1_d2i_read_bio(in, &mem) >= 0);
    if (ok) {
        *data = (unsigned char *)mem->data;
        *len = (long)mem->length;
        OPENSSL_free(mem);
    }
    BIO_free(in);
    return ok;
}
