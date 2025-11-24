/*
 * Copyright 2018-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <assert.h>
#include "../../ssl_local.h"
#include "../record_local.h"
#include "recmethod_local.h"

/* mod 128 saturating subtract of two 64-bit values in big-endian order */
static int satsub64be(const unsigned char *v1, const unsigned char *v2)
{
    int64_t ret;
    uint64_t l1, l2;

    n2l8(v1, l1);
    n2l8(v2, l2);

    ret = l1 - l2;

    /* We do not permit wrap-around */
    if (l1 > l2 && ret < 0)
        return 128;
    else if (l2 > l1 && ret > 0)
        return -128;

    if (ret > 128)
        return 128;
    else if (ret < -128)
        return -128;
    else
        return (int)ret;
}

static int dtls_record_replay_check(OSSL_RECORD_LAYER *rl, DTLS_BITMAP *bitmap)
{
    int cmp;
    unsigned int shift;
    const unsigned char *seq = rl->sequence;

    cmp = satsub64be(seq, bitmap->max_seq_num);
    if (cmp > 0) {
        ossl_tls_rl_record_set_seq_num(&rl->rrec[0], seq);
        return 1;               /* this record in new */
    }
    shift = -cmp;
    if (shift >= sizeof(bitmap->map) * 8)
        return 0;               /* stale, outside the window */
    else if (bitmap->map & ((uint64_t)1 << shift))
        return 0;               /* record previously received */

    ossl_tls_rl_record_set_seq_num(&rl->rrec[0], seq);
    return 1;
}

static void dtls_record_bitmap_update(OSSL_RECORD_LAYER *rl,
                                      DTLS_BITMAP *bitmap)
{
    int cmp;
    unsigned int shift;
    const unsigned char *seq = rl->sequence;

    cmp = satsub64be(seq, bitmap->max_seq_num);
    if (cmp > 0) {
        shift = cmp;
        if (shift < sizeof(bitmap->map) * 8)
            bitmap->map <<= shift, bitmap->map |= 1UL;
        else
            bitmap->map = 1UL;
        memcpy(bitmap->max_seq_num, seq, SEQ_NUM_SIZE);
    } else {
        shift = -cmp;
        if (shift < sizeof(bitmap->map) * 8)
            bitmap->map |= (uint64_t)1 << shift;
    }
}

static DTLS_BITMAP *dtls_get_bitmap(OSSL_RECORD_LAYER *rl, TLS_RL_RECORD *rr,
                                    unsigned int *is_next_epoch)
{
    *is_next_epoch = 0;

    /* In current epoch, accept HM, CCS, DATA, & ALERT */
    if (rr->epoch == rl->epoch)
        return &rl->bitmap;

    /*
     * Check if the message is from the next epoch
     */
    else if (rr->epoch == rl->epoch + 1) {
        *is_next_epoch = 1;
        return &rl->next_bitmap;
    }

    return NULL;
}

static void dtls_set_in_init(OSSL_RECORD_LAYER *rl, int in_init)
{
    rl->in_init = in_init;
}

static int dtls_process_record(OSSL_RECORD_LAYER *rl, DTLS_BITMAP *bitmap)
{
    int i;
    int enc_err;
    TLS_RL_RECORD *rr;
    int imac_size;
    size_t mac_size = 0;
    unsigned char md[EVP_MAX_MD_SIZE];
    SSL_MAC_BUF macbuf = { NULL, 0 };
    int ret = 0;

    rr = &rl->rrec[0];

    /*
     * At this point, rl->packet_length == DTLS1_RT_HEADER_LENGTH + rr->length,
     * and we have that many bytes in rl->packet
     */
    rr->input = &(rl->packet[DTLS1_RT_HEADER_LENGTH]);

    /*
     * ok, we can now read from 'rl->packet' data into 'rr'. rr->input
     * points at rr->length bytes, which need to be copied into rr->data by
     * either the decryption or by the decompression. When the data is 'copied'
     * into the rr->data buffer, rr->input will be pointed at the new buffer
     */

    /*
     * We now have - encrypted [ MAC [ compressed [ plain ] ] ] rr->length
     * bytes of encrypted compressed stuff.
     */

    /* check is not needed I believe */
    if (rr->length > SSL3_RT_MAX_ENCRYPTED_LENGTH) {
        RLAYERfatal(rl, SSL_AD_RECORD_OVERFLOW, SSL_R_ENCRYPTED_LENGTH_TOO_LONG);
        return 0;
    }

    /* decrypt in place in 'rr->input' */
    rr->data = rr->input;
    rr->orig_len = rr->length;

    if (rl->md_ctx != NULL) {
        const EVP_MD *tmpmd = EVP_MD_CTX_get0_md(rl->md_ctx);

        if (tmpmd != NULL) {
            imac_size = EVP_MD_get_size(tmpmd);
            if (!ossl_assert(imac_size > 0 && imac_size <= EVP_MAX_MD_SIZE)) {
                RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, ERR_R_EVP_LIB);
                return 0;
            }
            mac_size = (size_t)imac_size;
        }
    }

    if (rl->use_etm && rl->md_ctx != NULL) {
        unsigned char *mac;

        if (rr->orig_len < mac_size) {
            RLAYERfatal(rl, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_TOO_SHORT);
            return 0;
        }
        rr->length -= mac_size;
        mac = rr->data + rr->length;
        i = rl->funcs->mac(rl, rr, md, 0 /* not send */);
        if (i == 0 || CRYPTO_memcmp(md, mac, (size_t)mac_size) != 0) {
            RLAYERfatal(rl, SSL_AD_BAD_RECORD_MAC,
                        SSL_R_DECRYPTION_FAILED_OR_BAD_RECORD_MAC);
            return 0;
        }
        /*
         * We've handled the mac now - there is no MAC inside the encrypted
         * record
         */
        mac_size = 0;
    }

    /*
     * Set a mark around the packet decryption attempt.  This is DTLS, so
     * bad packets are just ignored, and we don't want to leave stray
     * errors in the queue from processing bogus junk that we ignored.
     */
    ERR_set_mark();
    enc_err = rl->funcs->cipher(rl, rr, 1, 0, &macbuf, mac_size);

    /*-
     * enc_err is:
     *    0: if the record is publicly invalid, or an internal error, or AEAD
     *       decryption failed, or ETM decryption failed.
     *    1: Success or MTE decryption failed (MAC will be randomised)
     */
    if (enc_err == 0) {
        ERR_pop_to_mark();
        if (rl->alert != SSL_AD_NO_ALERT) {
            /* RLAYERfatal() already called */
            goto end;
        }
        /* For DTLS we simply ignore bad packets. */
        rr->length = 0;
        rl->packet_length = 0;
        goto end;
    }
    ERR_clear_last_mark();
    OSSL_TRACE_BEGIN(TLS) {
        BIO_printf(trc_out, "dec %zd\n", rr->length);
        BIO_dump_indent(trc_out, rr->data, rr->length, 4);
    } OSSL_TRACE_END(TLS);

    /* r->length is now the compressed data plus mac */
    if (!rl->use_etm
            && (rl->enc_ctx != NULL)
            && (EVP_MD_CTX_get0_md(rl->md_ctx) != NULL)) {
        /* rl->md_ctx != NULL => mac_size != -1 */

        i = rl->funcs->mac(rl, rr, md, 0 /* not send */);
        if (i == 0 || macbuf.mac == NULL
            || CRYPTO_memcmp(md, macbuf.mac, mac_size) != 0)
            enc_err = 0;
        if (rr->length > SSL3_RT_MAX_COMPRESSED_LENGTH + mac_size)
            enc_err = 0;
    }

    if (enc_err == 0) {
        /* decryption failed, silently discard message */
        rr->length = 0;
        rl->packet_length = 0;
        goto end;
    }

    /* r->length is now just compressed */
    if (rl->compctx != NULL) {
        if (rr->length > SSL3_RT_MAX_COMPRESSED_LENGTH) {
            RLAYERfatal(rl, SSL_AD_RECORD_OVERFLOW,
                        SSL_R_COMPRESSED_LENGTH_TOO_LONG);
            goto end;
        }
        if (!tls_do_uncompress(rl, rr)) {
            RLAYERfatal(rl, SSL_AD_DECOMPRESSION_FAILURE, SSL_R_BAD_DECOMPRESSION);
            goto end;
        }
    }

    /*
     * Check if the received packet overflows the current Max Fragment
     * Length setting.
     */
    if (rr->length > rl->max_frag_len) {
        RLAYERfatal(rl, SSL_AD_RECORD_OVERFLOW, SSL_R_DATA_LENGTH_TOO_LONG);
        goto end;
    }

    rr->off = 0;
    /*-
     * So at this point the following is true
     * ssl->s3.rrec.type   is the type of record
     * ssl->s3.rrec.length == number of bytes in record
     * ssl->s3.rrec.off    == offset to first valid byte
     * ssl->s3.rrec.data   == where to take bytes from, increment
     *                        after use :-).
     */

    /* we have pulled in a full packet so zero things */
    rl->packet_length = 0;

    /* Mark receipt of record. */
    dtls_record_bitmap_update(rl, bitmap);

    ret = 1;
 end:
    if (macbuf.alloced)
        OPENSSL_free(macbuf.mac);
    return ret;
}

static int dtls_rlayer_buffer_record(OSSL_RECORD_LAYER *rl, struct pqueue_st *queue,
                                     unsigned char *priority)
{
    DTLS_RLAYER_RECORD_DATA *rdata;
    pitem *item;

    /* Limit the size of the queue to prevent DOS attacks */
    if (pqueue_size(queue) >= 100)
        return 0;

    rdata = OPENSSL_malloc(sizeof(*rdata));
    item = pitem_new(priority, rdata);
    if (rdata == NULL || item == NULL) {
        OPENSSL_free(rdata);
        pitem_free(item);
        RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return -1;
    }

    rdata->packet = rl->packet;
    rdata->packet_length = rl->packet_length;
    memcpy(&(rdata->rbuf), &rl->rbuf, sizeof(TLS_BUFFER));
    memcpy(&(rdata->rrec), &rl->rrec[0], sizeof(TLS_RL_RECORD));

    item->data = rdata;

    rl->packet = NULL;
    rl->packet_length = 0;
    memset(&rl->rbuf, 0, sizeof(TLS_BUFFER));
    memset(&rl->rrec[0], 0, sizeof(rl->rrec[0]));

    if (!tls_setup_read_buffer(rl)) {
        /* RLAYERfatal() already called */
        OPENSSL_free(rdata->rbuf.buf);
        OPENSSL_free(rdata);
        pitem_free(item);
        return -1;
    }

    if (pqueue_insert(queue, item) == NULL) {
        /* Must be a duplicate so ignore it */
        OPENSSL_free(rdata->rbuf.buf);
        OPENSSL_free(rdata);
        pitem_free(item);
    }

    return 1;
}

/* copy buffered record into OSSL_RECORD_LAYER structure */
static int dtls_copy_rlayer_record(OSSL_RECORD_LAYER *rl, pitem *item)
{
    DTLS_RLAYER_RECORD_DATA *rdata;

    rdata = (DTLS_RLAYER_RECORD_DATA *)item->data;

    ossl_tls_buffer_release(&rl->rbuf);

    rl->packet = rdata->packet;
    rl->packet_length = rdata->packet_length;
    memcpy(&rl->rbuf, &(rdata->rbuf), sizeof(TLS_BUFFER));
    memcpy(&rl->rrec[0], &(rdata->rrec), sizeof(TLS_RL_RECORD));

    /* Set proper sequence number for mac calculation */
    memcpy(&(rl->sequence[2]), &(rdata->packet[5]), 6);

    return 1;
}

static int dtls_retrieve_rlayer_buffered_record(OSSL_RECORD_LAYER *rl,
                                                struct pqueue_st *queue)
{
    pitem *item;

    item = pqueue_pop(queue);
    if (item) {
        dtls_copy_rlayer_record(rl, item);

        OPENSSL_free(item->data);
        pitem_free(item);

        return 1;
    }

    return 0;
}

/*-
 * Call this to get a new input record.
 * It will return <= 0 if more data is needed, normally due to an error
 * or non-blocking IO.
 * When it finishes, one packet has been decoded and can be found in
 * ssl->s3.rrec.type    - is the type of record
 * ssl->s3.rrec.data    - data
 * ssl->s3.rrec.length  - number of bytes
 */
int dtls_get_more_records(OSSL_RECORD_LAYER *rl)
{
    int ssl_major, ssl_minor;
    int rret;
    size_t more, n;
    TLS_RL_RECORD *rr;
    unsigned char *p = NULL;
    DTLS_BITMAP *bitmap;
    unsigned int is_next_epoch;

    rl->num_recs = 0;
    rl->curr_rec = 0;
    rl->num_released = 0;

    rr = rl->rrec;

    if (rl->rbuf.buf == NULL) {
        if (!tls_setup_read_buffer(rl)) {
            /* RLAYERfatal() already called */
            return OSSL_RECORD_RETURN_FATAL;
        }
    }

 again:
    /* if we're renegotiating, then there may be buffered records */
    if (dtls_retrieve_rlayer_buffered_record(rl, rl->processed_rcds)) {
        rl->num_recs = 1;
        return OSSL_RECORD_RETURN_SUCCESS;
    }

    /* get something from the wire */

    /* check if we have the header */
    if ((rl->rstate != SSL_ST_READ_BODY) ||
        (rl->packet_length < DTLS1_RT_HEADER_LENGTH)) {
        rret = rl->funcs->read_n(rl, DTLS1_RT_HEADER_LENGTH,
                                 TLS_BUFFER_get_len(&rl->rbuf), 0, 1, &n);
        /* read timeout is handled by dtls1_read_bytes */
        if (rret < OSSL_RECORD_RETURN_SUCCESS) {
            /* RLAYERfatal() already called if appropriate */
            return rret;         /* error or non-blocking */
        }

        /* this packet contained a partial record, dump it */
        if (rl->packet_length != DTLS1_RT_HEADER_LENGTH) {
            rl->packet_length = 0;
            goto again;
        }

        rl->rstate = SSL_ST_READ_BODY;

        p = rl->packet;

        /* Pull apart the header into the DTLS1_RECORD */
        rr->type = *(p++);
        ssl_major = *(p++);
        ssl_minor = *(p++);
        rr->rec_version = (ssl_major << 8) | ssl_minor;

        /* sequence number is 64 bits, with top 2 bytes = epoch */
        n2s(p, rr->epoch);

        memcpy(&(rl->sequence[2]), p, 6);
        p += 6;

        n2s(p, rr->length);

        if (rl->msg_callback != NULL)
            rl->msg_callback(0, rr->rec_version, SSL3_RT_HEADER, rl->packet, DTLS1_RT_HEADER_LENGTH,
                             rl->cbarg);

        /*
         * Lets check the version. We tolerate alerts that don't have the exact
         * version number (e.g. because of protocol version errors)
         */
        if (!rl->is_first_record && rr->type != SSL3_RT_ALERT) {
            if (rr->rec_version != rl->version) {
                /* unexpected version, silently discard */
                rr->length = 0;
                rl->packet_length = 0;
                goto again;
            }
        }

        if (ssl_major !=
                (rl->version == DTLS_ANY_VERSION ? DTLS1_VERSION_MAJOR
                                                 : rl->version >> 8)) {
            /* wrong version, silently discard record */
            rr->length = 0;
            rl->packet_length = 0;
            goto again;
        }

        if (rr->length > SSL3_RT_MAX_ENCRYPTED_LENGTH) {
            /* record too long, silently discard it */
            rr->length = 0;
            rl->packet_length = 0;
            goto again;
        }

        /*
         * If received packet overflows maximum possible fragment length then
         * silently discard it
         */
        if (rr->length > rl->max_frag_len + SSL3_RT_MAX_ENCRYPTED_OVERHEAD) {
            /* record too long, silently discard it */
            rr->length = 0;
            rl->packet_length = 0;
            goto again;
        }

        /* now rl->rstate == SSL_ST_READ_BODY */
    }

    /* rl->rstate == SSL_ST_READ_BODY, get and decode the data */

    if (rr->length > rl->packet_length - DTLS1_RT_HEADER_LENGTH) {
        /* now rl->packet_length == DTLS1_RT_HEADER_LENGTH */
        more = rr->length;
        rret = rl->funcs->read_n(rl, more, more, 1, 1, &n);
        /* this packet contained a partial record, dump it */
        if (rret < OSSL_RECORD_RETURN_SUCCESS || n != more) {
            if (rl->alert != SSL_AD_NO_ALERT) {
                /* read_n() called RLAYERfatal() */
                return OSSL_RECORD_RETURN_FATAL;
            }
            rr->length = 0;
            rl->packet_length = 0;
            goto again;
        }

        /*
         * now n == rr->length,
         * and rl->packet_length ==  DTLS1_RT_HEADER_LENGTH + rr->length
         */
    }
    /* set state for later operations */
    rl->rstate = SSL_ST_READ_HEADER;

    /* match epochs.  NULL means the packet is dropped on the floor */
    bitmap = dtls_get_bitmap(rl, rr, &is_next_epoch);
    if (bitmap == NULL) {
        rr->length = 0;
        rl->packet_length = 0; /* dump this record */
        goto again;             /* get another record */
    }
#ifndef OPENSSL_NO_SCTP
    /* Only do replay check if no SCTP bio */
    if (!BIO_dgram_is_sctp(rl->bio)) {
#endif
        /* Check whether this is a repeat, or aged record. */
        if (!dtls_record_replay_check(rl, bitmap)) {
            rr->length = 0;
            rl->packet_length = 0; /* dump this record */
            goto again;         /* get another record */
        }
#ifndef OPENSSL_NO_SCTP
    }
#endif

    /* just read a 0 length packet */
    if (rr->length == 0)
        goto again;

    /*
     * If this record is from the next epoch (either HM or ALERT), and a
     * handshake is currently in progress, buffer it since it cannot be
     * processed at this time.
     */
    if (is_next_epoch) {
        if (rl->in_init) {
            if (dtls_rlayer_buffer_record(rl, rl->unprocessed_rcds,
                                          rr->seq_num) < 0) {
                /* RLAYERfatal() already called */
                return OSSL_RECORD_RETURN_FATAL;
            }
        }
        rr->length = 0;
        rl->packet_length = 0;
        goto again;
    }

    if (!dtls_process_record(rl, bitmap)) {
        if (rl->alert != SSL_AD_NO_ALERT) {
            /* dtls_process_record() called RLAYERfatal */
            return OSSL_RECORD_RETURN_FATAL;
        }
        rr->length = 0;
        rl->packet_length = 0; /* dump this record */
        goto again;             /* get another record */
    }

    if (rl->funcs->post_process_record && !rl->funcs->post_process_record(rl, rr)) {
        /* RLAYERfatal already called */
        return OSSL_RECORD_RETURN_FATAL;
    }

    if (rr->length == 0) {
        /* No payload data in this record. Dump it */
        rl->packet_length = 0;
        goto again;
    }

    rl->num_recs = 1;
    return OSSL_RECORD_RETURN_SUCCESS;
}

static int dtls_free(OSSL_RECORD_LAYER *rl)
{
    TLS_BUFFER *rbuf;
    size_t left, written;
    pitem *item;
    DTLS_RLAYER_RECORD_DATA *rdata;
    int ret = 1;

    rbuf = &rl->rbuf;

    left = rbuf->left;
    if (left > 0) {
        /*
         * This record layer is closing but we still have data left in our
         * buffer. It must be destined for the next epoch - so push it there.
         */
        ret = BIO_write_ex(rl->next, rbuf->buf + rbuf->offset, left, &written);
        rbuf->left = 0;
    }

    if (rl->unprocessed_rcds != NULL) {
        while ((item = pqueue_pop(rl->unprocessed_rcds)) != NULL) {
            rdata = (DTLS_RLAYER_RECORD_DATA *)item->data;
            /* Push to the next record layer */
            ret &= BIO_write_ex(rl->next, rdata->packet, rdata->packet_length,
                                &written);
            OPENSSL_free(rdata->rbuf.buf);
            OPENSSL_free(item->data);
            pitem_free(item);
        }
        pqueue_free(rl->unprocessed_rcds);
    }

    if (rl->processed_rcds!= NULL) {
        while ((item = pqueue_pop(rl->processed_rcds)) != NULL) {
            rdata = (DTLS_RLAYER_RECORD_DATA *)item->data;
            OPENSSL_free(rdata->rbuf.buf);
            OPENSSL_free(item->data);
            pitem_free(item);
        }
        pqueue_free(rl->processed_rcds);
    }

    return tls_free(rl) && ret;
}

static int
dtls_new_record_layer(OSSL_LIB_CTX *libctx, const char *propq, int vers,
                      int role, int direction, int level, uint16_t epoch,
                      unsigned char *secret, size_t secretlen,
                      unsigned char *key, size_t keylen, unsigned char *iv,
                      size_t ivlen, unsigned char *mackey, size_t mackeylen,
                      const EVP_CIPHER *ciph, size_t taglen,
                      int mactype,
                      const EVP_MD *md, COMP_METHOD *comp,
                      const EVP_MD *kdfdigest, BIO *prev, BIO *transport,
                      BIO *next, BIO_ADDR *local, BIO_ADDR *peer,
                      const OSSL_PARAM *settings, const OSSL_PARAM *options,
                      const OSSL_DISPATCH *fns, void *cbarg, void *rlarg,
                      OSSL_RECORD_LAYER **retrl)
{
    int ret;

    ret = tls_int_new_record_layer(libctx, propq, vers, role, direction, level,
                                   ciph, taglen, md, comp, prev,
                                   transport, next, settings,
                                   options, fns, cbarg, retrl);

    if (ret != OSSL_RECORD_RETURN_SUCCESS)
        return ret;

    (*retrl)->unprocessed_rcds = pqueue_new();
    (*retrl)->processed_rcds = pqueue_new();

    if ((*retrl)->unprocessed_rcds == NULL
            || (*retrl)->processed_rcds == NULL) {
        dtls_free(*retrl);
        *retrl = NULL;
        ERR_raise(ERR_LIB_SSL, ERR_R_SSL_LIB);
        return OSSL_RECORD_RETURN_FATAL;
    }

    (*retrl)->isdtls = 1;
    (*retrl)->epoch = epoch;
    (*retrl)->in_init = 1;

    switch (vers) {
    case DTLS_ANY_VERSION:
        (*retrl)->funcs = &dtls_any_funcs;
        break;
    case DTLS1_2_VERSION:
    case DTLS1_VERSION:
    case DTLS1_BAD_VER:
        (*retrl)->funcs = &dtls_1_funcs;
        break;
    default:
        /* Should not happen */
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        ret = OSSL_RECORD_RETURN_FATAL;
        goto err;
    }

    ret = (*retrl)->funcs->set_crypto_state(*retrl, level, key, keylen, iv,
                                            ivlen, mackey, mackeylen, ciph,
                                            taglen, mactype, md, comp);

 err:
    if (ret != OSSL_RECORD_RETURN_SUCCESS) {
        dtls_free(*retrl);
        *retrl = NULL;
    }
    return ret;
}

int dtls_prepare_record_header(OSSL_RECORD_LAYER *rl,
                               WPACKET *thispkt,
                               OSSL_RECORD_TEMPLATE *templ,
                               uint8_t rectype,
                               unsigned char **recdata)
{
    size_t maxcomplen;

    *recdata = NULL;

    maxcomplen = templ->buflen;
    if (rl->compctx != NULL)
        maxcomplen += SSL3_RT_MAX_COMPRESSED_OVERHEAD;

    if (!WPACKET_put_bytes_u8(thispkt, rectype)
            || !WPACKET_put_bytes_u16(thispkt, templ->version)
            || !WPACKET_put_bytes_u16(thispkt, rl->epoch)
            || !WPACKET_memcpy(thispkt, &(rl->sequence[2]), 6)
            || !WPACKET_start_sub_packet_u16(thispkt)
            || (rl->eivlen > 0
                && !WPACKET_allocate_bytes(thispkt, rl->eivlen, NULL))
            || (maxcomplen > 0
                && !WPACKET_reserve_bytes(thispkt, maxcomplen,
                                          recdata))) {
        RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    return 1;
}

int dtls_post_encryption_processing(OSSL_RECORD_LAYER *rl,
                                    size_t mac_size,
                                    OSSL_RECORD_TEMPLATE *thistempl,
                                    WPACKET *thispkt,
                                    TLS_RL_RECORD *thiswr)
{
    if (!tls_post_encryption_processing_default(rl, mac_size, thistempl,
                                                thispkt, thiswr)) {
        /* RLAYERfatal() already called */
        return 0;
    }

    return tls_increment_sequence_ctr(rl);
}

static size_t dtls_get_max_record_overhead(OSSL_RECORD_LAYER *rl)
{
    size_t blocksize = 0;

    if (rl->enc_ctx != NULL &&
        (EVP_CIPHER_CTX_get_mode(rl->enc_ctx) == EVP_CIPH_CBC_MODE))
        blocksize = EVP_CIPHER_CTX_get_block_size(rl->enc_ctx);

    /*
     * If we have a cipher in place then the tag is mandatory. If the cipher is
     * CBC mode then an explicit IV is also mandatory. If we know the digest,
     * then we check it is consistent with the taglen. In the case of stitched
     * ciphers or AEAD ciphers we don't now the digest (or there isn't one) so
     * we just trust that the taglen is correct.
     */
    assert(rl->enc_ctx == NULL  || ((blocksize == 0 || rl->eivlen > 0)
                                    && rl->taglen > 0));
    assert(rl->md == NULL || (int)rl->taglen == EVP_MD_size(rl->md));

    /*
     * Record overhead consists of the record header, the explicit IV, any
     * expansion due to cbc padding, and the mac/tag len. There could be
     * further expansion due to compression - but we don't know what this will
     * be without knowing the length of the data. However when this function is
     * called we don't know what the length will be yet - so this is a catch-22.
     * We *could* use SSL_3_RT_MAX_COMPRESSED_OVERHEAD which is an upper limit
     * for the maximum record size. But this value is larger than our fallback
     * MTU size - so isn't very helpful. We just ignore potential expansion
     * due to compression.
     */
    return DTLS1_RT_HEADER_LENGTH + rl->eivlen + blocksize + rl->taglen;
}

const OSSL_RECORD_METHOD ossl_dtls_record_method = {
    dtls_new_record_layer,
    dtls_free,
    tls_unprocessed_read_pending,
    tls_processed_read_pending,
    tls_app_data_pending,
    tls_get_max_records,
    tls_write_records,
    tls_retry_write_records,
    tls_read_record,
    tls_release_record,
    tls_get_alert_code,
    tls_set1_bio,
    tls_set_protocol_version,
    NULL,
    tls_set_first_handshake,
    tls_set_max_pipelines,
    dtls_set_in_init,
    tls_get_state,
    tls_set_options,
    tls_get_compression,
    tls_set_max_frag_len,
    dtls_get_max_record_overhead,
    tls_increment_sequence_ctr,
    tls_alloc_buffers,
    tls_free_buffers
};
