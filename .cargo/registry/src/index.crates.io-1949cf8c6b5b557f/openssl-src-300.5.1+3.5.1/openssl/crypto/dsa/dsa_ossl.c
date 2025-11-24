/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * DSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include "internal/cryptlib.h"
#include "crypto/bn.h"
#include <openssl/bn.h>
#include <openssl/sha.h>
#include "dsa_local.h"
#include <openssl/asn1.h>
#include "internal/deterministic_nonce.h"

#define MIN_DSA_SIGN_QBITS   128
#define MAX_DSA_SIGN_RETRIES 8

static DSA_SIG *dsa_do_sign(const unsigned char *dgst, int dlen, DSA *dsa);
static int dsa_sign_setup_no_digest(DSA *dsa, BN_CTX *ctx_in, BIGNUM **kinvp,
                                    BIGNUM **rp);
static int dsa_sign_setup(DSA *dsa, BN_CTX *ctx_in, BIGNUM **kinvp,
                          BIGNUM **rp, const unsigned char *dgst, int dlen,
                          unsigned int nonce_type, const char *digestname,
                          OSSL_LIB_CTX *libctx, const char *propq);
static int dsa_do_verify(const unsigned char *dgst, int dgst_len,
                         DSA_SIG *sig, DSA *dsa);
static int dsa_init(DSA *dsa);
static int dsa_finish(DSA *dsa);
static BIGNUM *dsa_mod_inverse_fermat(const BIGNUM *k, const BIGNUM *q,
                                      BN_CTX *ctx);

static DSA_METHOD openssl_dsa_meth = {
    "OpenSSL DSA method",
    dsa_do_sign,
    dsa_sign_setup_no_digest,
    dsa_do_verify,
    NULL,                       /* dsa_mod_exp, */
    NULL,                       /* dsa_bn_mod_exp, */
    dsa_init,
    dsa_finish,
    DSA_FLAG_FIPS_METHOD,
    NULL,
    NULL,
    NULL
};

static const DSA_METHOD *default_DSA_method = &openssl_dsa_meth;

#ifndef FIPS_MODULE
void DSA_set_default_method(const DSA_METHOD *meth)
{
    default_DSA_method = meth;
}
#endif /* FIPS_MODULE */

const DSA_METHOD *DSA_get_default_method(void)
{
    return default_DSA_method;
}

const DSA_METHOD *DSA_OpenSSL(void)
{
    return &openssl_dsa_meth;
}

DSA_SIG *ossl_dsa_do_sign_int(const unsigned char *dgst, int dlen, DSA *dsa,
                              unsigned int nonce_type, const char *digestname,
                              OSSL_LIB_CTX *libctx, const char *propq)
{
    BIGNUM *kinv = NULL;
    BIGNUM *m, *blind, *blindm, *tmp;
    BN_CTX *ctx = NULL;
    int reason = ERR_R_BN_LIB;
    DSA_SIG *ret = NULL;
    int rv = 0;
    int retries = 0;

    if (dsa->params.p == NULL
        || dsa->params.q == NULL
        || dsa->params.g == NULL) {
        reason = DSA_R_MISSING_PARAMETERS;
        goto err;
    }
    if (dsa->priv_key == NULL) {
        reason = DSA_R_MISSING_PRIVATE_KEY;
        goto err;
    }

    ret = DSA_SIG_new();
    if (ret == NULL)
        goto err;
    ret->r = BN_new();
    ret->s = BN_new();
    if (ret->r == NULL || ret->s == NULL)
        goto err;

    ctx = BN_CTX_new_ex(dsa->libctx);
    if (ctx == NULL)
        goto err;
    m = BN_CTX_get(ctx);
    blind = BN_CTX_get(ctx);
    blindm = BN_CTX_get(ctx);
    tmp = BN_CTX_get(ctx);
    if (tmp == NULL)
        goto err;

 redo:
    if (!dsa_sign_setup(dsa, ctx, &kinv, &ret->r, dgst, dlen,
                        nonce_type, digestname, libctx, propq))
        goto err;

    if (dlen > BN_num_bytes(dsa->params.q))
        /*
         * if the digest length is greater than the size of q use the
         * BN_num_bits(dsa->q) leftmost bits of the digest, see fips 186-3,
         * 4.2
         */
        dlen = BN_num_bytes(dsa->params.q);
    if (BN_bin2bn(dgst, dlen, m) == NULL)
        goto err;

    /*
     * The normal signature calculation is:
     *
     *   s := k^-1 * (m + r * priv_key) mod q
     *
     * We will blind this to protect against side channel attacks
     *
     *   s := blind^-1 * k^-1 * (blind * m + blind * r * priv_key) mod q
     */

    /*
     * Generate a blinding value
     * The size of q is tested in dsa_sign_setup() so there should not be an infinite loop here.
     */
    do {
        if (!BN_priv_rand_ex(blind, BN_num_bits(dsa->params.q) - 1,
                             BN_RAND_TOP_ANY, BN_RAND_BOTTOM_ANY, 0, ctx))
            goto err;
    } while (BN_is_zero(blind));
    BN_set_flags(blind, BN_FLG_CONSTTIME);
    BN_set_flags(blindm, BN_FLG_CONSTTIME);
    BN_set_flags(tmp, BN_FLG_CONSTTIME);

    /* tmp := blind * priv_key * r mod q */
    if (!BN_mod_mul(tmp, blind, dsa->priv_key, dsa->params.q, ctx))
        goto err;
    if (!BN_mod_mul(tmp, tmp, ret->r, dsa->params.q, ctx))
        goto err;

    /* blindm := blind * m mod q */
    if (!BN_mod_mul(blindm, blind, m, dsa->params.q, ctx))
        goto err;

    /* s : = (blind * priv_key * r) + (blind * m) mod q */
    if (!BN_mod_add_quick(ret->s, tmp, blindm, dsa->params.q))
        goto err;

    /* s := s * k^-1 mod q */
    if (!BN_mod_mul(ret->s, ret->s, kinv, dsa->params.q, ctx))
        goto err;

    /* s:= s * blind^-1 mod q */
    if (BN_mod_inverse(blind, blind, dsa->params.q, ctx) == NULL)
        goto err;
    if (!BN_mod_mul(ret->s, ret->s, blind, dsa->params.q, ctx))
        goto err;

    /*
     * Redo if r or s is zero as required by FIPS 186-4: Section 4.6
     * This is very unlikely.
     * Limit the retries so there is no possibility of an infinite
     * loop for bad domain parameter values.
     */
    if (BN_is_zero(ret->r) || BN_is_zero(ret->s)) {
        if (retries++ > MAX_DSA_SIGN_RETRIES) {
            reason = DSA_R_TOO_MANY_RETRIES;
            goto err;
        }
        goto redo;
    }
    rv = 1;
 err:
    if (rv == 0) {
        ERR_raise(ERR_LIB_DSA, reason);
        DSA_SIG_free(ret);
        ret = NULL;
    }
    BN_CTX_free(ctx);
    BN_clear_free(kinv);
    return ret;
}

static DSA_SIG *dsa_do_sign(const unsigned char *dgst, int dlen, DSA *dsa)
{
    return ossl_dsa_do_sign_int(dgst, dlen, dsa,
                                0, NULL, NULL, NULL);
}

static int dsa_sign_setup_no_digest(DSA *dsa, BN_CTX *ctx_in,
                                    BIGNUM **kinvp, BIGNUM **rp)
{
    return dsa_sign_setup(dsa, ctx_in, kinvp, rp, NULL, 0,
                          0, NULL, NULL, NULL);
}

static int dsa_sign_setup(DSA *dsa, BN_CTX *ctx_in,
                          BIGNUM **kinvp, BIGNUM **rp,
                          const unsigned char *dgst, int dlen,
                          unsigned int nonce_type, const char *digestname,
                          OSSL_LIB_CTX *libctx, const char *propq)
{
    BN_CTX *ctx = NULL;
    BIGNUM *k, *kinv = NULL, *r = *rp;
    BIGNUM *l;
    int ret = 0;
    int q_bits, q_words;

    if (!dsa->params.p || !dsa->params.q || !dsa->params.g) {
        ERR_raise(ERR_LIB_DSA, DSA_R_MISSING_PARAMETERS);
        return 0;
    }

    /* Reject obviously invalid parameters */
    if (BN_is_zero(dsa->params.p)
        || BN_is_zero(dsa->params.q)
        || BN_is_zero(dsa->params.g)
        || BN_is_negative(dsa->params.p)
        || BN_is_negative(dsa->params.q)
        || BN_is_negative(dsa->params.g)) {
        ERR_raise(ERR_LIB_DSA, DSA_R_INVALID_PARAMETERS);
        return 0;
    }
    if (dsa->priv_key == NULL) {
        ERR_raise(ERR_LIB_DSA, DSA_R_MISSING_PRIVATE_KEY);
        return 0;
    }
    k = BN_new();
    l = BN_new();
    if (k == NULL || l == NULL)
        goto err;

    if (ctx_in == NULL) {
        /* if you don't pass in ctx_in you get a default libctx */
        if ((ctx = BN_CTX_new_ex(NULL)) == NULL)
            goto err;
    } else
        ctx = ctx_in;

    /* Preallocate space */
    q_bits = BN_num_bits(dsa->params.q);
    q_words = bn_get_top(dsa->params.q);
    if (q_bits < MIN_DSA_SIGN_QBITS
        || !bn_wexpand(k, q_words + 2)
        || !bn_wexpand(l, q_words + 2))
        goto err;

    /* Get random k */
    do {
        if (dgst != NULL) {
            if (nonce_type == 1) {
#ifndef FIPS_MODULE
                if (!ossl_gen_deterministic_nonce_rfc6979(k, dsa->params.q,
                                                          dsa->priv_key,
                                                          dgst, dlen,
                                                          digestname,
                                                          libctx, propq))
#endif
                    goto err;
            } else {
                /*
                 * We calculate k from SHA512(private_key + H(message) + random).
                 * This protects the private key from a weak PRNG.
                 */
                if (!ossl_bn_gen_dsa_nonce_fixed_top(k, dsa->params.q,
                                                     dsa->priv_key, dgst,
                                                     dlen, ctx))
                    goto err;
            }
        } else if (!ossl_bn_priv_rand_range_fixed_top(k, dsa->params.q, 0, ctx))
            goto err;
    } while (ossl_bn_is_word_fixed_top(k, 0));

    BN_set_flags(k, BN_FLG_CONSTTIME);
    BN_set_flags(l, BN_FLG_CONSTTIME);

    if (dsa->flags & DSA_FLAG_CACHE_MONT_P) {
        if (!BN_MONT_CTX_set_locked(&dsa->method_mont_p,
                                    dsa->lock, dsa->params.p, ctx))
            goto err;
    }

    /* Compute r = (g^k mod p) mod q */

    /*
     * We do not want timing information to leak the length of k, so we
     * compute G^k using an equivalent scalar of fixed bit-length.
     *
     * We unconditionally perform both of these additions to prevent a
     * small timing information leakage.  We then choose the sum that is
     * one bit longer than the modulus.
     *
     * There are some concerns about the efficacy of doing this.  More
     * specifically refer to the discussion starting with:
     *     https://github.com/openssl/openssl/pull/7486#discussion_r228323705
     * The fix is to rework BN so these gymnastics aren't required.
     */
    if (!BN_add(l, k, dsa->params.q)
        || !BN_add(k, l, dsa->params.q))
        goto err;

    BN_consttime_swap(BN_is_bit_set(l, q_bits), k, l, q_words + 2);

    if ((dsa)->meth->bn_mod_exp != NULL) {
            if (!dsa->meth->bn_mod_exp(dsa, r, dsa->params.g, k, dsa->params.p,
                                       ctx, dsa->method_mont_p))
                goto err;
    } else {
            if (!BN_mod_exp_mont(r, dsa->params.g, k, dsa->params.p, ctx,
                                 dsa->method_mont_p))
                goto err;
    }

    if (!BN_mod(r, r, dsa->params.q, ctx))
        goto err;

    /* Compute part of 's = inv(k) (m + xr) mod q' */
    if ((kinv = dsa_mod_inverse_fermat(k, dsa->params.q, ctx)) == NULL)
        goto err;

    BN_clear_free(*kinvp);
    *kinvp = kinv;
    kinv = NULL;
    ret = 1;
 err:
    if (!ret)
        ERR_raise(ERR_LIB_DSA, ERR_R_BN_LIB);
    if (ctx != ctx_in)
        BN_CTX_free(ctx);
    BN_clear_free(k);
    BN_clear_free(l);
    return ret;
}

static int dsa_do_verify(const unsigned char *dgst, int dgst_len,
                         DSA_SIG *sig, DSA *dsa)
{
    BN_CTX *ctx;
    BIGNUM *u1, *u2, *t1;
    BN_MONT_CTX *mont = NULL;
    const BIGNUM *r, *s;
    int ret = -1, i;

    if (dsa->params.p == NULL
        || dsa->params.q == NULL
        || dsa->params.g == NULL) {
        ERR_raise(ERR_LIB_DSA, DSA_R_MISSING_PARAMETERS);
        return -1;
    }

    i = BN_num_bits(dsa->params.q);
    /* fips 186-3 allows only different sizes for q */
    if (i != 160 && i != 224 && i != 256) {
        ERR_raise(ERR_LIB_DSA, DSA_R_BAD_Q_VALUE);
        return -1;
    }

    if (BN_num_bits(dsa->params.p) > OPENSSL_DSA_MAX_MODULUS_BITS) {
        ERR_raise(ERR_LIB_DSA, DSA_R_MODULUS_TOO_LARGE);
        return -1;
    }
    u1 = BN_new();
    u2 = BN_new();
    t1 = BN_new();
    ctx = BN_CTX_new_ex(NULL); /* verify does not need a libctx */
    if (u1 == NULL || u2 == NULL || t1 == NULL || ctx == NULL)
        goto err;

    DSA_SIG_get0(sig, &r, &s);

    if (BN_is_zero(r) || BN_is_negative(r) ||
        BN_ucmp(r, dsa->params.q) >= 0) {
        ret = 0;
        goto err;
    }
    if (BN_is_zero(s) || BN_is_negative(s) ||
        BN_ucmp(s, dsa->params.q) >= 0) {
        ret = 0;
        goto err;
    }

    /*
     * Calculate W = inv(S) mod Q save W in u2
     */
    if ((BN_mod_inverse(u2, s, dsa->params.q, ctx)) == NULL)
        goto err;

    /* save M in u1 */
    if (dgst_len > (i >> 3))
        /*
         * if the digest length is greater than the size of q use the
         * BN_num_bits(dsa->q) leftmost bits of the digest, see fips 186-3,
         * 4.2
         */
        dgst_len = (i >> 3);
    if (BN_bin2bn(dgst, dgst_len, u1) == NULL)
        goto err;

    /* u1 = M * w mod q */
    if (!BN_mod_mul(u1, u1, u2, dsa->params.q, ctx))
        goto err;

    /* u2 = r * w mod q */
    if (!BN_mod_mul(u2, r, u2, dsa->params.q, ctx))
        goto err;

    if (dsa->flags & DSA_FLAG_CACHE_MONT_P) {
        mont = BN_MONT_CTX_set_locked(&dsa->method_mont_p,
                                      dsa->lock, dsa->params.p, ctx);
        if (!mont)
            goto err;
    }

    if (dsa->meth->dsa_mod_exp != NULL) {
        if (!dsa->meth->dsa_mod_exp(dsa, t1, dsa->params.g, u1, dsa->pub_key, u2,
                                    dsa->params.p, ctx, mont))
            goto err;
    } else {
        if (!BN_mod_exp2_mont(t1, dsa->params.g, u1, dsa->pub_key, u2,
                              dsa->params.p, ctx, mont))
            goto err;
    }

    /* let u1 = u1 mod q */
    if (!BN_mod(u1, t1, dsa->params.q, ctx))
        goto err;

    /*
     * V is now in u1.  If the signature is correct, it will be equal to R.
     */
    ret = (BN_ucmp(u1, r) == 0);

 err:
    if (ret < 0)
        ERR_raise(ERR_LIB_DSA, ERR_R_BN_LIB);
    BN_CTX_free(ctx);
    BN_free(u1);
    BN_free(u2);
    BN_free(t1);
    return ret;
}

static int dsa_init(DSA *dsa)
{
    dsa->flags |= DSA_FLAG_CACHE_MONT_P;
    dsa->dirty_cnt++;
    return 1;
}

static int dsa_finish(DSA *dsa)
{
    BN_MONT_CTX_free(dsa->method_mont_p);
    return 1;
}

/*
 * Compute the inverse of k modulo q.
 * Since q is prime, Fermat's Little Theorem applies, which reduces this to
 * mod-exp operation.  Both the exponent and modulus are public information
 * so a mod-exp that doesn't leak the base is sufficient.  A newly allocated
 * BIGNUM is returned which the caller must free.
 */
static BIGNUM *dsa_mod_inverse_fermat(const BIGNUM *k, const BIGNUM *q,
                                      BN_CTX *ctx)
{
    BIGNUM *res = NULL;
    BIGNUM *r, *e;

    if ((r = BN_new()) == NULL)
        return NULL;

    BN_CTX_start(ctx);
    if ((e = BN_CTX_get(ctx)) != NULL
            && BN_set_word(r, 2)
            && BN_sub(e, q, r)
            && BN_mod_exp_mont(r, k, e, q, ctx, NULL))
        res = r;
    else
        BN_free(r);
    BN_CTX_end(ctx);
    return res;
}
