/*
 * Copyright 2023-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifdef FIPS_MODULE

# include <openssl/core.h> /* OSSL_CALLBACK, OSSL_LIB_CTX */
# include <openssl/indicator.h>
# include "crypto/types.h"
# include <openssl/ec.h>
# include "fipscommon.h"

/*
 * There may be multiple settables associated with an algorithm that allow
 * overriding the default status.
 * We associate an id with each of these.
 */
# define OSSL_FIPS_IND_SETTABLE0 0
# define OSSL_FIPS_IND_SETTABLE1 1
# define OSSL_FIPS_IND_SETTABLE2 2
# define OSSL_FIPS_IND_SETTABLE3 3
# define OSSL_FIPS_IND_SETTABLE4 4
# define OSSL_FIPS_IND_SETTABLE5 5
# define OSSL_FIPS_IND_SETTABLE6 6
# define OSSL_FIPS_IND_SETTABLE7 7
# define OSSL_FIPS_IND_SETTABLE_MAX (1 + OSSL_FIPS_IND_SETTABLE7)

/* Each settable is in one of 3 states */
#define OSSL_FIPS_IND_STATE_UNKNOWN    -1  /* Initial unknown state */
#define OSSL_FIPS_IND_STATE_STRICT      1  /* Strict enforcement */
#define OSSL_FIPS_IND_STATE_TOLERANT    0  /* Relaxation of rules */

/*
 * For each algorithm context there may be multiple checks that determine if
 * the algorithm is approved or not. These checks may be in different stages.
 * To keep it simple it is assumed that the algorithm is initially approved,
 * and may be unapproved when each check happens. Once unapproved the operation
 * will remain unapproved (otherwise we need to maintain state for each check).
 * The approved state should only be queried after the operation has completed
 * e.g. A digest final, or a KDF derive.
 *
 * If a FIPS approved check fails then we must decide what to do in this case.
 * In strict mode we would just return an error.
 * To override strict mode we either need to have a settable variable or have a
 * fips config flag that overrides strict mode.
 * If there are multiple checks, each one could possible have a different
 * configurable item. Each configurable item can be overridden by a different
 * settable.
 */
typedef struct ossl_fips_ind_st {
    unsigned char approved;
    signed char settable[OSSL_FIPS_IND_SETTABLE_MAX]; /* See OSSL_FIPS_IND_STATE */
} OSSL_FIPS_IND;

typedef int (OSSL_FIPS_IND_CHECK_CB)(OSSL_LIB_CTX *libctx);

int ossl_FIPS_IND_callback(OSSL_LIB_CTX *libctx, const char *type,
                           const char *desc);

void ossl_FIPS_IND_init(OSSL_FIPS_IND *ind);
void ossl_FIPS_IND_set_approved(OSSL_FIPS_IND *ind);
void ossl_FIPS_IND_set_settable(OSSL_FIPS_IND *ind, int id, int enable);
int ossl_FIPS_IND_get_settable(const OSSL_FIPS_IND *ind, int id);
int ossl_FIPS_IND_on_unapproved(OSSL_FIPS_IND *ind, int id, OSSL_LIB_CTX *libctx,
                                const char *algname, const char *opname,
                                OSSL_FIPS_IND_CHECK_CB *config_check_fn);
int ossl_FIPS_IND_set_ctx_param(OSSL_FIPS_IND *ind, int id,
                                const OSSL_PARAM params[], const char *name);
int ossl_FIPS_IND_get_ctx_param(const OSSL_FIPS_IND *ind,
                                      OSSL_PARAM params[]);
void ossl_FIPS_IND_copy(OSSL_FIPS_IND *dst, const OSSL_FIPS_IND *src);

/* Place this in the algorithm ctx structure */
# define OSSL_FIPS_IND_DECLARE OSSL_FIPS_IND indicator;
/* Call this to initialize the indicator */
# define OSSL_FIPS_IND_INIT(ctx) ossl_FIPS_IND_init(&ctx->indicator);
/*
 * Use the copy if an algorithm has a dup function that does not copy the src to
 * the dst.
 */
# define OSSL_FIPS_IND_COPY(dst, src) ossl_FIPS_IND_copy(&dst->indicator, &src->indicator);

/*
 * Required for reset - since once something becomes unapproved it will remain
 * unapproved unless this is used. This should be used in the init before
 * params are set into the ctx & before any FIPS checks are done.
 */
# define OSSL_FIPS_IND_SET_APPROVED(ctx) ossl_FIPS_IND_set_approved(&ctx->indicator);
/*
 * This should be called if a FIPS check fails, to indicate the operation is not approved
 * If there is more than 1 strict check flag per algorithm ctx, the id represents
 * the index.
 */
# define OSSL_FIPS_IND_ON_UNAPPROVED(ctx, id, libctx, algname, opname, config_check_fn) \
    ossl_FIPS_IND_on_unapproved(&ctx->indicator, id, libctx, algname, opname, config_check_fn)

# define OSSL_FIPS_IND_SETTABLE_CTX_PARAM(name) \
    OSSL_PARAM_int(name, NULL),

/*
 * The id here must match the one used by OSSL_FIPS_IND_ON_UNAPPROVED
 * The name must match the param used by OSSL_FIPS_IND_SETTABLE_CTX_PARAM
 */
# define OSSL_FIPS_IND_SET_CTX_PARAM(ctx, id, params, name) \
    ossl_FIPS_IND_set_ctx_param(&((ctx)->indicator), id, params, name)

# define OSSL_FIPS_IND_GETTABLE_CTX_PARAM() \
    OSSL_PARAM_int(OSSL_ALG_PARAM_FIPS_APPROVED_INDICATOR, NULL),

# define OSSL_FIPS_IND_GET_CTX_PARAM(ctx, prms) \
    ossl_FIPS_IND_get_ctx_param(&((ctx)->indicator), prms)

# define OSSL_FIPS_IND_GET(ctx) (&((ctx)->indicator))

# define OSSL_FIPS_IND_GET_PARAM(ctx, p, settable, id, name)                   \
    *settable = ossl_FIPS_IND_get_settable(&((ctx)->indicator), id);           \
    if (*settable != OSSL_FIPS_IND_STATE_UNKNOWN)                              \
        *p = OSSL_PARAM_construct_int(name, settable);

int ossl_fips_ind_rsa_key_check(OSSL_FIPS_IND *ind, int id, OSSL_LIB_CTX *libctx,
                                const RSA *rsa, const char *desc, int protect);
# ifndef OPENSSL_NO_EC
int ossl_fips_ind_ec_key_check(OSSL_FIPS_IND *ind, int id, OSSL_LIB_CTX *libctx,
                               const EC_GROUP *group, const char *desc,
                               int protect);
# endif
int ossl_fips_ind_digest_exch_check(OSSL_FIPS_IND *ind, int id, OSSL_LIB_CTX *libctx,
                                    const EVP_MD *md, const char *desc);
int ossl_fips_ind_digest_sign_check(OSSL_FIPS_IND *ind, int id,
                                    OSSL_LIB_CTX *libctx,
                                    int nid, int sha1_allowed,
                                    const char *desc,
                                    OSSL_FIPS_IND_CHECK_CB *config_check_f);

#else
# define OSSL_FIPS_IND_DECLARE
# define OSSL_FIPS_IND_INIT(ctx)
# define OSSL_FIPS_IND_SET_APPROVED(ctx)
# define OSSL_FIPS_IND_ON_UNAPPROVED(ctx, id, libctx, algname, opname, configopt_fn)
# define OSSL_FIPS_IND_SETTABLE_CTX_PARAM(name)
# define OSSL_FIPS_IND_SET_CTX_PARAM(ctx, id, params, name) 1
# define OSSL_FIPS_IND_GETTABLE_CTX_PARAM()
# define OSSL_FIPS_IND_GET_CTX_PARAM(ctx, params) 1
# define OSSL_FIPS_IND_COPY(dst, src)

#endif
