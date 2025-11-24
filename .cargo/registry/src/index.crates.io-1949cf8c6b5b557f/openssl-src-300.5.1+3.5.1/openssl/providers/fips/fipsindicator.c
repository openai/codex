/*
 * Copyright 2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/indicator.h>
#include <openssl/params.h>
#include <openssl/core_names.h>
#include "internal/common.h" /* for ossl_assert() */
#include "fips/fipsindicator.h"

void ossl_FIPS_IND_init(OSSL_FIPS_IND *ind)
{
    int i;

    ossl_FIPS_IND_set_approved(ind); /* Assume we are approved by default */
    for (i = 0; i < OSSL_FIPS_IND_SETTABLE_MAX; i++)
        ind->settable[i] = OSSL_FIPS_IND_STATE_UNKNOWN;
}

void ossl_FIPS_IND_set_approved(OSSL_FIPS_IND *ind)
{
    ind->approved = 1;
}

void ossl_FIPS_IND_copy(OSSL_FIPS_IND *dst, const OSSL_FIPS_IND *src)
{
    *dst = *src;
}

void ossl_FIPS_IND_set_settable(OSSL_FIPS_IND *ind, int id, int state)
{
    if (!ossl_assert(id < OSSL_FIPS_IND_SETTABLE_MAX))
        return;
    if (!ossl_assert(state == OSSL_FIPS_IND_STATE_STRICT
                     || state == OSSL_FIPS_IND_STATE_TOLERANT))
        return;
    ind->settable[id] = state;
}

int ossl_FIPS_IND_get_settable(const OSSL_FIPS_IND *ind, int id)
{
    if (!ossl_assert(id < OSSL_FIPS_IND_SETTABLE_MAX))
        return OSSL_FIPS_IND_STATE_UNKNOWN;
    return ind->settable[id];
}

/*
 * This should only be called when a strict FIPS algorithm check fails.
 * It assumes that we are in strict mode by default.
 * If the logic here is not sufficient for all cases, then additional
 * ossl_FIPS_IND_on_unapproved() functions may be required.
 */
int ossl_FIPS_IND_on_unapproved(OSSL_FIPS_IND *ind, int id,
                                OSSL_LIB_CTX *libctx,
                                const char *algname, const char *opname,
                                OSSL_FIPS_IND_CHECK_CB *config_check_fn)
{
    /* Set to unapproved. Once unapproved mode is set this will not be reset */
    ind->approved = 0;

    /*
     * We only trigger the indicator callback if the ctx variable is cleared OR
     * the configurable item is cleared. If the values are unknown they are
     * assumed to be strict.
     */
    if (ossl_FIPS_IND_get_settable(ind, id) == OSSL_FIPS_IND_STATE_TOLERANT
        || (config_check_fn != NULL
            && config_check_fn(libctx) == OSSL_FIPS_IND_STATE_TOLERANT)) {
        return ossl_FIPS_IND_callback(libctx, algname, opname);
    }
    /* Strict mode gets here: This returns an error */
    return 0;
}

int ossl_FIPS_IND_set_ctx_param(OSSL_FIPS_IND *ind, int id,
                                const OSSL_PARAM params[], const char *name)
{
    int in = 0;
    const OSSL_PARAM *p = OSSL_PARAM_locate_const(params, name);

    if (p != NULL) {
        if (!OSSL_PARAM_get_int(p, &in))
            return 0;
        ossl_FIPS_IND_set_settable(ind, id, in);
    }
    return 1;
}

int ossl_FIPS_IND_get_ctx_param(const OSSL_FIPS_IND *ind, OSSL_PARAM params[])
{
    OSSL_PARAM *p = OSSL_PARAM_locate(params, OSSL_ALG_PARAM_FIPS_APPROVED_INDICATOR);

    return p == NULL || OSSL_PARAM_set_int(p, ind->approved);
}

/*
 * Can be used during application testing to log that an indicator was
 * triggered. The callback will return 1 if the application wants an error
 * to occur based on the indicator type and description.
 */
int ossl_FIPS_IND_callback(OSSL_LIB_CTX *libctx, const char *type,
                           const char *desc)
{
    OSSL_INDICATOR_CALLBACK *cb = NULL;

    OSSL_INDICATOR_get_callback(libctx, &cb);
    if (cb == NULL)
        return 1;

    return cb(type, desc, NULL);
}
