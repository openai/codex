/*
 * Copyright 2020-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core.h>
#include <openssl/core_dispatch.h>
#include <openssl/types.h>
#include "prov/provider_ctx.h"

OSSL_FUNC_keymgmt_new_fn *ossl_prov_get_keymgmt_new(const OSSL_DISPATCH *fns);
OSSL_FUNC_keymgmt_free_fn *ossl_prov_get_keymgmt_free(const OSSL_DISPATCH *fns);
OSSL_FUNC_keymgmt_import_fn *ossl_prov_get_keymgmt_import(const OSSL_DISPATCH *fns);
OSSL_FUNC_keymgmt_export_fn *ossl_prov_get_keymgmt_export(const OSSL_DISPATCH *fns);

int ossl_prov_der_from_p8(unsigned char **new_der, long *new_der_len,
                          unsigned char *input_der, long input_der_len,
                          OSSL_PASSPHRASE_CALLBACK *pw_cb, void *pw_cbarg);

void *ossl_prov_import_key(const OSSL_DISPATCH *fns, void *provctx,
                           int selection, const OSSL_PARAM params[]);
void ossl_prov_free_key(const OSSL_DISPATCH *fns, void *key);
int ossl_read_der(PROV_CTX *provctx, OSSL_CORE_BIO *cin,  unsigned char **data,
                  long *len);
