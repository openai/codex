/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_QUIC_RECORD_UTIL_H
# define OSSL_QUIC_RECORD_UTIL_H

# include <openssl/ssl.h>
# include "internal/quic_types.h"

# ifndef OPENSSL_NO_QUIC

struct ossl_qrx_st;
struct ossl_qtx_st;

/*
 * QUIC Key Derivation Utilities
 * =============================
 */

/* HKDF-Extract(salt, IKM) (RFC 5869) */
int ossl_quic_hkdf_extract(OSSL_LIB_CTX *libctx,
                           const char *propq,
                           const EVP_MD *md,
                           const unsigned char *salt, size_t salt_len,
                           const unsigned char *ikm, size_t ikm_len,
                           unsigned char *out, size_t out_len);

/*
 * A QUIC client sends its first INITIAL packet with a random DCID, which
 * is used to compute the secrets used for INITIAL packet encryption in both
 * directions (both client-to-server and server-to-client).
 *
 * This function performs the necessary DCID-based key derivation, and then
 * provides the derived key material for the INITIAL encryption level to a QRX
 * instance, a QTX instance, or both.
 *
 * This function derives the necessary key material and then:
 *   - if qrx is non-NULL, provides the appropriate secret to it;
 *   - if qtx is non-NULL, provides the appropriate secret to it.
 *
 * If both qrx and qtx are NULL, this is a no-op. This function is equivalent to
 * making the appropriate calls to ossl_qrx_provide_secret() and
 * ossl_qtx_provide_secret().
 *
 * It is possible to use a QRX or QTX without ever calling this, for example if
 * there is no desire to handle INITIAL packets (e.g. if a QRX/QTX is
 * instantiated to succeed a previous QRX/QTX and handle a connection which is
 * already established). However in this case you should make sure you call
 * ossl_qrx_discard_enc_level(); see the header for that function for more
 * details. Calling ossl_qtx_discard_enc_level() is not essential but could
 * protect against programming errors.
 *
 * Returns 1 on success or 0 on error.
 */
int ossl_quic_provide_initial_secret(OSSL_LIB_CTX *libctx,
                                     const char *propq,
                                     const QUIC_CONN_ID *dst_conn_id,
                                     int is_server,
                                     struct ossl_qrx_st *qrx,
                                     struct ossl_qtx_st *qtx);

/*
 * QUIC Record Layer Ciphersuite Info
 * ==================================
 */

/* Available QUIC Record Layer (QRL) ciphersuites. */
# define QRL_SUITE_AES128GCM            1 /* SHA256 */
# define QRL_SUITE_AES256GCM            2 /* SHA384 */
# define QRL_SUITE_CHACHA20POLY1305     3 /* SHA256 */

/* Returns cipher name in bytes or NULL if suite ID is invalid. */
const char *ossl_qrl_get_suite_cipher_name(uint32_t suite_id);

/* Returns hash function name in bytes or NULL if suite ID is invalid. */
const char *ossl_qrl_get_suite_md_name(uint32_t suite_id);

/* Returns secret length in bytes or 0 if suite ID is invalid. */
uint32_t ossl_qrl_get_suite_secret_len(uint32_t suite_id);

/* Returns key length in bytes or 0 if suite ID is invalid. */
uint32_t ossl_qrl_get_suite_cipher_key_len(uint32_t suite_id);

/* Returns IV length in bytes or 0 if suite ID is invalid. */
uint32_t ossl_qrl_get_suite_cipher_iv_len(uint32_t suite_id);

/* Returns AEAD auth tag length in bytes or 0 if suite ID is invalid. */
uint32_t ossl_qrl_get_suite_cipher_tag_len(uint32_t suite_id);

/* Returns a QUIC_HDR_PROT_CIPHER_* value or 0 if suite ID is invalid. */
uint32_t ossl_qrl_get_suite_hdr_prot_cipher_id(uint32_t suite_id);

/* Returns header protection key length in bytes or 0 if suite ID is invalid. */
uint32_t ossl_qrl_get_suite_hdr_prot_key_len(uint32_t suite_id);

/*
 * Returns maximum number of packets which may be safely encrypted with a suite
 * or 0 if suite ID is invalid.
 */
uint64_t ossl_qrl_get_suite_max_pkt(uint32_t suite_id);

/*
 * Returns maximum number of RX'd packets which may safely fail AEAD decryption
 * for a given suite or 0 if suite ID is invalid.
 */
uint64_t ossl_qrl_get_suite_max_forged_pkt(uint32_t suite_id);

# endif

#endif
