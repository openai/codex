/*
 * Copyright 2022-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <assert.h>
#include <openssl/bio.h>
#include <openssl/ssl.h>
#include <openssl/err.h>
#include <openssl/core_names.h>
#include <openssl/comp.h>
#include <openssl/ssl.h>
#include "internal/e_os.h"
#include "internal/packet.h"
#include "internal/ssl3_cbc.h"
#include "../../ssl_local.h"
#include "../record_local.h"
#include "recmethod_local.h"

static void tls_int_free(OSSL_RECORD_LAYER *rl);

void ossl_tls_buffer_release(TLS_BUFFER *b)
{
    OPENSSL_free(b->buf);
    b->buf = NULL;
}

static void TLS_RL_RECORD_release(TLS_RL_RECORD *r, size_t num_recs)
{
    size_t i;

    for (i = 0; i < num_recs; i++) {
        OPENSSL_free(r[i].comp);
        r[i].comp = NULL;
    }
}

void ossl_tls_rl_record_set_seq_num(TLS_RL_RECORD *r,
                                    const unsigned char *seq_num)
{
    memcpy(r->seq_num, seq_num, SEQ_NUM_SIZE);
}

void ossl_rlayer_fatal(OSSL_RECORD_LAYER *rl, int al, int reason,
                       const char *fmt, ...)
{
    va_list args;

    va_start(args, fmt);
    ERR_vset_error(ERR_LIB_SSL, reason, fmt, args);
    va_end(args);

    rl->alert = al;
}

int ossl_set_tls_provider_parameters(OSSL_RECORD_LAYER *rl,
                                     EVP_CIPHER_CTX *ctx,
                                     const EVP_CIPHER *ciph,
                                     const EVP_MD *md)
{
    /*
     * Provided cipher, the TLS padding/MAC removal is performed provider
     * side so we need to tell the ctx about our TLS version and mac size
     */
    OSSL_PARAM params[3], *pprm = params;
    size_t macsize = 0;
    int imacsize = -1;

    if ((EVP_CIPHER_get_flags(ciph) & EVP_CIPH_FLAG_AEAD_CIPHER) == 0
            && !rl->use_etm)
        imacsize = EVP_MD_get_size(md);
    if (imacsize > 0)
        macsize = (size_t)imacsize;

    *pprm++ = OSSL_PARAM_construct_int(OSSL_CIPHER_PARAM_TLS_VERSION,
                                       &rl->version);
    *pprm++ = OSSL_PARAM_construct_size_t(OSSL_CIPHER_PARAM_TLS_MAC_SIZE,
                                          &macsize);
    *pprm = OSSL_PARAM_construct_end();

    if (!EVP_CIPHER_CTX_set_params(ctx, params)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    return 1;
}

/*
 * ssl3_cbc_record_digest_supported returns 1 iff |ctx| uses a hash function
 * which ssl3_cbc_digest_record supports.
 */
char ssl3_cbc_record_digest_supported(const EVP_MD_CTX *ctx)
{
    switch (EVP_MD_CTX_get_type(ctx)) {
    case NID_md5:
    case NID_sha1:
    case NID_sha224:
    case NID_sha256:
    case NID_sha384:
    case NID_sha512:
        return 1;
    default:
        return 0;
    }
}

#ifndef OPENSSL_NO_COMP
static int tls_allow_compression(OSSL_RECORD_LAYER *rl)
{
    if (rl->options & SSL_OP_NO_COMPRESSION)
        return 0;

    return rl->security == NULL
           || rl->security(rl->cbarg, SSL_SECOP_COMPRESSION, 0, 0, NULL);
}
#endif

static void tls_release_write_buffer_int(OSSL_RECORD_LAYER *rl, size_t start)
{
    TLS_BUFFER *wb;
    size_t pipes;

    pipes = rl->numwpipes;

    while (pipes > start) {
        wb = &rl->wbuf[pipes - 1];

        if (TLS_BUFFER_is_app_buffer(wb))
            TLS_BUFFER_set_app_buffer(wb, 0);
        else
            OPENSSL_free(wb->buf);
        wb->buf = NULL;
        pipes--;
    }
}

int tls_setup_write_buffer(OSSL_RECORD_LAYER *rl, size_t numwpipes,
                           size_t firstlen, size_t nextlen)
{
    unsigned char *p;
    size_t maxalign = 0, headerlen;
    TLS_BUFFER *wb;
    size_t currpipe;
    size_t defltlen = 0;
    size_t contenttypelen = 0;

    if (firstlen == 0 || (numwpipes > 1 && nextlen == 0)) {
        if (rl->isdtls)
            headerlen = DTLS1_RT_HEADER_LENGTH + 1;
        else
            headerlen = SSL3_RT_HEADER_LENGTH;

        /* TLSv1.3 adds an extra content type byte after payload data */
        if (rl->version == TLS1_3_VERSION)
            contenttypelen = 1;

#if defined(SSL3_ALIGN_PAYLOAD) && SSL3_ALIGN_PAYLOAD != 0
        maxalign = SSL3_ALIGN_PAYLOAD - 1;
#endif

        defltlen = maxalign + headerlen + rl->eivlen + rl->max_frag_len
                   + contenttypelen + SSL3_RT_SEND_MAX_ENCRYPTED_OVERHEAD;
#ifndef OPENSSL_NO_COMP
        if (tls_allow_compression(rl))
            defltlen += SSL3_RT_MAX_COMPRESSED_OVERHEAD;
#endif
        /*
         * We don't need to add eivlen here since empty fragments only occur
         * when we don't have an explicit IV. The contenttype byte will also
         * always be 0 in these protocol versions
         */
        if ((rl->options & SSL_OP_DONT_INSERT_EMPTY_FRAGMENTS) == 0)
            defltlen += headerlen + maxalign + SSL3_RT_SEND_MAX_ENCRYPTED_OVERHEAD;
    }

    wb = rl->wbuf;
    for (currpipe = 0; currpipe < numwpipes; currpipe++) {
        TLS_BUFFER *thiswb = &wb[currpipe];
        size_t len = (currpipe == 0) ? firstlen : nextlen;

        if (len == 0)
            len = defltlen;

        if (thiswb->len != len) {
            OPENSSL_free(thiswb->buf);
            thiswb->buf = NULL;         /* force reallocation */
        }

        p = thiswb->buf;
        if (p == NULL) {
            p = OPENSSL_malloc(len);
            if (p == NULL) {
                if (rl->numwpipes < currpipe)
                    rl->numwpipes = currpipe;
                /*
                 * We've got a malloc failure, and we're still initialising
                 * buffers. We assume we're so doomed that we won't even be able
                 * to send an alert.
                 */
                RLAYERfatal(rl, SSL_AD_NO_ALERT, ERR_R_CRYPTO_LIB);
                return 0;
            }
        }
        memset(thiswb, 0, sizeof(TLS_BUFFER));
        thiswb->buf = p;
        thiswb->len = len;
    }

    /* Free any previously allocated buffers that we are no longer using */
    tls_release_write_buffer_int(rl, currpipe);

    rl->numwpipes = numwpipes;

    return 1;
}

static void tls_release_write_buffer(OSSL_RECORD_LAYER *rl)
{
    tls_release_write_buffer_int(rl, 0);

    rl->numwpipes = 0;
}

int tls_setup_read_buffer(OSSL_RECORD_LAYER *rl)
{
    unsigned char *p;
    size_t len, maxalign = 0, headerlen;
    TLS_BUFFER *b;

    b = &rl->rbuf;

    if (rl->isdtls)
        headerlen = DTLS1_RT_HEADER_LENGTH;
    else
        headerlen = SSL3_RT_HEADER_LENGTH;

#if defined(SSL3_ALIGN_PAYLOAD) && SSL3_ALIGN_PAYLOAD != 0
    maxalign = SSL3_ALIGN_PAYLOAD - 1;
#endif

    if (b->buf == NULL) {
        len = rl->max_frag_len
              + SSL3_RT_MAX_ENCRYPTED_OVERHEAD + headerlen + maxalign;
#ifndef OPENSSL_NO_COMP
        if (tls_allow_compression(rl))
            len += SSL3_RT_MAX_COMPRESSED_OVERHEAD;
#endif

        /* Ensure our buffer is large enough to support all our pipelines */
        if (rl->max_pipelines > 1)
            len *= rl->max_pipelines;

        if (b->default_len > len)
            len = b->default_len;

        if ((p = OPENSSL_malloc(len)) == NULL) {
            /*
             * We've got a malloc failure, and we're still initialising buffers.
             * We assume we're so doomed that we won't even be able to send an
             * alert.
             */
            RLAYERfatal(rl, SSL_AD_NO_ALERT, ERR_R_CRYPTO_LIB);
            return 0;
        }
        b->buf = p;
        b->len = len;
    }

    return 1;
}

static int tls_release_read_buffer(OSSL_RECORD_LAYER *rl)
{
    TLS_BUFFER *b;

    b = &rl->rbuf;
    if ((rl->options & SSL_OP_CLEANSE_PLAINTEXT) != 0)
        OPENSSL_cleanse(b->buf, b->len);
    OPENSSL_free(b->buf);
    b->buf = NULL;
    rl->packet = NULL;
    rl->packet_length = 0;
    return 1;
}

/*
 * Return values are as per SSL_read()
 */
int tls_default_read_n(OSSL_RECORD_LAYER *rl, size_t n, size_t max, int extend,
                       int clearold, size_t *readbytes)
{
    /*
     * If extend == 0, obtain new n-byte packet; if extend == 1, increase
     * packet by another n bytes. The packet will be in the sub-array of
     * rl->rbuf.buf specified by rl->packet and rl->packet_length. (If
     * rl->read_ahead is set, 'max' bytes may be stored in rbuf [plus
     * rl->packet_length bytes if extend == 1].) if clearold == 1, move the
     * packet to the start of the buffer; if clearold == 0 then leave any old
     * packets where they were
     */
    size_t len, left, align = 0;
    unsigned char *pkt;
    TLS_BUFFER *rb;

    if (n == 0)
        return OSSL_RECORD_RETURN_NON_FATAL_ERR;

    rb = &rl->rbuf;
    left = rb->left;
#if defined(SSL3_ALIGN_PAYLOAD) && SSL3_ALIGN_PAYLOAD != 0
    align = (size_t)rb->buf + SSL3_RT_HEADER_LENGTH;
    align = SSL3_ALIGN_PAYLOAD - 1 - ((align - 1) % SSL3_ALIGN_PAYLOAD);
#endif

    if (!extend) {
        /* start with empty packet ... */
        if (left == 0)
            rb->offset = align;

        rl->packet = rb->buf + rb->offset;
        rl->packet_length = 0;
        /* ... now we can act as if 'extend' was set */
    }

    if (!ossl_assert(rl->packet != NULL)) {
        /* does not happen */
        RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return OSSL_RECORD_RETURN_FATAL;
    }

    len = rl->packet_length;
    pkt = rb->buf + align;
    /*
     * Move any available bytes to front of buffer: 'len' bytes already
     * pointed to by 'packet', 'left' extra ones at the end
     */
    if (rl->packet != pkt && clearold == 1) {
        memmove(pkt, rl->packet, len + left);
        rl->packet = pkt;
        rb->offset = len + align;
    }

    /*
     * For DTLS/UDP reads should not span multiple packets because the read
     * operation returns the whole packet at once (as long as it fits into
     * the buffer).
     */
    if (rl->isdtls) {
        if (left == 0 && extend) {
            /*
             * We received a record with a header but no body data. This will
             * get dumped.
             */
            return OSSL_RECORD_RETURN_NON_FATAL_ERR;
        }
        if (left > 0 && n > left)
            n = left;
    }

    /* if there is enough in the buffer from a previous read, take some */
    if (left >= n) {
        rl->packet_length += n;
        rb->left = left - n;
        rb->offset += n;
        *readbytes = n;
        return OSSL_RECORD_RETURN_SUCCESS;
    }

    /* else we need to read more data */

    if (n > rb->len - rb->offset) {
        /* does not happen */
        RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return OSSL_RECORD_RETURN_FATAL;
    }

    /* We always act like read_ahead is set for DTLS */
    if (!rl->read_ahead && !rl->isdtls) {
        /* ignore max parameter */
        max = n;
    } else {
        if (max < n)
            max = n;
        if (max > rb->len - rb->offset)
            max = rb->len - rb->offset;
    }

    while (left < n) {
        size_t bioread = 0;
        int ret;
        BIO *bio = rl->prev != NULL ? rl->prev : rl->bio;

        /*
         * Now we have len+left bytes at the front of rl->rbuf.buf and
         * need to read in more until we have len + n (up to len + max if
         * possible)
         */

        clear_sys_error();
        if (bio != NULL) {
            ret = BIO_read(bio, pkt + len + left, max - left);
            if (ret > 0) {
                bioread = ret;
                ret = OSSL_RECORD_RETURN_SUCCESS;
            } else if (BIO_should_retry(bio)) {
                if (rl->prev != NULL) {
                    /*
                     * We were reading from the previous epoch. Now there is no
                     * more data, so swap to the actual transport BIO
                     */
                    BIO_free(rl->prev);
                    rl->prev = NULL;
                    continue;
                }
                ret = OSSL_RECORD_RETURN_RETRY;
            } else if (BIO_eof(bio)) {
                ret = OSSL_RECORD_RETURN_EOF;
            } else {
                ret = OSSL_RECORD_RETURN_FATAL;
            }
        } else {
            RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, SSL_R_READ_BIO_NOT_SET);
            ret = OSSL_RECORD_RETURN_FATAL;
        }

        if (ret <= OSSL_RECORD_RETURN_RETRY) {
            rb->left = left;
            if ((rl->mode & SSL_MODE_RELEASE_BUFFERS) != 0 && !rl->isdtls)
                if (len + left == 0)
                    tls_release_read_buffer(rl);
            return ret;
        }
        left += bioread;
        /*
         * reads should *never* span multiple packets for DTLS because the
         * underlying transport protocol is message oriented as opposed to
         * byte oriented as in the TLS case.
         */
        if (rl->isdtls) {
            if (n > left)
                n = left;       /* makes the while condition false */
        }
    }

    /* done reading, now the book-keeping */
    rb->offset += n;
    rb->left = left - n;
    rl->packet_length += n;
    *readbytes = n;
    return OSSL_RECORD_RETURN_SUCCESS;
}

/*
 * Peeks ahead into "read_ahead" data to see if we have a whole record waiting
 * for us in the buffer.
 */
static int tls_record_app_data_waiting(OSSL_RECORD_LAYER *rl)
{
    TLS_BUFFER *rbuf;
    size_t left, len;
    unsigned char *p;

    rbuf = &rl->rbuf;

    p = TLS_BUFFER_get_buf(rbuf);
    if (p == NULL)
        return 0;

    left = TLS_BUFFER_get_left(rbuf);

    if (left < SSL3_RT_HEADER_LENGTH)
        return 0;

    p += TLS_BUFFER_get_offset(rbuf);

    /*
     * We only check the type and record length, we will sanity check version
     * etc later
     */
    if (*p != SSL3_RT_APPLICATION_DATA)
        return 0;

    p += 3;
    n2s(p, len);

    if (left < SSL3_RT_HEADER_LENGTH + len)
        return 0;

    return 1;
}

static int rlayer_early_data_count_ok(OSSL_RECORD_LAYER *rl, size_t length,
                                      size_t overhead, int send)
{
    uint32_t max_early_data = rl->max_early_data;

    if (max_early_data == 0) {
        RLAYERfatal(rl, send ? SSL_AD_INTERNAL_ERROR : SSL_AD_UNEXPECTED_MESSAGE,
                    SSL_R_TOO_MUCH_EARLY_DATA);
        return 0;
    }

    /* If we are dealing with ciphertext we need to allow for the overhead */
    max_early_data += overhead;

    if (rl->early_data_count + length > max_early_data) {
        RLAYERfatal(rl, send ? SSL_AD_INTERNAL_ERROR : SSL_AD_UNEXPECTED_MESSAGE,
                    SSL_R_TOO_MUCH_EARLY_DATA);
        return 0;
    }
    rl->early_data_count += length;

    return 1;
}

/*
 * MAX_EMPTY_RECORDS defines the number of consecutive, empty records that
 * will be processed per call to tls_get_more_records. Without this limit an
 * attacker could send empty records at a faster rate than we can process and
 * cause tls_get_more_records to loop forever.
 */
#define MAX_EMPTY_RECORDS 32

#define SSL2_RT_HEADER_LENGTH   2

/*-
 * Call this to buffer new input records in rl->rrec.
 * It will return a OSSL_RECORD_RETURN_* value.
 * When it finishes successfully (OSSL_RECORD_RETURN_SUCCESS), |rl->num_recs|
 * records have been decoded. For each record 'i':
 * rrec[i].type    - is the type of record
 * rrec[i].data,   - data
 * rrec[i].length, - number of bytes
 * Multiple records will only be returned if the record types are all
 * SSL3_RT_APPLICATION_DATA. The number of records returned will always be <=
 * |max_pipelines|
 */
int tls_get_more_records(OSSL_RECORD_LAYER *rl)
{
    int enc_err, rret;
    int i;
    size_t more, n;
    TLS_RL_RECORD *rr, *thisrr;
    TLS_BUFFER *rbuf;
    unsigned char *p;
    unsigned char md[EVP_MAX_MD_SIZE];
    unsigned int version;
    size_t mac_size = 0;
    int imac_size;
    size_t num_recs = 0, max_recs, j;
    PACKET pkt, sslv2pkt;
    SSL_MAC_BUF *macbufs = NULL;
    int ret = OSSL_RECORD_RETURN_FATAL;

    rr = rl->rrec;
    rbuf = &rl->rbuf;
    if (rbuf->buf == NULL) {
        if (!tls_setup_read_buffer(rl)) {
            /* RLAYERfatal() already called */
            return OSSL_RECORD_RETURN_FATAL;
        }
    }

    max_recs = rl->max_pipelines;

    if (max_recs == 0)
        max_recs = 1;

    do {
        thisrr = &rr[num_recs];

        /* check if we have the header */
        if ((rl->rstate != SSL_ST_READ_BODY) ||
            (rl->packet_length < SSL3_RT_HEADER_LENGTH)) {
            size_t sslv2len;
            unsigned int type;

            rret = rl->funcs->read_n(rl, SSL3_RT_HEADER_LENGTH,
                                     TLS_BUFFER_get_len(rbuf), 0,
                                     num_recs == 0 ? 1 : 0, &n);

            if (rret < OSSL_RECORD_RETURN_SUCCESS)
                return rret; /* error or non-blocking */

            rl->rstate = SSL_ST_READ_BODY;

            p = rl->packet;
            if (!PACKET_buf_init(&pkt, p, rl->packet_length)) {
                RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
                return OSSL_RECORD_RETURN_FATAL;
            }
            sslv2pkt = pkt;
            if (!PACKET_get_net_2_len(&sslv2pkt, &sslv2len)
                    || !PACKET_get_1(&sslv2pkt, &type)) {
                RLAYERfatal(rl, SSL_AD_DECODE_ERROR, ERR_R_INTERNAL_ERROR);
                return OSSL_RECORD_RETURN_FATAL;
            }
            /*
             * The first record received by the server may be a V2ClientHello.
             */
            if (rl->role == OSSL_RECORD_ROLE_SERVER
                    && rl->is_first_record
                    && (sslv2len & 0x8000) != 0
                    && (type == SSL2_MT_CLIENT_HELLO)) {
                /*
                 *  SSLv2 style record
                 *
                 * |num_recs| here will actually always be 0 because
                 * |num_recs > 0| only ever occurs when we are processing
                 * multiple app data records - which we know isn't the case here
                 * because it is an SSLv2ClientHello. We keep it using
                 * |num_recs| for the sake of consistency
                 */
                thisrr->type = SSL3_RT_HANDSHAKE;
                thisrr->rec_version = SSL2_VERSION;

                thisrr->length = sslv2len & 0x7fff;

                if (thisrr->length > TLS_BUFFER_get_len(rbuf)
                                     - SSL2_RT_HEADER_LENGTH) {
                    RLAYERfatal(rl, SSL_AD_RECORD_OVERFLOW,
                                SSL_R_PACKET_LENGTH_TOO_LONG);
                    return OSSL_RECORD_RETURN_FATAL;
                }
            } else {
                /* SSLv3+ style record */

                /* Pull apart the header into the TLS_RL_RECORD */
                if (!PACKET_get_1(&pkt, &type)
                        || !PACKET_get_net_2(&pkt, &version)
                        || !PACKET_get_net_2_len(&pkt, &thisrr->length)) {
                    if (rl->msg_callback != NULL)
                        rl->msg_callback(0, 0, SSL3_RT_HEADER, p, 5, rl->cbarg);
                    RLAYERfatal(rl, SSL_AD_DECODE_ERROR, ERR_R_INTERNAL_ERROR);
                    return OSSL_RECORD_RETURN_FATAL;
                }
                thisrr->type = type;
                thisrr->rec_version = version;

                /*
                 * When we call validate_record_header() only records actually
                 * received in SSLv2 format should have the record version set
                 * to SSL2_VERSION. This way validate_record_header() can know
                 * what format the record was in based on the version.
                 */
                if (thisrr->rec_version == SSL2_VERSION) {
                    RLAYERfatal(rl, SSL_AD_PROTOCOL_VERSION,
                                SSL_R_WRONG_VERSION_NUMBER);
                    return OSSL_RECORD_RETURN_FATAL;
                }

                if (rl->msg_callback != NULL)
                    rl->msg_callback(0, version, SSL3_RT_HEADER, p, 5, rl->cbarg);

                if (thisrr->length >
                    TLS_BUFFER_get_len(rbuf) - SSL3_RT_HEADER_LENGTH) {
                    RLAYERfatal(rl, SSL_AD_RECORD_OVERFLOW,
                                SSL_R_PACKET_LENGTH_TOO_LONG);
                    return OSSL_RECORD_RETURN_FATAL;
                }
            }

            if (!rl->funcs->validate_record_header(rl, thisrr)) {
                /* RLAYERfatal already called */
                return OSSL_RECORD_RETURN_FATAL;
            }

            /* now rl->rstate == SSL_ST_READ_BODY */
        }

        /*
         * rl->rstate == SSL_ST_READ_BODY, get and decode the data. Calculate
         * how much more data we need to read for the rest of the record
         */
        if (thisrr->rec_version == SSL2_VERSION) {
            more = thisrr->length + SSL2_RT_HEADER_LENGTH
                   - SSL3_RT_HEADER_LENGTH;
        } else {
            more = thisrr->length;
        }

        if (more > 0) {
            /* now rl->packet_length == SSL3_RT_HEADER_LENGTH */

            rret = rl->funcs->read_n(rl, more, more, 1, 0, &n);
            if (rret < OSSL_RECORD_RETURN_SUCCESS)
                return rret;     /* error or non-blocking io */
        }

        /* set state for later operations */
        rl->rstate = SSL_ST_READ_HEADER;

        /*
         * At this point, rl->packet_length == SSL3_RT_HEADER_LENGTH
         * + thisrr->length, or rl->packet_length == SSL2_RT_HEADER_LENGTH
         * + thisrr->length and we have that many bytes in rl->packet
         */
        if (thisrr->rec_version == SSL2_VERSION)
            thisrr->input = &(rl->packet[SSL2_RT_HEADER_LENGTH]);
        else
            thisrr->input = &(rl->packet[SSL3_RT_HEADER_LENGTH]);

        /*
         * ok, we can now read from 'rl->packet' data into 'thisrr'.
         * thisrr->input points at thisrr->length bytes, which need to be copied
         * into thisrr->data by either the decryption or by the decompression.
         * When the data is 'copied' into the thisrr->data buffer,
         * thisrr->input will be updated to point at the new buffer
         */

        /*
         * We now have - encrypted [ MAC [ compressed [ plain ] ] ]
         * thisrr->length bytes of encrypted compressed stuff.
         */

        /* decrypt in place in 'thisrr->input' */
        thisrr->data = thisrr->input;
        thisrr->orig_len = thisrr->length;

        num_recs++;

        /* we have pulled in a full packet so zero things */
        rl->packet_length = 0;
        rl->is_first_record = 0;
    } while (num_recs < max_recs
             && thisrr->type == SSL3_RT_APPLICATION_DATA
             && RLAYER_USE_EXPLICIT_IV(rl)
             && rl->enc_ctx != NULL
             && (EVP_CIPHER_get_flags(EVP_CIPHER_CTX_get0_cipher(rl->enc_ctx))
                 & EVP_CIPH_FLAG_PIPELINE) != 0
             && tls_record_app_data_waiting(rl));

    if (num_recs == 1
            && thisrr->type == SSL3_RT_CHANGE_CIPHER_SPEC
               /* The following can happen in tlsany_meth after HRR */
            && rl->version == TLS1_3_VERSION
            && rl->is_first_handshake) {
        /*
         * CCS messages must be exactly 1 byte long, containing the value 0x01
         */
        if (thisrr->length != 1 || thisrr->data[0] != 0x01) {
            RLAYERfatal(rl, SSL_AD_UNEXPECTED_MESSAGE,
                        SSL_R_INVALID_CCS_MESSAGE);
            return OSSL_RECORD_RETURN_FATAL;
        }
        /*
         * CCS messages are ignored in TLSv1.3. We treat it like an empty
         * handshake record - but we still call the msg_callback
         */
        if (rl->msg_callback != NULL)
            rl->msg_callback(0, TLS1_3_VERSION, SSL3_RT_CHANGE_CIPHER_SPEC,
                             thisrr->data, 1, rl->cbarg);
        thisrr->type = SSL3_RT_HANDSHAKE;
        if (++(rl->empty_record_count) > MAX_EMPTY_RECORDS) {
            RLAYERfatal(rl, SSL_AD_UNEXPECTED_MESSAGE,
                        SSL_R_UNEXPECTED_CCS_MESSAGE);
            return OSSL_RECORD_RETURN_FATAL;
        }
        rl->num_recs = 0;
        rl->curr_rec = 0;
        rl->num_released = 0;

        return OSSL_RECORD_RETURN_SUCCESS;
    }

    if (rl->md_ctx != NULL) {
        const EVP_MD *tmpmd = EVP_MD_CTX_get0_md(rl->md_ctx);

        if (tmpmd != NULL) {
            imac_size = EVP_MD_get_size(tmpmd);
            if (!ossl_assert(imac_size > 0 && imac_size <= EVP_MAX_MD_SIZE)) {
                RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, ERR_R_EVP_LIB);
                return OSSL_RECORD_RETURN_FATAL;
            }
            mac_size = (size_t)imac_size;
        }
    }

    /*
     * If in encrypt-then-mac mode calculate mac from encrypted record. All
     * the details below are public so no timing details can leak.
     */
    if (rl->use_etm && rl->md_ctx != NULL) {
        unsigned char *mac;

        for (j = 0; j < num_recs; j++) {
            thisrr = &rr[j];

            if (thisrr->length < mac_size) {
                RLAYERfatal(rl, SSL_AD_DECODE_ERROR, SSL_R_LENGTH_TOO_SHORT);
                return OSSL_RECORD_RETURN_FATAL;
            }
            thisrr->length -= mac_size;
            mac = thisrr->data + thisrr->length;
            i = rl->funcs->mac(rl, thisrr, md, 0 /* not send */);
            if (i == 0 || CRYPTO_memcmp(md, mac, mac_size) != 0) {
                RLAYERfatal(rl, SSL_AD_BAD_RECORD_MAC,
                            SSL_R_DECRYPTION_FAILED_OR_BAD_RECORD_MAC);
                return OSSL_RECORD_RETURN_FATAL;
            }
        }
        /*
         * We've handled the mac now - there is no MAC inside the encrypted
         * record
         */
        mac_size = 0;
    }

    if (mac_size > 0) {
        macbufs = OPENSSL_zalloc(sizeof(*macbufs) * num_recs);
        if (macbufs == NULL) {
            RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, ERR_R_CRYPTO_LIB);
            return OSSL_RECORD_RETURN_FATAL;
        }
    }

    ERR_set_mark();
    enc_err = rl->funcs->cipher(rl, rr, num_recs, 0, macbufs, mac_size);

    /*-
     * enc_err is:
     *    0: if the record is publicly invalid, or an internal error, or AEAD
     *       decryption failed, or ETM decryption failed.
     *    1: Success or MTE decryption failed (MAC will be randomised)
     */
    if (enc_err == 0) {
        if (rl->alert != SSL_AD_NO_ALERT) {
            /* RLAYERfatal() already got called */
            ERR_clear_last_mark();
            goto end;
        }
        if (num_recs == 1
                && rl->skip_early_data != NULL
                && rl->skip_early_data(rl->cbarg)) {
            /*
             * Valid early_data that we cannot decrypt will fail here. We treat
             * it like an empty record.
             */

            /*
             * Remove any errors from the stack. Decryption failures are normal
             * behaviour.
             */
            ERR_pop_to_mark();

            thisrr = &rr[0];

            if (!rlayer_early_data_count_ok(rl, thisrr->length,
                                            EARLY_DATA_CIPHERTEXT_OVERHEAD, 0)) {
                /* RLAYERfatal() already called */
                goto end;
            }

            thisrr->length = 0;
            rl->num_recs = 0;
            rl->curr_rec = 0;
            rl->num_released = 0;
            /* Reset the read sequence */
            memset(rl->sequence, 0, sizeof(rl->sequence));
            ret = 1;
            goto end;
        }
        ERR_clear_last_mark();
        RLAYERfatal(rl, SSL_AD_BAD_RECORD_MAC,
                    SSL_R_DECRYPTION_FAILED_OR_BAD_RECORD_MAC);
        goto end;
    } else {
        ERR_clear_last_mark();
    }
    OSSL_TRACE_BEGIN(TLS) {
        BIO_printf(trc_out, "dec %lu\n", (unsigned long)rr[0].length);
        BIO_dump_indent(trc_out, rr[0].data, rr[0].length, 4);
    } OSSL_TRACE_END(TLS);

    /* r->length is now the compressed data plus mac */
    if (rl->enc_ctx != NULL
            && !rl->use_etm
            && EVP_MD_CTX_get0_md(rl->md_ctx) != NULL) {
        for (j = 0; j < num_recs; j++) {
            SSL_MAC_BUF *thismb = &macbufs[j];

            thisrr = &rr[j];

            i = rl->funcs->mac(rl, thisrr, md, 0 /* not send */);
            if (i == 0 || thismb == NULL || thismb->mac == NULL
                || CRYPTO_memcmp(md, thismb->mac, (size_t)mac_size) != 0)
                enc_err = 0;
            if (thisrr->length > SSL3_RT_MAX_COMPRESSED_LENGTH + mac_size)
                enc_err = 0;
#ifdef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
            if (enc_err == 0 && mac_size > 0 && thismb != NULL &&
                thismb->mac != NULL && (md[0] ^ thismb->mac[0]) != 0xFF) {
                enc_err = 1;
            }
#endif
        }
    }

    if (enc_err == 0) {
        if (rl->alert != SSL_AD_NO_ALERT) {
            /* We already called RLAYERfatal() */
            goto end;
        }
        /*
         * A separate 'decryption_failed' alert was introduced with TLS 1.0,
         * SSL 3.0 only has 'bad_record_mac'.  But unless a decryption
         * failure is directly visible from the ciphertext anyway, we should
         * not reveal which kind of error occurred -- this might become
         * visible to an attacker (e.g. via a logfile)
         */
        RLAYERfatal(rl, SSL_AD_BAD_RECORD_MAC,
                    SSL_R_DECRYPTION_FAILED_OR_BAD_RECORD_MAC);
        goto end;
    }

    for (j = 0; j < num_recs; j++) {
        thisrr = &rr[j];

        if (!rl->funcs->post_process_record(rl, thisrr)) {
            /* RLAYERfatal already called */
            goto end;
        }

        /*
         * Record overflow checking (e.g. checking if
         * thisrr->length > SSL3_RT_MAX_PLAIN_LENGTH) is the responsibility of
         * the post_process_record() function above. However we check here if
         * the received packet overflows the current Max Fragment Length setting
         * if there is one.
         * Note: rl->max_frag_len != SSL3_RT_MAX_PLAIN_LENGTH and KTLS are
         * mutually exclusive. Also note that with KTLS thisrr->length can
         * be > SSL3_RT_MAX_PLAIN_LENGTH (and rl->max_frag_len must be ignored)
         */
        if (rl->max_frag_len != SSL3_RT_MAX_PLAIN_LENGTH
                && thisrr->length > rl->max_frag_len) {
            RLAYERfatal(rl, SSL_AD_RECORD_OVERFLOW, SSL_R_DATA_LENGTH_TOO_LONG);
            goto end;
        }

        thisrr->off = 0;
        /*-
         * So at this point the following is true
         * thisrr->type   is the type of record
         * thisrr->length == number of bytes in record
         * thisrr->off    == offset to first valid byte
         * thisrr->data   == where to take bytes from, increment after use :-).
         */

        /* just read a 0 length packet */
        if (thisrr->length == 0) {
            if (++(rl->empty_record_count) > MAX_EMPTY_RECORDS) {
                RLAYERfatal(rl, SSL_AD_UNEXPECTED_MESSAGE,
                            SSL_R_RECORD_TOO_SMALL);
                goto end;
            }
        } else {
            rl->empty_record_count = 0;
        }
    }

    if (rl->level == OSSL_RECORD_PROTECTION_LEVEL_EARLY) {
        thisrr = &rr[0];
        if (thisrr->type == SSL3_RT_APPLICATION_DATA
                && !rlayer_early_data_count_ok(rl, thisrr->length, 0, 0)) {
            /* RLAYERfatal already called */
            goto end;
        }
    }

    rl->num_recs = num_recs;
    rl->curr_rec = 0;
    rl->num_released = 0;
    ret = OSSL_RECORD_RETURN_SUCCESS;
 end:
    if (macbufs != NULL) {
        for (j = 0; j < num_recs; j++) {
            if (macbufs[j].alloced)
                OPENSSL_free(macbufs[j].mac);
        }
        OPENSSL_free(macbufs);
    }
    return ret;
}

/* Shared by ssl3_meth and tls1_meth */
int tls_default_validate_record_header(OSSL_RECORD_LAYER *rl, TLS_RL_RECORD *rec)
{
    size_t len = SSL3_RT_MAX_ENCRYPTED_LENGTH;

    if (rec->rec_version != rl->version) {
        RLAYERfatal(rl, SSL_AD_PROTOCOL_VERSION, SSL_R_WRONG_VERSION_NUMBER);
        return 0;
    }

#ifndef OPENSSL_NO_COMP
    /*
     * If OPENSSL_NO_COMP is defined then SSL3_RT_MAX_ENCRYPTED_LENGTH
     * does not include the compression overhead anyway.
     */
    if (rl->compctx == NULL)
        len -= SSL3_RT_MAX_COMPRESSED_OVERHEAD;
#endif

    if (rec->length > len) {
        RLAYERfatal(rl, SSL_AD_RECORD_OVERFLOW,
                    SSL_R_ENCRYPTED_LENGTH_TOO_LONG);
        return 0;
    }

    return 1;
}

int tls_do_compress(OSSL_RECORD_LAYER *rl, TLS_RL_RECORD *wr)
{
#ifndef OPENSSL_NO_COMP
    int i;

    i = COMP_compress_block(rl->compctx, wr->data,
                            (int)(wr->length + SSL3_RT_MAX_COMPRESSED_OVERHEAD),
                            wr->input, (int)wr->length);
    if (i < 0)
        return 0;

    wr->length = i;
    wr->input = wr->data;
    return 1;
#else
    return 0;
#endif
}

int tls_do_uncompress(OSSL_RECORD_LAYER *rl, TLS_RL_RECORD *rec)
{
#ifndef OPENSSL_NO_COMP
    int i;

    if (rec->comp == NULL) {
        rec->comp = (unsigned char *)
            OPENSSL_malloc(SSL3_RT_MAX_ENCRYPTED_LENGTH);
    }
    if (rec->comp == NULL)
        return 0;

    i = COMP_expand_block(rl->compctx, rec->comp, SSL3_RT_MAX_PLAIN_LENGTH,
                          rec->data, (int)rec->length);
    if (i < 0)
        return 0;
    else
        rec->length = i;
    rec->data = rec->comp;
    return 1;
#else
    return 0;
#endif
}

/* Shared by tlsany_meth, ssl3_meth and tls1_meth */
int tls_default_post_process_record(OSSL_RECORD_LAYER *rl, TLS_RL_RECORD *rec)
{
    if (rl->compctx != NULL) {
        if (rec->length > SSL3_RT_MAX_COMPRESSED_LENGTH) {
            RLAYERfatal(rl, SSL_AD_RECORD_OVERFLOW,
                        SSL_R_COMPRESSED_LENGTH_TOO_LONG);
            return 0;
        }
        if (!tls_do_uncompress(rl, rec)) {
            RLAYERfatal(rl, SSL_AD_DECOMPRESSION_FAILURE,
                        SSL_R_BAD_DECOMPRESSION);
            return 0;
        }
    }

    if (rec->length > SSL3_RT_MAX_PLAIN_LENGTH) {
        RLAYERfatal(rl, SSL_AD_RECORD_OVERFLOW, SSL_R_DATA_LENGTH_TOO_LONG);
        return 0;
    }

    return 1;
}

/* Shared by tls13_meth and ktls_meth */
int tls13_common_post_process_record(OSSL_RECORD_LAYER *rl, TLS_RL_RECORD *rec)
{
    if (rec->type != SSL3_RT_APPLICATION_DATA
            && rec->type != SSL3_RT_ALERT
            && rec->type != SSL3_RT_HANDSHAKE) {
        RLAYERfatal(rl, SSL_AD_UNEXPECTED_MESSAGE, SSL_R_BAD_RECORD_TYPE);
        return 0;
    }

    if (rl->msg_callback != NULL)
        rl->msg_callback(0, rl->version, SSL3_RT_INNER_CONTENT_TYPE, &rec->type,
                        1, rl->cbarg);

    /*
     * TLSv1.3 alert and handshake records are required to be non-zero in
     * length.
     */
    if ((rec->type == SSL3_RT_HANDSHAKE || rec->type == SSL3_RT_ALERT)
            && rec->length == 0) {
        RLAYERfatal(rl, SSL_AD_UNEXPECTED_MESSAGE, SSL_R_BAD_LENGTH);
        return 0;
    }

    return 1;
}

int tls_read_record(OSSL_RECORD_LAYER *rl, void **rechandle, int *rversion,
                    uint8_t *type, const unsigned char **data, size_t *datalen,
                    uint16_t *epoch, unsigned char *seq_num)
{
    TLS_RL_RECORD *rec;

    /*
     * tls_get_more_records() can return success without actually reading
     * anything useful (i.e. if empty records are read). We loop here until
     * we have something useful. tls_get_more_records() will eventually fail if
     * too many sequential empty records are read.
     */
    while (rl->curr_rec >= rl->num_recs) {
        int ret;

        if (rl->num_released != rl->num_recs) {
            RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, SSL_R_RECORDS_NOT_RELEASED);
            return OSSL_RECORD_RETURN_FATAL;
        }

        ret = rl->funcs->get_more_records(rl);

        if (ret != OSSL_RECORD_RETURN_SUCCESS)
            return ret;
    }

    /*
     * We have now got rl->num_recs records buffered in rl->rrec. rl->curr_rec
     * points to the next one to read.
     */
    rec = &rl->rrec[rl->curr_rec++];

    *rechandle = rec;
    *rversion = rec->rec_version;
    *type = rec->type;
    *data = rec->data + rec->off;
    *datalen = rec->length;
    if (rl->isdtls) {
        *epoch = rec->epoch;
        memcpy(seq_num, rec->seq_num, sizeof(rec->seq_num));
    }

    return OSSL_RECORD_RETURN_SUCCESS;
}

int tls_release_record(OSSL_RECORD_LAYER *rl, void *rechandle, size_t length)
{
    TLS_RL_RECORD *rec = &rl->rrec[rl->num_released];

    if (!ossl_assert(rl->num_released < rl->curr_rec)
            || !ossl_assert(rechandle == rec)) {
        /* Should not happen */
        RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, SSL_R_INVALID_RECORD);
        return OSSL_RECORD_RETURN_FATAL;
    }

    if (rec->length < length) {
        /* Should not happen */
        RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return OSSL_RECORD_RETURN_FATAL;
    }

    if ((rl->options & SSL_OP_CLEANSE_PLAINTEXT) != 0)
        OPENSSL_cleanse(rec->data + rec->off, length);

    rec->off += length;
    rec->length -= length;

    if (rec->length > 0)
        return OSSL_RECORD_RETURN_SUCCESS;

    rl->num_released++;

    if (rl->curr_rec == rl->num_released
            && (rl->mode & SSL_MODE_RELEASE_BUFFERS) != 0
            && TLS_BUFFER_get_left(&rl->rbuf) == 0)
        tls_release_read_buffer(rl);

    return OSSL_RECORD_RETURN_SUCCESS;
}

int tls_set_options(OSSL_RECORD_LAYER *rl, const OSSL_PARAM *options)
{
    const OSSL_PARAM *p;

    p = OSSL_PARAM_locate_const(options, OSSL_LIBSSL_RECORD_LAYER_PARAM_OPTIONS);
    if (p != NULL && !OSSL_PARAM_get_uint64(p, &rl->options)) {
        ERR_raise(ERR_LIB_SSL, SSL_R_FAILED_TO_GET_PARAMETER);
        return 0;
    }

    p = OSSL_PARAM_locate_const(options, OSSL_LIBSSL_RECORD_LAYER_PARAM_MODE);
    if (p != NULL && !OSSL_PARAM_get_uint32(p, &rl->mode)) {
        ERR_raise(ERR_LIB_SSL, SSL_R_FAILED_TO_GET_PARAMETER);
        return 0;
    }

    if (rl->direction == OSSL_RECORD_DIRECTION_READ) {
        p = OSSL_PARAM_locate_const(options,
                                    OSSL_LIBSSL_RECORD_LAYER_READ_BUFFER_LEN);
        if (p != NULL && !OSSL_PARAM_get_size_t(p, &rl->rbuf.default_len)) {
            ERR_raise(ERR_LIB_SSL, SSL_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
    } else {
        p = OSSL_PARAM_locate_const(options,
                                    OSSL_LIBSSL_RECORD_LAYER_PARAM_BLOCK_PADDING);
        if (p != NULL && !OSSL_PARAM_get_size_t(p, &rl->block_padding)) {
            ERR_raise(ERR_LIB_SSL, SSL_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        p = OSSL_PARAM_locate_const(options,
                                    OSSL_LIBSSL_RECORD_LAYER_PARAM_HS_PADDING);
        if (p != NULL && !OSSL_PARAM_get_size_t(p, &rl->hs_padding)) {
            ERR_raise(ERR_LIB_SSL, SSL_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
    }

    if (rl->level == OSSL_RECORD_PROTECTION_LEVEL_APPLICATION) {
        /*
         * We ignore any read_ahead setting prior to the application protection
         * level. Otherwise we may read ahead data in a lower protection level
         * that is destined for a higher protection level. To simplify the logic
         * we don't support that at this stage.
         */
        p = OSSL_PARAM_locate_const(options,
                                    OSSL_LIBSSL_RECORD_LAYER_PARAM_READ_AHEAD);
        if (p != NULL && !OSSL_PARAM_get_int(p, &rl->read_ahead)) {
            ERR_raise(ERR_LIB_SSL, SSL_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
    }

    return 1;
}

int
tls_int_new_record_layer(OSSL_LIB_CTX *libctx, const char *propq, int vers,
                         int role, int direction, int level,
                         const EVP_CIPHER *ciph, size_t taglen,
                         const EVP_MD *md, COMP_METHOD *comp, BIO *prev,
                         BIO *transport, BIO *next, const OSSL_PARAM *settings,
                         const OSSL_PARAM *options,
                         const OSSL_DISPATCH *fns, void *cbarg,
                         OSSL_RECORD_LAYER **retrl)
{
    OSSL_RECORD_LAYER *rl = OPENSSL_zalloc(sizeof(*rl));
    const OSSL_PARAM *p;

    *retrl = NULL;

    if (rl == NULL)
        return OSSL_RECORD_RETURN_FATAL;

    /*
     * Default the value for max_frag_len. This may be overridden by the
     * settings
     */
    rl->max_frag_len = SSL3_RT_MAX_PLAIN_LENGTH;

    /* Loop through all the settings since they must all be understood */
    if (settings != NULL) {
        for (p = settings; p->key != NULL; p++) {
            if (strcmp(p->key, OSSL_LIBSSL_RECORD_LAYER_PARAM_USE_ETM) == 0) {
                if (!OSSL_PARAM_get_int(p, &rl->use_etm)) {
                    ERR_raise(ERR_LIB_SSL, SSL_R_FAILED_TO_GET_PARAMETER);
                    goto err;
                }
            } else if (strcmp(p->key,
                              OSSL_LIBSSL_RECORD_LAYER_PARAM_MAX_FRAG_LEN) == 0) {
                if (!OSSL_PARAM_get_uint(p, &rl->max_frag_len)) {
                    ERR_raise(ERR_LIB_SSL, SSL_R_FAILED_TO_GET_PARAMETER);
                    goto err;
                }
            } else if (strcmp(p->key,
                              OSSL_LIBSSL_RECORD_LAYER_PARAM_MAX_EARLY_DATA) == 0) {
                if (!OSSL_PARAM_get_uint32(p, &rl->max_early_data)) {
                    ERR_raise(ERR_LIB_SSL, SSL_R_FAILED_TO_GET_PARAMETER);
                    goto err;
                }
            } else if (strcmp(p->key,
                              OSSL_LIBSSL_RECORD_LAYER_PARAM_STREAM_MAC) == 0) {
                if (!OSSL_PARAM_get_int(p, &rl->stream_mac)) {
                    ERR_raise(ERR_LIB_SSL, SSL_R_FAILED_TO_GET_PARAMETER);
                    goto err;
                }
            } else if (strcmp(p->key,
                              OSSL_LIBSSL_RECORD_LAYER_PARAM_TLSTREE) == 0) {
                if (!OSSL_PARAM_get_int(p, &rl->tlstree)) {
                    ERR_raise(ERR_LIB_SSL, SSL_R_FAILED_TO_GET_PARAMETER);
                    goto err;
                }
            } else {
                ERR_raise(ERR_LIB_SSL, SSL_R_UNKNOWN_MANDATORY_PARAMETER);
                goto err;
            }
        }
    }

    rl->libctx = libctx;
    rl->propq = propq;

    rl->version = vers;
    rl->role = role;
    rl->direction = direction;
    rl->level = level;
    rl->taglen = taglen;
    rl->md = md;

    rl->alert = SSL_AD_NO_ALERT;
    rl->rstate = SSL_ST_READ_HEADER;

    if (level == OSSL_RECORD_PROTECTION_LEVEL_NONE)
        rl->is_first_record = 1;

    if (!tls_set1_bio(rl, transport))
        goto err;

    if (prev != NULL && !BIO_up_ref(prev))
        goto err;
    rl->prev = prev;

    if (next != NULL && !BIO_up_ref(next))
        goto err;
    rl->next = next;

    rl->cbarg = cbarg;
    if (fns != NULL) {
        for (; fns->function_id != 0; fns++) {
            switch (fns->function_id) {
            case OSSL_FUNC_RLAYER_SKIP_EARLY_DATA:
                rl->skip_early_data = OSSL_FUNC_rlayer_skip_early_data(fns);
                break;
            case OSSL_FUNC_RLAYER_MSG_CALLBACK:
                rl->msg_callback = OSSL_FUNC_rlayer_msg_callback(fns);
                break;
            case OSSL_FUNC_RLAYER_SECURITY:
                rl->security = OSSL_FUNC_rlayer_security(fns);
                break;
            case OSSL_FUNC_RLAYER_PADDING:
                rl->padding = OSSL_FUNC_rlayer_padding(fns);
            default:
                /* Just ignore anything we don't understand */
                break;
            }
        }
    }

    if (!tls_set_options(rl, options)) {
        ERR_raise(ERR_LIB_SSL, SSL_R_FAILED_TO_GET_PARAMETER);
        goto err;
    }

    if ((rl->options & SSL_OP_DONT_INSERT_EMPTY_FRAGMENTS) == 0
            && rl->version <= TLS1_VERSION
            && !EVP_CIPHER_is_a(ciph, "NULL")
            && !EVP_CIPHER_is_a(ciph, "RC4")) {
        /*
         * Enable vulnerability countermeasure for CBC ciphers with known-IV
         * problem (http://www.openssl.org/~bodo/tls-cbc.txt)
         */
        rl->need_empty_fragments = 1;
    }

    *retrl = rl;
    return OSSL_RECORD_RETURN_SUCCESS;
 err:
    tls_int_free(rl);
    return OSSL_RECORD_RETURN_FATAL;
}

static int
tls_new_record_layer(OSSL_LIB_CTX *libctx, const char *propq, int vers,
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

    switch (vers) {
    case TLS_ANY_VERSION:
        (*retrl)->funcs = &tls_any_funcs;
        break;
    case TLS1_3_VERSION:
        (*retrl)->funcs = &tls_1_3_funcs;
        break;
    case TLS1_2_VERSION:
    case TLS1_1_VERSION:
    case TLS1_VERSION:
        (*retrl)->funcs = &tls_1_funcs;
        break;
    case SSL3_VERSION:
        (*retrl)->funcs = &ssl_3_0_funcs;
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
        tls_int_free(*retrl);
        *retrl = NULL;
    }
    return ret;
}

static void tls_int_free(OSSL_RECORD_LAYER *rl)
{
    BIO_free(rl->prev);
    BIO_free(rl->bio);
    BIO_free(rl->next);
    ossl_tls_buffer_release(&rl->rbuf);

    tls_release_write_buffer(rl);

    EVP_CIPHER_CTX_free(rl->enc_ctx);
    EVP_MAC_CTX_free(rl->mac_ctx);
    EVP_MD_CTX_free(rl->md_ctx);
#ifndef OPENSSL_NO_COMP
    COMP_CTX_free(rl->compctx);
#endif
    OPENSSL_free(rl->iv);
    OPENSSL_free(rl->nonce);
    if (rl->version == SSL3_VERSION)
        OPENSSL_cleanse(rl->mac_secret, sizeof(rl->mac_secret));

    TLS_RL_RECORD_release(rl->rrec, SSL_MAX_PIPELINES);

    OPENSSL_free(rl);
}

int tls_free(OSSL_RECORD_LAYER *rl)
{
    TLS_BUFFER *rbuf;
    size_t left, written;
    int ret = 1;

    if (rl == NULL)
        return 1;

    rbuf = &rl->rbuf;

    left = TLS_BUFFER_get_left(rbuf);
    if (left > 0) {
        /*
         * This record layer is closing but we still have data left in our
         * buffer. It must be destined for the next epoch - so push it there.
         */
        ret = BIO_write_ex(rl->next, rbuf->buf + rbuf->offset, left, &written);
    }
    tls_int_free(rl);

    return ret;
}

int tls_unprocessed_read_pending(OSSL_RECORD_LAYER *rl)
{
    return TLS_BUFFER_get_left(&rl->rbuf) != 0;
}

int tls_processed_read_pending(OSSL_RECORD_LAYER *rl)
{
    return rl->curr_rec < rl->num_recs;
}

size_t tls_app_data_pending(OSSL_RECORD_LAYER *rl)
{
    size_t i;
    size_t num = 0;

    for (i = rl->curr_rec; i < rl->num_recs; i++) {
        if (rl->rrec[i].type != SSL3_RT_APPLICATION_DATA)
            return num;
        num += rl->rrec[i].length;
    }
    return num;
}

size_t tls_get_max_records_default(OSSL_RECORD_LAYER *rl, uint8_t type,
                                   size_t len,
                                   size_t maxfrag, size_t *preffrag)
{
    /*
     * If we have a pipeline capable cipher, and we have been configured to use
     * it, then return the preferred number of pipelines.
     */
    if (rl->max_pipelines > 0
            && rl->enc_ctx != NULL
            && (EVP_CIPHER_get_flags(EVP_CIPHER_CTX_get0_cipher(rl->enc_ctx))
                & EVP_CIPH_FLAG_PIPELINE) != 0
            && RLAYER_USE_EXPLICIT_IV(rl)) {
        size_t pipes;

        if (len == 0)
            return 1;
        pipes = ((len - 1) / *preffrag) + 1;

        return (pipes < rl->max_pipelines) ? pipes : rl->max_pipelines;
    }

    return 1;
}

size_t tls_get_max_records(OSSL_RECORD_LAYER *rl, uint8_t type, size_t len,
                           size_t maxfrag, size_t *preffrag)
{
    return rl->funcs->get_max_records(rl, type, len, maxfrag, preffrag);
}

int tls_allocate_write_buffers_default(OSSL_RECORD_LAYER *rl,
                                         OSSL_RECORD_TEMPLATE *templates,
                                         size_t numtempl,
                                         size_t *prefix)
{
    if (!tls_setup_write_buffer(rl, numtempl, 0, 0)) {
        /* RLAYERfatal() already called */
        return 0;
    }

    return 1;
}

int tls_initialise_write_packets_default(OSSL_RECORD_LAYER *rl,
                                         OSSL_RECORD_TEMPLATE *templates,
                                         size_t numtempl,
                                         OSSL_RECORD_TEMPLATE *prefixtempl,
                                         WPACKET *pkt,
                                         TLS_BUFFER *bufs,
                                         size_t *wpinited)
{
    WPACKET *thispkt;
    size_t j, align;
    TLS_BUFFER *wb;

    for (j = 0; j < numtempl; j++) {
        thispkt = &pkt[j];
        wb = &bufs[j];

        wb->type = templates[j].type;

#if defined(SSL3_ALIGN_PAYLOAD) && SSL3_ALIGN_PAYLOAD != 0
        align = (size_t)TLS_BUFFER_get_buf(wb);
        align += rl->isdtls ? DTLS1_RT_HEADER_LENGTH : SSL3_RT_HEADER_LENGTH;
        align = SSL3_ALIGN_PAYLOAD - 1
                - ((align - 1) % SSL3_ALIGN_PAYLOAD);
#endif
        TLS_BUFFER_set_offset(wb, align);

        if (!WPACKET_init_static_len(thispkt, TLS_BUFFER_get_buf(wb),
                                     TLS_BUFFER_get_len(wb), 0)) {
            RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            return 0;
        }
        (*wpinited)++;
        if (!WPACKET_allocate_bytes(thispkt, align, NULL)) {
            RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            return 0;
        }
    }

    return 1;
}

int tls_prepare_record_header_default(OSSL_RECORD_LAYER *rl,
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

int tls_prepare_for_encryption_default(OSSL_RECORD_LAYER *rl,
                                       size_t mac_size,
                                       WPACKET *thispkt,
                                       TLS_RL_RECORD *thiswr)
{
    size_t len;
    unsigned char *recordstart;

    /*
     * we should still have the output to thiswr->data and the input from
     * wr->input. Length should be thiswr->length. thiswr->data still points
     * in the wb->buf
     */

    if (!rl->use_etm && mac_size != 0) {
        unsigned char *mac;

        if (!WPACKET_allocate_bytes(thispkt, mac_size, &mac)
                || !rl->funcs->mac(rl, thiswr, mac, 1)) {
            RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            return 0;
        }
    }

    /*
     * Reserve some bytes for any growth that may occur during encryption. If
     * we are adding the MAC independently of the cipher algorithm, then the
     * max encrypted overhead does not need to include an allocation for that
     * MAC
     */
    if (!WPACKET_reserve_bytes(thispkt, SSL3_RT_SEND_MAX_ENCRYPTED_OVERHEAD
                               - mac_size, NULL)
            /*
             * We also need next the amount of bytes written to this
             * sub-packet
             */
            || !WPACKET_get_length(thispkt, &len)) {
        RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    /* Get a pointer to the start of this record excluding header */
    recordstart = WPACKET_get_curr(thispkt) - len;
    TLS_RL_RECORD_set_data(thiswr, recordstart);
    TLS_RL_RECORD_reset_input(thiswr);
    TLS_RL_RECORD_set_length(thiswr, len);

    return 1;
}

int tls_post_encryption_processing_default(OSSL_RECORD_LAYER *rl,
                                           size_t mac_size,
                                           OSSL_RECORD_TEMPLATE *thistempl,
                                           WPACKET *thispkt,
                                           TLS_RL_RECORD *thiswr)
{
    size_t origlen, len;
    size_t headerlen = rl->isdtls ? DTLS1_RT_HEADER_LENGTH
                                  : SSL3_RT_HEADER_LENGTH;

    /* Allocate bytes for the encryption overhead */
    if (!WPACKET_get_length(thispkt, &origlen)
               /* Check we allowed enough room for the encryption growth */
            || !ossl_assert(origlen + SSL3_RT_SEND_MAX_ENCRYPTED_OVERHEAD
                            - mac_size >= thiswr->length)
            /* Encryption should never shrink the data! */
            || origlen > thiswr->length
            || (thiswr->length > origlen
                && !WPACKET_allocate_bytes(thispkt,
                                           thiswr->length - origlen,
                                           NULL))) {
        RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return 0;
    }
    if (rl->use_etm && mac_size != 0) {
        unsigned char *mac;

        if (!WPACKET_allocate_bytes(thispkt, mac_size, &mac)
                || !rl->funcs->mac(rl, thiswr, mac, 1)) {
            RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            return 0;
        }

        TLS_RL_RECORD_add_length(thiswr, mac_size);
    }

    if (!WPACKET_get_length(thispkt, &len)
            || !WPACKET_close(thispkt)) {
        RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    if (rl->msg_callback != NULL) {
        unsigned char *recordstart;

        recordstart = WPACKET_get_curr(thispkt) - len - headerlen;
        rl->msg_callback(1, thiswr->rec_version, SSL3_RT_HEADER, recordstart,
                         headerlen, rl->cbarg);

        if (rl->version == TLS1_3_VERSION && rl->enc_ctx != NULL) {
            unsigned char ctype = thistempl->type;

            rl->msg_callback(1, thiswr->rec_version, SSL3_RT_INNER_CONTENT_TYPE,
                             &ctype, 1, rl->cbarg);
        }
    }

    if (!WPACKET_finish(thispkt)) {
        RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    TLS_RL_RECORD_add_length(thiswr, headerlen);

    return 1;
}

int tls_write_records_default(OSSL_RECORD_LAYER *rl,
                              OSSL_RECORD_TEMPLATE *templates,
                              size_t numtempl)
{
    WPACKET pkt[SSL_MAX_PIPELINES + 1];
    TLS_RL_RECORD wr[SSL_MAX_PIPELINES + 1];
    WPACKET *thispkt;
    TLS_RL_RECORD *thiswr;
    int mac_size = 0, ret = 0;
    size_t wpinited = 0;
    size_t j, prefix = 0;
    OSSL_RECORD_TEMPLATE prefixtempl;
    OSSL_RECORD_TEMPLATE *thistempl;

    if (rl->md_ctx != NULL && EVP_MD_CTX_get0_md(rl->md_ctx) != NULL) {
        mac_size = EVP_MD_CTX_get_size(rl->md_ctx);
        if (mac_size < 0) {
            RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            goto err;
        }
    }

    if (!rl->funcs->allocate_write_buffers(rl, templates, numtempl, &prefix)) {
        /* RLAYERfatal() already called */
        goto err;
    }

    if (!rl->funcs->initialise_write_packets(rl, templates, numtempl,
                                             &prefixtempl, pkt, rl->wbuf,
                                             &wpinited)) {
        /* RLAYERfatal() already called */
        goto err;
    }

    /* Clear our TLS_RL_RECORD structures */
    memset(wr, 0, sizeof(wr));
    for (j = 0; j < numtempl + prefix; j++) {
        unsigned char *compressdata = NULL;
        uint8_t rectype;

        thispkt = &pkt[j];
        thiswr = &wr[j];
        thistempl = (j < prefix) ? &prefixtempl : &templates[j - prefix];

        /*
         * Default to the record type as specified in the template unless the
         * protocol implementation says differently.
         */
        if (rl->funcs->get_record_type != NULL)
            rectype = rl->funcs->get_record_type(rl, thistempl);
        else
            rectype = thistempl->type;

        TLS_RL_RECORD_set_type(thiswr, rectype);
        TLS_RL_RECORD_set_rec_version(thiswr, thistempl->version);

        if (!rl->funcs->prepare_record_header(rl, thispkt, thistempl, rectype,
                                              &compressdata)) {
            /* RLAYERfatal() already called */
            goto err;
        }

        /* lets setup the record stuff. */
        TLS_RL_RECORD_set_data(thiswr, compressdata);
        TLS_RL_RECORD_set_length(thiswr, thistempl->buflen);

        TLS_RL_RECORD_set_input(thiswr, (unsigned char *)thistempl->buf);

        /*
         * we now 'read' from thiswr->input, thiswr->length bytes into
         * thiswr->data
         */

        /* first we compress */
        if (rl->compctx != NULL) {
            if (!tls_do_compress(rl, thiswr)
                    || !WPACKET_allocate_bytes(thispkt, thiswr->length, NULL)) {
                RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, SSL_R_COMPRESSION_FAILURE);
                goto err;
            }
        } else if (compressdata != NULL) {
            if (!WPACKET_memcpy(thispkt, thiswr->input, thiswr->length)) {
                RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
                goto err;
            }
            TLS_RL_RECORD_reset_input(&wr[j]);
        }

        if (rl->funcs->add_record_padding != NULL
                && !rl->funcs->add_record_padding(rl, thistempl, thispkt,
                                                  thiswr)) {
            /* RLAYERfatal() already called */
            goto err;
        }

        if (!rl->funcs->prepare_for_encryption(rl, mac_size, thispkt, thiswr)) {
            /* RLAYERfatal() already called */
            goto err;
        }
    }

    if (prefix) {
        if (rl->funcs->cipher(rl, wr, 1, 1, NULL, mac_size) < 1) {
            if (rl->alert == SSL_AD_NO_ALERT) {
                RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            }
            goto err;
        }
    }

    if (rl->funcs->cipher(rl, wr + prefix, numtempl, 1, NULL, mac_size) < 1) {
        if (rl->alert == SSL_AD_NO_ALERT) {
            RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        }
        goto err;
    }

    for (j = 0; j < numtempl + prefix; j++) {
        thispkt = &pkt[j];
        thiswr = &wr[j];
        thistempl = (j < prefix) ? &prefixtempl : &templates[j - prefix];

        if (!rl->funcs->post_encryption_processing(rl, mac_size, thistempl,
                                                   thispkt, thiswr)) {
            /* RLAYERfatal() already called */
            goto err;
        }

        /* now let's set up wb */
        TLS_BUFFER_set_left(&rl->wbuf[j], TLS_RL_RECORD_get_length(thiswr));
    }

    ret = 1;
 err:
    for (j = 0; j < wpinited; j++)
        WPACKET_cleanup(&pkt[j]);
    return ret;
}

int tls_write_records(OSSL_RECORD_LAYER *rl, OSSL_RECORD_TEMPLATE *templates,
                      size_t numtempl)
{
    /* Check we don't have pending data waiting to write */
    if (!ossl_assert(rl->nextwbuf >= rl->numwpipes
                     || TLS_BUFFER_get_left(&rl->wbuf[rl->nextwbuf]) == 0)) {
        RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return OSSL_RECORD_RETURN_FATAL;
    }

    if (!rl->funcs->write_records(rl, templates, numtempl)) {
        /* RLAYERfatal already called */
        return OSSL_RECORD_RETURN_FATAL;
    }

    rl->nextwbuf = 0;
    /* we now just need to write the buffers */
    return tls_retry_write_records(rl);
}

int tls_retry_write_records(OSSL_RECORD_LAYER *rl)
{
    int i, ret;
    TLS_BUFFER *thiswb;
    size_t tmpwrit = 0;

    if (rl->nextwbuf >= rl->numwpipes)
        return OSSL_RECORD_RETURN_SUCCESS;

    for (;;) {
        thiswb = &rl->wbuf[rl->nextwbuf];

        clear_sys_error();
        if (rl->bio != NULL) {
            if (rl->funcs->prepare_write_bio != NULL) {
                ret = rl->funcs->prepare_write_bio(rl, thiswb->type);
                if (ret != OSSL_RECORD_RETURN_SUCCESS)
                    return ret;
            }
            i = BIO_write(rl->bio, (char *)
                          &(TLS_BUFFER_get_buf(thiswb)
                            [TLS_BUFFER_get_offset(thiswb)]),
                          (unsigned int)TLS_BUFFER_get_left(thiswb));
            if (i >= 0) {
                tmpwrit = i;
                if (i == 0 && BIO_should_retry(rl->bio))
                    ret = OSSL_RECORD_RETURN_RETRY;
                else
                    ret = OSSL_RECORD_RETURN_SUCCESS;
            } else {
                if (BIO_should_retry(rl->bio)) {
                    ret = OSSL_RECORD_RETURN_RETRY;
                } else {
                    ERR_raise_data(ERR_LIB_SYS, get_last_sys_error(),
                                   "tls_retry_write_records failure");
                    ret = OSSL_RECORD_RETURN_FATAL;
                }
            }
        } else {
            RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, SSL_R_BIO_NOT_SET);
            ret = OSSL_RECORD_RETURN_FATAL;
            i = -1;
        }

        /*
         * When an empty fragment is sent on a connection using KTLS,
         * it is sent as a write of zero bytes.  If this zero byte
         * write succeeds, i will be 0 rather than a non-zero value.
         * Treat i == 0 as success rather than an error for zero byte
         * writes to permit this case.
         */
        if (i >= 0 && tmpwrit == TLS_BUFFER_get_left(thiswb)) {
            TLS_BUFFER_set_left(thiswb, 0);
            TLS_BUFFER_add_offset(thiswb, tmpwrit);
            if (++(rl->nextwbuf) < rl->numwpipes)
                continue;

            if (rl->nextwbuf == rl->numwpipes
                    && (rl->mode & SSL_MODE_RELEASE_BUFFERS) != 0)
                tls_release_write_buffer(rl);
            return OSSL_RECORD_RETURN_SUCCESS;
        } else if (i <= 0) {
            if (rl->isdtls) {
                /*
                 * For DTLS, just drop it. That's kind of the whole point in
                 * using a datagram service
                 */
                TLS_BUFFER_set_left(thiswb, 0);
                if (++(rl->nextwbuf) == rl->numwpipes
                        && (rl->mode & SSL_MODE_RELEASE_BUFFERS) != 0)
                    tls_release_write_buffer(rl);

            }
            return ret;
        }
        TLS_BUFFER_add_offset(thiswb, tmpwrit);
        TLS_BUFFER_sub_left(thiswb, tmpwrit);
    }
}

int tls_get_alert_code(OSSL_RECORD_LAYER *rl)
{
    return rl->alert;
}

int tls_set1_bio(OSSL_RECORD_LAYER *rl, BIO *bio)
{
    if (bio != NULL && !BIO_up_ref(bio))
        return 0;
    BIO_free(rl->bio);
    rl->bio = bio;

    return 1;
}

/* Shared by most methods except tlsany_meth */
int tls_default_set_protocol_version(OSSL_RECORD_LAYER *rl, int version)
{
    if (rl->version != version)
        return 0;

    return 1;
}

int tls_set_protocol_version(OSSL_RECORD_LAYER *rl, int version)
{
    return rl->funcs->set_protocol_version(rl, version);
}

void tls_set_plain_alerts(OSSL_RECORD_LAYER *rl, int allow)
{
    rl->allow_plain_alerts = allow;
}

void tls_set_first_handshake(OSSL_RECORD_LAYER *rl, int first)
{
    rl->is_first_handshake = first;
}

void tls_set_max_pipelines(OSSL_RECORD_LAYER *rl, size_t max_pipelines)
{
    rl->max_pipelines = max_pipelines;
    if (max_pipelines > 1)
        rl->read_ahead = 1;
}

void tls_get_state(OSSL_RECORD_LAYER *rl, const char **shortstr,
                   const char **longstr)
{
    const char *shrt, *lng;

    switch (rl->rstate) {
    case SSL_ST_READ_HEADER:
        shrt = "RH";
        lng = "read header";
        break;
    case SSL_ST_READ_BODY:
        shrt = "RB";
        lng = "read body";
        break;
    default:
        shrt = lng = "unknown";
        break;
    }
    if (shortstr != NULL)
        *shortstr = shrt;
    if (longstr != NULL)
        *longstr = lng;
}

const COMP_METHOD *tls_get_compression(OSSL_RECORD_LAYER *rl)
{
#ifndef OPENSSL_NO_COMP
    return (rl->compctx == NULL) ? NULL : COMP_CTX_get_method(rl->compctx);
#else
    return NULL;
#endif
}

void tls_set_max_frag_len(OSSL_RECORD_LAYER *rl, size_t max_frag_len)
{
    rl->max_frag_len = max_frag_len;
    /*
     * We don't need to adjust buffer sizes. Write buffer sizes are
     * automatically checked anyway. We should only be changing the read buffer
     * size during the handshake, so we will create a new buffer when we create
     * the new record layer. We can't change the existing buffer because it may
     * already have data in it.
     */
}

int tls_increment_sequence_ctr(OSSL_RECORD_LAYER *rl)
{
    int i;

    /* Increment the sequence counter */
    for (i = SEQ_NUM_SIZE; i > 0; i--) {
        ++(rl->sequence[i - 1]);
        if (rl->sequence[i - 1] != 0)
            break;
    }
    if (i == 0) {
        /* Sequence has wrapped */
        RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, SSL_R_SEQUENCE_CTR_WRAPPED);
        return 0;
    }
    return 1;
}

int tls_alloc_buffers(OSSL_RECORD_LAYER *rl)
{
    if (rl->direction == OSSL_RECORD_DIRECTION_WRITE) {
        /* If we have a pending write then buffers are already allocated */
        if (rl->nextwbuf < rl->numwpipes)
            return 1;
        /*
         * We assume 1 pipe with default sized buffer. If what we need ends up
         * being a different size to that then it will be reallocated on demand.
         * If we need more than 1 pipe then that will also be allocated on
         * demand
         */
        if (!tls_setup_write_buffer(rl, 1, 0, 0))
            return 0;

        /*
         * Normally when we allocate write buffers we immediately write
         * something into it. In this case we're not doing that so mark the
         * buffer as empty.
         */
        TLS_BUFFER_set_left(&rl->wbuf[0], 0);
        return 1;
    }

    /* Read direction */

    /* If we have pending data to be read then buffers are already allocated */
    if (rl->curr_rec < rl->num_recs || TLS_BUFFER_get_left(&rl->rbuf) != 0)
        return 1;
    return tls_setup_read_buffer(rl);
}

int tls_free_buffers(OSSL_RECORD_LAYER *rl)
{
    if (rl->direction == OSSL_RECORD_DIRECTION_WRITE) {
        if (rl->nextwbuf < rl->numwpipes) {
            /*
             * We may have pending data. If we've just got one empty buffer
             * allocated then it has probably just been alloc'd via
             * tls_alloc_buffers, and it is fine to free it. Otherwise this
             * looks like real pending data and it is an error.
             */
            if (rl->nextwbuf != 0
                    || rl->numwpipes != 1
                    || TLS_BUFFER_get_left(&rl->wbuf[0]) != 0)
                return 0;
        }
        tls_release_write_buffer(rl);
        return 1;
    }

    /* Read direction */

    /* If we have pending data to be read then fail */
    if (rl->curr_rec < rl->num_recs
            || rl->curr_rec != rl->num_released
            || TLS_BUFFER_get_left(&rl->rbuf) != 0
            || rl->rstate == SSL_ST_READ_BODY)
        return 0;

    return tls_release_read_buffer(rl);
}

const OSSL_RECORD_METHOD ossl_tls_record_method = {
    tls_new_record_layer,
    tls_free,
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
    tls_set_plain_alerts,
    tls_set_first_handshake,
    tls_set_max_pipelines,
    NULL,
    tls_get_state,
    tls_set_options,
    tls_get_compression,
    tls_set_max_frag_len,
    NULL,
    tls_increment_sequence_ctr,
    tls_alloc_buffers,
    tls_free_buffers
};
