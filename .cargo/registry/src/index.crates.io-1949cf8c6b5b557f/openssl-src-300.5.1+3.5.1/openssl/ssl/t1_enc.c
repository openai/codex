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
#include "record/record_local.h"
#include "internal/ktls.h"
#include "internal/cryptlib.h"
#include "internal/ssl_unwrap.h"
#include <openssl/comp.h>
#include <openssl/evp.h>
#include <openssl/kdf.h>
#include <openssl/rand.h>
#include <openssl/obj_mac.h>
#include <openssl/core_names.h>
#include <openssl/trace.h>

/* seed1 through seed5 are concatenated */
static int tls1_PRF(SSL_CONNECTION *s,
                    const void *seed1, size_t seed1_len,
                    const void *seed2, size_t seed2_len,
                    const void *seed3, size_t seed3_len,
                    const void *seed4, size_t seed4_len,
                    const void *seed5, size_t seed5_len,
                    const unsigned char *sec, size_t slen,
                    unsigned char *out, size_t olen, int fatal)
{
    const EVP_MD *md = ssl_prf_md(s);
    EVP_KDF *kdf;
    EVP_KDF_CTX *kctx = NULL;
    OSSL_PARAM params[8], *p = params;
    const char *mdname;

    if (md == NULL) {
        /* Should never happen */
        if (fatal)
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        else
            ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        return 0;
    }
    kdf = EVP_KDF_fetch(SSL_CONNECTION_GET_CTX(s)->libctx,
                        OSSL_KDF_NAME_TLS1_PRF,
                        SSL_CONNECTION_GET_CTX(s)->propq);
    if (kdf == NULL)
        goto err;
    kctx = EVP_KDF_CTX_new(kdf);
    EVP_KDF_free(kdf);
    if (kctx == NULL)
        goto err;
    mdname = EVP_MD_get0_name(md);
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_DIGEST,
                                            (char *)mdname, 0);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_SECRET,
                                             (unsigned char *)sec,
                                             (size_t)slen);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_SEED,
                                             (void *)seed1, (size_t)seed1_len);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_SEED,
                                             (void *)seed2, (size_t)seed2_len);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_SEED,
                                             (void *)seed3, (size_t)seed3_len);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_SEED,
                                             (void *)seed4, (size_t)seed4_len);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_SEED,
                                             (void *)seed5, (size_t)seed5_len);
    *p = OSSL_PARAM_construct_end();
    if (EVP_KDF_derive(kctx, out, olen, params)) {
        EVP_KDF_CTX_free(kctx);
        return 1;
    }

 err:
    if (fatal)
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
    else
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
    EVP_KDF_CTX_free(kctx);
    return 0;
}

static int tls1_generate_key_block(SSL_CONNECTION *s, unsigned char *km,
                                   size_t num)
{
    int ret;

    /* Calls SSLfatal() as required */
    ret = tls1_PRF(s,
                   TLS_MD_KEY_EXPANSION_CONST,
                   TLS_MD_KEY_EXPANSION_CONST_SIZE, s->s3.server_random,
                   SSL3_RANDOM_SIZE, s->s3.client_random, SSL3_RANDOM_SIZE,
                   NULL, 0, NULL, 0, s->session->master_key,
                   s->session->master_key_length, km, num, 1);

    return ret;
}

static int tls_iv_length_within_key_block(const EVP_CIPHER *c)
{
    /* If GCM/CCM mode only part of IV comes from PRF */
    if (EVP_CIPHER_get_mode(c) == EVP_CIPH_GCM_MODE)
        return EVP_GCM_TLS_FIXED_IV_LEN;
    else if (EVP_CIPHER_get_mode(c) == EVP_CIPH_CCM_MODE)
        return EVP_CCM_TLS_FIXED_IV_LEN;
    else
        return EVP_CIPHER_get_iv_length(c);
}

int tls1_change_cipher_state(SSL_CONNECTION *s, int which)
{
    unsigned char *p, *mac_secret;
    unsigned char *key, *iv;
    const EVP_CIPHER *c;
    const SSL_COMP *comp = NULL;
    const EVP_MD *m;
    int mac_type;
    size_t mac_secret_size;
    size_t n, i, j, k, cl;
    int iivlen;
    /*
     * Taglen is only relevant for CCM ciphersuites. Other ciphersuites
     * ignore this value so we can default it to 0.
     */
    size_t taglen = 0;
    int direction;

    c = s->s3.tmp.new_sym_enc;
    m = s->s3.tmp.new_hash;
    mac_type = s->s3.tmp.new_mac_pkey_type;
#ifndef OPENSSL_NO_COMP
    comp = s->s3.tmp.new_compression;
#endif

    p = s->s3.tmp.key_block;
    i = mac_secret_size = s->s3.tmp.new_mac_secret_size;

    cl = EVP_CIPHER_get_key_length(c);
    j = cl;
    iivlen = tls_iv_length_within_key_block(c);
    if (iivlen < 0) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    k = iivlen;
    if ((which == SSL3_CHANGE_CIPHER_CLIENT_WRITE) ||
        (which == SSL3_CHANGE_CIPHER_SERVER_READ)) {
        mac_secret = &(p[0]);
        n = i + i;
        key = &(p[n]);
        n += j + j;
        iv = &(p[n]);
        n += k + k;
    } else {
        n = i;
        mac_secret = &(p[n]);
        n += i + j;
        key = &(p[n]);
        n += j + k;
        iv = &(p[n]);
        n += k;
    }

    if (n > s->s3.tmp.key_block_length) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }

    switch (EVP_CIPHER_get_mode(c)) {
    case EVP_CIPH_GCM_MODE:
        taglen = EVP_GCM_TLS_TAG_LEN;
        break;
    case EVP_CIPH_CCM_MODE:
        if ((s->s3.tmp.new_cipher->algorithm_enc
                & (SSL_AES128CCM8 | SSL_AES256CCM8)) != 0)
            taglen = EVP_CCM8_TLS_TAG_LEN;
        else
            taglen = EVP_CCM_TLS_TAG_LEN;
        break;
    default:
        if (EVP_CIPHER_is_a(c, "CHACHA20-POLY1305")) {
            taglen = EVP_CHACHAPOLY_TLS_TAG_LEN;
        } else {
            /* MAC secret size corresponds to the MAC output size */
            taglen = s->s3.tmp.new_mac_secret_size;
        }
        break;
    }

    if (which & SSL3_CC_READ) {
        if (s->ext.use_etm)
            s->s3.flags |= TLS1_FLAGS_ENCRYPT_THEN_MAC_READ;
        else
            s->s3.flags &= ~TLS1_FLAGS_ENCRYPT_THEN_MAC_READ;

        if (s->s3.tmp.new_cipher->algorithm2 & TLS1_STREAM_MAC)
            s->mac_flags |= SSL_MAC_FLAG_READ_MAC_STREAM;
        else
            s->mac_flags &= ~SSL_MAC_FLAG_READ_MAC_STREAM;

        if (s->s3.tmp.new_cipher->algorithm2 & TLS1_TLSTREE)
            s->mac_flags |= SSL_MAC_FLAG_READ_MAC_TLSTREE;
        else
            s->mac_flags &= ~SSL_MAC_FLAG_READ_MAC_TLSTREE;

        direction = OSSL_RECORD_DIRECTION_READ;
    } else {
        if (s->ext.use_etm)
            s->s3.flags |= TLS1_FLAGS_ENCRYPT_THEN_MAC_WRITE;
        else
            s->s3.flags &= ~TLS1_FLAGS_ENCRYPT_THEN_MAC_WRITE;

        if (s->s3.tmp.new_cipher->algorithm2 & TLS1_STREAM_MAC)
            s->mac_flags |= SSL_MAC_FLAG_WRITE_MAC_STREAM;
        else
            s->mac_flags &= ~SSL_MAC_FLAG_WRITE_MAC_STREAM;

        if (s->s3.tmp.new_cipher->algorithm2 & TLS1_TLSTREE)
            s->mac_flags |= SSL_MAC_FLAG_WRITE_MAC_TLSTREE;
        else
            s->mac_flags &= ~SSL_MAC_FLAG_WRITE_MAC_TLSTREE;

        direction = OSSL_RECORD_DIRECTION_WRITE;
    }

    if (SSL_CONNECTION_IS_DTLS(s))
        dtls1_increment_epoch(s, which);

    if (!ssl_set_new_record_layer(s, s->version, direction,
                                    OSSL_RECORD_PROTECTION_LEVEL_APPLICATION,
                                    NULL, 0, key, cl, iv, (size_t)k, mac_secret,
                                    mac_secret_size, c, taglen, mac_type,
                                    m, comp, NULL)) {
        /* SSLfatal already called */
        goto err;
    }

    OSSL_TRACE_BEGIN(TLS) {
        BIO_printf(trc_out, "which = %04X, key:\n", which);
        BIO_dump_indent(trc_out, key, EVP_CIPHER_get_key_length(c), 4);
        BIO_printf(trc_out, "iv:\n");
        BIO_dump_indent(trc_out, iv, k, 4);
    } OSSL_TRACE_END(TLS);

    return 1;
 err:
    return 0;
}

int tls1_setup_key_block(SSL_CONNECTION *s)
{
    unsigned char *p;
    const EVP_CIPHER *c;
    const EVP_MD *hash;
    SSL_COMP *comp;
    int mac_type = NID_undef;
    size_t num, mac_secret_size = 0;
    int ret = 0;
    int ivlen;

    if (s->s3.tmp.key_block_length != 0)
        return 1;

    if (!ssl_cipher_get_evp(SSL_CONNECTION_GET_CTX(s), s->session, &c, &hash,
                            &mac_type, &mac_secret_size, &comp,
                            s->ext.use_etm)) {
        /* Error is already recorded */
        SSLfatal_alert(s, SSL_AD_INTERNAL_ERROR);
        return 0;
    }

    ssl_evp_cipher_free(s->s3.tmp.new_sym_enc);
    s->s3.tmp.new_sym_enc = c;
    ssl_evp_md_free(s->s3.tmp.new_hash);
    s->s3.tmp.new_hash = hash;
    s->s3.tmp.new_mac_pkey_type = mac_type;
    s->s3.tmp.new_mac_secret_size = mac_secret_size;
    ivlen = tls_iv_length_within_key_block(c);
    if (ivlen < 0) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return 0;
    }
    num = mac_secret_size + EVP_CIPHER_get_key_length(c) + ivlen;
    num *= 2;

    ssl3_cleanup_key_block(s);

    if ((p = OPENSSL_malloc(num)) == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_CRYPTO_LIB);
        goto err;
    }

    s->s3.tmp.key_block_length = num;
    s->s3.tmp.key_block = p;

    OSSL_TRACE_BEGIN(TLS) {
        BIO_printf(trc_out, "key block length: %zu\n", num);
        BIO_printf(trc_out, "client random\n");
        BIO_dump_indent(trc_out, s->s3.client_random, SSL3_RANDOM_SIZE, 4);
        BIO_printf(trc_out, "server random\n");
        BIO_dump_indent(trc_out, s->s3.server_random, SSL3_RANDOM_SIZE, 4);
        BIO_printf(trc_out, "master key\n");
        BIO_dump_indent(trc_out,
                        s->session->master_key,
                        s->session->master_key_length, 4);
    } OSSL_TRACE_END(TLS);

    if (!tls1_generate_key_block(s, p, num)) {
        /* SSLfatal() already called */
        goto err;
    }

    OSSL_TRACE_BEGIN(TLS) {
        BIO_printf(trc_out, "key block\n");
        BIO_dump_indent(trc_out, p, num, 4);
    } OSSL_TRACE_END(TLS);

    ret = 1;
 err:
    return ret;
}

size_t tls1_final_finish_mac(SSL_CONNECTION *s, const char *str,
                             size_t slen, unsigned char *out)
{
    size_t hashlen;
    unsigned char hash[EVP_MAX_MD_SIZE];
    size_t finished_size = TLS1_FINISH_MAC_LENGTH;

    if (s->s3.tmp.new_cipher->algorithm_mkey & SSL_kGOST18)
        finished_size = 32;

    if (!ssl3_digest_cached_records(s, 0)) {
        /* SSLfatal() already called */
        return 0;
    }

    if (!ssl_handshake_hash(s, hash, sizeof(hash), &hashlen)) {
        /* SSLfatal() already called */
        return 0;
    }

    if (!tls1_PRF(s, str, slen, hash, hashlen, NULL, 0, NULL, 0, NULL, 0,
                  s->session->master_key, s->session->master_key_length,
                  out, finished_size, 1)) {
        /* SSLfatal() already called */
        return 0;
    }
    OPENSSL_cleanse(hash, hashlen);
    return finished_size;
}

int tls1_generate_master_secret(SSL_CONNECTION *s, unsigned char *out,
                                unsigned char *p, size_t len,
                                size_t *secret_size)
{
    if (s->session->flags & SSL_SESS_FLAG_EXTMS) {
        unsigned char hash[EVP_MAX_MD_SIZE * 2];
        size_t hashlen;
        /*
         * Digest cached records keeping record buffer (if present): this won't
         * affect client auth because we're freezing the buffer at the same
         * point (after client key exchange and before certificate verify)
         */
        if (!ssl3_digest_cached_records(s, 1)
                || !ssl_handshake_hash(s, hash, sizeof(hash), &hashlen)) {
            /* SSLfatal() already called */
            return 0;
        }
        OSSL_TRACE_BEGIN(TLS) {
            BIO_printf(trc_out, "Handshake hashes:\n");
            BIO_dump(trc_out, (char *)hash, hashlen);
        } OSSL_TRACE_END(TLS);
        if (!tls1_PRF(s,
                      TLS_MD_EXTENDED_MASTER_SECRET_CONST,
                      TLS_MD_EXTENDED_MASTER_SECRET_CONST_SIZE,
                      hash, hashlen,
                      NULL, 0,
                      NULL, 0,
                      NULL, 0, p, len, out,
                      SSL3_MASTER_SECRET_SIZE, 1)) {
            /* SSLfatal() already called */
            return 0;
        }
        OPENSSL_cleanse(hash, hashlen);
    } else {
        if (!tls1_PRF(s,
                      TLS_MD_MASTER_SECRET_CONST,
                      TLS_MD_MASTER_SECRET_CONST_SIZE,
                      s->s3.client_random, SSL3_RANDOM_SIZE,
                      NULL, 0,
                      s->s3.server_random, SSL3_RANDOM_SIZE,
                      NULL, 0, p, len, out,
                      SSL3_MASTER_SECRET_SIZE, 1)) {
           /* SSLfatal() already called */
            return 0;
        }
    }

    OSSL_TRACE_BEGIN(TLS) {
        BIO_printf(trc_out, "Premaster Secret:\n");
        BIO_dump_indent(trc_out, p, len, 4);
        BIO_printf(trc_out, "Client Random:\n");
        BIO_dump_indent(trc_out, s->s3.client_random, SSL3_RANDOM_SIZE, 4);
        BIO_printf(trc_out, "Server Random:\n");
        BIO_dump_indent(trc_out, s->s3.server_random, SSL3_RANDOM_SIZE, 4);
        BIO_printf(trc_out, "Master Secret:\n");
        BIO_dump_indent(trc_out,
                        s->session->master_key,
                        SSL3_MASTER_SECRET_SIZE, 4);
    } OSSL_TRACE_END(TLS);

    *secret_size = SSL3_MASTER_SECRET_SIZE;
    return 1;
}

int tls1_export_keying_material(SSL_CONNECTION *s, unsigned char *out,
                                size_t olen, const char *label, size_t llen,
                                const unsigned char *context,
                                size_t contextlen, int use_context)
{
    unsigned char *val = NULL;
    size_t vallen = 0, currentvalpos;
    int rv = 0;

    /*
     * RFC 5705 embeds context length as uint16; reject longer context
     * before proceeding.
     */
    if (contextlen > 0xffff) {
        ERR_raise(ERR_LIB_SSL, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }

    /*
     * construct PRF arguments we construct the PRF argument ourself rather
     * than passing separate values into the TLS PRF to ensure that the
     * concatenation of values does not create a prohibited label.
     */
    vallen = llen + SSL3_RANDOM_SIZE * 2;
    if (use_context) {
        vallen += 2 + contextlen;
    }

    val = OPENSSL_malloc(vallen);
    if (val == NULL)
        goto ret;
    currentvalpos = 0;
    memcpy(val + currentvalpos, (unsigned char *)label, llen);
    currentvalpos += llen;
    memcpy(val + currentvalpos, s->s3.client_random, SSL3_RANDOM_SIZE);
    currentvalpos += SSL3_RANDOM_SIZE;
    memcpy(val + currentvalpos, s->s3.server_random, SSL3_RANDOM_SIZE);
    currentvalpos += SSL3_RANDOM_SIZE;

    if (use_context) {
        val[currentvalpos] = (contextlen >> 8) & 0xff;
        currentvalpos++;
        val[currentvalpos] = contextlen & 0xff;
        currentvalpos++;
        if ((contextlen > 0) || (context != NULL)) {
            memcpy(val + currentvalpos, context, contextlen);
        }
    }

    /*
     * disallow prohibited labels note that SSL3_RANDOM_SIZE > max(prohibited
     * label len) = 15, so size of val > max(prohibited label len) = 15 and
     * the comparisons won't have buffer overflow
     */
    if (memcmp(val, TLS_MD_CLIENT_FINISH_CONST,
               TLS_MD_CLIENT_FINISH_CONST_SIZE) == 0)
        goto err1;
    if (memcmp(val, TLS_MD_SERVER_FINISH_CONST,
               TLS_MD_SERVER_FINISH_CONST_SIZE) == 0)
        goto err1;
    if (memcmp(val, TLS_MD_MASTER_SECRET_CONST,
               TLS_MD_MASTER_SECRET_CONST_SIZE) == 0)
        goto err1;
    if (memcmp(val, TLS_MD_EXTENDED_MASTER_SECRET_CONST,
               TLS_MD_EXTENDED_MASTER_SECRET_CONST_SIZE) == 0)
        goto err1;
    if (memcmp(val, TLS_MD_KEY_EXPANSION_CONST,
               TLS_MD_KEY_EXPANSION_CONST_SIZE) == 0)
        goto err1;

    rv = tls1_PRF(s,
                  val, vallen,
                  NULL, 0,
                  NULL, 0,
                  NULL, 0,
                  NULL, 0,
                  s->session->master_key, s->session->master_key_length,
                  out, olen, 0);

    goto ret;
 err1:
    ERR_raise(ERR_LIB_SSL, SSL_R_TLS_ILLEGAL_EXPORTER_LABEL);
 ret:
    OPENSSL_clear_free(val, vallen);
    return rv;
}

int tls1_alert_code(int code)
{
    switch (code) {
    case SSL_AD_CLOSE_NOTIFY:
        return SSL3_AD_CLOSE_NOTIFY;
    case SSL_AD_UNEXPECTED_MESSAGE:
        return SSL3_AD_UNEXPECTED_MESSAGE;
    case SSL_AD_BAD_RECORD_MAC:
        return SSL3_AD_BAD_RECORD_MAC;
    case SSL_AD_DECRYPTION_FAILED:
        return TLS1_AD_DECRYPTION_FAILED;
    case SSL_AD_RECORD_OVERFLOW:
        return TLS1_AD_RECORD_OVERFLOW;
    case SSL_AD_DECOMPRESSION_FAILURE:
        return SSL3_AD_DECOMPRESSION_FAILURE;
    case SSL_AD_HANDSHAKE_FAILURE:
        return SSL3_AD_HANDSHAKE_FAILURE;
    case SSL_AD_NO_CERTIFICATE:
        return -1;
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
        return TLS1_AD_UNKNOWN_CA;
    case SSL_AD_ACCESS_DENIED:
        return TLS1_AD_ACCESS_DENIED;
    case SSL_AD_DECODE_ERROR:
        return TLS1_AD_DECODE_ERROR;
    case SSL_AD_DECRYPT_ERROR:
        return TLS1_AD_DECRYPT_ERROR;
    case SSL_AD_EXPORT_RESTRICTION:
        return TLS1_AD_EXPORT_RESTRICTION;
    case SSL_AD_PROTOCOL_VERSION:
        return TLS1_AD_PROTOCOL_VERSION;
    case SSL_AD_INSUFFICIENT_SECURITY:
        return TLS1_AD_INSUFFICIENT_SECURITY;
    case SSL_AD_INTERNAL_ERROR:
        return TLS1_AD_INTERNAL_ERROR;
    case SSL_AD_USER_CANCELLED:
        return TLS1_AD_USER_CANCELLED;
    case SSL_AD_NO_RENEGOTIATION:
        return TLS1_AD_NO_RENEGOTIATION;
    case SSL_AD_UNSUPPORTED_EXTENSION:
        return TLS1_AD_UNSUPPORTED_EXTENSION;
    case SSL_AD_CERTIFICATE_UNOBTAINABLE:
        return TLS1_AD_CERTIFICATE_UNOBTAINABLE;
    case SSL_AD_UNRECOGNIZED_NAME:
        return TLS1_AD_UNRECOGNIZED_NAME;
    case SSL_AD_BAD_CERTIFICATE_STATUS_RESPONSE:
        return TLS1_AD_BAD_CERTIFICATE_STATUS_RESPONSE;
    case SSL_AD_BAD_CERTIFICATE_HASH_VALUE:
        return TLS1_AD_BAD_CERTIFICATE_HASH_VALUE;
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
