/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core_names.h>
#include <openssl/evp.h>
#include "internal/nelem.h"
#include "testutil.h"
#include "ml_dsa.inc"
#include "crypto/ml_dsa.h"

typedef enum OPTION_choice {
    OPT_ERR = -1,
    OPT_EOF = 0,
    OPT_CONFIG_FILE,
    OPT_TEST_ENUM
} OPTION_CHOICE;

static OSSL_LIB_CTX *lib_ctx = NULL;
static OSSL_PROVIDER *null_prov = NULL;
static OSSL_PROVIDER *lib_prov = NULL;

static EVP_PKEY *do_gen_key(const char *alg,
                            const uint8_t *seed, size_t seed_len)
{
    EVP_PKEY *pkey = NULL;
    EVP_PKEY_CTX *ctx = NULL;
    OSSL_PARAM params[3], *p = params;

    if (seed_len != 0)
        *p++ = OSSL_PARAM_construct_octet_string(OSSL_PKEY_PARAM_ML_DSA_SEED,
                                                 (char *)seed, seed_len);
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_PKEY_PARAM_PROPERTIES,
                                            "?fips=yes", 0);
    *p = OSSL_PARAM_construct_end();

    if (!TEST_ptr(ctx = EVP_PKEY_CTX_new_from_name(lib_ctx, alg, NULL))
            || !TEST_int_eq(EVP_PKEY_keygen_init(ctx), 1)
            || !TEST_int_eq(EVP_PKEY_CTX_set_params(ctx, params), 1)
            || !TEST_int_eq(EVP_PKEY_generate(ctx, &pkey), 1))
        pkey = NULL;

    EVP_PKEY_CTX_free(ctx);
    return pkey;
}

static int ml_dsa_create_keypair(EVP_PKEY **pkey, const char *name,
                                 const uint8_t *priv, size_t priv_len,
                                 const uint8_t *pub, size_t pub_len,
                                 int expected)
{
    int ret = 0, selection = 0;
    EVP_PKEY_CTX *ctx = NULL;
    OSSL_PARAM params[3], *p = params;

    if (priv != NULL) {
        *p++ = OSSL_PARAM_construct_octet_string(OSSL_PKEY_PARAM_PRIV_KEY,
                                                 (uint8_t *)priv, priv_len);
        selection = OSSL_KEYMGMT_SELECT_PRIVATE_KEY;
    }
    if (pub != NULL) {
        *p++ = OSSL_PARAM_construct_octet_string(OSSL_PKEY_PARAM_PUB_KEY,
                                                 (uint8_t *)pub, pub_len);
        selection |= OSSL_KEYMGMT_SELECT_PUBLIC_KEY;
    }
    *p = OSSL_PARAM_construct_end();

    if (!TEST_ptr(ctx = EVP_PKEY_CTX_new_from_name(lib_ctx, name, NULL))
            || !TEST_int_eq(EVP_PKEY_fromdata_init(ctx), 1)
            || !TEST_int_eq(EVP_PKEY_fromdata(ctx, pkey, selection,
                                              params), expected))
        goto err;

    ret = 1;
err:
    EVP_PKEY_CTX_free(ctx);
    return ret;
}

static int ml_dsa_keygen_test(int tst_id)
{
    int ret = 0;
    const ML_DSA_KEYGEN_TEST_DATA *tst = &ml_dsa_keygen_testdata[tst_id];
    EVP_PKEY *pkey = NULL;
    uint8_t priv[5 * 1024], pub[3 * 1024], seed[ML_DSA_SEED_BYTES];
    size_t priv_len, pub_len, seed_len;
    int bits = 0, sec_bits = 0, sig_len = 0;

    if (!TEST_ptr(pkey = do_gen_key(tst->name, tst->seed, tst->seed_len))
            || !TEST_true(EVP_PKEY_get_octet_string_param(pkey, OSSL_PKEY_PARAM_ML_DSA_SEED,
                                                          seed, sizeof(seed), &seed_len))
            || !TEST_true(EVP_PKEY_get_octet_string_param(pkey, OSSL_PKEY_PARAM_PRIV_KEY,
                                                          priv, sizeof(priv), &priv_len))
            || !TEST_true(EVP_PKEY_get_octet_string_param(pkey, OSSL_PKEY_PARAM_PUB_KEY,
                                                          pub, sizeof(pub), &pub_len))
            || !TEST_mem_eq(pub, pub_len, tst->pub, tst->pub_len)
            || !TEST_mem_eq(priv, priv_len, tst->priv, tst->priv_len)
            || !TEST_mem_eq(seed, seed_len, tst->seed, tst->seed_len)
            /* The following checks assume that algorithm is ML-DSA-65 */
            || !TEST_true(EVP_PKEY_get_int_param(pkey, OSSL_PKEY_PARAM_BITS, &bits))
            || !TEST_int_eq(bits, 1952 * 8)
            || !TEST_true(EVP_PKEY_get_int_param(pkey, OSSL_PKEY_PARAM_SECURITY_BITS,
                                                 &sec_bits))
            || !TEST_int_eq(sec_bits, 192)
            || !TEST_true(EVP_PKEY_get_int_param(pkey, OSSL_PKEY_PARAM_MAX_SIZE,
                                                 &sig_len))
            || !TEST_int_ge(sig_len, 3309))
        goto err;
    ret = 1;
err:
    EVP_PKEY_free(pkey);
    return ret;
}

static int ml_dsa_siggen_test(int tst_id)
{
    int ret = 0;
    const ML_DSA_SIG_GEN_TEST_DATA *td = &ml_dsa_siggen_testdata[tst_id];
    EVP_PKEY_CTX *sctx = NULL;
    EVP_PKEY *pkey = NULL;
    EVP_SIGNATURE *sig_alg = NULL;
    OSSL_PARAM params[4], *p = params;
    uint8_t *psig = NULL;
    size_t psig_len = 0, sig_len2 = 0;
    uint8_t digest[32];
    size_t digest_len = sizeof(digest);
    int encode = 0, deterministic = 1;

    *p++ = OSSL_PARAM_construct_int(OSSL_SIGNATURE_PARAM_DETERMINISTIC, &deterministic);
    *p++ = OSSL_PARAM_construct_int(OSSL_SIGNATURE_PARAM_MESSAGE_ENCODING, &encode);
    if (td->add_random != NULL)
        *p++ = OSSL_PARAM_construct_octet_string(OSSL_SIGNATURE_PARAM_TEST_ENTROPY,
                                                 (char *)td->add_random,
                                                 td->add_random_len);
    *p = OSSL_PARAM_construct_end();

    /*
     * This just uses from data here, but keygen also works.
     * The keygen path is tested via ml_dsa_keygen_test
     */
    if (!TEST_true(ml_dsa_create_keypair(&pkey, td->alg, td->priv, td->priv_len,
                                         NULL, 0, 1))
            || !TEST_ptr(sctx = EVP_PKEY_CTX_new_from_pkey(lib_ctx, pkey, NULL))
            || !TEST_ptr(sig_alg = EVP_SIGNATURE_fetch(lib_ctx, td->alg, NULL))
            || !TEST_int_eq(EVP_PKEY_sign_message_init(sctx, sig_alg, params), 1)
            || !TEST_int_eq(EVP_PKEY_sign(sctx, NULL, &psig_len,
                                          td->msg, td->msg_len), 1)
            || !TEST_true(EVP_PKEY_get_size_t_param(pkey, OSSL_PKEY_PARAM_MAX_SIZE,
                                                    &sig_len2))
            || !TEST_int_eq(sig_len2, psig_len)
            || !TEST_ptr(psig = OPENSSL_zalloc(psig_len))
            || !TEST_int_eq(EVP_PKEY_sign(sctx, psig, &psig_len,
                                          td->msg, td->msg_len), 1)
            || !TEST_int_eq(EVP_Q_digest(lib_ctx, "SHA256", NULL, psig, psig_len,
                                         digest, &digest_len), 1)
            || !TEST_mem_eq(digest, digest_len, td->sig_digest, td->sig_digest_len))
        goto err;
    ret = 1;
err:
    EVP_SIGNATURE_free(sig_alg);
    EVP_PKEY_free(pkey);
    EVP_PKEY_CTX_free(sctx);
    OPENSSL_free(psig);
    return ret;
}

static int ml_dsa_sigver_test(int tst_id)
{
    int ret = 0;
    const ML_DSA_SIG_VER_TEST_DATA *td = &ml_dsa_sigver_testdata[tst_id];
    EVP_PKEY_CTX *vctx = NULL;
    EVP_PKEY *pkey = NULL;
    EVP_SIGNATURE *sig_alg = NULL;
    OSSL_PARAM params[2], *p = params;
    int encode = 0;

    *p++ = OSSL_PARAM_construct_int(OSSL_SIGNATURE_PARAM_MESSAGE_ENCODING, &encode);
    *p = OSSL_PARAM_construct_end();

    if (!TEST_true(ml_dsa_create_keypair(&pkey, td->alg, NULL, 0,
                                         td->pub, td->pub_len, 1)))
        goto err;

    if (!TEST_ptr(vctx = EVP_PKEY_CTX_new_from_pkey(lib_ctx, pkey, NULL)))
        goto err;
    if (!TEST_ptr(sig_alg = EVP_SIGNATURE_fetch(lib_ctx, td->alg, NULL)))
        goto err;
    if (!TEST_int_eq(EVP_PKEY_verify_message_init(vctx, sig_alg, params), 1)
            || !TEST_int_eq(EVP_PKEY_verify(vctx, td->sig, td->sig_len,
                                            td->msg, td->msg_len), td->expected))
        goto err;
    ret = 1;
err:
    EVP_SIGNATURE_free(sig_alg);
    EVP_PKEY_free(pkey);
    EVP_PKEY_CTX_free(vctx);
    return ret;
}

static int ml_dsa_key_dup_test(void)
{
    int ret = 0;
    const ML_DSA_KEYGEN_TEST_DATA *tst = &ml_dsa_keygen_testdata[0];
    EVP_PKEY *pkey = NULL, *pkey_copy = NULL;
    EVP_PKEY_CTX *ctx = NULL;

    if (!TEST_ptr(pkey = do_gen_key(tst->name, tst->seed, tst->seed_len))
            || !TEST_ptr(pkey_copy = EVP_PKEY_dup(pkey))
            || !TEST_int_eq(EVP_PKEY_eq(pkey, pkey_copy), 1)
            || !TEST_ptr(ctx = EVP_PKEY_CTX_new_from_pkey(lib_ctx, pkey_copy, NULL))
            || !TEST_int_eq(EVP_PKEY_check(ctx), 1))
        goto err;
    ret = 1;
 err:
    EVP_PKEY_free(pkey);
    EVP_PKEY_free(pkey_copy);
    EVP_PKEY_CTX_free(ctx);
    return ret;
}

static int ml_dsa_key_internal_test(void)
{
    int ret = 0;
    ML_DSA_KEY *key = NULL, *key1 = NULL;

    /* check passing NULL is ok */
    ossl_ml_dsa_key_free(NULL);

    /* We should fail to fetch and fail here if the libctx is not set */
    if (!TEST_ptr_null(key = ossl_ml_dsa_key_new(NULL, NULL, EVP_PKEY_ML_DSA_44))
            /* fail if the algorithm is invalid */
            || !TEST_ptr_null(key = ossl_ml_dsa_key_new(lib_ctx, "", NID_undef))
            /* Dup should fail if the src is NULL */
            || !TEST_ptr_null(key1 = ossl_ml_dsa_key_dup(NULL, OSSL_KEYMGMT_SELECT_KEYPAIR))
            || !TEST_ptr(key = ossl_ml_dsa_key_new(lib_ctx, "?fips=yes",
                                                   EVP_PKEY_ML_DSA_44))
            || !TEST_ptr(key1 = ossl_ml_dsa_key_dup(key, OSSL_KEYMGMT_SELECT_KEYPAIR))
            || !TEST_true(ossl_ml_dsa_key_pub_alloc(key1))
            || !TEST_false(ossl_ml_dsa_key_pub_alloc(key1))
            || !TEST_true(ossl_ml_dsa_key_priv_alloc(key))
            || !TEST_false(ossl_ml_dsa_key_priv_alloc(key)))
        goto err;

    ret = 1;
 err:
    ossl_ml_dsa_key_free(key1);
    ossl_ml_dsa_key_free(key);
    return ret;
}

static int from_data_invalid_public_test(void)
{
    int ret = 0;
    const ML_DSA_KEYGEN_TEST_DATA *tst = &ml_dsa_keygen_testdata[0];
    uint8_t *pub = NULL;
    EVP_PKEY *pkey = NULL;

    /* corrupt the public key */
    if (!TEST_ptr(pub = OPENSSL_memdup(tst->pub, tst->pub_len)))
        goto err;
    pub[0] ^= 1;

    if (!TEST_true(ml_dsa_create_keypair(&pkey, tst->name, tst->priv, tst->priv_len,
                                         tst->pub, tst->pub_len, 1))
            || !TEST_true(ml_dsa_create_keypair(&pkey, tst->name, tst->priv, tst->priv_len,
                                                pub, tst->pub_len, 0))
            || !TEST_true(ml_dsa_create_keypair(&pkey, tst->name, tst->priv, tst->priv_len,
                                                tst->pub, tst->pub_len - 1, 0))
            || !TEST_true(ml_dsa_create_keypair(&pkey, tst->name, tst->priv, tst->priv_len,
                                                tst->pub, tst->pub_len + 1, 0))
            || !TEST_true(ml_dsa_create_keypair(&pkey, tst->name, tst->priv, tst->priv_len - 1,
                                                tst->pub, tst->pub_len, 0))
            || !TEST_true(ml_dsa_create_keypair(&pkey, tst->name, tst->priv, tst->priv_len + 1,
                                                tst->pub, tst->pub_len, 0)))
        goto err;

    ret = 1;
 err:
    OPENSSL_free(pub);
    EVP_PKEY_free(pkey);
    return ret;
}

static int from_data_bad_input_test(void)
{
    int ret = 0;
    EVP_PKEY_CTX *ctx = NULL;
    OSSL_PARAM params[2];
    EVP_PKEY *pkey = NULL;
    uint32_t i = 0;

    if (!TEST_ptr(ctx = EVP_PKEY_CTX_new_from_name(lib_ctx, "ML-DSA-44", NULL))
            || !TEST_int_eq(EVP_PKEY_fromdata_init(ctx), 1)
            || !TEST_ptr(EVP_PKEY_fromdata_settable(ctx, OSSL_KEYMGMT_SELECT_KEYPAIR))
            || !TEST_ptr_null(EVP_PKEY_fromdata_settable(ctx, 0)))
        goto err;

    params[0] = OSSL_PARAM_construct_uint32(OSSL_PKEY_PARAM_PRIV_KEY, &i);
    params[1] = OSSL_PARAM_construct_end();
    if (!TEST_int_ne(EVP_PKEY_fromdata(ctx, &pkey, OSSL_KEYMGMT_SELECT_PRIVATE_KEY,
                                       params), 1))
        goto err;
    params[0] = OSSL_PARAM_construct_uint32(OSSL_PKEY_PARAM_PUB_KEY, &i);
    params[1] = OSSL_PARAM_construct_end();
    if (!TEST_int_ne(EVP_PKEY_fromdata(ctx, &pkey, OSSL_KEYMGMT_SELECT_PUBLIC_KEY,
                                       params), 1))
        goto err;
    ret = 1;
 err:
    EVP_PKEY_free(pkey);
    EVP_PKEY_CTX_free(ctx);
    return ret;
}

static int ml_dsa_keygen_drbg_test(void)
{
    int ret = 0;
    EVP_PKEY *pkey = NULL, *pkey2 = NULL, *pkey3 = NULL, *pkey_pub = NULL;
    size_t len = 0;
    uint8_t *priv = NULL, *pub = NULL;

    if (!TEST_ptr(pkey = do_gen_key("ML-DSA-44", NULL, 0))
            || !TEST_ptr(pkey2 = do_gen_key("ML-DSA-44", NULL, 0))
            || !TEST_ptr(pkey3 = do_gen_key("ML-DSA-65", NULL, 0))
            || !TEST_int_eq(EVP_PKEY_eq(pkey, pkey2), 0)
            || !TEST_int_eq(EVP_PKEY_eq(pkey, pkey3), -1)
            || !TEST_int_eq(EVP_PKEY_get_raw_private_key(pkey, NULL, &len), 1)
            || !TEST_int_gt(len, 0)
            || !TEST_ptr(priv = OPENSSL_malloc(len))
            || !TEST_int_eq(EVP_PKEY_get_raw_private_key(pkey, priv, &len), 1)
            || !TEST_int_eq(EVP_PKEY_get_raw_public_key(pkey, NULL, &len), 1)
            || !TEST_int_gt(len, 0)
            || !TEST_ptr(pub = OPENSSL_malloc(len))
            || !TEST_int_eq(EVP_PKEY_get_raw_public_key(pkey, pub, &len), 1)
            /* Load just the public part into a PKEY */
            || !TEST_true(ml_dsa_create_keypair(&pkey_pub, "ML-DSA-44", NULL, 0,
                                                pub, len, 1))
            /* test that the KEY's are equal */
            || !TEST_int_eq(EVP_PKEY_eq(pkey_pub, pkey), 1))
        goto err;
    ret = 1;
 err:
    OPENSSL_free(pub);
    OPENSSL_free(priv);
    EVP_PKEY_free(pkey);
    EVP_PKEY_free(pkey2);
    EVP_PKEY_free(pkey3);
    EVP_PKEY_free(pkey_pub);
    return ret;
}

static uint8_t msg1[] = "Hello World";
static uint8_t ctx1[] = "Context Test";
/* This message size will not fit into the internal buffer - so an alloc occurs */
static uint8_t msg2[2048] = {0};
/* This ctx is larger than 255 and will fail */
static uint8_t ctx2[256] = {0};

static struct sig_params_st {
    uint8_t *msg;
    size_t msg_len;
    uint8_t *ctx;
    size_t ctx_len;
    int encoded;
    int expected;
} sig_params[] = {
    { msg1, sizeof(msg1), NULL, 0, 0, 1 },
    { msg1, sizeof(msg1), NULL, 0, 1, 1 },
    { msg1, sizeof(msg1), ctx1, sizeof(ctx1), 0, 1 },
    { msg1, sizeof(msg1), ctx1, sizeof(ctx1), 1, 1 },
    { msg1, sizeof(msg1), ctx2, sizeof(ctx2), 0, 0 },
    { msg1, sizeof(msg1), ctx2, sizeof(ctx2), 1, 0 },
    { msg2, sizeof(msg2), NULL, 0, 0, 1 },
    { msg2, sizeof(msg2), NULL, 0, 1, 1 },
    { msg2, sizeof(msg2), ctx1, sizeof(ctx1), 0, 1 },
    { msg2, sizeof(msg2), ctx1, sizeof(ctx1), 1, 1 },
    { msg2, sizeof(msg2), ctx2, sizeof(ctx2), 0, 0 },
    { msg2, sizeof(msg2), ctx2, sizeof(ctx2), 1, 0 },
};

static int do_ml_dsa_sign_verify(const char *alg, int tstid)
{
    int ret = 0;
    const struct sig_params_st *sp = &sig_params[tstid];
    EVP_PKEY_CTX *sctx = NULL, *vctx = NULL;
    EVP_PKEY *key = NULL;
    EVP_SIGNATURE *sig_alg = NULL;
    uint8_t *sig = NULL;
    size_t sig_len = 0;
    OSSL_PARAM params[3], *p = params;

    if (!TEST_ptr(key = do_gen_key(alg, NULL, 0)))
        goto err;

    *p++ = OSSL_PARAM_construct_int(OSSL_SIGNATURE_PARAM_MESSAGE_ENCODING,
                                    (int *)&sp->encoded);
    if (sp->ctx != NULL)
        *p++ = OSSL_PARAM_construct_octet_string(OSSL_SIGNATURE_PARAM_CONTEXT_STRING,
                                                 sp->ctx, sp->ctx_len);
    *p++ = OSSL_PARAM_construct_end();

    if (!TEST_ptr(sctx = EVP_PKEY_CTX_new_from_pkey(lib_ctx, key, NULL))
            || !TEST_ptr(sig_alg = EVP_SIGNATURE_fetch(lib_ctx, alg, NULL))
            || !TEST_int_eq(EVP_PKEY_sign_message_init(sctx, sig_alg, params), sp->expected))
        goto err;
    if (sp->expected == 0) {
        ret = 1; /* return true as we expected to fail */
        goto err;
    }
    if (!TEST_int_eq(EVP_PKEY_sign(sctx, NULL, &sig_len, sp->msg, sp->msg_len), 1)
            || !TEST_ptr(sig = OPENSSL_zalloc(sig_len)))
        goto err;
    sig_len--;
    if (!TEST_int_eq(EVP_PKEY_sign(sctx, sig, &sig_len, sp->msg, sp->msg_len), 0))
        goto err;
    sig_len++;
    if (!TEST_int_eq(EVP_PKEY_sign(sctx, sig, &sig_len, sp->msg, sp->msg_len), 1)
            || !TEST_ptr(vctx = EVP_PKEY_CTX_new_from_pkey(lib_ctx, key, NULL))
            || !TEST_int_eq(EVP_PKEY_verify_message_init(vctx, sig_alg, params), 1)
            || !TEST_int_eq(EVP_PKEY_verify(vctx, sig, sig_len, sp->msg, sp->msg_len), 1))
        goto err;
    ret = 1;
err:
    EVP_PKEY_free(key);
    EVP_SIGNATURE_free(sig_alg);
    OPENSSL_free(sig);
    EVP_PKEY_CTX_free(sctx);
    EVP_PKEY_CTX_free(vctx);
    return ret;
}
static int ml_dsa_44_sign_verify_test(int tstid)
{
    return do_ml_dsa_sign_verify("ML-DSA-44", tstid);
}
static int ml_dsa_65_sign_verify_test(int tstid)
{
    return do_ml_dsa_sign_verify("ML-DSA-65", tstid);
}
static int ml_dsa_87_sign_verify_test(int tstid)
{
    return do_ml_dsa_sign_verify("ML-DSA-87", tstid);
}

static int ml_dsa_digest_sign_verify_test(void)
{
    int ret = 0;
    const struct sig_params_st *sp = &sig_params[0];
    EVP_PKEY *key = NULL;
    uint8_t *sig = NULL;
    size_t sig_len = 0;
    OSSL_PARAM params[3], *p = params;
    const char *alg = "ML-DSA-44";
    EVP_MD_CTX *mctx = NULL;

    if (!TEST_ptr(key = do_gen_key(alg, NULL, 0)))
        goto err;

    *p++ = OSSL_PARAM_construct_int(OSSL_SIGNATURE_PARAM_MESSAGE_ENCODING,
                                    (int *)&sp->encoded);
    if (sp->ctx != NULL)
        *p++ = OSSL_PARAM_construct_octet_string(OSSL_SIGNATURE_PARAM_CONTEXT_STRING,
                                                 sp->ctx, sp->ctx_len);
    *p++ = OSSL_PARAM_construct_end();

    if (!TEST_ptr(mctx = EVP_MD_CTX_new())
            || !TEST_int_eq(EVP_DigestSignInit_ex(mctx, NULL, "SHA256",
                                                  lib_ctx, "?fips=true",
                                                  key, params), 0)
            || !TEST_int_eq(EVP_DigestSignInit_ex(mctx, NULL, NULL, lib_ctx,
                                                  "?fips=true", key, params), 1))
        goto err;
    if (sp->expected == 0) {
        ret = 1; /* return true as we expected to fail */
        goto err;
    }
    if (!TEST_int_eq(EVP_DigestSign(mctx, NULL, &sig_len, sp->msg, sp->msg_len), 1)
            || !TEST_ptr(sig = OPENSSL_zalloc(sig_len)))
        goto err;
    sig_len--;
    if (!TEST_int_eq(EVP_DigestSign(mctx, sig, &sig_len, sp->msg, sp->msg_len), 0))
        goto err;
    sig_len++;
    if (!TEST_int_eq(EVP_DigestSignInit_ex(mctx, NULL, NULL, lib_ctx, "?fips=true",
                                           key, params), 1)
            || !TEST_int_eq(EVP_DigestSign(mctx, sig, &sig_len,
                                           sp->msg, sp->msg_len), 1)
            || !TEST_int_eq(EVP_DigestVerifyInit_ex(mctx, NULL, "SHA256",
                                                    lib_ctx, "?fips=true",
                                                    key, params), 0)
            || !TEST_int_eq(EVP_DigestVerifyInit_ex(mctx, NULL, NULL,
                                                    lib_ctx, "?fips=true",
                                                    key, params), 1)
            || !TEST_int_eq(EVP_DigestVerify(mctx, sig, sig_len,
                                             sp->msg, sp->msg_len), 1))
        goto err;
    ret = 1;
err:
    EVP_PKEY_free(key);
    EVP_MD_CTX_free(mctx);
    OPENSSL_free(sig);
    return ret;
}

static int ml_dsa_priv_pub_bad_t0_test(void)
{
    int ret = 0;
    EVP_PKEY *key = NULL;
    ML_DSA_SIG_GEN_TEST_DATA *td = &ml_dsa_siggen_testdata[0];
    uint8_t *priv = OPENSSL_memdup(td->priv, td->priv_len);

    if (!TEST_ptr(priv))
        goto err;
    memcpy(priv, td->priv, td->priv_len);
    /*
     * t0 is at the end of the encoding so corrupt it.
     * This offset is the start of t0 (which is the last 416 * k bytes))
     */
    priv[td->priv_len - 6 * 416] ^= 1;
    if (!TEST_true(ml_dsa_create_keypair(&key, td->alg,
                                         priv, td->priv_len, NULL, 0, 0)))
        goto err;

    priv[td->priv_len - 6 * 416] ^= 1;
    if (!TEST_true(ml_dsa_create_keypair(&key, td->alg,
                                         priv, td->priv_len, NULL, 0, 1)))
        goto err;
    ret = 1;
 err:
    OPENSSL_free(priv);
    EVP_PKEY_free(key);
    return ret;
}

const OPTIONS *test_get_options(void)
{
    static const OPTIONS options[] = {
        OPT_TEST_OPTIONS_DEFAULT_USAGE,
        { "config", OPT_CONFIG_FILE, '<',
          "The configuration file to use for the libctx" },
        { NULL }
    };
    return options;
}

int setup_tests(void)
{
    OPTION_CHOICE o;
    char *config_file = NULL;

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
    if (!test_get_libctx(&lib_ctx, &null_prov, config_file, &lib_prov, NULL))
        return 0;

    ADD_ALL_TESTS(ml_dsa_keygen_test, OSSL_NELEM(ml_dsa_keygen_testdata));
    ADD_ALL_TESTS(ml_dsa_siggen_test, OSSL_NELEM(ml_dsa_siggen_testdata));
    ADD_ALL_TESTS(ml_dsa_sigver_test, OSSL_NELEM(ml_dsa_sigver_testdata));
    ADD_TEST(ml_dsa_key_dup_test);
    ADD_TEST(ml_dsa_key_internal_test);
    ADD_TEST(ml_dsa_keygen_drbg_test);
    ADD_ALL_TESTS(ml_dsa_44_sign_verify_test, OSSL_NELEM(sig_params));
    ADD_ALL_TESTS(ml_dsa_65_sign_verify_test, OSSL_NELEM(sig_params));
    ADD_ALL_TESTS(ml_dsa_87_sign_verify_test, OSSL_NELEM(sig_params));
    ADD_TEST(from_data_invalid_public_test);
    ADD_TEST(from_data_bad_input_test);
    ADD_TEST(ml_dsa_digest_sign_verify_test);
    ADD_TEST(ml_dsa_priv_pub_bad_t0_test);
    return 1;
}

void cleanup_tests(void)
{
    OSSL_PROVIDER_unload(null_prov);
    OSSL_PROVIDER_unload(lib_prov);
    OSSL_LIB_CTX_free(lib_ctx);
}
