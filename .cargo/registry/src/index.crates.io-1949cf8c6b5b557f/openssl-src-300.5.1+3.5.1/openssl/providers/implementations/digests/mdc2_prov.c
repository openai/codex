/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * MDC2 low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <openssl/crypto.h>
#include <openssl/params.h>
#include <openssl/mdc2.h>
#include <openssl/core_names.h>
#include <openssl/err.h>
#include <openssl/proverr.h>
#include "prov/digestcommon.h"
#include "prov/implementations.h"

static OSSL_FUNC_digest_set_ctx_params_fn mdc2_set_ctx_params;
static OSSL_FUNC_digest_settable_ctx_params_fn mdc2_settable_ctx_params;

static const OSSL_PARAM known_mdc2_settable_ctx_params[] = {
    OSSL_PARAM_uint(OSSL_DIGEST_PARAM_PAD_TYPE, NULL),
    OSSL_PARAM_END
};

static const OSSL_PARAM *mdc2_settable_ctx_params(ossl_unused void *ctx,
                                                  ossl_unused void *provctx)
{
    return known_mdc2_settable_ctx_params;
}

static int mdc2_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    const OSSL_PARAM *p;
    MDC2_CTX *ctx = (MDC2_CTX *)vctx;

    if (ctx == NULL)
        return 0;
    if (ossl_param_is_empty(params))
        return 1;

    p = OSSL_PARAM_locate_const(params, OSSL_DIGEST_PARAM_PAD_TYPE);
    if (p != NULL && !OSSL_PARAM_get_uint(p, &ctx->pad_type)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
        return 0;
    }
    return 1;
}

/* ossl_mdc2_functions */
IMPLEMENT_digest_functions_with_settable_ctx(
    mdc2, MDC2_CTX, MDC2_BLOCK, MDC2_DIGEST_LENGTH, 0,
    MDC2_Init, MDC2_Update, MDC2_Final,
    mdc2_settable_ctx_params, mdc2_set_ctx_params)
