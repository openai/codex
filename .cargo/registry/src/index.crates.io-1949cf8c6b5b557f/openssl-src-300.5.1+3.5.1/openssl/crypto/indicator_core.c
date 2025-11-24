/*
 * Copyright 2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/indicator.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include "internal/cryptlib.h"
#include "crypto/context.h"

typedef struct indicator_cb_st
{
    OSSL_INDICATOR_CALLBACK *cb;
} INDICATOR_CB;

void *ossl_indicator_set_callback_new(OSSL_LIB_CTX *ctx)
{
    INDICATOR_CB *cb;

    cb = OPENSSL_zalloc(sizeof(*cb));
    return cb;
}

void ossl_indicator_set_callback_free(void *cb)
{
    OPENSSL_free(cb);
}

static INDICATOR_CB *get_indicator_callback(OSSL_LIB_CTX *libctx)
{
    return ossl_lib_ctx_get_data(libctx, OSSL_LIB_CTX_INDICATOR_CB_INDEX);
}

void OSSL_INDICATOR_set_callback(OSSL_LIB_CTX *libctx,
                                 OSSL_INDICATOR_CALLBACK *cb)
{
    INDICATOR_CB *icb = get_indicator_callback(libctx);

    if (icb != NULL)
        icb->cb = cb;
}

void OSSL_INDICATOR_get_callback(OSSL_LIB_CTX *libctx,
                                 OSSL_INDICATOR_CALLBACK **cb)
{
    INDICATOR_CB *icb = get_indicator_callback(libctx);

    if (cb != NULL)
        *cb = (icb != NULL ? icb->cb : NULL);
}
