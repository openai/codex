/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/e_os.h"

#include <stdio.h>
#include <limits.h>
#include <errno.h>
#include <assert.h>
#include "../ssl_local.h"
#include "../quic/quic_local.h"
#include <openssl/evp.h>
#include <openssl/buffer.h>
#include <openssl/rand.h>
#include <openssl/core_names.h>
#include "record_local.h"
#include "internal/packet.h"
#include "internal/comp.h"
#include "internal/ssl_unwrap.h"

void RECORD_LAYER_init(RECORD_LAYER *rl, SSL_CONNECTION *s)
{
    rl->s = s;
}

int RECORD_LAYER_clear(RECORD_LAYER *rl)
{
    int ret = 1;

    /* Clear any buffered records we no longer need */
    while (rl->curr_rec < rl->num_recs)
        ret &= ssl_release_record(rl->s,
                                  &(rl->tlsrecs[rl->curr_rec++]),
                                  0);


    rl->wnum = 0;
    memset(rl->handshake_fragment, 0, sizeof(rl->handshake_fragment));
    rl->handshake_fragment_len = 0;
    rl->wpend_tot = 0;
    rl->wpend_type = 0;
    rl->wpend_buf = NULL;
    rl->alert_count = 0;
    rl->num_recs = 0;
    rl->curr_rec = 0;

    BIO_free(rl->rrlnext);
    rl->rrlnext = NULL;

    if (rl->rrlmethod != NULL)
        rl->rrlmethod->free(rl->rrl); /* Ignore return value */
    if (rl->wrlmethod != NULL)
        rl->wrlmethod->free(rl->wrl); /* Ignore return value */
    BIO_free(rl->rrlnext);
    rl->rrlmethod = NULL;
    rl->wrlmethod = NULL;
    rl->rrlnext = NULL;
    rl->rrl = NULL;
    rl->wrl = NULL;

    if (rl->d)
        DTLS_RECORD_LAYER_clear(rl);

    return ret;
}

int RECORD_LAYER_reset(RECORD_LAYER *rl)
{
    int ret;

    ret = RECORD_LAYER_clear(rl);

    /* We try and reset both record layers even if one fails */
    ret &= ssl_set_new_record_layer(rl->s,
                                    SSL_CONNECTION_IS_DTLS(rl->s)
                                        ? DTLS_ANY_VERSION : TLS_ANY_VERSION,
                                    OSSL_RECORD_DIRECTION_READ,
                                    OSSL_RECORD_PROTECTION_LEVEL_NONE, NULL, 0,
                                    NULL, 0, NULL, 0, NULL,  0, NULL, 0,
                                    NID_undef, NULL, NULL, NULL);

    ret &= ssl_set_new_record_layer(rl->s,
                                    SSL_CONNECTION_IS_DTLS(rl->s)
                                        ? DTLS_ANY_VERSION : TLS_ANY_VERSION,
                                    OSSL_RECORD_DIRECTION_WRITE,
                                    OSSL_RECORD_PROTECTION_LEVEL_NONE, NULL, 0,
                                    NULL, 0, NULL, 0, NULL,  0, NULL, 0,
                                    NID_undef, NULL, NULL, NULL);

    /* SSLfatal already called in the event of failure */
    return ret;
}

/* Checks if we have unprocessed read ahead data pending */
int RECORD_LAYER_read_pending(const RECORD_LAYER *rl)
{
    return rl->rrlmethod->unprocessed_read_pending(rl->rrl);
}

/* Checks if we have decrypted unread record data pending */
int RECORD_LAYER_processed_read_pending(const RECORD_LAYER *rl)
{
    return (rl->curr_rec < rl->num_recs)
           || rl->rrlmethod->processed_read_pending(rl->rrl);
}

int RECORD_LAYER_write_pending(const RECORD_LAYER *rl)
{
    return rl->wpend_tot > 0;
}

static uint32_t ossl_get_max_early_data(SSL_CONNECTION *s)
{
    uint32_t max_early_data;
    SSL_SESSION *sess = s->session;

    /*
     * If we are a client then we always use the max_early_data from the
     * session/psksession. Otherwise we go with the lowest out of the max early
     * data set in the session and the configured max_early_data.
     */
    if (!s->server && sess->ext.max_early_data == 0) {
        if (!ossl_assert(s->psksession != NULL
                         && s->psksession->ext.max_early_data > 0)) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            return 0;
        }
        sess = s->psksession;
    }

    if (!s->server)
        max_early_data = sess->ext.max_early_data;
    else if (s->ext.early_data != SSL_EARLY_DATA_ACCEPTED)
        max_early_data = s->recv_max_early_data;
    else
        max_early_data = s->recv_max_early_data < sess->ext.max_early_data
                         ? s->recv_max_early_data : sess->ext.max_early_data;

    return max_early_data;
}

static int ossl_early_data_count_ok(SSL_CONNECTION *s, size_t length,
                                    size_t overhead, int send)
{
    uint32_t max_early_data;

    max_early_data = ossl_get_max_early_data(s);

    if (max_early_data == 0) {
        SSLfatal(s, send ? SSL_AD_INTERNAL_ERROR : SSL_AD_UNEXPECTED_MESSAGE,
                 SSL_R_TOO_MUCH_EARLY_DATA);
        return 0;
    }

    /* If we are dealing with ciphertext we need to allow for the overhead */
    max_early_data += overhead;

    if (s->early_data_count + length > max_early_data) {
        SSLfatal(s, send ? SSL_AD_INTERNAL_ERROR : SSL_AD_UNEXPECTED_MESSAGE,
                 SSL_R_TOO_MUCH_EARLY_DATA);
        return 0;
    }
    s->early_data_count += length;

    return 1;
}

size_t ssl3_pending(const SSL *s)
{
    size_t i, num = 0;
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL)
        return 0;

    if (SSL_CONNECTION_IS_DTLS(sc)) {
        TLS_RECORD *rdata;
        pitem *item, *iter;

        iter = pqueue_iterator(sc->rlayer.d->buffered_app_data);
        while ((item = pqueue_next(&iter)) != NULL) {
            rdata = item->data;
            num += rdata->length;
        }
    }

    for (i = 0; i < sc->rlayer.num_recs; i++) {
        if (sc->rlayer.tlsrecs[i].type != SSL3_RT_APPLICATION_DATA)
            return num;
        num += sc->rlayer.tlsrecs[i].length;
    }

    num += sc->rlayer.rrlmethod->app_data_pending(sc->rlayer.rrl);

    return num;
}

void SSL_CTX_set_default_read_buffer_len(SSL_CTX *ctx, size_t len)
{
    ctx->default_read_buf_len = len;
}

void SSL_set_default_read_buffer_len(SSL *s, size_t len)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL || IS_QUIC(s))
        return;
    sc->rlayer.default_read_buf_len = len;
}

const char *SSL_rstate_string_long(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);
    const char *lng;

    if (sc == NULL)
        return NULL;

    if (sc->rlayer.rrlmethod == NULL || sc->rlayer.rrl == NULL)
        return "unknown";

    sc->rlayer.rrlmethod->get_state(sc->rlayer.rrl, NULL, &lng);

    return lng;
}

const char *SSL_rstate_string(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);
    const char *shrt;

    if (sc == NULL)
        return NULL;

    if (sc->rlayer.rrlmethod == NULL || sc->rlayer.rrl == NULL)
        return "unknown";

    sc->rlayer.rrlmethod->get_state(sc->rlayer.rrl, &shrt, NULL);

    return shrt;
}

static int tls_write_check_pending(SSL_CONNECTION *s, uint8_t type,
                                   const unsigned char *buf, size_t len)
{
    if (s->rlayer.wpend_tot == 0)
        return 0;

    /* We have pending data, so do some sanity checks */
    if ((s->rlayer.wpend_tot > len)
        || (!(s->mode & SSL_MODE_ACCEPT_MOVING_WRITE_BUFFER)
            && (s->rlayer.wpend_buf != buf))
        || (s->rlayer.wpend_type != type)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, SSL_R_BAD_WRITE_RETRY);
        return -1;
    }
    return 1;
}

/*
 * Call this to write data in records of type 'type' It will return <= 0 if
 * not all data has been sent or non-blocking IO.
 */
int ssl3_write_bytes(SSL *ssl, uint8_t type, const void *buf_, size_t len,
                     size_t *written)
{
    const unsigned char *buf = buf_;
    size_t tot;
    size_t n, max_send_fragment, split_send_fragment, maxpipes;
    int i;
    SSL_CONNECTION *s = SSL_CONNECTION_FROM_SSL_ONLY(ssl);
    OSSL_RECORD_TEMPLATE tmpls[SSL_MAX_PIPELINES];
    unsigned int recversion;

    if (s == NULL)
        return -1;

    s->rwstate = SSL_NOTHING;
    tot = s->rlayer.wnum;
    /*
     * ensure that if we end up with a smaller value of data to write out
     * than the original len from a write which didn't complete for
     * non-blocking I/O and also somehow ended up avoiding the check for
     * this in tls_write_check_pending/SSL_R_BAD_WRITE_RETRY as it must never be
     * possible to end up with (len-tot) as a large number that will then
     * promptly send beyond the end of the users buffer ... so we trap and
     * report the error in a way the user will notice
     */
    if ((len < s->rlayer.wnum)
        || ((s->rlayer.wpend_tot != 0)
            && (len < (s->rlayer.wnum + s->rlayer.wpend_tot)))) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, SSL_R_BAD_LENGTH);
        return -1;
    }

    if (s->early_data_state == SSL_EARLY_DATA_WRITING
            && !ossl_early_data_count_ok(s, len, 0, 1)) {
        /* SSLfatal() already called */
        return -1;
    }

    s->rlayer.wnum = 0;

    /*
     * If we are supposed to be sending a KeyUpdate or NewSessionTicket then go
     * into init unless we have writes pending - in which case we should finish
     * doing that first.
     */
    if (s->rlayer.wpend_tot == 0 && (s->key_update != SSL_KEY_UPDATE_NONE
                                     || s->ext.extra_tickets_expected > 0))
        ossl_statem_set_in_init(s, 1);

    /*
     * When writing early data on the server side we could be "in_init" in
     * between receiving the EoED and the CF - but we don't want to handle those
     * messages yet.
     */
    if (SSL_in_init(ssl) && !ossl_statem_get_in_handshake(s)
            && s->early_data_state != SSL_EARLY_DATA_UNAUTH_WRITING) {
        i = s->handshake_func(ssl);
        /* SSLfatal() already called */
        if (i < 0)
            return i;
        if (i == 0) {
            return -1;
        }
    }

    i = tls_write_check_pending(s, type, buf, len);
    if (i < 0) {
        /* SSLfatal() already called */
        return i;
    } else if (i > 0) {
        /* Retry needed */
        i = HANDLE_RLAYER_WRITE_RETURN(s,
                s->rlayer.wrlmethod->retry_write_records(s->rlayer.wrl));
        if (i <= 0) {
            s->rlayer.wnum = tot;
            return i;
        }
        tot += s->rlayer.wpend_tot;
        s->rlayer.wpend_tot = 0;
    } /* else no retry required */

    if (tot == 0) {
        /*
         * We've not previously sent any data for this write so memorize
         * arguments so that we can detect bad write retries later
         */
        s->rlayer.wpend_tot = 0;
        s->rlayer.wpend_type = type;
        s->rlayer.wpend_buf = buf;
    }

    if (tot == len) {           /* done? */
        *written = tot;
        return 1;
    }

    /* If we have an alert to send, lets send it */
    if (s->s3.alert_dispatch > 0) {
        i = ssl->method->ssl_dispatch_alert(ssl);
        if (i <= 0) {
            /* SSLfatal() already called if appropriate */
            s->rlayer.wnum = tot;
            return i;
        }
        /* if it went, fall through and send more stuff */
    }

    n = (len - tot);

    max_send_fragment = ssl_get_max_send_fragment(s);
    split_send_fragment = ssl_get_split_send_fragment(s);

    if (max_send_fragment == 0
            || split_send_fragment == 0
            || split_send_fragment > max_send_fragment) {
        /*
         * We should have prevented this when we set/get the split and max send
         * fragments so we shouldn't get here
         */
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return -1;
    }

    /*
     * Some servers hang if initial client hello is larger than 256 bytes
     * and record version number > TLS 1.0
     */
    recversion = (s->version == TLS1_3_VERSION) ? TLS1_2_VERSION : s->version;
    if (SSL_get_state(ssl) == TLS_ST_CW_CLNT_HELLO
            && !s->renegotiate
            && TLS1_get_version(ssl) > TLS1_VERSION
            && s->hello_retry_request == SSL_HRR_NONE)
        recversion = TLS1_VERSION;

    for (;;) {
        size_t tmppipelen, remain;
        size_t j, lensofar = 0;

        /*
        * Ask the record layer how it would like to split the amount of data
        * that we have, and how many of those records it would like in one go.
        */
        maxpipes = s->rlayer.wrlmethod->get_max_records(s->rlayer.wrl, type, n,
                                                        max_send_fragment,
                                                        &split_send_fragment);
        /*
        * If max_pipelines is 0 then this means "undefined" and we default to
        * whatever the record layer wants to do. Otherwise we use the smallest
        * value from the number requested by the record layer, and max number
        * configured by the user.
        */
        if (s->max_pipelines > 0 && maxpipes > s->max_pipelines)
            maxpipes = s->max_pipelines;

        if (maxpipes > SSL_MAX_PIPELINES)
            maxpipes = SSL_MAX_PIPELINES;

        if (split_send_fragment > max_send_fragment) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            return -1;
        }

        if (n / maxpipes >= split_send_fragment) {
            /*
             * We have enough data to completely fill all available
             * pipelines
             */
            for (j = 0; j < maxpipes; j++) {
                tmpls[j].type = type;
                tmpls[j].version = recversion;
                tmpls[j].buf = &(buf[tot]) + (j * split_send_fragment);
                tmpls[j].buflen = split_send_fragment;
            }
            /* Remember how much data we are going to be sending */
            s->rlayer.wpend_tot = maxpipes * split_send_fragment;
        } else {
            /* We can partially fill all available pipelines */
            tmppipelen = n / maxpipes;
            remain = n % maxpipes;
            /*
             * If there is a remainder we add an extra byte to the first few
             * pipelines
             */
            if (remain > 0)
                tmppipelen++;
            for (j = 0; j < maxpipes; j++) {
                tmpls[j].type = type;
                tmpls[j].version = recversion;
                tmpls[j].buf = &(buf[tot]) + lensofar;
                tmpls[j].buflen = tmppipelen;
                lensofar += tmppipelen;
                if (j + 1 == remain)
                    tmppipelen--;
            }
            /* Remember how much data we are going to be sending */
            s->rlayer.wpend_tot = n;
        }

        i = HANDLE_RLAYER_WRITE_RETURN(s,
            s->rlayer.wrlmethod->write_records(s->rlayer.wrl, tmpls, maxpipes));
        if (i <= 0) {
            /* SSLfatal() already called if appropriate */
            s->rlayer.wnum = tot;
            return i;
        }

        if (s->rlayer.wpend_tot == n
                || (type == SSL3_RT_APPLICATION_DATA
                    && (s->mode & SSL_MODE_ENABLE_PARTIAL_WRITE) != 0)) {
            *written = tot + s->rlayer.wpend_tot;
            s->rlayer.wpend_tot = 0;
            return 1;
        }

        n -= s->rlayer.wpend_tot;
        tot += s->rlayer.wpend_tot;
    }
}

int ossl_tls_handle_rlayer_return(SSL_CONNECTION *s, int writing, int ret,
                                  char *file, int line)
{
    SSL *ssl = SSL_CONNECTION_GET_SSL(s);

    if (ret == OSSL_RECORD_RETURN_RETRY) {
        s->rwstate = writing ? SSL_WRITING : SSL_READING;
        ret = -1;
    } else {
        s->rwstate = SSL_NOTHING;
        if (ret == OSSL_RECORD_RETURN_EOF) {
            if (writing) {
                /*
                 * This shouldn't happen with a writing operation. We treat it
                 * as fatal.
                 */
                ERR_new();
                ERR_set_debug(file, line, 0);
                ossl_statem_fatal(s, SSL_AD_INTERNAL_ERROR,
                                  ERR_R_INTERNAL_ERROR, NULL);
                ret = OSSL_RECORD_RETURN_FATAL;
            } else if ((s->options & SSL_OP_IGNORE_UNEXPECTED_EOF) != 0) {
                SSL_set_shutdown(ssl, SSL_RECEIVED_SHUTDOWN);
                s->s3.warn_alert = SSL_AD_CLOSE_NOTIFY;
            } else {
                ERR_new();
                ERR_set_debug(file, line, 0);
                /*
                 * This reason code is part of the API and may be used by
                 * applications for control flow decisions.
                 */
                ossl_statem_fatal(s, SSL_AD_DECODE_ERROR,
                                  SSL_R_UNEXPECTED_EOF_WHILE_READING, NULL);
            }
        } else if (ret == OSSL_RECORD_RETURN_FATAL) {
            int al = s->rlayer.rrlmethod->get_alert_code(s->rlayer.rrl);

            if (al != SSL_AD_NO_ALERT) {
                ERR_new();
                ERR_set_debug(file, line, 0);
                ossl_statem_fatal(s, al, SSL_R_RECORD_LAYER_FAILURE, NULL);
            }
            /*
             * else some failure but there is no alert code. We don't log an
             * error for this. The record layer should have logged an error
             * already or, if not, its due to some sys call error which will be
             * reported via SSL_ERROR_SYSCALL and errno.
             */
        }
        /*
         * The record layer distinguishes the cases of EOF, non-fatal
         * err and retry. Upper layers do not.
         * If we got a retry or success then *ret is already correct,
         * otherwise we need to convert the return value.
         */
        if (ret == OSSL_RECORD_RETURN_NON_FATAL_ERR || ret == OSSL_RECORD_RETURN_EOF)
            ret = 0;
        else if (ret < OSSL_RECORD_RETURN_NON_FATAL_ERR)
            ret = -1;
    }

    return ret;
}

/*
 * Release data from a record.
 * If length == 0 then we will release the entire record.
 */
int ssl_release_record(SSL_CONNECTION *s, TLS_RECORD *rr, size_t length)
{
    assert(rr->length >= length);
    if (rr->rechandle != NULL) {
        if (length == 0)
            length = rr->length;
        /* The record layer allocated the buffers for this record */
        if (HANDLE_RLAYER_READ_RETURN(s,
                s->rlayer.rrlmethod->release_record(s->rlayer.rrl,
                                                    rr->rechandle,
                                                    length)) <= 0) {
            /* RLAYER_fatal already called */
            return 0;
        }

        if (length == rr->length)
            s->rlayer.curr_rec++;
    } else if (length == 0 || length == rr->length) {
        /* We allocated the buffers for this record (only happens with DTLS) */
        OPENSSL_free(rr->allocdata);
        rr->allocdata = NULL;
        s->rlayer.curr_rec++;
    }
    rr->length -= length;
    if (rr->length > 0)
        rr->off += length;
    else
        rr->off = 0;

    return 1;
}

/*-
 * Return up to 'len' payload bytes received in 'type' records.
 * 'type' is one of the following:
 *
 *   -  SSL3_RT_HANDSHAKE (when tls_get_message_header and tls_get_message_body
 *			   call us)
 *   -  SSL3_RT_APPLICATION_DATA (when ssl3_read calls us)
 *   -  0 (during a shutdown, no data has to be returned)
 *
 * If we don't have stored data to work from, read an SSL/TLS record first
 * (possibly multiple records if we still don't have anything to return).
 *
 * This function must handle any surprises the peer may have for us, such as
 * Alert records (e.g. close_notify) or renegotiation requests. ChangeCipherSpec
 * messages are treated as if they were handshake messages *if* the |recvd_type|
 * argument is non NULL.
 * Also if record payloads contain fragments too small to process, we store
 * them until there is enough for the respective protocol (the record protocol
 * may use arbitrary fragmentation and even interleaving):
 *     Change cipher spec protocol
 *             just 1 byte needed, no need for keeping anything stored
 *     Alert protocol
 *             2 bytes needed (AlertLevel, AlertDescription)
 *     Handshake protocol
 *             4 bytes needed (HandshakeType, uint24 length) -- we just have
 *             to detect unexpected Client Hello and Hello Request messages
 *             here, anything else is handled by higher layers
 *     Application data protocol
 *             none of our business
 */
int ssl3_read_bytes(SSL *ssl, uint8_t type, uint8_t *recvd_type,
                    unsigned char *buf, size_t len,
                    int peek, size_t *readbytes)
{
    int i, j, ret;
    size_t n, curr_rec, totalbytes;
    TLS_RECORD *rr;
    void (*cb) (const SSL *ssl, int type2, int val) = NULL;
    int is_tls13;
    SSL_CONNECTION *s = SSL_CONNECTION_FROM_SSL_ONLY(ssl);

    is_tls13 = SSL_CONNECTION_IS_TLS13(s);

    if ((type != 0
            && (type != SSL3_RT_APPLICATION_DATA)
            && (type != SSL3_RT_HANDSHAKE))
        || (peek && (type != SSL3_RT_APPLICATION_DATA))) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return -1;
    }

    if ((type == SSL3_RT_HANDSHAKE) && (s->rlayer.handshake_fragment_len > 0))
        /* (partially) satisfy request from storage */
    {
        unsigned char *src = s->rlayer.handshake_fragment;
        unsigned char *dst = buf;
        unsigned int k;

        /* peek == 0 */
        n = 0;
        while ((len > 0) && (s->rlayer.handshake_fragment_len > 0)) {
            *dst++ = *src++;
            len--;
            s->rlayer.handshake_fragment_len--;
            n++;
        }
        /* move any remaining fragment bytes: */
        for (k = 0; k < s->rlayer.handshake_fragment_len; k++)
            s->rlayer.handshake_fragment[k] = *src++;

        if (recvd_type != NULL)
            *recvd_type = SSL3_RT_HANDSHAKE;

        *readbytes = n;
        return 1;
    }

    /*
     * Now s->rlayer.handshake_fragment_len == 0 if type == SSL3_RT_HANDSHAKE.
     */

    if (!ossl_statem_get_in_handshake(s) && SSL_in_init(ssl)) {
        /* type == SSL3_RT_APPLICATION_DATA */
        i = s->handshake_func(ssl);
        /* SSLfatal() already called */
        if (i < 0)
            return i;
        if (i == 0)
            return -1;
    }
 start:
    s->rwstate = SSL_NOTHING;

    /*-
     * For each record 'i' up to |num_recs]
     * rr[i].type     - is the type of record
     * rr[i].data,    - data
     * rr[i].off,     - offset into 'data' for next read
     * rr[i].length,  - number of bytes.
     */
    /* get new records if necessary */
    if (s->rlayer.curr_rec >= s->rlayer.num_recs) {
        s->rlayer.curr_rec = s->rlayer.num_recs = 0;
        do {
            rr = &s->rlayer.tlsrecs[s->rlayer.num_recs];

            ret = HANDLE_RLAYER_READ_RETURN(s,
                    s->rlayer.rrlmethod->read_record(s->rlayer.rrl,
                                                     &rr->rechandle,
                                                     &rr->version, &rr->type,
                                                     &rr->data, &rr->length,
                                                     NULL, NULL));
            if (ret <= 0) {
                /* SSLfatal() already called if appropriate */
                return ret;
            }
            rr->off = 0;
            s->rlayer.num_recs++;
        } while (s->rlayer.rrlmethod->processed_read_pending(s->rlayer.rrl)
                 && s->rlayer.num_recs < SSL_MAX_PIPELINES);
    }
    rr = &s->rlayer.tlsrecs[s->rlayer.curr_rec];

    if (s->rlayer.handshake_fragment_len > 0
            && rr->type != SSL3_RT_HANDSHAKE
            && SSL_CONNECTION_IS_TLS13(s)) {
        SSLfatal(s, SSL_AD_UNEXPECTED_MESSAGE,
                 SSL_R_MIXED_HANDSHAKE_AND_NON_HANDSHAKE_DATA);
        return -1;
    }

    /*
     * Reset the count of consecutive warning alerts if we've got a non-empty
     * record that isn't an alert.
     */
    if (rr->type != SSL3_RT_ALERT && rr->length != 0)
        s->rlayer.alert_count = 0;

    /* we now have a packet which can be read and processed */

    if (s->s3.change_cipher_spec /* set when we receive ChangeCipherSpec,
                                  * reset by ssl3_get_finished */
        && (rr->type != SSL3_RT_HANDSHAKE)) {
        SSLfatal(s, SSL_AD_UNEXPECTED_MESSAGE,
                 SSL_R_DATA_BETWEEN_CCS_AND_FINISHED);
        return -1;
    }

    /*
     * If the other end has shut down, throw anything we read away (even in
     * 'peek' mode)
     */
    if (s->shutdown & SSL_RECEIVED_SHUTDOWN) {
        s->rlayer.curr_rec++;
        s->rwstate = SSL_NOTHING;
        return 0;
    }

    if (type == rr->type
        || (rr->type == SSL3_RT_CHANGE_CIPHER_SPEC
            && type == SSL3_RT_HANDSHAKE && recvd_type != NULL
            && !is_tls13)) {
        /*
         * SSL3_RT_APPLICATION_DATA or
         * SSL3_RT_HANDSHAKE or
         * SSL3_RT_CHANGE_CIPHER_SPEC
         */
        /*
         * make sure that we are not getting application data when we are
         * doing a handshake for the first time
         */
        if (SSL_in_init(ssl) && type == SSL3_RT_APPLICATION_DATA
                && SSL_IS_FIRST_HANDSHAKE(s)) {
            SSLfatal(s, SSL_AD_UNEXPECTED_MESSAGE, SSL_R_APP_DATA_IN_HANDSHAKE);
            return -1;
        }

        if (type == SSL3_RT_HANDSHAKE
            && rr->type == SSL3_RT_CHANGE_CIPHER_SPEC
            && s->rlayer.handshake_fragment_len > 0) {
            SSLfatal(s, SSL_AD_UNEXPECTED_MESSAGE, SSL_R_CCS_RECEIVED_EARLY);
            return -1;
        }

        if (recvd_type != NULL)
            *recvd_type = rr->type;

        if (len == 0) {
            /*
             * Skip a zero length record. This ensures multiple calls to
             * SSL_read() with a zero length buffer will eventually cause
             * SSL_pending() to report data as being available.
             */
            if (rr->length == 0 && !ssl_release_record(s, rr, 0))
                return -1;

            return 0;
        }

        totalbytes = 0;
        curr_rec = s->rlayer.curr_rec;
        do {
            if (len - totalbytes > rr->length)
                n = rr->length;
            else
                n = len - totalbytes;

            memcpy(buf, &(rr->data[rr->off]), n);
            buf += n;
            if (peek) {
                /* Mark any zero length record as consumed CVE-2016-6305 */
                if (rr->length == 0 && !ssl_release_record(s, rr, 0))
                    return -1;
            } else {
                if (!ssl_release_record(s, rr, n))
                    return -1;
            }
            if (rr->length == 0
                || (peek && n == rr->length)) {
                rr++;
                curr_rec++;
            }
            totalbytes += n;
        } while (type == SSL3_RT_APPLICATION_DATA
                    && curr_rec < s->rlayer.num_recs
                    && totalbytes < len);
        if (totalbytes == 0) {
            /* We must have read empty records. Get more data */
            goto start;
        }
        *readbytes = totalbytes;
        return 1;
    }

    /*
     * If we get here, then type != rr->type; if we have a handshake message,
     * then it was unexpected (Hello Request or Client Hello) or invalid (we
     * were actually expecting a CCS).
     */

    /*
     * Lets just double check that we've not got an SSLv2 record
     */
    if (rr->version == SSL2_VERSION) {
        /*
         * Should never happen. ssl3_get_record() should only give us an SSLv2
         * record back if this is the first packet and we are looking for an
         * initial ClientHello. Therefore |type| should always be equal to
         * |rr->type|. If not then something has gone horribly wrong
         */
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return -1;
    }

    if (ssl->method->version == TLS_ANY_VERSION
        && (s->server || rr->type != SSL3_RT_ALERT)) {
        /*
         * If we've got this far and still haven't decided on what version
         * we're using then this must be a client side alert we're dealing
         * with. We shouldn't be receiving anything other than a ClientHello
         * if we are a server.
         */
        s->version = rr->version;
        SSLfatal(s, SSL_AD_UNEXPECTED_MESSAGE, SSL_R_UNEXPECTED_MESSAGE);
        return -1;
    }

    /*-
     * s->rlayer.handshake_fragment_len == 4  iff  rr->type == SSL3_RT_HANDSHAKE;
     * (Possibly rr is 'empty' now, i.e. rr->length may be 0.)
     */

    if (rr->type == SSL3_RT_ALERT) {
        unsigned int alert_level, alert_descr;
        const unsigned char *alert_bytes = rr->data + rr->off;
        PACKET alert;

        if (!PACKET_buf_init(&alert, alert_bytes, rr->length)
                || !PACKET_get_1(&alert, &alert_level)
                || !PACKET_get_1(&alert, &alert_descr)
                || PACKET_remaining(&alert) != 0) {
            SSLfatal(s, SSL_AD_UNEXPECTED_MESSAGE, SSL_R_INVALID_ALERT);
            return -1;
        }

        if (s->msg_callback)
            s->msg_callback(0, s->version, SSL3_RT_ALERT, alert_bytes, 2, ssl,
                            s->msg_callback_arg);

        if (s->info_callback != NULL)
            cb = s->info_callback;
        else if (ssl->ctx->info_callback != NULL)
            cb = ssl->ctx->info_callback;

        if (cb != NULL) {
            j = (alert_level << 8) | alert_descr;
            cb(ssl, SSL_CB_READ_ALERT, j);
        }

        if ((!is_tls13 && alert_level == SSL3_AL_WARNING)
                || (is_tls13 && alert_descr == SSL_AD_USER_CANCELLED)) {
            s->s3.warn_alert = alert_descr;
            if (!ssl_release_record(s, rr, 0))
                return -1;

            s->rlayer.alert_count++;
            if (s->rlayer.alert_count == MAX_WARN_ALERT_COUNT) {
                SSLfatal(s, SSL_AD_UNEXPECTED_MESSAGE,
                         SSL_R_TOO_MANY_WARN_ALERTS);
                return -1;
            }
        }

        /*
         * Apart from close_notify the only other warning alert in TLSv1.3
         * is user_cancelled - which we just ignore.
         */
        if (is_tls13 && alert_descr == SSL_AD_USER_CANCELLED) {
            goto start;
        } else if (alert_descr == SSL_AD_CLOSE_NOTIFY
                && (is_tls13 || alert_level == SSL3_AL_WARNING)) {
            s->shutdown |= SSL_RECEIVED_SHUTDOWN;
            return 0;
        } else if (alert_level == SSL3_AL_FATAL || is_tls13) {
            s->rwstate = SSL_NOTHING;
            s->s3.fatal_alert = alert_descr;
            SSLfatal_data(s, SSL_AD_NO_ALERT,
                          SSL_AD_REASON_OFFSET + alert_descr,
                          "SSL alert number %d", alert_descr);
            s->shutdown |= SSL_RECEIVED_SHUTDOWN;
            if (!ssl_release_record(s, rr, 0))
                return -1;
            SSL_CTX_remove_session(s->session_ctx, s->session);
            return 0;
        } else if (alert_descr == SSL_AD_NO_RENEGOTIATION) {
            /*
             * This is a warning but we receive it if we requested
             * renegotiation and the peer denied it. Terminate with a fatal
             * alert because if the application tried to renegotiate it
             * presumably had a good reason and expects it to succeed. In
             * the future we might have a renegotiation where we don't care
             * if the peer refused it where we carry on.
             */
            SSLfatal(s, SSL_AD_HANDSHAKE_FAILURE, SSL_R_NO_RENEGOTIATION);
            return -1;
        } else if (alert_level == SSL3_AL_WARNING) {
            /* We ignore any other warning alert in TLSv1.2 and below */
            goto start;
        }

        SSLfatal(s, SSL_AD_ILLEGAL_PARAMETER, SSL_R_UNKNOWN_ALERT_TYPE);
        return -1;
    }

    if ((s->shutdown & SSL_SENT_SHUTDOWN) != 0) {
        if (rr->type == SSL3_RT_HANDSHAKE) {
            BIO *rbio;

            /*
             * We ignore any handshake messages sent to us unless they are
             * TLSv1.3 in which case we want to process them. For all other
             * handshake messages we can't do anything reasonable with them
             * because we are unable to write any response due to having already
             * sent close_notify.
             */
            if (!SSL_CONNECTION_IS_TLS13(s)) {
                if (!ssl_release_record(s, rr, 0))
                    return -1;

                if ((s->mode & SSL_MODE_AUTO_RETRY) != 0)
                    goto start;

                s->rwstate = SSL_READING;
                rbio = SSL_get_rbio(ssl);
                BIO_clear_retry_flags(rbio);
                BIO_set_retry_read(rbio);
                return -1;
            }
        } else {
            /*
             * The peer is continuing to send application data, but we have
             * already sent close_notify. If this was expected we should have
             * been called via SSL_read() and this would have been handled
             * above.
             * No alert sent because we already sent close_notify
             */
            if (!ssl_release_record(s, rr, 0))
                return -1;
            SSLfatal(s, SSL_AD_NO_ALERT,
                     SSL_R_APPLICATION_DATA_AFTER_CLOSE_NOTIFY);
            return -1;
        }
    }

    /*
     * For handshake data we have 'fragment' storage, so fill that so that we
     * can process the header at a fixed place. This is done after the
     * "SHUTDOWN" code above to avoid filling the fragment storage with data
     * that we're just going to discard.
     */
    if (rr->type == SSL3_RT_HANDSHAKE) {
        size_t dest_maxlen = sizeof(s->rlayer.handshake_fragment);
        unsigned char *dest = s->rlayer.handshake_fragment;
        size_t *dest_len = &s->rlayer.handshake_fragment_len;

        n = dest_maxlen - *dest_len; /* available space in 'dest' */
        if (rr->length < n)
            n = rr->length; /* available bytes */

        /* now move 'n' bytes: */
        if (n > 0) {
            memcpy(dest + *dest_len, rr->data + rr->off, n);
            *dest_len += n;
        }
        /*
         * We release the number of bytes consumed, or the whole record if it
         * is zero length
         */
        if ((n > 0 || rr->length == 0) && !ssl_release_record(s, rr, n))
            return -1;

        if (*dest_len < dest_maxlen)
            goto start;     /* fragment was too small */
    }

    if (rr->type == SSL3_RT_CHANGE_CIPHER_SPEC) {
        SSLfatal(s, SSL_AD_UNEXPECTED_MESSAGE, SSL_R_CCS_RECEIVED_EARLY);
        return -1;
    }

    /*
     * Unexpected handshake message (ClientHello, NewSessionTicket (TLS1.3) or
     * protocol violation)
     */
    if ((s->rlayer.handshake_fragment_len >= 4)
            && !ossl_statem_get_in_handshake(s)) {
        int ined = (s->early_data_state == SSL_EARLY_DATA_READING);

        /* We found handshake data, so we're going back into init */
        ossl_statem_set_in_init(s, 1);

        i = s->handshake_func(ssl);
        /* SSLfatal() already called if appropriate */
        if (i < 0)
            return i;
        if (i == 0) {
            return -1;
        }

        /*
         * If we were actually trying to read early data and we found a
         * handshake message, then we don't want to continue to try and read
         * the application data any more. It won't be "early" now.
         */
        if (ined)
            return -1;

        if (!(s->mode & SSL_MODE_AUTO_RETRY)) {
            if (!RECORD_LAYER_read_pending(&s->rlayer)) {
                BIO *bio;
                /*
                 * In the case where we try to read application data, but we
                 * trigger an SSL handshake, we return -1 with the retry
                 * option set.  Otherwise renegotiation may cause nasty
                 * problems in the blocking world
                 */
                s->rwstate = SSL_READING;
                bio = SSL_get_rbio(ssl);
                BIO_clear_retry_flags(bio);
                BIO_set_retry_read(bio);
                return -1;
            }
        }
        goto start;
    }

    switch (rr->type) {
    default:
        /*
         * TLS 1.0 and 1.1 say you SHOULD ignore unrecognised record types, but
         * TLS 1.2 says you MUST send an unexpected message alert. We use the
         * TLS 1.2 behaviour for all protocol versions to prevent issues where
         * no progress is being made and the peer continually sends unrecognised
         * record types, using up resources processing them.
         */
        SSLfatal(s, SSL_AD_UNEXPECTED_MESSAGE, SSL_R_UNEXPECTED_RECORD);
        return -1;
    case SSL3_RT_CHANGE_CIPHER_SPEC:
    case SSL3_RT_ALERT:
    case SSL3_RT_HANDSHAKE:
        /*
         * we already handled all of these, with the possible exception of
         * SSL3_RT_HANDSHAKE when ossl_statem_get_in_handshake(s) is true, but
         * that should not happen when type != rr->type
         */
        SSLfatal(s, SSL_AD_UNEXPECTED_MESSAGE, ERR_R_INTERNAL_ERROR);
        return -1;
    case SSL3_RT_APPLICATION_DATA:
        /*
         * At this point, we were expecting handshake data, but have
         * application data.  If the library was running inside ssl3_read()
         * (i.e. in_read_app_data is set) and it makes sense to read
         * application data at this point (session renegotiation not yet
         * started), we will indulge it.
         */
        if (ossl_statem_app_data_allowed(s)) {
            s->s3.in_read_app_data = 2;
            return -1;
        } else if (ossl_statem_skip_early_data(s)) {
            /*
             * This can happen after a client sends a CH followed by early_data,
             * but the server responds with a HelloRetryRequest. The server
             * reads the next record from the client expecting to find a
             * plaintext ClientHello but gets a record which appears to be
             * application data. The trial decrypt "works" because null
             * decryption was applied. We just skip it and move on to the next
             * record.
             */
            if (!ossl_early_data_count_ok(s, rr->length,
                                          EARLY_DATA_CIPHERTEXT_OVERHEAD, 0)) {
                /* SSLfatal() already called */
                return -1;
            }
            if (!ssl_release_record(s, rr, 0))
                return -1;
            goto start;
        } else {
            SSLfatal(s, SSL_AD_UNEXPECTED_MESSAGE, SSL_R_UNEXPECTED_RECORD);
            return -1;
        }
    }
}

/*
 * Returns true if the current rrec was sent in SSLv2 backwards compatible
 * format and false otherwise.
 */
int RECORD_LAYER_is_sslv2_record(RECORD_LAYER *rl)
{
    if (SSL_CONNECTION_IS_DTLS(rl->s))
        return 0;
    return rl->tlsrecs[0].version == SSL2_VERSION;
}

static OSSL_FUNC_rlayer_msg_callback_fn rlayer_msg_callback_wrapper;
static void rlayer_msg_callback_wrapper(int write_p, int version,
                                        int content_type, const void *buf,
                                        size_t len, void *cbarg)
{
    SSL_CONNECTION *s = cbarg;
    SSL *ssl = SSL_CONNECTION_GET_USER_SSL(s);

    if (s->msg_callback != NULL)
        s->msg_callback(write_p, version, content_type, buf, len, ssl,
                        s->msg_callback_arg);
}

static OSSL_FUNC_rlayer_security_fn rlayer_security_wrapper;
static int rlayer_security_wrapper(void *cbarg, int op, int bits, int nid,
                                   void *other)
{
    SSL_CONNECTION *s = cbarg;

    return ssl_security(s, op, bits, nid, other);
}

static OSSL_FUNC_rlayer_padding_fn rlayer_padding_wrapper;
static size_t rlayer_padding_wrapper(void *cbarg, int type, size_t len)
{
    SSL_CONNECTION *s = cbarg;
    SSL *ssl = SSL_CONNECTION_GET_USER_SSL(s);

    return s->rlayer.record_padding_cb(ssl, type, len,
                                       s->rlayer.record_padding_arg);
}

static const OSSL_DISPATCH rlayer_dispatch[] = {
    { OSSL_FUNC_RLAYER_SKIP_EARLY_DATA, (void (*)(void))ossl_statem_skip_early_data },
    { OSSL_FUNC_RLAYER_MSG_CALLBACK, (void (*)(void))rlayer_msg_callback_wrapper },
    { OSSL_FUNC_RLAYER_SECURITY, (void (*)(void))rlayer_security_wrapper },
    { OSSL_FUNC_RLAYER_PADDING, (void (*)(void))rlayer_padding_wrapper },
    OSSL_DISPATCH_END
};

void ossl_ssl_set_custom_record_layer(SSL_CONNECTION *s,
                                      const OSSL_RECORD_METHOD *meth,
                                      void *rlarg)
{
    s->rlayer.custom_rlmethod = meth;
    s->rlayer.rlarg = rlarg;
}

static const OSSL_RECORD_METHOD *ssl_select_next_record_layer(SSL_CONNECTION *s,
                                                              int direction,
                                                              int level)
{
    if (s->rlayer.custom_rlmethod != NULL)
        return s->rlayer.custom_rlmethod;

    if (level == OSSL_RECORD_PROTECTION_LEVEL_NONE) {
        if (SSL_CONNECTION_IS_DTLS(s))
            return &ossl_dtls_record_method;

        return &ossl_tls_record_method;
    }

#ifndef OPENSSL_NO_KTLS
    /* KTLS does not support renegotiation */
    if (level == OSSL_RECORD_PROTECTION_LEVEL_APPLICATION
            && (s->options & SSL_OP_ENABLE_KTLS) != 0
            && (SSL_CONNECTION_IS_TLS13(s) || SSL_IS_FIRST_HANDSHAKE(s)))
        return &ossl_ktls_record_method;
#endif

    /* Default to the current OSSL_RECORD_METHOD */
    return direction == OSSL_RECORD_DIRECTION_READ ? s->rlayer.rrlmethod
                                                   : s->rlayer.wrlmethod;
}

static int ssl_post_record_layer_select(SSL_CONNECTION *s, int direction)
{
    const OSSL_RECORD_METHOD *thismethod;
    OSSL_RECORD_LAYER *thisrl;

    if (direction == OSSL_RECORD_DIRECTION_READ) {
        thismethod = s->rlayer.rrlmethod;
        thisrl = s->rlayer.rrl;
    } else {
        thismethod = s->rlayer.wrlmethod;
        thisrl = s->rlayer.wrl;
    }

#ifndef OPENSSL_NO_KTLS
    {
        SSL *ssl = SSL_CONNECTION_GET_SSL(s);

        if (s->rlayer.rrlmethod == &ossl_ktls_record_method) {
            /* KTLS does not support renegotiation so disallow it */
            SSL_set_options(ssl, SSL_OP_NO_RENEGOTIATION);
        }
    }
#endif
    if (SSL_IS_FIRST_HANDSHAKE(s) && thismethod->set_first_handshake != NULL)
        thismethod->set_first_handshake(thisrl, 1);

    if (s->max_pipelines != 0 && thismethod->set_max_pipelines != NULL)
        thismethod->set_max_pipelines(thisrl, s->max_pipelines);

    return 1;
}

int ssl_set_new_record_layer(SSL_CONNECTION *s, int version,
                             int direction, int level,
                             unsigned char *secret, size_t secretlen,
                             unsigned char *key, size_t keylen,
                             unsigned char *iv,  size_t ivlen,
                             unsigned char *mackey, size_t mackeylen,
                             const EVP_CIPHER *ciph, size_t taglen,
                             int mactype, const EVP_MD *md,
                             const SSL_COMP *comp, const EVP_MD *kdfdigest)
{
    OSSL_PARAM options[5], *opts = options;
    OSSL_PARAM settings[6], *set =  settings;
    const OSSL_RECORD_METHOD **thismethod;
    OSSL_RECORD_LAYER **thisrl, *newrl = NULL;
    BIO *thisbio;
    SSL_CTX *sctx = SSL_CONNECTION_GET_CTX(s);
    const OSSL_RECORD_METHOD *meth;
    int use_etm, stream_mac = 0, tlstree = 0;
    unsigned int maxfrag = (direction == OSSL_RECORD_DIRECTION_WRITE)
                           ? ssl_get_max_send_fragment(s)
                           : SSL3_RT_MAX_PLAIN_LENGTH;
    int use_early_data = 0;
    uint32_t max_early_data;
    COMP_METHOD *compm = (comp == NULL) ? NULL : comp->method;

    meth = ssl_select_next_record_layer(s, direction, level);

    if (direction == OSSL_RECORD_DIRECTION_READ) {
        thismethod = &s->rlayer.rrlmethod;
        thisrl = &s->rlayer.rrl;
        thisbio = s->rbio;
    } else {
        thismethod = &s->rlayer.wrlmethod;
        thisrl = &s->rlayer.wrl;
        thisbio = s->wbio;
    }

    if (meth == NULL)
        meth = *thismethod;

    if (!ossl_assert(meth != NULL)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    /* Parameters that *may* be supported by a record layer if passed */
    *opts++ = OSSL_PARAM_construct_uint64(OSSL_LIBSSL_RECORD_LAYER_PARAM_OPTIONS,
                                          &s->options);
    *opts++ = OSSL_PARAM_construct_uint32(OSSL_LIBSSL_RECORD_LAYER_PARAM_MODE,
                                          &s->mode);
    if (direction == OSSL_RECORD_DIRECTION_READ) {
        *opts++ = OSSL_PARAM_construct_size_t(OSSL_LIBSSL_RECORD_LAYER_READ_BUFFER_LEN,
                                              &s->rlayer.default_read_buf_len);
        *opts++ = OSSL_PARAM_construct_int(OSSL_LIBSSL_RECORD_LAYER_PARAM_READ_AHEAD,
                                           &s->rlayer.read_ahead);
    } else {
        *opts++ = OSSL_PARAM_construct_size_t(OSSL_LIBSSL_RECORD_LAYER_PARAM_BLOCK_PADDING,
                                              &s->rlayer.block_padding);
        *opts++ = OSSL_PARAM_construct_size_t(OSSL_LIBSSL_RECORD_LAYER_PARAM_HS_PADDING,
                                              &s->rlayer.hs_padding);
    }
    *opts = OSSL_PARAM_construct_end();

    /* Parameters that *must* be supported by a record layer if passed */
    if (direction == OSSL_RECORD_DIRECTION_READ) {
        use_etm = SSL_READ_ETM(s) ? 1 : 0;
        if ((s->mac_flags & SSL_MAC_FLAG_READ_MAC_STREAM) != 0)
            stream_mac = 1;

        if ((s->mac_flags & SSL_MAC_FLAG_READ_MAC_TLSTREE) != 0)
            tlstree = 1;
    } else {
        use_etm = SSL_WRITE_ETM(s) ? 1 : 0;
        if ((s->mac_flags & SSL_MAC_FLAG_WRITE_MAC_STREAM) != 0)
            stream_mac = 1;

        if ((s->mac_flags & SSL_MAC_FLAG_WRITE_MAC_TLSTREE) != 0)
            tlstree = 1;
    }

    if (use_etm)
        *set++ = OSSL_PARAM_construct_int(OSSL_LIBSSL_RECORD_LAYER_PARAM_USE_ETM,
                                          &use_etm);

    if (stream_mac)
        *set++ = OSSL_PARAM_construct_int(OSSL_LIBSSL_RECORD_LAYER_PARAM_STREAM_MAC,
                                          &stream_mac);

    if (tlstree)
        *set++ = OSSL_PARAM_construct_int(OSSL_LIBSSL_RECORD_LAYER_PARAM_TLSTREE,
                                          &tlstree);

    /*
     * We only need to do this for the read side. The write side should already
     * have the correct value due to the ssl_get_max_send_fragment() call above
     */
    if (direction == OSSL_RECORD_DIRECTION_READ
            && s->session != NULL
            && USE_MAX_FRAGMENT_LENGTH_EXT(s->session))
        maxfrag = GET_MAX_FRAGMENT_LENGTH(s->session);


    if (maxfrag != SSL3_RT_MAX_PLAIN_LENGTH)
        *set++ = OSSL_PARAM_construct_uint(OSSL_LIBSSL_RECORD_LAYER_PARAM_MAX_FRAG_LEN,
                                           &maxfrag);

    /*
     * The record layer must check the amount of early data sent or received
     * using the early keys. A server also needs to worry about rejected early
     * data that might arrive when the handshake keys are in force.
     */
    if (s->server && direction == OSSL_RECORD_DIRECTION_READ) {
        use_early_data = (level == OSSL_RECORD_PROTECTION_LEVEL_EARLY
                          || level == OSSL_RECORD_PROTECTION_LEVEL_HANDSHAKE);
    } else if (!s->server && direction == OSSL_RECORD_DIRECTION_WRITE) {
        use_early_data = (level == OSSL_RECORD_PROTECTION_LEVEL_EARLY);
    }
    if (use_early_data) {
        max_early_data = ossl_get_max_early_data(s);

        if (max_early_data != 0)
            *set++ = OSSL_PARAM_construct_uint32(OSSL_LIBSSL_RECORD_LAYER_PARAM_MAX_EARLY_DATA,
                                                 &max_early_data);
    }

    *set = OSSL_PARAM_construct_end();

    for (;;) {
        int rlret;
        BIO *prev = NULL;
        BIO *next = NULL;
        unsigned int epoch = 0;
        OSSL_DISPATCH rlayer_dispatch_tmp[OSSL_NELEM(rlayer_dispatch)];
        size_t i, j;

        if (direction == OSSL_RECORD_DIRECTION_READ) {
            prev = s->rlayer.rrlnext;
            if (SSL_CONNECTION_IS_DTLS(s)
                    && level != OSSL_RECORD_PROTECTION_LEVEL_NONE)
                epoch = dtls1_get_epoch(s, SSL3_CC_READ); /* new epoch */

#ifndef OPENSSL_NO_DGRAM
            if (SSL_CONNECTION_IS_DTLS(s))
                next = BIO_new(BIO_s_dgram_mem());
            else
#endif
                next = BIO_new(BIO_s_mem());

            if (next == NULL) {
                SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
                return 0;
            }
            s->rlayer.rrlnext = next;
        } else {
            if (SSL_CONNECTION_IS_DTLS(s)
                    && level != OSSL_RECORD_PROTECTION_LEVEL_NONE)
                epoch = dtls1_get_epoch(s, SSL3_CC_WRITE); /* new epoch */
        }

        /*
         * Create a copy of the dispatch array, missing out wrappers for
         * callbacks that we don't need.
         */
        for (i = 0, j = 0; i < OSSL_NELEM(rlayer_dispatch); i++) {
            switch (rlayer_dispatch[i].function_id) {
            case OSSL_FUNC_RLAYER_MSG_CALLBACK:
                if (s->msg_callback == NULL)
                    continue;
                break;
            case OSSL_FUNC_RLAYER_PADDING:
                if (s->rlayer.record_padding_cb == NULL)
                    continue;
                break;
            default:
                break;
            }
            rlayer_dispatch_tmp[j++] = rlayer_dispatch[i];
        }

        rlret = meth->new_record_layer(sctx->libctx, sctx->propq, version,
                                       s->server, direction, level, epoch,
                                       secret, secretlen, key, keylen, iv,
                                       ivlen, mackey, mackeylen, ciph, taglen,
                                       mactype, md, compm, kdfdigest, prev,
                                       thisbio, next, NULL, NULL, settings,
                                       options, rlayer_dispatch_tmp, s,
                                       s->rlayer.rlarg, &newrl);
        BIO_free(prev);
        switch (rlret) {
        case OSSL_RECORD_RETURN_FATAL:
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, SSL_R_RECORD_LAYER_FAILURE);
            return 0;

        case OSSL_RECORD_RETURN_NON_FATAL_ERR:
            if (*thismethod != meth && *thismethod != NULL) {
                /*
                 * We tried a new record layer method, but it didn't work out,
                 * so we fallback to the original method and try again
                 */
                meth = *thismethod;
                continue;
            }
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, SSL_R_NO_SUITABLE_RECORD_LAYER);
            return 0;

        case OSSL_RECORD_RETURN_SUCCESS:
            break;

        default:
            /* Should not happen */
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            return 0;
        }
        break;
    }

    /*
     * Free the old record layer if we have one except in the case of DTLS when
     * writing and there are still buffered sent messages in our queue. In that
     * case the record layer is still referenced by those buffered messages for
     * potential retransmit. Only when those buffered messages get freed do we
     * free the record layer object (see dtls1_hm_fragment_free)
     */
    if (!SSL_CONNECTION_IS_DTLS(s)
            || direction == OSSL_RECORD_DIRECTION_READ
            || pqueue_peek(s->d1->sent_messages) == NULL) {
        if (*thismethod != NULL && !(*thismethod)->free(*thisrl)) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            return 0;
        }
    }

    *thisrl = newrl;
    *thismethod = meth;

    return ssl_post_record_layer_select(s, direction);
}

int ssl_set_record_protocol_version(SSL_CONNECTION *s, int vers)
{
    if (!ossl_assert(s->rlayer.rrlmethod != NULL)
            || !ossl_assert(s->rlayer.wrlmethod != NULL))
        return 0;
    s->rlayer.rrlmethod->set_protocol_version(s->rlayer.rrl, s->version);
    s->rlayer.wrlmethod->set_protocol_version(s->rlayer.wrl, s->version);

    return 1;
}
