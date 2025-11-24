/*
 * Copyright 1995-2023 The OpenSSL Project Authors. All Rights Reserved.
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
#include <time.h>
#include "internal/cryptlib.h"
#include <openssl/bn.h>
#include <openssl/self_test.h>
#include "prov/providercommon.h"
#include "crypto/dsa.h"
#include "dsa_local.h"

#ifdef FIPS_MODULE
# define MIN_STRENGTH 112
#else
# define MIN_STRENGTH 80
#endif

static int dsa_keygen(DSA *dsa);

int DSA_generate_key(DSA *dsa)
{
#ifndef FIPS_MODULE
    if (dsa->meth->dsa_keygen != NULL)
        return dsa->meth->dsa_keygen(dsa);
#endif
    return dsa_keygen(dsa);
}

int ossl_dsa_generate_public_key(BN_CTX *ctx, const DSA *dsa,
                                 const BIGNUM *priv_key, BIGNUM *pub_key)
{
    int ret = 0;
    BIGNUM *prk = BN_new();

    if (prk == NULL)
        return 0;
    BN_with_flags(prk, priv_key, BN_FLG_CONSTTIME);

    /* pub_key = g ^ priv_key mod p */
    if (!BN_mod_exp(pub_key, dsa->params.g, prk, dsa->params.p, ctx))
        goto err;
    ret = 1;
err:
    BN_clear_free(prk);
    return ret;
}

#ifdef FIPS_MODULE
/*
 * Refer: FIPS 140-3 IG 10.3.A Additional Comment 1
 * Perform a KAT by duplicating the public key generation.
 *
 * NOTE: This issue requires a background understanding, provided in a separate
 * document; the current IG 10.3.A AC1 is insufficient regarding the PCT for
 * the key agreement scenario.
 *
 * Currently IG 10.3.A requires PCT in the mode of use prior to use of the
 * key pair, citing the PCT defined in the associated standard. For key
 * agreement, the only PCT defined in SP 800-56A is that of Section 5.6.2.4:
 * the comparison of the original public key to a newly calculated public key.
 */
static int dsa_keygen_knownanswer_test(DSA *dsa, BN_CTX *ctx,
                                       OSSL_CALLBACK *cb, void *cbarg)
{
    int len, ret = 0;
    OSSL_SELF_TEST *st = NULL;
    unsigned char bytes[512] = {0};
    BIGNUM *pub_key2 = BN_new();

    if (pub_key2 == NULL)
        return 0;

    st = OSSL_SELF_TEST_new(cb, cbarg);
    if (st == NULL)
        goto err;

    OSSL_SELF_TEST_onbegin(st, OSSL_SELF_TEST_TYPE_PCT_KAT,
                               OSSL_SELF_TEST_DESC_PCT_DSA);

    if (!ossl_dsa_generate_public_key(ctx, dsa, dsa->priv_key, pub_key2))
        goto err;

    if (BN_num_bytes(pub_key2) > (int)sizeof(bytes))
        goto err;
    len = BN_bn2bin(pub_key2, bytes);
    OSSL_SELF_TEST_oncorrupt_byte(st, bytes);
    if (BN_bin2bn(bytes, len, pub_key2) != NULL)
        ret = !BN_cmp(dsa->pub_key, pub_key2);

err:
    OSSL_SELF_TEST_onend(st, ret);
    OSSL_SELF_TEST_free(st);
    BN_free(pub_key2);
    return ret;
}

/*
 * FIPS 140-2 IG 9.9 AS09.33
 * Perform a sign/verify operation.
 */
static int dsa_keygen_pairwise_test(DSA *dsa, OSSL_CALLBACK *cb, void *cbarg)
{
    int ret = 0;
    unsigned char dgst[16] = {0};
    unsigned int dgst_len = (unsigned int)sizeof(dgst);
    DSA_SIG *sig = NULL;
    OSSL_SELF_TEST *st = NULL;

    st = OSSL_SELF_TEST_new(cb, cbarg);
    if (st == NULL)
        goto err;

    OSSL_SELF_TEST_onbegin(st, OSSL_SELF_TEST_TYPE_PCT,
                           OSSL_SELF_TEST_DESC_PCT_DSA);

    sig = DSA_do_sign(dgst, (int)dgst_len, dsa);
    if (sig == NULL)
        goto err;

    OSSL_SELF_TEST_oncorrupt_byte(st, dgst);

    if (DSA_do_verify(dgst, dgst_len, sig, dsa) != 1)
        goto err;

    ret = 1;
err:
    OSSL_SELF_TEST_onend(st, ret);
    OSSL_SELF_TEST_free(st);
    DSA_SIG_free(sig);
    return ret;
}
#endif /* FIPS_MODULE */

static int dsa_keygen(DSA *dsa)
{
    int ok = 0;
    BN_CTX *ctx = NULL;
    BIGNUM *pub_key = NULL, *priv_key = NULL;

    if ((ctx = BN_CTX_new_ex(dsa->libctx)) == NULL)
        goto err;

    if (dsa->priv_key == NULL) {
        if ((priv_key = BN_secure_new()) == NULL)
            goto err;
    } else {
        priv_key = dsa->priv_key;
    }

    /* Do a partial check for invalid p, q, g */
    if (!ossl_ffc_params_simple_validate(dsa->libctx, &dsa->params,
                                         FFC_PARAM_TYPE_DSA, NULL))
        goto err;

    /*
     * For FFC FIPS 186-4 keygen
     * security strength s = 112,
     * Max Private key size N = len(q)
     */
    if (!ossl_ffc_generate_private_key(ctx, &dsa->params,
                                       BN_num_bits(dsa->params.q),
                                       MIN_STRENGTH, priv_key))
        goto err;

    if (dsa->pub_key == NULL) {
        if ((pub_key = BN_new()) == NULL)
            goto err;
    } else {
        pub_key = dsa->pub_key;
    }

    if (!ossl_dsa_generate_public_key(ctx, dsa, priv_key, pub_key))
        goto err;

    dsa->priv_key = priv_key;
    dsa->pub_key = pub_key;

    ok = 1;
#ifdef FIPS_MODULE
    {
        OSSL_CALLBACK *cb = NULL;
        void *cbarg = NULL;

        OSSL_SELF_TEST_get_callback(dsa->libctx, &cb, &cbarg);
        ok = dsa_keygen_pairwise_test(dsa, cb, cbarg)
             && dsa_keygen_knownanswer_test(dsa, ctx, cb, cbarg);
        if (!ok) {
            ossl_set_error_state(OSSL_SELF_TEST_TYPE_PCT);
            BN_free(dsa->pub_key);
            BN_clear_free(dsa->priv_key);
            dsa->pub_key = NULL;
            dsa->priv_key = NULL;
            BN_CTX_free(ctx);
            return ok;
        }
    }
#endif
    dsa->dirty_cnt++;

 err:
    if (pub_key != dsa->pub_key)
        BN_free(pub_key);
    if (priv_key != dsa->priv_key)
        BN_free(priv_key);
    BN_CTX_free(ctx);

    return ok;
}
