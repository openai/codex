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
#include <openssl/param_build.h>
#include <openssl/rand.h>
#include <openssl/pem.h>
#include "crypto/slh_dsa.h"
#include "internal/nelem.h"
#include "testutil.h"
#include "slh_dsa.inc"

typedef enum OPTION_choice {
    OPT_ERR = -1,
    OPT_EOF = 0,
    OPT_CONFIG_FILE,
    OPT_TEST_ENUM
} OPTION_CHOICE;

static OSSL_LIB_CTX *lib_ctx = NULL;
static OSSL_PROVIDER *null_prov = NULL;
static OSSL_PROVIDER *lib_prov = NULL;

static EVP_PKEY *slh_dsa_key_from_data(const char *alg,
                                       const unsigned char *data, size_t datalen,
                                       int public)
{
    int ret;
    EVP_PKEY_CTX *ctx = NULL;
    EVP_PKEY *key = NULL;
    OSSL_PARAM params[2];
    const char *keytype = public ? OSSL_PKEY_PARAM_PUB_KEY : OSSL_PKEY_PARAM_PRIV_KEY;
    int selection = public ? EVP_PKEY_PUBLIC_KEY : EVP_PKEY_KEYPAIR;

    params[0] = OSSL_PARAM_construct_octet_string(keytype, (uint8_t *)data, datalen);
    params[1] = OSSL_PARAM_construct_end();
    ret = TEST_ptr(ctx = EVP_PKEY_CTX_new_from_name(lib_ctx, alg, NULL))
        && TEST_int_eq(EVP_PKEY_fromdata_init(ctx), 1)
        && (EVP_PKEY_fromdata(ctx, &key, selection, params) == 1);
    if (ret == 0) {
        EVP_PKEY_free(key);
        key = NULL;
    }
    EVP_PKEY_CTX_free(ctx);
    return key;
}

static int slh_dsa_create_keypair(EVP_PKEY **pkey, const char *name,
                                  const uint8_t *priv, size_t priv_len,
                                  const uint8_t *pub, size_t pub_len)
{
    int ret = 0;
    EVP_PKEY_CTX *ctx = NULL;
    OSSL_PARAM_BLD *bld = NULL;
    OSSL_PARAM *params = NULL;
    const char *pub_name = OSSL_PKEY_PARAM_PUB_KEY;

    if (!TEST_ptr(bld = OSSL_PARAM_BLD_new())
            || !TEST_true(OSSL_PARAM_BLD_push_octet_string(bld,
                                                           OSSL_PKEY_PARAM_PRIV_KEY,
                                                           priv, priv_len) > 0)
            || !TEST_true(OSSL_PARAM_BLD_push_octet_string(bld,
                                                           pub_name,
                                                           pub, pub_len) > 0)
            || !TEST_ptr(params = OSSL_PARAM_BLD_to_param(bld))
            || !TEST_ptr(ctx = EVP_PKEY_CTX_new_from_name(lib_ctx, name, NULL))
            || !TEST_int_eq(EVP_PKEY_fromdata_init(ctx), 1)
            || !TEST_int_eq(EVP_PKEY_fromdata(ctx, pkey, EVP_PKEY_KEYPAIR,
                                              params), 1))
        goto err;

    ret = 1;
err:
    OSSL_PARAM_free(params);
    OSSL_PARAM_BLD_free(bld);
    EVP_PKEY_CTX_free(ctx);
    return ret;
}

static int slh_dsa_bad_pub_len_test(void)
{
    int ret = 0;
    SLH_DSA_SIG_TEST_DATA *td = &slh_dsa_sig_testdata[0];
    EVP_PKEY *pkey = NULL;
    size_t pub_len = 0;
    unsigned char pubdata[64 + 1];

    if (!TEST_size_t_le(td->pub_len, sizeof(pubdata)))
        goto end;

    OPENSSL_cleanse(pubdata, sizeof(pubdata));
    memcpy(pubdata, td->pub, td->pub_len);

    if (!TEST_ptr_null(pkey = slh_dsa_key_from_data(td->alg, pubdata,
                                                    td->pub_len - 1, 1))
            || !TEST_ptr_null(pkey = slh_dsa_key_from_data(td->alg, pubdata,
                                                           td->pub_len + 1, 1)))
        goto end;

    ret = 1;
end:
    if (ret == 0)
        TEST_note("Incorrectly accepted public key of length %u (expected %u)",
                  (unsigned)pub_len, (unsigned)td->pub_len);
    EVP_PKEY_free(pkey);
    return ret == 1;
}

static int slh_dsa_key_eq_test(void)
{
    int ret = 0;
    size_t i;
    EVP_PKEY *key[4] = { NULL, NULL, NULL, NULL };
    SLH_DSA_SIG_TEST_DATA *td1 = &slh_dsa_sig_testdata[0];
    SLH_DSA_SIG_TEST_DATA *td2 = &slh_dsa_sig_testdata[1];
#ifndef OPENSSL_NO_EC
    EVP_PKEY *eckey = NULL;
#endif

    if (!TEST_ptr(key[0] = slh_dsa_key_from_data(td1->alg, td1->pub, td1->pub_len, 1))
            || !TEST_ptr(key[1] = slh_dsa_key_from_data(td1->alg, td1->pub, td1->pub_len, 1))
            || !TEST_ptr(key[2] = slh_dsa_key_from_data(td2->alg, td2->pub, td2->pub_len, 1))
            || !TEST_ptr(key[3] = EVP_PKEY_dup(key[0])))
        goto end;

    if (!TEST_int_eq(EVP_PKEY_eq(key[0], key[1]), 1)
            || !TEST_int_ne(EVP_PKEY_eq(key[0], key[2]), 1)
            || !TEST_int_eq(EVP_PKEY_eq(key[0], key[3]), 1))
        goto end;

#ifndef OPENSSL_NO_EC
    if (!TEST_ptr(eckey = EVP_PKEY_Q_keygen(lib_ctx, NULL, "EC", "P-256")))
        goto end;
    ret = TEST_int_ne(EVP_PKEY_eq(key[0], eckey), 1);
    EVP_PKEY_free(eckey);
#else
    ret = 1;
#endif
 end:
    for (i = 0; i < OSSL_NELEM(key); ++i)
        EVP_PKEY_free(key[i]);
    return ret;
}

static int slh_dsa_key_validate_test(void)
{
    int ret = 0;
    SLH_DSA_SIG_TEST_DATA *td = &slh_dsa_sig_testdata[0];
    EVP_PKEY_CTX *vctx = NULL;
    EVP_PKEY *key = NULL;

    if (!TEST_ptr(key = slh_dsa_key_from_data(td->alg, td->pub, td->pub_len, 1)))
        return 0;
    if (!TEST_ptr(vctx = EVP_PKEY_CTX_new_from_pkey(lib_ctx, key, NULL)))
        goto end;
    if (!TEST_int_eq(EVP_PKEY_public_check(vctx), 1))
        goto end;
    if (!TEST_int_eq(EVP_PKEY_private_check(vctx), 0))
        goto end;
    if (!TEST_int_eq(EVP_PKEY_pairwise_check(vctx), 0))
        goto end;
    ret = 1;
end:
    EVP_PKEY_CTX_free(vctx);
    EVP_PKEY_free(key);
    return ret;
}

static int slh_dsa_key_validate_failure_test(void)
{
    int ret = 0;
    EVP_PKEY_CTX *vctx = NULL;
    EVP_PKEY *key = NULL;

    /*
     * Loading 128s private key data into a 128f algorithm will have an incorrect
     * public key.
     */
    if (!TEST_ptr(key = slh_dsa_key_from_data("SLH-DSA-SHA2-128f",
                                              slh_dsa_sha2_128s_0_keygen_priv,
                                              sizeof(slh_dsa_sha2_128s_0_keygen_priv), 0)))
        return 0;
    if (!TEST_ptr(vctx = EVP_PKEY_CTX_new_from_pkey(lib_ctx, key, NULL)))
        goto end;
    if (!TEST_int_eq(EVP_PKEY_pairwise_check(vctx), 0))
        goto end;
    ret = 1;
end:
    EVP_PKEY_CTX_free(vctx);
    EVP_PKEY_free(key);
    return ret;
}

/*
 * Rather than having to store the full signature into a file, we just do a
 * verify using the output of a sign. The sign test already does a Known answer
 * test (KAT) using the digest of the signature, so this should be sufficient to
 * run as a KAT for the verify.
 */
static int do_slh_dsa_verify(const SLH_DSA_SIG_TEST_DATA *td,
                             uint8_t *sig, size_t sig_len)
{
    int ret = 0;
    EVP_PKEY_CTX *vctx = NULL;
    EVP_PKEY *key = NULL;
    EVP_SIGNATURE *sig_alg = NULL;
    OSSL_PARAM params[2], *p = params;
    int encode = 0;

    *p++ = OSSL_PARAM_construct_int(OSSL_SIGNATURE_PARAM_MESSAGE_ENCODING, &encode);
    *p = OSSL_PARAM_construct_end();

    if (!TEST_ptr(key = slh_dsa_key_from_data(td->alg, td->pub, td->pub_len, 1)))
        return 0;
    if (!TEST_ptr(vctx = EVP_PKEY_CTX_new_from_pkey(lib_ctx, key, NULL)))
        goto err;
    if (!TEST_ptr(sig_alg = EVP_SIGNATURE_fetch(lib_ctx, td->alg, NULL)))
        goto err;
    if (!TEST_int_eq(EVP_PKEY_verify_message_init(vctx, sig_alg, params), 1)
            || !TEST_int_eq(EVP_PKEY_verify(vctx, sig, sig_len,
                                            td->msg, td->msg_len), 1))
        goto err;
    ret = 1;
err:
    EVP_SIGNATURE_free(sig_alg);
    EVP_PKEY_free(key);
    EVP_PKEY_CTX_free(vctx);
    return ret;
}

static int slh_dsa_sign_verify_test(int tst_id)
{
    int ret = 0;
    SLH_DSA_SIG_TEST_DATA *td = &slh_dsa_sig_testdata[tst_id];
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
     * The keygen path is tested via slh_dsa_keygen_test
     */
    if (!slh_dsa_create_keypair(&pkey, td->alg, td->priv, td->priv_len,
                                td->pub, td->pub_len))
        goto err;

    if (!TEST_ptr(sctx = EVP_PKEY_CTX_new_from_pkey(lib_ctx, pkey, NULL)))
        goto err;
    if (!TEST_ptr(sig_alg = EVP_SIGNATURE_fetch(lib_ctx, td->alg, NULL)))
        goto err;
    if (!TEST_int_eq(EVP_PKEY_sign_message_init(sctx, sig_alg, params), 1)
            || !TEST_int_eq(EVP_PKEY_sign(sctx, NULL, &psig_len,
                                          td->msg, td->msg_len), 1)
            || !TEST_true(EVP_PKEY_get_size_t_param(pkey, OSSL_PKEY_PARAM_MAX_SIZE,
                                                    &sig_len2))
            || !TEST_int_eq(sig_len2, psig_len)
            || !TEST_ptr(psig = OPENSSL_zalloc(psig_len))
            || !TEST_int_eq(EVP_PKEY_sign(sctx, psig, &psig_len,
                                          td->msg, td->msg_len), 1))
        goto err;
    if (!TEST_int_eq(EVP_Q_digest(lib_ctx, "SHA256", NULL, psig, psig_len,
                                  digest, &digest_len), 1))
        goto err;
    if (!TEST_mem_eq(digest, digest_len, td->sig_digest, td->sig_digest_len))
        goto err;
    if (!do_slh_dsa_verify(td, psig, psig_len))
        goto err;
    ret = 1;
err:
    EVP_SIGNATURE_free(sig_alg);
    EVP_PKEY_free(pkey);
    EVP_PKEY_CTX_free(sctx);
    OPENSSL_free(psig);
    return ret;
}

static EVP_PKEY *do_gen_key(const char *alg,
                            const uint8_t *seed, size_t seed_len)
{
    EVP_PKEY *pkey = NULL;
    EVP_PKEY_CTX *ctx = NULL;
    OSSL_PARAM params[2], *p = params;

    if (seed_len != 0)
        *p++ = OSSL_PARAM_construct_octet_string(OSSL_PKEY_PARAM_SLH_DSA_SEED,
                                                 (char *)seed, seed_len);
    *p = OSSL_PARAM_construct_end();

    if (!TEST_ptr(ctx = EVP_PKEY_CTX_new_from_name(lib_ctx, alg, NULL))
            || !TEST_int_eq(EVP_PKEY_keygen_init(ctx), 1)
            || !TEST_int_eq(EVP_PKEY_CTX_set_params(ctx, params), 1)
            || !TEST_int_eq(EVP_PKEY_generate(ctx, &pkey), 1))
        pkey = NULL;

    EVP_PKEY_CTX_free(ctx);
    return pkey;
}

static int slh_dsa_keygen_test(int tst_id)
{
    int ret = 0;
    const SLH_DSA_KEYGEN_TEST_DATA *tst = &slh_dsa_keygen_testdata[tst_id];
    EVP_PKEY *pkey = NULL;
    uint8_t priv[64 * 2], pub[32 * 2];
    size_t priv_len, pub_len;
    size_t key_len = tst->priv_len;
    size_t n = key_len / 4;
    int bits = 0, sec_bits = 0, sig_len = 0;

    if (!TEST_ptr(pkey = do_gen_key(tst->name, tst->priv, key_len - n)))
        goto err;

    if (!TEST_true(EVP_PKEY_get_octet_string_param(pkey, OSSL_PKEY_PARAM_PRIV_KEY,
                                                   priv, sizeof(priv), &priv_len)))
        goto err;
    if (!TEST_true(EVP_PKEY_get_octet_string_param(pkey, OSSL_PKEY_PARAM_PUB_KEY,
                                                   pub, sizeof(pub), &pub_len)))
        goto err;
    if (!TEST_true(EVP_PKEY_get_int_param(pkey, OSSL_PKEY_PARAM_BITS, &bits))
            || !TEST_int_eq(bits, 8 * 2 * n)
            || !TEST_true(EVP_PKEY_get_int_param(pkey, OSSL_PKEY_PARAM_SECURITY_BITS,
                                                 &sec_bits))
            || !TEST_int_eq(sec_bits, 8 * n)
            || !TEST_true(EVP_PKEY_get_int_param(pkey, OSSL_PKEY_PARAM_MAX_SIZE,
                                                 &sig_len))
            || !TEST_int_ge(sig_len, 7856)
            || !TEST_int_le(sig_len, 49856))
        goto err;

    if (!TEST_size_t_eq(priv_len, key_len)
            || !TEST_size_t_eq(pub_len, key_len / 2))
        goto err;
    if (!TEST_mem_eq(pub, pub_len, tst->priv + 2 * n, 2 * n))
        goto err;
    ret = 1;
err:
    EVP_PKEY_free(pkey);
    return ret;
}

static int slh_dsa_usage_test(void)
{
    int ret = 0;
    EVP_CIPHER *cipher = NULL; /* Used to encrypt the private key */
    char *pass = "Password";
    BIO *pub_bio = NULL, *priv_bio = NULL;
    EVP_PKEY_CTX *gctx = NULL, *sctx = NULL, *vctx = NULL;
    EVP_PKEY *gkey = NULL, *pub = NULL, *priv = NULL;
    EVP_SIGNATURE *sig_alg = NULL;
    uint8_t *sig  = NULL;
    size_t sig_len = 0;
    uint8_t msg[] = "Hello World";
    size_t msg_len = sizeof(msg) - 1;

    /* Generate a key */
    if (!TEST_ptr(gctx = EVP_PKEY_CTX_new_from_name(lib_ctx, "SLH-DSA-SHA2-128s", NULL))
            || !TEST_int_eq(EVP_PKEY_keygen_init(gctx), 1)
            || !TEST_int_eq(EVP_PKEY_keygen(gctx, &gkey), 1))
        goto err;

    /* Save it to a BIO - it uses a mem bio for testing */
    if (!TEST_ptr(pub_bio = BIO_new(BIO_s_mem()))
            || !TEST_ptr(priv_bio = BIO_new(BIO_s_mem()))
            || !TEST_ptr(cipher = EVP_CIPHER_fetch(lib_ctx, "AES-256-CBC", NULL))
            || !TEST_true(PEM_write_bio_PUBKEY_ex(pub_bio, gkey, lib_ctx, NULL))
            || !TEST_true(PEM_write_bio_PrivateKey_ex(priv_bio, gkey, cipher,
                                                      NULL, 0, NULL, (void *)pass,
                                                      lib_ctx, NULL)))
        goto err;

    /* Read the private key and add to a signing ctx */
    if (!TEST_ptr(PEM_read_bio_PrivateKey_ex(priv_bio, &priv, NULL, pass, lib_ctx, NULL))
            || !TEST_ptr(sctx = EVP_PKEY_CTX_new_from_pkey(lib_ctx, priv, NULL))
            || !TEST_ptr(sig_alg = EVP_SIGNATURE_fetch(lib_ctx, "SLH-DSA-SHA2-128s", NULL))
            || !TEST_int_eq(EVP_PKEY_sign_message_init(sctx, sig_alg, NULL), 1))
        goto err;
    /* Determine the size of the signature & allocate space */
    if (!TEST_int_eq(EVP_PKEY_sign(sctx, NULL, &sig_len, msg, msg_len), 1)
            || !TEST_ptr(sig = OPENSSL_malloc(sig_len))
            || !TEST_int_eq(EVP_PKEY_sign(sctx, sig, &sig_len, msg, msg_len), 1))
        goto err;
    if (!TEST_true(EVP_PKEY_pairwise_check(sctx))
            || !TEST_true(EVP_PKEY_public_check(sctx))
            || !TEST_true(EVP_PKEY_private_check(sctx)))
        goto err;
    /* Read the public key and add to a verify ctx */
    if (!TEST_ptr(PEM_read_bio_PUBKEY_ex(pub_bio, &pub, NULL, NULL, lib_ctx, NULL))
            || !TEST_ptr(vctx = EVP_PKEY_CTX_new_from_pkey(lib_ctx, pub, NULL)))
        goto err;
    /* verify the signature */
    if (!TEST_int_eq(EVP_PKEY_verify_message_init(vctx, sig_alg, NULL), 1)
            || !TEST_int_eq(EVP_PKEY_verify(vctx, sig, sig_len, msg, msg_len), 1))
        goto err;

    ret = 1;
err:
    EVP_CIPHER_free(cipher);
    EVP_SIGNATURE_free(sig_alg);
    EVP_PKEY_free(gkey);
    EVP_PKEY_free(pub);
    EVP_PKEY_free(priv);
    EVP_PKEY_CTX_free(gctx);
    EVP_PKEY_CTX_free(sctx);
    EVP_PKEY_CTX_free(vctx);
    BIO_free(pub_bio);
    BIO_free(priv_bio);
    OPENSSL_free(sig);
    return ret;
}

static int slh_dsa_deterministic_usage_test(void)
{
    int ret = 0;
    EVP_CIPHER *cipher = NULL; /* Used to encrypt the private key */
    char *pass = "Password";
    BIO *pub_bio = NULL, *priv_bio = NULL;
    EVP_PKEY_CTX *gctx = NULL, *sctx = NULL, *vctx = NULL, *dupctx = NULL;
    EVP_PKEY *gkey = NULL, *pub = NULL, *priv = NULL;
    EVP_SIGNATURE *sig_alg = NULL;
    uint8_t *sig  = NULL;
    size_t sig_len = 0, len = 0;
    uint8_t msg[] = { 0x01, 0x02, 0x03, 0x04 };
    size_t msg_len = sizeof(msg);
    const SLH_DSA_KEYGEN_TEST_DATA *tst = &slh_dsa_keygen_testdata[0];
    size_t key_len = tst->priv_len / 2;
    size_t n = key_len / 2;
    int deterministic = 1;
    OSSL_PARAM params[2], *p = params;

    /* Generate a key */
    if (!TEST_ptr(gkey = do_gen_key(tst->name, tst->priv, key_len + n)))
        goto err;

    /* Save it to a BIO - it uses a mem bio for testing */
    if (!TEST_ptr(pub_bio = BIO_new(BIO_s_mem()))
            || !TEST_ptr(priv_bio = BIO_new(BIO_s_mem()))
            || !TEST_ptr(cipher = EVP_CIPHER_fetch(lib_ctx, "AES-256-CBC", NULL))
            || !TEST_true(PEM_write_bio_PUBKEY_ex(pub_bio, gkey, lib_ctx, NULL))
            || !TEST_true(PEM_write_bio_PrivateKey_ex(priv_bio, gkey, cipher,
                                                      NULL, 0, NULL, (void *)pass,
                                                      lib_ctx, NULL)))
        goto err;

    *p++ = OSSL_PARAM_construct_int(OSSL_SIGNATURE_PARAM_DETERMINISTIC, &deterministic);
    *p = OSSL_PARAM_construct_end();

    /* Read the private key and add to a signing ctx */
    if (!TEST_ptr(PEM_read_bio_PrivateKey_ex(priv_bio, &priv, NULL, pass, lib_ctx, NULL))
            || !TEST_ptr(sctx = EVP_PKEY_CTX_new_from_pkey(lib_ctx, priv, NULL))
            /* Init the signature */
            || !TEST_ptr(sig_alg = EVP_SIGNATURE_fetch(lib_ctx, tst->name, NULL))
            || !TEST_int_eq(EVP_PKEY_sign_message_init(sctx, sig_alg, params), 1))
        goto err;

    if (!TEST_ptr(dupctx = EVP_PKEY_CTX_dup(sctx)))
        goto err;

    /* Determine the size of the signature & allocate space */
    if (!TEST_int_eq(EVP_PKEY_sign(sctx, NULL, &sig_len, msg, msg_len), 1))
        goto err;
    len = sig_len;
    if (!TEST_ptr(sig = OPENSSL_zalloc(sig_len * 2))
            || !TEST_int_eq(EVP_PKEY_sign(sctx, sig, &len, msg, msg_len), 1)
            || !TEST_size_t_eq(sig_len, len)
            || !TEST_int_eq(EVP_PKEY_sign(dupctx, sig + sig_len, &len,
                                          msg, msg_len), 1)
            || !TEST_size_t_eq(sig_len, len))
        goto err;
    /* Read the public key and add to a verify ctx */
    if (!TEST_ptr(PEM_read_bio_PUBKEY_ex(pub_bio, &pub, NULL, NULL, lib_ctx, NULL))
            || !TEST_ptr(vctx = EVP_PKEY_CTX_new_from_pkey(lib_ctx, pub, NULL)))
        goto err;
    EVP_PKEY_CTX_free(dupctx);

    /* verify the signature */
    if (!TEST_int_eq(EVP_PKEY_verify_message_init(vctx, sig_alg, NULL), 1)
            || !TEST_ptr(dupctx = EVP_PKEY_CTX_dup(vctx))
            || !TEST_int_eq(EVP_PKEY_verify(vctx, sig, sig_len, msg, msg_len), 1)
            || !TEST_int_eq(EVP_PKEY_verify(dupctx, sig + sig_len, sig_len,
                                            msg, msg_len), 1))
        goto err;
    ret = 1;
err:
    EVP_CIPHER_free(cipher);
    EVP_SIGNATURE_free(sig_alg);
    EVP_PKEY_free(gkey);
    EVP_PKEY_free(pub);
    EVP_PKEY_free(priv);
    EVP_PKEY_CTX_free(gctx);
    EVP_PKEY_CTX_free(sctx);
    EVP_PKEY_CTX_free(vctx);
    EVP_PKEY_CTX_free(dupctx);
    BIO_free(pub_bio);
    BIO_free(priv_bio);
    OPENSSL_free(sig);
    return ret;
}

static int slh_dsa_digest_sign_verify_test(void)
{
    int ret = 0;
    EVP_PKEY *key = NULL;
    uint8_t *sig = NULL;
    size_t sig_len = 0;
    OSSL_PARAM params[3], *p = params;
    const char *alg = "SLH-DSA-SHA2-128s";
    EVP_MD_CTX *mctx = NULL;
    static uint8_t context[] = "A context String";
    static uint8_t msg[] = "Hello World";
    size_t msg_len = sizeof(msg);

    if (!TEST_ptr(key = do_gen_key(alg, NULL, 0)))
        goto err;

    *p++ = OSSL_PARAM_construct_octet_string(OSSL_SIGNATURE_PARAM_CONTEXT_STRING,
                                             context, sizeof(context));
    *p++ = OSSL_PARAM_construct_end();

    if (!TEST_ptr(mctx = EVP_MD_CTX_new())
            || !TEST_int_eq(EVP_DigestSignInit_ex(mctx, NULL, "SHA256",
                                                  lib_ctx, "?fips=true",
                                                  key, params), 0)
            || !TEST_int_eq(EVP_DigestSignInit_ex(mctx, NULL, NULL, lib_ctx,
                                                  "?fips=true", key, params), 1))
        goto err;
    if (!TEST_int_eq(EVP_DigestSign(mctx, NULL, &sig_len, msg, msg_len), 1)
            || !TEST_ptr(sig = OPENSSL_zalloc(sig_len)))
        goto err;
    sig_len--;
    if (!TEST_int_eq(EVP_DigestSign(mctx, sig, &sig_len, msg, msg_len), 0))
        goto err;
    sig_len++;
    if (!TEST_int_eq(EVP_DigestSignInit_ex(mctx, NULL, NULL, lib_ctx, "?fips=true",
                                           key, params), 1)
            || !TEST_int_eq(EVP_DigestSign(mctx, sig, &sig_len, msg, msg_len), 1)
            || !TEST_int_eq(EVP_DigestVerifyInit_ex(mctx, NULL, "SHA256",
                                                    lib_ctx, "?fips=true",
                                                    key, params), 0)
            || !TEST_int_eq(EVP_DigestVerifyInit_ex(mctx, NULL, NULL,
                                                    lib_ctx, "?fips=true",
                                                    key, params), 1)
            || !TEST_int_eq(EVP_DigestVerify(mctx, sig, sig_len, msg, msg_len), 1))
        goto err;
    ret = 1;
err:
    EVP_PKEY_free(key);
    EVP_MD_CTX_free(mctx);
    OPENSSL_free(sig);
    return ret;
}

static int slh_dsa_keygen_invalid_test(void)
{
    int ret = 0;
    const SLH_DSA_KEYGEN_TEST_DATA *tst = &slh_dsa_keygen_testdata[0];
    EVP_PKEY *pkey = NULL;
    EVP_PKEY_CTX *ctx = NULL;
    OSSL_PARAM params[2], *p = params;
    size_t key_len = tst->priv_len;
    size_t n = key_len / 4;
    uint8_t seed[128] = {0};

    if (!TEST_ptr(ctx = EVP_PKEY_CTX_new_from_name(lib_ctx, tst->name, NULL))
            || !TEST_int_eq(EVP_PKEY_keygen_init(ctx), 1))
        goto err;

    /* Test the set fails if the seed is larger than the internal buffer */
    p[0] = OSSL_PARAM_construct_octet_string(OSSL_PKEY_PARAM_SLH_DSA_SEED,
                                             seed, 97);
    p[1] = OSSL_PARAM_construct_end();
    if (!TEST_int_eq(EVP_PKEY_CTX_set_params(ctx, params), 0))
        goto err;

    /* Test the generate fails if the seed is not the correct size */
    p[0] = OSSL_PARAM_construct_octet_string(OSSL_PKEY_PARAM_SLH_DSA_SEED,
                                             seed, n * 3 - 1);
    p[1] = OSSL_PARAM_construct_end();

    if (!TEST_int_eq(EVP_PKEY_CTX_set_params(ctx, params), 1)
            || !TEST_int_eq(EVP_PKEY_generate(ctx, &pkey), 0))
        goto err;

    /* Test the generate fails if the seed is not the correct size */
    p[0] = OSSL_PARAM_construct_octet_string(OSSL_PKEY_PARAM_SLH_DSA_SEED,
                                             seed, n * 3 + 1);
    p[1] = OSSL_PARAM_construct_end();
    if (!TEST_int_eq(EVP_PKEY_CTX_set_params(ctx, params), 1)
            || !TEST_int_eq(EVP_PKEY_generate(ctx, &pkey), 0))
        goto err;
    ret = 1;
err:
    EVP_PKEY_free(pkey);
    EVP_PKEY_CTX_free(ctx);
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

    ADD_TEST(slh_dsa_bad_pub_len_test);
    ADD_TEST(slh_dsa_key_validate_test);
    ADD_TEST(slh_dsa_key_validate_failure_test);
    ADD_TEST(slh_dsa_key_eq_test);
    ADD_TEST(slh_dsa_usage_test);
    ADD_TEST(slh_dsa_deterministic_usage_test);
    ADD_ALL_TESTS(slh_dsa_sign_verify_test, OSSL_NELEM(slh_dsa_sig_testdata));
    ADD_ALL_TESTS(slh_dsa_keygen_test, OSSL_NELEM(slh_dsa_keygen_testdata));
    ADD_TEST(slh_dsa_digest_sign_verify_test);
    ADD_TEST(slh_dsa_keygen_invalid_test);
    return 1;
}

void cleanup_tests(void)
{
    OSSL_PROVIDER_unload(null_prov);
    OSSL_PROVIDER_unload(lib_prov);
    OSSL_LIB_CTX_free(lib_ctx);
}
