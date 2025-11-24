/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Internal EVP utility functions */

#include <openssl/core.h>
#include <openssl/evp.h>
#include <openssl/err.h>
#include <openssl/asn1.h>        /* evp_local.h needs it */
#include <openssl/safestack.h>   /* evp_local.h needs it */
#include "crypto/evp.h"    /* evp_local.h needs it */
#include "evp_local.h"

/*
 * EVP_CTRL_RET_UNSUPPORTED = -1 is the returned value from any ctrl function
 * where the control command isn't supported, and an alternative code path
 * may be chosen.
 * Since these functions are used to implement ctrl functionality, we
 * use the same value, and other callers will have to compensate.
 */
#define PARAM_CHECK(obj, func, errfunc)                                        \
    if (obj == NULL)                                                           \
        return 0;                                                              \
    if (obj->prov == NULL)                                                     \
        return EVP_CTRL_RET_UNSUPPORTED;                                       \
    if (obj->func == NULL) {                                                   \
        errfunc();                                                             \
        return 0;                                                              \
    }

#define PARAM_FUNC(name, func, type, err)                                      \
int name (const type *obj, OSSL_PARAM params[])                                \
{                                                                              \
    PARAM_CHECK(obj, func, err)                                                \
    return obj->func(params);                                                  \
}

#define PARAM_CTX_FUNC(name, func, type, err)                                  \
int name (const type *obj, void *algctx, OSSL_PARAM params[])                  \
{                                                                              \
    PARAM_CHECK(obj, func, err)                                                \
    return obj->func(algctx, params);                                          \
}

#define PARAM_FUNCTIONS(type,                                                  \
                        getname, getfunc,                                      \
                        getctxname, getctxfunc,                                \
                        setctxname, setctxfunc)                                \
    PARAM_FUNC(getname, getfunc, type, geterr)                                 \
    PARAM_CTX_FUNC(getctxname, getctxfunc, type, geterr)                       \
    PARAM_CTX_FUNC(setctxname, setctxfunc, type, seterr)

/*
 * These error functions are a workaround for the error scripts, which
 * currently require that XXXerr method appears inside a function (not a macro).
 */
static void geterr(void)
{
    ERR_raise(ERR_LIB_EVP, EVP_R_CANNOT_GET_PARAMETERS);
}

static void seterr(void)
{
    ERR_raise(ERR_LIB_EVP, EVP_R_CANNOT_SET_PARAMETERS);
}

PARAM_FUNCTIONS(EVP_CIPHER,
                evp_do_ciph_getparams, get_params,
                evp_do_ciph_ctx_getparams, get_ctx_params,
                evp_do_ciph_ctx_setparams, set_ctx_params)

PARAM_FUNCTIONS(EVP_MD,
                evp_do_md_getparams, get_params,
                evp_do_md_ctx_getparams, get_ctx_params,
                evp_do_md_ctx_setparams, set_ctx_params)
