/*
 * Copyright 2022-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/quic_record_util.h"
#include "internal/quic_record_rx.h"
#include "internal/quic_record_tx.h"
#include "internal/quic_wire_pkt.h"
#include "../ssl_local.h"
#include <openssl/kdf.h>
#include <openssl/core_names.h>

/*
 * QUIC Key Derivation Utilities
 * =============================
 */
int ossl_quic_hkdf_extract(OSSL_LIB_CTX *libctx,
                           const char *propq,
                           const EVP_MD *md,
                           const unsigned char *salt, size_t salt_len,
                           const unsigned char *ikm, size_t ikm_len,
                           unsigned char *out, size_t out_len)
{
    int ret = 0;
    EVP_KDF *kdf = NULL;
    EVP_KDF_CTX *kctx = NULL;
    OSSL_PARAM params[8], *p = params;
    int key_check = 0;
    int mode = EVP_PKEY_HKDEF_MODE_EXTRACT_ONLY;
    const char *md_name;

    if ((md_name = EVP_MD_get0_name(md)) == NULL
        || (kdf = EVP_KDF_fetch(libctx, OSSL_KDF_NAME_HKDF, propq)) == NULL
        || (kctx = EVP_KDF_CTX_new(kdf)) == NULL)
        goto err;

    /*
     * According to RFC 9000, the length of destination connection ID must be
     * at least 8 bytes. It means that the length of destination connection ID
     * may be less than the minimum length for HKDF required by FIPS provider.
     *
     * Therefore, we need to set `key-check` to zero to allow using destionation
     * connection ID as IKM.
     */
    *p++ = OSSL_PARAM_construct_int(OSSL_KDF_PARAM_FIPS_KEY_CHECK, &key_check);
    *p++ = OSSL_PARAM_construct_int(OSSL_KDF_PARAM_MODE, &mode);
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_DIGEST,
                                            (char *)md_name, 0);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_SALT,
                                             (unsigned char *)salt, salt_len);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_KEY,
                                             (unsigned char *)ikm, ikm_len);
    *p++ = OSSL_PARAM_construct_end();

    ret = EVP_KDF_derive(kctx, out, out_len, params);

err:
    EVP_KDF_CTX_free(kctx);
    EVP_KDF_free(kdf);
    return ret;
}

/* Constants used for key derivation in QUIC v1. */
static const unsigned char quic_client_in_label[] = {
    0x63, 0x6c, 0x69, 0x65, 0x6e, 0x74, 0x20, 0x69, 0x6e /* "client in" */
};
static const unsigned char quic_server_in_label[] = {
    0x73, 0x65, 0x72, 0x76, 0x65, 0x72, 0x20, 0x69, 0x6e /* "server in" */
};

/* Salt used to derive Initial packet protection keys (RFC 9001 Section 5.2). */
static const unsigned char quic_v1_initial_salt[] = {
    0x38, 0x76, 0x2c, 0xf7, 0xf5, 0x59, 0x34, 0xb3, 0x4d, 0x17,
    0x9a, 0xe6, 0xa4, 0xc8, 0x0c, 0xad, 0xcc, 0xbb, 0x7f, 0x0a
};

int ossl_quic_provide_initial_secret(OSSL_LIB_CTX *libctx,
                                     const char *propq,
                                     const QUIC_CONN_ID *dst_conn_id,
                                     int is_server,
                                     struct ossl_qrx_st *qrx,
                                     struct ossl_qtx_st *qtx)
{
    unsigned char initial_secret[32];
    unsigned char client_initial_secret[32], server_initial_secret[32];
    unsigned char *rx_secret, *tx_secret;
    EVP_MD *sha256;

    if (qrx == NULL && qtx == NULL)
        return 1;

    /* Initial encryption always uses SHA-256. */
    if ((sha256 = EVP_MD_fetch(libctx, "SHA256", propq)) == NULL)
        return 0;

    if (is_server) {
        rx_secret = client_initial_secret;
        tx_secret = server_initial_secret;
    } else {
        rx_secret = server_initial_secret;
        tx_secret = client_initial_secret;
    }

    /* Derive initial secret from destination connection ID. */
    if (!ossl_quic_hkdf_extract(libctx, propq,
                                sha256,
                                quic_v1_initial_salt,
                                sizeof(quic_v1_initial_salt),
                                dst_conn_id->id,
                                dst_conn_id->id_len,
                                initial_secret,
                                sizeof(initial_secret)))
        goto err;

    /* Derive "client in" secret. */
    if (((qtx != NULL && tx_secret == client_initial_secret)
         || (qrx != NULL && rx_secret == client_initial_secret))
        && !tls13_hkdf_expand_ex(libctx, propq,
                                 sha256,
                                 initial_secret,
                                 quic_client_in_label,
                                 sizeof(quic_client_in_label),
                                 NULL, 0,
                                 client_initial_secret,
                                 sizeof(client_initial_secret), 1))
        goto err;

    /* Derive "server in" secret. */
    if (((qtx != NULL && tx_secret == server_initial_secret)
         || (qrx != NULL && rx_secret == server_initial_secret))
        && !tls13_hkdf_expand_ex(libctx, propq,
                                 sha256,
                                 initial_secret,
                                 quic_server_in_label,
                                 sizeof(quic_server_in_label),
                                 NULL, 0,
                                 server_initial_secret,
                                 sizeof(server_initial_secret), 1))
        goto err;

    /* Setup RX EL. Initial encryption always uses AES-128-GCM. */
    if (qrx != NULL
        && !ossl_qrx_provide_secret(qrx, QUIC_ENC_LEVEL_INITIAL,
                                    QRL_SUITE_AES128GCM,
                                    sha256,
                                    rx_secret,
                                    sizeof(server_initial_secret)))
        goto err;

    /*
     * ossl_qrx_provide_secret takes ownership of our ref to SHA256, so if we
     * are initialising both sides, get a new ref for the following call for the
     * TX side.
     */
    if (qrx != NULL && qtx != NULL && !EVP_MD_up_ref(sha256)) {
        sha256 = NULL;
        goto err;
    }

    /* Setup TX cipher. */
    if (qtx != NULL
        && !ossl_qtx_provide_secret(qtx, QUIC_ENC_LEVEL_INITIAL,
                                    QRL_SUITE_AES128GCM,
                                    sha256,
                                    tx_secret,
                                    sizeof(server_initial_secret)))
        goto err;

    return 1;

err:
    EVP_MD_free(sha256);
    return 0;
}

/*
 * QUIC Record Layer Ciphersuite Info
 * ==================================
 */

struct suite_info {
    const char *cipher_name, *md_name;
    uint32_t secret_len, cipher_key_len, cipher_iv_len, cipher_tag_len;
    uint32_t hdr_prot_key_len, hdr_prot_cipher_id;
    uint64_t max_pkt, max_forged_pkt;
};

static const struct suite_info suite_aes128gcm = {
    "AES-128-GCM", "SHA256", 32, 16, 12, 16, 16,
    QUIC_HDR_PROT_CIPHER_AES_128,
    ((uint64_t)1) << 23, /* Limits as prescribed by RFC 9001 */
    ((uint64_t)1) << 52,
};

static const struct suite_info suite_aes256gcm = {
    "AES-256-GCM", "SHA384", 48, 32, 12, 16, 32,
    QUIC_HDR_PROT_CIPHER_AES_256,
    ((uint64_t)1) << 23, /* Limits as prescribed by RFC 9001 */
    ((uint64_t)1) << 52,
};

static const struct suite_info suite_chacha20poly1305 = {
    "ChaCha20-Poly1305", "SHA256", 32, 32, 12, 16, 32,
    QUIC_HDR_PROT_CIPHER_CHACHA,
    /* Do not use UINT64_MAX here as this represents an invalid value */
    UINT64_MAX - 1,         /* No applicable limit for this suite (RFC 9001) */
    ((uint64_t)1) << 36,    /* Limit as prescribed by RFC 9001 */
};

static const struct suite_info *get_suite(uint32_t suite_id)
{
    switch (suite_id) {
        case QRL_SUITE_AES128GCM:
            return &suite_aes128gcm;
        case QRL_SUITE_AES256GCM:
            return &suite_aes256gcm;
        case QRL_SUITE_CHACHA20POLY1305:
            return &suite_chacha20poly1305;
        default:
            return NULL;
    }
}

const char *ossl_qrl_get_suite_cipher_name(uint32_t suite_id)
{
    const struct suite_info *c = get_suite(suite_id);
    return c != NULL ? c->cipher_name : NULL;
}

const char *ossl_qrl_get_suite_md_name(uint32_t suite_id)
{
    const struct suite_info *c = get_suite(suite_id);
    return c != NULL ? c->md_name : NULL;
}

uint32_t ossl_qrl_get_suite_secret_len(uint32_t suite_id)
{
    const struct suite_info *c = get_suite(suite_id);
    return c != NULL ? c->secret_len : 0;
}

uint32_t ossl_qrl_get_suite_cipher_key_len(uint32_t suite_id)
{
    const struct suite_info *c = get_suite(suite_id);
    return c != NULL ? c->cipher_key_len : 0;
}

uint32_t ossl_qrl_get_suite_cipher_iv_len(uint32_t suite_id)
{
    const struct suite_info *c = get_suite(suite_id);
    return c != NULL ? c->cipher_iv_len : 0;
}

uint32_t ossl_qrl_get_suite_cipher_tag_len(uint32_t suite_id)
{
    const struct suite_info *c = get_suite(suite_id);
    return c != NULL ? c->cipher_tag_len : 0;
}

uint32_t ossl_qrl_get_suite_hdr_prot_cipher_id(uint32_t suite_id)
{
    const struct suite_info *c = get_suite(suite_id);
    return c != NULL ? c->hdr_prot_cipher_id : 0;
}

uint32_t ossl_qrl_get_suite_hdr_prot_key_len(uint32_t suite_id)
{
    const struct suite_info *c = get_suite(suite_id);
    return c != NULL ? c->hdr_prot_key_len : 0;
}

uint64_t ossl_qrl_get_suite_max_pkt(uint32_t suite_id)
{
    const struct suite_info *c = get_suite(suite_id);
    return c != NULL ? c->max_pkt : UINT64_MAX;
}

uint64_t ossl_qrl_get_suite_max_forged_pkt(uint32_t suite_id)
{
    const struct suite_info *c = get_suite(suite_id);
    return c != NULL ? c->max_forged_pkt : UINT64_MAX;
}
