/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright 2005 Nokia. All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "ssl_local.h"
#include <openssl/evp.h>
#include <openssl/md5.h>
#include <openssl/core_names.h>
#include "internal/cryptlib.h"
#include "internal/ssl_unwrap.h"

static int ssl3_generate_key_block(SSL_CONNECTION *s, unsigned char *km, int num)
{
    const EVP_MD *md5 = NULL, *sha1 = NULL;
    EVP_MD_CTX *m5;
    EVP_MD_CTX *s1;
    unsigned char buf[16], smd[SHA_DIGEST_LENGTH];
    unsigned char c = 'A';
    unsigned int i, k;
    int ret = 0;
    SSL_CTX *sctx = SSL_CONNECTION_GET_CTX(s);

#ifdef CHARSET_EBCDIC
    c = os_toascii[c];          /* 'A' in ASCII */
#endif
    k = 0;
    md5 = ssl_evp_md_fetch(sctx->libctx, NID_md5, sctx->propq);
    sha1 = ssl_evp_md_fetch(sctx->libctx, NID_sha1, sctx->propq);
    m5 = EVP_MD_CTX_new();
    s1 = EVP_MD_CTX_new();
    if (md5 == NULL || sha1 == NULL || m5 == NULL || s1 == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_EVP_LIB);
        goto err;
    }
    for (i = 0; (int)i < num; i += MD5_DIGEST_LENGTH) {
        k++;
        if (k > sizeof(buf)) {
            /* bug: 'buf' is too small for this ciphersuite */
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            goto err;
        }

        memset(buf, c, k);
        c++;
        if (!EVP_DigestInit_ex(s1, sha1, NULL)
            || !EVP_DigestUpdate(s1, buf, k)
            || !EVP_DigestUpdate(s1, s->session->master_key,
                                 s->session->master_key_length)
            || !EVP_DigestUpdate(s1, s->s3.server_random, SSL3_RANDOM_SIZE)
            || !EVP_DigestUpdate(s1, s->s3.client_random, SSL3_RANDOM_SIZE)
            || !EVP_DigestFinal_ex(s1, smd, NULL)
            || !EVP_DigestInit_ex(m5, md5, NULL)
            || !EVP_DigestUpdate(m5, s->session->master_key,
                                 s->session->master_key_length)
            || !EVP_DigestUpdate(m5, smd, SHA_DIGEST_LENGTH)) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            goto err;
        }
        if ((int)(i + MD5_DIGEST_LENGTH) > num) {
            if (!EVP_DigestFinal_ex(m5, smd, NULL)) {
                SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
                goto err;
            }
            memcpy(km, smd, (num - i));
        } else {
            if (!EVP_DigestFinal_ex(m5, km, NULL)) {
                SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
                goto err;
            }
        }

        km += MD5_DIGEST_LENGTH;
    }
    OPENSSL_cleanse(smd, sizeof(smd));
    ret = 1;
 err:
    EVP_MD_CTX_free(m5);
    EVP_MD_CTX_free(s1);
    ssl_evp_md_free(md5);
    ssl_evp_md_free(sha1);
    return ret;
}

int ssl3_change_cipher_state(SSL_CONNECTION *s, int which)
{
    unsigned char *p, *mac_secret;
    size_t md_len;
    unsigned char *key, *iv;
    const EVP_CIPHER *ciph;
    const SSL_COMP *comp = NULL;
    const EVP_MD *md;
    int mdi;
    size_t n, iv_len, key_len;
    int direction = (which & SSL3_CC_READ) != 0 ? OSSL_RECORD_DIRECTION_READ
                                                : OSSL_RECORD_DIRECTION_WRITE;

    ciph = s->s3.tmp.new_sym_enc;
    md = s->s3.tmp.new_hash;
    /* m == NULL will lead to a crash later */
    if (!ossl_assert(md != NULL)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }
#ifndef OPENSSL_NO_COMP
    comp = s->s3.tmp.new_compression;
#endif

    p = s->s3.tmp.key_block;
    mdi = EVP_MD_get_size(md);
    if (mdi <= 0) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    md_len = (size_t)mdi;
    key_len = EVP_CIPHER_get_key_length(ciph);
    iv_len = EVP_CIPHER_get_iv_length(ciph);

    if ((which == SSL3_CHANGE_CIPHER_CLIENT_WRITE) ||
        (which == SSL3_CHANGE_CIPHER_SERVER_READ)) {
        mac_secret = &(p[0]);
        n = md_len + md_len;
        key = &(p[n]);
        n += key_len + key_len;
        iv = &(p[n]);
        n += iv_len + iv_len;
    } else {
        n = md_len;
        mac_secret = &(p[n]);
        n += md_len + key_len;
        key = &(p[n]);
        n += key_len + iv_len;
        iv = &(p[n]);
        n += iv_len;
    }

    if (n > s->s3.tmp.key_block_length) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }

    if (!ssl_set_new_record_layer(s, SSL3_VERSION,
                                  direction,
                                  OSSL_RECORD_PROTECTION_LEVEL_APPLICATION,
                                  NULL, 0, key, key_len, iv, iv_len, mac_secret,
                                  md_len, ciph, 0, NID_undef, md, comp, NULL)) {
        /* SSLfatal already called */
        goto err;
    }

    return 1;
 err:
    return 0;
}

int ssl3_setup_key_block(SSL_CONNECTION *s)
{
    unsigned char *p;
    const EVP_CIPHER *c;
    const EVP_MD *hash;
    int num;
    int ret = 0;
    SSL_COMP *comp;

    if (s->s3.tmp.key_block_length != 0)
        return 1;

    if (!ssl_cipher_get_evp(SSL_CONNECTION_GET_CTX(s), s->session, &c, &hash,
                            NULL, NULL, &comp, 0)) {
        /* Error is already recorded */
        SSLfatal_alert(s, SSL_AD_INTERNAL_ERROR);
        return 0;
    }

    ssl_evp_cipher_free(s->s3.tmp.new_sym_enc);
    s->s3.tmp.new_sym_enc = c;
    ssl_evp_md_free(s->s3.tmp.new_hash);
    s->s3.tmp.new_hash = hash;
#ifdef OPENSSL_NO_COMP
    s->s3.tmp.new_compression = NULL;
#else
    s->s3.tmp.new_compression = comp;
#endif

    num = EVP_MD_get_size(hash);
    if (num <= 0)
        return 0;

    num = EVP_CIPHER_get_key_length(c) + num + EVP_CIPHER_get_iv_length(c);
    num *= 2;

    ssl3_cleanup_key_block(s);

    if ((p = OPENSSL_malloc(num)) == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_CRYPTO_LIB);
        return 0;
    }

    s->s3.tmp.key_block_length = num;
    s->s3.tmp.key_block = p;

    /* Calls SSLfatal() as required */
    ret = ssl3_generate_key_block(s, p, num);

    return ret;
}

void ssl3_cleanup_key_block(SSL_CONNECTION *s)
{
    OPENSSL_clear_free(s->s3.tmp.key_block, s->s3.tmp.key_block_length);
    s->s3.tmp.key_block = NULL;
    s->s3.tmp.key_block_length = 0;
}

int ssl3_init_finished_mac(SSL_CONNECTION *s)
{
    BIO *buf = BIO_new(BIO_s_mem());

    if (buf == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_BIO_LIB);
        return 0;
    }
    ssl3_free_digest_list(s);
    s->s3.handshake_buffer = buf;
    (void)BIO_set_close(s->s3.handshake_buffer, BIO_CLOSE);
    return 1;
}

/*
 * Free digest list. Also frees handshake buffer since they are always freed
 * together.
 */

void ssl3_free_digest_list(SSL_CONNECTION *s)
{
    BIO_free(s->s3.handshake_buffer);
    s->s3.handshake_buffer = NULL;
    EVP_MD_CTX_free(s->s3.handshake_dgst);
    s->s3.handshake_dgst = NULL;
}

int ssl3_finish_mac(SSL_CONNECTION *s, const unsigned char *buf, size_t len)
{
    int ret;

    if (s->s3.handshake_dgst == NULL) {
        /* Note: this writes to a memory BIO so a failure is a fatal error */
        if (len > INT_MAX) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, SSL_R_OVERFLOW_ERROR);
            return 0;
        }
        ret = BIO_write(s->s3.handshake_buffer, (void *)buf, (int)len);
        if (ret <= 0 || ret != (int)len) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            return 0;
        }
    } else {
        ret = EVP_DigestUpdate(s->s3.handshake_dgst, buf, len);
        if (!ret) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            return 0;
        }
    }
    return 1;
}

int ssl3_digest_cached_records(SSL_CONNECTION *s, int keep)
{
    const EVP_MD *md;
    long hdatalen;
    void *hdata;

    if (s->s3.handshake_dgst == NULL) {
        hdatalen = BIO_get_mem_data(s->s3.handshake_buffer, &hdata);
        if (hdatalen <= 0) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, SSL_R_BAD_HANDSHAKE_LENGTH);
            return 0;
        }

        s->s3.handshake_dgst = EVP_MD_CTX_new();
        if (s->s3.handshake_dgst == NULL) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_EVP_LIB);
            return 0;
        }

        md = ssl_handshake_md(s);
        if (md == NULL) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR,
                     SSL_R_NO_SUITABLE_DIGEST_ALGORITHM);
            return 0;
        }
        if (!EVP_DigestInit_ex(s->s3.handshake_dgst, md, NULL)
            || !EVP_DigestUpdate(s->s3.handshake_dgst, hdata, hdatalen)) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            return 0;
        }
    }
    if (keep == 0) {
        BIO_free(s->s3.handshake_buffer);
        s->s3.handshake_buffer = NULL;
    }

    return 1;
}

void ssl3_digest_master_key_set_params(const SSL_SESSION *session,
                                       OSSL_PARAM params[])
{
    int n = 0;
    params[n++] = OSSL_PARAM_construct_octet_string(OSSL_DIGEST_PARAM_SSL3_MS,
                                                    (void *)session->master_key,
                                                    session->master_key_length);
    params[n++] = OSSL_PARAM_construct_end();
}

size_t ssl3_final_finish_mac(SSL_CONNECTION *s, const char *sender, size_t len,
                             unsigned char *p)
{
    int ret;
    EVP_MD_CTX *ctx = NULL;

    if (!ssl3_digest_cached_records(s, 0)) {
        /* SSLfatal() already called */
        return 0;
    }

    if (EVP_MD_CTX_get_type(s->s3.handshake_dgst) != NID_md5_sha1) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, SSL_R_NO_REQUIRED_DIGEST);
        return 0;
    }

    ctx = EVP_MD_CTX_new();
    if (ctx == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_EVP_LIB);
        return 0;
    }
    if (!EVP_MD_CTX_copy_ex(ctx, s->s3.handshake_dgst)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        ret = 0;
        goto err;
    }

    ret = EVP_MD_CTX_get_size(ctx);
    if (ret < 0) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        ret = 0;
        goto err;
    }

    if (sender != NULL) {
        OSSL_PARAM digest_cmd_params[3];

        ssl3_digest_master_key_set_params(s->session, digest_cmd_params);

        if (EVP_DigestUpdate(ctx, sender, len) <= 0
            || EVP_MD_CTX_set_params(ctx, digest_cmd_params) <= 0
            || EVP_DigestFinal_ex(ctx, p, NULL) <= 0) {
                SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
                ret = 0;
        }
    }

 err:
    EVP_MD_CTX_free(ctx);

    return ret;
}

int ssl3_generate_master_secret(SSL_CONNECTION *s, unsigned char *out,
                                unsigned char *p,
                                size_t len, size_t *secret_size)
{
    static const unsigned char *const salt[3] = {
#ifndef CHARSET_EBCDIC
        (const unsigned char *)"A",
        (const unsigned char *)"BB",
        (const unsigned char *)"CCC",
#else
        (const unsigned char *)"\x41",
        (const unsigned char *)"\x42\x42",
        (const unsigned char *)"\x43\x43\x43",
#endif
    };
    unsigned char buf[EVP_MAX_MD_SIZE];
    EVP_MD_CTX *ctx = EVP_MD_CTX_new();
    int i, ret = 1;
    unsigned int n;
    size_t ret_secret_size = 0;

    if (ctx == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_EVP_LIB);
        return 0;
    }
    for (i = 0; i < 3; i++) {
        if (EVP_DigestInit_ex(ctx, SSL_CONNECTION_GET_CTX(s)->sha1, NULL) <= 0
            || EVP_DigestUpdate(ctx, salt[i],
                                strlen((const char *)salt[i])) <= 0
            || EVP_DigestUpdate(ctx, p, len) <= 0
            || EVP_DigestUpdate(ctx, &(s->s3.client_random[0]),
                                SSL3_RANDOM_SIZE) <= 0
            || EVP_DigestUpdate(ctx, &(s->s3.server_random[0]),
                                SSL3_RANDOM_SIZE) <= 0
            || EVP_DigestFinal_ex(ctx, buf, &n) <= 0
            || EVP_DigestInit_ex(ctx, SSL_CONNECTION_GET_CTX(s)->md5, NULL) <= 0
            || EVP_DigestUpdate(ctx, p, len) <= 0
            || EVP_DigestUpdate(ctx, buf, n) <= 0
            || EVP_DigestFinal_ex(ctx, out, &n) <= 0) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            ret = 0;
            break;
        }
        out += n;
        ret_secret_size += n;
    }
    EVP_MD_CTX_free(ctx);

    OPENSSL_cleanse(buf, sizeof(buf));
    if (ret)
        *secret_size = ret_secret_size;
    return ret;
}

int ssl3_alert_code(int code)
{
    switch (code) {
    case SSL_AD_CLOSE_NOTIFY:
        return SSL3_AD_CLOSE_NOTIFY;
    case SSL_AD_UNEXPECTED_MESSAGE:
        return SSL3_AD_UNEXPECTED_MESSAGE;
    case SSL_AD_BAD_RECORD_MAC:
        return SSL3_AD_BAD_RECORD_MAC;
    case SSL_AD_DECRYPTION_FAILED:
        return SSL3_AD_BAD_RECORD_MAC;
    case SSL_AD_RECORD_OVERFLOW:
        return SSL3_AD_BAD_RECORD_MAC;
    case SSL_AD_DECOMPRESSION_FAILURE:
        return SSL3_AD_DECOMPRESSION_FAILURE;
    case SSL_AD_HANDSHAKE_FAILURE:
        return SSL3_AD_HANDSHAKE_FAILURE;
    case SSL_AD_NO_CERTIFICATE:
        return SSL3_AD_NO_CERTIFICATE;
    case SSL_AD_BAD_CERTIFICATE:
        return SSL3_AD_BAD_CERTIFICATE;
    case SSL_AD_UNSUPPORTED_CERTIFICATE:
        return SSL3_AD_UNSUPPORTED_CERTIFICATE;
    case SSL_AD_CERTIFICATE_REVOKED:
        return SSL3_AD_CERTIFICATE_REVOKED;
    case SSL_AD_CERTIFICATE_EXPIRED:
        return SSL3_AD_CERTIFICATE_EXPIRED;
    case SSL_AD_CERTIFICATE_UNKNOWN:
        return SSL3_AD_CERTIFICATE_UNKNOWN;
    case SSL_AD_ILLEGAL_PARAMETER:
        return SSL3_AD_ILLEGAL_PARAMETER;
    case SSL_AD_UNKNOWN_CA:
        return SSL3_AD_BAD_CERTIFICATE;
    case SSL_AD_ACCESS_DENIED:
        return SSL3_AD_HANDSHAKE_FAILURE;
    case SSL_AD_DECODE_ERROR:
        return SSL3_AD_HANDSHAKE_FAILURE;
    case SSL_AD_DECRYPT_ERROR:
        return SSL3_AD_HANDSHAKE_FAILURE;
    case SSL_AD_EXPORT_RESTRICTION:
        return SSL3_AD_HANDSHAKE_FAILURE;
    case SSL_AD_PROTOCOL_VERSION:
        return SSL3_AD_HANDSHAKE_FAILURE;
    case SSL_AD_INSUFFICIENT_SECURITY:
        return SSL3_AD_HANDSHAKE_FAILURE;
    case SSL_AD_INTERNAL_ERROR:
        return SSL3_AD_HANDSHAKE_FAILURE;
    case SSL_AD_USER_CANCELLED:
        return SSL3_AD_HANDSHAKE_FAILURE;
    case SSL_AD_NO_RENEGOTIATION:
        return -1;            /* Don't send it :-) */
    case SSL_AD_UNSUPPORTED_EXTENSION:
        return SSL3_AD_HANDSHAKE_FAILURE;
    case SSL_AD_CERTIFICATE_UNOBTAINABLE:
        return SSL3_AD_HANDSHAKE_FAILURE;
    case SSL_AD_UNRECOGNIZED_NAME:
        return SSL3_AD_HANDSHAKE_FAILURE;
    case SSL_AD_BAD_CERTIFICATE_STATUS_RESPONSE:
        return SSL3_AD_HANDSHAKE_FAILURE;
    case SSL_AD_BAD_CERTIFICATE_HASH_VALUE:
        return SSL3_AD_HANDSHAKE_FAILURE;
    case SSL_AD_UNKNOWN_PSK_IDENTITY:
        return TLS1_AD_UNKNOWN_PSK_IDENTITY;
    case SSL_AD_INAPPROPRIATE_FALLBACK:
        return TLS1_AD_INAPPROPRIATE_FALLBACK;
    case SSL_AD_NO_APPLICATION_PROTOCOL:
        return TLS1_AD_NO_APPLICATION_PROTOCOL;
    case SSL_AD_CERTIFICATE_REQUIRED:
        return SSL_AD_HANDSHAKE_FAILURE;
    case TLS13_AD_MISSING_EXTENSION:
        return SSL_AD_HANDSHAKE_FAILURE;
    default:
        return -1;
    }
}
