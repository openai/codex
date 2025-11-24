/*
 * Copyright 1999-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * RSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <openssl/bn.h>
#include <openssl/err.h>
#include "crypto/rsa.h"
#include "rsa_local.h"

#ifndef FIPS_MODULE
static int rsa_validate_keypair_multiprime(const RSA *key, BN_GENCB *cb)
{
    BIGNUM *i, *j, *k, *l, *m;
    BN_CTX *ctx;
    int ret = 1, ex_primes = 0, idx;
    RSA_PRIME_INFO *pinfo;

    if (key->p == NULL || key->q == NULL || key->n == NULL
            || key->e == NULL || key->d == NULL) {
        ERR_raise(ERR_LIB_RSA, RSA_R_VALUE_MISSING);
        return 0;
    }

    /* multi-prime? */
    if (key->version == RSA_ASN1_VERSION_MULTI) {
        ex_primes = sk_RSA_PRIME_INFO_num(key->prime_infos);
        if (ex_primes <= 0
                || (ex_primes + 2) > ossl_rsa_multip_cap(BN_num_bits(key->n))) {
            ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_MULTI_PRIME_KEY);
            return 0;
        }
    }

    i = BN_new();
    j = BN_new();
    k = BN_new();
    l = BN_new();
    m = BN_new();
    ctx = BN_CTX_new_ex(key->libctx);
    if (i == NULL || j == NULL || k == NULL || l == NULL
            || m == NULL || ctx == NULL) {
        ret = -1;
        ERR_raise(ERR_LIB_RSA, ERR_R_BN_LIB);
        goto err;
    }

    if (BN_is_one(key->e)) {
        ret = 0;
        ERR_raise(ERR_LIB_RSA, RSA_R_BAD_E_VALUE);
    }
    if (!BN_is_odd(key->e)) {
        ret = 0;
        ERR_raise(ERR_LIB_RSA, RSA_R_BAD_E_VALUE);
    }

    /* p prime? */
    if (BN_check_prime(key->p, ctx, cb) != 1) {
        ret = 0;
        ERR_raise(ERR_LIB_RSA, RSA_R_P_NOT_PRIME);
    }

    /* q prime? */
    if (BN_check_prime(key->q, ctx, cb) != 1) {
        ret = 0;
        ERR_raise(ERR_LIB_RSA, RSA_R_Q_NOT_PRIME);
    }

    /* r_i prime? */
    for (idx = 0; idx < ex_primes; idx++) {
        pinfo = sk_RSA_PRIME_INFO_value(key->prime_infos, idx);
        if (BN_check_prime(pinfo->r, ctx, cb) != 1) {
            ret = 0;
            ERR_raise(ERR_LIB_RSA, RSA_R_MP_R_NOT_PRIME);
        }
    }

    /* n = p*q * r_3...r_i? */
    if (!BN_mul(i, key->p, key->q, ctx)) {
        ret = -1;
        goto err;
    }
    for (idx = 0; idx < ex_primes; idx++) {
        pinfo = sk_RSA_PRIME_INFO_value(key->prime_infos, idx);
        if (!BN_mul(i, i, pinfo->r, ctx)) {
            ret = -1;
            goto err;
        }
    }
    if (BN_cmp(i, key->n) != 0) {
        ret = 0;
        if (ex_primes)
            ERR_raise(ERR_LIB_RSA, RSA_R_N_DOES_NOT_EQUAL_PRODUCT_OF_PRIMES);
        else
            ERR_raise(ERR_LIB_RSA, RSA_R_N_DOES_NOT_EQUAL_P_Q);
    }

    /* d*e = 1  mod \lambda(n)? */
    if (!BN_sub(i, key->p, BN_value_one())) {
        ret = -1;
        goto err;
    }
    if (!BN_sub(j, key->q, BN_value_one())) {
        ret = -1;
        goto err;
    }

    /* now compute k = \lambda(n) = LCM(i, j, r_3 - 1...) */
    if (!BN_mul(l, i, j, ctx)) {
        ret = -1;
        goto err;
    }
    if (!BN_gcd(m, i, j, ctx)) {
        ret = -1;
        goto err;
    }
    if (!BN_div(m, NULL, l, m, ctx)) { /* remainder is 0 */
        ret = -1;
        goto err;
    }
    for (idx = 0; idx < ex_primes; idx++) {
        pinfo = sk_RSA_PRIME_INFO_value(key->prime_infos, idx);
        if (!BN_sub(k, pinfo->r, BN_value_one())) {
            ret = -1;
            goto err;
        }
        if (!BN_mul(l, m, k, ctx)) {
            ret = -1;
            goto err;
        }
        if (!BN_gcd(m, m, k, ctx)) {
            ret = -1;
            goto err;
        }
        if (!BN_div(m, NULL, l, m, ctx)) { /* remainder is 0 */
            ret = -1;
            goto err;
        }
    }
    if (!BN_mod_mul(i, key->d, key->e, m, ctx)) {
        ret = -1;
        goto err;
    }

    if (!BN_is_one(i)) {
        ret = 0;
        ERR_raise(ERR_LIB_RSA, RSA_R_D_E_NOT_CONGRUENT_TO_1);
    }

    if (key->dmp1 != NULL && key->dmq1 != NULL && key->iqmp != NULL) {
        /* dmp1 = d mod (p-1)? */
        if (!BN_sub(i, key->p, BN_value_one())) {
            ret = -1;
            goto err;
        }
        if (!BN_mod(j, key->d, i, ctx)) {
            ret = -1;
            goto err;
        }
        if (BN_cmp(j, key->dmp1) != 0) {
            ret = 0;
            ERR_raise(ERR_LIB_RSA, RSA_R_DMP1_NOT_CONGRUENT_TO_D);
        }

        /* dmq1 = d mod (q-1)? */
        if (!BN_sub(i, key->q, BN_value_one())) {
            ret = -1;
            goto err;
        }
        if (!BN_mod(j, key->d, i, ctx)) {
            ret = -1;
            goto err;
        }
        if (BN_cmp(j, key->dmq1) != 0) {
            ret = 0;
            ERR_raise(ERR_LIB_RSA, RSA_R_DMQ1_NOT_CONGRUENT_TO_D);
        }

        /* iqmp = q^-1 mod p? */
        if (!BN_mod_inverse(i, key->q, key->p, ctx)) {
            ret = -1;
            goto err;
        }
        if (BN_cmp(i, key->iqmp) != 0) {
            ret = 0;
            ERR_raise(ERR_LIB_RSA, RSA_R_IQMP_NOT_INVERSE_OF_Q);
        }
    }

    for (idx = 0; idx < ex_primes; idx++) {
        pinfo = sk_RSA_PRIME_INFO_value(key->prime_infos, idx);
        /* d_i = d mod (r_i - 1)? */
        if (!BN_sub(i, pinfo->r, BN_value_one())) {
            ret = -1;
            goto err;
        }
        if (!BN_mod(j, key->d, i, ctx)) {
            ret = -1;
            goto err;
        }
        if (BN_cmp(j, pinfo->d) != 0) {
            ret = 0;
            ERR_raise(ERR_LIB_RSA, RSA_R_MP_EXPONENT_NOT_CONGRUENT_TO_D);
        }
        /* t_i = R_i ^ -1 mod r_i ? */
        if (!BN_mod_inverse(i, pinfo->pp, pinfo->r, ctx)) {
            ret = -1;
            goto err;
        }
        if (BN_cmp(i, pinfo->t) != 0) {
            ret = 0;
            ERR_raise(ERR_LIB_RSA, RSA_R_MP_COEFFICIENT_NOT_INVERSE_OF_R);
        }
    }

 err:
    BN_free(i);
    BN_free(j);
    BN_free(k);
    BN_free(l);
    BN_free(m);
    BN_CTX_free(ctx);
    return ret;
}
#endif /* FIPS_MODULE */

int ossl_rsa_validate_public(const RSA *key)
{
    return ossl_rsa_sp800_56b_check_public(key);
}

int ossl_rsa_validate_private(const RSA *key)
{
    return ossl_rsa_sp800_56b_check_private(key);
}

int ossl_rsa_validate_pairwise(const RSA *key)
{
#ifdef FIPS_MODULE
    return ossl_rsa_sp800_56b_check_keypair(key, NULL, -1, RSA_bits(key));
#else
    return rsa_validate_keypair_multiprime(key, NULL) > 0;
#endif
}

int RSA_check_key(const RSA *key)
{
    return RSA_check_key_ex(key, NULL);
}

int RSA_check_key_ex(const RSA *key, BN_GENCB *cb)
{
#ifdef FIPS_MODULE
    return ossl_rsa_validate_public(key)
           && ossl_rsa_validate_private(key)
           && ossl_rsa_validate_pairwise(key);
#else
    return rsa_validate_keypair_multiprime(key, cb);
#endif /* FIPS_MODULE */
}
