/*
 * Copyright 2015-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * This is the OSSLTEST engine. It provides deliberately crippled digest
 * implementations for test purposes. It is highly insecure and must NOT be
 * used for any purpose except testing
 */

/* We need to use some engine deprecated APIs */
#define OPENSSL_SUPPRESS_DEPRECATED

/*
 * SHA low level APIs are deprecated for public use, but still ok for
 * internal use.  Note, that due to symbols not being exported, only the
 * #defines and type definitions can be accessed, function calls are not
 * available.  The digest lengths, block sizes and sizeof(CTX) are used herein
 * for several different digests.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include <string.h>
#include "internal/common.h" /* for CHECK_AND_SKIP_CASE_PREFIX */

#include <openssl/engine.h>
#include <openssl/sha.h>
#include <openssl/md5.h>
#include <openssl/rsa.h>
#include <openssl/evp.h>
#include <openssl/modes.h>
#include <openssl/aes.h>
#include <openssl/rand.h>
#include <openssl/crypto.h>
#include <openssl/pem.h>
#include <crypto/evp.h>

#include "e_ossltest_err.c"

/* Engine Id and Name */
static const char *engine_ossltest_id = "ossltest";
static const char *engine_ossltest_name = "OpenSSL Test engine support";


/* Engine Lifetime functions */
static int ossltest_destroy(ENGINE *e);
static int ossltest_init(ENGINE *e);
static int ossltest_finish(ENGINE *e);
void ENGINE_load_ossltest(void);


/* Set up digests */
static int ossltest_digests(ENGINE *e, const EVP_MD **digest,
                          const int **nids, int nid);
static const RAND_METHOD *ossltest_rand_method(void);

/* MD5 */
static int digest_md5_init(EVP_MD_CTX *ctx);
static int digest_md5_update(EVP_MD_CTX *ctx, const void *data,
                             size_t count);
static int digest_md5_final(EVP_MD_CTX *ctx, unsigned char *md);

static EVP_MD *_hidden_md5_md = NULL;
static const EVP_MD *digest_md5(void)
{
    if (_hidden_md5_md == NULL) {
        EVP_MD *md;

        if ((md = EVP_MD_meth_new(NID_md5, NID_md5WithRSAEncryption)) == NULL
            || !EVP_MD_meth_set_result_size(md, MD5_DIGEST_LENGTH)
            || !EVP_MD_meth_set_input_blocksize(md, MD5_CBLOCK)
            || !EVP_MD_meth_set_app_datasize(md,
                                             sizeof(EVP_MD *) + sizeof(MD5_CTX))
            || !EVP_MD_meth_set_flags(md, 0)
            || !EVP_MD_meth_set_init(md, digest_md5_init)
            || !EVP_MD_meth_set_update(md, digest_md5_update)
            || !EVP_MD_meth_set_final(md, digest_md5_final)) {
            EVP_MD_meth_free(md);
            md = NULL;
        }
        _hidden_md5_md = md;
    }
    return _hidden_md5_md;
}

/* SHA1 */
static int digest_sha1_init(EVP_MD_CTX *ctx);
static int digest_sha1_update(EVP_MD_CTX *ctx, const void *data,
                              size_t count);
static int digest_sha1_final(EVP_MD_CTX *ctx, unsigned char *md);

static EVP_MD *_hidden_sha1_md = NULL;
static const EVP_MD *digest_sha1(void)
{
    if (_hidden_sha1_md == NULL) {
        EVP_MD *md;

        if ((md = EVP_MD_meth_new(NID_sha1, NID_sha1WithRSAEncryption)) == NULL
            || !EVP_MD_meth_set_result_size(md, SHA_DIGEST_LENGTH)
            || !EVP_MD_meth_set_input_blocksize(md, SHA_CBLOCK)
            || !EVP_MD_meth_set_app_datasize(md,
                                             sizeof(EVP_MD *) + sizeof(SHA_CTX))
            || !EVP_MD_meth_set_flags(md, EVP_MD_FLAG_DIGALGID_ABSENT)
            || !EVP_MD_meth_set_init(md, digest_sha1_init)
            || !EVP_MD_meth_set_update(md, digest_sha1_update)
            || !EVP_MD_meth_set_final(md, digest_sha1_final)) {
            EVP_MD_meth_free(md);
            md = NULL;
        }
        _hidden_sha1_md = md;
    }
    return _hidden_sha1_md;
}

/* SHA256 */
static int digest_sha256_init(EVP_MD_CTX *ctx);
static int digest_sha256_update(EVP_MD_CTX *ctx, const void *data,
                                size_t count);
static int digest_sha256_final(EVP_MD_CTX *ctx, unsigned char *md);

static EVP_MD *_hidden_sha256_md = NULL;
static const EVP_MD *digest_sha256(void)
{
    if (_hidden_sha256_md == NULL) {
        EVP_MD *md;

        if ((md = EVP_MD_meth_new(NID_sha256, NID_sha256WithRSAEncryption)) == NULL
            || !EVP_MD_meth_set_result_size(md, SHA256_DIGEST_LENGTH)
            || !EVP_MD_meth_set_input_blocksize(md, SHA256_CBLOCK)
            || !EVP_MD_meth_set_app_datasize(md,
                                             sizeof(EVP_MD *) + sizeof(SHA256_CTX))
            || !EVP_MD_meth_set_flags(md, EVP_MD_FLAG_DIGALGID_ABSENT)
            || !EVP_MD_meth_set_init(md, digest_sha256_init)
            || !EVP_MD_meth_set_update(md, digest_sha256_update)
            || !EVP_MD_meth_set_final(md, digest_sha256_final)) {
            EVP_MD_meth_free(md);
            md = NULL;
        }
        _hidden_sha256_md = md;
    }
    return _hidden_sha256_md;
}

/* SHA384/SHA512 */
static int digest_sha384_init(EVP_MD_CTX *ctx);
static int digest_sha384_update(EVP_MD_CTX *ctx, const void *data,
                                size_t count);
static int digest_sha384_final(EVP_MD_CTX *ctx, unsigned char *md);

static int digest_sha512_init(EVP_MD_CTX *ctx);
static int digest_sha512_update(EVP_MD_CTX *ctx, const void *data,
                                size_t count);
static int digest_sha512_final(EVP_MD_CTX *ctx, unsigned char *md);

static EVP_MD *_hidden_sha384_md = NULL;
static const EVP_MD *digest_sha384(void)
{
    if (_hidden_sha384_md == NULL) {
        EVP_MD *md;

        if ((md = EVP_MD_meth_new(NID_sha384, NID_sha384WithRSAEncryption)) == NULL
            || !EVP_MD_meth_set_result_size(md, SHA384_DIGEST_LENGTH)
            || !EVP_MD_meth_set_input_blocksize(md, SHA512_CBLOCK)
            || !EVP_MD_meth_set_app_datasize(md,
                                             sizeof(EVP_MD *) + sizeof(SHA512_CTX))
            || !EVP_MD_meth_set_flags(md, EVP_MD_FLAG_DIGALGID_ABSENT)
            || !EVP_MD_meth_set_init(md, digest_sha384_init)
            || !EVP_MD_meth_set_update(md, digest_sha384_update)
            || !EVP_MD_meth_set_final(md, digest_sha384_final)) {
            EVP_MD_meth_free(md);
            md = NULL;
        }
        _hidden_sha384_md = md;
    }
    return _hidden_sha384_md;
}
static EVP_MD *_hidden_sha512_md = NULL;
static const EVP_MD *digest_sha512(void)
{
    if (_hidden_sha512_md == NULL) {
        EVP_MD *md;

        if ((md = EVP_MD_meth_new(NID_sha512, NID_sha512WithRSAEncryption)) == NULL
            || !EVP_MD_meth_set_result_size(md, SHA512_DIGEST_LENGTH)
            || !EVP_MD_meth_set_input_blocksize(md, SHA512_CBLOCK)
            || !EVP_MD_meth_set_app_datasize(md,
                                             sizeof(EVP_MD *) + sizeof(SHA512_CTX))
            || !EVP_MD_meth_set_flags(md, EVP_MD_FLAG_DIGALGID_ABSENT)
            || !EVP_MD_meth_set_init(md, digest_sha512_init)
            || !EVP_MD_meth_set_update(md, digest_sha512_update)
            || !EVP_MD_meth_set_final(md, digest_sha512_final)) {
            EVP_MD_meth_free(md);
            md = NULL;
        }
        _hidden_sha512_md = md;
    }
    return _hidden_sha512_md;
}
static void destroy_digests(void)
{
    EVP_MD_meth_free(_hidden_md5_md);
    _hidden_md5_md = NULL;
    EVP_MD_meth_free(_hidden_sha1_md);
    _hidden_sha1_md = NULL;
    EVP_MD_meth_free(_hidden_sha256_md);
    _hidden_sha256_md = NULL;
    EVP_MD_meth_free(_hidden_sha384_md);
    _hidden_sha384_md = NULL;
    EVP_MD_meth_free(_hidden_sha512_md);
    _hidden_sha512_md = NULL;
}
static int ossltest_digest_nids(const int **nids)
{
    static int digest_nids[6] = { 0, 0, 0, 0, 0, 0 };
    static int pos = 0;
    static int init = 0;

    if (!init) {
        const EVP_MD *md;
        if ((md = digest_md5()) != NULL)
            digest_nids[pos++] = EVP_MD_get_type(md);
        if ((md = digest_sha1()) != NULL)
            digest_nids[pos++] = EVP_MD_get_type(md);
        if ((md = digest_sha256()) != NULL)
            digest_nids[pos++] = EVP_MD_get_type(md);
        if ((md = digest_sha384()) != NULL)
            digest_nids[pos++] = EVP_MD_get_type(md);
        if ((md = digest_sha512()) != NULL)
            digest_nids[pos++] = EVP_MD_get_type(md);
        digest_nids[pos] = 0;
        init = 1;
    }
    *nids = digest_nids;
    return pos;
}

/* Setup ciphers */
static int ossltest_ciphers(ENGINE *, const EVP_CIPHER **,
                            const int **, int);

static int ossltest_cipher_nids[] = {
    NID_aes_128_cbc, NID_aes_128_gcm,
    NID_aes_128_cbc_hmac_sha1, 0
};

/* AES128 */

static int ossltest_aes128_init_key(EVP_CIPHER_CTX *ctx,
                                    const unsigned char *key,
                                    const unsigned char *iv, int enc);
static int ossltest_aes128_cbc_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                                      const unsigned char *in, size_t inl);
static int ossltest_aes128_gcm_init_key(EVP_CIPHER_CTX *ctx,
                                        const unsigned char *key,
                                        const unsigned char *iv, int enc);
static int ossltest_aes128_gcm_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                                      const unsigned char *in, size_t inl);
static int ossltest_aes128_gcm_ctrl(EVP_CIPHER_CTX *ctx, int type, int arg,
                                    void *ptr);
static int ossltest_aes128_cbc_hmac_sha1_init_key(EVP_CIPHER_CTX *ctx,
                                                  const unsigned char *key,
                                                  const unsigned char *iv,
                                                  int enc);
static int ossltest_aes128_cbc_hmac_sha1_cipher(EVP_CIPHER_CTX *ctx,
                                                unsigned char *out,
                                                const unsigned char *in,
                                                size_t inl);
static int ossltest_aes128_cbc_hmac_sha1_ctrl(EVP_CIPHER_CTX *ctx, int type,
                                              int arg, void *ptr);

typedef struct {
    size_t payload_length;      /* AAD length in decrypt case */
    unsigned int tls_ver;
} EVP_AES_HMAC_SHA1;

static EVP_CIPHER *_hidden_aes_128_cbc = NULL;
static const EVP_CIPHER *ossltest_aes_128_cbc(void)
{
    if (_hidden_aes_128_cbc == NULL
        && ((_hidden_aes_128_cbc = EVP_CIPHER_meth_new(NID_aes_128_cbc,
                                                       16 /* block size */,
                                                       16 /* key len */)) == NULL
            || !EVP_CIPHER_meth_set_iv_length(_hidden_aes_128_cbc,16)
            || !EVP_CIPHER_meth_set_flags(_hidden_aes_128_cbc,
                                          EVP_CIPH_FLAG_DEFAULT_ASN1
                                          | EVP_CIPH_CBC_MODE)
            || !EVP_CIPHER_meth_set_init(_hidden_aes_128_cbc,
                                         ossltest_aes128_init_key)
            || !EVP_CIPHER_meth_set_do_cipher(_hidden_aes_128_cbc,
                                              ossltest_aes128_cbc_cipher)
            || !EVP_CIPHER_meth_set_impl_ctx_size(_hidden_aes_128_cbc,
                    EVP_CIPHER_impl_ctx_size(EVP_aes_128_cbc())))) {
        EVP_CIPHER_meth_free(_hidden_aes_128_cbc);
        _hidden_aes_128_cbc = NULL;
    }
    return _hidden_aes_128_cbc;
}

static EVP_CIPHER *_hidden_aes_128_gcm = NULL;

#define AES_GCM_FLAGS   (EVP_CIPH_FLAG_DEFAULT_ASN1 \
                | EVP_CIPH_CUSTOM_IV | EVP_CIPH_FLAG_CUSTOM_CIPHER \
                | EVP_CIPH_ALWAYS_CALL_INIT | EVP_CIPH_CTRL_INIT \
                | EVP_CIPH_CUSTOM_COPY |EVP_CIPH_FLAG_AEAD_CIPHER \
                | EVP_CIPH_GCM_MODE)

static const EVP_CIPHER *ossltest_aes_128_gcm(void)
{
    if (_hidden_aes_128_gcm == NULL
        && ((_hidden_aes_128_gcm = EVP_CIPHER_meth_new(NID_aes_128_gcm,
                                                       1 /* block size */,
                                                       16 /* key len */)) == NULL
            || !EVP_CIPHER_meth_set_iv_length(_hidden_aes_128_gcm,12)
            || !EVP_CIPHER_meth_set_flags(_hidden_aes_128_gcm, AES_GCM_FLAGS)
            || !EVP_CIPHER_meth_set_init(_hidden_aes_128_gcm,
                                         ossltest_aes128_gcm_init_key)
            || !EVP_CIPHER_meth_set_do_cipher(_hidden_aes_128_gcm,
                                              ossltest_aes128_gcm_cipher)
            || !EVP_CIPHER_meth_set_ctrl(_hidden_aes_128_gcm,
                                              ossltest_aes128_gcm_ctrl)
            || !EVP_CIPHER_meth_set_impl_ctx_size(_hidden_aes_128_gcm,
                    EVP_CIPHER_impl_ctx_size(EVP_aes_128_gcm())))) {
        EVP_CIPHER_meth_free(_hidden_aes_128_gcm);
        _hidden_aes_128_gcm = NULL;
    }
    return _hidden_aes_128_gcm;
}

static EVP_CIPHER *_hidden_aes_128_cbc_hmac_sha1 = NULL;

static const EVP_CIPHER *ossltest_aes_128_cbc_hmac_sha1(void)
{
    if (_hidden_aes_128_cbc_hmac_sha1 == NULL
        && ((_hidden_aes_128_cbc_hmac_sha1
             = EVP_CIPHER_meth_new(NID_aes_128_cbc_hmac_sha1,
                                   16 /* block size */,
                                   16 /* key len */)) == NULL
            || !EVP_CIPHER_meth_set_iv_length(_hidden_aes_128_cbc_hmac_sha1,16)
            || !EVP_CIPHER_meth_set_flags(_hidden_aes_128_cbc_hmac_sha1,
                   EVP_CIPH_CBC_MODE | EVP_CIPH_FLAG_DEFAULT_ASN1 |
                   EVP_CIPH_FLAG_AEAD_CIPHER)
            || !EVP_CIPHER_meth_set_init(_hidden_aes_128_cbc_hmac_sha1,
                   ossltest_aes128_cbc_hmac_sha1_init_key)
            || !EVP_CIPHER_meth_set_do_cipher(_hidden_aes_128_cbc_hmac_sha1,
                   ossltest_aes128_cbc_hmac_sha1_cipher)
            || !EVP_CIPHER_meth_set_ctrl(_hidden_aes_128_cbc_hmac_sha1,
                                         ossltest_aes128_cbc_hmac_sha1_ctrl)
            || !EVP_CIPHER_meth_set_set_asn1_params(_hidden_aes_128_cbc_hmac_sha1,
                   EVP_CIPH_FLAG_DEFAULT_ASN1 ? NULL : EVP_CIPHER_set_asn1_iv)
            || !EVP_CIPHER_meth_set_get_asn1_params(_hidden_aes_128_cbc_hmac_sha1,
                   EVP_CIPH_FLAG_DEFAULT_ASN1 ? NULL : EVP_CIPHER_get_asn1_iv)
            || !EVP_CIPHER_meth_set_impl_ctx_size(_hidden_aes_128_cbc_hmac_sha1,
                   sizeof(EVP_AES_HMAC_SHA1)))) {
        EVP_CIPHER_meth_free(_hidden_aes_128_cbc_hmac_sha1);
        _hidden_aes_128_cbc_hmac_sha1 = NULL;
    }
    return _hidden_aes_128_cbc_hmac_sha1;
}

static void destroy_ciphers(void)
{
    EVP_CIPHER_meth_free(_hidden_aes_128_cbc);
    EVP_CIPHER_meth_free(_hidden_aes_128_gcm);
    EVP_CIPHER_meth_free(_hidden_aes_128_cbc_hmac_sha1);
    _hidden_aes_128_cbc = NULL;
    _hidden_aes_128_gcm = NULL;
    _hidden_aes_128_cbc_hmac_sha1 = NULL;
}

/* Key loading */
static EVP_PKEY *load_key(ENGINE *eng, const char *key_id, int pub,
                          UI_METHOD *ui_method, void *ui_data)
{
    BIO *in;
    EVP_PKEY *key;

    if (!CHECK_AND_SKIP_CASE_PREFIX(key_id, "ot:"))
        return NULL;

    fprintf(stderr, "[ossltest]Loading %s key %s\n",
            pub ? "Public" : "Private", key_id);
    in = BIO_new_file(key_id, "r");
    if (!in)
        return NULL;
    if (pub)
        key = PEM_read_bio_PUBKEY(in, NULL, 0, NULL);
    else
        key = PEM_read_bio_PrivateKey(in, NULL, 0, NULL);
    BIO_free(in);
    return key;
}

static EVP_PKEY *ossltest_load_privkey(ENGINE *eng, const char *key_id,
                                       UI_METHOD *ui_method, void *ui_data)
{
    return load_key(eng, key_id, 0, ui_method, ui_data);
}

static EVP_PKEY *ossltest_load_pubkey(ENGINE *eng, const char *key_id,
                                      UI_METHOD *ui_method, void *ui_data)
{
    return load_key(eng, key_id, 1, ui_method, ui_data);
}


static int bind_ossltest(ENGINE *e)
{
    /* Ensure the ossltest error handling is set up */
    ERR_load_OSSLTEST_strings();

    if (!ENGINE_set_id(e, engine_ossltest_id)
        || !ENGINE_set_name(e, engine_ossltest_name)
        || !ENGINE_set_digests(e, ossltest_digests)
        || !ENGINE_set_ciphers(e, ossltest_ciphers)
        || !ENGINE_set_RAND(e, ossltest_rand_method())
        || !ENGINE_set_destroy_function(e, ossltest_destroy)
        || !ENGINE_set_load_privkey_function(e, ossltest_load_privkey)
        || !ENGINE_set_load_pubkey_function(e, ossltest_load_pubkey)
        || !ENGINE_set_init_function(e, ossltest_init)
        || !ENGINE_set_finish_function(e, ossltest_finish)) {
        OSSLTESTerr(OSSLTEST_F_BIND_OSSLTEST, OSSLTEST_R_INIT_FAILED);
        return 0;
    }

    return 1;
}

#ifndef OPENSSL_NO_DYNAMIC_ENGINE
static int bind_helper(ENGINE *e, const char *id)
{
    if (id && (strcmp(id, engine_ossltest_id) != 0))
        return 0;
    if (!bind_ossltest(e))
        return 0;
    return 1;
}

IMPLEMENT_DYNAMIC_CHECK_FN()
    IMPLEMENT_DYNAMIC_BIND_FN(bind_helper)
#endif

static ENGINE *engine_ossltest(void)
{
    ENGINE *ret = ENGINE_new();
    if (ret == NULL)
        return NULL;
    if (!bind_ossltest(ret)) {
        ENGINE_free(ret);
        return NULL;
    }
    return ret;
}

void ENGINE_load_ossltest(void)
{
    /* Copied from eng_[openssl|dyn].c */
    ENGINE *toadd = engine_ossltest();
    if (!toadd)
        return;
    ENGINE_add(toadd);
    ENGINE_free(toadd);
    ERR_clear_error();
}


static int ossltest_init(ENGINE *e)
{
    return 1;
}


static int ossltest_finish(ENGINE *e)
{
    return 1;
}


static int ossltest_destroy(ENGINE *e)
{
    destroy_digests();
    destroy_ciphers();
    ERR_unload_OSSLTEST_strings();
    return 1;
}

static int ossltest_digests(ENGINE *e, const EVP_MD **digest,
                          const int **nids, int nid)
{
    int ok = 1;
    if (!digest) {
        /* We are returning a list of supported nids */
        return ossltest_digest_nids(nids);
    }
    /* We are being asked for a specific digest */
    switch (nid) {
    case NID_md5:
        *digest = digest_md5();
        break;
    case NID_sha1:
        *digest = digest_sha1();
        break;
    case NID_sha256:
        *digest = digest_sha256();
        break;
    case NID_sha384:
        *digest = digest_sha384();
        break;
    case NID_sha512:
        *digest = digest_sha512();
        break;
    default:
        ok = 0;
        *digest = NULL;
        break;
    }
    return ok;
}

static int ossltest_ciphers(ENGINE *e, const EVP_CIPHER **cipher,
                          const int **nids, int nid)
{
    int ok = 1;
    if (!cipher) {
        /* We are returning a list of supported nids */
        *nids = ossltest_cipher_nids;
        return (sizeof(ossltest_cipher_nids) - 1)
               / sizeof(ossltest_cipher_nids[0]);
    }
    /* We are being asked for a specific cipher */
    switch (nid) {
    case NID_aes_128_cbc:
        *cipher = ossltest_aes_128_cbc();
        break;
    case NID_aes_128_gcm:
        *cipher = ossltest_aes_128_gcm();
        break;
    case NID_aes_128_cbc_hmac_sha1:
        *cipher = ossltest_aes_128_cbc_hmac_sha1();
        break;
    default:
        ok = 0;
        *cipher = NULL;
        break;
    }
    return ok;
}

static void fill_known_data(unsigned char *md, unsigned int len)
{
    unsigned int i;

    for (i=0; i<len; i++) {
        md[i] = (unsigned char)(i & 0xff);
    }
}

/*
 * MD5 implementation. We go through the motions of doing MD5 by deferring to
 * the standard implementation. Then we overwrite the result with a will defined
 * value, so that all "MD5" digests using the test engine always end up with
 * the same value.
 */
static int digest_md5_init(EVP_MD_CTX *ctx)
{
   return EVP_MD_meth_get_init(EVP_md5())(ctx);
}

static int digest_md5_update(EVP_MD_CTX *ctx, const void *data,
                             size_t count)
{
    return EVP_MD_meth_get_update(EVP_md5())(ctx, data, count);
}

static int digest_md5_final(EVP_MD_CTX *ctx, unsigned char *md)
{
    int ret = EVP_MD_meth_get_final(EVP_md5())(ctx, md);

    if (ret > 0) {
        fill_known_data(md, MD5_DIGEST_LENGTH);
    }
    return ret;
}

/*
 * SHA1 implementation.
 */
static int digest_sha1_init(EVP_MD_CTX *ctx)
{
    return EVP_MD_meth_get_init(EVP_sha1())(ctx);
}

static int digest_sha1_update(EVP_MD_CTX *ctx, const void *data,
                              size_t count)
{
    return EVP_MD_meth_get_update(EVP_sha1())(ctx, data, count);
}

static int digest_sha1_final(EVP_MD_CTX *ctx, unsigned char *md)
{
    int ret = EVP_MD_meth_get_final(EVP_sha1())(ctx, md);

    if (ret > 0) {
        fill_known_data(md, SHA_DIGEST_LENGTH);
    }
    return ret;
}

/*
 * SHA256 implementation.
 */
static int digest_sha256_init(EVP_MD_CTX *ctx)
{
    return EVP_MD_meth_get_init(EVP_sha256())(ctx);
}

static int digest_sha256_update(EVP_MD_CTX *ctx, const void *data,
                                size_t count)
{
    return EVP_MD_meth_get_update(EVP_sha256())(ctx, data, count);
}

static int digest_sha256_final(EVP_MD_CTX *ctx, unsigned char *md)
{
    int ret = EVP_MD_meth_get_final(EVP_sha256())(ctx, md);

    if (ret > 0) {
        fill_known_data(md, SHA256_DIGEST_LENGTH);
    }
    return ret;
}

/*
 * SHA384 implementation.
 */
static int digest_sha384_init(EVP_MD_CTX *ctx)
{
    return EVP_MD_meth_get_init(EVP_sha384())(ctx);
}

static int digest_sha384_update(EVP_MD_CTX *ctx, const void *data,
                                size_t count)
{
    return EVP_MD_meth_get_update(EVP_sha384())(ctx, data, count);
}

static int digest_sha384_final(EVP_MD_CTX *ctx, unsigned char *md)
{
    int ret = EVP_MD_meth_get_final(EVP_sha384())(ctx, md);

    if (ret > 0) {
        fill_known_data(md, SHA384_DIGEST_LENGTH);
    }
    return ret;
}

/*
 * SHA512 implementation.
 */
static int digest_sha512_init(EVP_MD_CTX *ctx)
{
    return EVP_MD_meth_get_init(EVP_sha512())(ctx);
}

static int digest_sha512_update(EVP_MD_CTX *ctx, const void *data,
                                size_t count)
{
    return EVP_MD_meth_get_update(EVP_sha512())(ctx, data, count);
}

static int digest_sha512_final(EVP_MD_CTX *ctx, unsigned char *md)
{
    int ret = EVP_MD_meth_get_final(EVP_sha512())(ctx, md);

    if (ret > 0) {
        fill_known_data(md, SHA512_DIGEST_LENGTH);
    }
    return ret;
}

/*
 * AES128 Implementation
 */

static int ossltest_aes128_init_key(EVP_CIPHER_CTX *ctx,
                                    const unsigned char *key,
                                    const unsigned char *iv, int enc)
{
    return EVP_CIPHER_meth_get_init(EVP_aes_128_cbc()) (ctx, key, iv, enc);
}

static int ossltest_aes128_cbc_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                                      const unsigned char *in, size_t inl)
{
    unsigned char *tmpbuf;
    int ret;

    tmpbuf = OPENSSL_malloc(inl);

    /* OPENSSL_malloc will return NULL if inl == 0 */
    if (tmpbuf == NULL && inl > 0)
        return -1;

    /* Remember what we were asked to encrypt */
    if (tmpbuf != NULL)
        memcpy(tmpbuf, in, inl);

    /* Go through the motions of encrypting it */
    ret = EVP_CIPHER_meth_get_do_cipher(EVP_aes_128_cbc())(ctx, out, in, inl);

    /* Throw it all away and just use the plaintext as the output */
    if (tmpbuf != NULL)
        memcpy(out, tmpbuf, inl);
    OPENSSL_free(tmpbuf);

    return ret;
}

static int ossltest_aes128_gcm_init_key(EVP_CIPHER_CTX *ctx,
                                        const unsigned char *key,
                                        const unsigned char *iv, int enc)
{
    return EVP_CIPHER_meth_get_init(EVP_aes_128_gcm()) (ctx, key, iv, enc);
}

static int ossltest_aes128_gcm_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                                      const unsigned char *in, size_t inl)
{
    unsigned char *tmpbuf = OPENSSL_malloc(inl);

    /* OPENSSL_malloc will return NULL if inl == 0 */
    if (tmpbuf == NULL && inl > 0)
        return -1;

    /* Remember what we were asked to encrypt */
    if (tmpbuf != NULL)
        memcpy(tmpbuf, in, inl);

    /* Go through the motions of encrypting it */
    EVP_CIPHER_meth_get_do_cipher(EVP_aes_128_gcm())(ctx, out, in, inl);

    /* Throw it all away and just use the plaintext as the output */
    if (tmpbuf != NULL && out != NULL)
        memcpy(out, tmpbuf, inl);
    OPENSSL_free(tmpbuf);

    return inl;
}

static int ossltest_aes128_gcm_ctrl(EVP_CIPHER_CTX *ctx, int type, int arg,
                                    void *ptr)
{
    /* Pass the ctrl down */
    int ret = EVP_CIPHER_meth_get_ctrl(EVP_aes_128_gcm())(ctx, type, arg, ptr);

    if (ret <= 0)
        return ret;

    switch (type) {
    case EVP_CTRL_AEAD_GET_TAG:
        /* Always give the same tag */
        memset(ptr, 0, EVP_GCM_TLS_TAG_LEN);
        break;

    default:
        break;
    }

    return 1;
}

#define NO_PAYLOAD_LENGTH       ((size_t)-1)
# define data(ctx) ((EVP_AES_HMAC_SHA1 *)EVP_CIPHER_CTX_get_cipher_data(ctx))

static int ossltest_aes128_cbc_hmac_sha1_init_key(EVP_CIPHER_CTX *ctx,
                                                  const unsigned char *inkey,
                                                  const unsigned char *iv,
                                                  int enc)
{
    EVP_AES_HMAC_SHA1 *key = data(ctx);
    key->payload_length = NO_PAYLOAD_LENGTH;
    return 1;
}

static int ossltest_aes128_cbc_hmac_sha1_cipher(EVP_CIPHER_CTX *ctx,
                                                unsigned char *out,
                                                const unsigned char *in,
                                                size_t len)
{
    EVP_AES_HMAC_SHA1 *key = data(ctx);
    unsigned int l;
    size_t plen = key->payload_length;

    key->payload_length = NO_PAYLOAD_LENGTH;

    if (len % AES_BLOCK_SIZE)
        return 0;

    if (EVP_CIPHER_CTX_is_encrypting(ctx)) {
        if (plen == NO_PAYLOAD_LENGTH)
            plen = len;
        else if (len !=
                 ((plen + SHA_DIGEST_LENGTH +
                   AES_BLOCK_SIZE) & -AES_BLOCK_SIZE))
            return 0;

        memmove(out, in, plen);

        if (plen != len) {      /* "TLS" mode of operation */
            /* calculate HMAC and append it to payload */
            fill_known_data(out + plen, SHA_DIGEST_LENGTH);

            /* pad the payload|hmac */
            plen += SHA_DIGEST_LENGTH;
            for (l = len - plen - 1; plen < len; plen++)
                out[plen] = l;
        }
    } else {
        /* decrypt HMAC|padding at once */
        memmove(out, in, len);

        if (plen != NO_PAYLOAD_LENGTH) { /* "TLS" mode of operation */
            unsigned int maxpad, pad;

            if (key->tls_ver >= TLS1_1_VERSION) {
                if (len < (AES_BLOCK_SIZE + SHA_DIGEST_LENGTH + 1))
                    return 0;

                /* omit explicit iv */
                in += AES_BLOCK_SIZE;
                out += AES_BLOCK_SIZE;
                len -= AES_BLOCK_SIZE;
            } else if (len < (SHA_DIGEST_LENGTH + 1))
                return 0;

            /* figure out payload length */
            pad = out[len - 1];
            maxpad = len - (SHA_DIGEST_LENGTH + 1);
            if (pad > maxpad)
                return 0;
            for (plen = len - pad - 1; plen < len; plen++)
                if (out[plen] != pad)
                    return 0;
        }
    }

    return 1;
}

static int ossltest_aes128_cbc_hmac_sha1_ctrl(EVP_CIPHER_CTX *ctx, int type,
                                              int arg, void *ptr)
{
    EVP_AES_HMAC_SHA1 *key = data(ctx);

    switch (type) {
    case EVP_CTRL_AEAD_SET_MAC_KEY:
        return 1;

    case EVP_CTRL_AEAD_TLS1_AAD:
        {
            unsigned char *p = ptr;
            unsigned int len;

            if (arg != EVP_AEAD_TLS1_AAD_LEN)
                return -1;

            len = p[arg - 2] << 8 | p[arg - 1];
            key->tls_ver = p[arg - 4] << 8 | p[arg - 3];

            if (EVP_CIPHER_CTX_is_encrypting(ctx)) {
                key->payload_length = len;
                if (key->tls_ver >= TLS1_1_VERSION) {
                    if (len < AES_BLOCK_SIZE)
                        return 0;
                    len -= AES_BLOCK_SIZE;
                    p[arg - 2] = len >> 8;
                    p[arg - 1] = len;
                }

                return (int)(((len + SHA_DIGEST_LENGTH +
                               AES_BLOCK_SIZE) & -AES_BLOCK_SIZE)
                             - len);
            } else {
                key->payload_length = arg;

                return SHA_DIGEST_LENGTH;
            }
        }
    default:
        return -1;
    }
}

static int ossltest_rand_bytes(unsigned char *buf, int num)
{
    unsigned char val = 1;

    while (--num >= 0)
        *buf++ = val++;
    return 1;
}

static int ossltest_rand_status(void)
{
    return 1;
}

static const RAND_METHOD *ossltest_rand_method(void)
{

    static RAND_METHOD osslt_rand_meth = {
        NULL,
        ossltest_rand_bytes,
        NULL,
        NULL,
        ossltest_rand_bytes,
        ossltest_rand_status
    };

    return &osslt_rand_meth;
}
