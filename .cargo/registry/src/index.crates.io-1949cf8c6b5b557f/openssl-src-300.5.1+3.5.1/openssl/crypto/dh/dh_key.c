/*
 * Copyright 1995-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * DH low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include "internal/cryptlib.h"
#include "dh_local.h"
#include "crypto/bn.h"
#include "crypto/dh.h"
#include "crypto/security_bits.h"

#ifdef FIPS_MODULE
# define MIN_STRENGTH 112
#else
# define MIN_STRENGTH 80
#endif

static int generate_key(DH *dh);
static int dh_bn_mod_exp(const DH *dh, BIGNUM *r,
                         const BIGNUM *a, const BIGNUM *p,
                         const BIGNUM *m, BN_CTX *ctx, BN_MONT_CTX *m_ctx);
static int dh_init(DH *dh);
static int dh_finish(DH *dh);

/*
 * See SP800-56Ar3 Section 5.7.1.1
 * Finite Field Cryptography Diffie-Hellman (FFC DH) Primitive
 */
int ossl_dh_compute_key(unsigned char *key, const BIGNUM *pub_key, DH *dh)
{
    BN_CTX *ctx = NULL;
    BN_MONT_CTX *mont = NULL;
    BIGNUM *z = NULL, *pminus1;
    int ret = -1;

    if (BN_num_bits(dh->params.p) > OPENSSL_DH_MAX_MODULUS_BITS) {
        ERR_raise(ERR_LIB_DH, DH_R_MODULUS_TOO_LARGE);
        goto err;
    }

    if (dh->params.q != NULL
        && BN_num_bits(dh->params.q) > OPENSSL_DH_MAX_MODULUS_BITS) {
        ERR_raise(ERR_LIB_DH, DH_R_Q_TOO_LARGE);
        goto err;
    }

    if (BN_num_bits(dh->params.p) < DH_MIN_MODULUS_BITS) {
        ERR_raise(ERR_LIB_DH, DH_R_MODULUS_TOO_SMALL);
        return 0;
    }

    ctx = BN_CTX_new_ex(dh->libctx);
    if (ctx == NULL)
        goto err;
    BN_CTX_start(ctx);
    pminus1 = BN_CTX_get(ctx);
    z = BN_CTX_get(ctx);
    if (z == NULL)
        goto err;

    if (dh->priv_key == NULL) {
        ERR_raise(ERR_LIB_DH, DH_R_NO_PRIVATE_VALUE);
        goto err;
    }

    if (dh->flags & DH_FLAG_CACHE_MONT_P) {
        mont = BN_MONT_CTX_set_locked(&dh->method_mont_p,
                                      dh->lock, dh->params.p, ctx);
        BN_set_flags(dh->priv_key, BN_FLG_CONSTTIME);
        if (!mont)
            goto err;
    }

    /* (Step 1) Z = pub_key^priv_key mod p */
    if (!dh->meth->bn_mod_exp(dh, z, pub_key, dh->priv_key, dh->params.p, ctx,
                              mont)) {
        ERR_raise(ERR_LIB_DH, ERR_R_BN_LIB);
        goto err;
    }

    /* (Step 2) Error if z <= 1 or z = p - 1 */
    if (BN_copy(pminus1, dh->params.p) == NULL
        || !BN_sub_word(pminus1, 1)
        || BN_cmp(z, BN_value_one()) <= 0
        || BN_cmp(z, pminus1) == 0) {
        ERR_raise(ERR_LIB_DH, DH_R_INVALID_SECRET);
        goto err;
    }

    /* return the padded key, i.e. same number of bytes as the modulus */
    ret = BN_bn2binpad(z, key, BN_num_bytes(dh->params.p));
 err:
    BN_clear(z); /* (Step 2) destroy intermediate values */
    BN_CTX_end(ctx);
    BN_CTX_free(ctx);
    return ret;
}

/*-
 * NB: This function is inherently not constant time due to the
 * RFC 5246 (8.1.2) padding style that strips leading zero bytes.
 */
int DH_compute_key(unsigned char *key, const BIGNUM *pub_key, DH *dh)
{
    int ret = 0, i;
    volatile size_t npad = 0, mask = 1;

    /* compute the key; ret is constant unless compute_key is external */
#ifdef FIPS_MODULE
    ret = ossl_dh_compute_key(key, pub_key, dh);
#else
    ret = dh->meth->compute_key(key, pub_key, dh);
#endif
    if (ret <= 0)
        return ret;

    /* count leading zero bytes, yet still touch all bytes */
    for (i = 0; i < ret; i++) {
        mask &= !key[i];
        npad += mask;
    }

    /* unpad key */
    ret -= npad;
    /* key-dependent memory access, potentially leaking npad / ret */
    memmove(key, key + npad, ret);
    /* key-dependent memory access, potentially leaking npad / ret */
    memset(key + ret, 0, npad);

    return ret;
}

int DH_compute_key_padded(unsigned char *key, const BIGNUM *pub_key, DH *dh)
{
    int rv, pad;

    /* rv is constant unless compute_key is external */
#ifdef FIPS_MODULE
    rv = ossl_dh_compute_key(key, pub_key, dh);
#else
    rv = dh->meth->compute_key(key, pub_key, dh);
#endif
    if (rv <= 0)
        return rv;
    pad = BN_num_bytes(dh->params.p) - rv;
    /* pad is constant (zero) unless compute_key is external */
    if (pad > 0) {
        memmove(key + pad, key, rv);
        memset(key, 0, pad);
    }
    return rv + pad;
}

static DH_METHOD dh_ossl = {
    "OpenSSL DH Method",
    generate_key,
    ossl_dh_compute_key,
    dh_bn_mod_exp,
    dh_init,
    dh_finish,
    DH_FLAG_FIPS_METHOD,
    NULL,
    NULL
};

static const DH_METHOD *default_DH_method = &dh_ossl;

const DH_METHOD *DH_OpenSSL(void)
{
    return &dh_ossl;
}

const DH_METHOD *DH_get_default_method(void)
{
    return default_DH_method;
}

static int dh_bn_mod_exp(const DH *dh, BIGNUM *r,
                         const BIGNUM *a, const BIGNUM *p,
                         const BIGNUM *m, BN_CTX *ctx, BN_MONT_CTX *m_ctx)
{
#ifdef S390X_MOD_EXP
    return s390x_mod_exp(r, a, p, m, ctx, m_ctx);
#else
    return BN_mod_exp_mont(r, a, p, m, ctx, m_ctx);
#endif
}

static int dh_init(DH *dh)
{
    dh->flags |= DH_FLAG_CACHE_MONT_P;
    dh->dirty_cnt++;
    return 1;
}

static int dh_finish(DH *dh)
{
    BN_MONT_CTX_free(dh->method_mont_p);
    return 1;
}

#ifndef FIPS_MODULE
void DH_set_default_method(const DH_METHOD *meth)
{
    default_DH_method = meth;
}
#endif /* FIPS_MODULE */

int DH_generate_key(DH *dh)
{
#ifdef FIPS_MODULE
    return generate_key(dh);
#else
    return dh->meth->generate_key(dh);
#endif
}

int ossl_dh_generate_public_key(BN_CTX *ctx, const DH *dh,
                                const BIGNUM *priv_key, BIGNUM *pub_key)
{
    int ret = 0;
    BIGNUM *prk = BN_new();
    BN_MONT_CTX *mont = NULL;

    if (prk == NULL)
        return 0;

    if (dh->flags & DH_FLAG_CACHE_MONT_P) {
        /*
         * We take the input DH as const, but we lie, because in some cases we
         * want to get a hold of its Montgomery context.
         *
         * We cast to remove the const qualifier in this case, it should be
         * fine...
         */
        BN_MONT_CTX **pmont = (BN_MONT_CTX **)&dh->method_mont_p;

        mont = BN_MONT_CTX_set_locked(pmont, dh->lock, dh->params.p, ctx);
        if (mont == NULL)
            goto err;
    }
    BN_with_flags(prk, priv_key, BN_FLG_CONSTTIME);

    /* pub_key = g^priv_key mod p */
    if (!dh->meth->bn_mod_exp(dh, pub_key, dh->params.g, prk, dh->params.p,
                              ctx, mont))
        goto err;
    ret = 1;
err:
    BN_clear_free(prk);
    return ret;
}

static int generate_key(DH *dh)
{
    int ok = 0;
    int generate_new_key = 0;
#ifndef FIPS_MODULE
    unsigned l;
#endif
    BN_CTX *ctx = NULL;
    BIGNUM *pub_key = NULL, *priv_key = NULL;

    if (BN_num_bits(dh->params.p) > OPENSSL_DH_MAX_MODULUS_BITS) {
        ERR_raise(ERR_LIB_DH, DH_R_MODULUS_TOO_LARGE);
        return 0;
    }

    if (dh->params.q != NULL
        && BN_num_bits(dh->params.q) > OPENSSL_DH_MAX_MODULUS_BITS) {
        ERR_raise(ERR_LIB_DH, DH_R_Q_TOO_LARGE);
        return 0;
    }

    if (BN_num_bits(dh->params.p) < DH_MIN_MODULUS_BITS) {
        ERR_raise(ERR_LIB_DH, DH_R_MODULUS_TOO_SMALL);
        return 0;
    }

    ctx = BN_CTX_new_ex(dh->libctx);
    if (ctx == NULL)
        goto err;

    if (dh->priv_key == NULL) {
        priv_key = BN_secure_new();
        if (priv_key == NULL)
            goto err;
        generate_new_key = 1;
    } else {
        priv_key = dh->priv_key;
    }

    if (dh->pub_key == NULL) {
        pub_key = BN_new();
        if (pub_key == NULL)
            goto err;
    } else {
        pub_key = dh->pub_key;
    }
    if (generate_new_key) {
        /* Is it an approved safe prime ?*/
        if (DH_get_nid(dh) != NID_undef) {
            int max_strength =
                    ossl_ifc_ffc_compute_security_bits(BN_num_bits(dh->params.p));

            if (dh->params.q == NULL
                || dh->length > BN_num_bits(dh->params.q))
                goto err;
            /* dh->length = maximum bit length of generated private key */
            if (!ossl_ffc_generate_private_key(ctx, &dh->params, dh->length,
                                               max_strength, priv_key))
                goto err;
        } else {
#ifdef FIPS_MODULE
            if (dh->params.q == NULL)
                goto err;
#else
            if (dh->params.q == NULL) {
                /* secret exponent length, must satisfy 2^(l-1) <= p */
                if (dh->length != 0
                    && dh->length >= BN_num_bits(dh->params.p))
                    goto err;
                l = dh->length ? dh->length : BN_num_bits(dh->params.p) - 1;
                if (!BN_priv_rand_ex(priv_key, l, BN_RAND_TOP_ONE,
                                     BN_RAND_BOTTOM_ANY, 0, ctx))
                    goto err;
                /*
                 * We handle just one known case where g is a quadratic non-residue:
                 * for g = 2: p % 8 == 3
                 */
                if (BN_is_word(dh->params.g, DH_GENERATOR_2)
                    && !BN_is_bit_set(dh->params.p, 2)) {
                    /* clear bit 0, since it won't be a secret anyway */
                    if (!BN_clear_bit(priv_key, 0))
                        goto err;
                }
            } else
#endif
            {
                /* Do a partial check for invalid p, q, g */
                if (!ossl_ffc_params_simple_validate(dh->libctx, &dh->params,
                                                     FFC_PARAM_TYPE_DH, NULL))
                    goto err;
                /*
                 * For FFC FIPS 186-4 keygen
                 * security strength s = 112,
                 * Max Private key size N = len(q)
                 */
                if (!ossl_ffc_generate_private_key(ctx, &dh->params,
                                                   BN_num_bits(dh->params.q),
                                                   MIN_STRENGTH,
                                                   priv_key))
                    goto err;
            }
        }
    }

    if (!ossl_dh_generate_public_key(ctx, dh, priv_key, pub_key))
        goto err;

    dh->pub_key = pub_key;
    dh->priv_key = priv_key;
    dh->dirty_cnt++;
    ok = 1;
 err:
    if (ok != 1)
        ERR_raise(ERR_LIB_DH, ERR_R_BN_LIB);

    if (pub_key != dh->pub_key)
        BN_free(pub_key);
    if (priv_key != dh->priv_key)
        BN_free(priv_key);
    BN_CTX_free(ctx);
    return ok;
}

int ossl_dh_buf2key(DH *dh, const unsigned char *buf, size_t len)
{
    int err_reason = DH_R_BN_ERROR;
    BIGNUM *pubkey = NULL;
    const BIGNUM *p;
    int ret;

    if ((pubkey = BN_bin2bn(buf, len, NULL)) == NULL)
        goto err;
    DH_get0_pqg(dh, &p, NULL, NULL);
    if (p == NULL || BN_num_bytes(p) == 0) {
        err_reason = DH_R_NO_PARAMETERS_SET;
        goto err;
    }
    /* Prevent small subgroup attacks per RFC 8446 Section 4.2.8.1 */
    if (!ossl_dh_check_pub_key_partial(dh, pubkey, &ret)) {
        err_reason = DH_R_INVALID_PUBKEY;
        goto err;
    }
    if (DH_set0_key(dh, pubkey, NULL) != 1)
        goto err;
    return 1;
err:
    ERR_raise(ERR_LIB_DH, err_reason);
    BN_free(pubkey);
    return 0;
}

size_t ossl_dh_key2buf(const DH *dh, unsigned char **pbuf_out, size_t size,
                       int alloc)
{
    const BIGNUM *pubkey;
    unsigned char *pbuf = NULL;
    const BIGNUM *p;
    int p_size;

    DH_get0_pqg(dh, &p, NULL, NULL);
    DH_get0_key(dh, &pubkey, NULL);
    if (p == NULL || pubkey == NULL
            || (p_size = BN_num_bytes(p)) == 0
            || BN_num_bytes(pubkey) == 0) {
        ERR_raise(ERR_LIB_DH, DH_R_INVALID_PUBKEY);
        return 0;
    }
    if (pbuf_out != NULL && (alloc || *pbuf_out != NULL)) {
        if (!alloc) {
            if (size >= (size_t)p_size)
                pbuf = *pbuf_out;
            if (pbuf == NULL)
                ERR_raise(ERR_LIB_DH, DH_R_INVALID_SIZE);
        } else {
            pbuf = OPENSSL_malloc(p_size);
        }

        /* Errors raised above */
        if (pbuf == NULL)
            return 0;
        /*
         * As per Section 4.2.8.1 of RFC 8446 left pad public
         * key with zeros to the size of p
         */
        if (BN_bn2binpad(pubkey, pbuf, p_size) < 0) {
            if (alloc)
                OPENSSL_free(pbuf);
            ERR_raise(ERR_LIB_DH, DH_R_BN_ERROR);
            return 0;
        }
        *pbuf_out = pbuf;
    }
    return p_size;
}
