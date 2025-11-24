/*
 * Copyright 1995-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * NB: These functions have been upgraded - the previous prototypes are in
 * dh_depr.c as wrappers to these ones.  - Geoff
 */

/*
 * DH low level APIs are deprecated for public use, but still ok for
 * internal use.
 *
 * NOTE: When generating keys for key-agreement schemes - FIPS 140-2 IG 9.9
 * states that no additional pairwise tests are required (apart from the tests
 * specified in SP800-56A) when generating keys. Hence DH pairwise tests are
 * omitted here.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/bn.h>
#include <openssl/sha.h>
#include "crypto/dh.h"
#include "crypto/security_bits.h"
#include "dh_local.h"

#ifndef FIPS_MODULE
static int dh_builtin_genparams(DH *ret, int prime_len, int generator,
                                BN_GENCB *cb);
#endif /* FIPS_MODULE */

int ossl_dh_generate_ffc_parameters(DH *dh, int type, int pbits, int qbits,
                                    BN_GENCB *cb)
{
    int ret, res;

#ifndef FIPS_MODULE
    if (type == DH_PARAMGEN_TYPE_FIPS_186_2)
        ret = ossl_ffc_params_FIPS186_2_generate(dh->libctx, &dh->params,
                                                 FFC_PARAM_TYPE_DH,
                                                 pbits, qbits, &res, cb);
    else
#endif
        ret = ossl_ffc_params_FIPS186_4_generate(dh->libctx, &dh->params,
                                                 FFC_PARAM_TYPE_DH,
                                                 pbits, qbits, &res, cb);
    if (ret > 0)
        dh->dirty_cnt++;
    return ret;
}

int ossl_dh_get_named_group_uid_from_size(int pbits)
{
    /*
     * Just choose an approved safe prime group.
     * The alternative to this is to generate FIPS186-4 domain parameters i.e.
     * return dh_generate_ffc_parameters(ret, prime_len, 0, NULL, cb);
     * As the FIPS186-4 generated params are for backwards compatibility,
     * the safe prime group should be used as the default.
     */
    int nid;

    switch (pbits) {
    case 2048:
        nid = NID_ffdhe2048;
        break;
    case 3072:
        nid = NID_ffdhe3072;
        break;
    case 4096:
        nid = NID_ffdhe4096;
        break;
    case 6144:
        nid = NID_ffdhe6144;
        break;
    case 8192:
        nid = NID_ffdhe8192;
        break;
    /* unsupported prime_len */
    default:
        return NID_undef;
    }
    return nid;
}

#ifdef FIPS_MODULE

static int dh_gen_named_group(OSSL_LIB_CTX *libctx, DH *ret, int prime_len)
{
    DH *dh;
    int ok = 0;
    int nid = ossl_dh_get_named_group_uid_from_size(prime_len);

    if (nid == NID_undef)
        return 0;

    dh = ossl_dh_new_by_nid_ex(libctx, nid);
    if (dh != NULL
        && ossl_ffc_params_copy(&ret->params, &dh->params)) {
        ok = 1;
        ret->dirty_cnt++;
    }
    DH_free(dh);
    return ok;
}
#endif /* FIPS_MODULE */

int DH_generate_parameters_ex(DH *ret, int prime_len, int generator,
                              BN_GENCB *cb)
{
#ifdef FIPS_MODULE
    if (generator != 2)
        return 0;
    return dh_gen_named_group(ret->libctx, ret, prime_len);
#else
    if (ret->meth->generate_params)
        return ret->meth->generate_params(ret, prime_len, generator, cb);
    return dh_builtin_genparams(ret, prime_len, generator, cb);
#endif /* FIPS_MODULE */
}

#ifndef FIPS_MODULE
/*-
 * We generate DH parameters as follows
 * find a prime p which is prime_len bits long,
 * where q=(p-1)/2 is also prime.
 * In the following we assume that g is not 0, 1 or p-1, since it
 * would generate only trivial subgroups.
 * For this case, g is a generator of the order-q subgroup if
 * g^q mod p == 1.
 * Or in terms of the Legendre symbol: (g/p) == 1.
 *
 * Having said all that,
 * there is another special case method for the generators 2, 3 and 5.
 * Using the quadratic reciprocity law it is possible to solve
 * (g/p) == 1 for the special values 2, 3, 5:
 * (2/p) == 1 if p mod 8 == 1 or 7.
 * (3/p) == 1 if p mod 12 == 1 or 11.
 * (5/p) == 1 if p mod 5 == 1 or 4.
 * See for instance: https://en.wikipedia.org/wiki/Legendre_symbol
 *
 * Since all safe primes > 7 must satisfy p mod 12 == 11
 * and all safe primes > 11 must satisfy p mod 5 != 1
 * we can further improve the condition for g = 2, 3 and 5:
 * for 2, p mod 24 == 23
 * for 3, p mod 12 == 11
 * for 5, p mod 60 == 59
 */
static int dh_builtin_genparams(DH *ret, int prime_len, int generator,
                                BN_GENCB *cb)
{
    BIGNUM *t1, *t2;
    int g, ok = -1;
    BN_CTX *ctx = NULL;

    if (prime_len > OPENSSL_DH_MAX_MODULUS_BITS) {
        ERR_raise(ERR_LIB_DH, DH_R_MODULUS_TOO_LARGE);
        return 0;
    }

    if (prime_len < DH_MIN_MODULUS_BITS) {
        ERR_raise(ERR_LIB_DH, DH_R_MODULUS_TOO_SMALL);
        return 0;
    }

    ctx = BN_CTX_new_ex(ret->libctx);
    if (ctx == NULL)
        goto err;
    BN_CTX_start(ctx);
    t1 = BN_CTX_get(ctx);
    t2 = BN_CTX_get(ctx);
    if (t2 == NULL)
        goto err;

    /* Make sure 'ret' has the necessary elements */
    if (ret->params.p == NULL && ((ret->params.p = BN_new()) == NULL))
        goto err;
    if (ret->params.g == NULL && ((ret->params.g = BN_new()) == NULL))
        goto err;

    if (generator <= 1) {
        ERR_raise(ERR_LIB_DH, DH_R_BAD_GENERATOR);
        goto err;
    }
    if (generator == DH_GENERATOR_2) {
        if (!BN_set_word(t1, 24))
            goto err;
        if (!BN_set_word(t2, 23))
            goto err;
        g = 2;
    } else if (generator == DH_GENERATOR_5) {
        if (!BN_set_word(t1, 60))
            goto err;
        if (!BN_set_word(t2, 59))
            goto err;
        g = 5;
    } else {
        /*
         * in the general case, don't worry if 'generator' is a generator or
         * not: since we are using safe primes, it will generate either an
         * order-q or an order-2q group, which both is OK
         */
        if (!BN_set_word(t1, 12))
            goto err;
        if (!BN_set_word(t2, 11))
            goto err;
        g = generator;
    }

    if (!BN_generate_prime_ex2(ret->params.p, prime_len, 1, t1, t2, cb, ctx))
        goto err;
    if (!BN_GENCB_call(cb, 3, 0))
        goto err;
    if (!BN_set_word(ret->params.g, g))
        goto err;
    /* We are using safe prime p, set key length equivalent to RFC 7919 */
    ret->length = (2 * ossl_ifc_ffc_compute_security_bits(prime_len)
                   + 24) / 25 * 25;
    ret->dirty_cnt++;
    ok = 1;
 err:
    if (ok == -1) {
        ERR_raise(ERR_LIB_DH, ERR_R_BN_LIB);
        ok = 0;
    }

    BN_CTX_end(ctx);
    BN_CTX_free(ctx);
    return ok;
}
#endif /* FIPS_MODULE */
