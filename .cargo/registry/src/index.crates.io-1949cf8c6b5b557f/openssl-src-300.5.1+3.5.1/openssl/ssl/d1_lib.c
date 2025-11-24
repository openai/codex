/*
 * Copyright 2005-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/e_os.h"
#include "internal/e_winsock.h"          /* struct timeval for DTLS_CTRL_GET_TIMEOUT */
#include <stdio.h>
#include <openssl/objects.h>
#include <openssl/rand.h>
#include "ssl_local.h"
#include "internal/time.h"
#include "internal/ssl_unwrap.h"

static int dtls1_handshake_write(SSL_CONNECTION *s);
static size_t dtls1_link_min_mtu(void);

/* XDTLS:  figure out the right values */
static const size_t g_probable_mtu[] = { 1500, 512, 256 };

const SSL3_ENC_METHOD DTLSv1_enc_data = {
    tls1_setup_key_block,
    tls1_generate_master_secret,
    tls1_change_cipher_state,
    tls1_final_finish_mac,
    TLS_MD_CLIENT_FINISH_CONST, TLS_MD_CLIENT_FINISH_CONST_SIZE,
    TLS_MD_SERVER_FINISH_CONST, TLS_MD_SERVER_FINISH_CONST_SIZE,
    tls1_alert_code,
    tls1_export_keying_material,
    SSL_ENC_FLAG_DTLS,
    dtls1_set_handshake_header,
    dtls1_close_construct_packet,
    dtls1_handshake_write
};

const SSL3_ENC_METHOD DTLSv1_2_enc_data = {
    tls1_setup_key_block,
    tls1_generate_master_secret,
    tls1_change_cipher_state,
    tls1_final_finish_mac,
    TLS_MD_CLIENT_FINISH_CONST, TLS_MD_CLIENT_FINISH_CONST_SIZE,
    TLS_MD_SERVER_FINISH_CONST, TLS_MD_SERVER_FINISH_CONST_SIZE,
    tls1_alert_code,
    tls1_export_keying_material,
    SSL_ENC_FLAG_DTLS | SSL_ENC_FLAG_SIGALGS
        | SSL_ENC_FLAG_SHA256_PRF | SSL_ENC_FLAG_TLS1_2_CIPHERS,
    dtls1_set_handshake_header,
    dtls1_close_construct_packet,
    dtls1_handshake_write
};

OSSL_TIME dtls1_default_timeout(void)
{
    /*
     * 2 hours, the 24 hours mentioned in the DTLSv1 spec is way too long for
     * http, the cache would over fill
     */
    return ossl_seconds2time(60 * 60 * 2);
}

int dtls1_new(SSL *ssl)
{
    DTLS1_STATE *d1;
    SSL_CONNECTION *s = SSL_CONNECTION_FROM_SSL_ONLY(ssl);

    if (s == NULL)
        return 0;

    if (!DTLS_RECORD_LAYER_new(&s->rlayer)) {
        return 0;
    }

    if (!ssl3_new(ssl))
        return 0;
    if ((d1 = OPENSSL_zalloc(sizeof(*d1))) == NULL) {
        ssl3_free(ssl);
        return 0;
    }

    d1->buffered_messages = pqueue_new();
    d1->sent_messages = pqueue_new();

    if (s->server) {
        d1->cookie_len = sizeof(s->d1->cookie);
    }

    d1->link_mtu = 0;
    d1->mtu = 0;

    if (d1->buffered_messages == NULL || d1->sent_messages == NULL) {
        pqueue_free(d1->buffered_messages);
        pqueue_free(d1->sent_messages);
        OPENSSL_free(d1);
        ssl3_free(ssl);
        return 0;
    }

    s->d1 = d1;

    if (!ssl->method->ssl_clear(ssl))
        return 0;

    return 1;
}

static void dtls1_clear_queues(SSL_CONNECTION *s)
{
    dtls1_clear_received_buffer(s);
    dtls1_clear_sent_buffer(s);
}

void dtls1_clear_received_buffer(SSL_CONNECTION *s)
{
    pitem *item = NULL;
    hm_fragment *frag = NULL;

    while ((item = pqueue_pop(s->d1->buffered_messages)) != NULL) {
        frag = (hm_fragment *)item->data;
        dtls1_hm_fragment_free(frag);
        pitem_free(item);
    }
}

void dtls1_clear_sent_buffer(SSL_CONNECTION *s)
{
    pitem *item = NULL;
    hm_fragment *frag = NULL;

    while ((item = pqueue_pop(s->d1->sent_messages)) != NULL) {
        frag = (hm_fragment *)item->data;

        if (frag->msg_header.is_ccs
                && frag->msg_header.saved_retransmit_state.wrlmethod != NULL
                && s->rlayer.wrl != frag->msg_header.saved_retransmit_state.wrl) {
            /*
             * If we're freeing the CCS then we're done with the old wrl and it
             * can bee freed
             */
            frag->msg_header.saved_retransmit_state.wrlmethod->free(frag->msg_header.saved_retransmit_state.wrl);
        }

        dtls1_hm_fragment_free(frag);
        pitem_free(item);
    }
}


void dtls1_free(SSL *ssl)
{
    SSL_CONNECTION *s = SSL_CONNECTION_FROM_SSL_ONLY(ssl);

    if (s == NULL)
        return;

    if (s->d1 != NULL) {
        dtls1_clear_queues(s);
        pqueue_free(s->d1->buffered_messages);
        pqueue_free(s->d1->sent_messages);
    }

    DTLS_RECORD_LAYER_free(&s->rlayer);

    ssl3_free(ssl);

    OPENSSL_free(s->d1);
    s->d1 = NULL;
}

int dtls1_clear(SSL *ssl)
{
    pqueue *buffered_messages;
    pqueue *sent_messages;
    size_t mtu;
    size_t link_mtu;

    SSL_CONNECTION *s = SSL_CONNECTION_FROM_SSL_ONLY(ssl);

    if (s == NULL)
        return 0;

    DTLS_RECORD_LAYER_clear(&s->rlayer);

    if (s->d1) {
        DTLS_timer_cb timer_cb = s->d1->timer_cb;

        buffered_messages = s->d1->buffered_messages;
        sent_messages = s->d1->sent_messages;
        mtu = s->d1->mtu;
        link_mtu = s->d1->link_mtu;

        dtls1_clear_queues(s);

        memset(s->d1, 0, sizeof(*s->d1));

        /* Restore the timer callback from previous state */
        s->d1->timer_cb = timer_cb;

        if (s->server) {
            s->d1->cookie_len = sizeof(s->d1->cookie);
        }

        if (SSL_get_options(ssl) & SSL_OP_NO_QUERY_MTU) {
            s->d1->mtu = mtu;
            s->d1->link_mtu = link_mtu;
        }

        s->d1->buffered_messages = buffered_messages;
        s->d1->sent_messages = sent_messages;
    }

    if (!ssl3_clear(ssl))
        return 0;

    if (ssl->method->version == DTLS_ANY_VERSION)
        s->version = DTLS_MAX_VERSION_INTERNAL;
#ifndef OPENSSL_NO_DTLS1_METHOD
    else if (s->options & SSL_OP_CISCO_ANYCONNECT)
        s->client_version = s->version = DTLS1_BAD_VER;
#endif
    else
        s->version = ssl->method->version;

    return 1;
}

long dtls1_ctrl(SSL *ssl, int cmd, long larg, void *parg)
{
    int ret = 0;
    OSSL_TIME t;
    SSL_CONNECTION *s = SSL_CONNECTION_FROM_SSL_ONLY(ssl);

    if (s == NULL)
        return 0;

    switch (cmd) {
    case DTLS_CTRL_GET_TIMEOUT:
        if (dtls1_get_timeout(s, &t)) {
            *(struct timeval *)parg = ossl_time_to_timeval(t);
            ret = 1;
        }
        break;
    case DTLS_CTRL_HANDLE_TIMEOUT:
        ret = dtls1_handle_timeout(s);
        break;
    case DTLS_CTRL_SET_LINK_MTU:
        if (larg < (long)dtls1_link_min_mtu())
            return 0;
        s->d1->link_mtu = larg;
        return 1;
    case DTLS_CTRL_GET_LINK_MIN_MTU:
        return (long)dtls1_link_min_mtu();
    case SSL_CTRL_SET_MTU:
        /*
         *  We may not have a BIO set yet so can't call dtls1_min_mtu()
         *  We'll have to make do with dtls1_link_min_mtu() and max overhead
         */
        if (larg < (long)dtls1_link_min_mtu() - DTLS1_MAX_MTU_OVERHEAD)
            return 0;
        s->d1->mtu = larg;
        return larg;
    default:
        ret = ssl3_ctrl(ssl, cmd, larg, parg);
        break;
    }
    return ret;
}

static void dtls1_bio_set_next_timeout(BIO *bio, const DTLS1_STATE *d1)
{
    struct timeval tv = ossl_time_to_timeval(d1->next_timeout);

    BIO_ctrl(bio, BIO_CTRL_DGRAM_SET_NEXT_TIMEOUT, 0, &tv);
}

void dtls1_start_timer(SSL_CONNECTION *s)
{
    OSSL_TIME duration;
    SSL *ssl = SSL_CONNECTION_GET_SSL(s);

#ifndef OPENSSL_NO_SCTP
    /* Disable timer for SCTP */
    if (BIO_dgram_is_sctp(SSL_get_wbio(ssl))) {
        s->d1->next_timeout = ossl_time_zero();
        return;
    }
#endif

    /*
     * If timer is not set, initialize duration with 1 second or
     * a user-specified value if the timer callback is installed.
     */
    if (ossl_time_is_zero(s->d1->next_timeout)) {
        if (s->d1->timer_cb != NULL)
            s->d1->timeout_duration_us = s->d1->timer_cb(ssl, 0);
        else
            s->d1->timeout_duration_us = 1000000;
    }

    /* Set timeout to current time plus duration */
    duration = ossl_us2time(s->d1->timeout_duration_us);
    s->d1->next_timeout = ossl_time_add(ossl_time_now(), duration);

    /* set s->d1->next_timeout into ssl->rbio interface */
    dtls1_bio_set_next_timeout(SSL_get_rbio(ssl), s->d1);
}

int dtls1_get_timeout(const SSL_CONNECTION *s, OSSL_TIME *timeleft)
{
    OSSL_TIME timenow;

    /* If no timeout is set, just return NULL */
    if (ossl_time_is_zero(s->d1->next_timeout))
        return 0;

    /* Get current time */
    timenow = ossl_time_now();

    /*
     * If timer already expired or if remaining time is less than 15 ms,
     * set it to 0 to prevent issues because of small divergences with
     * socket timeouts.
     */
    *timeleft = ossl_time_subtract(s->d1->next_timeout, timenow);
    if (ossl_time_compare(*timeleft, ossl_ms2time(15)) <= 0)
        *timeleft = ossl_time_zero();
    return 1;
}

int dtls1_is_timer_expired(SSL_CONNECTION *s)
{
    OSSL_TIME timeleft;

    /* Get time left until timeout, return false if no timer running */
    if (!dtls1_get_timeout(s, &timeleft))
        return 0;

    /* Return false if timer is not expired yet */
    if (!ossl_time_is_zero(timeleft))
        return 0;

    /* Timer expired, so return true */
    return 1;
}

static void dtls1_double_timeout(SSL_CONNECTION *s)
{
    s->d1->timeout_duration_us *= 2;
    if (s->d1->timeout_duration_us > 60000000)
        s->d1->timeout_duration_us = 60000000;
}

void dtls1_stop_timer(SSL_CONNECTION *s)
{
    /* Reset everything */
    s->d1->timeout_num_alerts = 0;
    s->d1->next_timeout = ossl_time_zero();
    s->d1->timeout_duration_us = 1000000;
    dtls1_bio_set_next_timeout(s->rbio, s->d1);
    /* Clear retransmission buffer */
    dtls1_clear_sent_buffer(s);
}

int dtls1_check_timeout_num(SSL_CONNECTION *s)
{
    size_t mtu;
    SSL *ssl = SSL_CONNECTION_GET_SSL(s);

    s->d1->timeout_num_alerts++;

    /* Reduce MTU after 2 unsuccessful retransmissions */
    if (s->d1->timeout_num_alerts > 2
        && !(SSL_get_options(ssl) & SSL_OP_NO_QUERY_MTU)) {
        mtu =
            BIO_ctrl(SSL_get_wbio(ssl), BIO_CTRL_DGRAM_GET_FALLBACK_MTU, 0, NULL);
        if (mtu < s->d1->mtu)
            s->d1->mtu = mtu;
    }

    if (s->d1->timeout_num_alerts > DTLS1_TMO_ALERT_COUNT) {
        /* fail the connection, enough alerts have been sent */
        SSLfatal(s, SSL_AD_NO_ALERT, SSL_R_READ_TIMEOUT_EXPIRED);
        return -1;
    }

    return 0;
}

int dtls1_handle_timeout(SSL_CONNECTION *s)
{
    /* if no timer is expired, don't do anything */
    if (!dtls1_is_timer_expired(s)) {
        return 0;
    }

    if (s->d1->timer_cb != NULL)
        s->d1->timeout_duration_us = s->d1->timer_cb(SSL_CONNECTION_GET_USER_SSL(s),
                                                     s->d1->timeout_duration_us);
    else
        dtls1_double_timeout(s);

    if (dtls1_check_timeout_num(s) < 0) {
        /* SSLfatal() already called */
        return -1;
    }

    dtls1_start_timer(s);
    /* Calls SSLfatal() if required */
    return dtls1_retransmit_buffered_messages(s);
}

#define LISTEN_SUCCESS              2
#define LISTEN_SEND_VERIFY_REQUEST  1

#ifndef OPENSSL_NO_SOCK
int DTLSv1_listen(SSL *ssl, BIO_ADDR *client)
{
    int next, n, ret = 0;
    unsigned char cookie[DTLS1_COOKIE_LENGTH];
    unsigned char seq[SEQ_NUM_SIZE];
    const unsigned char *data;
    unsigned char *buf = NULL, *wbuf;
    size_t fragoff, fraglen, msglen;
    unsigned int rectype, versmajor, versminor, msgseq, msgtype, clientvers, cookielen;
    BIO *rbio, *wbio;
    BIO_ADDR *tmpclient = NULL;
    PACKET pkt, msgpkt, msgpayload, session, cookiepkt;
    SSL_CONNECTION *s = SSL_CONNECTION_FROM_SSL_ONLY(ssl);

    if (s == NULL)
        return -1;

    if (s->handshake_func == NULL) {
        /* Not properly initialized yet */
        SSL_set_accept_state(ssl);
    }

    /* Ensure there is no state left over from a previous invocation */
    if (!SSL_clear(ssl))
        return -1;

    ERR_clear_error();

    rbio = SSL_get_rbio(ssl);
    wbio = SSL_get_wbio(ssl);

    if (!rbio || !wbio) {
        ERR_raise(ERR_LIB_SSL, SSL_R_BIO_NOT_SET);
        return -1;
    }

    /*
     * Note: This check deliberately excludes DTLS1_BAD_VER because that version
     * requires the MAC to be calculated *including* the first ClientHello
     * (without the cookie). Since DTLSv1_listen is stateless that cannot be
     * supported. DTLS1_BAD_VER must use cookies in a stateful manner (e.g. via
     * SSL_accept)
     */
    if ((s->version & 0xff00) != (DTLS1_VERSION & 0xff00)) {
        ERR_raise(ERR_LIB_SSL, SSL_R_UNSUPPORTED_SSL_VERSION);
        return -1;
    }

    buf = OPENSSL_malloc(DTLS1_RT_HEADER_LENGTH + SSL3_RT_MAX_PLAIN_LENGTH);
    if (buf == NULL)
        return -1;
    wbuf = OPENSSL_malloc(DTLS1_RT_HEADER_LENGTH + SSL3_RT_MAX_PLAIN_LENGTH);
    if (wbuf == NULL) {
        OPENSSL_free(buf);
        return -1;
    }

    do {
        /* Get a packet */

        clear_sys_error();
        n = BIO_read(rbio, buf, SSL3_RT_MAX_PLAIN_LENGTH
                                + DTLS1_RT_HEADER_LENGTH);
        if (n <= 0) {
            if (BIO_should_retry(rbio)) {
                /* Non-blocking IO */
                goto end;
            }
            ret = -1;
            goto end;
        }

        if (!PACKET_buf_init(&pkt, buf, n)) {
            ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
            ret = -1;
            goto end;
        }

        /*
         * Parse the received record. If there are any problems with it we just
         * dump it - with no alert. RFC6347 says this "Unlike TLS, DTLS is
         * resilient in the face of invalid records (e.g., invalid formatting,
         * length, MAC, etc.).  In general, invalid records SHOULD be silently
         * discarded, thus preserving the association; however, an error MAY be
         * logged for diagnostic purposes."
         */

        /* this packet contained a partial record, dump it */
        if (n < DTLS1_RT_HEADER_LENGTH) {
            ERR_raise(ERR_LIB_SSL, SSL_R_RECORD_TOO_SMALL);
            goto end;
        }

        /* Get the record header */
        if (!PACKET_get_1(&pkt, &rectype)
            || !PACKET_get_1(&pkt, &versmajor)
            || !PACKET_get_1(&pkt, &versminor)) {
            ERR_raise(ERR_LIB_SSL, SSL_R_LENGTH_MISMATCH);
            goto end;
        }

        if (s->msg_callback)
            s->msg_callback(0, (versmajor << 8) | versminor, SSL3_RT_HEADER, buf,
                            DTLS1_RT_HEADER_LENGTH, ssl, s->msg_callback_arg);

        if (rectype != SSL3_RT_HANDSHAKE) {
            ERR_raise(ERR_LIB_SSL, SSL_R_UNEXPECTED_MESSAGE);
            goto end;
        }

        /*
         * Check record version number. We only check that the major version is
         * the same.
         */
        if (versmajor != DTLS1_VERSION_MAJOR) {
            ERR_raise(ERR_LIB_SSL, SSL_R_BAD_PROTOCOL_VERSION_NUMBER);
            goto end;
        }

        /* Save the sequence number: 64 bits, with top 2 bytes = epoch */
        if (!PACKET_copy_bytes(&pkt, seq, SEQ_NUM_SIZE)
            || !PACKET_get_length_prefixed_2(&pkt, &msgpkt)) {
            ERR_raise(ERR_LIB_SSL, SSL_R_LENGTH_MISMATCH);
            goto end;
        }
        /*
         * We allow data remaining at the end of the packet because there could
         * be a second record (but we ignore it)
         */

        /* This is an initial ClientHello so the epoch has to be 0 */
        if (seq[0] != 0 || seq[1] != 0) {
            ERR_raise(ERR_LIB_SSL, SSL_R_UNEXPECTED_MESSAGE);
            goto end;
        }

        /* Get a pointer to the raw message for the later callback */
        data = PACKET_data(&msgpkt);

        /* Finished processing the record header, now process the message */
        if (!PACKET_get_1(&msgpkt, &msgtype)
            || !PACKET_get_net_3_len(&msgpkt, &msglen)
            || !PACKET_get_net_2(&msgpkt, &msgseq)
            || !PACKET_get_net_3_len(&msgpkt, &fragoff)
            || !PACKET_get_net_3_len(&msgpkt, &fraglen)
            || !PACKET_get_sub_packet(&msgpkt, &msgpayload, fraglen)
            || PACKET_remaining(&msgpkt) != 0) {
            ERR_raise(ERR_LIB_SSL, SSL_R_LENGTH_MISMATCH);
            goto end;
        }

        if (msgtype != SSL3_MT_CLIENT_HELLO) {
            ERR_raise(ERR_LIB_SSL, SSL_R_UNEXPECTED_MESSAGE);
            goto end;
        }

        /* Message sequence number can only be 0 or 1 */
        if (msgseq > 2) {
            ERR_raise(ERR_LIB_SSL, SSL_R_INVALID_SEQUENCE_NUMBER);
            goto end;
        }

        /*
         * We don't support fragment reassembly for ClientHellos whilst
         * listening because that would require server side state (which is
         * against the whole point of the ClientHello/HelloVerifyRequest
         * mechanism). Instead we only look at the first ClientHello fragment
         * and require that the cookie must be contained within it.
         */
        if (fragoff != 0 || fraglen > msglen) {
            /* Non initial ClientHello fragment (or bad fragment) */
            ERR_raise(ERR_LIB_SSL, SSL_R_FRAGMENTED_CLIENT_HELLO);
            goto end;
        }

        if (s->msg_callback)
            s->msg_callback(0, s->version, SSL3_RT_HANDSHAKE, data,
                            fraglen + DTLS1_HM_HEADER_LENGTH, ssl,
                            s->msg_callback_arg);

        if (!PACKET_get_net_2(&msgpayload, &clientvers)) {
            ERR_raise(ERR_LIB_SSL, SSL_R_LENGTH_MISMATCH);
            goto end;
        }

        /*
         * Verify client version is supported
         */
        if (DTLS_VERSION_LT(clientvers, (unsigned int)ssl->method->version) &&
            ssl->method->version != DTLS_ANY_VERSION) {
            ERR_raise(ERR_LIB_SSL, SSL_R_WRONG_VERSION_NUMBER);
            goto end;
        }

        if (!PACKET_forward(&msgpayload, SSL3_RANDOM_SIZE)
            || !PACKET_get_length_prefixed_1(&msgpayload, &session)
            || !PACKET_get_length_prefixed_1(&msgpayload, &cookiepkt)) {
            /*
             * Could be malformed or the cookie does not fit within the initial
             * ClientHello fragment. Either way we can't handle it.
             */
            ERR_raise(ERR_LIB_SSL, SSL_R_LENGTH_MISMATCH);
            goto end;
        }

        /*
         * Check if we have a cookie or not. If not we need to send a
         * HelloVerifyRequest.
         */
        if (PACKET_remaining(&cookiepkt) == 0) {
            next = LISTEN_SEND_VERIFY_REQUEST;
        } else {
            /*
             * We have a cookie, so lets check it.
             */
            if (ssl->ctx->app_verify_cookie_cb == NULL) {
                ERR_raise(ERR_LIB_SSL, SSL_R_NO_VERIFY_COOKIE_CALLBACK);
                /* This is fatal */
                ret = -1;
                goto end;
            }
            if (ssl->ctx->app_verify_cookie_cb(ssl, PACKET_data(&cookiepkt),
                    (unsigned int)PACKET_remaining(&cookiepkt)) == 0) {
                /*
                 * We treat invalid cookies in the same was as no cookie as
                 * per RFC6347
                 */
                next = LISTEN_SEND_VERIFY_REQUEST;
            } else {
                /* Cookie verification succeeded */
                next = LISTEN_SUCCESS;
            }
        }

        if (next == LISTEN_SEND_VERIFY_REQUEST) {
            WPACKET wpkt;
            unsigned int version;
            size_t wreclen;

            /*
             * There was no cookie in the ClientHello so we need to send a
             * HelloVerifyRequest. If this fails we do not worry about trying
             * to resend, we just drop it.
             */

            /* Generate the cookie */
            if (ssl->ctx->app_gen_cookie_cb == NULL ||
                ssl->ctx->app_gen_cookie_cb(ssl, cookie, &cookielen) == 0 ||
                cookielen > 255) {
                ERR_raise(ERR_LIB_SSL, SSL_R_COOKIE_GEN_CALLBACK_FAILURE);
                /* This is fatal */
                ret = -1;
                goto end;
            }

            /*
             * Special case: for hello verify request, client version 1.0 and we
             * haven't decided which version to use yet send back using version
             * 1.0 header: otherwise some clients will ignore it.
             */
            version = (ssl->method->version == DTLS_ANY_VERSION) ? DTLS1_VERSION
                                                                 : s->version;

            /* Construct the record and message headers */
            if (!WPACKET_init_static_len(&wpkt,
                                         wbuf,
                                         ssl_get_max_send_fragment(s)
                                         + DTLS1_RT_HEADER_LENGTH,
                                         0)
                    || !WPACKET_put_bytes_u8(&wpkt, SSL3_RT_HANDSHAKE)
                    || !WPACKET_put_bytes_u16(&wpkt, version)
                       /*
                        * Record sequence number is always the same as in the
                        * received ClientHello
                        */
                    || !WPACKET_memcpy(&wpkt, seq, SEQ_NUM_SIZE)
                       /* End of record, start sub packet for message */
                    || !WPACKET_start_sub_packet_u16(&wpkt)
                       /* Message type */
                    || !WPACKET_put_bytes_u8(&wpkt,
                                             DTLS1_MT_HELLO_VERIFY_REQUEST)
                       /*
                        * Message length - doesn't follow normal TLS convention:
                        * the length isn't the last thing in the message header.
                        * We'll need to fill this in later when we know the
                        * length. Set it to zero for now
                        */
                    || !WPACKET_put_bytes_u24(&wpkt, 0)
                       /*
                        * Message sequence number is always 0 for a
                        * HelloVerifyRequest
                        */
                    || !WPACKET_put_bytes_u16(&wpkt, 0)
                       /*
                        * We never fragment a HelloVerifyRequest, so fragment
                        * offset is 0
                        */
                    || !WPACKET_put_bytes_u24(&wpkt, 0)
                       /*
                        * Fragment length is the same as message length, but
                        * this *is* the last thing in the message header so we
                        * can just start a sub-packet. No need to come back
                        * later for this one.
                        */
                    || !WPACKET_start_sub_packet_u24(&wpkt)
                       /* Create the actual HelloVerifyRequest body */
                    || !dtls_raw_hello_verify_request(&wpkt, cookie, cookielen)
                       /* Close message body */
                    || !WPACKET_close(&wpkt)
                       /* Close record body */
                    || !WPACKET_close(&wpkt)
                    || !WPACKET_get_total_written(&wpkt, &wreclen)
                    || !WPACKET_finish(&wpkt)) {
                ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
                WPACKET_cleanup(&wpkt);
                /* This is fatal */
                ret = -1;
                goto end;
            }

            /*
             * Fix up the message len in the message header. Its the same as the
             * fragment len which has been filled in by WPACKET, so just copy
             * that. Destination for the message len is after the record header
             * plus one byte for the message content type. The source is the
             * last 3 bytes of the message header
             */
            memcpy(&wbuf[DTLS1_RT_HEADER_LENGTH + 1],
                   &wbuf[DTLS1_RT_HEADER_LENGTH + DTLS1_HM_HEADER_LENGTH - 3],
                   3);

            if (s->msg_callback)
                s->msg_callback(1, 0, SSL3_RT_HEADER, buf,
                                DTLS1_RT_HEADER_LENGTH, ssl,
                                s->msg_callback_arg);

            if ((tmpclient = BIO_ADDR_new()) == NULL) {
                ERR_raise(ERR_LIB_SSL, ERR_R_BIO_LIB);
                goto end;
            }

            /*
             * This is unnecessary if rbio and wbio are one and the same - but
             * maybe they're not. We ignore errors here - some BIOs do not
             * support this.
             */
            if (BIO_dgram_get_peer(rbio, tmpclient) > 0) {
                (void)BIO_dgram_set_peer(wbio, tmpclient);
            }
            BIO_ADDR_free(tmpclient);
            tmpclient = NULL;

            if (BIO_write(wbio, wbuf, wreclen) < (int)wreclen) {
                if (BIO_should_retry(wbio)) {
                    /*
                     * Non-blocking IO...but we're stateless, so we're just
                     * going to drop this packet.
                     */
                    goto end;
                }
                ret = -1;
                goto end;
            }

            if (BIO_flush(wbio) <= 0) {
                if (BIO_should_retry(wbio)) {
                    /*
                     * Non-blocking IO...but we're stateless, so we're just
                     * going to drop this packet.
                     */
                    goto end;
                }
                ret = -1;
                goto end;
            }
        }
    } while (next != LISTEN_SUCCESS);

    /*
     * Set expected sequence numbers to continue the handshake.
     */
    s->d1->handshake_read_seq = 1;
    s->d1->handshake_write_seq = 1;
    s->d1->next_handshake_write_seq = 1;
    s->rlayer.wrlmethod->increment_sequence_ctr(s->rlayer.wrl);

    /*
     * We are doing cookie exchange, so make sure we set that option in the
     * SSL object
     */
    SSL_set_options(ssl, SSL_OP_COOKIE_EXCHANGE);

    /*
     * Tell the state machine that we've done the initial hello verify
     * exchange
     */
    ossl_statem_set_hello_verify_done(s);

    /*
     * Some BIOs may not support this. If we fail we clear the client address
     */
    if (BIO_dgram_get_peer(rbio, client) <= 0)
        BIO_ADDR_clear(client);

    /* Buffer the record for use by the record layer */
    if (BIO_write(s->rlayer.rrlnext, buf, n) != n) {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        ret = -1;
        goto end;
    }

    /*
     * Reset the record layer - but this time we can use the record we just
     * buffered in s->rlayer.rrlnext
     */
    if (!ssl_set_new_record_layer(s,
                                  DTLS_ANY_VERSION,
                                  OSSL_RECORD_DIRECTION_READ,
                                  OSSL_RECORD_PROTECTION_LEVEL_NONE, NULL, 0,
                                  NULL, 0, NULL, 0, NULL,  0, NULL, 0,
                                  NID_undef, NULL, NULL, NULL)) {
        /* SSLfatal already called */
        ret = -1;
        goto end;
    }

    ret = 1;
 end:
    BIO_ADDR_free(tmpclient);
    OPENSSL_free(buf);
    OPENSSL_free(wbuf);
    return ret;
}
#endif

static int dtls1_handshake_write(SSL_CONNECTION *s)
{
    return dtls1_do_write(s, SSL3_RT_HANDSHAKE);
}

int dtls1_shutdown(SSL *s)
{
    int ret;
#ifndef OPENSSL_NO_SCTP
    BIO *wbio;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL_ONLY(s);

    if (s == NULL)
        return -1;

    wbio = SSL_get_wbio(s);
    if (wbio != NULL && BIO_dgram_is_sctp(wbio) &&
        !(sc->shutdown & SSL_SENT_SHUTDOWN)) {
        ret = BIO_dgram_sctp_wait_for_dry(wbio);
        if (ret < 0)
            return -1;

        if (ret == 0)
            BIO_ctrl(SSL_get_wbio(s), BIO_CTRL_DGRAM_SCTP_SAVE_SHUTDOWN, 1,
                     NULL);
    }
#endif
    ret = ssl3_shutdown(s);
#ifndef OPENSSL_NO_SCTP
    BIO_ctrl(SSL_get_wbio(s), BIO_CTRL_DGRAM_SCTP_SAVE_SHUTDOWN, 0, NULL);
#endif
    return ret;
}

int dtls1_query_mtu(SSL_CONNECTION *s)
{
    SSL *ssl = SSL_CONNECTION_GET_SSL(s);

    if (s->d1->link_mtu) {
        s->d1->mtu =
            s->d1->link_mtu - BIO_dgram_get_mtu_overhead(SSL_get_wbio(ssl));
        s->d1->link_mtu = 0;
    }

    /* AHA!  Figure out the MTU, and stick to the right size */
    if (s->d1->mtu < dtls1_min_mtu(s)) {
        if (!(SSL_get_options(ssl) & SSL_OP_NO_QUERY_MTU)) {
            s->d1->mtu =
                BIO_ctrl(SSL_get_wbio(ssl), BIO_CTRL_DGRAM_QUERY_MTU, 0, NULL);

            /*
             * I've seen the kernel return bogus numbers when it doesn't know
             * (initial write), so just make sure we have a reasonable number
             */
            if (s->d1->mtu < dtls1_min_mtu(s)) {
                /* Set to min mtu */
                s->d1->mtu = dtls1_min_mtu(s);
                BIO_ctrl(SSL_get_wbio(ssl), BIO_CTRL_DGRAM_SET_MTU,
                         (long)s->d1->mtu, NULL);
            }
        } else
            return 0;
    }
    return 1;
}

static size_t dtls1_link_min_mtu(void)
{
    return (g_probable_mtu[(sizeof(g_probable_mtu) /
                            sizeof(g_probable_mtu[0])) - 1]);
}

size_t dtls1_min_mtu(SSL_CONNECTION *s)
{
    SSL *ssl = SSL_CONNECTION_GET_SSL(s);

    return dtls1_link_min_mtu() - BIO_dgram_get_mtu_overhead(SSL_get_wbio(ssl));
}

size_t DTLS_get_data_mtu(const SSL *ssl)
{
    size_t mac_overhead, int_overhead, blocksize, ext_overhead;
    const SSL_CIPHER *ciph = SSL_get_current_cipher(ssl);
    size_t mtu;
    const SSL_CONNECTION *s = SSL_CONNECTION_FROM_CONST_SSL_ONLY(ssl);

    if (s == NULL)
        return 0;

    mtu = s->d1->mtu;

    if (ciph == NULL)
        return 0;

    if (!ssl_cipher_get_overhead(ciph, &mac_overhead, &int_overhead,
                                 &blocksize, &ext_overhead))
        return 0;

    if (SSL_READ_ETM(s))
        ext_overhead += mac_overhead;
    else
        int_overhead += mac_overhead;

    /* Subtract external overhead (e.g. IV/nonce, separate MAC) */
    if (ext_overhead + DTLS1_RT_HEADER_LENGTH >= mtu)
        return 0;
    mtu -= ext_overhead + DTLS1_RT_HEADER_LENGTH;

    /* Round encrypted payload down to cipher block size (for CBC etc.)
     * No check for overflow since 'mtu % blocksize' cannot exceed mtu. */
    if (blocksize)
        mtu -= (mtu % blocksize);

    /* Subtract internal overhead (e.g. CBC padding len byte) */
    if (int_overhead >= mtu)
        return 0;
    mtu -= int_overhead;

    return mtu;
}

void DTLS_set_timer_cb(SSL *ssl, DTLS_timer_cb cb)
{
    SSL_CONNECTION *s = SSL_CONNECTION_FROM_SSL_ONLY(ssl);

    if (s == NULL)
        return;

    s->d1->timer_cb = cb;
}
