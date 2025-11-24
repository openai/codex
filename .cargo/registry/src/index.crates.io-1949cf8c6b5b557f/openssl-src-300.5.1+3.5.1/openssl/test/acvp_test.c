/*
 * Copyright 2020-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * A set of tests demonstrating uses cases for CAVS/ACVP testing.
 *
 * For examples of testing KDF's, Digests, KeyAgreement & DRBG's refer to
 * providers/fips/self_test_kats.c
 */

#include <string.h>
#include <openssl/opensslconf.h> /* To see if OPENSSL_NO_EC is defined */
#include <openssl/core_names.h>
#include <openssl/evp.h>
#include <openssl/ec.h>
#include <openssl/dh.h>
#include <openssl/dsa.h>
#include <openssl/rsa.h>
#include <openssl/param_build.h>
#include <openssl/provider.h>
#include <openssl/self_test.h>
#include "testutil.h"
#include "testutil/output.h"
#include "acvp_test.inc"
#include "internal/nelem.h"

typedef enum OPTION_choice {
    OPT_ERR = -1,
    OPT_EOF = 0,
    OPT_CONFIG_FILE,
    OPT_TEST_ENUM
} OPTION_CHOICE;

typedef struct st_args {
    int enable;
    int called;
} SELF_TEST_ARGS;

static OSSL_PROVIDER *prov_null = NULL;
static OSSL_LIB_CTX *libctx = NULL;
static SELF_TEST_ARGS self_test_args = { 0 };
static OSSL_CALLBACK self_test_events;
static int pass_sig_gen_params = 1;
static int rsa_sign_x931_pad_allowed = 1;
#ifndef OPENSSL_NO_DSA
static int dsasign_allowed = 1;
#endif
#ifndef OPENSSL_NO_EC
static int ec_cofactors = 1;
#endif

const OPTIONS *test_get_options(void)
{
    static const OPTIONS test_options[] = {
        OPT_TEST_OPTIONS_DEFAULT_USAGE,
        { "config", OPT_CONFIG_FILE, '<',
          "The configuration file to use for the libctx" },
        { NULL }
    };
    return test_options;
}

static int pkey_get_bn_bytes(EVP_PKEY *pkey, const char *name,
                             unsigned char **out, size_t *out_len)
{
    unsigned char *buf = NULL;
    BIGNUM *bn = NULL;
    int sz;

    if (!EVP_PKEY_get_bn_param(pkey, name, &bn))
        goto err;
    sz = BN_num_bytes(bn);
    buf = OPENSSL_zalloc(sz);
    if (buf == NULL)
        goto err;
    if (BN_bn2binpad(bn, buf, sz) <= 0)
        goto err;

    *out_len = sz;
    *out = buf;
    BN_free(bn);
    return 1;
err:
    OPENSSL_free(buf);
    BN_free(bn);
    return 0;
}

static int sig_gen(EVP_PKEY *pkey, OSSL_PARAM *params, const char *digest_name,
                   const unsigned char *msg, size_t msg_len,
                   unsigned char **sig_out, size_t *sig_out_len)
{
    int ret = 0;
    EVP_MD_CTX *md_ctx = NULL;
    unsigned char *sig = NULL;
    size_t sig_len;
    size_t sz = EVP_PKEY_get_size(pkey);
    OSSL_PARAM *p = pass_sig_gen_params ? params : NULL;

    sig_len = sz;
    if (!TEST_ptr(sig = OPENSSL_malloc(sz))
        || !TEST_ptr(md_ctx = EVP_MD_CTX_new())
        || !TEST_int_eq(EVP_DigestSignInit_ex(md_ctx, NULL, digest_name, libctx,
                                              NULL, pkey, p), 1)
        || !TEST_int_gt(EVP_DigestSign(md_ctx, sig, &sig_len, msg, msg_len), 0))
        goto err;
    *sig_out = sig;
    *sig_out_len = sig_len;
    sig = NULL;
    ret = 1;
err:
    OPENSSL_free(sig);
    EVP_MD_CTX_free(md_ctx);
    return ret;
}

static int check_verify_message(EVP_PKEY_CTX *pkey_ctx, int expected)
{
    OSSL_PARAM params[2], *p = params;
    int verify_message = -1;

    if (!OSSL_PROVIDER_available(libctx, "fips")
            || fips_provider_version_match(libctx, "<3.4.0"))
        return 1;

    *p++ = OSSL_PARAM_construct_int(OSSL_SIGNATURE_PARAM_FIPS_VERIFY_MESSAGE,
                                    &verify_message);
    *p = OSSL_PARAM_construct_end();

    if (!TEST_true(EVP_PKEY_CTX_get_params(pkey_ctx, params))
            || !TEST_int_eq(verify_message, expected))
        return 0;
    return 1;
}

#ifndef OPENSSL_NO_EC
static int ecdsa_keygen_test(int id)
{
    int ret = 0;
    EVP_PKEY *pkey = NULL;
    unsigned char *priv = NULL;
    unsigned char *pubx = NULL, *puby = NULL;
    size_t priv_len = 0, pubx_len = 0, puby_len = 0;
    const struct ecdsa_keygen_st *tst = &ecdsa_keygen_data[id];

    self_test_args.called = 0;
    self_test_args.enable = 1;
    if (!TEST_ptr(pkey = EVP_PKEY_Q_keygen(libctx, NULL, "EC", tst->curve_name))
        || !TEST_int_ge(self_test_args.called, 3)
        || !TEST_true(pkey_get_bn_bytes(pkey, OSSL_PKEY_PARAM_PRIV_KEY, &priv,
                                        &priv_len))
        || !TEST_true(pkey_get_bn_bytes(pkey, OSSL_PKEY_PARAM_EC_PUB_X, &pubx,
                                        &pubx_len))
        || !TEST_true(pkey_get_bn_bytes(pkey, OSSL_PKEY_PARAM_EC_PUB_Y, &puby,
                                        &puby_len)))
        goto err;

    test_output_memory("qy", puby, puby_len);
    test_output_memory("qx", pubx, pubx_len);
    test_output_memory("d", priv, priv_len);
    ret = 1;
err:
    self_test_args.enable = 0;
    self_test_args.called = 0;
    OPENSSL_clear_free(priv, priv_len);
    OPENSSL_free(pubx);
    OPENSSL_free(puby);
    EVP_PKEY_free(pkey);
    return ret;
}

static int ecdsa_create_pkey(EVP_PKEY **pkey, const char *curve_name,
                             const unsigned char *pub, size_t pub_len,
                             int expected)
{
    int ret = 0;
    EVP_PKEY_CTX *ctx = NULL;
    OSSL_PARAM_BLD *bld = NULL;
    OSSL_PARAM *params = NULL;

    if (!TEST_ptr(bld = OSSL_PARAM_BLD_new())
        || (curve_name != NULL
            && !TEST_true(OSSL_PARAM_BLD_push_utf8_string(
                              bld, OSSL_PKEY_PARAM_GROUP_NAME, curve_name, 0) > 0))
        || !TEST_true(OSSL_PARAM_BLD_push_octet_string(bld,
                                                       OSSL_PKEY_PARAM_PUB_KEY,
                                                       pub, pub_len) > 0)
        || !TEST_ptr(params = OSSL_PARAM_BLD_to_param(bld))
        || !TEST_ptr(ctx = EVP_PKEY_CTX_new_from_name(libctx, "EC", NULL))
        || !TEST_int_eq(EVP_PKEY_fromdata_init(ctx), 1)
        || !TEST_int_eq(EVP_PKEY_fromdata(ctx, pkey, EVP_PKEY_PUBLIC_KEY,
                                          params), expected))
    goto err;

    ret = 1;
err:
    OSSL_PARAM_free(params);
    OSSL_PARAM_BLD_free(bld);
    EVP_PKEY_CTX_free(ctx);
    return ret;
}

static int ecdsa_pub_verify_test(int id)
{
    const struct ecdsa_pub_verify_st *tst = &ecdsa_pv_data[id];

    int ret = 0;
    EVP_PKEY_CTX *key_ctx = NULL;
    EVP_PKEY *pkey = NULL;

    if (!TEST_true(ecdsa_create_pkey(&pkey, tst->curve_name,
                                     tst->pub, tst->pub_len, tst->pass)))
        goto err;

    if (tst->pass) {
        if (!TEST_ptr(key_ctx = EVP_PKEY_CTX_new_from_pkey(libctx, pkey, ""))
            || !TEST_int_eq(EVP_PKEY_public_check(key_ctx), tst->pass))
            goto err;
    }
    ret = 1;
err:
    EVP_PKEY_free(pkey);
    EVP_PKEY_CTX_free(key_ctx);
    return ret;
}

/* Extract r and s  from an ecdsa signature */
static int get_ecdsa_sig_rs_bytes(const unsigned char *sig, size_t sig_len,
                                  unsigned char **r, unsigned char **s,
                                  size_t *rlen, size_t *slen)
{
    int ret = 0;
    unsigned char *rbuf = NULL, *sbuf = NULL;
    size_t r1_len, s1_len;
    const BIGNUM *r1, *s1;
    ECDSA_SIG *sign = d2i_ECDSA_SIG(NULL, &sig, sig_len);

    if (sign == NULL)
        return 0;
    r1 = ECDSA_SIG_get0_r(sign);
    s1 = ECDSA_SIG_get0_s(sign);
    if (r1 == NULL || s1 == NULL)
        goto err;

    r1_len = BN_num_bytes(r1);
    s1_len = BN_num_bytes(s1);
    rbuf = OPENSSL_zalloc(r1_len);
    sbuf = OPENSSL_zalloc(s1_len);
    if (rbuf == NULL || sbuf == NULL)
        goto err;
    if (BN_bn2binpad(r1, rbuf, r1_len) <= 0)
        goto err;
    if (BN_bn2binpad(s1, sbuf, s1_len) <= 0)
        goto err;
    *r = rbuf;
    *s = sbuf;
    *rlen = r1_len;
    *slen = s1_len;
    ret = 1;
err:
    if (ret == 0) {
        OPENSSL_free(rbuf);
        OPENSSL_free(sbuf);
    }
    ECDSA_SIG_free(sign);
    return ret;
}

static int ecdsa_siggen_test(int id)
{
    int ret = 0;
    EVP_PKEY *pkey = NULL;
    size_t sig_len = 0, rlen = 0, slen = 0;
    unsigned char *sig = NULL;
    unsigned char *r = NULL, *s = NULL;
    const struct ecdsa_siggen_st *tst = &ecdsa_siggen_data[id];

    if (!TEST_ptr(pkey = EVP_PKEY_Q_keygen(libctx, NULL, "EC", tst->curve_name)))
        goto err;

    if (!TEST_true(sig_gen(pkey, NULL, tst->digest_alg, tst->msg, tst->msg_len,
                           &sig, &sig_len))
        || !TEST_true(get_ecdsa_sig_rs_bytes(sig, sig_len, &r, &s, &rlen, &slen)))
        goto err;
    test_output_memory("r", r, rlen);
    test_output_memory("s", s, slen);
    ret = 1;
err:
    OPENSSL_free(r);
    OPENSSL_free(s);
    OPENSSL_free(sig);
    EVP_PKEY_free(pkey);
    return ret;
}

static int ecdsa_sigver_test(int id)
{
    int ret = 0;
    EVP_MD_CTX *md_ctx = NULL;
    EVP_PKEY *pkey = NULL;
    EVP_PKEY_CTX *pkey_ctx;
    ECDSA_SIG *sign = NULL;
    size_t sig_len;
    unsigned char *sig = NULL;
    BIGNUM *rbn = NULL, *sbn = NULL;
    const struct ecdsa_sigver_st *tst = &ecdsa_sigver_data[id];

    if (!TEST_true(ecdsa_create_pkey(&pkey, tst->curve_name,
                                     tst->pub, tst->pub_len, 1)))
        goto err;

    if (!TEST_ptr(sign = ECDSA_SIG_new())
        || !TEST_ptr(rbn = BN_bin2bn(tst->r, tst->r_len, NULL))
        || !TEST_ptr(sbn = BN_bin2bn(tst->s, tst->s_len, NULL))
        || !TEST_true(ECDSA_SIG_set0(sign, rbn, sbn)))
        goto err;
    rbn = sbn = NULL;

    if (!TEST_int_gt((sig_len = i2d_ECDSA_SIG(sign, &sig)), 0)
        || !TEST_ptr(md_ctx = EVP_MD_CTX_new())
        || !TEST_true(EVP_DigestVerifyInit_ex(md_ctx, NULL, tst->digest_alg,
                                              libctx, NULL, pkey, NULL))
        || !TEST_ptr(pkey_ctx = EVP_MD_CTX_get_pkey_ctx(md_ctx))
        || !check_verify_message(pkey_ctx, 1)
        || !TEST_int_eq(EVP_DigestVerify(md_ctx, sig, sig_len,
                                         tst->msg, tst->msg_len), tst->pass)
        || !check_verify_message(pkey_ctx, 1)
        || !TEST_true(EVP_PKEY_verify_init(pkey_ctx))
        || !check_verify_message(pkey_ctx, 0))
        goto err;

    ret = 1;
err:
    BN_free(rbn);
    BN_free(sbn);
    OPENSSL_free(sig);
    ECDSA_SIG_free(sign);
    EVP_PKEY_free(pkey);
    EVP_MD_CTX_free(md_ctx);
    return ret;

}

static int ecdh_cofactor_derive_test(int tstid)
{
    int ret = 0;
    const struct ecdh_cofactor_derive_st *t = &ecdh_cofactor_derive_data[tstid];
    unsigned char secret1[16];
    size_t secret1_len = sizeof(secret1);
    const char *curve = "K-283"; /* A curve that has a cofactor that it not 1 */
    EVP_PKEY *peer1 = NULL, *peer2 = NULL;
    EVP_PKEY_CTX *p1ctx = NULL;
    OSSL_PARAM params[2], *prms = NULL;
    int use_cofactordh = t->key_cofactor;
    int cofactor_mode = t->derive_cofactor_mode;

    if (!ec_cofactors)
        return TEST_skip("not supported by FIPS provider version");

    if (!TEST_ptr(peer1 = EVP_PKEY_Q_keygen(libctx, NULL, "EC", curve)))
        return TEST_skip("Curve %s not supported by the FIPS provider", curve);

    if (!TEST_ptr(peer2 = EVP_PKEY_Q_keygen(libctx, NULL, "EC", curve)))
        goto err;

    params[1] = OSSL_PARAM_construct_end();

    prms = NULL;
    if (t->key_cofactor != COFACTOR_NOT_SET) {
        params[0] = OSSL_PARAM_construct_int(OSSL_PKEY_PARAM_USE_COFACTOR_ECDH,
                                             &use_cofactordh);
        prms = params;
    }
    if (!TEST_int_eq(EVP_PKEY_set_params(peer1, prms), 1)
            || !TEST_ptr(p1ctx = EVP_PKEY_CTX_new_from_pkey(libctx, peer1, NULL)))
        goto err;

    prms = NULL;
    if (t->derive_cofactor_mode != COFACTOR_NOT_SET) {
        params[0] = OSSL_PARAM_construct_int(OSSL_EXCHANGE_PARAM_EC_ECDH_COFACTOR_MODE,
                                             &cofactor_mode);
        prms = params;
    }
    if (!TEST_int_eq(EVP_PKEY_derive_init_ex(p1ctx, prms), 1)
            || !TEST_int_eq(EVP_PKEY_derive_set_peer(p1ctx, peer2), 1)
            || !TEST_int_eq(EVP_PKEY_derive(p1ctx, secret1, &secret1_len),
                            t->expected))
        goto err;

    ret = 1;
err:
    if (ret == 0) {
        static const char *state[] = { "unset", "-1", "disabled", "enabled" };

        TEST_note("ECDH derive() was expected to %s if key cofactor is"
                  "%s and derive mode is %s", t->expected ? "Pass" : "Fail",
                  state[2 + t->key_cofactor], state[2 + t->derive_cofactor_mode]);
    }
    EVP_PKEY_free(peer1);
    EVP_PKEY_free(peer2);
    EVP_PKEY_CTX_free(p1ctx);
    return ret;
}

#endif /* OPENSSL_NO_EC */

#if !defined(OPENSSL_NO_DSA) || !defined(OPENSSL_NO_ECX)
static int pkey_get_octet_bytes(EVP_PKEY *pkey, const char *name,
                                unsigned char **out, size_t *out_len)
{
    size_t len = 0;
    unsigned char *buf = NULL;

    if (!EVP_PKEY_get_octet_string_param(pkey, name, NULL, 0, &len))
        goto err;

    buf = OPENSSL_zalloc(len);
    if (buf == NULL)
        goto err;

    if (!EVP_PKEY_get_octet_string_param(pkey, name, buf, len, out_len))
        goto err;
    *out = buf;
    return 1;
err:
    OPENSSL_free(buf);
    return 0;
}
#endif /* !defined(OPENSSL_NO_DSA) || !defined(OPENSSL_NO_ECX) */

#ifndef OPENSSL_NO_ECX
static int eddsa_create_pkey(EVP_PKEY **pkey, const char *algname,
                             const unsigned char *pub, size_t pub_len,
                             int expected)
{
    int ret = 0;
    EVP_PKEY_CTX *ctx = NULL;
    OSSL_PARAM_BLD *bld = NULL;
    OSSL_PARAM *params = NULL;

    if (!TEST_ptr(bld = OSSL_PARAM_BLD_new())
        || !TEST_true(OSSL_PARAM_BLD_push_octet_string(bld,
                                                       OSSL_PKEY_PARAM_PUB_KEY,
                                                       pub, pub_len) > 0)
        || !TEST_ptr(params = OSSL_PARAM_BLD_to_param(bld))
        || !TEST_ptr(ctx = EVP_PKEY_CTX_new_from_name(libctx, algname, NULL))
        || !TEST_int_eq(EVP_PKEY_fromdata_init(ctx), 1)
        || !TEST_int_eq(EVP_PKEY_fromdata(ctx, pkey, EVP_PKEY_PUBLIC_KEY,
                                          params), expected))
        goto err;

    ret = 1;
err:
    OSSL_PARAM_free(params);
    OSSL_PARAM_BLD_free(bld);
    EVP_PKEY_CTX_free(ctx);
    return ret;
}

static int eddsa_pub_verify_test(int id)
{
    const struct ecdsa_pub_verify_st *tst = &eddsa_pv_data[id];
    int ret = 0;
    EVP_PKEY_CTX *key_ctx = NULL;
    EVP_PKEY *pkey = NULL;

    if (!TEST_true(eddsa_create_pkey(&pkey, tst->curve_name,
                                     tst->pub, tst->pub_len, 1)))
        goto err;

    if (!TEST_ptr(key_ctx = EVP_PKEY_CTX_new_from_pkey(libctx, pkey, ""))
            || !TEST_int_eq(EVP_PKEY_public_check(key_ctx), tst->pass))
        goto err;
    ret = 1;
err:
    EVP_PKEY_free(pkey);
    EVP_PKEY_CTX_free(key_ctx);
    return ret;
}

static int eddsa_keygen_test(int id)
{
    int ret = 0;
    EVP_PKEY *pkey = NULL;
    unsigned char *priv = NULL, *pub = NULL;
    size_t priv_len = 0, pub_len = 0;
    const struct ecdsa_pub_verify_st *tst = &eddsa_pv_data[id];

    self_test_args.called = 0;
    self_test_args.enable = 1;
    if (!TEST_ptr(pkey = EVP_PKEY_Q_keygen(libctx, NULL, tst->curve_name))
        || !TEST_int_ge(self_test_args.called, 3)
        || !TEST_true(pkey_get_octet_bytes(pkey, OSSL_PKEY_PARAM_PRIV_KEY,
                                           &priv, &priv_len))
        || !TEST_true(pkey_get_octet_bytes(pkey, OSSL_PKEY_PARAM_PUB_KEY, &pub,
                                           &pub_len)))
        goto err;

    test_output_memory("q", pub, pub_len);
    test_output_memory("d", priv, priv_len);
    ret = 1;
err:
    self_test_args.enable = 0;
    self_test_args.called = 0;
    OPENSSL_clear_free(priv, priv_len);
    OPENSSL_free(pub);
    EVP_PKEY_free(pkey);
    return ret;
}

#endif /* OPENSSL_NO_ECX */

#ifndef OPENSSL_NO_DSA

static EVP_PKEY *dsa_paramgen(int L, int N)
{
    EVP_PKEY_CTX *paramgen_ctx = NULL;
    EVP_PKEY *param_key = NULL;

    if (!TEST_ptr(paramgen_ctx = EVP_PKEY_CTX_new_from_name(libctx, "DSA", NULL))
        || !TEST_int_gt(EVP_PKEY_paramgen_init(paramgen_ctx), 0)
        || !TEST_true(EVP_PKEY_CTX_set_dsa_paramgen_bits(paramgen_ctx, L))
        || !TEST_true(EVP_PKEY_CTX_set_dsa_paramgen_q_bits(paramgen_ctx, N))
        || !TEST_true(EVP_PKEY_paramgen(paramgen_ctx, &param_key)))
        TEST_info("dsa_paramgen failed");
    EVP_PKEY_CTX_free(paramgen_ctx);
    return param_key;
}

static EVP_PKEY *dsa_keygen(int L, int N)
{
    EVP_PKEY *param_key = NULL, *key = NULL;
    EVP_PKEY_CTX *keygen_ctx = NULL;

    if (!TEST_ptr(param_key = dsa_paramgen(L, N))
        || !TEST_ptr(keygen_ctx = EVP_PKEY_CTX_new_from_pkey(libctx, param_key,
                                                             NULL))
        || !TEST_int_gt(EVP_PKEY_keygen_init(keygen_ctx), 0)
        || !TEST_int_gt(EVP_PKEY_keygen(keygen_ctx, &key), 0))
        goto err;
err:
    EVP_PKEY_free(param_key);
    EVP_PKEY_CTX_free(keygen_ctx);
    return key;
}

static int dsa_keygen_test(int id)
{
    int ret = 0, i;
    EVP_PKEY_CTX *paramgen_ctx = NULL, *keygen_ctx = NULL;
    EVP_PKEY *param_key = NULL, *key = NULL;
    unsigned char *priv = NULL, *pub = NULL;
    size_t priv_len = 0, pub_len = 0;
    const struct dsa_paramgen_st *tst = &dsa_keygen_data[id];

    if (!dsasign_allowed)
        return TEST_skip("DSA signing is not allowed");
    if (!TEST_ptr(param_key = dsa_paramgen(tst->L, tst->N))
        || !TEST_ptr(keygen_ctx = EVP_PKEY_CTX_new_from_pkey(libctx, param_key,
                                                             NULL))
        || !TEST_int_gt(EVP_PKEY_keygen_init(keygen_ctx), 0))
        goto err;
    for (i = 0; i < 2; ++i) {
        if (!TEST_int_gt(EVP_PKEY_keygen(keygen_ctx, &key), 0)
            || !TEST_true(pkey_get_bn_bytes(key, OSSL_PKEY_PARAM_PRIV_KEY,
                                            &priv, &priv_len))
            || !TEST_true(pkey_get_bn_bytes(key, OSSL_PKEY_PARAM_PUB_KEY,
                                            &pub, &pub_len)))
            goto err;
        test_output_memory("y", pub, pub_len);
        test_output_memory("x", priv, priv_len);
        EVP_PKEY_free(key);
        OPENSSL_clear_free(priv, priv_len);
        OPENSSL_free(pub);
        key = NULL;
        pub = priv = NULL;
    }
    ret = 1;
err:
    OPENSSL_clear_free(priv, priv_len);
    OPENSSL_free(pub);
    EVP_PKEY_free(param_key);
    EVP_PKEY_free(key);
    EVP_PKEY_CTX_free(keygen_ctx);
    EVP_PKEY_CTX_free(paramgen_ctx);
    return ret;
}

static int dsa_paramgen_test(int id)
{
    int ret = 0, counter = 0;
    EVP_PKEY_CTX *paramgen_ctx = NULL;
    EVP_PKEY *param_key = NULL;
    unsigned char *p = NULL, *q = NULL, *seed = NULL;
    size_t plen = 0, qlen = 0, seedlen = 0;
    const struct dsa_paramgen_st *tst = &dsa_paramgen_data[id];

    if (!TEST_ptr(paramgen_ctx = EVP_PKEY_CTX_new_from_name(libctx, "DSA", NULL))
        || !TEST_int_gt(EVP_PKEY_paramgen_init(paramgen_ctx), 0)
        || !TEST_true(EVP_PKEY_CTX_set_dsa_paramgen_bits(paramgen_ctx, tst->L))
        || !TEST_true(EVP_PKEY_CTX_set_dsa_paramgen_q_bits(paramgen_ctx,
                                                           tst->N)))
        goto err;

    if (!dsasign_allowed) {
        if (!TEST_false(EVP_PKEY_paramgen(paramgen_ctx, &param_key)))
            goto err;
    } else {
        if (!TEST_true(EVP_PKEY_paramgen(paramgen_ctx, &param_key))
            || !TEST_true(pkey_get_bn_bytes(param_key, OSSL_PKEY_PARAM_FFC_P,
                                            &p, &plen))
            || !TEST_true(pkey_get_bn_bytes(param_key, OSSL_PKEY_PARAM_FFC_Q,
                                            &q, &qlen))
            || !TEST_true(pkey_get_octet_bytes(param_key,
                                               OSSL_PKEY_PARAM_FFC_SEED,
                                               &seed, &seedlen))
            || !TEST_true(EVP_PKEY_get_int_param(param_key,
                                                 OSSL_PKEY_PARAM_FFC_PCOUNTER,
                                                 &counter)))
            goto err;
        test_output_memory("p", p, plen);
        test_output_memory("q", q, qlen);
        test_output_memory("domainSeed", seed, seedlen);
        test_printf_stderr("%s: %d\n", "counter", counter);
    }
    ret = 1;
err:
    OPENSSL_free(p);
    OPENSSL_free(q);
    OPENSSL_free(seed);
    EVP_PKEY_free(param_key);
    EVP_PKEY_CTX_free(paramgen_ctx);
    return ret;
}

static int dsa_create_pkey(EVP_PKEY **pkey,
                           const unsigned char *p, size_t p_len,
                           const unsigned char *q, size_t q_len,
                           const unsigned char *g, size_t g_len,
                           const unsigned char *seed, size_t seed_len,
                           int counter,
                           int validate_pq, int validate_g,
                           const unsigned char *pub, size_t pub_len,
                           BN_CTX *bn_ctx)
{
    int ret = 0;
    EVP_PKEY_CTX *ctx = NULL;
    OSSL_PARAM_BLD *bld = NULL;
    OSSL_PARAM *params = NULL;
    BIGNUM *p_bn = NULL, *q_bn = NULL, *g_bn = NULL, *pub_bn = NULL;

    if (!TEST_ptr(bld = OSSL_PARAM_BLD_new())
        || !TEST_ptr(p_bn = BN_CTX_get(bn_ctx))
        || !TEST_ptr(BN_bin2bn(p, p_len, p_bn))
        || !TEST_true(OSSL_PARAM_BLD_push_int(bld,
                                              OSSL_PKEY_PARAM_FFC_VALIDATE_PQ,
                                              validate_pq))
        || !TEST_true(OSSL_PARAM_BLD_push_int(bld,
                                              OSSL_PKEY_PARAM_FFC_VALIDATE_G,
                                              validate_g))
        || !TEST_true(OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_FFC_P, p_bn))
        || !TEST_ptr(q_bn = BN_CTX_get(bn_ctx))
        || !TEST_ptr(BN_bin2bn(q, q_len, q_bn))
        || !TEST_true(OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_FFC_Q, q_bn)))
        goto err;

     if (g != NULL) {
         if (!TEST_ptr(g_bn = BN_CTX_get(bn_ctx))
             || !TEST_ptr(BN_bin2bn(g, g_len, g_bn))
             || !TEST_true(OSSL_PARAM_BLD_push_BN(bld,
                                                  OSSL_PKEY_PARAM_FFC_G, g_bn)))
             goto err;
     }
     if (seed != NULL) {
         if (!TEST_true(OSSL_PARAM_BLD_push_octet_string(bld,
                            OSSL_PKEY_PARAM_FFC_SEED, seed, seed_len)))
             goto err;
     }
     if (counter != -1) {
         if (!TEST_true(OSSL_PARAM_BLD_push_int(bld,
                                                OSSL_PKEY_PARAM_FFC_PCOUNTER,
                                                counter)))
             goto err;
     }
     if (pub != NULL) {
         if (!TEST_ptr(pub_bn = BN_CTX_get(bn_ctx))
             || !TEST_ptr(BN_bin2bn(pub, pub_len, pub_bn))
             || !TEST_true(OSSL_PARAM_BLD_push_BN(bld,
                                                  OSSL_PKEY_PARAM_PUB_KEY,
                                                  pub_bn)))
             goto err;
     }
     if (!TEST_ptr(params = OSSL_PARAM_BLD_to_param(bld))
         || !TEST_ptr(ctx = EVP_PKEY_CTX_new_from_name(libctx, "DSA", NULL))
         || !TEST_int_eq(EVP_PKEY_fromdata_init(ctx), 1)
         || !TEST_int_eq(EVP_PKEY_fromdata(ctx, pkey, EVP_PKEY_PUBLIC_KEY,
                                           params), 1))
         goto err;

    ret = 1;
err:
    OSSL_PARAM_free(params);
    OSSL_PARAM_BLD_free(bld);
    EVP_PKEY_CTX_free(ctx);
    return ret;
}

static int dsa_pqver_test(int id)
{
    int ret = 0;
    BN_CTX *bn_ctx = NULL;
    EVP_PKEY_CTX *key_ctx = NULL;
    EVP_PKEY *param_key = NULL;
    const struct dsa_pqver_st *tst = &dsa_pqver_data[id];

    if (!TEST_ptr(bn_ctx = BN_CTX_new_ex(libctx))
        || !TEST_true(dsa_create_pkey(&param_key, tst->p, tst->p_len,
                                      tst->q, tst->q_len, NULL, 0,
                                      tst->seed, tst->seed_len, tst->counter,
                                      1, 0,
                                      NULL, 0,
                                      bn_ctx))
        || !TEST_ptr(key_ctx = EVP_PKEY_CTX_new_from_pkey(libctx, param_key,
                                                          NULL))
        || !TEST_int_eq(EVP_PKEY_param_check(key_ctx), tst->pass))
        goto err;

    ret = 1;
err:
    BN_CTX_free(bn_ctx);
    EVP_PKEY_free(param_key);
    EVP_PKEY_CTX_free(key_ctx);
    return ret;
}

/* Extract r and s from a dsa signature */
static int get_dsa_sig_rs_bytes(const unsigned char *sig, size_t sig_len,
                                unsigned char **r, unsigned char **s,
                                size_t *r_len, size_t *s_len)
{
    int ret = 0;
    unsigned char *rbuf = NULL, *sbuf = NULL;
    size_t r1_len, s1_len;
    const BIGNUM *r1, *s1;
    DSA_SIG *sign = d2i_DSA_SIG(NULL, &sig, sig_len);

    if (sign == NULL)
        return 0;
    DSA_SIG_get0(sign, &r1, &s1);
    if (r1 == NULL || s1 == NULL)
        goto err;

    r1_len = BN_num_bytes(r1);
    s1_len = BN_num_bytes(s1);
    rbuf = OPENSSL_zalloc(r1_len);
    sbuf = OPENSSL_zalloc(s1_len);
    if (rbuf == NULL || sbuf == NULL)
        goto err;
    if (BN_bn2binpad(r1, rbuf, r1_len) <= 0)
        goto err;
    if (BN_bn2binpad(s1, sbuf, s1_len) <= 0)
        goto err;
    *r = rbuf;
    *s = sbuf;
    *r_len = r1_len;
    *s_len = s1_len;
    ret = 1;
err:
    if (ret == 0) {
        OPENSSL_free(rbuf);
        OPENSSL_free(sbuf);
    }
    DSA_SIG_free(sign);
    return ret;
}

static int dsa_siggen_test(int id)
{
    int ret = 0;
    EVP_PKEY *pkey = NULL;
    unsigned char *sig = NULL, *r = NULL, *s = NULL;
    size_t sig_len = 0, rlen = 0, slen = 0;
    const struct dsa_siggen_st *tst = &dsa_siggen_data[id];

    if (!dsasign_allowed) {
        if (!TEST_ptr_null(pkey = dsa_keygen(tst->L, tst->N)))
            goto err;
    } else {
        if (!TEST_ptr(pkey = dsa_keygen(tst->L, tst->N)))
            goto err;
        if (!TEST_true(sig_gen(pkey, NULL, tst->digest_alg, tst->msg, tst->msg_len,
                               &sig, &sig_len))
            || !TEST_true(get_dsa_sig_rs_bytes(sig, sig_len, &r, &s, &rlen, &slen)))
            goto err;
        test_output_memory("r", r, rlen);
        test_output_memory("s", s, slen);
    }
    ret = 1;
err:
    OPENSSL_free(r);
    OPENSSL_free(s);
    OPENSSL_free(sig);
    EVP_PKEY_free(pkey);
    return ret;
}

static int dsa_sigver_test(int id)
{
    int ret = 0;
    EVP_PKEY_CTX *ctx = NULL;
    EVP_PKEY *pkey = NULL;
    DSA_SIG *sign = NULL;
    size_t sig_len;
    unsigned char *sig = NULL;
    BIGNUM *rbn = NULL, *sbn = NULL;
    EVP_MD *md = NULL;
    unsigned char digest[EVP_MAX_MD_SIZE];
    unsigned int digest_len;
    BN_CTX *bn_ctx = NULL;
    const struct dsa_sigver_st *tst  = &dsa_sigver_data[id];

    if (!TEST_ptr(bn_ctx = BN_CTX_new())
        || !TEST_true(dsa_create_pkey(&pkey, tst->p, tst->p_len,
                                      tst->q, tst->q_len, tst->g, tst->g_len,
                                      NULL, 0, 0, 0, 0, tst->pub, tst->pub_len,
                                      bn_ctx)))
        goto err;

    if (!TEST_ptr(sign = DSA_SIG_new())
        || !TEST_ptr(rbn = BN_bin2bn(tst->r, tst->r_len, NULL))
        || !TEST_ptr(sbn = BN_bin2bn(tst->s, tst->s_len, NULL))
        || !TEST_true(DSA_SIG_set0(sign, rbn, sbn)))
        goto err;
    rbn = sbn = NULL;

    if (!TEST_ptr(md = EVP_MD_fetch(libctx, tst->digest_alg, ""))
        || !TEST_true(EVP_Digest(tst->msg, tst->msg_len,
                                 digest, &digest_len, md, NULL)))
        goto err;

    if (!TEST_int_gt((sig_len = i2d_DSA_SIG(sign, &sig)), 0)
        || !TEST_ptr(ctx = EVP_PKEY_CTX_new_from_pkey(libctx, pkey, ""))
        || !TEST_int_gt(EVP_PKEY_verify_init(ctx), 0)
        || !TEST_int_eq(EVP_PKEY_verify(ctx, sig, sig_len, digest, digest_len),
                        tst->pass))
        goto err;
    ret = 1;
err:
    EVP_PKEY_CTX_free(ctx);
    OPENSSL_free(sig);
    EVP_MD_free(md);
    DSA_SIG_free(sign);
    EVP_PKEY_free(pkey);
    BN_free(rbn);
    BN_free(sbn);
    BN_CTX_free(bn_ctx);
    return ret;
}
#endif /* OPENSSL_NO_DSA */


/* cipher encrypt/decrypt */
static int cipher_enc(const char *alg,
                      const unsigned char *pt, size_t pt_len,
                      const unsigned char *key, size_t key_len,
                      const unsigned char *iv, size_t iv_len,
                      const unsigned char *ct, size_t ct_len,
                      int enc)
{
    int ret = 0, out_len = 0, len = 0;
    EVP_CIPHER_CTX *ctx = NULL;
    EVP_CIPHER *cipher = NULL;
    unsigned char out[256] = { 0 };

    TEST_note("%s : %s", alg, enc ? "encrypt" : "decrypt");
    if (!TEST_ptr(ctx = EVP_CIPHER_CTX_new())
        || !TEST_ptr(cipher = EVP_CIPHER_fetch(libctx, alg, ""))
        || !TEST_true(EVP_CipherInit_ex(ctx, cipher, NULL, key, iv, enc))
        || !TEST_true(EVP_CIPHER_CTX_set_padding(ctx, 0))
        || !TEST_true(EVP_CipherUpdate(ctx, out, &len, pt, pt_len))
        || !TEST_true(EVP_CipherFinal_ex(ctx, out + len, &out_len)))
        goto err;
    out_len += len;
    if (!TEST_mem_eq(out, out_len, ct, ct_len))
        goto err;
    ret = 1;
err:
    EVP_CIPHER_free(cipher);
    EVP_CIPHER_CTX_free(ctx);
    return ret;
}

static int cipher_enc_dec_test(int id)
{
    const struct cipher_st *tst = &cipher_enc_data[id];
    const int enc = 1;

    return TEST_true(cipher_enc(tst->alg, tst->pt, tst->pt_len,
                                tst->key, tst->key_len,
                                tst->iv, tst->iv_len,
                                tst->ct, tst->ct_len, enc))
           && TEST_true(cipher_enc(tst->alg, tst->ct, tst->ct_len,
                                   tst->key, tst->key_len,
                                   tst->iv, tst->iv_len,
                                   tst->pt, tst->pt_len, !enc));
}

static int aes_ccm_enc_dec(const char *alg,
                           const unsigned char *pt, size_t pt_len,
                           const unsigned char *key, size_t key_len,
                           const unsigned char *iv, size_t iv_len,
                           const unsigned char *aad, size_t aad_len,
                           const unsigned char *ct, size_t ct_len,
                           const unsigned char *tag, size_t tag_len,
                           int enc, int pass)
{
    int ret = 0;
    EVP_CIPHER_CTX *ctx;
    EVP_CIPHER *cipher = NULL;
    int out_len, len;
    unsigned char out[1024];

    TEST_note("%s : %s : expected to %s", alg, enc ? "encrypt" : "decrypt",
              pass ? "pass" : "fail");

    if (!TEST_ptr(ctx = EVP_CIPHER_CTX_new())
        || !TEST_ptr(cipher = EVP_CIPHER_fetch(libctx, alg, ""))
        || !TEST_true(EVP_CipherInit_ex(ctx, cipher, NULL, NULL, NULL, enc))
        || !TEST_int_gt(EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_SET_IVLEN, iv_len,
                                          NULL), 0)
        || !TEST_int_gt(EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_SET_TAG, tag_len,
                                          enc ? NULL : (void *)tag), 0)
        || !TEST_true(EVP_CipherInit_ex(ctx, NULL, NULL, key, iv, enc))
        || !TEST_true(EVP_CIPHER_CTX_set_padding(ctx, 0))
        || !TEST_true(EVP_CipherUpdate(ctx, NULL, &len, NULL, pt_len))
        || !TEST_true(EVP_CipherUpdate(ctx, NULL, &len, aad, aad_len))
        || !TEST_int_eq(EVP_CipherUpdate(ctx, out, &len, pt, pt_len), pass))
        goto err;

    if (!pass) {
        ret = 1;
        goto err;
    }
    if (!TEST_true(EVP_CipherFinal_ex(ctx, out + len, &out_len)))
        goto err;
    if (enc) {
        out_len += len;
        if (!TEST_int_gt(EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_GET_TAG,
                                           tag_len, out + out_len), 0)
            || !TEST_mem_eq(out, out_len, ct, ct_len)
            || !TEST_mem_eq(out + out_len, tag_len, tag, tag_len))
            goto err;
    } else {
        if (!TEST_mem_eq(out, out_len + len, ct, ct_len))
            goto err;
    }

    ret = 1;
err:
    EVP_CIPHER_free(cipher);
    EVP_CIPHER_CTX_free(ctx);
    return ret;
}

static int aes_ccm_enc_dec_test(int id)
{
    const struct cipher_ccm_st *tst = &aes_ccm_enc_data[id];

    /* The tag is on the end of the cipher text */
    const size_t tag_len = tst->ct_len - tst->pt_len;
    const size_t ct_len = tst->ct_len - tag_len;
    const unsigned char *tag = tst->ct + ct_len;
    const int enc = 1;
    const int pass = 1;

    if (ct_len < 1)
        return 0;

    return aes_ccm_enc_dec(tst->alg, tst->pt, tst->pt_len,
                           tst->key, tst->key_len,
                           tst->iv, tst->iv_len, tst->aad, tst->aad_len,
                           tst->ct, ct_len, tag, tag_len, enc, pass)
            && aes_ccm_enc_dec(tst->alg, tst->ct, ct_len,
                               tst->key, tst->key_len,
                               tst->iv, tst->iv_len, tst->aad, tst->aad_len,
                               tst->pt, tst->pt_len, tag, tag_len, !enc, pass)
            /* test that it fails if the tag is incorrect */
            && aes_ccm_enc_dec(tst->alg, tst->ct, ct_len,
                               tst->key, tst->key_len,
                               tst->iv, tst->iv_len, tst->aad, tst->aad_len,
                               tst->pt, tst->pt_len,
                               tag - 1, tag_len, !enc, !pass);
}

static int aes_gcm_enc_dec(const char *alg,
                           const unsigned char *pt, size_t pt_len,
                           const unsigned char *key, size_t key_len,
                           const unsigned char *iv, size_t iv_len,
                           const unsigned char *aad, size_t aad_len,
                           const unsigned char *ct, size_t ct_len,
                           const unsigned char *tag, size_t tag_len,
                           int enc, int pass,
                           unsigned char *out, int *out_len,
                           unsigned char *outiv)
{
    int ret = 0;
    EVP_CIPHER_CTX *ctx;
    EVP_CIPHER *cipher = NULL;
    int olen, len;

    TEST_note("%s : %s : expected to %s", alg, enc ? "encrypt" : "decrypt",
              pass ? "pass" : "fail");

    if (!TEST_ptr(ctx = EVP_CIPHER_CTX_new())
        || !TEST_ptr(cipher = EVP_CIPHER_fetch(libctx, alg, ""))
        || !TEST_true(EVP_CipherInit_ex(ctx, cipher, NULL, NULL, NULL, enc))
        || !TEST_int_gt(EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_SET_IVLEN, iv_len,
                                          NULL), 0))
        goto err;

    if (!enc) {
        if (!TEST_int_gt(EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_SET_TAG, tag_len,
                                           (void *)tag), 0))
            goto err;
    }
    /*
     * For testing purposes the IV may be passed in here. In a compliant
     * application the IV would be generated internally. A fake entropy source
     * could also be used to feed in the random IV bytes (see fake_random.c)
     */
    if (!TEST_true(EVP_CipherInit_ex(ctx, NULL, NULL, key, iv, enc))
        || !TEST_true(EVP_CIPHER_CTX_set_padding(ctx, 0))
        || !TEST_true(EVP_CipherUpdate(ctx, NULL, &len, aad, aad_len))
        || !TEST_true(EVP_CipherUpdate(ctx, out, &len, pt, pt_len)))
        goto err;

    if (!TEST_int_eq(EVP_CipherFinal_ex(ctx, out + len, &olen), pass))
        goto err;
    if (!pass) {
        ret = 1;
        goto err;
    }
    olen += len;
    if (enc) {
        if ((ct != NULL && !TEST_mem_eq(out, olen, ct, ct_len))
                || !TEST_int_gt(EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_GET_TAG,
                                                    tag_len, out + olen), 0)
                || (tag != NULL
                    && !TEST_mem_eq(out + olen, tag_len, tag, tag_len)))
            goto err;
    } else {
        if (ct != NULL && !TEST_mem_eq(out, olen, ct, ct_len))
            goto err;
    }

    {
        OSSL_PARAM params[] = { OSSL_PARAM_END, OSSL_PARAM_END, OSSL_PARAM_END };
        OSSL_PARAM *p = params;
        unsigned int iv_generated = -1;
        const OSSL_PARAM *gettables = EVP_CIPHER_CTX_gettable_params(ctx);
        const char *ivgenkey = OSSL_CIPHER_PARAM_AEAD_IV_GENERATED;
        int ivgen = (OSSL_PARAM_locate_const(gettables, ivgenkey) != NULL);

        if (ivgen != 0)
            *p++ = OSSL_PARAM_construct_uint(ivgenkey, &iv_generated);
        if (outiv != NULL)
            *p = OSSL_PARAM_construct_octet_string(OSSL_CIPHER_PARAM_IV,
                                                   outiv, iv_len);
        if (!TEST_true(EVP_CIPHER_CTX_get_params(ctx, params)))
            goto err;
        if (ivgen != 0
                && !TEST_uint_eq(iv_generated, (enc == 0 || iv != NULL ? 0 : 1)))
            goto err;
    }
    if (out_len != NULL)
        *out_len = olen;

    ret = 1;
err:
    EVP_CIPHER_free(cipher);
    EVP_CIPHER_CTX_free(ctx);
    return ret;
}

static int aes_gcm_enc_dec_test(int id)
{
    const struct cipher_gcm_st *tst = &aes_gcm_enc_data[id];
    int enc = 1;
    int pass = 1;
    unsigned char out[1024];

    return aes_gcm_enc_dec(tst->alg, tst->pt, tst->pt_len,
                           tst->key, tst->key_len,
                           tst->iv, tst->iv_len, tst->aad, tst->aad_len,
                           tst->ct, tst->ct_len, tst->tag, tst->tag_len,
                           enc, pass, out, NULL, NULL)
            && aes_gcm_enc_dec(tst->alg, tst->ct, tst->ct_len,
                               tst->key, tst->key_len,
                               tst->iv, tst->iv_len, tst->aad, tst->aad_len,
                               tst->pt, tst->pt_len, tst->tag, tst->tag_len,
                               !enc, pass, out, NULL, NULL)
            /* Fail if incorrect tag passed to decrypt */
            && aes_gcm_enc_dec(tst->alg, tst->ct, tst->ct_len,
                               tst->key, tst->key_len,
                               tst->iv, tst->iv_len, tst->aad, tst->aad_len,
                               tst->pt, tst->pt_len, tst->aad, tst->tag_len,
                               !enc, !pass, out, NULL, NULL);
}

static int aes_gcm_gen_iv_internal_test(void)
{
    const struct cipher_gcm_st *tst = &aes_gcm_enc_data[0];
    int enc = 1;
    int pass = 1;
    int out_len = 0;
    unsigned char out[1024];
    unsigned char iv[16];

    return aes_gcm_enc_dec(tst->alg, tst->pt, tst->pt_len,
                           tst->key, tst->key_len,
                           NULL, tst->iv_len, tst->aad, tst->aad_len,
                           NULL, tst->ct_len, NULL, tst->tag_len,
                           enc, pass, out, &out_len, iv)
            && aes_gcm_enc_dec(tst->alg, out, out_len,
                               tst->key, tst->key_len,
                               iv, tst->iv_len, tst->aad, tst->aad_len,
                               tst->pt, tst->pt_len, out + out_len, tst->tag_len,
                               !enc, pass, out, NULL, NULL);
}

#ifndef OPENSSL_NO_DH
static int dh_create_pkey(EVP_PKEY **pkey, const char *group_name,
                          const unsigned char *pub, size_t pub_len,
                          const unsigned char *priv, size_t priv_len,
                          BN_CTX *bn_ctx, int pass)
{
    int ret = 0;
    EVP_PKEY_CTX *ctx = NULL;
    OSSL_PARAM_BLD *bld = NULL;
    OSSL_PARAM *params = NULL;
    BIGNUM *pub_bn = NULL, *priv_bn = NULL;

    if (!TEST_ptr(bld = OSSL_PARAM_BLD_new())
        || (group_name != NULL
            && !TEST_int_gt(OSSL_PARAM_BLD_push_utf8_string(
                              bld, OSSL_PKEY_PARAM_GROUP_NAME,
                              group_name, 0), 0)))
        goto err;

    if (pub != NULL) {
        if (!TEST_ptr(pub_bn = BN_CTX_get(bn_ctx))
            || !TEST_ptr(BN_bin2bn(pub, pub_len, pub_bn))
            || !TEST_true(OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_PUB_KEY,
                                                 pub_bn)))
            goto err;
    }
    if (priv != NULL) {
        if (!TEST_ptr(priv_bn = BN_CTX_get(bn_ctx))
            || !TEST_ptr(BN_bin2bn(priv, priv_len, priv_bn))
            || !TEST_true(OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_PRIV_KEY,
                                                 priv_bn)))
            goto err;
    }

    if (!TEST_ptr(params = OSSL_PARAM_BLD_to_param(bld))
        || !TEST_ptr(ctx = EVP_PKEY_CTX_new_from_name(libctx, "DH", NULL))
        || !TEST_int_eq(EVP_PKEY_fromdata_init(ctx), 1)
        || !TEST_int_eq(EVP_PKEY_fromdata(ctx, pkey, EVP_PKEY_KEYPAIR, params),
                        pass))
    goto err;

    ret = 1;
err:
    OSSL_PARAM_free(params);
    OSSL_PARAM_BLD_free(bld);
    EVP_PKEY_CTX_free(ctx);
    return ret;
}

static int dh_safe_prime_keygen_test(int id)
{
    int ret = 0;
    EVP_PKEY_CTX *ctx = NULL;
    EVP_PKEY *pkey = NULL;
    unsigned char *priv = NULL;
    unsigned char *pub = NULL;
    size_t priv_len = 0, pub_len = 0;
    OSSL_PARAM params[2];
    const struct dh_safe_prime_keygen_st *tst = &dh_safe_prime_keygen_data[id];

    params[0] = OSSL_PARAM_construct_utf8_string(OSSL_PKEY_PARAM_GROUP_NAME,
                                                 (char *)tst->group_name, 0);
    params[1] = OSSL_PARAM_construct_end();

    if (!TEST_ptr(ctx = EVP_PKEY_CTX_new_from_name(libctx, "DH", NULL))
        || !TEST_int_gt(EVP_PKEY_keygen_init(ctx), 0)
        || !TEST_true(EVP_PKEY_CTX_set_params(ctx, params))
        || !TEST_int_gt(EVP_PKEY_keygen(ctx, &pkey), 0)
        || !TEST_true(pkey_get_bn_bytes(pkey, OSSL_PKEY_PARAM_PRIV_KEY,
                                        &priv, &priv_len))
        || !TEST_true(pkey_get_bn_bytes(pkey, OSSL_PKEY_PARAM_PUB_KEY,
                                        &pub, &pub_len)))
        goto err;

    test_output_memory("x", priv, priv_len);
    test_output_memory("y", pub, pub_len);
    ret = 1;
err:
    OPENSSL_clear_free(priv, priv_len);
    OPENSSL_free(pub);
    EVP_PKEY_free(pkey);
    EVP_PKEY_CTX_free(ctx);
    return ret;
}

static int dh_safe_prime_keyver_test(int id)
{
    int ret = 0;
    BN_CTX *bn_ctx = NULL;
    EVP_PKEY_CTX *key_ctx = NULL;
    EVP_PKEY *pkey = NULL;
    const struct dh_safe_prime_keyver_st *tst = &dh_safe_prime_keyver_data[id];

    if (!TEST_ptr(bn_ctx = BN_CTX_new_ex(libctx))
        || !TEST_true(dh_create_pkey(&pkey, tst->group_name,
                                     tst->pub, tst->pub_len,
                                     tst->priv, tst->priv_len, bn_ctx, 1))
        || !TEST_ptr(key_ctx = EVP_PKEY_CTX_new_from_pkey(libctx, pkey, ""))
        || !TEST_int_eq(EVP_PKEY_check(key_ctx), tst->pass))
        goto err;

    ret = 1;
err:
    EVP_PKEY_free(pkey);
    EVP_PKEY_CTX_free(key_ctx);
    BN_CTX_free(bn_ctx);
    return ret;
}
#endif /* OPENSSL_NO_DH */


static int rsa_create_pkey(EVP_PKEY **pkey,
                           const unsigned char *n, size_t n_len,
                           const unsigned char *e, size_t e_len,
                           const unsigned char *d, size_t d_len,
                           BN_CTX *bn_ctx)
{
    int ret = 0;
    EVP_PKEY_CTX *ctx = NULL;
    OSSL_PARAM_BLD *bld = NULL;
    OSSL_PARAM *params = NULL;
    BIGNUM *e_bn = NULL, *d_bn = NULL, *n_bn = NULL;

    if (!TEST_ptr(bld = OSSL_PARAM_BLD_new())
        || !TEST_ptr(n_bn = BN_CTX_get(bn_ctx))
        || !TEST_ptr(BN_bin2bn(n, n_len, n_bn))
        || !TEST_true(OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_RSA_N, n_bn)))
        goto err;

    if (e != NULL) {
        if (!TEST_ptr(e_bn = BN_CTX_get(bn_ctx))
            || !TEST_ptr(BN_bin2bn(e, e_len, e_bn))
            || !TEST_true(OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_RSA_E,
                          e_bn)))
            goto err;
    }
    if (d != NULL) {
        if (!TEST_ptr(d_bn = BN_CTX_get(bn_ctx))
            || !TEST_ptr(BN_bin2bn(d, d_len, d_bn))
            || !TEST_true(OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_RSA_D,
                          d_bn)))
            goto err;
    }
    if (!TEST_ptr(params = OSSL_PARAM_BLD_to_param(bld))
        || !TEST_ptr(ctx = EVP_PKEY_CTX_new_from_name(libctx, "RSA", NULL))
        || !TEST_int_eq(EVP_PKEY_fromdata_init(ctx), 1)
        || !TEST_int_eq(EVP_PKEY_fromdata(ctx, pkey, EVP_PKEY_KEYPAIR, params),
                        1))
        goto err;

    ret = 1;
err:
    OSSL_PARAM_free(params);
    OSSL_PARAM_BLD_free(bld);
    EVP_PKEY_CTX_free(ctx);
    return ret;
}

static int rsa_keygen_test(int id)
{
    int ret = 0;
    EVP_PKEY_CTX *ctx = NULL;
    EVP_PKEY *pkey = NULL;
    BIGNUM *e_bn = NULL;
    BIGNUM *xp1_bn = NULL, *xp2_bn = NULL, *xp_bn = NULL;
    BIGNUM *xq1_bn = NULL, *xq2_bn = NULL, *xq_bn = NULL;
    unsigned char *n = NULL, *d = NULL;
    unsigned char *p = NULL, *p1 = NULL, *p2 = NULL;
    unsigned char *q = NULL, *q1 = NULL, *q2 = NULL;
    size_t n_len = 0, d_len = 0;
    size_t p_len = 0, p1_len = 0, p2_len = 0;
    size_t q_len = 0, q1_len = 0, q2_len = 0;
    OSSL_PARAM_BLD *bld = NULL;
    OSSL_PARAM *params = NULL;
    const struct rsa_keygen_st *tst = &rsa_keygen_data[id];

    if (!TEST_ptr(bld = OSSL_PARAM_BLD_new())
        || !TEST_ptr(xp1_bn = BN_bin2bn(tst->xp1, tst->xp1_len, NULL))
        || !TEST_ptr(xp2_bn = BN_bin2bn(tst->xp2, tst->xp2_len, NULL))
        || !TEST_ptr(xp_bn = BN_bin2bn(tst->xp, tst->xp_len, NULL))
        || !TEST_ptr(xq1_bn = BN_bin2bn(tst->xq1, tst->xq1_len, NULL))
        || !TEST_ptr(xq2_bn = BN_bin2bn(tst->xq2, tst->xq2_len, NULL))
        || !TEST_ptr(xq_bn = BN_bin2bn(tst->xq, tst->xq_len, NULL))
        || !TEST_true(OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_RSA_TEST_XP1,
                                             xp1_bn))
        || !TEST_true(OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_RSA_TEST_XP2,
                                             xp2_bn))
        || !TEST_true(OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_RSA_TEST_XP,
                                             xp_bn))
        || !TEST_true(OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_RSA_TEST_XQ1,
                                             xq1_bn))
        || !TEST_true(OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_RSA_TEST_XQ2,
                                             xq2_bn))
        || !TEST_true(OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_RSA_TEST_XQ,
                                             xq_bn))
        || !TEST_ptr(params = OSSL_PARAM_BLD_to_param(bld)))
        goto err;

    if (!TEST_ptr(ctx = EVP_PKEY_CTX_new_from_name(libctx, "RSA", NULL))
        || !TEST_ptr(e_bn = BN_bin2bn(tst->e, tst->e_len, NULL))
        || !TEST_int_gt(EVP_PKEY_keygen_init(ctx), 0)
        || !TEST_int_gt(EVP_PKEY_CTX_set_params(ctx, params), 0)
        || !TEST_int_gt(EVP_PKEY_CTX_set_rsa_keygen_bits(ctx, tst->mod), 0)
        || !TEST_int_gt(EVP_PKEY_CTX_set1_rsa_keygen_pubexp(ctx, e_bn), 0)
        || !TEST_int_gt(EVP_PKEY_keygen(ctx, &pkey), 0)
        || !TEST_true(pkey_get_bn_bytes(pkey, OSSL_PKEY_PARAM_RSA_TEST_P1,
                                        &p1, &p1_len))
        || !TEST_true(pkey_get_bn_bytes(pkey, OSSL_PKEY_PARAM_RSA_TEST_P2,
                                        &p2, &p2_len))
        || !TEST_true(pkey_get_bn_bytes(pkey, OSSL_PKEY_PARAM_RSA_TEST_Q1,
                                        &q1, &q1_len))
        || !TEST_true(pkey_get_bn_bytes(pkey, OSSL_PKEY_PARAM_RSA_TEST_Q2,
                                        &q2, &q2_len))
        || !TEST_true(pkey_get_bn_bytes(pkey, OSSL_PKEY_PARAM_RSA_FACTOR1,
                                        &p, &p_len))
        || !TEST_true(pkey_get_bn_bytes(pkey, OSSL_PKEY_PARAM_RSA_FACTOR2,
                                        &q, &q_len))
        || !TEST_true(pkey_get_bn_bytes(pkey, OSSL_PKEY_PARAM_RSA_N,
                                        &n, &n_len))
        || !TEST_true(pkey_get_bn_bytes(pkey, OSSL_PKEY_PARAM_RSA_D,
                                        &d, &d_len)))
        goto err;

    if (!TEST_mem_eq(tst->p1, tst->p1_len, p1, p1_len)
        || !TEST_mem_eq(tst->p2, tst->p2_len, p2, p2_len)
        || !TEST_mem_eq(tst->p, tst->p_len, p, p_len)
        || !TEST_mem_eq(tst->q1, tst->q1_len, q1, q1_len)
        || !TEST_mem_eq(tst->q2, tst->q2_len, q2, q2_len)
        || !TEST_mem_eq(tst->q, tst->q_len, q, q_len)
        || !TEST_mem_eq(tst->n, tst->n_len, n, n_len)
        || !TEST_mem_eq(tst->d, tst->d_len, d, d_len))
        goto err;

    test_output_memory("p1", p1, p1_len);
    test_output_memory("p2", p2, p2_len);
    test_output_memory("p", p, p_len);
    test_output_memory("q1", q1, q1_len);
    test_output_memory("q2", q2, q2_len);
    test_output_memory("q", q, q_len);
    test_output_memory("n", n, n_len);
    test_output_memory("d", d, d_len);
    ret = 1;
err:
    BN_free(xp1_bn);
    BN_free(xp2_bn);
    BN_free(xp_bn);
    BN_free(xq1_bn);
    BN_free(xq2_bn);
    BN_free(xq_bn);
    BN_free(e_bn);
    OPENSSL_free(p1);
    OPENSSL_free(p2);
    OPENSSL_free(q1);
    OPENSSL_free(q2);
    OPENSSL_free(p);
    OPENSSL_free(q);
    OPENSSL_free(n);
    OPENSSL_free(d);
    EVP_PKEY_free(pkey);
    EVP_PKEY_CTX_free(ctx);
    OSSL_PARAM_free(params);
    OSSL_PARAM_BLD_free(bld);
    return ret;
}

static int rsa_siggen_test(int id)
{
    int ret = 0;
    EVP_PKEY *pkey = NULL;
    unsigned char *sig = NULL, *n = NULL, *e = NULL;
    size_t sig_len = 0, n_len = 0, e_len = 0;
    OSSL_PARAM params[4], *p;
    const struct rsa_siggen_st *tst = &rsa_siggen_data[id];
    int salt_len = tst->pss_salt_len;

    if (!rsa_sign_x931_pad_allowed
            && (strcmp(tst->sig_pad_mode, OSSL_PKEY_RSA_PAD_MODE_X931) == 0))
        return TEST_skip("x931 signing is not allowed");

    TEST_note("RSA %s signature generation", tst->sig_pad_mode);

    p = params;
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_SIGNATURE_PARAM_PAD_MODE,
                                            (char *)tst->sig_pad_mode, 0);
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_SIGNATURE_PARAM_DIGEST,
                                            (char *)tst->digest_alg, 0);
    if (salt_len >= 0)
        *p++ = OSSL_PARAM_construct_int(OSSL_SIGNATURE_PARAM_PSS_SALTLEN,
                                        &salt_len);
    *p++ = OSSL_PARAM_construct_end();

    if (!TEST_ptr(pkey = EVP_PKEY_Q_keygen(libctx, NULL, "RSA", tst->mod))
        || !TEST_true(pkey_get_bn_bytes(pkey, OSSL_PKEY_PARAM_RSA_N, &n, &n_len))
        || !TEST_true(pkey_get_bn_bytes(pkey, OSSL_PKEY_PARAM_RSA_E, &e, &e_len))
        || !TEST_true(sig_gen(pkey, params, tst->digest_alg,
                              tst->msg, tst->msg_len,
                              &sig, &sig_len)))
        goto err;
    test_output_memory("n", n, n_len);
    test_output_memory("e", e, e_len);
    test_output_memory("sig", sig, sig_len);
    ret = 1;
err:
    OPENSSL_free(n);
    OPENSSL_free(e);
    OPENSSL_free(sig);
    EVP_PKEY_free(pkey);
    return ret;
}

static int rsa_sigver_test(int id)
{
    int ret = 0;
    EVP_PKEY_CTX *pkey_ctx = NULL;
    EVP_PKEY *pkey = NULL;
    EVP_MD_CTX *md_ctx = NULL;
    BN_CTX *bn_ctx = NULL;
    OSSL_PARAM params[4], *p;
    const struct rsa_sigver_st *tst  = &rsa_sigver_data[id];
    int salt_len = tst->pss_salt_len;

    TEST_note("RSA %s Signature Verify : expected to %s ", tst->sig_pad_mode,
               tst->pass == PASS ? "pass" : "fail");

    p = params;
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_SIGNATURE_PARAM_PAD_MODE,
                                            (char *)tst->sig_pad_mode, 0);
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_SIGNATURE_PARAM_DIGEST,
                                            (char *)tst->digest_alg, 0);
    if (salt_len >= 0)
        *p++ = OSSL_PARAM_construct_int(OSSL_SIGNATURE_PARAM_PSS_SALTLEN,
                                        &salt_len);
    *p = OSSL_PARAM_construct_end();

    if (!TEST_ptr(bn_ctx = BN_CTX_new())
        || !TEST_true(rsa_create_pkey(&pkey, tst->n, tst->n_len,
                                      tst->e, tst->e_len, NULL, 0, bn_ctx))
        || !TEST_ptr(md_ctx = EVP_MD_CTX_new())
        || !TEST_true(EVP_DigestVerifyInit_ex(md_ctx, &pkey_ctx,
                                              tst->digest_alg, libctx, NULL,
                                              pkey, NULL))
        || !check_verify_message(pkey_ctx, 1)
        || !TEST_true(EVP_PKEY_CTX_set_params(pkey_ctx, params))
        || !TEST_int_eq(EVP_DigestVerify(md_ctx, tst->sig, tst->sig_len,
                                         tst->msg, tst->msg_len), tst->pass)
        || !check_verify_message(pkey_ctx, 1)
        || !TEST_true(EVP_PKEY_verify_init(pkey_ctx))
        || !check_verify_message(pkey_ctx, 0))
        goto err;

    ret = 1;
err:
    EVP_PKEY_free(pkey);
    BN_CTX_free(bn_ctx);
    EVP_MD_CTX_free(md_ctx);
    return ret;
}

static int rsa_decryption_primitive_test(int id)
{
    int ret = 0;
    EVP_PKEY_CTX *ctx = NULL;
    EVP_PKEY *pkey = NULL;
    unsigned char pt[2048];
    size_t pt_len = sizeof(pt);
    unsigned char *n = NULL, *e = NULL;
    size_t n_len = 0, e_len = 0;
    BN_CTX *bn_ctx = NULL;
    const struct rsa_decrypt_prim_st *tst  = &rsa_decrypt_prim_data[id];

    if (!TEST_ptr(pkey = EVP_PKEY_Q_keygen(libctx, NULL, "RSA", (size_t)2048))
        || !TEST_true(pkey_get_bn_bytes(pkey, OSSL_PKEY_PARAM_RSA_N, &n, &n_len))
        || !TEST_true(pkey_get_bn_bytes(pkey, OSSL_PKEY_PARAM_RSA_E, &e, &e_len))
        || !TEST_ptr(ctx = EVP_PKEY_CTX_new_from_pkey(libctx, pkey, ""))
        || !TEST_int_gt(EVP_PKEY_decrypt_init(ctx), 0)
        || !TEST_int_gt(EVP_PKEY_CTX_set_rsa_padding(ctx, RSA_NO_PADDING), 0))
        goto err;

    test_output_memory("n", n, n_len);
    test_output_memory("e", e, e_len);
    if (EVP_PKEY_decrypt(ctx, pt, &pt_len, tst->ct, tst->ct_len) <= 0)
        TEST_note("Decryption Failed");
    else
        test_output_memory("pt", pt, pt_len);
    ret = 1;
err:
    OPENSSL_free(n);
    OPENSSL_free(e);
    EVP_PKEY_CTX_free(ctx);
    EVP_PKEY_free(pkey);
    BN_CTX_free(bn_ctx);
    return ret;
}

static int self_test_events(const OSSL_PARAM params[], void *varg)
{
    SELF_TEST_ARGS *args = varg;
    const OSSL_PARAM *p = NULL;
    const char *phase = NULL, *type = NULL, *desc = NULL;
    int ret = 0;

    if (!args->enable)
        return 1;

    args->called++;
    p = OSSL_PARAM_locate_const(params, OSSL_PROV_PARAM_SELF_TEST_PHASE);
    if (p == NULL || p->data_type != OSSL_PARAM_UTF8_STRING)
        goto err;
    phase = (const char *)p->data;

    p = OSSL_PARAM_locate_const(params, OSSL_PROV_PARAM_SELF_TEST_DESC);
    if (p == NULL || p->data_type != OSSL_PARAM_UTF8_STRING)
        goto err;
    desc = (const char *)p->data;

    p = OSSL_PARAM_locate_const(params, OSSL_PROV_PARAM_SELF_TEST_TYPE);
    if (p == NULL || p->data_type != OSSL_PARAM_UTF8_STRING)
        goto err;
    type = (const char *)p->data;

    BIO_printf(bio_out, "%s %s %s\n", phase, desc, type);
    ret = 1;
err:
    return ret;
}

static int drbg_test(int id)
{
    OSSL_PARAM params[3];
    EVP_RAND *rand = NULL;
    EVP_RAND_CTX *ctx = NULL, *parent = NULL;
    unsigned char returned_bits[64];
    const size_t returned_bits_len = sizeof(returned_bits);
    unsigned int strength = 256;
    const struct drbg_st *tst  = &drbg_data[id];
    int res = 0;

    /* Create the seed source */
    if (!TEST_ptr(rand = EVP_RAND_fetch(libctx, "TEST-RAND", "-fips"))
        || !TEST_ptr(parent = EVP_RAND_CTX_new(rand, NULL)))
        goto err;
    EVP_RAND_free(rand);
    rand = NULL;

    params[0] = OSSL_PARAM_construct_uint(OSSL_RAND_PARAM_STRENGTH, &strength);
    params[1] = OSSL_PARAM_construct_end();
    if (!TEST_true(EVP_RAND_CTX_set_params(parent, params)))
        goto err;

    /* Get the DRBG */
    if (!TEST_ptr(rand = EVP_RAND_fetch(libctx, tst->drbg_name, ""))
        || !TEST_ptr(ctx = EVP_RAND_CTX_new(rand, parent)))
        goto err;

    /* Set the DRBG up */
    params[0] = OSSL_PARAM_construct_int(OSSL_DRBG_PARAM_USE_DF,
                                         (int *)&tst->use_df);
    params[1] = OSSL_PARAM_construct_utf8_string(OSSL_DRBG_PARAM_CIPHER,
                                                 (char *)tst->cipher, 0);
    params[2] = OSSL_PARAM_construct_end();
    if (!TEST_true(EVP_RAND_CTX_set_params(ctx, params)))
        goto err;

    /* Feed in the entropy and nonce */
    params[0] = OSSL_PARAM_construct_octet_string(OSSL_RAND_PARAM_TEST_ENTROPY,
                                                  (void *)tst->entropy_input,
                                                  tst->entropy_input_len);
    params[1] = OSSL_PARAM_construct_octet_string(OSSL_RAND_PARAM_TEST_NONCE,
                                                  (void *)tst->nonce,
                                                  tst->nonce_len);
    params[2] = OSSL_PARAM_construct_end();
    if (!TEST_true(EVP_RAND_CTX_set_params(parent, params)))
        goto err;

    /*
     * Run the test
     * A NULL personalisation string defaults to the built in so something
     * non-NULL is needed if there is no personalisation string
     */
    if (!TEST_true(EVP_RAND_instantiate(ctx, 0, 0, (void *)"", 0, NULL))
        || !TEST_true(EVP_RAND_generate(ctx, returned_bits, returned_bits_len,
                                        0, 0, NULL, 0))
        || !TEST_true(EVP_RAND_generate(ctx, returned_bits, returned_bits_len,
                                        0, 0, NULL, 0)))
        goto err;

    test_output_memory("returned bits", returned_bits, returned_bits_len);

    /* Clean up */
    if (!TEST_true(EVP_RAND_uninstantiate(ctx))
        || !TEST_true(EVP_RAND_uninstantiate(parent)))
        goto err;

    /* Verify the output */
    if (!TEST_mem_eq(returned_bits, returned_bits_len,
                     tst->returned_bits, tst->returned_bits_len))
        goto err;
    res = 1;
err:
    EVP_RAND_CTX_free(ctx);
    /* Coverity is confused by the upref/free in EVP_RAND_CTX_new() subdue it */
    /* coverity[pass_freed_arg] */
    EVP_RAND_CTX_free(parent);
    EVP_RAND_free(rand);
    return res;
}

static int aes_cfb1_bits_test(void)
{
    int ret = 0;
    EVP_CIPHER *cipher = NULL;
    EVP_CIPHER_CTX *ctx = NULL;
    unsigned char out[16] = { 0 };
    int outlen;
    const OSSL_PARAM *params, *p;

    static const unsigned char key[] = {
        0x12, 0x22, 0x58, 0x2F, 0x1C, 0x1A, 0x8A, 0x88,
        0x30, 0xFC, 0x18, 0xB7, 0x24, 0x89, 0x7F, 0xC0
    };
    static const unsigned char iv[] = {
        0x05, 0x28, 0xB5, 0x2B, 0x58, 0x27, 0x63, 0x5C,
        0x81, 0x86, 0xD3, 0x63, 0x60, 0xB0, 0xAA, 0x2B
    };
    static const unsigned char pt[] = {
        0xB4
    };
    static const unsigned char expected[] = {
        0x6C
    };

    if (!TEST_ptr(cipher = EVP_CIPHER_fetch(libctx, "AES-128-CFB1", "fips=yes")))
        goto err;
    if (!TEST_ptr(ctx = EVP_CIPHER_CTX_new()))
        goto err;
    if (!TEST_int_gt(EVP_CipherInit_ex(ctx, cipher, NULL, key, iv, 1), 0))
        goto err;
    if (!TEST_ptr(params = EVP_CIPHER_CTX_settable_params(ctx))
        || !TEST_ptr(p = OSSL_PARAM_locate_const(params,
                                                 OSSL_CIPHER_PARAM_USE_BITS)))
        goto err;
    EVP_CIPHER_CTX_set_flags(ctx, EVP_CIPH_FLAG_LENGTH_BITS);
    if (!TEST_int_gt(EVP_CipherUpdate(ctx, out, &outlen, pt, 7), 0))
        goto err;
    if (!TEST_int_eq(outlen, 7))
        goto err;
    if (!TEST_mem_eq(out, (outlen + 7) / 8, expected, sizeof(expected)))
        goto err;
    ret = 1;
err:
    EVP_CIPHER_free(cipher);
    EVP_CIPHER_CTX_free(ctx);
    return ret;
}

int setup_tests(void)
{
    char *config_file = NULL;

    OPTION_CHOICE o;

    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_CONFIG_FILE:
            config_file = opt_arg();
            break;
        case OPT_TEST_CASES:
           break;
        default:
        case OPT_ERR:
            return 0;
        }
    }

    if (!test_get_libctx(&libctx, &prov_null, config_file, NULL, NULL))
        return 0;

    OSSL_SELF_TEST_set_callback(libctx, self_test_events, &self_test_args);

    ADD_TEST(aes_cfb1_bits_test);
    ADD_ALL_TESTS(cipher_enc_dec_test, OSSL_NELEM(cipher_enc_data));
    ADD_ALL_TESTS(aes_ccm_enc_dec_test, OSSL_NELEM(aes_ccm_enc_data));
    ADD_ALL_TESTS(aes_gcm_enc_dec_test, OSSL_NELEM(aes_gcm_enc_data));
    if (fips_provider_version_ge(libctx, 3, 4, 0))
        ADD_TEST(aes_gcm_gen_iv_internal_test);

    pass_sig_gen_params = fips_provider_version_ge(libctx, 3, 4, 0);
    rsa_sign_x931_pad_allowed = fips_provider_version_lt(libctx, 3, 4, 0);
    ADD_ALL_TESTS(rsa_keygen_test, OSSL_NELEM(rsa_keygen_data));
    ADD_ALL_TESTS(rsa_siggen_test, OSSL_NELEM(rsa_siggen_data));
    ADD_ALL_TESTS(rsa_sigver_test, OSSL_NELEM(rsa_sigver_data));
    ADD_ALL_TESTS(rsa_decryption_primitive_test,
                  OSSL_NELEM(rsa_decrypt_prim_data));

#ifndef OPENSSL_NO_DH
    ADD_ALL_TESTS(dh_safe_prime_keygen_test,
                  OSSL_NELEM(dh_safe_prime_keygen_data));
    ADD_ALL_TESTS(dh_safe_prime_keyver_test,
                  OSSL_NELEM(dh_safe_prime_keyver_data));
#endif /* OPENSSL_NO_DH */

#ifndef OPENSSL_NO_DSA
    dsasign_allowed = fips_provider_version_lt(libctx, 3, 4, 0);
    ADD_ALL_TESTS(dsa_keygen_test, OSSL_NELEM(dsa_keygen_data));
    ADD_ALL_TESTS(dsa_paramgen_test, OSSL_NELEM(dsa_paramgen_data));
    ADD_ALL_TESTS(dsa_pqver_test, OSSL_NELEM(dsa_pqver_data));
    ADD_ALL_TESTS(dsa_siggen_test, OSSL_NELEM(dsa_siggen_data));
    ADD_ALL_TESTS(dsa_sigver_test, OSSL_NELEM(dsa_sigver_data));
#endif /* OPENSSL_NO_DSA */

#ifndef OPENSSL_NO_EC
    ec_cofactors = fips_provider_version_ge(libctx, 3, 4, 0);
    ADD_ALL_TESTS(ecdsa_keygen_test, OSSL_NELEM(ecdsa_keygen_data));
    ADD_ALL_TESTS(ecdsa_pub_verify_test, OSSL_NELEM(ecdsa_pv_data));
    ADD_ALL_TESTS(ecdsa_siggen_test, OSSL_NELEM(ecdsa_siggen_data));
    ADD_ALL_TESTS(ecdsa_sigver_test, OSSL_NELEM(ecdsa_sigver_data));
    ADD_ALL_TESTS(ecdh_cofactor_derive_test,
                  OSSL_NELEM(ecdh_cofactor_derive_data));
#endif /* OPENSSL_NO_EC */

#ifndef OPENSSL_NO_ECX
    if (fips_provider_version_ge(libctx, 3, 4, 0)) {
        ADD_ALL_TESTS(eddsa_keygen_test, OSSL_NELEM(eddsa_pv_data));
        ADD_ALL_TESTS(eddsa_pub_verify_test, OSSL_NELEM(eddsa_pv_data));
    }
#endif
    ADD_ALL_TESTS(drbg_test, OSSL_NELEM(drbg_data));
    return 1;
}

void cleanup_tests(void)
{
    OSSL_PROVIDER_unload(prov_null);
    OSSL_LIB_CTX_free(libctx);
}
