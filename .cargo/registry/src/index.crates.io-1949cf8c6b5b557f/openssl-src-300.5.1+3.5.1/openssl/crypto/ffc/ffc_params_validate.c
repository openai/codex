/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Finite Field cryptography (FFC) is used for DSA and DH.
 * This file contains methods for validation of FFC parameters.
 * It calls the same functions as the generation as the code is very similar.
 */

#include <openssl/err.h>
#include <openssl/bn.h>
#include <openssl/dsaerr.h>
#include <openssl/dherr.h>
#include "internal/ffc.h"

/* FIPS186-4 A.2.2 Unverifiable partial validation of Generator g */
int ossl_ffc_params_validate_unverifiable_g(BN_CTX *ctx, BN_MONT_CTX *mont,
                                            const BIGNUM *p, const BIGNUM *q,
                                            const BIGNUM *g, BIGNUM *tmp,
                                            int *ret)
{
    /*
     * A.2.2 Step (1) AND
     * A.2.4 Step (2)
     * Verify that 2 <= g <= (p - 1)
     */
    if (BN_cmp(g, BN_value_one()) <= 0 || BN_cmp(g, p) >= 0) {
        *ret |= FFC_ERROR_NOT_SUITABLE_GENERATOR;
        return 0;
    }

    /*
     * A.2.2 Step (2) AND
     * A.2.4 Step (3)
     * Check g^q mod p = 1
     */
    if (!BN_mod_exp_mont(tmp, g, q, p, ctx, mont))
        return 0;
    if (BN_cmp(tmp, BN_value_one()) != 0) {
        *ret |= FFC_ERROR_NOT_SUITABLE_GENERATOR;
        return 0;
    }
    return 1;
}

int ossl_ffc_params_FIPS186_4_validate(OSSL_LIB_CTX *libctx,
                                       const FFC_PARAMS *params, int type,
                                       int *res, BN_GENCB *cb)
{
    size_t L, N;

    if (params == NULL || params->p == NULL || params->q == NULL)
        return FFC_PARAM_RET_STATUS_FAILED;

    /* A.1.1.3 Step (1..2) : L = len(p), N = len(q) */
    L = BN_num_bits(params->p);
    N = BN_num_bits(params->q);
    return ossl_ffc_params_FIPS186_4_gen_verify(libctx, (FFC_PARAMS *)params,
                                                FFC_PARAM_MODE_VERIFY, type,
                                                L, N, res, cb);
}

/* This may be used in FIPS mode to validate deprecated FIPS-186-2 Params */
int ossl_ffc_params_FIPS186_2_validate(OSSL_LIB_CTX *libctx,
                                       const FFC_PARAMS *params, int type,
                                       int *res, BN_GENCB *cb)
{
    size_t L, N;

    if (params == NULL || params->p == NULL || params->q == NULL) {
        *res = FFC_CHECK_INVALID_PQ;
        return FFC_PARAM_RET_STATUS_FAILED;
    }

    /* A.1.1.3 Step (1..2) : L = len(p), N = len(q) */
    L = BN_num_bits(params->p);
    N = BN_num_bits(params->q);
    return ossl_ffc_params_FIPS186_2_gen_verify(libctx, (FFC_PARAMS *)params,
                                                FFC_PARAM_MODE_VERIFY, type,
                                                L, N, res, cb);
}

/*
 * This does a simple check of L and N and partial g.
 * It makes no attempt to do a full validation of p, q or g since these require
 * extra parameters such as the digest and seed, which may not be available for
 * this test.
 */
int ossl_ffc_params_simple_validate(OSSL_LIB_CTX *libctx, const FFC_PARAMS *params,
                                    int paramstype, int *res)
{
    int ret;
    int tmpres = 0;
    FFC_PARAMS tmpparams = {0};

    if (params == NULL)
        return 0;

    if (res == NULL)
        res = &tmpres;

    if (!ossl_ffc_params_copy(&tmpparams, params))
        return 0;

    tmpparams.flags = FFC_PARAM_FLAG_VALIDATE_G;
    tmpparams.gindex = FFC_UNVERIFIABLE_GINDEX;

#ifndef FIPS_MODULE
    if (params->flags & FFC_PARAM_FLAG_VALIDATE_LEGACY)
        ret = ossl_ffc_params_FIPS186_2_validate(libctx, &tmpparams, paramstype,
                                                 res, NULL);
    else
#endif
        ret = ossl_ffc_params_FIPS186_4_validate(libctx, &tmpparams, paramstype,
                                                 res, NULL);
#ifndef OPENSSL_NO_DH
    if (ret == FFC_PARAM_RET_STATUS_FAILED
        && (*res & FFC_ERROR_NOT_SUITABLE_GENERATOR) != 0) {
        ERR_raise(ERR_LIB_DH, DH_R_NOT_SUITABLE_GENERATOR);
    }
#endif

    ossl_ffc_params_cleanup(&tmpparams);

    return ret != FFC_PARAM_RET_STATUS_FAILED;
}

/*
 * If possible (or always in FIPS_MODULE) do full FIPS 186-4 validation.
 * Otherwise do simple check but in addition also check the primality of the
 * p and q.
 */
int ossl_ffc_params_full_validate(OSSL_LIB_CTX *libctx, const FFC_PARAMS *params,
                                  int paramstype, int *res)
{
    int tmpres = 0;

    if (params == NULL)
        return 0;

    if (res == NULL)
        res = &tmpres;

#ifdef FIPS_MODULE
    return ossl_ffc_params_FIPS186_4_validate(libctx, params, paramstype,
                                              res, NULL);
#else
    if (params->seed != NULL) {
        if (params->flags & FFC_PARAM_FLAG_VALIDATE_LEGACY)
            return ossl_ffc_params_FIPS186_2_validate(libctx, params, paramstype,
                                                      res, NULL);
        else
            return ossl_ffc_params_FIPS186_4_validate(libctx, params, paramstype,
                                                      res, NULL);
    } else {
        int ret = 0;

        ret = ossl_ffc_params_simple_validate(libctx, params, paramstype, res);
        if (ret) {
            BN_CTX *ctx;

            if ((ctx = BN_CTX_new_ex(libctx)) == NULL)
                return 0;
            if (BN_check_prime(params->q, ctx, NULL) != 1) {
# ifndef OPENSSL_NO_DSA
                ERR_raise(ERR_LIB_DSA, DSA_R_Q_NOT_PRIME);
# endif
                ret = 0;
            }
            if (ret && BN_check_prime(params->p, ctx, NULL) != 1) {
# ifndef OPENSSL_NO_DSA
                ERR_raise(ERR_LIB_DSA, DSA_R_P_NOT_PRIME);
# endif
                ret = 0;
            }
            BN_CTX_free(ctx);
        }
        return ret;
    }
#endif
}
