/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2002, Oracle and/or its affiliates. All rights reserved
 * Copyright 2005 Nokia. All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <time.h>
#include <assert.h>
#include "../ssl_local.h"
#include "statem_local.h"
#include <openssl/buffer.h>
#include <openssl/rand.h>
#include <openssl/objects.h>
#include <openssl/evp.h>
#include <openssl/md5.h>
#include <openssl/dh.h>
#include <openssl/rsa.h>
#include <openssl/bn.h>
#include <openssl/engine.h>
#include <openssl/trace.h>
#include <openssl/core_names.h>
#include <openssl/param_build.h>
#include "internal/cryptlib.h"
#include "internal/comp.h"
#include "internal/ssl_unwrap.h"

static MSG_PROCESS_RETURN tls_process_as_hello_retry_request(SSL_CONNECTION *s,
                                                             PACKET *pkt);
static MSG_PROCESS_RETURN tls_process_encrypted_extensions(SSL_CONNECTION *s,
                                                           PACKET *pkt);

static ossl_inline int cert_req_allowed(SSL_CONNECTION *s);
static int key_exchange_expected(SSL_CONNECTION *s);
static int ssl_cipher_list_to_bytes(SSL_CONNECTION *s, STACK_OF(SSL_CIPHER) *sk,
                                    WPACKET *pkt);

static ossl_inline int received_server_cert(SSL_CONNECTION *sc)
{
    return sc->session->peer_rpk != NULL || sc->session->peer != NULL;
}

/*
 * Is a CertificateRequest message allowed at the moment or not?
 *
 *  Return values are:
 *  1: Yes
 *  0: No
 */
static ossl_inline int cert_req_allowed(SSL_CONNECTION *s)
{
    /* TLS does not like anon-DH with client cert */
    if ((s->version > SSL3_VERSION
         && (s->s3.tmp.new_cipher->algorithm_auth & SSL_aNULL))
        || (s->s3.tmp.new_cipher->algorithm_auth & (SSL_aSRP | SSL_aPSK)))
        return 0;

    return 1;
}

/*
 * Should we expect the ServerKeyExchange message or not?
 *
 *  Return values are:
 *  1: Yes
 *  0: No
 */
static int key_exchange_expected(SSL_CONNECTION *s)
{
    long alg_k = s->s3.tmp.new_cipher->algorithm_mkey;

    /*
     * Can't skip server key exchange if this is an ephemeral
     * ciphersuite or for SRP
     */
    if (alg_k & (SSL_kDHE | SSL_kECDHE | SSL_kDHEPSK | SSL_kECDHEPSK
                 | SSL_kSRP)) {
        return 1;
    }

    return 0;
}

/*
 * ossl_statem_client_read_transition() encapsulates the logic for the allowed
 * handshake state transitions when a TLS1.3 client is reading messages from the
 * server. The message type that the server has sent is provided in |mt|. The
 * current state is in |s->statem.hand_state|.
 *
 * Return values are 1 for success (transition allowed) and  0 on error
 * (transition not allowed)
 */
static int ossl_statem_client13_read_transition(SSL_CONNECTION *s, int mt)
{
    OSSL_STATEM *st = &s->statem;

    /*
     * Note: There is no case for TLS_ST_CW_CLNT_HELLO, because we haven't
     * yet negotiated TLSv1.3 at that point so that is handled by
     * ossl_statem_client_read_transition()
     */

    switch (st->hand_state) {
    default:
        break;

    case TLS_ST_CW_CLNT_HELLO:
        /*
         * This must a ClientHello following a HelloRetryRequest, so the only
         * thing we can get now is a ServerHello.
         */
        if (mt == SSL3_MT_SERVER_HELLO) {
            st->hand_state = TLS_ST_CR_SRVR_HELLO;
            return 1;
        }
        break;

    case TLS_ST_CR_SRVR_HELLO:
        if (mt == SSL3_MT_ENCRYPTED_EXTENSIONS) {
            st->hand_state = TLS_ST_CR_ENCRYPTED_EXTENSIONS;
            return 1;
        }
        break;

    case TLS_ST_CR_ENCRYPTED_EXTENSIONS:
        if (s->hit) {
            if (mt == SSL3_MT_FINISHED) {
                st->hand_state = TLS_ST_CR_FINISHED;
                return 1;
            }
        } else {
            if (mt == SSL3_MT_CERTIFICATE_REQUEST) {
                st->hand_state = TLS_ST_CR_CERT_REQ;
                return 1;
            }
            if (mt == SSL3_MT_CERTIFICATE) {
                st->hand_state = TLS_ST_CR_CERT;
                return 1;
            }
#ifndef OPENSSL_NO_COMP_ALG
            if (mt == SSL3_MT_COMPRESSED_CERTIFICATE
                    && s->ext.compress_certificate_sent) {
                st->hand_state = TLS_ST_CR_COMP_CERT;
                return 1;
            }
#endif
        }
        break;

    case TLS_ST_CR_CERT_REQ:
        if (mt == SSL3_MT_CERTIFICATE) {
            st->hand_state = TLS_ST_CR_CERT;
            return 1;
        }
#ifndef OPENSSL_NO_COMP_ALG
        if (mt == SSL3_MT_COMPRESSED_CERTIFICATE
                && s->ext.compress_certificate_sent) {
            st->hand_state = TLS_ST_CR_COMP_CERT;
            return 1;
        }
#endif
        break;

    case TLS_ST_CR_CERT:
    case TLS_ST_CR_COMP_CERT:
        if (mt == SSL3_MT_CERTIFICATE_VERIFY) {
            st->hand_state = TLS_ST_CR_CERT_VRFY;
            return 1;
        }
        break;

    case TLS_ST_CR_CERT_VRFY:
        if (mt == SSL3_MT_FINISHED) {
            st->hand_state = TLS_ST_CR_FINISHED;
            return 1;
        }
        break;

    case TLS_ST_OK:
        if (mt == SSL3_MT_NEWSESSION_TICKET) {
            st->hand_state = TLS_ST_CR_SESSION_TICKET;
            return 1;
        }
        if (mt == SSL3_MT_KEY_UPDATE && !SSL_IS_QUIC_HANDSHAKE(s)) {
            st->hand_state = TLS_ST_CR_KEY_UPDATE;
            return 1;
        }
        if (mt == SSL3_MT_CERTIFICATE_REQUEST) {
#if DTLS_MAX_VERSION_INTERNAL != DTLS1_2_VERSION
            /* Restore digest for PHA before adding message.*/
# error Internal DTLS version error
#endif
            if (!SSL_CONNECTION_IS_DTLS(s)
                && s->post_handshake_auth == SSL_PHA_EXT_SENT) {
                s->post_handshake_auth = SSL_PHA_REQUESTED;
                /*
                 * In TLS, this is called before the message is added to the
                 * digest. In DTLS, this is expected to be called after adding
                 * to the digest. Either move the digest restore, or add the
                 * message here after the swap, or do it after the clientFinished?
                 */
                if (!tls13_restore_handshake_digest_for_pha(s)) {
                    /* SSLfatal() already called */
                    return 0;
                }
                st->hand_state = TLS_ST_CR_CERT_REQ;
                return 1;
            }
        }
        break;
    }

    /* No valid transition found */
    return 0;
}

/*
 * ossl_statem_client_read_transition() encapsulates the logic for the allowed
 * handshake state transitions when the client is reading messages from the
 * server. The message type that the server has sent is provided in |mt|. The
 * current state is in |s->statem.hand_state|.
 *
 * Return values are 1 for success (transition allowed) and  0 on error
 * (transition not allowed)
 */
int ossl_statem_client_read_transition(SSL_CONNECTION *s, int mt)
{
    OSSL_STATEM *st = &s->statem;
    int ske_expected;

    /*
     * Note that after writing the first ClientHello we don't know what version
     * we are going to negotiate yet, so we don't take this branch until later.
     */
    if (SSL_CONNECTION_IS_TLS13(s)) {
        if (!ossl_statem_client13_read_transition(s, mt))
            goto err;
        return 1;
    }

    switch (st->hand_state) {
    default:
        break;

    case TLS_ST_CW_CLNT_HELLO:
        if (mt == SSL3_MT_SERVER_HELLO) {
            st->hand_state = TLS_ST_CR_SRVR_HELLO;
            return 1;
        }

        if (SSL_CONNECTION_IS_DTLS(s)) {
            if (mt == DTLS1_MT_HELLO_VERIFY_REQUEST) {
                st->hand_state = DTLS_ST_CR_HELLO_VERIFY_REQUEST;
                return 1;
            }
        }
        break;

    case TLS_ST_EARLY_DATA:
        /*
         * We've not actually selected TLSv1.3 yet, but we have sent early
         * data. The only thing allowed now is a ServerHello or a
         * HelloRetryRequest.
         */
        if (mt == SSL3_MT_SERVER_HELLO) {
            st->hand_state = TLS_ST_CR_SRVR_HELLO;
            return 1;
        }
        break;

    case TLS_ST_CR_SRVR_HELLO:
        if (s->hit) {
            if (s->ext.ticket_expected) {
                if (mt == SSL3_MT_NEWSESSION_TICKET) {
                    st->hand_state = TLS_ST_CR_SESSION_TICKET;
                    return 1;
                }
            } else if (mt == SSL3_MT_CHANGE_CIPHER_SPEC) {
                st->hand_state = TLS_ST_CR_CHANGE;
                return 1;
            }
        } else {
            if (SSL_CONNECTION_IS_DTLS(s)
                && mt == DTLS1_MT_HELLO_VERIFY_REQUEST) {
                st->hand_state = DTLS_ST_CR_HELLO_VERIFY_REQUEST;
                return 1;
            } else if (s->version >= TLS1_VERSION
                       && s->ext.session_secret_cb != NULL
                       && s->session->ext.tick != NULL
                       && mt == SSL3_MT_CHANGE_CIPHER_SPEC) {
                /*
                 * Normally, we can tell if the server is resuming the session
                 * from the session ID. EAP-FAST (RFC 4851), however, relies on
                 * the next server message after the ServerHello to determine if
                 * the server is resuming.
                 */
                s->hit = 1;
                st->hand_state = TLS_ST_CR_CHANGE;
                return 1;
            } else if (!(s->s3.tmp.new_cipher->algorithm_auth
                         & (SSL_aNULL | SSL_aSRP | SSL_aPSK))) {
                if (mt == SSL3_MT_CERTIFICATE) {
                    st->hand_state = TLS_ST_CR_CERT;
                    return 1;
                }
            } else {
                ske_expected = key_exchange_expected(s);
                /* SKE is optional for some PSK ciphersuites */
                if (ske_expected
                    || ((s->s3.tmp.new_cipher->algorithm_mkey & SSL_PSK)
                        && mt == SSL3_MT_SERVER_KEY_EXCHANGE)) {
                    if (mt == SSL3_MT_SERVER_KEY_EXCHANGE) {
                        st->hand_state = TLS_ST_CR_KEY_EXCH;
                        return 1;
                    }
                } else if (mt == SSL3_MT_CERTIFICATE_REQUEST
                           && cert_req_allowed(s)) {
                    st->hand_state = TLS_ST_CR_CERT_REQ;
                    return 1;
                } else if (mt == SSL3_MT_SERVER_DONE) {
                    st->hand_state = TLS_ST_CR_SRVR_DONE;
                    return 1;
                }
            }
        }
        break;

    case TLS_ST_CR_CERT:
    case TLS_ST_CR_COMP_CERT:
        /*
         * The CertificateStatus message is optional even if
         * |ext.status_expected| is set
         */
        if (s->ext.status_expected && mt == SSL3_MT_CERTIFICATE_STATUS) {
            st->hand_state = TLS_ST_CR_CERT_STATUS;
            return 1;
        }
        /* Fall through */

    case TLS_ST_CR_CERT_STATUS:
        ske_expected = key_exchange_expected(s);
        /* SKE is optional for some PSK ciphersuites */
        if (ske_expected || ((s->s3.tmp.new_cipher->algorithm_mkey & SSL_PSK)
                             && mt == SSL3_MT_SERVER_KEY_EXCHANGE)) {
            if (mt == SSL3_MT_SERVER_KEY_EXCHANGE) {
                st->hand_state = TLS_ST_CR_KEY_EXCH;
                return 1;
            }
            goto err;
        }
        /* Fall through */

    case TLS_ST_CR_KEY_EXCH:
        if (mt == SSL3_MT_CERTIFICATE_REQUEST) {
            if (cert_req_allowed(s)) {
                st->hand_state = TLS_ST_CR_CERT_REQ;
                return 1;
            }
            goto err;
        }
        /* Fall through */

    case TLS_ST_CR_CERT_REQ:
        if (mt == SSL3_MT_SERVER_DONE) {
            st->hand_state = TLS_ST_CR_SRVR_DONE;
            return 1;
        }
        break;

    case TLS_ST_CW_FINISHED:
        if (s->ext.ticket_expected) {
            if (mt == SSL3_MT_NEWSESSION_TICKET) {
                st->hand_state = TLS_ST_CR_SESSION_TICKET;
                return 1;
            }
        } else if (mt == SSL3_MT_CHANGE_CIPHER_SPEC) {
            st->hand_state = TLS_ST_CR_CHANGE;
            return 1;
        }
        break;

    case TLS_ST_CR_SESSION_TICKET:
        if (mt == SSL3_MT_CHANGE_CIPHER_SPEC) {
            st->hand_state = TLS_ST_CR_CHANGE;
            return 1;
        }
        break;

    case TLS_ST_CR_CHANGE:
        if (mt == SSL3_MT_FINISHED) {
            st->hand_state = TLS_ST_CR_FINISHED;
            return 1;
        }
        break;

    case TLS_ST_OK:
        if (mt == SSL3_MT_HELLO_REQUEST) {
            st->hand_state = TLS_ST_CR_HELLO_REQ;
            return 1;
        }
        break;
    }

 err:
    /* No valid transition found */
    if (SSL_CONNECTION_IS_DTLS(s) && mt == SSL3_MT_CHANGE_CIPHER_SPEC) {
        BIO *rbio;

        /*
         * CCS messages don't have a message sequence number so this is probably
         * because of an out-of-order CCS. We'll just drop it.
         */
        s->init_num = 0;
        s->rwstate = SSL_READING;
        rbio = SSL_get_rbio(SSL_CONNECTION_GET_SSL(s));
        BIO_clear_retry_flags(rbio);
        BIO_set_retry_read(rbio);
        return 0;
    }
    SSLfatal(s, SSL3_AD_UNEXPECTED_MESSAGE, SSL_R_UNEXPECTED_MESSAGE);
    return 0;
}

static int do_compressed_cert(SSL_CONNECTION *sc)
{
    /* If we negotiated RPK, we won't try to compress it */
    return sc->ext.client_cert_type == TLSEXT_cert_type_x509
        && sc->ext.compress_certificate_from_peer[0] != TLSEXT_comp_cert_none;
}

/*
 * ossl_statem_client13_write_transition() works out what handshake state to
 * move to next when the TLSv1.3 client is writing messages to be sent to the
 * server.
 */
static WRITE_TRAN ossl_statem_client13_write_transition(SSL_CONNECTION *s)
{
    OSSL_STATEM *st = &s->statem;

    /*
     * Note: There are no cases for TLS_ST_BEFORE because we haven't negotiated
     * TLSv1.3 yet at that point. They are handled by
     * ossl_statem_client_write_transition().
     */
    switch (st->hand_state) {
    default:
        /* Shouldn't happen */
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return WRITE_TRAN_ERROR;

    case TLS_ST_CR_CERT_REQ:
        if (s->post_handshake_auth == SSL_PHA_REQUESTED) {
            if (do_compressed_cert(s))
                st->hand_state = TLS_ST_CW_COMP_CERT;
            else
                st->hand_state = TLS_ST_CW_CERT;
            return WRITE_TRAN_CONTINUE;
        }
        /*
         * We should only get here if we received a CertificateRequest after
         * we already sent close_notify
         */
        if (!ossl_assert((s->shutdown & SSL_SENT_SHUTDOWN) != 0)) {
            /* Shouldn't happen - same as default case */
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            return WRITE_TRAN_ERROR;
        }
        st->hand_state = TLS_ST_OK;
        return WRITE_TRAN_CONTINUE;

    case TLS_ST_CR_FINISHED:
        if (s->early_data_state == SSL_EARLY_DATA_WRITE_RETRY
                || s->early_data_state == SSL_EARLY_DATA_FINISHED_WRITING)
            st->hand_state = TLS_ST_PENDING_EARLY_DATA_END;
        else if ((s->options & SSL_OP_ENABLE_MIDDLEBOX_COMPAT) != 0
                 && s->hello_retry_request == SSL_HRR_NONE)
            st->hand_state = TLS_ST_CW_CHANGE;
        else if (s->s3.tmp.cert_req == 0)
            st->hand_state = TLS_ST_CW_FINISHED;
        else if (do_compressed_cert(s))
            st->hand_state = TLS_ST_CW_COMP_CERT;
        else
            st->hand_state = TLS_ST_CW_CERT;

        s->ts_msg_read = ossl_time_now();
        return WRITE_TRAN_CONTINUE;

    case TLS_ST_PENDING_EARLY_DATA_END:
        if (s->ext.early_data == SSL_EARLY_DATA_ACCEPTED && !SSL_NO_EOED(s)) {
            st->hand_state = TLS_ST_CW_END_OF_EARLY_DATA;
            return WRITE_TRAN_CONTINUE;
        }
        /* Fall through */

    case TLS_ST_CW_END_OF_EARLY_DATA:
    case TLS_ST_CW_CHANGE:
        if (s->s3.tmp.cert_req == 0)
            st->hand_state = TLS_ST_CW_FINISHED;
        else if (do_compressed_cert(s))
            st->hand_state = TLS_ST_CW_COMP_CERT;
        else
            st->hand_state = TLS_ST_CW_CERT;
        return WRITE_TRAN_CONTINUE;

    case TLS_ST_CW_COMP_CERT:
    case TLS_ST_CW_CERT:
        /* If a non-empty Certificate we also send CertificateVerify */
        st->hand_state = (s->s3.tmp.cert_req == 1) ? TLS_ST_CW_CERT_VRFY
                                                    : TLS_ST_CW_FINISHED;
        return WRITE_TRAN_CONTINUE;

    case TLS_ST_CW_CERT_VRFY:
        st->hand_state = TLS_ST_CW_FINISHED;
        return WRITE_TRAN_CONTINUE;

    case TLS_ST_CR_KEY_UPDATE:
    case TLS_ST_CW_KEY_UPDATE:
    case TLS_ST_CR_SESSION_TICKET:
    case TLS_ST_CW_FINISHED:
        st->hand_state = TLS_ST_OK;
        return WRITE_TRAN_CONTINUE;

    case TLS_ST_OK:
        if (s->key_update != SSL_KEY_UPDATE_NONE) {
            st->hand_state = TLS_ST_CW_KEY_UPDATE;
            return WRITE_TRAN_CONTINUE;
        }

        /* Try to read from the server instead */
        return WRITE_TRAN_FINISHED;
    }
}

/*
 * ossl_statem_client_write_transition() works out what handshake state to
 * move to next when the client is writing messages to be sent to the server.
 */
WRITE_TRAN ossl_statem_client_write_transition(SSL_CONNECTION *s)
{
    OSSL_STATEM *st = &s->statem;

    /*
     * Note that immediately before/after a ClientHello we don't know what
     * version we are going to negotiate yet, so we don't take this branch until
     * later
     */
    if (SSL_CONNECTION_IS_TLS13(s))
        return ossl_statem_client13_write_transition(s);

    switch (st->hand_state) {
    default:
        /* Shouldn't happen */
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return WRITE_TRAN_ERROR;

    case TLS_ST_OK:
        if (!s->renegotiate) {
            /*
             * We haven't requested a renegotiation ourselves so we must have
             * received a message from the server. Better read it.
             */
            return WRITE_TRAN_FINISHED;
        }
        /* Renegotiation */
        /* fall thru */
    case TLS_ST_BEFORE:
        st->hand_state = TLS_ST_CW_CLNT_HELLO;
        return WRITE_TRAN_CONTINUE;

    case TLS_ST_CW_CLNT_HELLO:
        if (s->early_data_state == SSL_EARLY_DATA_CONNECTING
                && !SSL_IS_QUIC_HANDSHAKE(s)) {
            /*
             * We are assuming this is a TLSv1.3 connection, although we haven't
             * actually selected a version yet.
             */
            if ((s->options & SSL_OP_ENABLE_MIDDLEBOX_COMPAT) != 0)
                st->hand_state = TLS_ST_CW_CHANGE;
            else
                st->hand_state = TLS_ST_EARLY_DATA;
            return WRITE_TRAN_CONTINUE;
        }
        /*
         * No transition at the end of writing because we don't know what
         * we will be sent
         */
        s->ts_msg_write = ossl_time_now();
        return WRITE_TRAN_FINISHED;

    case TLS_ST_CR_SRVR_HELLO:
        /*
         * We only get here in TLSv1.3. We just received an HRR, so issue a
         * CCS unless middlebox compat mode is off, or we already issued one
         * because we did early data.
         */
        if ((s->options & SSL_OP_ENABLE_MIDDLEBOX_COMPAT) != 0
                && s->early_data_state != SSL_EARLY_DATA_FINISHED_WRITING)
            st->hand_state = TLS_ST_CW_CHANGE;
        else
            st->hand_state = TLS_ST_CW_CLNT_HELLO;
        return WRITE_TRAN_CONTINUE;

    case TLS_ST_EARLY_DATA:
        s->ts_msg_write = ossl_time_now();
        return WRITE_TRAN_FINISHED;

    case DTLS_ST_CR_HELLO_VERIFY_REQUEST:
        st->hand_state = TLS_ST_CW_CLNT_HELLO;
        return WRITE_TRAN_CONTINUE;

    case TLS_ST_CR_SRVR_DONE:
        s->ts_msg_read = ossl_time_now();
        if (s->s3.tmp.cert_req)
            st->hand_state = TLS_ST_CW_CERT;
        else
            st->hand_state = TLS_ST_CW_KEY_EXCH;
        return WRITE_TRAN_CONTINUE;

    case TLS_ST_CW_CERT:
        st->hand_state = TLS_ST_CW_KEY_EXCH;
        return WRITE_TRAN_CONTINUE;

    case TLS_ST_CW_KEY_EXCH:
        /*
         * For TLS, cert_req is set to 2, so a cert chain of nothing is
         * sent, but no verify packet is sent
         */
        /*
         * XXX: For now, we do not support client authentication in ECDH
         * cipher suites with ECDH (rather than ECDSA) certificates. We
         * need to skip the certificate verify message when client's
         * ECDH public key is sent inside the client certificate.
         */
        if (s->s3.tmp.cert_req == 1) {
            st->hand_state = TLS_ST_CW_CERT_VRFY;
        } else {
            st->hand_state = TLS_ST_CW_CHANGE;
        }
        if (s->s3.flags & TLS1_FLAGS_SKIP_CERT_VERIFY) {
            st->hand_state = TLS_ST_CW_CHANGE;
        }
        return WRITE_TRAN_CONTINUE;

    case TLS_ST_CW_CERT_VRFY:
        st->hand_state = TLS_ST_CW_CHANGE;
        return WRITE_TRAN_CONTINUE;

    case TLS_ST_CW_CHANGE:
        if (s->hello_retry_request == SSL_HRR_PENDING) {
            st->hand_state = TLS_ST_CW_CLNT_HELLO;
        } else if (s->early_data_state == SSL_EARLY_DATA_CONNECTING) {
            st->hand_state = TLS_ST_EARLY_DATA;
        } else {
#if defined(OPENSSL_NO_NEXTPROTONEG)
            st->hand_state = TLS_ST_CW_FINISHED;
#else
            if (!SSL_CONNECTION_IS_DTLS(s) && s->s3.npn_seen)
                st->hand_state = TLS_ST_CW_NEXT_PROTO;
            else
                st->hand_state = TLS_ST_CW_FINISHED;
#endif
        }
        return WRITE_TRAN_CONTINUE;

#if !defined(OPENSSL_NO_NEXTPROTONEG)
    case TLS_ST_CW_NEXT_PROTO:
        st->hand_state = TLS_ST_CW_FINISHED;
        return WRITE_TRAN_CONTINUE;
#endif

    case TLS_ST_CW_FINISHED:
        if (s->hit) {
            st->hand_state = TLS_ST_OK;
            return WRITE_TRAN_CONTINUE;
        } else {
            return WRITE_TRAN_FINISHED;
        }

    case TLS_ST_CR_FINISHED:
        if (s->hit) {
            st->hand_state = TLS_ST_CW_CHANGE;
            return WRITE_TRAN_CONTINUE;
        } else {
            st->hand_state = TLS_ST_OK;
            return WRITE_TRAN_CONTINUE;
        }

    case TLS_ST_CR_HELLO_REQ:
        /*
         * If we can renegotiate now then do so, otherwise wait for a more
         * convenient time.
         */
        if (ssl3_renegotiate_check(SSL_CONNECTION_GET_SSL(s), 1)) {
            if (!tls_setup_handshake(s)) {
                /* SSLfatal() already called */
                return WRITE_TRAN_ERROR;
            }
            st->hand_state = TLS_ST_CW_CLNT_HELLO;
            return WRITE_TRAN_CONTINUE;
        }
        st->hand_state = TLS_ST_OK;
        return WRITE_TRAN_CONTINUE;
    }
}

/*
 * Perform any pre work that needs to be done prior to sending a message from
 * the client to the server.
 */
WORK_STATE ossl_statem_client_pre_work(SSL_CONNECTION *s, WORK_STATE wst)
{
    OSSL_STATEM *st = &s->statem;

    switch (st->hand_state) {
    default:
        /* No pre work to be done */
        break;

    case TLS_ST_CW_CLNT_HELLO:
        s->shutdown = 0;
        if (SSL_CONNECTION_IS_DTLS(s)) {
            /* every DTLS ClientHello resets Finished MAC */
            if (!ssl3_init_finished_mac(s)) {
                /* SSLfatal() already called */
                return WORK_ERROR;
            }
        } else if (s->ext.early_data == SSL_EARLY_DATA_REJECTED) {
            /*
             * This must be a second ClientHello after an HRR following an
             * earlier rejected attempt to send early data. Since we were
             * previously encrypting the early data we now need to reset the
             * write record layer in order to write in plaintext again.
             */
            if (!ssl_set_new_record_layer(s,
                                          TLS_ANY_VERSION,
                                          OSSL_RECORD_DIRECTION_WRITE,
                                          OSSL_RECORD_PROTECTION_LEVEL_NONE,
                                          NULL, 0, NULL, 0, NULL, 0, NULL,  0,
                                          NULL, 0, NID_undef, NULL, NULL,
                                          NULL)) {
                /* SSLfatal already called */
                return WORK_ERROR;
            }
        }
        break;

    case TLS_ST_CW_CHANGE:
        if (SSL_CONNECTION_IS_DTLS(s)) {
            if (s->hit) {
                /*
                 * We're into the last flight so we don't retransmit these
                 * messages unless we need to.
                 */
                st->use_timer = 0;
            }
#ifndef OPENSSL_NO_SCTP
            if (BIO_dgram_is_sctp(SSL_get_wbio(SSL_CONNECTION_GET_SSL(s)))) {
                /* Calls SSLfatal() as required */
                return dtls_wait_for_dry(s);
            }
#endif
        }
        break;

    case TLS_ST_PENDING_EARLY_DATA_END:
        /*
         * If we've been called by SSL_do_handshake()/SSL_write(), or we did not
         * attempt to write early data before calling SSL_read() then we press
         * on with the handshake. Otherwise we pause here.
         */
        if (s->early_data_state == SSL_EARLY_DATA_FINISHED_WRITING
                || s->early_data_state == SSL_EARLY_DATA_NONE)
            return WORK_FINISHED_CONTINUE;
        /* Fall through */

    case TLS_ST_EARLY_DATA:
        return tls_finish_handshake(s, wst, 0, 1);

    case TLS_ST_OK:
        /* Calls SSLfatal() as required */
        return tls_finish_handshake(s, wst, 1, 1);
    }

    return WORK_FINISHED_CONTINUE;
}

/*
 * Perform any work that needs to be done after sending a message from the
 * client to the server.
 */
WORK_STATE ossl_statem_client_post_work(SSL_CONNECTION *s, WORK_STATE wst)
{
    OSSL_STATEM *st = &s->statem;
    SSL *ssl = SSL_CONNECTION_GET_SSL(s);

    s->init_num = 0;

    switch (st->hand_state) {
    default:
        /* No post work to be done */
        break;

    case TLS_ST_CW_CLNT_HELLO:
        if (s->early_data_state == SSL_EARLY_DATA_CONNECTING
                && s->max_early_data > 0) {
            /*
             * We haven't selected TLSv1.3 yet so we don't call the change
             * cipher state function associated with the SSL_METHOD. Instead
             * we call tls13_change_cipher_state() directly.
             */
            if ((s->options & SSL_OP_ENABLE_MIDDLEBOX_COMPAT) == 0) {
                if (!tls13_change_cipher_state(s,
                            SSL3_CC_EARLY | SSL3_CHANGE_CIPHER_CLIENT_WRITE)) {
                    /* SSLfatal() already called */
                    return WORK_ERROR;
                }
            }
            /* else we're in compat mode so we delay flushing until after CCS */
        } else if (!statem_flush(s)) {
            return WORK_MORE_A;
        }

        if (SSL_CONNECTION_IS_DTLS(s)) {
            /* Treat the next message as the first packet */
            s->first_packet = 1;
        }
        break;

    case TLS_ST_CW_KEY_EXCH:
        if (tls_client_key_exchange_post_work(s) == 0) {
            /* SSLfatal() already called */
            return WORK_ERROR;
        }
        break;

    case TLS_ST_CW_CHANGE:
        if (SSL_CONNECTION_IS_TLS13(s)
            || s->hello_retry_request == SSL_HRR_PENDING)
            break;
        if (s->early_data_state == SSL_EARLY_DATA_CONNECTING
                    && s->max_early_data > 0) {
            /*
             * We haven't selected TLSv1.3 yet so we don't call the change
             * cipher state function associated with the SSL_METHOD. Instead
             * we call tls13_change_cipher_state() directly.
             */
            if (!tls13_change_cipher_state(s,
                        SSL3_CC_EARLY | SSL3_CHANGE_CIPHER_CLIENT_WRITE))
                return WORK_ERROR;
            break;
        }
        s->session->cipher = s->s3.tmp.new_cipher;
#ifdef OPENSSL_NO_COMP
        s->session->compress_meth = 0;
#else
        if (s->s3.tmp.new_compression == NULL)
            s->session->compress_meth = 0;
        else
            s->session->compress_meth = s->s3.tmp.new_compression->id;
#endif
        if (!ssl->method->ssl3_enc->setup_key_block(s)) {
            /* SSLfatal() already called */
            return WORK_ERROR;
        }

        if (!ssl->method->ssl3_enc->change_cipher_state(s,
                                          SSL3_CHANGE_CIPHER_CLIENT_WRITE)) {
            /* SSLfatal() already called */
            return WORK_ERROR;
        }

#ifndef OPENSSL_NO_SCTP
        if (SSL_CONNECTION_IS_DTLS(s) && s->hit) {
            /*
            * Change to new shared key of SCTP-Auth, will be ignored if
            * no SCTP used.
            */
            BIO_ctrl(SSL_get_wbio(ssl), BIO_CTRL_DGRAM_SCTP_NEXT_AUTH_KEY,
                     0, NULL);
        }
#endif
        break;

    case TLS_ST_CW_FINISHED:
#ifndef OPENSSL_NO_SCTP
        if (wst == WORK_MORE_A && SSL_CONNECTION_IS_DTLS(s) && s->hit == 0) {
            /*
             * Change to new shared key of SCTP-Auth, will be ignored if
             * no SCTP used.
             */
            BIO_ctrl(SSL_get_wbio(ssl), BIO_CTRL_DGRAM_SCTP_NEXT_AUTH_KEY,
                     0, NULL);
        }
#endif
        if (statem_flush(s) != 1)
            return WORK_MORE_B;

        if (SSL_CONNECTION_IS_TLS13(s)) {
            if (!tls13_save_handshake_digest_for_pha(s)) {
                /* SSLfatal() already called */
                return WORK_ERROR;
            }
            if (s->post_handshake_auth != SSL_PHA_REQUESTED) {
                if (!ssl->method->ssl3_enc->change_cipher_state(s,
                        SSL3_CC_APPLICATION | SSL3_CHANGE_CIPHER_CLIENT_WRITE)) {
                    /* SSLfatal() already called */
                    return WORK_ERROR;
                }
                /*
                 * For QUIC we deferred setting up these keys until now so
                 * that we can ensure write keys are always set up before read
                 * keys.
                 */
                if (SSL_IS_QUIC_HANDSHAKE(s)
                        && !ssl->method->ssl3_enc->change_cipher_state(s,
                            SSL3_CC_APPLICATION | SSL3_CHANGE_CIPHER_CLIENT_READ)) {
                    /* SSLfatal() already called */
                    return WORK_ERROR;
                }
            }
        }
        break;

    case TLS_ST_CW_KEY_UPDATE:
        if (statem_flush(s) != 1)
            return WORK_MORE_A;
        if (!tls13_update_key(s, 1)) {
            /* SSLfatal() already called */
            return WORK_ERROR;
        }
        break;
    }

    return WORK_FINISHED_CONTINUE;
}

/*
 * Get the message construction function and message type for sending from the
 * client
 *
 * Valid return values are:
 *   1: Success
 *   0: Error
 */
int ossl_statem_client_construct_message(SSL_CONNECTION *s,
                                         confunc_f *confunc, int *mt)
{
    OSSL_STATEM *st = &s->statem;

    switch (st->hand_state) {
    default:
        /* Shouldn't happen */
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, SSL_R_BAD_HANDSHAKE_STATE);
        return 0;

    case TLS_ST_CW_CHANGE:
        if (SSL_CONNECTION_IS_DTLS(s))
            *confunc = dtls_construct_change_cipher_spec;
        else
            *confunc = tls_construct_change_cipher_spec;
        *mt = SSL3_MT_CHANGE_CIPHER_SPEC;
        break;

    case TLS_ST_CW_CLNT_HELLO:
        *confunc = tls_construct_client_hello;
        *mt = SSL3_MT_CLIENT_HELLO;
        break;

    case TLS_ST_CW_END_OF_EARLY_DATA:
        *confunc = tls_construct_end_of_early_data;
        *mt = SSL3_MT_END_OF_EARLY_DATA;
        break;

    case TLS_ST_PENDING_EARLY_DATA_END:
        *confunc = NULL;
        *mt = SSL3_MT_DUMMY;
        break;

    case TLS_ST_CW_CERT:
        *confunc = tls_construct_client_certificate;
        *mt = SSL3_MT_CERTIFICATE;
        break;

#ifndef OPENSSL_NO_COMP_ALG
    case TLS_ST_CW_COMP_CERT:
        *confunc = tls_construct_client_compressed_certificate;
        *mt = SSL3_MT_COMPRESSED_CERTIFICATE;
        break;
#endif

    case TLS_ST_CW_KEY_EXCH:
        *confunc = tls_construct_client_key_exchange;
        *mt = SSL3_MT_CLIENT_KEY_EXCHANGE;
        break;

    case TLS_ST_CW_CERT_VRFY:
        *confunc = tls_construct_cert_verify;
        *mt = SSL3_MT_CERTIFICATE_VERIFY;
        break;

#if !defined(OPENSSL_NO_NEXTPROTONEG)
    case TLS_ST_CW_NEXT_PROTO:
        *confunc = tls_construct_next_proto;
        *mt = SSL3_MT_NEXT_PROTO;
        break;
#endif
    case TLS_ST_CW_FINISHED:
        *confunc = tls_construct_finished;
        *mt = SSL3_MT_FINISHED;
        break;

    case TLS_ST_CW_KEY_UPDATE:
        *confunc = tls_construct_key_update;
        *mt = SSL3_MT_KEY_UPDATE;
        break;
    }

    return 1;
}

/*
 * Returns the maximum allowed length for the current message that we are
 * reading. Excludes the message header.
 */
size_t ossl_statem_client_max_message_size(SSL_CONNECTION *s)
{
    OSSL_STATEM *st = &s->statem;

    switch (st->hand_state) {
    default:
        /* Shouldn't happen */
        return 0;

    case TLS_ST_CR_SRVR_HELLO:
        return SERVER_HELLO_MAX_LENGTH;

    case DTLS_ST_CR_HELLO_VERIFY_REQUEST:
        return HELLO_VERIFY_REQUEST_MAX_LENGTH;

    case TLS_ST_CR_COMP_CERT:
    case TLS_ST_CR_CERT:
        return s->max_cert_list;

    case TLS_ST_CR_CERT_VRFY:
        return CERTIFICATE_VERIFY_MAX_LENGTH;

    case TLS_ST_CR_CERT_STATUS:
        return SSL3_RT_MAX_PLAIN_LENGTH;

    case TLS_ST_CR_KEY_EXCH:
        return SERVER_KEY_EXCH_MAX_LENGTH;

    case TLS_ST_CR_CERT_REQ:
        /*
         * Set to s->max_cert_list for compatibility with previous releases. In
         * practice these messages can get quite long if servers are configured
         * to provide a long list of acceptable CAs
         */
        return s->max_cert_list;

    case TLS_ST_CR_SRVR_DONE:
        return SERVER_HELLO_DONE_MAX_LENGTH;

    case TLS_ST_CR_CHANGE:
        if (s->version == DTLS1_BAD_VER)
            return 3;
        return CCS_MAX_LENGTH;

    case TLS_ST_CR_SESSION_TICKET:
        return (SSL_CONNECTION_IS_TLS13(s)) ? SESSION_TICKET_MAX_LENGTH_TLS13
                                            : SESSION_TICKET_MAX_LENGTH_TLS12;

    case TLS_ST_CR_FINISHED:
        return FINISHED_MAX_LENGTH;

    case TLS_ST_CR_ENCRYPTED_EXTENSIONS:
        return ENCRYPTED_EXTENSIONS_MAX_LENGTH;

    case TLS_ST_CR_KEY_UPDATE:
        return KEY_UPDATE_MAX_LENGTH;
    }
}

/*
 * Process a message that the client has received from the server.
 */
MSG_PROCESS_RETURN ossl_statem_client_process_message(SSL_CONNECTION *s,
                                                      PACKET *pkt)
{
    OSSL_STATEM *st = &s->statem;

    switch (st->hand_state) {
    default:
        /* Shouldn't happen */
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return MSG_PROCESS_ERROR;

    case TLS_ST_CR_SRVR_HELLO:
        return tls_process_server_hello(s, pkt);

    case DTLS_ST_CR_HELLO_VERIFY_REQUEST:
        return dtls_process_hello_verify(s, pkt);

    case TLS_ST_CR_CERT:
        return tls_process_server_certificate(s, pkt);

#ifndef OPENSSL_NO_COMP_ALG
    case TLS_ST_CR_COMP_CERT:
        return tls_process_server_compressed_certificate(s, pkt);
#endif

    case TLS_ST_CR_CERT_VRFY:
        return tls_process_cert_verify(s, pkt);

    case TLS_ST_CR_CERT_STATUS:
        return tls_process_cert_status(s, pkt);

    case TLS_ST_CR_KEY_EXCH:
        return tls_process_key_exchange(s, pkt);

    case TLS_ST_CR_CERT_REQ:
        return tls_process_certificate_request(s, pkt);

    case TLS_ST_CR_SRVR_DONE:
        return tls_process_server_done(s, pkt);

    case TLS_ST_CR_CHANGE:
        return tls_process_change_cipher_spec(s, pkt);

    case TLS_ST_CR_SESSION_TICKET:
        return tls_process_new_session_ticket(s, pkt);

    case TLS_ST_CR_FINISHED:
        return tls_process_finished(s, pkt);

    case TLS_ST_CR_HELLO_REQ:
        return tls_process_hello_req(s, pkt);

    case TLS_ST_CR_ENCRYPTED_EXTENSIONS:
        return tls_process_encrypted_extensions(s, pkt);

    case TLS_ST_CR_KEY_UPDATE:
        return tls_process_key_update(s, pkt);
    }
}

/*
 * Perform any further processing required following the receipt of a message
 * from the server
 */
WORK_STATE ossl_statem_client_post_process_message(SSL_CONNECTION *s,
                                                   WORK_STATE wst)
{
    OSSL_STATEM *st = &s->statem;

    switch (st->hand_state) {
    default:
        /* Shouldn't happen */
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return WORK_ERROR;

    case TLS_ST_CR_CERT:
    case TLS_ST_CR_COMP_CERT:
        return tls_post_process_server_certificate(s, wst);

    case TLS_ST_CR_CERT_VRFY:
    case TLS_ST_CR_CERT_REQ:
        return tls_prepare_client_certificate(s, wst);
    }
}

CON_FUNC_RETURN tls_construct_client_hello(SSL_CONNECTION *s, WPACKET *pkt)
{
    unsigned char *p;
    size_t sess_id_len;
    int i, protverr;
#ifndef OPENSSL_NO_COMP
    SSL_COMP *comp;
#endif
    SSL_SESSION *sess = s->session;
    unsigned char *session_id;
    SSL_CTX *sctx = SSL_CONNECTION_GET_CTX(s);

    /* Work out what SSL/TLS/DTLS version to use */
    protverr = ssl_set_client_hello_version(s);
    if (protverr != 0) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, protverr);
        return CON_FUNC_ERROR;
    }

    if (sess == NULL
            || !ssl_version_supported(s, sess->ssl_version, NULL)
            || !SSL_SESSION_is_resumable(sess)) {
        if (s->hello_retry_request == SSL_HRR_NONE
                && !ssl_get_new_session(s, 0)) {
            /* SSLfatal() already called */
            return CON_FUNC_ERROR;
        }
    }
    /* else use the pre-loaded session */

    p = s->s3.client_random;

    /*
     * for DTLS if client_random is initialized, reuse it, we are
     * required to use same upon reply to HelloVerify
     */
    if (SSL_CONNECTION_IS_DTLS(s)) {
        size_t idx;
        i = 1;
        for (idx = 0; idx < sizeof(s->s3.client_random); idx++) {
            if (p[idx]) {
                i = 0;
                break;
            }
        }
    } else {
        i = (s->hello_retry_request == SSL_HRR_NONE);
    }

    if (i && ssl_fill_hello_random(s, 0, p, sizeof(s->s3.client_random),
                                   DOWNGRADE_NONE) <= 0) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return CON_FUNC_ERROR;
    }

    /*-
     * version indicates the negotiated version: for example from
     * an SSLv2/v3 compatible client hello). The client_version
     * field is the maximum version we permit and it is also
     * used in RSA encrypted premaster secrets. Some servers can
     * choke if we initially report a higher version then
     * renegotiate to a lower one in the premaster secret. This
     * didn't happen with TLS 1.0 as most servers supported it
     * but it can with TLS 1.1 or later if the server only supports
     * 1.0.
     *
     * Possible scenario with previous logic:
     *      1. Client hello indicates TLS 1.2
     *      2. Server hello says TLS 1.0
     *      3. RSA encrypted premaster secret uses 1.2.
     *      4. Handshake proceeds using TLS 1.0.
     *      5. Server sends hello request to renegotiate.
     *      6. Client hello indicates TLS v1.0 as we now
     *         know that is maximum server supports.
     *      7. Server chokes on RSA encrypted premaster secret
     *         containing version 1.0.
     *
     * For interoperability it should be OK to always use the
     * maximum version we support in client hello and then rely
     * on the checking of version to ensure the servers isn't
     * being inconsistent: for example initially negotiating with
     * TLS 1.0 and renegotiating with TLS 1.2. We do this by using
     * client_version in client hello and not resetting it to
     * the negotiated version.
     *
     * For TLS 1.3 we always set the ClientHello version to 1.2 and rely on the
     * supported_versions extension for the real supported versions.
     */
    if (!WPACKET_put_bytes_u16(pkt, s->client_version)
            || !WPACKET_memcpy(pkt, s->s3.client_random, SSL3_RANDOM_SIZE)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return CON_FUNC_ERROR;
    }

    /* Session ID */
    session_id = s->session->session_id;
    if (s->new_session || s->session->ssl_version == TLS1_3_VERSION) {
        if (s->version == TLS1_3_VERSION
                && (s->options & SSL_OP_ENABLE_MIDDLEBOX_COMPAT) != 0) {
            sess_id_len = sizeof(s->tmp_session_id);
            s->tmp_session_id_len = sess_id_len;
            session_id = s->tmp_session_id;
            if (s->hello_retry_request == SSL_HRR_NONE
                    && RAND_bytes_ex(sctx->libctx, s->tmp_session_id,
                                     sess_id_len, 0) <= 0) {
                SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
                return CON_FUNC_ERROR;
            }
        } else {
            sess_id_len = 0;
        }
    } else {
        assert(s->session->session_id_length <= sizeof(s->session->session_id));
        sess_id_len = s->session->session_id_length;
        if (s->version == TLS1_3_VERSION) {
            s->tmp_session_id_len = sess_id_len;
            memcpy(s->tmp_session_id, s->session->session_id, sess_id_len);
        }
    }
    if (!WPACKET_start_sub_packet_u8(pkt)
            || (sess_id_len != 0 && !WPACKET_memcpy(pkt, session_id,
                                                    sess_id_len))
            || !WPACKET_close(pkt)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return CON_FUNC_ERROR;
    }

    /* cookie stuff for DTLS */
    if (SSL_CONNECTION_IS_DTLS(s)) {
        if (s->d1->cookie_len > sizeof(s->d1->cookie)
                || !WPACKET_sub_memcpy_u8(pkt, s->d1->cookie,
                                          s->d1->cookie_len)) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            return CON_FUNC_ERROR;
        }
    }

    /* Ciphers supported */
    if (!WPACKET_start_sub_packet_u16(pkt)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return CON_FUNC_ERROR;
    }

    if (!ssl_cipher_list_to_bytes(s, SSL_get_ciphers(SSL_CONNECTION_GET_SSL(s)),
                                  pkt)) {
        /* SSLfatal() already called */
        return CON_FUNC_ERROR;
    }
    if (!WPACKET_close(pkt)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return CON_FUNC_ERROR;
    }

    /* COMPRESSION */
    if (!WPACKET_start_sub_packet_u8(pkt)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return CON_FUNC_ERROR;
    }
#ifndef OPENSSL_NO_COMP
    if (ssl_allow_compression(s)
            && sctx->comp_methods
            && (SSL_CONNECTION_IS_DTLS(s)
                || s->s3.tmp.max_ver < TLS1_3_VERSION)) {
        int compnum = sk_SSL_COMP_num(sctx->comp_methods);
        for (i = 0; i < compnum; i++) {
            comp = sk_SSL_COMP_value(sctx->comp_methods, i);
            if (!WPACKET_put_bytes_u8(pkt, comp->id)) {
                SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
                return CON_FUNC_ERROR;
            }
        }
    }
#endif
    /* Add the NULL method */
    if (!WPACKET_put_bytes_u8(pkt, 0) || !WPACKET_close(pkt)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return CON_FUNC_ERROR;
    }

    /* TLS extensions */
    if (!tls_construct_extensions(s, pkt, SSL_EXT_CLIENT_HELLO, NULL, 0)) {
        /* SSLfatal() already called */
        return CON_FUNC_ERROR;
    }

    return CON_FUNC_SUCCESS;
}

MSG_PROCESS_RETURN dtls_process_hello_verify(SSL_CONNECTION *s, PACKET *pkt)
{
    size_t cookie_len;
    PACKET cookiepkt;

    if (!PACKET_forward(pkt, 2)
        || !PACKET_get_length_prefixed_1(pkt, &cookiepkt)) {
        SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_MISMATCH);
        return MSG_PROCESS_ERROR;
    }

    cookie_len = PACKET_remaining(&cookiepkt);
    if (cookie_len > sizeof(s->d1->cookie)) {
        SSLfatal(s, SSL_AD_ILLEGAL_PARAMETER, SSL_R_LENGTH_TOO_LONG);
        return MSG_PROCESS_ERROR;
    }

    if (!PACKET_copy_bytes(&cookiepkt, s->d1->cookie, cookie_len)) {
        SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_MISMATCH);
        return MSG_PROCESS_ERROR;
    }
    s->d1->cookie_len = cookie_len;

    return MSG_PROCESS_FINISHED_READING;
}

static int set_client_ciphersuite(SSL_CONNECTION *s,
                                  const unsigned char *cipherchars)
{
    STACK_OF(SSL_CIPHER) *sk;
    const SSL_CIPHER *c;
    int i;
    SSL_CTX *sctx = SSL_CONNECTION_GET_CTX(s);

    c = ssl_get_cipher_by_char(s, cipherchars, 0);
    if (c == NULL) {
        /* unknown cipher */
        SSLfatal(s, SSL_AD_ILLEGAL_PARAMETER, SSL_R_UNKNOWN_CIPHER_RETURNED);
        return 0;
    }
    /*
     * If it is a disabled cipher we either didn't send it in client hello,
     * or it's not allowed for the selected protocol. So we return an error.
     */
    if (ssl_cipher_disabled(s, c, SSL_SECOP_CIPHER_CHECK, 1)) {
        SSLfatal(s, SSL_AD_ILLEGAL_PARAMETER, SSL_R_WRONG_CIPHER_RETURNED);
        return 0;
    }

    sk = ssl_get_ciphers_by_id(s);
    i = sk_SSL_CIPHER_find(sk, c);
    if (i < 0) {
        /* we did not say we would use this cipher */
        SSLfatal(s, SSL_AD_ILLEGAL_PARAMETER, SSL_R_WRONG_CIPHER_RETURNED);
        return 0;
    }

    if (SSL_CONNECTION_IS_TLS13(s) && s->s3.tmp.new_cipher != NULL
            && s->s3.tmp.new_cipher->id != c->id) {
        /* ServerHello selected a different ciphersuite to that in the HRR */
        SSLfatal(s, SSL_AD_ILLEGAL_PARAMETER, SSL_R_WRONG_CIPHER_RETURNED);
        return 0;
    }

    /*
     * Depending on the session caching (internal/external), the cipher
     * and/or cipher_id values may not be set. Make sure that cipher_id is
     * set and use it for comparison.
     */
    if (s->session->cipher != NULL)
        s->session->cipher_id = s->session->cipher->id;
    if (s->hit && (s->session->cipher_id != c->id)) {
        if (SSL_CONNECTION_IS_TLS13(s)) {
            const EVP_MD *md = ssl_md(sctx, c->algorithm2);

            if (!ossl_assert(s->session->cipher != NULL)) {
                SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
                return 0;
            }
            /*
             * In TLSv1.3 it is valid for the server to select a different
             * ciphersuite as long as the hash is the same.
             */
            if (md == NULL
                    || md != ssl_md(sctx, s->session->cipher->algorithm2)) {
                SSLfatal(s, SSL_AD_ILLEGAL_PARAMETER,
                         SSL_R_CIPHERSUITE_DIGEST_HAS_CHANGED);
                return 0;
            }
        } else {
            /*
             * Prior to TLSv1.3 resuming a session always meant using the same
             * ciphersuite.
             */
            SSLfatal(s, SSL_AD_ILLEGAL_PARAMETER,
                     SSL_R_OLD_SESSION_CIPHER_NOT_RETURNED);
            return 0;
        }
    }
    s->s3.tmp.new_cipher = c;

    return 1;
}

MSG_PROCESS_RETURN tls_process_server_hello(SSL_CONNECTION *s, PACKET *pkt)
{
    PACKET session_id, extpkt;
    size_t session_id_len;
    const unsigned char *cipherchars;
    int hrr = 0;
    unsigned int compression;
    unsigned int sversion;
    unsigned int context;
    RAW_EXTENSION *extensions = NULL;
    SSL *ssl = SSL_CONNECTION_GET_SSL(s);
    SSL *ussl = SSL_CONNECTION_GET_USER_SSL(s);
#ifndef OPENSSL_NO_COMP
    SSL_COMP *comp;
#endif

    if (!PACKET_get_net_2(pkt, &sversion)) {
        SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_MISMATCH);
        goto err;
    }

    /* load the server random */
    if (s->version == TLS1_3_VERSION
            && sversion == TLS1_2_VERSION
            && PACKET_remaining(pkt) >= SSL3_RANDOM_SIZE
            && memcmp(hrrrandom, PACKET_data(pkt), SSL3_RANDOM_SIZE) == 0) {
        if (s->hello_retry_request != SSL_HRR_NONE) {
            SSLfatal(s, SSL_AD_UNEXPECTED_MESSAGE, SSL_R_UNEXPECTED_MESSAGE);
            goto err;
        }
        s->hello_retry_request = SSL_HRR_PENDING;
        /* Tell the record layer that we know we're going to get TLSv1.3 */
        if (!ssl_set_record_protocol_version(s, s->version)) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            goto err;
        }
        hrr = 1;
        if (!PACKET_forward(pkt, SSL3_RANDOM_SIZE)) {
            SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_MISMATCH);
            goto err;
        }
    } else {
        if (!PACKET_copy_bytes(pkt, s->s3.server_random, SSL3_RANDOM_SIZE)) {
            SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_MISMATCH);
            goto err;
        }
    }

    /* Get the session-id. */
    if (!PACKET_get_length_prefixed_1(pkt, &session_id)) {
        SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_MISMATCH);
        goto err;
    }
    session_id_len = PACKET_remaining(&session_id);
    if (session_id_len > sizeof(s->session->session_id)
        || session_id_len > SSL3_SESSION_ID_SIZE) {
        SSLfatal(s, SSL_AD_ILLEGAL_PARAMETER, SSL_R_SSL3_SESSION_ID_TOO_LONG);
        goto err;
    }

    if (!PACKET_get_bytes(pkt, &cipherchars, TLS_CIPHER_LEN)) {
        SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_MISMATCH);
        goto err;
    }

    if (!PACKET_get_1(pkt, &compression)) {
        SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_MISMATCH);
        goto err;
    }

    /* TLS extensions */
    if (PACKET_remaining(pkt) == 0 && !hrr) {
        PACKET_null_init(&extpkt);
    } else if (!PACKET_as_length_prefixed_2(pkt, &extpkt)
               || PACKET_remaining(pkt) != 0) {
        SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_BAD_LENGTH);
        goto err;
    }

    if (!hrr) {
        if (!tls_collect_extensions(s, &extpkt,
                                    SSL_EXT_TLS1_2_SERVER_HELLO
                                    | SSL_EXT_TLS1_3_SERVER_HELLO,
                                    &extensions, NULL, 1)) {
            /* SSLfatal() already called */
            goto err;
        }

        if (!ssl_choose_client_version(s, sversion, extensions)) {
            /* SSLfatal() already called */
            goto err;
        }
    }

    if (SSL_CONNECTION_IS_TLS13(s) || hrr) {
        if (compression != 0) {
            SSLfatal(s, SSL_AD_ILLEGAL_PARAMETER,
                     SSL_R_INVALID_COMPRESSION_ALGORITHM);
            goto err;
        }

        if (session_id_len != s->tmp_session_id_len
                || memcmp(PACKET_data(&session_id), s->tmp_session_id,
                          session_id_len) != 0) {
            SSLfatal(s, SSL_AD_ILLEGAL_PARAMETER, SSL_R_INVALID_SESSION_ID);
            goto err;
        }
    }

    if (hrr) {
        if (!set_client_ciphersuite(s, cipherchars)) {
            /* SSLfatal() already called */
            goto err;
        }

        return tls_process_as_hello_retry_request(s, &extpkt);
    }

    /*
     * Now we have chosen the version we need to check again that the extensions
     * are appropriate for this version.
     */
    context = SSL_CONNECTION_IS_TLS13(s) ? SSL_EXT_TLS1_3_SERVER_HELLO
                                         : SSL_EXT_TLS1_2_SERVER_HELLO;
    if (!tls_validate_all_contexts(s, context, extensions)) {
        SSLfatal(s, SSL_AD_ILLEGAL_PARAMETER, SSL_R_BAD_EXTENSION);
        goto err;
    }

    s->hit = 0;

    if (SSL_CONNECTION_IS_TLS13(s)) {
        /*
         * In TLSv1.3 a ServerHello message signals a key change so the end of
         * the message must be on a record boundary.
         */
        if (RECORD_LAYER_processed_read_pending(&s->rlayer)) {
            SSLfatal(s, SSL_AD_UNEXPECTED_MESSAGE,
                     SSL_R_NOT_ON_RECORD_BOUNDARY);
            goto err;
        }

        /* This will set s->hit if we are resuming */
        if (!tls_parse_extension(s, TLSEXT_IDX_psk,
                                 SSL_EXT_TLS1_3_SERVER_HELLO,
                                 extensions, NULL, 0)) {
            /* SSLfatal() already called */
            goto err;
        }
    } else {
        /*
         * Check if we can resume the session based on external pre-shared
         * secret. EAP-FAST (RFC 4851) supports two types of session resumption.
         * Resumption based on server-side state works with session IDs.
         * Resumption based on pre-shared Protected Access Credentials (PACs)
         * works by overriding the SessionTicket extension at the application
         * layer, and does not send a session ID. (We do not know whether
         * EAP-FAST servers would honour the session ID.) Therefore, the session
         * ID alone is not a reliable indicator of session resumption, so we
         * first check if we can resume, and later peek at the next handshake
         * message to see if the server wants to resume.
         */
        if (s->version >= TLS1_VERSION
                && s->ext.session_secret_cb != NULL && s->session->ext.tick) {
            const SSL_CIPHER *pref_cipher = NULL;
            /*
             * s->session->master_key_length is a size_t, but this is an int for
             * backwards compat reasons
             */
            int master_key_length;

            master_key_length = sizeof(s->session->master_key);
            if (s->ext.session_secret_cb(ussl, s->session->master_key,
                                         &master_key_length,
                                         NULL, &pref_cipher,
                                         s->ext.session_secret_cb_arg)
                     && master_key_length > 0) {
                s->session->master_key_length = master_key_length;
                s->session->cipher = pref_cipher ?
                    pref_cipher : ssl_get_cipher_by_char(s, cipherchars, 0);
            } else {
                SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
                goto err;
            }
        }

        if (session_id_len != 0
                && session_id_len == s->session->session_id_length
                && memcmp(PACKET_data(&session_id), s->session->session_id,
                          session_id_len) == 0)
            s->hit = 1;
    }

    if (s->hit) {
        if (s->sid_ctx_length != s->session->sid_ctx_length
                || memcmp(s->session->sid_ctx, s->sid_ctx, s->sid_ctx_length)) {
            /* actually a client application bug */
            SSLfatal(s, SSL_AD_ILLEGAL_PARAMETER,
                     SSL_R_ATTEMPT_TO_REUSE_SESSION_IN_DIFFERENT_CONTEXT);
            goto err;
        }
    } else {
        /*
         * If we were trying for session-id reuse but the server
         * didn't resume, make a new SSL_SESSION.
         * In the case of EAP-FAST and PAC, we do not send a session ID,
         * so the PAC-based session secret is always preserved. It'll be
         * overwritten if the server refuses resumption.
         */
        if (s->session->session_id_length > 0) {
            ssl_tsan_counter(s->session_ctx, &s->session_ctx->stats.sess_miss);
            if (!ssl_get_new_session(s, 0)) {
                /* SSLfatal() already called */
                goto err;
            }
        }

        s->session->ssl_version = s->version;
        /*
         * In TLSv1.2 and below we save the session id we were sent so we can
         * resume it later. In TLSv1.3 the session id we were sent is just an
         * echo of what we originally sent in the ClientHello and should not be
         * used for resumption.
         */
        if (!SSL_CONNECTION_IS_TLS13(s)) {
            s->session->session_id_length = session_id_len;
            /* session_id_len could be 0 */
            if (session_id_len > 0)
                memcpy(s->session->session_id, PACKET_data(&session_id),
                       session_id_len);
        }
    }

    /* Session version and negotiated protocol version should match */
    if (s->version != s->session->ssl_version) {
        SSLfatal(s, SSL_AD_PROTOCOL_VERSION,
                 SSL_R_SSL_SESSION_VERSION_MISMATCH);
        goto err;
    }
    /*
     * Now that we know the version, update the check to see if it's an allowed
     * version.
     */
    s->s3.tmp.min_ver = s->version;
    s->s3.tmp.max_ver = s->version;

    if (!set_client_ciphersuite(s, cipherchars)) {
        /* SSLfatal() already called */
        goto err;
    }

#ifdef OPENSSL_NO_COMP
    if (compression != 0) {
        SSLfatal(s, SSL_AD_ILLEGAL_PARAMETER,
                 SSL_R_UNSUPPORTED_COMPRESSION_ALGORITHM);
        goto err;
    }
    /*
     * If compression is disabled we'd better not try to resume a session
     * using compression.
     */
    if (s->session->compress_meth != 0) {
        SSLfatal(s, SSL_AD_HANDSHAKE_FAILURE, SSL_R_INCONSISTENT_COMPRESSION);
        goto err;
    }
#else
    if (s->hit && compression != s->session->compress_meth) {
        SSLfatal(s, SSL_AD_ILLEGAL_PARAMETER,
                 SSL_R_OLD_SESSION_COMPRESSION_ALGORITHM_NOT_RETURNED);
        goto err;
    }
    if (compression == 0)
        comp = NULL;
    else if (!ssl_allow_compression(s)) {
        SSLfatal(s, SSL_AD_ILLEGAL_PARAMETER, SSL_R_COMPRESSION_DISABLED);
        goto err;
    } else {
        comp = ssl3_comp_find(SSL_CONNECTION_GET_CTX(s)->comp_methods,
                              compression);
    }

    if (compression != 0 && comp == NULL) {
        SSLfatal(s, SSL_AD_ILLEGAL_PARAMETER,
                 SSL_R_UNSUPPORTED_COMPRESSION_ALGORITHM);
        goto err;
    } else {
        s->s3.tmp.new_compression = comp;
    }
#endif

    if (!tls_parse_all_extensions(s, context, extensions, NULL, 0, 1)) {
        /* SSLfatal() already called */
        goto err;
    }

#ifndef OPENSSL_NO_SCTP
    if (SSL_CONNECTION_IS_DTLS(s) && s->hit) {
        unsigned char sctpauthkey[64];
        char labelbuffer[sizeof(DTLS1_SCTP_AUTH_LABEL)];
        size_t labellen;

        /*
         * Add new shared key for SCTP-Auth, will be ignored if
         * no SCTP used.
         */
        memcpy(labelbuffer, DTLS1_SCTP_AUTH_LABEL,
               sizeof(DTLS1_SCTP_AUTH_LABEL));

        /* Don't include the terminating zero. */
        labellen = sizeof(labelbuffer) - 1;
        if (s->mode & SSL_MODE_DTLS_SCTP_LABEL_LENGTH_BUG)
            labellen += 1;

        if (SSL_export_keying_material(ssl, sctpauthkey,
                                       sizeof(sctpauthkey),
                                       labelbuffer,
                                       labellen, NULL, 0, 0) <= 0) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            goto err;
        }

        BIO_ctrl(SSL_get_wbio(ssl),
                 BIO_CTRL_DGRAM_SCTP_ADD_AUTH_KEY,
                 sizeof(sctpauthkey), sctpauthkey);
    }
#endif

    /*
     * In TLSv1.3 we have some post-processing to change cipher state, otherwise
     * we're done with this message
     */
    if (SSL_CONNECTION_IS_TLS13(s)) {
        if (!ssl->method->ssl3_enc->setup_key_block(s)
                || !tls13_store_handshake_traffic_hash(s)) {
            /* SSLfatal() already called */
            goto err;
        }
        /*
         * If we're not doing early-data and we're not going to send a dummy CCS
         * (i.e. no middlebox compat mode) then we can change the write keys
         * immediately. Otherwise we have to defer this until after all possible
         * early data is written. We could just always defer until the last
         * moment except QUIC needs it done at the same time as the read keys
         * are changed. Since QUIC doesn't do TLS early data or need middlebox
         * compat this doesn't cause a problem.
         */
        if (SSL_IS_QUIC_HANDSHAKE(s)
                || (s->early_data_state == SSL_EARLY_DATA_NONE
                    && (s->options & SSL_OP_ENABLE_MIDDLEBOX_COMPAT) == 0)) {
            if (!ssl->method->ssl3_enc->change_cipher_state(s,
                    SSL3_CC_HANDSHAKE | SSL3_CHANGE_CIPHER_CLIENT_WRITE)) {
                /* SSLfatal() already called */
                goto err;
                    }
        }
        if (!ssl->method->ssl3_enc->change_cipher_state(s,
                    SSL3_CC_HANDSHAKE | SSL3_CHANGE_CIPHER_CLIENT_READ)) {
            /* SSLfatal() already called */
            goto err;
        }
    }

    OPENSSL_free(extensions);
    return MSG_PROCESS_CONTINUE_READING;
 err:
    OPENSSL_free(extensions);
    return MSG_PROCESS_ERROR;
}

static MSG_PROCESS_RETURN tls_process_as_hello_retry_request(SSL_CONNECTION *s,
                                                             PACKET *extpkt)
{
    RAW_EXTENSION *extensions = NULL;

    /*
     * If we were sending early_data then any alerts should not be sent using
     * the old wrlmethod.
     */
    if (s->early_data_state == SSL_EARLY_DATA_FINISHED_WRITING
            && !ssl_set_new_record_layer(s,
                                         TLS_ANY_VERSION,
                                         OSSL_RECORD_DIRECTION_WRITE,
                                         OSSL_RECORD_PROTECTION_LEVEL_NONE,
                                         NULL, 0, NULL, 0, NULL, 0, NULL,  0,
                                         NULL, 0, NID_undef, NULL, NULL, NULL)) {
        /* SSLfatal already called */
        goto err;
    }
    /* We are definitely going to be using TLSv1.3 */
    s->rlayer.wrlmethod->set_protocol_version(s->rlayer.wrl, TLS1_3_VERSION);

    if (!tls_collect_extensions(s, extpkt, SSL_EXT_TLS1_3_HELLO_RETRY_REQUEST,
                                &extensions, NULL, 1)
            || !tls_parse_all_extensions(s, SSL_EXT_TLS1_3_HELLO_RETRY_REQUEST,
                                         extensions, NULL, 0, 1)) {
        /* SSLfatal() already called */
        goto err;
    }

    OPENSSL_free(extensions);
    extensions = NULL;

    if (s->ext.tls13_cookie_len == 0 && s->s3.tmp.pkey != NULL) {
        /*
         * We didn't receive a cookie or a new key_share so the next
         * ClientHello will not change
         */
        SSLfatal(s, SSL_AD_ILLEGAL_PARAMETER, SSL_R_NO_CHANGE_FOLLOWING_HRR);
        goto err;
    }

    /*
     * Re-initialise the Transcript Hash. We're going to prepopulate it with
     * a synthetic message_hash in place of ClientHello1.
     */
    if (!create_synthetic_message_hash(s, NULL, 0, NULL, 0)) {
        /* SSLfatal() already called */
        goto err;
    }

    /*
     * Add this message to the Transcript Hash. Normally this is done
     * automatically prior to the message processing stage. However due to the
     * need to create the synthetic message hash, we defer that step until now
     * for HRR messages.
     */
    if (!ssl3_finish_mac(s, (unsigned char *)s->init_buf->data,
                                s->init_num + SSL3_HM_HEADER_LENGTH)) {
        /* SSLfatal() already called */
        goto err;
    }

    return MSG_PROCESS_FINISHED_READING;
 err:
    OPENSSL_free(extensions);
    return MSG_PROCESS_ERROR;
}

MSG_PROCESS_RETURN tls_process_server_rpk(SSL_CONNECTION *sc, PACKET *pkt)
{
    EVP_PKEY *peer_rpk = NULL;

    if (!tls_process_rpk(sc, pkt, &peer_rpk)) {
        /* SSLfatal() already called */
        return MSG_PROCESS_ERROR;
    }

    if (peer_rpk == NULL) {
        SSLfatal(sc, SSL_AD_DECODE_ERROR, SSL_R_BAD_CERTIFICATE);
        return MSG_PROCESS_ERROR;
    }

    EVP_PKEY_free(sc->session->peer_rpk);
    sc->session->peer_rpk = peer_rpk;

    return MSG_PROCESS_CONTINUE_PROCESSING;
}

static WORK_STATE tls_post_process_server_rpk(SSL_CONNECTION *sc,
                                              WORK_STATE wst)
{
    size_t certidx;
    const SSL_CERT_LOOKUP *clu;
    int v_ok;

    if (sc->session->peer_rpk == NULL) {
        SSLfatal(sc, SSL_AD_ILLEGAL_PARAMETER,
                 SSL_R_INVALID_RAW_PUBLIC_KEY);
        return WORK_ERROR;
    }

    if (sc->rwstate == SSL_RETRY_VERIFY)
        sc->rwstate = SSL_NOTHING;

    ERR_set_mark();
    v_ok = ssl_verify_rpk(sc, sc->session->peer_rpk);
    if (v_ok <= 0 && sc->verify_mode != SSL_VERIFY_NONE) {
        ERR_clear_last_mark();
        SSLfatal(sc, ssl_x509err2alert(sc->verify_result),
                 SSL_R_CERTIFICATE_VERIFY_FAILED);
        return WORK_ERROR;
    }
    ERR_pop_to_mark();      /* but we keep s->verify_result */
    if (v_ok > 0 && sc->rwstate == SSL_RETRY_VERIFY) {
        return WORK_MORE_A;
    }

    if ((clu = ssl_cert_lookup_by_pkey(sc->session->peer_rpk, &certidx,
                                       SSL_CONNECTION_GET_CTX(sc))) == NULL) {
        SSLfatal(sc, SSL_AD_ILLEGAL_PARAMETER, SSL_R_UNKNOWN_CERTIFICATE_TYPE);
        return WORK_ERROR;
    }

    /*
     * Check certificate type is consistent with ciphersuite. For TLS 1.3
     * skip check since TLS 1.3 ciphersuites can be used with any certificate
     * type.
     */
    if (!SSL_CONNECTION_IS_TLS13(sc)) {
        if ((clu->amask & sc->s3.tmp.new_cipher->algorithm_auth) == 0) {
            SSLfatal(sc, SSL_AD_ILLEGAL_PARAMETER, SSL_R_WRONG_RPK_TYPE);
            return WORK_ERROR;
        }
    }

    /* Ensure there is no peer/peer_chain */
    X509_free(sc->session->peer);
    sc->session->peer = NULL;
    sk_X509_pop_free(sc->session->peer_chain, X509_free);
    sc->session->peer_chain = NULL;
    sc->session->verify_result = sc->verify_result;

    /* Save the current hash state for when we receive the CertificateVerify */
    if (SSL_CONNECTION_IS_TLS13(sc)
            && !ssl_handshake_hash(sc, sc->cert_verify_hash,
                                   sizeof(sc->cert_verify_hash),
                                   &sc->cert_verify_hash_len)) {
        /* SSLfatal() already called */
        return WORK_ERROR;
    }

    return WORK_FINISHED_CONTINUE;
}

/* prepare server cert verification by setting s->session->peer_chain from pkt */
MSG_PROCESS_RETURN tls_process_server_certificate(SSL_CONNECTION *s,
                                                  PACKET *pkt)
{
    unsigned long cert_list_len, cert_len;
    X509 *x = NULL;
    const unsigned char *certstart, *certbytes;
    size_t chainidx;
    unsigned int context = 0;
    SSL_CTX *sctx = SSL_CONNECTION_GET_CTX(s);

    if (s->ext.server_cert_type == TLSEXT_cert_type_rpk)
        return tls_process_server_rpk(s, pkt);
    if (s->ext.server_cert_type != TLSEXT_cert_type_x509) {
        SSLfatal(s, SSL_AD_UNSUPPORTED_CERTIFICATE,
                 SSL_R_UNKNOWN_CERTIFICATE_TYPE);
        goto err;
    }

    if ((s->session->peer_chain = sk_X509_new_null()) == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_CRYPTO_LIB);
        goto err;
    }

    if ((SSL_CONNECTION_IS_TLS13(s) && !PACKET_get_1(pkt, &context))
            || context != 0
            || !PACKET_get_net_3(pkt, &cert_list_len)
            || PACKET_remaining(pkt) != cert_list_len
            || PACKET_remaining(pkt) == 0) {
        SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_MISMATCH);
        goto err;
    }
    for (chainidx = 0; PACKET_remaining(pkt); chainidx++) {
        if (!PACKET_get_net_3(pkt, &cert_len)
            || !PACKET_get_bytes(pkt, &certbytes, cert_len)) {
            SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_CERT_LENGTH_MISMATCH);
            goto err;
        }

        certstart = certbytes;
        x = X509_new_ex(sctx->libctx, sctx->propq);
        if (x == NULL) {
            SSLfatal(s, SSL_AD_DECODE_ERROR, ERR_R_ASN1_LIB);
            goto err;
        }
        if (d2i_X509(&x, (const unsigned char **)&certbytes,
                     cert_len) == NULL) {
            SSLfatal(s, SSL_AD_BAD_CERTIFICATE, ERR_R_ASN1_LIB);
            goto err;
        }

        if (certbytes != (certstart + cert_len)) {
            SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_CERT_LENGTH_MISMATCH);
            goto err;
        }

        if (SSL_CONNECTION_IS_TLS13(s)) {
            RAW_EXTENSION *rawexts = NULL;
            PACKET extensions;

            if (!PACKET_get_length_prefixed_2(pkt, &extensions)) {
                SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_BAD_LENGTH);
                goto err;
            }
            if (!tls_collect_extensions(s, &extensions,
                                        SSL_EXT_TLS1_3_CERTIFICATE, &rawexts,
                                        NULL, chainidx == 0)
                || !tls_parse_all_extensions(s, SSL_EXT_TLS1_3_CERTIFICATE,
                                             rawexts, x, chainidx,
                                             PACKET_remaining(pkt) == 0)) {
                OPENSSL_free(rawexts);
                /* SSLfatal already called */
                goto err;
            }
            OPENSSL_free(rawexts);
        }

        if (!sk_X509_push(s->session->peer_chain, x)) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_CRYPTO_LIB);
            goto err;
        }
        x = NULL;
    }
    return MSG_PROCESS_CONTINUE_PROCESSING;

 err:
    X509_free(x);
    OSSL_STACK_OF_X509_free(s->session->peer_chain);
    s->session->peer_chain = NULL;
    return MSG_PROCESS_ERROR;
}

/*
 * Verify the s->session->peer_chain and check server cert type.
 * On success set s->session->peer and s->session->verify_result.
 * Else the peer certificate verification callback may request retry.
 */
WORK_STATE tls_post_process_server_certificate(SSL_CONNECTION *s,
                                               WORK_STATE wst)
{
    X509 *x;
    EVP_PKEY *pkey = NULL;
    const SSL_CERT_LOOKUP *clu;
    size_t certidx;
    int i;

    if (s->ext.server_cert_type == TLSEXT_cert_type_rpk)
        return tls_post_process_server_rpk(s, wst);

    if (s->rwstate == SSL_RETRY_VERIFY)
        s->rwstate = SSL_NOTHING;

    /*
     * The documented interface is that SSL_VERIFY_PEER should be set in order
     * for client side verification of the server certificate to take place.
     * However, historically the code has only checked that *any* flag is set
     * to cause server verification to take place. Use of the other flags makes
     * no sense in client mode. An attempt to clean up the semantics was
     * reverted because at least one application *only* set
     * SSL_VERIFY_FAIL_IF_NO_PEER_CERT. Prior to the clean up this still caused
     * server verification to take place, after the clean up it silently did
     * nothing. SSL_CTX_set_verify()/SSL_set_verify() cannot validate the flags
     * sent to them because they are void functions. Therefore, we now use the
     * (less clean) historic behaviour of performing validation if any flag is
     * set. The *documented* interface remains the same.
     */
    ERR_set_mark();
    i = ssl_verify_cert_chain(s, s->session->peer_chain);
    if (i <= 0 && s->verify_mode != SSL_VERIFY_NONE) {
        ERR_clear_last_mark();
        SSLfatal(s, ssl_x509err2alert(s->verify_result),
                 SSL_R_CERTIFICATE_VERIFY_FAILED);
        return WORK_ERROR;
    }
    ERR_pop_to_mark();      /* but we keep s->verify_result */
    if (i > 0 && s->rwstate == SSL_RETRY_VERIFY)
        return WORK_MORE_A;

    /*
     * Inconsistency alert: cert_chain does include the peer's certificate,
     * which we don't include in statem_srvr.c
     */
    x = sk_X509_value(s->session->peer_chain, 0);

    pkey = X509_get0_pubkey(x);

    if (pkey == NULL || EVP_PKEY_missing_parameters(pkey)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR,
                 SSL_R_UNABLE_TO_FIND_PUBLIC_KEY_PARAMETERS);
        return WORK_ERROR;
    }

    if ((clu = ssl_cert_lookup_by_pkey(pkey, &certidx,
				       SSL_CONNECTION_GET_CTX(s))) == NULL) {
        SSLfatal(s, SSL_AD_ILLEGAL_PARAMETER, SSL_R_UNKNOWN_CERTIFICATE_TYPE);
        return WORK_ERROR;
    }
    /*
     * Check certificate type is consistent with ciphersuite. For TLS 1.3
     * skip check since TLS 1.3 ciphersuites can be used with any certificate
     * type.
     */
    if (!SSL_CONNECTION_IS_TLS13(s)) {
        if ((clu->amask & s->s3.tmp.new_cipher->algorithm_auth) == 0) {
            SSLfatal(s, SSL_AD_ILLEGAL_PARAMETER, SSL_R_WRONG_CERTIFICATE_TYPE);
            return WORK_ERROR;
        }
    }

    if (!X509_up_ref(x)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return WORK_ERROR;
    }

    X509_free(s->session->peer);
    s->session->peer = x;
    s->session->verify_result = s->verify_result;
    /* Ensure there is no RPK */
    EVP_PKEY_free(s->session->peer_rpk);
    s->session->peer_rpk = NULL;

    /* Save the current hash state for when we receive the CertificateVerify */
    if (SSL_CONNECTION_IS_TLS13(s)
            && !ssl_handshake_hash(s, s->cert_verify_hash,
                                   sizeof(s->cert_verify_hash),
                                   &s->cert_verify_hash_len)) {
        /* SSLfatal() already called */;
        return WORK_ERROR;
    }
    return WORK_FINISHED_CONTINUE;
}

#ifndef OPENSSL_NO_COMP_ALG
MSG_PROCESS_RETURN tls_process_server_compressed_certificate(SSL_CONNECTION *sc, PACKET *pkt)
{
    MSG_PROCESS_RETURN ret = MSG_PROCESS_ERROR;
    PACKET tmppkt;
    BUF_MEM *buf = BUF_MEM_new();

    if (tls13_process_compressed_certificate(sc, pkt, &tmppkt, buf) != MSG_PROCESS_ERROR)
        ret = tls_process_server_certificate(sc, &tmppkt);

    BUF_MEM_free(buf);
    return ret;
}
#endif

static int tls_process_ske_psk_preamble(SSL_CONNECTION *s, PACKET *pkt)
{
#ifndef OPENSSL_NO_PSK
    PACKET psk_identity_hint;

    /* PSK ciphersuites are preceded by an identity hint */

    if (!PACKET_get_length_prefixed_2(pkt, &psk_identity_hint)) {
        SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_MISMATCH);
        return 0;
    }

    /*
     * Store PSK identity hint for later use, hint is used in
     * tls_construct_client_key_exchange.  Assume that the maximum length of
     * a PSK identity hint can be as long as the maximum length of a PSK
     * identity.
     */
    if (PACKET_remaining(&psk_identity_hint) > PSK_MAX_IDENTITY_LEN) {
        SSLfatal(s, SSL_AD_HANDSHAKE_FAILURE, SSL_R_DATA_LENGTH_TOO_LONG);
        return 0;
    }

    if (PACKET_remaining(&psk_identity_hint) == 0) {
        OPENSSL_free(s->session->psk_identity_hint);
        s->session->psk_identity_hint = NULL;
    } else if (!PACKET_strndup(&psk_identity_hint,
                               &s->session->psk_identity_hint)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    return 1;
#else
    SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
    return 0;
#endif
}

static int tls_process_ske_srp(SSL_CONNECTION *s, PACKET *pkt, EVP_PKEY **pkey)
{
#ifndef OPENSSL_NO_SRP
    PACKET prime, generator, salt, server_pub;

    if (!PACKET_get_length_prefixed_2(pkt, &prime)
        || !PACKET_get_length_prefixed_2(pkt, &generator)
        || !PACKET_get_length_prefixed_1(pkt, &salt)
        || !PACKET_get_length_prefixed_2(pkt, &server_pub)) {
        SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_MISMATCH);
        return 0;
    }

    if ((s->srp_ctx.N =
         BN_bin2bn(PACKET_data(&prime),
                   (int)PACKET_remaining(&prime), NULL)) == NULL
        || (s->srp_ctx.g =
            BN_bin2bn(PACKET_data(&generator),
                      (int)PACKET_remaining(&generator), NULL)) == NULL
        || (s->srp_ctx.s =
            BN_bin2bn(PACKET_data(&salt),
                      (int)PACKET_remaining(&salt), NULL)) == NULL
        || (s->srp_ctx.B =
            BN_bin2bn(PACKET_data(&server_pub),
                      (int)PACKET_remaining(&server_pub), NULL)) == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_BN_LIB);
        return 0;
    }

    if (!srp_verify_server_param(s)) {
        /* SSLfatal() already called */
        return 0;
    }

    /* We must check if there is a certificate */
    if (s->s3.tmp.new_cipher->algorithm_auth & (SSL_aRSA | SSL_aDSS))
        *pkey = tls_get_peer_pkey(s);

    return 1;
#else
    SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
    return 0;
#endif
}

static int tls_process_ske_dhe(SSL_CONNECTION *s, PACKET *pkt, EVP_PKEY **pkey)
{
    PACKET prime, generator, pub_key;
    EVP_PKEY *peer_tmp = NULL;
    BIGNUM *p = NULL, *g = NULL, *bnpub_key = NULL;
    EVP_PKEY_CTX *pctx = NULL;
    OSSL_PARAM *params = NULL;
    OSSL_PARAM_BLD *tmpl = NULL;
    SSL_CTX *sctx = SSL_CONNECTION_GET_CTX(s);
    int ret = 0;

    if (!PACKET_get_length_prefixed_2(pkt, &prime)
        || !PACKET_get_length_prefixed_2(pkt, &generator)
        || !PACKET_get_length_prefixed_2(pkt, &pub_key)) {
        SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_MISMATCH);
        return 0;
    }

    p = BN_bin2bn(PACKET_data(&prime), (int)PACKET_remaining(&prime), NULL);
    g = BN_bin2bn(PACKET_data(&generator), (int)PACKET_remaining(&generator),
                  NULL);
    bnpub_key = BN_bin2bn(PACKET_data(&pub_key),
                          (int)PACKET_remaining(&pub_key), NULL);
    if (p == NULL || g == NULL || bnpub_key == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_BN_LIB);
        goto err;
    }

    tmpl = OSSL_PARAM_BLD_new();
    if (tmpl == NULL
            || !OSSL_PARAM_BLD_push_BN(tmpl, OSSL_PKEY_PARAM_FFC_P, p)
            || !OSSL_PARAM_BLD_push_BN(tmpl, OSSL_PKEY_PARAM_FFC_G, g)
            || !OSSL_PARAM_BLD_push_BN(tmpl, OSSL_PKEY_PARAM_PUB_KEY,
                                       bnpub_key)
            || (params = OSSL_PARAM_BLD_to_param(tmpl)) == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }

    pctx = EVP_PKEY_CTX_new_from_name(sctx->libctx, "DH", sctx->propq);
    if (pctx == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    if (EVP_PKEY_fromdata_init(pctx) <= 0
            || EVP_PKEY_fromdata(pctx, &peer_tmp, EVP_PKEY_KEYPAIR, params) <= 0) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, SSL_R_BAD_DH_VALUE);
        goto err;
    }

    EVP_PKEY_CTX_free(pctx);
    pctx = EVP_PKEY_CTX_new_from_pkey(sctx->libctx, peer_tmp, sctx->propq);
    if (pctx == NULL
            /*
             * EVP_PKEY_param_check() will verify that the DH params are using
             * a safe prime. In this context, because we're using ephemeral DH,
             * we're ok with it not being a safe prime.
             * EVP_PKEY_param_check_quick() skips the safe prime check.
             */
            || EVP_PKEY_param_check_quick(pctx) != 1
            || EVP_PKEY_public_check(pctx) != 1) {
        SSLfatal(s, SSL_AD_ILLEGAL_PARAMETER, SSL_R_BAD_DH_VALUE);
        goto err;
    }

    if (!ssl_security(s, SSL_SECOP_TMP_DH,
                      EVP_PKEY_get_security_bits(peer_tmp),
                      0, peer_tmp)) {
        SSLfatal(s, SSL_AD_HANDSHAKE_FAILURE, SSL_R_DH_KEY_TOO_SMALL);
        goto err;
    }

    s->s3.peer_tmp = peer_tmp;
    peer_tmp = NULL;

    /*
     * FIXME: This makes assumptions about which ciphersuites come with
     * public keys. We should have a less ad-hoc way of doing this
     */
    if (s->s3.tmp.new_cipher->algorithm_auth & (SSL_aRSA | SSL_aDSS))
        *pkey = tls_get_peer_pkey(s);
    /* else anonymous DH, so no certificate or pkey. */

    ret = 1;

 err:
    OSSL_PARAM_BLD_free(tmpl);
    OSSL_PARAM_free(params);
    EVP_PKEY_free(peer_tmp);
    EVP_PKEY_CTX_free(pctx);
    BN_free(p);
    BN_free(g);
    BN_free(bnpub_key);

    return ret;
}

static int tls_process_ske_ecdhe(SSL_CONNECTION *s, PACKET *pkt, EVP_PKEY **pkey)
{
    PACKET encoded_pt;
    unsigned int curve_type, curve_id;

    /*
     * Extract elliptic curve parameters and the server's ephemeral ECDH
     * public key. We only support named (not generic) curves and
     * ECParameters in this case is just three bytes.
     */
    if (!PACKET_get_1(pkt, &curve_type) || !PACKET_get_net_2(pkt, &curve_id)) {
        SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_TOO_SHORT);
        return 0;
    }
    /*
     * Check curve is named curve type and one of our preferences, if not
     * server has sent an invalid curve.
     */
    if (curve_type != NAMED_CURVE_TYPE
            || !tls1_check_group_id(s, curve_id, 1)) {
        SSLfatal(s, SSL_AD_ILLEGAL_PARAMETER, SSL_R_WRONG_CURVE);
        return 0;
    }

    if ((s->s3.peer_tmp = ssl_generate_param_group(s, curve_id)) == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR,
                 SSL_R_UNABLE_TO_FIND_ECDH_PARAMETERS);
        return 0;
    }

    if (!PACKET_get_length_prefixed_1(pkt, &encoded_pt)) {
        SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_MISMATCH);
        return 0;
    }

    if (EVP_PKEY_set1_encoded_public_key(s->s3.peer_tmp,
                                         PACKET_data(&encoded_pt),
                                         PACKET_remaining(&encoded_pt)) <= 0) {
        SSLfatal(s, SSL_AD_ILLEGAL_PARAMETER, SSL_R_BAD_ECPOINT);
        return 0;
    }

    /*
     * The ECC/TLS specification does not mention the use of DSA to sign
     * ECParameters in the server key exchange message. We do support RSA
     * and ECDSA.
     */
    if (s->s3.tmp.new_cipher->algorithm_auth & SSL_aECDSA)
        *pkey = tls_get_peer_pkey(s);
    else if (s->s3.tmp.new_cipher->algorithm_auth & SSL_aRSA)
        *pkey = tls_get_peer_pkey(s);
    /* else anonymous ECDH, so no certificate or pkey. */

    /* Cache the agreed upon group in the SSL_SESSION */
    s->session->kex_group = curve_id;
    return 1;
}

MSG_PROCESS_RETURN tls_process_key_exchange(SSL_CONNECTION *s, PACKET *pkt)
{
    long alg_k;
    EVP_PKEY *pkey = NULL;
    EVP_MD_CTX *md_ctx = NULL;
    EVP_PKEY_CTX *pctx = NULL;
    PACKET save_param_start, signature;
    SSL_CTX *sctx = SSL_CONNECTION_GET_CTX(s);

    alg_k = s->s3.tmp.new_cipher->algorithm_mkey;

    save_param_start = *pkt;

    EVP_PKEY_free(s->s3.peer_tmp);
    s->s3.peer_tmp = NULL;

    if (alg_k & SSL_PSK) {
        if (!tls_process_ske_psk_preamble(s, pkt)) {
            /* SSLfatal() already called */
            goto err;
        }
    }

    /* Nothing else to do for plain PSK or RSAPSK */
    if (alg_k & (SSL_kPSK | SSL_kRSAPSK)) {
    } else if (alg_k & SSL_kSRP) {
        if (!tls_process_ske_srp(s, pkt, &pkey)) {
            /* SSLfatal() already called */
            goto err;
        }
    } else if (alg_k & (SSL_kDHE | SSL_kDHEPSK)) {
        if (!tls_process_ske_dhe(s, pkt, &pkey)) {
            /* SSLfatal() already called */
            goto err;
        }
    } else if (alg_k & (SSL_kECDHE | SSL_kECDHEPSK)) {
        if (!tls_process_ske_ecdhe(s, pkt, &pkey)) {
            /* SSLfatal() already called */
            goto err;
        }
    } else if (alg_k) {
        SSLfatal(s, SSL_AD_UNEXPECTED_MESSAGE, SSL_R_UNEXPECTED_MESSAGE);
        goto err;
    }

    /* if it was signed, check the signature */
    if (pkey != NULL) {
        PACKET params;
        const EVP_MD *md = NULL;
        unsigned char *tbs;
        size_t tbslen;
        int rv;

        /*
         * |pkt| now points to the beginning of the signature, so the difference
         * equals the length of the parameters.
         */
        if (!PACKET_get_sub_packet(&save_param_start, &params,
                                   PACKET_remaining(&save_param_start) -
                                   PACKET_remaining(pkt))) {
            SSLfatal(s, SSL_AD_DECODE_ERROR, ERR_R_INTERNAL_ERROR);
            goto err;
        }

        if (SSL_USE_SIGALGS(s)) {
            unsigned int sigalg;

            if (!PACKET_get_net_2(pkt, &sigalg)) {
                SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_TOO_SHORT);
                goto err;
            }
            if (tls12_check_peer_sigalg(s, sigalg, pkey) <=0) {
                /* SSLfatal() already called */
                goto err;
            }
        } else if (!tls1_set_peer_legacy_sigalg(s, pkey)) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR,
                     SSL_R_LEGACY_SIGALG_DISALLOWED_OR_UNSUPPORTED);
            goto err;
        }

        if (!tls1_lookup_md(sctx, s->s3.tmp.peer_sigalg, &md)) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR,
                     SSL_R_NO_SUITABLE_DIGEST_ALGORITHM);
            goto err;
        }
        if (SSL_USE_SIGALGS(s))
            OSSL_TRACE1(TLS, "USING TLSv1.2 HASH %s\n",
                        md == NULL ? "n/a" : EVP_MD_get0_name(md));

        if (!PACKET_get_length_prefixed_2(pkt, &signature)
            || PACKET_remaining(pkt) != 0) {
            SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_MISMATCH);
            goto err;
        }

        md_ctx = EVP_MD_CTX_new();
        if (md_ctx == NULL) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_EVP_LIB);
            goto err;
        }

        if (EVP_DigestVerifyInit_ex(md_ctx, &pctx,
                                    md == NULL ? NULL : EVP_MD_get0_name(md),
                                    sctx->libctx, sctx->propq, pkey,
                                    NULL) <= 0) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_EVP_LIB);
            goto err;
        }
        if (SSL_USE_PSS(s)) {
            if (EVP_PKEY_CTX_set_rsa_padding(pctx, RSA_PKCS1_PSS_PADDING) <= 0
                || EVP_PKEY_CTX_set_rsa_pss_saltlen(pctx,
                                                RSA_PSS_SALTLEN_DIGEST) <= 0) {
                SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_EVP_LIB);
                goto err;
            }
        }
        tbslen = construct_key_exchange_tbs(s, &tbs, PACKET_data(&params),
                                            PACKET_remaining(&params));
        if (tbslen == 0) {
            /* SSLfatal() already called */
            goto err;
        }

        rv = EVP_DigestVerify(md_ctx, PACKET_data(&signature),
                              PACKET_remaining(&signature), tbs, tbslen);
        OPENSSL_free(tbs);
        if (rv <= 0) {
            SSLfatal(s, SSL_AD_DECRYPT_ERROR, SSL_R_BAD_SIGNATURE);
            goto err;
        }
        EVP_MD_CTX_free(md_ctx);
        md_ctx = NULL;
    } else {
        /* aNULL, aSRP or PSK do not need public keys */
        if (!(s->s3.tmp.new_cipher->algorithm_auth & (SSL_aNULL | SSL_aSRP))
            && !(alg_k & SSL_PSK)) {
            /* Might be wrong key type, check it */
            if (ssl3_check_cert_and_algorithm(s)) {
                SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_BAD_DATA);
            }
            /* else this shouldn't happen, SSLfatal() already called */
            goto err;
        }
        /* still data left over */
        if (PACKET_remaining(pkt) != 0) {
            SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_EXTRA_DATA_IN_MESSAGE);
            goto err;
        }
    }

    return MSG_PROCESS_CONTINUE_READING;
 err:
    EVP_MD_CTX_free(md_ctx);
    return MSG_PROCESS_ERROR;
}

MSG_PROCESS_RETURN tls_process_certificate_request(SSL_CONNECTION *s,
                                                   PACKET *pkt)
{
    /* Clear certificate validity flags */
    if (s->s3.tmp.valid_flags != NULL)
        memset(s->s3.tmp.valid_flags, 0, s->ssl_pkey_num * sizeof(uint32_t));
    else
        s->s3.tmp.valid_flags = OPENSSL_zalloc(s->ssl_pkey_num * sizeof(uint32_t));

    /* Give up for good if allocation didn't work */
    if (s->s3.tmp.valid_flags == NULL)
        return 0;

    if (SSL_CONNECTION_IS_TLS13(s)) {
        PACKET reqctx, extensions;
        RAW_EXTENSION *rawexts = NULL;

        if ((s->shutdown & SSL_SENT_SHUTDOWN) != 0) {
            /*
             * We already sent close_notify. This can only happen in TLSv1.3
             * post-handshake messages. We can't reasonably respond to this, so
             * we just ignore it
             */
            return MSG_PROCESS_FINISHED_READING;
        }

        /* Free and zero certificate types: it is not present in TLS 1.3 */
        OPENSSL_free(s->s3.tmp.ctype);
        s->s3.tmp.ctype = NULL;
        s->s3.tmp.ctype_len = 0;
        OPENSSL_free(s->pha_context);
        s->pha_context = NULL;
        s->pha_context_len = 0;

        if (!PACKET_get_length_prefixed_1(pkt, &reqctx) ||
            !PACKET_memdup(&reqctx, &s->pha_context, &s->pha_context_len)) {
            SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_MISMATCH);
            return MSG_PROCESS_ERROR;
        }

        if (!PACKET_get_length_prefixed_2(pkt, &extensions)) {
            SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_BAD_LENGTH);
            return MSG_PROCESS_ERROR;
        }
        if (!tls_collect_extensions(s, &extensions,
                                    SSL_EXT_TLS1_3_CERTIFICATE_REQUEST,
                                    &rawexts, NULL, 1)
            || !tls_parse_all_extensions(s, SSL_EXT_TLS1_3_CERTIFICATE_REQUEST,
                                         rawexts, NULL, 0, 1)) {
            /* SSLfatal() already called */
            OPENSSL_free(rawexts);
            return MSG_PROCESS_ERROR;
        }
        OPENSSL_free(rawexts);
        if (!tls1_process_sigalgs(s)) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, SSL_R_BAD_LENGTH);
            return MSG_PROCESS_ERROR;
        }
    } else {
        PACKET ctypes;

        /* get the certificate types */
        if (!PACKET_get_length_prefixed_1(pkt, &ctypes)) {
            SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_MISMATCH);
            return MSG_PROCESS_ERROR;
        }

        if (!PACKET_memdup(&ctypes, &s->s3.tmp.ctype, &s->s3.tmp.ctype_len)) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            return MSG_PROCESS_ERROR;
        }

        if (SSL_USE_SIGALGS(s)) {
            PACKET sigalgs;

            if (!PACKET_get_length_prefixed_2(pkt, &sigalgs)) {
                SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_MISMATCH);
                return MSG_PROCESS_ERROR;
            }

            /*
             * Despite this being for certificates, preserve compatibility
             * with pre-TLS 1.3 and use the regular sigalgs field.
             */
            if (!tls1_save_sigalgs(s, &sigalgs, 0)) {
                SSLfatal(s, SSL_AD_INTERNAL_ERROR,
                         SSL_R_SIGNATURE_ALGORITHMS_ERROR);
                return MSG_PROCESS_ERROR;
            }
            if (!tls1_process_sigalgs(s)) {
                SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_SSL_LIB);
                return MSG_PROCESS_ERROR;
            }
        }

        /* get the CA RDNs */
        if (!parse_ca_names(s, pkt)) {
            /* SSLfatal() already called */
            return MSG_PROCESS_ERROR;
        }
    }

    if (PACKET_remaining(pkt) != 0) {
        SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_MISMATCH);
        return MSG_PROCESS_ERROR;
    }

    /* we should setup a certificate to return.... */
    s->s3.tmp.cert_req = 1;

    /*
     * In TLSv1.3 we don't prepare the client certificate yet. We wait until
     * after the CertificateVerify message has been received. This is because
     * in TLSv1.3 the CertificateRequest arrives before the Certificate message
     * but in TLSv1.2 it is the other way around. We want to make sure that
     * SSL_get1_peer_certificate() returns something sensible in
     * client_cert_cb.
     */
    if (SSL_CONNECTION_IS_TLS13(s)
        && s->post_handshake_auth != SSL_PHA_REQUESTED)
        return MSG_PROCESS_CONTINUE_READING;

    return MSG_PROCESS_CONTINUE_PROCESSING;
}

MSG_PROCESS_RETURN tls_process_new_session_ticket(SSL_CONNECTION *s,
                                                  PACKET *pkt)
{
    unsigned int ticklen;
    unsigned long ticket_lifetime_hint, age_add = 0;
    unsigned int sess_len;
    RAW_EXTENSION *exts = NULL;
    PACKET nonce;
    EVP_MD *sha256 = NULL;
    SSL_CTX *sctx = SSL_CONNECTION_GET_CTX(s);

    PACKET_null_init(&nonce);

    if (!PACKET_get_net_4(pkt, &ticket_lifetime_hint)
        || (SSL_CONNECTION_IS_TLS13(s)
            && (!PACKET_get_net_4(pkt, &age_add)
                || !PACKET_get_length_prefixed_1(pkt, &nonce)))
        || !PACKET_get_net_2(pkt, &ticklen)
        || (SSL_CONNECTION_IS_TLS13(s) ? (ticklen == 0
                                          || PACKET_remaining(pkt) < ticklen)
                                       : PACKET_remaining(pkt) != ticklen)) {
        SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_MISMATCH);
        goto err;
    }

    /*
     * Server is allowed to change its mind (in <=TLSv1.2) and send an empty
     * ticket. We already checked this TLSv1.3 case above, so it should never
     * be 0 here in that instance
     */
    if (ticklen == 0)
        return MSG_PROCESS_CONTINUE_READING;

    /*
     * Sessions must be immutable once they go into the session cache. Otherwise
     * we can get multi-thread problems. Therefore we don't "update" sessions,
     * we replace them with a duplicate. In TLSv1.3 we need to do this every
     * time a NewSessionTicket arrives because those messages arrive
     * post-handshake and the session may have already gone into the session
     * cache.
     */
    if (SSL_CONNECTION_IS_TLS13(s) || s->session->session_id_length > 0) {
        SSL_SESSION *new_sess;

        /*
         * We reused an existing session, so we need to replace it with a new
         * one
         */
        if ((new_sess = ssl_session_dup(s->session, 0)) == 0) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_SSL_LIB);
            goto err;
        }

        if ((s->session_ctx->session_cache_mode & SSL_SESS_CACHE_CLIENT) != 0
                && !SSL_CONNECTION_IS_TLS13(s)) {
            /*
             * In TLSv1.2 and below the arrival of a new tickets signals that
             * any old ticket we were using is now out of date, so we remove the
             * old session from the cache. We carry on if this fails
             */
            SSL_CTX_remove_session(s->session_ctx, s->session);
        }

        SSL_SESSION_free(s->session);
        s->session = new_sess;
    }

    s->session->time = ossl_time_now();
    ssl_session_calculate_timeout(s->session);

    OPENSSL_free(s->session->ext.tick);
    s->session->ext.tick = NULL;
    s->session->ext.ticklen = 0;

    s->session->ext.tick = OPENSSL_malloc(ticklen);
    if (s->session->ext.tick == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_CRYPTO_LIB);
        goto err;
    }
    if (!PACKET_copy_bytes(pkt, s->session->ext.tick, ticklen)) {
        SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_MISMATCH);
        goto err;
    }

    s->session->ext.tick_lifetime_hint = ticket_lifetime_hint;
    s->session->ext.tick_age_add = age_add;
    s->session->ext.ticklen = ticklen;

    if (SSL_CONNECTION_IS_TLS13(s)) {
        PACKET extpkt;

        if (!PACKET_as_length_prefixed_2(pkt, &extpkt)
                || PACKET_remaining(pkt) != 0) {
            SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_MISMATCH);
            goto err;
        }

        if (!tls_collect_extensions(s, &extpkt,
                                    SSL_EXT_TLS1_3_NEW_SESSION_TICKET, &exts,
                                    NULL, 1)
                || !tls_parse_all_extensions(s,
                                             SSL_EXT_TLS1_3_NEW_SESSION_TICKET,
                                             exts, NULL, 0, 1)) {
            /* SSLfatal() already called */
            goto err;
        }
    }

    /*
     * There are two ways to detect a resumed ticket session. One is to set
     * an appropriate session ID and then the server must return a match in
     * ServerHello. This allows the normal client session ID matching to work
     * and we know much earlier that the ticket has been accepted. The
     * other way is to set zero length session ID when the ticket is
     * presented and rely on the handshake to determine session resumption.
     * We choose the former approach because this fits in with assumptions
     * elsewhere in OpenSSL. The session ID is set to the SHA256 hash of the
     * ticket.
     */
    sha256 = EVP_MD_fetch(sctx->libctx, "SHA2-256", sctx->propq);
    if (sha256 == NULL) {
        /* Error is already recorded */
        SSLfatal_alert(s, SSL_AD_INTERNAL_ERROR);
        goto err;
    }
    /*
     * We use sess_len here because EVP_Digest expects an int
     * but s->session->session_id_length is a size_t
     */
    if (!EVP_Digest(s->session->ext.tick, ticklen,
                    s->session->session_id, &sess_len,
                    sha256, NULL)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_EVP_LIB);
        goto err;
    }
    EVP_MD_free(sha256);
    sha256 = NULL;
    s->session->session_id_length = sess_len;
    s->session->not_resumable = 0;

    /* This is a standalone message in TLSv1.3, so there is no more to read */
    if (SSL_CONNECTION_IS_TLS13(s)) {
        const EVP_MD *md = ssl_handshake_md(s);
        int hashleni = EVP_MD_get_size(md);
        size_t hashlen;
        static const unsigned char nonce_label[] = "resumption";

        /* Ensure cast to size_t is safe */
        if (!ossl_assert(hashleni > 0)) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            goto err;
        }
        hashlen = (size_t)hashleni;

        if (!tls13_hkdf_expand(s, md, s->resumption_master_secret,
                               nonce_label,
                               sizeof(nonce_label) - 1,
                               PACKET_data(&nonce),
                               PACKET_remaining(&nonce),
                               s->session->master_key,
                               hashlen, 1)) {
            /* SSLfatal() already called */
            goto err;
        }
        s->session->master_key_length = hashlen;

        OPENSSL_free(exts);
        ssl_update_cache(s, SSL_SESS_CACHE_CLIENT);
        return MSG_PROCESS_FINISHED_READING;
    }

    return MSG_PROCESS_CONTINUE_READING;
 err:
    EVP_MD_free(sha256);
    OPENSSL_free(exts);
    return MSG_PROCESS_ERROR;
}

/*
 * In TLSv1.3 this is called from the extensions code, otherwise it is used to
 * parse a separate message. Returns 1 on success or 0 on failure
 */
int tls_process_cert_status_body(SSL_CONNECTION *s, PACKET *pkt)
{
    size_t resplen;
    unsigned int type;

    if (!PACKET_get_1(pkt, &type)
        || type != TLSEXT_STATUSTYPE_ocsp) {
        SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_UNSUPPORTED_STATUS_TYPE);
        return 0;
    }
    if (!PACKET_get_net_3_len(pkt, &resplen)
        || PACKET_remaining(pkt) != resplen) {
        SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_MISMATCH);
        return 0;
    }
    s->ext.ocsp.resp = OPENSSL_malloc(resplen);
    if (s->ext.ocsp.resp == NULL) {
        s->ext.ocsp.resp_len = 0;
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_CRYPTO_LIB);
        return 0;
    }
    s->ext.ocsp.resp_len = resplen;
    if (!PACKET_copy_bytes(pkt, s->ext.ocsp.resp, resplen)) {
        SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_MISMATCH);
        return 0;
    }

    return 1;
}


MSG_PROCESS_RETURN tls_process_cert_status(SSL_CONNECTION *s, PACKET *pkt)
{
    if (!tls_process_cert_status_body(s, pkt)) {
        /* SSLfatal() already called */
        return MSG_PROCESS_ERROR;
    }

    return MSG_PROCESS_CONTINUE_READING;
}

/*
 * Perform miscellaneous checks and processing after we have received the
 * server's initial flight. In TLS1.3 this is after the Server Finished message.
 * In <=TLS1.2 this is after the ServerDone message. Returns 1 on success or 0
 * on failure.
 */
int tls_process_initial_server_flight(SSL_CONNECTION *s)
{
    SSL_CTX *sctx = SSL_CONNECTION_GET_CTX(s);

    /*
     * at this point we check that we have the required stuff from
     * the server
     */
    if (!ssl3_check_cert_and_algorithm(s)) {
        /* SSLfatal() already called */
        return 0;
    }

    /*
     * Call the ocsp status callback if needed. The |ext.ocsp.resp| and
     * |ext.ocsp.resp_len| values will be set if we actually received a status
     * message, or NULL and -1 otherwise
     */
    if (s->ext.status_type != TLSEXT_STATUSTYPE_nothing
            && sctx->ext.status_cb != NULL) {
        int ret = sctx->ext.status_cb(SSL_CONNECTION_GET_USER_SSL(s),
                                      sctx->ext.status_arg);

        if (ret == 0) {
            SSLfatal(s, SSL_AD_BAD_CERTIFICATE_STATUS_RESPONSE,
                     SSL_R_INVALID_STATUS_RESPONSE);
            return 0;
        }
        if (ret < 0) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR,
                     SSL_R_OCSP_CALLBACK_FAILURE);
            return 0;
        }
    }
#ifndef OPENSSL_NO_CT
    if (s->ct_validation_callback != NULL) {
        /* Note we validate the SCTs whether or not we abort on error */
        if (!ssl_validate_ct(s) && (s->verify_mode & SSL_VERIFY_PEER)) {
            /* SSLfatal() already called */
            return 0;
        }
    }
#endif

    return 1;
}

MSG_PROCESS_RETURN tls_process_server_done(SSL_CONNECTION *s, PACKET *pkt)
{
    if (PACKET_remaining(pkt) > 0) {
        /* should contain no data */
        SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_MISMATCH);
        return MSG_PROCESS_ERROR;
    }
#ifndef OPENSSL_NO_SRP
    if (s->s3.tmp.new_cipher->algorithm_mkey & SSL_kSRP) {
        if (ssl_srp_calc_a_param_intern(s) <= 0) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, SSL_R_SRP_A_CALC);
            return MSG_PROCESS_ERROR;
        }
    }
#endif

    if (!tls_process_initial_server_flight(s)) {
        /* SSLfatal() already called */
        return MSG_PROCESS_ERROR;
    }

    return MSG_PROCESS_FINISHED_READING;
}

static int tls_construct_cke_psk_preamble(SSL_CONNECTION *s, WPACKET *pkt)
{
#ifndef OPENSSL_NO_PSK
    int ret = 0;
    /*
     * The callback needs PSK_MAX_IDENTITY_LEN + 1 bytes to return a
     * \0-terminated identity. The last byte is for us for simulating
     * strnlen.
     */
    char identity[PSK_MAX_IDENTITY_LEN + 1];
    size_t identitylen = 0;
    unsigned char psk[PSK_MAX_PSK_LEN];
    unsigned char *tmppsk = NULL;
    char *tmpidentity = NULL;
    size_t psklen = 0;

    if (s->psk_client_callback == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, SSL_R_PSK_NO_CLIENT_CB);
        goto err;
    }

    memset(identity, 0, sizeof(identity));

    psklen = s->psk_client_callback(SSL_CONNECTION_GET_USER_SSL(s),
                                    s->session->psk_identity_hint,
                                    identity, sizeof(identity) - 1,
                                    psk, sizeof(psk));

    if (psklen > PSK_MAX_PSK_LEN) {
        SSLfatal(s, SSL_AD_HANDSHAKE_FAILURE, ERR_R_INTERNAL_ERROR);
        psklen = PSK_MAX_PSK_LEN;   /* Avoid overrunning the array on cleanse */
        goto err;
    } else if (psklen == 0) {
        SSLfatal(s, SSL_AD_HANDSHAKE_FAILURE, SSL_R_PSK_IDENTITY_NOT_FOUND);
        goto err;
    }

    identitylen = strlen(identity);
    if (identitylen > PSK_MAX_IDENTITY_LEN) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }

    tmppsk = OPENSSL_memdup(psk, psklen);
    tmpidentity = OPENSSL_strdup(identity);
    if (tmppsk == NULL || tmpidentity == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_CRYPTO_LIB);
        goto err;
    }

    OPENSSL_free(s->s3.tmp.psk);
    s->s3.tmp.psk = tmppsk;
    s->s3.tmp.psklen = psklen;
    tmppsk = NULL;
    OPENSSL_free(s->session->psk_identity);
    s->session->psk_identity = tmpidentity;
    tmpidentity = NULL;

    if (!WPACKET_sub_memcpy_u16(pkt, identity, identitylen))  {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }

    ret = 1;

 err:
    OPENSSL_cleanse(psk, psklen);
    OPENSSL_cleanse(identity, sizeof(identity));
    OPENSSL_clear_free(tmppsk, psklen);
    OPENSSL_clear_free(tmpidentity, identitylen);

    return ret;
#else
    SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
    return 0;
#endif
}

static int tls_construct_cke_rsa(SSL_CONNECTION *s, WPACKET *pkt)
{
    unsigned char *encdata = NULL;
    EVP_PKEY *pkey = NULL;
    EVP_PKEY_CTX *pctx = NULL;
    size_t enclen;
    unsigned char *pms = NULL;
    size_t pmslen = 0;
    SSL_CTX *sctx = SSL_CONNECTION_GET_CTX(s);

    if (!received_server_cert(s)) {
        /*
         * We should always have a server certificate with SSL_kRSA.
         */
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    if ((pkey = tls_get_peer_pkey(s)) == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    if (!EVP_PKEY_is_a(pkey, "RSA")) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    pmslen = SSL_MAX_MASTER_KEY_LENGTH;
    pms = OPENSSL_malloc(pmslen);
    if (pms == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_CRYPTO_LIB);
        return 0;
    }

    pms[0] = s->client_version >> 8;
    pms[1] = s->client_version & 0xff;
    if (RAND_bytes_ex(sctx->libctx, pms + 2, pmslen - 2, 0) <= 0) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_RAND_LIB);
        goto err;
    }

    /* Fix buf for TLS and beyond */
    if (s->version > SSL3_VERSION && !WPACKET_start_sub_packet_u16(pkt)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }

    pctx = EVP_PKEY_CTX_new_from_pkey(sctx->libctx, pkey, sctx->propq);
    if (pctx == NULL || EVP_PKEY_encrypt_init(pctx) <= 0
        || EVP_PKEY_encrypt(pctx, NULL, &enclen, pms, pmslen) <= 0) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_EVP_LIB);
        goto err;
    }
    if (!WPACKET_allocate_bytes(pkt, enclen, &encdata)
            || EVP_PKEY_encrypt(pctx, encdata, &enclen, pms, pmslen) <= 0) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, SSL_R_BAD_RSA_ENCRYPT);
        goto err;
    }
    EVP_PKEY_CTX_free(pctx);
    pctx = NULL;

    /* Fix buf for TLS and beyond */
    if (s->version > SSL3_VERSION && !WPACKET_close(pkt)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }

    /* Log the premaster secret, if logging is enabled. */
    if (!ssl_log_rsa_client_key_exchange(s, encdata, enclen, pms, pmslen)) {
        /* SSLfatal() already called */
        goto err;
    }

    s->s3.tmp.pms = pms;
    s->s3.tmp.pmslen = pmslen;

    return 1;
 err:
    OPENSSL_clear_free(pms, pmslen);
    EVP_PKEY_CTX_free(pctx);

    return 0;
}

static int tls_construct_cke_dhe(SSL_CONNECTION *s, WPACKET *pkt)
{
    EVP_PKEY *ckey = NULL, *skey = NULL;
    unsigned char *keybytes = NULL;
    int prime_len;
    unsigned char *encoded_pub = NULL;
    size_t encoded_pub_len, pad_len;
    int ret = 0;

    skey = s->s3.peer_tmp;
    if (skey == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }

    ckey = ssl_generate_pkey(s, skey);
    if (ckey == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }

    if (ssl_derive(s, ckey, skey, 0) == 0) {
        /* SSLfatal() already called */
        goto err;
    }

    /* send off the data */

    /* Generate encoding of server key */
    encoded_pub_len = EVP_PKEY_get1_encoded_public_key(ckey, &encoded_pub);
    if (encoded_pub_len == 0) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        EVP_PKEY_free(ckey);
        return EXT_RETURN_FAIL;
    }

    /*
     * For interoperability with some versions of the Microsoft TLS
     * stack, we need to zero pad the DHE pub key to the same length
     * as the prime.
     */
    prime_len = EVP_PKEY_get_size(ckey);
    pad_len = prime_len - encoded_pub_len;
    if (pad_len > 0) {
        if (!WPACKET_sub_allocate_bytes_u16(pkt, pad_len, &keybytes)) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            goto err;
        }
        memset(keybytes, 0, pad_len);
    }

    if (!WPACKET_sub_memcpy_u16(pkt, encoded_pub, encoded_pub_len)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }

    ret = 1;
 err:
    OPENSSL_free(encoded_pub);
    EVP_PKEY_free(ckey);
    return ret;
}

static int tls_construct_cke_ecdhe(SSL_CONNECTION *s, WPACKET *pkt)
{
    unsigned char *encodedPoint = NULL;
    size_t encoded_pt_len = 0;
    EVP_PKEY *ckey = NULL, *skey = NULL;
    int ret = 0;

    skey = s->s3.peer_tmp;
    if (skey == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    ckey = ssl_generate_pkey(s, skey);
    if (ckey == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_SSL_LIB);
        goto err;
    }

    if (ssl_derive(s, ckey, skey, 0) == 0) {
        /* SSLfatal() already called */
        goto err;
    }

    /* Generate encoding of client key */
    encoded_pt_len = EVP_PKEY_get1_encoded_public_key(ckey, &encodedPoint);

    if (encoded_pt_len == 0) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_EC_LIB);
        goto err;
    }

    if (!WPACKET_sub_memcpy_u8(pkt, encodedPoint, encoded_pt_len)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }

    ret = 1;
 err:
    OPENSSL_free(encodedPoint);
    EVP_PKEY_free(ckey);
    return ret;
}

static int tls_construct_cke_gost(SSL_CONNECTION *s, WPACKET *pkt)
{
#ifndef OPENSSL_NO_GOST
    /* GOST key exchange message creation */
    EVP_PKEY_CTX *pkey_ctx = NULL;
    EVP_PKEY *pkey = NULL;
    size_t msglen;
    unsigned int md_len;
    unsigned char shared_ukm[32], tmp[256];
    EVP_MD_CTX *ukm_hash = NULL;
    int dgst_nid = NID_id_GostR3411_94;
    unsigned char *pms = NULL;
    size_t pmslen = 0;
    SSL_CTX *sctx = SSL_CONNECTION_GET_CTX(s);

    if ((s->s3.tmp.new_cipher->algorithm_auth & SSL_aGOST12) != 0)
        dgst_nid = NID_id_GostR3411_2012_256;

    /*
     * Get server certificate PKEY and create ctx from it
     */
    if ((pkey = tls_get_peer_pkey(s)) == NULL) {
        SSLfatal(s, SSL_AD_HANDSHAKE_FAILURE,
                 SSL_R_NO_GOST_CERTIFICATE_SENT_BY_PEER);
        return 0;
    }

    pkey_ctx = EVP_PKEY_CTX_new_from_pkey(sctx->libctx,
                                          pkey,
                                          sctx->propq);
    if (pkey_ctx == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_EVP_LIB);
        return 0;
    }
    /*
     * If we have send a certificate, and certificate key
     * parameters match those of server certificate, use
     * certificate key for key exchange
     */

    /* Otherwise, generate ephemeral key pair */
    pmslen = 32;
    pms = OPENSSL_malloc(pmslen);
    if (pms == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_CRYPTO_LIB);
        goto err;
    }

    if (EVP_PKEY_encrypt_init(pkey_ctx) <= 0
        /* Generate session key
         */
        || RAND_bytes_ex(sctx->libctx, pms, pmslen, 0) <= 0) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    };
    /*
     * Compute shared IV and store it in algorithm-specific context
     * data
     */
    ukm_hash = EVP_MD_CTX_new();
    if (ukm_hash == NULL
        || EVP_DigestInit(ukm_hash, EVP_get_digestbynid(dgst_nid)) <= 0
        || EVP_DigestUpdate(ukm_hash, s->s3.client_random,
                            SSL3_RANDOM_SIZE) <= 0
        || EVP_DigestUpdate(ukm_hash, s->s3.server_random,
                            SSL3_RANDOM_SIZE) <= 0
        || EVP_DigestFinal_ex(ukm_hash, shared_ukm, &md_len) <= 0) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    EVP_MD_CTX_free(ukm_hash);
    ukm_hash = NULL;
    if (EVP_PKEY_CTX_ctrl(pkey_ctx, -1, EVP_PKEY_OP_ENCRYPT,
                          EVP_PKEY_CTRL_SET_IV, 8, shared_ukm) <= 0) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, SSL_R_LIBRARY_BUG);
        goto err;
    }
    /* Make GOST keytransport blob message */
    /*
     * Encapsulate it into sequence
     */
    msglen = 255;
    if (EVP_PKEY_encrypt(pkey_ctx, tmp, &msglen, pms, pmslen) <= 0) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, SSL_R_LIBRARY_BUG);
        goto err;
    }

    if (!WPACKET_put_bytes_u8(pkt, V_ASN1_SEQUENCE | V_ASN1_CONSTRUCTED)
            || (msglen >= 0x80 && !WPACKET_put_bytes_u8(pkt, 0x81))
            || !WPACKET_sub_memcpy_u8(pkt, tmp, msglen)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }

    EVP_PKEY_CTX_free(pkey_ctx);
    s->s3.tmp.pms = pms;
    s->s3.tmp.pmslen = pmslen;

    return 1;
 err:
    EVP_PKEY_CTX_free(pkey_ctx);
    OPENSSL_clear_free(pms, pmslen);
    EVP_MD_CTX_free(ukm_hash);
    return 0;
#else
    SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
    return 0;
#endif
}

#ifndef OPENSSL_NO_GOST
int ossl_gost18_cke_cipher_nid(const SSL_CONNECTION *s)
{
    if ((s->s3.tmp.new_cipher->algorithm_enc & SSL_MAGMA) != 0)
        return NID_magma_ctr;
    else if ((s->s3.tmp.new_cipher->algorithm_enc & SSL_KUZNYECHIK) != 0)
        return NID_kuznyechik_ctr;

    return NID_undef;
}

int ossl_gost_ukm(const SSL_CONNECTION *s, unsigned char *dgst_buf)
{
    EVP_MD_CTX *hash = NULL;
    unsigned int md_len;
    SSL_CTX *sctx = SSL_CONNECTION_GET_CTX(s);
    const EVP_MD *md = ssl_evp_md_fetch(sctx->libctx, NID_id_GostR3411_2012_256,
                                        sctx->propq);

    if (md == NULL)
        return 0;

    if ((hash = EVP_MD_CTX_new()) == NULL
        || EVP_DigestInit(hash, md) <= 0
        || EVP_DigestUpdate(hash, s->s3.client_random, SSL3_RANDOM_SIZE) <= 0
        || EVP_DigestUpdate(hash, s->s3.server_random, SSL3_RANDOM_SIZE) <= 0
        || EVP_DigestFinal_ex(hash, dgst_buf, &md_len) <= 0) {
        EVP_MD_CTX_free(hash);
        ssl_evp_md_free(md);
        return 0;
    }

    EVP_MD_CTX_free(hash);
    ssl_evp_md_free(md);
    return 1;
}
#endif

static int tls_construct_cke_gost18(SSL_CONNECTION *s, WPACKET *pkt)
{
#ifndef OPENSSL_NO_GOST
    /* GOST 2018 key exchange message creation */
    unsigned char rnd_dgst[32];
    unsigned char *encdata = NULL;
    EVP_PKEY_CTX *pkey_ctx = NULL;
    EVP_PKEY *pkey;
    unsigned char *pms = NULL;
    size_t pmslen = 0;
    size_t msglen;
    int cipher_nid = ossl_gost18_cke_cipher_nid(s);
    SSL_CTX *sctx = SSL_CONNECTION_GET_CTX(s);

    if (cipher_nid == NID_undef) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    if (ossl_gost_ukm(s, rnd_dgst) <= 0) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }

    /* Pre-master secret - random bytes */
    pmslen = 32;
    pms = OPENSSL_malloc(pmslen);
    if (pms == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_CRYPTO_LIB);
        goto err;
    }

    if (RAND_bytes_ex(sctx->libctx, pms, pmslen, 0) <= 0) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }

     /* Get server certificate PKEY and create ctx from it */
    if ((pkey = tls_get_peer_pkey(s)) == NULL) {
        SSLfatal(s, SSL_AD_HANDSHAKE_FAILURE,
                 SSL_R_NO_GOST_CERTIFICATE_SENT_BY_PEER);
        goto err;
    }

    pkey_ctx = EVP_PKEY_CTX_new_from_pkey(sctx->libctx,
                                          pkey,
                                          sctx->propq);
    if (pkey_ctx == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_EVP_LIB);
        goto err;
    }

    if (EVP_PKEY_encrypt_init(pkey_ctx) <= 0) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    };

    /* Reuse EVP_PKEY_CTRL_SET_IV, make choice in engine code */
    if (EVP_PKEY_CTX_ctrl(pkey_ctx, -1, EVP_PKEY_OP_ENCRYPT,
                          EVP_PKEY_CTRL_SET_IV, 32, rnd_dgst) <= 0) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, SSL_R_LIBRARY_BUG);
        goto err;
    }

    if (EVP_PKEY_CTX_ctrl(pkey_ctx, -1, EVP_PKEY_OP_ENCRYPT,
                          EVP_PKEY_CTRL_CIPHER, cipher_nid, NULL) <= 0) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, SSL_R_LIBRARY_BUG);
        goto err;
    }

    if (EVP_PKEY_encrypt(pkey_ctx, NULL, &msglen, pms, pmslen) <= 0) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_EVP_LIB);
        goto err;
    }

    if (!WPACKET_allocate_bytes(pkt, msglen, &encdata)
            || EVP_PKEY_encrypt(pkey_ctx, encdata, &msglen, pms, pmslen) <= 0) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_EVP_LIB);
        goto err;
    }

    EVP_PKEY_CTX_free(pkey_ctx);
    pkey_ctx = NULL;
    s->s3.tmp.pms = pms;
    s->s3.tmp.pmslen = pmslen;

    return 1;
 err:
    EVP_PKEY_CTX_free(pkey_ctx);
    OPENSSL_clear_free(pms, pmslen);
    return 0;
#else
    SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
    return 0;
#endif
}

static int tls_construct_cke_srp(SSL_CONNECTION *s, WPACKET *pkt)
{
#ifndef OPENSSL_NO_SRP
    unsigned char *abytes = NULL;

    if (s->srp_ctx.A == NULL
            || !WPACKET_sub_allocate_bytes_u16(pkt, BN_num_bytes(s->srp_ctx.A),
                                               &abytes)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return 0;
    }
    BN_bn2bin(s->srp_ctx.A, abytes);

    OPENSSL_free(s->session->srp_username);
    s->session->srp_username = OPENSSL_strdup(s->srp_ctx.login);
    if (s->session->srp_username == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_CRYPTO_LIB);
        return 0;
    }

    return 1;
#else
    SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
    return 0;
#endif
}

CON_FUNC_RETURN tls_construct_client_key_exchange(SSL_CONNECTION *s,
                                                  WPACKET *pkt)
{
    unsigned long alg_k;

    alg_k = s->s3.tmp.new_cipher->algorithm_mkey;

    /*
     * All of the construct functions below call SSLfatal() if necessary so
     * no need to do so here.
     */
    if ((alg_k & SSL_PSK)
        && !tls_construct_cke_psk_preamble(s, pkt))
        goto err;

    if (alg_k & (SSL_kRSA | SSL_kRSAPSK)) {
        if (!tls_construct_cke_rsa(s, pkt))
            goto err;
    } else if (alg_k & (SSL_kDHE | SSL_kDHEPSK)) {
        if (!tls_construct_cke_dhe(s, pkt))
            goto err;
    } else if (alg_k & (SSL_kECDHE | SSL_kECDHEPSK)) {
        if (!tls_construct_cke_ecdhe(s, pkt))
            goto err;
    } else if (alg_k & SSL_kGOST) {
        if (!tls_construct_cke_gost(s, pkt))
            goto err;
    } else if (alg_k & SSL_kGOST18) {
        if (!tls_construct_cke_gost18(s, pkt))
            goto err;
    } else if (alg_k & SSL_kSRP) {
        if (!tls_construct_cke_srp(s, pkt))
            goto err;
    } else if (!(alg_k & SSL_kPSK)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }

    return CON_FUNC_SUCCESS;
 err:
    OPENSSL_clear_free(s->s3.tmp.pms, s->s3.tmp.pmslen);
    s->s3.tmp.pms = NULL;
    s->s3.tmp.pmslen = 0;
#ifndef OPENSSL_NO_PSK
    OPENSSL_clear_free(s->s3.tmp.psk, s->s3.tmp.psklen);
    s->s3.tmp.psk = NULL;
    s->s3.tmp.psklen = 0;
#endif
    return CON_FUNC_ERROR;
}

int tls_client_key_exchange_post_work(SSL_CONNECTION *s)
{
    unsigned char *pms = NULL;
    size_t pmslen = 0;

    pms = s->s3.tmp.pms;
    pmslen = s->s3.tmp.pmslen;

#ifndef OPENSSL_NO_SRP
    /* Check for SRP */
    if (s->s3.tmp.new_cipher->algorithm_mkey & SSL_kSRP) {
        if (!srp_generate_client_master_secret(s)) {
            /* SSLfatal() already called */
            goto err;
        }
        return 1;
    }
#endif

    if (pms == NULL && !(s->s3.tmp.new_cipher->algorithm_mkey & SSL_kPSK)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_PASSED_INVALID_ARGUMENT);
        goto err;
    }
    if (!ssl_generate_master_secret(s, pms, pmslen, 1)) {
        /* SSLfatal() already called */
        /* ssl_generate_master_secret frees the pms even on error */
        pms = NULL;
        pmslen = 0;
        goto err;
    }
    pms = NULL;
    pmslen = 0;

#ifndef OPENSSL_NO_SCTP
    if (SSL_CONNECTION_IS_DTLS(s)) {
        unsigned char sctpauthkey[64];
        char labelbuffer[sizeof(DTLS1_SCTP_AUTH_LABEL)];
        size_t labellen;
        SSL *ssl = SSL_CONNECTION_GET_SSL(s);

        /*
         * Add new shared key for SCTP-Auth, will be ignored if no SCTP
         * used.
         */
        memcpy(labelbuffer, DTLS1_SCTP_AUTH_LABEL,
               sizeof(DTLS1_SCTP_AUTH_LABEL));

        /* Don't include the terminating zero. */
        labellen = sizeof(labelbuffer) - 1;
        if (s->mode & SSL_MODE_DTLS_SCTP_LABEL_LENGTH_BUG)
            labellen += 1;

        if (SSL_export_keying_material(ssl, sctpauthkey,
                                       sizeof(sctpauthkey), labelbuffer,
                                       labellen, NULL, 0, 0) <= 0) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            goto err;
        }

        BIO_ctrl(SSL_get_wbio(ssl), BIO_CTRL_DGRAM_SCTP_ADD_AUTH_KEY,
                 sizeof(sctpauthkey), sctpauthkey);
    }
#endif

    return 1;
 err:
    OPENSSL_clear_free(pms, pmslen);
    s->s3.tmp.pms = NULL;
    s->s3.tmp.pmslen = 0;
    return 0;
}

/*
 * Check a certificate can be used for client authentication. Currently check
 * cert exists, if we have a suitable digest for TLS 1.2 if static DH client
 * certificates can be used and optionally checks suitability for Suite B.
 */
static int ssl3_check_client_certificate(SSL_CONNECTION *s)
{
    /* If no suitable signature algorithm can't use certificate */
    if (!tls_choose_sigalg(s, 0) || s->s3.tmp.sigalg == NULL)
        return 0;
    /*
     * If strict mode check suitability of chain before using it. This also
     * adjusts suite B digest if necessary.
     */
    if (s->cert->cert_flags & SSL_CERT_FLAGS_CHECK_TLS_STRICT &&
        !tls1_check_chain(s, NULL, NULL, NULL, -2))
        return 0;
    return 1;
}

WORK_STATE tls_prepare_client_certificate(SSL_CONNECTION *s, WORK_STATE wst)
{
    X509 *x509 = NULL;
    EVP_PKEY *pkey = NULL;
    int i;
    SSL *ssl = SSL_CONNECTION_GET_SSL(s);

    if (wst == WORK_MORE_A) {
        /* Let cert callback update client certificates if required */
        if (s->cert->cert_cb) {
            i = s->cert->cert_cb(ssl, s->cert->cert_cb_arg);
            if (i < 0) {
                s->rwstate = SSL_X509_LOOKUP;
                return WORK_MORE_A;
            }
            if (i == 0) {
                SSLfatal(s, SSL_AD_INTERNAL_ERROR, SSL_R_CALLBACK_FAILED);
                return WORK_ERROR;
            }
            s->rwstate = SSL_NOTHING;
        }
        if (ssl3_check_client_certificate(s)) {
            if (s->post_handshake_auth == SSL_PHA_REQUESTED) {
                return WORK_FINISHED_STOP;
            }
            return WORK_FINISHED_CONTINUE;
        }

        /* Fall through to WORK_MORE_B */
        wst = WORK_MORE_B;
    }

    /* We need to get a client cert */
    if (wst == WORK_MORE_B) {
        /*
         * If we get an error, we need to ssl->rwstate=SSL_X509_LOOKUP;
         * return(-1); We then get retied later
         */
        i = ssl_do_client_cert_cb(s, &x509, &pkey);
        if (i < 0) {
            s->rwstate = SSL_X509_LOOKUP;
            return WORK_MORE_B;
        }
        s->rwstate = SSL_NOTHING;
        if ((i == 1) && (pkey != NULL) && (x509 != NULL)) {
            if (!SSL_use_certificate(ssl, x509)
                || !SSL_use_PrivateKey(ssl, pkey))
                i = 0;
        } else if (i == 1) {
            i = 0;
            ERR_raise(ERR_LIB_SSL, SSL_R_BAD_DATA_RETURNED_BY_CALLBACK);
        }

        X509_free(x509);
        EVP_PKEY_free(pkey);
        if (i && !ssl3_check_client_certificate(s))
            i = 0;
        if (i == 0) {
            if (s->version == SSL3_VERSION) {
                s->s3.tmp.cert_req = 0;
                ssl3_send_alert(s, SSL3_AL_WARNING, SSL_AD_NO_CERTIFICATE);
                return WORK_FINISHED_CONTINUE;
            } else {
                s->s3.tmp.cert_req = 2;
                s->ext.compress_certificate_from_peer[0] = TLSEXT_comp_cert_none;
                if (!ssl3_digest_cached_records(s, 0)) {
                    /* SSLfatal() already called */
                    return WORK_ERROR;
                }
            }
        }

        if (!SSL_CONNECTION_IS_TLS13(s)
                || (s->options & SSL_OP_NO_TX_CERTIFICATE_COMPRESSION) != 0)
            s->ext.compress_certificate_from_peer[0] = TLSEXT_comp_cert_none;

        if (s->post_handshake_auth == SSL_PHA_REQUESTED)
            return WORK_FINISHED_STOP;
        return WORK_FINISHED_CONTINUE;
    }

    /* Shouldn't ever get here */
    SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
    return WORK_ERROR;
}

CON_FUNC_RETURN tls_construct_client_certificate(SSL_CONNECTION *s,
                                                 WPACKET *pkt)
{
    CERT_PKEY *cpk = NULL;
    SSL *ssl = SSL_CONNECTION_GET_SSL(s);

    if (SSL_CONNECTION_IS_TLS13(s)) {
        if (s->pha_context == NULL) {
            /* no context available, add 0-length context */
            if (!WPACKET_put_bytes_u8(pkt, 0)) {
                SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
                return CON_FUNC_ERROR;
            }
        } else if (!WPACKET_sub_memcpy_u8(pkt, s->pha_context, s->pha_context_len)) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            return CON_FUNC_ERROR;
        }
    }
    if (s->s3.tmp.cert_req != 2)
        cpk = s->cert->key;
    switch (s->ext.client_cert_type) {
    case TLSEXT_cert_type_rpk:
        if (!tls_output_rpk(s, pkt, cpk)) {
            /* SSLfatal() already called */
            return CON_FUNC_ERROR;
        }
        break;
    case TLSEXT_cert_type_x509:
        if (!ssl3_output_cert_chain(s, pkt, cpk, 0)) {
            /* SSLfatal() already called */
            return CON_FUNC_ERROR;
        }
        break;
    default:
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return CON_FUNC_ERROR;
    }

    /*
     * If we attempted to write early data or we're in middlebox compat mode
     * then we deferred changing the handshake write keys to the last possible
     * moment. We need to do it now.
     */
    if (SSL_CONNECTION_IS_TLS13(s)
            && !SSL_IS_QUIC_HANDSHAKE(s)
            && SSL_IS_FIRST_HANDSHAKE(s)
            && (s->early_data_state != SSL_EARLY_DATA_NONE
                || (s->options & SSL_OP_ENABLE_MIDDLEBOX_COMPAT) != 0)
            && (!ssl->method->ssl3_enc->change_cipher_state(s,
                    SSL3_CC_HANDSHAKE | SSL3_CHANGE_CIPHER_CLIENT_WRITE))) {
        /*
         * This is a fatal error, which leaves enc_write_ctx in an inconsistent
         * state and thus ssl3_send_alert may crash.
         */
        SSLfatal(s, SSL_AD_NO_ALERT, SSL_R_CANNOT_CHANGE_CIPHER);
        return CON_FUNC_ERROR;
    }

    return CON_FUNC_SUCCESS;
}

#ifndef OPENSSL_NO_COMP_ALG
CON_FUNC_RETURN tls_construct_client_compressed_certificate(SSL_CONNECTION *sc,
                                                            WPACKET *pkt)
{
    SSL *ssl = SSL_CONNECTION_GET_SSL(sc);
    WPACKET tmppkt;
    BUF_MEM *buf = NULL;
    size_t length;
    size_t max_length;
    COMP_METHOD *method;
    COMP_CTX *comp = NULL;
    int comp_len;
    int ret = 0;
    int alg = sc->ext.compress_certificate_from_peer[0];

    /* Note that sc->s3.tmp.cert_req == 2 is checked in write transition */

    if ((buf = BUF_MEM_new()) == NULL || !WPACKET_init(&tmppkt, buf))
        goto err;

    /* Use the |tmppkt| for the to-be-compressed data */
    if (sc->pha_context == NULL) {
        /* no context available, add 0-length context */
        if (!WPACKET_put_bytes_u8(&tmppkt, 0))
            goto err;
    } else if (!WPACKET_sub_memcpy_u8(&tmppkt, sc->pha_context, sc->pha_context_len))
        goto err;

    if (!ssl3_output_cert_chain(sc, &tmppkt, sc->cert->key, 0)) {
        /* SSLfatal() already called */
        goto out;
    }

    /* continue with the real |pkt| */
    if (!WPACKET_put_bytes_u16(pkt, alg)
            || !WPACKET_get_total_written(&tmppkt, &length)
            || !WPACKET_put_bytes_u24(pkt, length))
        goto err;

    switch (alg) {
    case TLSEXT_comp_cert_zlib:
        method = COMP_zlib_oneshot();
        break;
    case TLSEXT_comp_cert_brotli:
        method = COMP_brotli_oneshot();
        break;
    case TLSEXT_comp_cert_zstd:
        method = COMP_zstd_oneshot();
        break;
    default:
        goto err;
    }
    max_length = ossl_calculate_comp_expansion(alg, length);

    if ((comp = COMP_CTX_new(method)) == NULL
            || !WPACKET_start_sub_packet_u24(pkt)
            || !WPACKET_reserve_bytes(pkt, max_length, NULL))
        goto err;

    comp_len = COMP_compress_block(comp, WPACKET_get_curr(pkt), max_length,
                                   (unsigned char *)buf->data, length);
    if (comp_len <= 0)
        goto err;

    if (!WPACKET_allocate_bytes(pkt, comp_len, NULL)
            || !WPACKET_close(pkt))
        goto err;

    /*
     * If we attempted to write early data or we're in middlebox compat mode
     * then we deferred changing the handshake write keys to the last possible
     * moment. We need to do it now.
     */
    if (SSL_IS_FIRST_HANDSHAKE(sc)
            && !SSL_IS_QUIC_HANDSHAKE(sc)
            && (sc->early_data_state != SSL_EARLY_DATA_NONE
                || (sc->options & SSL_OP_ENABLE_MIDDLEBOX_COMPAT) != 0)
            && (!ssl->method->ssl3_enc->change_cipher_state(sc,
                    SSL3_CC_HANDSHAKE | SSL3_CHANGE_CIPHER_CLIENT_WRITE))) {
        /*
         * This is a fatal error, which leaves sc->enc_write_ctx in an
         * inconsistent state and thus ssl3_send_alert may crash.
         */
        SSLfatal(sc, SSL_AD_NO_ALERT, SSL_R_CANNOT_CHANGE_CIPHER);
        goto out;
    }
    ret = 1;
    goto out;

 err:
    SSLfatal(sc, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
 out:
    if (buf != NULL) {
        /* If |buf| is NULL, then |tmppkt| could not have been initialized */
        WPACKET_cleanup(&tmppkt);
    }
    BUF_MEM_free(buf);
    COMP_CTX_free(comp);
    return ret;
}
#endif

int ssl3_check_cert_and_algorithm(SSL_CONNECTION *s)
{
    const SSL_CERT_LOOKUP *clu;
    size_t idx;
    long alg_k, alg_a;
    EVP_PKEY *pkey;

    alg_k = s->s3.tmp.new_cipher->algorithm_mkey;
    alg_a = s->s3.tmp.new_cipher->algorithm_auth;

    /* we don't have a certificate */
    if (!(alg_a & SSL_aCERT))
        return 1;

    /* This is the passed certificate */
    pkey = tls_get_peer_pkey(s);
    clu = ssl_cert_lookup_by_pkey(pkey, &idx, SSL_CONNECTION_GET_CTX(s));

    /* Check certificate is recognised and suitable for cipher */
    if (clu == NULL || (alg_a & clu->amask) == 0) {
        SSLfatal(s, SSL_AD_HANDSHAKE_FAILURE, SSL_R_MISSING_SIGNING_CERT);
        return 0;
    }

    if (alg_k & (SSL_kRSA | SSL_kRSAPSK) && idx != SSL_PKEY_RSA) {
        SSLfatal(s, SSL_AD_HANDSHAKE_FAILURE,
                 SSL_R_MISSING_RSA_ENCRYPTING_CERT);
        return 0;
    }

    if ((alg_k & SSL_kDHE) && (s->s3.peer_tmp == NULL)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    /* Early out to skip the checks below */
    if (s->session->peer_rpk != NULL)
        return 1;

    if (clu->amask & SSL_aECDSA) {
        if (ssl_check_srvr_ecc_cert_and_alg(s->session->peer, s))
            return 1;
        SSLfatal(s, SSL_AD_HANDSHAKE_FAILURE, SSL_R_BAD_ECC_CERT);
        return 0;
    }

    return 1;
}

#ifndef OPENSSL_NO_NEXTPROTONEG
CON_FUNC_RETURN tls_construct_next_proto(SSL_CONNECTION *s, WPACKET *pkt)
{
    size_t len, padding_len;
    unsigned char *padding = NULL;

    len = s->ext.npn_len;
    padding_len = 32 - ((len + 2) % 32);

    if (!WPACKET_sub_memcpy_u8(pkt, s->ext.npn, len)
            || !WPACKET_sub_allocate_bytes_u8(pkt, padding_len, &padding)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return CON_FUNC_ERROR;
    }

    memset(padding, 0, padding_len);

    return CON_FUNC_SUCCESS;
}
#endif

MSG_PROCESS_RETURN tls_process_hello_req(SSL_CONNECTION *s, PACKET *pkt)
{
    SSL *ssl = SSL_CONNECTION_GET_SSL(s);

    if (PACKET_remaining(pkt) > 0) {
        /* should contain no data */
        SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_MISMATCH);
        return MSG_PROCESS_ERROR;
    }

    if ((s->options & SSL_OP_NO_RENEGOTIATION)) {
        ssl3_send_alert(s, SSL3_AL_WARNING, SSL_AD_NO_RENEGOTIATION);
        return MSG_PROCESS_FINISHED_READING;
    }

    /*
     * This is a historical discrepancy (not in the RFC) maintained for
     * compatibility reasons. If a TLS client receives a HelloRequest it will
     * attempt an abbreviated handshake. However if a DTLS client receives a
     * HelloRequest it will do a full handshake. Either behaviour is reasonable
     * but doing one for TLS and another for DTLS is odd.
     */
    if (SSL_CONNECTION_IS_DTLS(s))
        SSL_renegotiate(ssl);
    else
        SSL_renegotiate_abbreviated(ssl);

    return MSG_PROCESS_FINISHED_READING;
}

static MSG_PROCESS_RETURN tls_process_encrypted_extensions(SSL_CONNECTION *s,
                                                           PACKET *pkt)
{
    PACKET extensions;
    RAW_EXTENSION *rawexts = NULL;

    if (!PACKET_as_length_prefixed_2(pkt, &extensions)
            || PACKET_remaining(pkt) != 0) {
        SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_MISMATCH);
        goto err;
    }

    if (!tls_collect_extensions(s, &extensions,
                                SSL_EXT_TLS1_3_ENCRYPTED_EXTENSIONS, &rawexts,
                                NULL, 1)
            || !tls_parse_all_extensions(s, SSL_EXT_TLS1_3_ENCRYPTED_EXTENSIONS,
                                         rawexts, NULL, 0, 1)) {
        /* SSLfatal() already called */
        goto err;
    }

    OPENSSL_free(rawexts);
    return MSG_PROCESS_CONTINUE_READING;

 err:
    OPENSSL_free(rawexts);
    return MSG_PROCESS_ERROR;
}

int ssl_do_client_cert_cb(SSL_CONNECTION *s, X509 **px509, EVP_PKEY **ppkey)
{
    int i = 0;
    SSL_CTX *sctx = SSL_CONNECTION_GET_CTX(s);

#ifndef OPENSSL_NO_ENGINE
    if (sctx->client_cert_engine) {
        i = tls_engine_load_ssl_client_cert(s, px509, ppkey);
        if (i != 0)
            return i;
    }
#endif
    if (sctx->client_cert_cb)
        i = sctx->client_cert_cb(SSL_CONNECTION_GET_USER_SSL(s), px509, ppkey);
    return i;
}

int ssl_cipher_list_to_bytes(SSL_CONNECTION *s, STACK_OF(SSL_CIPHER) *sk,
                             WPACKET *pkt)
{
    int i;
    size_t totlen = 0, len, maxlen, maxverok = 0;
    int empty_reneg_info_scsv = !s->renegotiate
                                && !SSL_CONNECTION_IS_DTLS(s)
                                && ssl_security(s, SSL_SECOP_VERSION, 0, TLS1_VERSION, NULL)
                                && s->min_proto_version <= TLS1_VERSION;
    SSL *ssl = SSL_CONNECTION_GET_SSL(s);

    /* Set disabled masks for this session */
    if (!ssl_set_client_disabled(s)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, SSL_R_NO_PROTOCOLS_AVAILABLE);
        return 0;
    }

    if (sk == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return 0;
    }

#ifdef OPENSSL_MAX_TLS1_2_CIPHER_LENGTH
# if OPENSSL_MAX_TLS1_2_CIPHER_LENGTH < 6
#  error Max cipher length too short
# endif
    /*
     * Some servers hang if client hello > 256 bytes as hack workaround
     * chop number of supported ciphers to keep it well below this if we
     * use TLS v1.2
     */
    if (TLS1_get_version(ssl) >= TLS1_2_VERSION)
        maxlen = OPENSSL_MAX_TLS1_2_CIPHER_LENGTH & ~1;
    else
#endif
        /* Maximum length that can be stored in 2 bytes. Length must be even */
        maxlen = 0xfffe;

    if (empty_reneg_info_scsv)
        maxlen -= 2;
    if (s->mode & SSL_MODE_SEND_FALLBACK_SCSV)
        maxlen -= 2;

    for (i = 0; i < sk_SSL_CIPHER_num(sk) && totlen < maxlen; i++) {
        const SSL_CIPHER *c;

        c = sk_SSL_CIPHER_value(sk, i);
        /* Skip disabled ciphers */
        if (ssl_cipher_disabled(s, c, SSL_SECOP_CIPHER_SUPPORTED, 0))
            continue;

        if (!ssl->method->put_cipher_by_char(c, pkt, &len)) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            return 0;
        }

        /* Sanity check that the maximum version we offer has ciphers enabled */
        if (!maxverok) {
            int minproto = SSL_CONNECTION_IS_DTLS(s) ? c->min_dtls : c->min_tls;
            int maxproto = SSL_CONNECTION_IS_DTLS(s) ? c->max_dtls : c->max_tls;

            if (ssl_version_cmp(s, maxproto, s->s3.tmp.max_ver) >= 0
                    && ssl_version_cmp(s, minproto, s->s3.tmp.max_ver) <= 0)
                maxverok = 1;
        }

        totlen += len;
    }

    if (totlen == 0 || !maxverok) {
        const char *maxvertext =
            !maxverok
            ? "No ciphers enabled for max supported SSL/TLS version"
            : NULL;

        SSLfatal_data(s, SSL_AD_INTERNAL_ERROR, SSL_R_NO_CIPHERS_AVAILABLE,
                      maxvertext);
        return 0;
    }

    if (totlen != 0) {
        if (empty_reneg_info_scsv) {
            static const SSL_CIPHER scsv = {
                0, NULL, NULL, SSL3_CK_SCSV, 0, 0, 0, 0, 0, 0, 0, 0, 0
            };
            if (!ssl->method->put_cipher_by_char(&scsv, pkt, &len)) {
                SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
                return 0;
            }
        }
        if (s->mode & SSL_MODE_SEND_FALLBACK_SCSV) {
            static const SSL_CIPHER scsv = {
                0, NULL, NULL, SSL3_CK_FALLBACK_SCSV, 0, 0, 0, 0, 0, 0, 0, 0, 0
            };
            if (!ssl->method->put_cipher_by_char(&scsv, pkt, &len)) {
                SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
                return 0;
            }
        }
    }

    return 1;
}

CON_FUNC_RETURN tls_construct_end_of_early_data(SSL_CONNECTION *s, WPACKET *pkt)
{
    if (s->early_data_state != SSL_EARLY_DATA_WRITE_RETRY
            && s->early_data_state != SSL_EARLY_DATA_FINISHED_WRITING) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return CON_FUNC_ERROR;
    }

    s->early_data_state = SSL_EARLY_DATA_FINISHED_WRITING;
    return CON_FUNC_SUCCESS;
}
