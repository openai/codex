/*
 * Copyright 2002-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * ECDSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <string.h>
#include <openssl/err.h>
#include <openssl/obj_mac.h>
#include <openssl/rand.h>
#include "crypto/bn.h"
#include "ec_local.h"
#include "internal/deterministic_nonce.h"

#define MIN_ECDSA_SIGN_ORDERBITS 64
/*
 * It is highly unlikely that a retry will happen,
 * Multiple retries would indicate that something is wrong
 * with the group parameters (which would normally only happen
 * with a bad custom group).
 */
#define MAX_ECDSA_SIGN_RETRIES 8

static int ecdsa_sign_setup(EC_KEY *eckey, BN_CTX *ctx_in,
                            BIGNUM **kinvp, BIGNUM **rp,
                            const unsigned char *dgst, int dlen,
                            unsigned int nonce_type, const char *digestname,
                            OSSL_LIB_CTX *libctx, const char *propq);

int ossl_ecdsa_sign_setup(EC_KEY *eckey, BN_CTX *ctx_in, BIGNUM **kinvp,
                          BIGNUM **rp)
{
    if (eckey->group->meth->ecdsa_sign_setup == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_CURVE_DOES_NOT_SUPPORT_ECDSA);
        return 0;
    }

    return eckey->group->meth->ecdsa_sign_setup(eckey, ctx_in, kinvp, rp);
}

ECDSA_SIG *ossl_ecdsa_sign_sig(const unsigned char *dgst, int dgst_len,
                               const BIGNUM *in_kinv, const BIGNUM *in_r,
                               EC_KEY *eckey)
{
    if (eckey->group->meth->ecdsa_sign_sig == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_CURVE_DOES_NOT_SUPPORT_ECDSA);
        return NULL;
    }

    return eckey->group->meth->ecdsa_sign_sig(dgst, dgst_len,
                                              in_kinv, in_r, eckey);
}

int ossl_ecdsa_verify_sig(const unsigned char *dgst, int dgst_len,
                          const ECDSA_SIG *sig, EC_KEY *eckey)
{
    if (eckey->group->meth->ecdsa_verify_sig == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_CURVE_DOES_NOT_SUPPORT_ECDSA);
        return 0;
    }

    return eckey->group->meth->ecdsa_verify_sig(dgst, dgst_len, sig, eckey);
}

int ossl_ecdsa_sign(int type, const unsigned char *dgst, int dlen,
                    unsigned char *sig, unsigned int *siglen,
                    const BIGNUM *kinv, const BIGNUM *r, EC_KEY *eckey)
{
    ECDSA_SIG *s;

    if (sig == NULL && (kinv == NULL || r == NULL)) {
        *siglen = ECDSA_size(eckey);
        return 1;
    }

    s = ECDSA_do_sign_ex(dgst, dlen, kinv, r, eckey);
    if (s == NULL) {
        *siglen = 0;
        return 0;
    }
    *siglen = i2d_ECDSA_SIG(s, sig != NULL ? &sig : NULL);
    ECDSA_SIG_free(s);
    return 1;
}

int ossl_ecdsa_deterministic_sign(const unsigned char *dgst, int dlen,
                                  unsigned char *sig, unsigned int *siglen,
                                  EC_KEY *eckey, unsigned int nonce_type,
                                  const char *digestname,
                                  OSSL_LIB_CTX *libctx, const char *propq)
{
    ECDSA_SIG *s;
    BIGNUM *kinv = NULL, *r = NULL;
    int ret = 0;

    if (sig == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (digestname == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_DIGEST);
        return 0;
    }

    *siglen = 0;
    if (!ecdsa_sign_setup(eckey, NULL, &kinv, &r, dgst, dlen,
                          nonce_type, digestname, libctx, propq))
        return 0;

    s = ECDSA_do_sign_ex(dgst, dlen, kinv, r, eckey);
    if (s == NULL)
        goto end;

    *siglen = i2d_ECDSA_SIG(s, &sig);
    ECDSA_SIG_free(s);
    ret = 1;
end:
    BN_clear_free(kinv);
    BN_clear_free(r);
    return ret;
}

static int ecdsa_sign_setup(EC_KEY *eckey, BN_CTX *ctx_in,
                            BIGNUM **kinvp, BIGNUM **rp,
                            const unsigned char *dgst, int dlen,
                            unsigned int nonce_type, const char *digestname,
                            OSSL_LIB_CTX *libctx, const char *propq)
{
    BN_CTX *ctx = NULL;
    BIGNUM *k = NULL, *r = NULL, *X = NULL;
    const BIGNUM *order;
    EC_POINT *tmp_point = NULL;
    const EC_GROUP *group;
    int ret = 0;
    int order_bits;
    const BIGNUM *priv_key;

    if (eckey == NULL || (group = EC_KEY_get0_group(eckey)) == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if ((priv_key = EC_KEY_get0_private_key(eckey)) == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_MISSING_PRIVATE_KEY);
        return 0;
    }

    if (!EC_KEY_can_sign(eckey)) {
        ERR_raise(ERR_LIB_EC, EC_R_CURVE_DOES_NOT_SUPPORT_SIGNING);
        return 0;
    }

    if ((ctx = ctx_in) == NULL) {
        if ((ctx = BN_CTX_new_ex(eckey->libctx)) == NULL) {
            ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
            return 0;
        }
    }

    k = BN_secure_new();        /* this value is later returned in *kinvp */
    r = BN_new();               /* this value is later returned in *rp */
    X = BN_new();
    if (k == NULL || r == NULL || X == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
        goto err;
    }
    if ((tmp_point = EC_POINT_new(group)) == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
        goto err;
    }

    if ((order = EC_GROUP_get0_order(group)) == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
        goto err;
    }

    /* Preallocate space */
    order_bits = BN_num_bits(order);
    /* Check the number of bits here so that an infinite loop is not possible */
    if (order_bits < MIN_ECDSA_SIGN_ORDERBITS
        || !BN_set_bit(k, order_bits)
        || !BN_set_bit(r, order_bits)
        || !BN_set_bit(X, order_bits))
        goto err;

    do {
        /* get random or deterministic value of k */
        do {
            int res = 0;

            if (dgst != NULL) {
                if (nonce_type == 1) {
#ifndef FIPS_MODULE
                    res = ossl_gen_deterministic_nonce_rfc6979(k, order,
                                                               priv_key,
                                                               dgst, dlen,
                                                               digestname,
                                                               libctx, propq);
#endif
                } else {
                    res = ossl_bn_gen_dsa_nonce_fixed_top(k, order, priv_key,
                                                          dgst, dlen, ctx);
                }
            } else {
                res = ossl_bn_priv_rand_range_fixed_top(k, order, 0, ctx);
            }
            if (!res) {
                ERR_raise(ERR_LIB_EC, EC_R_RANDOM_NUMBER_GENERATION_FAILED);
                goto err;
            }
        } while (ossl_bn_is_word_fixed_top(k, 0));

        /* compute r the x-coordinate of generator * k */
        if (!EC_POINT_mul(group, tmp_point, k, NULL, NULL, ctx)) {
            ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
            goto err;
        }

        if (!EC_POINT_get_affine_coordinates(group, tmp_point, X, NULL, ctx)) {
            ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
            goto err;
        }

        if (!BN_nnmod(r, X, order, ctx)) {
            ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
            goto err;
        }
    } while (BN_is_zero(r));

    /* compute the inverse of k */
    if (!ossl_ec_group_do_inverse_ord(group, k, k, ctx)) {
        ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
        goto err;
    }

    /* clear old values if necessary */
    BN_clear_free(*rp);
    BN_clear_free(*kinvp);
    /* save the pre-computed values  */
    *rp = r;
    *kinvp = k;
    ret = 1;
 err:
    if (!ret) {
        BN_clear_free(k);
        BN_clear_free(r);
    }
    if (ctx != ctx_in)
        BN_CTX_free(ctx);
    EC_POINT_free(tmp_point);
    BN_clear_free(X);
    return ret;
}

int ossl_ecdsa_simple_sign_setup(EC_KEY *eckey, BN_CTX *ctx_in, BIGNUM **kinvp,
                                 BIGNUM **rp)
{
    return ecdsa_sign_setup(eckey, ctx_in, kinvp, rp, NULL, 0,
                            0, NULL, NULL, NULL);
}

ECDSA_SIG *ossl_ecdsa_simple_sign_sig(const unsigned char *dgst, int dgst_len,
                                      const BIGNUM *in_kinv, const BIGNUM *in_r,
                                      EC_KEY *eckey)
{
    int ok = 0, i;
    int retries = 0;
    BIGNUM *kinv = NULL, *s, *m = NULL;
    const BIGNUM *order, *ckinv;
    BN_CTX *ctx = NULL;
    const EC_GROUP *group;
    ECDSA_SIG *ret;
    const BIGNUM *priv_key;

    group = EC_KEY_get0_group(eckey);
    priv_key = EC_KEY_get0_private_key(eckey);

    if (group == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }
    if (priv_key == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_MISSING_PRIVATE_KEY);
        return NULL;
    }

    if (!EC_KEY_can_sign(eckey)) {
        ERR_raise(ERR_LIB_EC, EC_R_CURVE_DOES_NOT_SUPPORT_SIGNING);
        return NULL;
    }

    ret = ECDSA_SIG_new();
    if (ret == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_ECDSA_LIB);
        return NULL;
    }
    ret->r = BN_new();
    ret->s = BN_new();
    if (ret->r == NULL || ret->s == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
        goto err;
    }
    s = ret->s;

    if ((ctx = BN_CTX_new_ex(eckey->libctx)) == NULL
        || (m = BN_new()) == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
        goto err;
    }

    if ((order = EC_GROUP_get0_order(group)) == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
        goto err;
    }

    i = BN_num_bits(order);
    /*
     * Need to truncate digest if it is too long: first truncate whole bytes.
     */
    if (8 * dgst_len > i)
        dgst_len = (i + 7) / 8;
    if (!BN_bin2bn(dgst, dgst_len, m)) {
        ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
        goto err;
    }
    /* If still too long, truncate remaining bits with a shift */
    if ((8 * dgst_len > i) && !BN_rshift(m, m, 8 - (i & 0x7))) {
        ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
        goto err;
    }
    do {
        if (in_kinv == NULL || in_r == NULL) {
            if (!ecdsa_sign_setup(eckey, ctx, &kinv, &ret->r, dgst, dgst_len,
                                  0, NULL, NULL, NULL)) {
                ERR_raise(ERR_LIB_EC, ERR_R_ECDSA_LIB);
                goto err;
            }
            ckinv = kinv;
        } else {
            ckinv = in_kinv;
            if (BN_copy(ret->r, in_r) == NULL) {
                ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
                goto err;
            }
        }

        /*
         * With only one multiplicant being in Montgomery domain
         * multiplication yields real result without post-conversion.
         * Also note that all operations but last are performed with
         * zero-padded vectors. Last operation, BN_mod_mul_montgomery
         * below, returns user-visible value with removed zero padding.
         */
        if (!bn_to_mont_fixed_top(s, ret->r, group->mont_data, ctx)
            || !bn_mul_mont_fixed_top(s, s, priv_key, group->mont_data, ctx)) {
            ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
            goto err;
        }
        if (!bn_mod_add_fixed_top(s, s, m, order)) {
            ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
            goto err;
        }
        /*
         * |s| can still be larger than modulus, because |m| can be. In
         * such case we count on Montgomery reduction to tie it up.
         */
        if (!bn_to_mont_fixed_top(s, s, group->mont_data, ctx)
            || !BN_mod_mul_montgomery(s, s, ckinv, group->mont_data, ctx)) {
            ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
            goto err;
        }

        if (BN_is_zero(s)) {
            /*
             * if kinv and r have been supplied by the caller, don't
             * generate new kinv and r values
             */
            if (in_kinv != NULL && in_r != NULL) {
                ERR_raise(ERR_LIB_EC, EC_R_NEED_NEW_SETUP_VALUES);
                goto err;
            }
            /* Avoid infinite loops cause by invalid group parameters */
            if (retries++ > MAX_ECDSA_SIGN_RETRIES) {
                ERR_raise(ERR_LIB_EC, EC_R_TOO_MANY_RETRIES);
                goto err;
            }
        } else {
            /* s != 0 => we have a valid signature */
            break;
        }
    } while (1);

    ok = 1;
 err:
    if (!ok) {
        ECDSA_SIG_free(ret);
        ret = NULL;
    }
    BN_CTX_free(ctx);
    BN_clear_free(m);
    BN_clear_free(kinv);
    return ret;
}

/*-
 * returns
 *      1: correct signature
 *      0: incorrect signature
 *     -1: error
 */
int ossl_ecdsa_verify(int type, const unsigned char *dgst, int dgst_len,
                      const unsigned char *sigbuf, int sig_len, EC_KEY *eckey)
{
    ECDSA_SIG *s;
    const unsigned char *p = sigbuf;
    unsigned char *der = NULL;
    int derlen = -1;
    int ret = -1;

    s = ECDSA_SIG_new();
    if (s == NULL)
        return ret;
    if (d2i_ECDSA_SIG(&s, &p, sig_len) == NULL)
        goto err;
    /* Ensure signature uses DER and doesn't have trailing garbage */
    derlen = i2d_ECDSA_SIG(s, &der);
    if (derlen != sig_len || memcmp(sigbuf, der, derlen) != 0)
        goto err;
    ret = ECDSA_do_verify(dgst, dgst_len, s, eckey);
 err:
    OPENSSL_free(der);
    ECDSA_SIG_free(s);
    return ret;
}

int ossl_ecdsa_simple_verify_sig(const unsigned char *dgst, int dgst_len,
                                 const ECDSA_SIG *sig, EC_KEY *eckey)
{
    int ret = -1, i;
    BN_CTX *ctx;
    const BIGNUM *order;
    BIGNUM *u1, *u2, *m, *X;
    EC_POINT *point = NULL;
    const EC_GROUP *group;
    const EC_POINT *pub_key;

    /* check input values */
    if (eckey == NULL || (group = EC_KEY_get0_group(eckey)) == NULL ||
        (pub_key = EC_KEY_get0_public_key(eckey)) == NULL || sig == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_MISSING_PARAMETERS);
        return -1;
    }

    if (!EC_KEY_can_sign(eckey)) {
        ERR_raise(ERR_LIB_EC, EC_R_CURVE_DOES_NOT_SUPPORT_SIGNING);
        return -1;
    }

    ctx = BN_CTX_new_ex(eckey->libctx);
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
        return -1;
    }
    BN_CTX_start(ctx);
    u1 = BN_CTX_get(ctx);
    u2 = BN_CTX_get(ctx);
    m = BN_CTX_get(ctx);
    X = BN_CTX_get(ctx);
    if (X == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
        goto err;
    }

    order = EC_GROUP_get0_order(group);
    if (order == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
        goto err;
    }

    if (BN_is_zero(sig->r) || BN_is_negative(sig->r) ||
        BN_ucmp(sig->r, order) >= 0 || BN_is_zero(sig->s) ||
        BN_is_negative(sig->s) || BN_ucmp(sig->s, order) >= 0) {
        ERR_raise(ERR_LIB_EC, EC_R_BAD_SIGNATURE);
        ret = 0;                /* signature is invalid */
        goto err;
    }
    /* calculate tmp1 = inv(S) mod order */
    if (!ossl_ec_group_do_inverse_ord(group, u2, sig->s, ctx)) {
        ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
        goto err;
    }
    /* digest -> m */
    i = BN_num_bits(order);
    /*
     * Need to truncate digest if it is too long: first truncate whole bytes.
     */
    if (8 * dgst_len > i)
        dgst_len = (i + 7) / 8;
    if (!BN_bin2bn(dgst, dgst_len, m)) {
        ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
        goto err;
    }
    /* If still too long truncate remaining bits with a shift */
    if ((8 * dgst_len > i) && !BN_rshift(m, m, 8 - (i & 0x7))) {
        ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
        goto err;
    }
    /* u1 = m * tmp mod order */
    if (!BN_mod_mul(u1, m, u2, order, ctx)) {
        ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
        goto err;
    }
    /* u2 = r * w mod q */
    if (!BN_mod_mul(u2, sig->r, u2, order, ctx)) {
        ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
        goto err;
    }

    if ((point = EC_POINT_new(group)) == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
        goto err;
    }
    if (!EC_POINT_mul(group, point, u1, pub_key, u2, ctx)) {
        ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
        goto err;
    }

    if (!EC_POINT_get_affine_coordinates(group, point, X, NULL, ctx)) {
        ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
        goto err;
    }

    if (!BN_nnmod(u1, X, order, ctx)) {
        ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
        goto err;
    }
    /*  if the signature is correct u1 is equal to sig->r */
    ret = (BN_ucmp(u1, sig->r) == 0);
 err:
    BN_CTX_end(ctx);
    BN_CTX_free(ctx);
    EC_POINT_free(point);
    return ret;
}
