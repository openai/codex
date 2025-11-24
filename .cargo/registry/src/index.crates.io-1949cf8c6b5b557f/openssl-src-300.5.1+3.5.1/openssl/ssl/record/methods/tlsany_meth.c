/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/evp.h>
#include "../../ssl_local.h"
#include "../record_local.h"
#include "recmethod_local.h"

#define MIN_SSL2_RECORD_LEN     9

static int tls_any_set_crypto_state(OSSL_RECORD_LAYER *rl, int level,
                                    unsigned char *key, size_t keylen,
                                    unsigned char *iv, size_t ivlen,
                                    unsigned char *mackey, size_t mackeylen,
                                    const EVP_CIPHER *ciph,
                                    size_t taglen,
                                    int mactype,
                                    const EVP_MD *md,
                                    COMP_METHOD *comp)
{
    if (level != OSSL_RECORD_PROTECTION_LEVEL_NONE) {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        return OSSL_RECORD_RETURN_FATAL;
    }

    /* No crypto protection at the "NONE" level so nothing to be done */

    return OSSL_RECORD_RETURN_SUCCESS;
}

static int tls_any_cipher(OSSL_RECORD_LAYER *rl, TLS_RL_RECORD *recs,
                          size_t n_recs, int sending, SSL_MAC_BUF *macs,
                          size_t macsize)
{
    return 1;
}

static int tls_validate_record_header(OSSL_RECORD_LAYER *rl, TLS_RL_RECORD *rec)
{
    if (rec->rec_version == SSL2_VERSION) {
        /* SSLv2 format ClientHello */
        if (!ossl_assert(rl->version == TLS_ANY_VERSION)) {
            RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            return 0;
        }
        if (rec->length < MIN_SSL2_RECORD_LEN) {
            RLAYERfatal(rl, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_TOO_SHORT);
            return 0;
        }
    } else {
        if (rl->version == TLS_ANY_VERSION) {
            if ((rec->rec_version >> 8) != SSL3_VERSION_MAJOR) {
                if (rl->is_first_record) {
                    unsigned char *p;

                    /*
                     * Go back to start of packet, look at the five bytes that
                     * we have.
                     */
                    p = rl->packet;
                    if (HAS_PREFIX((char *)p, "GET ") ||
                        HAS_PREFIX((char *)p, "POST ") ||
                        HAS_PREFIX((char *)p, "HEAD ") ||
                        HAS_PREFIX((char *)p, "PATCH") ||
                        HAS_PREFIX((char *)p, "OPTIO") ||
                        HAS_PREFIX((char *)p, "DELET") ||
                        HAS_PREFIX((char *)p, "TRACE") ||
                        HAS_PREFIX((char *)p, "PUT ")) {
                        RLAYERfatal(rl, SSL_AD_NO_ALERT, SSL_R_HTTP_REQUEST);
                        return 0;
                    } else if (HAS_PREFIX((char *)p, "CONNE")) {
                        RLAYERfatal(rl, SSL_AD_NO_ALERT,
                                    SSL_R_HTTPS_PROXY_REQUEST);
                        return 0;
                    }

                    /* Doesn't look like TLS - don't send an alert */
                    RLAYERfatal(rl, SSL_AD_NO_ALERT,
                                SSL_R_WRONG_VERSION_NUMBER);
                    return 0;
                } else {
                    RLAYERfatal(rl, SSL_AD_PROTOCOL_VERSION,
                                SSL_R_WRONG_VERSION_NUMBER);
                    return 0;
                }
            }
        } else if (rl->version == TLS1_3_VERSION) {
            /*
             * In this case we know we are going to negotiate TLSv1.3, but we've
             * had an HRR, so we haven't actually done so yet. In TLSv1.3 we
             * must ignore the legacy record version in plaintext records.
             */
        } else if (rec->rec_version != rl->version) {
            if ((rl->version & 0xFF00) == (rec->rec_version & 0xFF00)) {
                if (rec->type == SSL3_RT_ALERT) {
                    /*
                     * The record is using an incorrect version number,
                     * but what we've got appears to be an alert. We
                     * haven't read the body yet to check whether its a
                     * fatal or not - but chances are it is. We probably
                     * shouldn't send a fatal alert back. We'll just
                     * end.
                     */
                    RLAYERfatal(rl, SSL_AD_NO_ALERT,
                                SSL_R_WRONG_VERSION_NUMBER);
                    return 0;
                }
                /* Send back error using their minor version number */
                rl->version = (unsigned short)rec->rec_version;
            }
            RLAYERfatal(rl, SSL_AD_PROTOCOL_VERSION,
                        SSL_R_WRONG_VERSION_NUMBER);
            return 0;
        }
    }
    if (rec->length > SSL3_RT_MAX_PLAIN_LENGTH) {
        /*
         * We use SSL_R_DATA_LENGTH_TOO_LONG instead of
         * SSL_R_ENCRYPTED_LENGTH_TOO_LONG here because we are the "any" method
         * and we know that we are dealing with plaintext data
         */
        RLAYERfatal(rl, SSL_AD_RECORD_OVERFLOW, SSL_R_DATA_LENGTH_TOO_LONG);
        return 0;
    }
    return 1;
}

static int tls_any_set_protocol_version(OSSL_RECORD_LAYER *rl, int vers)
{
    if (rl->version != TLS_ANY_VERSION && rl->version != vers)
        return 0;
    rl->version = vers;

    return 1;
}

static int tls_any_prepare_for_encryption(OSSL_RECORD_LAYER *rl,
                                          size_t mac_size,
                                          WPACKET *thispkt,
                                          TLS_RL_RECORD *thiswr)
{
    /* No encryption, so nothing to do */
    return 1;
}

const struct record_functions_st tls_any_funcs = {
    tls_any_set_crypto_state,
    tls_any_cipher,
    NULL,
    tls_any_set_protocol_version,
    tls_default_read_n,
    tls_get_more_records,
    tls_validate_record_header,
    tls_default_post_process_record,
    tls_get_max_records_default,
    tls_write_records_default,
    tls_allocate_write_buffers_default,
    tls_initialise_write_packets_default,
    NULL,
    tls_prepare_record_header_default,
    NULL,
    tls_any_prepare_for_encryption,
    tls_post_encryption_processing_default,
    NULL
};

static int dtls_any_set_protocol_version(OSSL_RECORD_LAYER *rl, int vers)
{
    if (rl->version != DTLS_ANY_VERSION && rl->version != vers)
        return 0;
    rl->version = vers;

    return 1;
}

const struct record_functions_st dtls_any_funcs = {
    tls_any_set_crypto_state,
    tls_any_cipher,
    NULL,
    dtls_any_set_protocol_version,
    tls_default_read_n,
    dtls_get_more_records,
    NULL,
    NULL,
    NULL,
    tls_write_records_default,
    tls_allocate_write_buffers_default,
    tls_initialise_write_packets_default,
    NULL,
    dtls_prepare_record_header,
    NULL,
    tls_prepare_for_encryption_default,
    dtls_post_encryption_processing,
    NULL
};
