/*
 * Copyright 2016-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/ssl.h>
#include <openssl/evp.h>

#include "../ssl/ssl_local.h"
#include "internal/ssl_unwrap.h"
#include "testutil.h"

#define IVLEN   12
#define KEYLEN  16

/*
 * Based on the test vectors available in:
 * https://tools.ietf.org/html/draft-ietf-tls-tls13-vectors-06
 */

static unsigned char hs_start_hash[] = {
0xc6, 0xc9, 0x18, 0xad, 0x2f, 0x41, 0x99, 0xd5, 0x59, 0x8e, 0xaf, 0x01, 0x16,
0xcb, 0x7a, 0x5c, 0x2c, 0x14, 0xcb, 0x54, 0x78, 0x12, 0x18, 0x88, 0x8d, 0xb7,
0x03, 0x0d, 0xd5, 0x0d, 0x5e, 0x6d
};

static unsigned char hs_full_hash[] = {
0xf8, 0xc1, 0x9e, 0x8c, 0x77, 0xc0, 0x38, 0x79, 0xbb, 0xc8, 0xeb, 0x6d, 0x56,
0xe0, 0x0d, 0xd5, 0xd8, 0x6e, 0xf5, 0x59, 0x27, 0xee, 0xfc, 0x08, 0xe1, 0xb0,
0x02, 0xb6, 0xec, 0xe0, 0x5d, 0xbf
};

static unsigned char early_secret[] = {
0x33, 0xad, 0x0a, 0x1c, 0x60, 0x7e, 0xc0, 0x3b, 0x09, 0xe6, 0xcd, 0x98, 0x93,
0x68, 0x0c, 0xe2, 0x10, 0xad, 0xf3, 0x00, 0xaa, 0x1f, 0x26, 0x60, 0xe1, 0xb2,
0x2e, 0x10, 0xf1, 0x70, 0xf9, 0x2a
};

static unsigned char ecdhe_secret[] = {
0x81, 0x51, 0xd1, 0x46, 0x4c, 0x1b, 0x55, 0x53, 0x36, 0x23, 0xb9, 0xc2, 0x24,
0x6a, 0x6a, 0x0e, 0x6e, 0x7e, 0x18, 0x50, 0x63, 0xe1, 0x4a, 0xfd, 0xaf, 0xf0,
0xb6, 0xe1, 0xc6, 0x1a, 0x86, 0x42
};

static unsigned char handshake_secret[] = {
0x5b, 0x4f, 0x96, 0x5d, 0xf0, 0x3c, 0x68, 0x2c, 0x46, 0xe6, 0xee, 0x86, 0xc3,
0x11, 0x63, 0x66, 0x15, 0xa1, 0xd2, 0xbb, 0xb2, 0x43, 0x45, 0xc2, 0x52, 0x05,
0x95, 0x3c, 0x87, 0x9e, 0x8d, 0x06
};

static const char *client_hts_label = "c hs traffic";

static unsigned char client_hts[] = {
0xe2, 0xe2, 0x32, 0x07, 0xbd, 0x93, 0xfb, 0x7f, 0xe4, 0xfc, 0x2e, 0x29, 0x7a,
0xfe, 0xab, 0x16, 0x0e, 0x52, 0x2b, 0x5a, 0xb7, 0x5d, 0x64, 0xa8, 0x6e, 0x75,
0xbc, 0xac, 0x3f, 0x3e, 0x51, 0x03
};

static unsigned char client_hts_key[] = {
0x26, 0x79, 0xa4, 0x3e, 0x1d, 0x76, 0x78, 0x40, 0x34, 0xea, 0x17, 0x97, 0xd5,
0xad, 0x26, 0x49
};

static unsigned char client_hts_iv[] = {
0x54, 0x82, 0x40, 0x52, 0x90, 0xdd, 0x0d, 0x2f, 0x81, 0xc0, 0xd9, 0x42
};

static const char *server_hts_label = "s hs traffic";

static unsigned char server_hts[] = {
0x3b, 0x7a, 0x83, 0x9c, 0x23, 0x9e, 0xf2, 0xbf, 0x0b, 0x73, 0x05, 0xa0, 0xe0,
0xc4, 0xe5, 0xa8, 0xc6, 0xc6, 0x93, 0x30, 0xa7, 0x53, 0xb3, 0x08, 0xf5, 0xe3,
0xa8, 0x3a, 0xa2, 0xef, 0x69, 0x79
};

static unsigned char server_hts_key[] = {
0xc6, 0x6c, 0xb1, 0xae, 0xc5, 0x19, 0xdf, 0x44, 0xc9, 0x1e, 0x10, 0x99, 0x55,
0x11, 0xac, 0x8b
};

static unsigned char server_hts_iv[] = {
0xf7, 0xf6, 0x88, 0x4c, 0x49, 0x81, 0x71, 0x6c, 0x2d, 0x0d, 0x29, 0xa4
};

static unsigned char master_secret[] = {
0x5c, 0x79, 0xd1, 0x69, 0x42, 0x4e, 0x26, 0x2b, 0x56, 0x32, 0x03, 0x62, 0x7b,
0xe4, 0xeb, 0x51, 0x03, 0x3f, 0x58, 0x8c, 0x43, 0xc9, 0xce, 0x03, 0x73, 0x37,
0x2d, 0xbc, 0xbc, 0x01, 0x85, 0xa7
};

static const char *client_ats_label = "c ap traffic";

static unsigned char client_ats[] = {
0xe2, 0xf0, 0xdb, 0x6a, 0x82, 0xe8, 0x82, 0x80, 0xfc, 0x26, 0xf7, 0x3c, 0x89,
0x85, 0x4e, 0xe8, 0x61, 0x5e, 0x25, 0xdf, 0x28, 0xb2, 0x20, 0x79, 0x62, 0xfa,
0x78, 0x22, 0x26, 0xb2, 0x36, 0x26
};

static unsigned char client_ats_key[] = {
0x88, 0xb9, 0x6a, 0xd6, 0x86, 0xc8, 0x4b, 0xe5, 0x5a, 0xce, 0x18, 0xa5, 0x9c,
0xce, 0x5c, 0x87
};

static unsigned char client_ats_iv[] = {
0xb9, 0x9d, 0xc5, 0x8c, 0xd5, 0xff, 0x5a, 0xb0, 0x82, 0xfd, 0xad, 0x19
};

static const char *server_ats_label = "s ap traffic";

static unsigned char server_ats[] = {
0x5b, 0x73, 0xb1, 0x08, 0xd9, 0xac, 0x1b, 0x9b, 0x0c, 0x82, 0x48, 0xca, 0x39,
0x26, 0xec, 0x6e, 0x7b, 0xc4, 0x7e, 0x41, 0x17, 0x06, 0x96, 0x39, 0x87, 0xec,
0x11, 0x43, 0x5d, 0x30, 0x57, 0x19
};

static unsigned char server_ats_key[] = {
0xa6, 0x88, 0xeb, 0xb5, 0xac, 0x82, 0x6d, 0x6f, 0x42, 0xd4, 0x5c, 0x0c, 0xc4,
0x4b, 0x9b, 0x7d
};

static unsigned char server_ats_iv[] = {
0xc1, 0xca, 0xd4, 0x42, 0x5a, 0x43, 0x8b, 0x5d, 0xe7, 0x14, 0x83, 0x0a
};

/* Mocked out implementations of various functions */
int ssl3_digest_cached_records(SSL_CONNECTION *s, int keep)
{
    return 1;
}

static int full_hash = 0;

/* Give a hash of the currently set handshake */
int ssl_handshake_hash(SSL_CONNECTION *s, unsigned char *out, size_t outlen,
                       size_t *hashlen)
{
    if (sizeof(hs_start_hash) > outlen
            || sizeof(hs_full_hash) != sizeof(hs_start_hash))
        return 0;

    if (full_hash) {
        memcpy(out, hs_full_hash, sizeof(hs_full_hash));
        *hashlen = sizeof(hs_full_hash);
    } else {
        memcpy(out, hs_start_hash, sizeof(hs_start_hash));
        *hashlen = sizeof(hs_start_hash);
    }

    return 1;
}

const EVP_MD *ssl_handshake_md(SSL_CONNECTION *s)
{
    return EVP_sha256();
}

int ssl_cipher_get_evp_cipher(SSL_CTX *ctx, const SSL_CIPHER *sslc,
                                     const EVP_CIPHER **enc)
{
    return 0;
}

int ssl_cipher_get_evp_md_mac(SSL_CTX *ctx, const SSL_CIPHER *sslc,
                              const EVP_MD **md,
                              int *mac_pkey_type, size_t *mac_secret_size)
{
    return 0;
}

int ssl_cipher_get_evp(SSL_CTX *ctx, const SSL_SESSION *s,
                       const EVP_CIPHER **enc, const EVP_MD **md,
                       int *mac_pkey_type, size_t *mac_secret_size,
                       SSL_COMP **comp, int use_etm)

{
    return 0;
}

int tls1_alert_code(int code)
{
    return code;
}

int ssl_log_secret(SSL_CONNECTION *sc,
                   const char *label,
                   const uint8_t *secret,
                   size_t secret_len)
{
    return 1;
}

const EVP_MD *ssl_md(SSL_CTX *ctx, int idx)
{
    return EVP_sha256();
}

void ossl_statem_send_fatal(SSL_CONNECTION *s, int al)
{
}

void ossl_statem_fatal(SSL_CONNECTION *s, int al, int reason,
                       const char *fmt, ...)
{
}

int ossl_statem_export_allowed(SSL_CONNECTION *s)
{
    return 1;
}

int ossl_statem_export_early_allowed(SSL_CONNECTION *s)
{
    return 1;
}

void ssl_evp_cipher_free(const EVP_CIPHER *cipher)
{
}

void ssl_evp_md_free(const EVP_MD *md)
{
}

int ssl_set_new_record_layer(SSL_CONNECTION *s, int version, int direction,
                             int level, unsigned char *secret, size_t secretlen,
                             unsigned char *key, size_t keylen,
                             unsigned char *iv,  size_t ivlen,
                             unsigned char *mackey, size_t mackeylen,
                             const EVP_CIPHER *ciph, size_t taglen,
                             int mactype, const EVP_MD *md,
                             const SSL_COMP *comp, const EVP_MD *kdfdigest)
{
    return 0;
}

/* End of mocked out code */

static int test_secret(SSL_CONNECTION *s, unsigned char *prk,
                       const unsigned char *label, size_t labellen,
                       const unsigned char *ref_secret,
                       const unsigned char *ref_key, const unsigned char *ref_iv)
{
    size_t hashsize;
    unsigned char gensecret[EVP_MAX_MD_SIZE];
    unsigned char hash[EVP_MAX_MD_SIZE];
    unsigned char key[KEYLEN];
    unsigned char iv[IVLEN];
    const EVP_MD *md = ssl_handshake_md(s);

    if (!ssl_handshake_hash(s, hash, sizeof(hash), &hashsize)) {
        TEST_error("Failed to get hash");
        return 0;
    }

    if (!tls13_hkdf_expand(s, md, prk, label, labellen, hash, hashsize,
                           gensecret, hashsize, 1)) {
        TEST_error("Secret generation failed");
        return 0;
    }

    if (!TEST_mem_eq(gensecret, hashsize, ref_secret, hashsize))
        return 0;

    if (!tls13_derive_key(s, md, gensecret, key, KEYLEN)) {
        TEST_error("Key generation failed");
        return 0;
    }

    if (!TEST_mem_eq(key, KEYLEN, ref_key, KEYLEN))
        return 0;

    if (!tls13_derive_iv(s, md, gensecret, iv, IVLEN)) {
        TEST_error("IV generation failed");
        return 0;
    }

    if (!TEST_mem_eq(iv, IVLEN, ref_iv, IVLEN))
        return 0;

    return 1;
}

static int test_handshake_secrets(void)
{
    SSL_CTX *ctx = NULL;
    SSL *ssl = NULL;
    SSL_CONNECTION *s;
    int ret = 0;
    size_t hashsize;
    unsigned char out_master_secret[EVP_MAX_MD_SIZE];
    size_t master_secret_length;

    ctx = SSL_CTX_new(TLS_method());
    if (!TEST_ptr(ctx))
        goto err;

    ssl = SSL_new(ctx);
    if (!TEST_ptr(ssl) || !TEST_ptr(s = SSL_CONNECTION_FROM_SSL_ONLY(ssl)))
        goto err;

    s->session = SSL_SESSION_new();
    if (!TEST_ptr(s->session))
        goto err;

    if (!TEST_true(tls13_generate_secret(s, ssl_handshake_md(s), NULL, NULL, 0,
                                         (unsigned char *)&s->early_secret))) {
        TEST_info("Early secret generation failed");
        goto err;
    }

    if (!TEST_mem_eq(s->early_secret, sizeof(early_secret),
                     early_secret, sizeof(early_secret))) {
        TEST_info("Early secret does not match");
        goto err;
    }

    if (!TEST_true(tls13_generate_handshake_secret(s, ecdhe_secret,
                                                   sizeof(ecdhe_secret)))) {
        TEST_info("Handshake secret generation failed");
        goto err;
    }

    if (!TEST_mem_eq(s->handshake_secret, sizeof(handshake_secret),
                     handshake_secret, sizeof(handshake_secret)))
        goto err;

    hashsize = EVP_MD_get_size(ssl_handshake_md(s));
    if (!TEST_size_t_eq(sizeof(client_hts), hashsize))
        goto err;
    if (!TEST_size_t_eq(sizeof(client_hts_key), KEYLEN))
        goto err;
    if (!TEST_size_t_eq(sizeof(client_hts_iv), IVLEN))
        goto err;

    if (!TEST_true(test_secret(s, s->handshake_secret,
                               (unsigned char *)client_hts_label,
                               strlen(client_hts_label), client_hts,
                               client_hts_key, client_hts_iv))) {
        TEST_info("Client handshake secret test failed");
        goto err;
    }

    if (!TEST_size_t_eq(sizeof(server_hts), hashsize))
        goto err;
    if (!TEST_size_t_eq(sizeof(server_hts_key), KEYLEN))
        goto err;
    if (!TEST_size_t_eq(sizeof(server_hts_iv), IVLEN))
        goto err;

    if (!TEST_true(test_secret(s, s->handshake_secret,
                               (unsigned char *)server_hts_label,
                               strlen(server_hts_label), server_hts,
                               server_hts_key, server_hts_iv))) {
        TEST_info("Server handshake secret test failed");
        goto err;
    }

    /*
     * Ensure the mocked out ssl_handshake_hash() returns the full handshake
     * hash.
     */
    full_hash = 1;

    if (!TEST_true(tls13_generate_master_secret(s, out_master_secret,
                                                s->handshake_secret, hashsize,
                                                &master_secret_length))) {
        TEST_info("Master secret generation failed");
        goto err;
    }

    if (!TEST_mem_eq(out_master_secret, master_secret_length,
                     master_secret, sizeof(master_secret))) {
        TEST_info("Master secret does not match");
        goto err;
    }

    if (!TEST_size_t_eq(sizeof(client_ats), hashsize))
        goto err;
    if (!TEST_size_t_eq(sizeof(client_ats_key), KEYLEN))
        goto err;
    if (!TEST_size_t_eq(sizeof(client_ats_iv), IVLEN))
        goto err;

    if (!TEST_true(test_secret(s, out_master_secret,
                               (unsigned char *)client_ats_label,
                               strlen(client_ats_label), client_ats,
                               client_ats_key, client_ats_iv))) {
        TEST_info("Client application data secret test failed");
        goto err;
    }

    if (!TEST_size_t_eq(sizeof(server_ats), hashsize))
        goto err;
    if (!TEST_size_t_eq(sizeof(server_ats_key), KEYLEN))
        goto err;
    if (!TEST_size_t_eq(sizeof(server_ats_iv), IVLEN))
        goto err;

    if (!TEST_true(test_secret(s, out_master_secret,
                               (unsigned char *)server_ats_label,
                               strlen(server_ats_label), server_ats,
                               server_ats_key, server_ats_iv))) {
        TEST_info("Server application data secret test failed");
        goto err;
    }

    ret = 1;
 err:
    SSL_free(ssl);
    SSL_CTX_free(ctx);
    return ret;
}

int setup_tests(void)
{
    ADD_TEST(test_handshake_secrets);
    return 1;
}
