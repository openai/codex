/*
 * Copyright 2016-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * We need access to the deprecated low level ENGINE APIs for legacy purposes
 * when the deprecated calls are not hidden
 */
#ifndef OPENSSL_NO_DEPRECATED_3_0
# define OPENSSL_SUPPRESS_DEPRECATED
#endif

#include <string.h>

#include <openssl/engine.h>
#include "internal/e_os.h"
#include "internal/nelem.h"
#include "ssltestlib.h"
#include "../testutil.h"

#if (!defined(OPENSSL_NO_KTLS) || !defined(OPENSSL_NO_QUIC)) && !defined(OPENSSL_NO_POSIX_IO) && !defined(OPENSSL_NO_SOCK)
# define OSSL_USE_SOCKETS 1
# include "internal/e_winsock.h"
# include "internal/sockets.h"
# include <openssl/bio.h>
#endif

static int tls_dump_new(BIO *bi);
static int tls_dump_free(BIO *a);
static int tls_dump_read(BIO *b, char *out, int outl);
static int tls_dump_write(BIO *b, const char *in, int inl);
static long tls_dump_ctrl(BIO *b, int cmd, long num, void *ptr);
static int tls_dump_gets(BIO *bp, char *buf, int size);
static int tls_dump_puts(BIO *bp, const char *str);

/* Choose a sufficiently large type likely to be unused for this custom BIO */
#define BIO_TYPE_TLS_DUMP_FILTER   (0x80 | BIO_TYPE_FILTER)
#define BIO_TYPE_MEMPACKET_TEST    0x81
#define BIO_TYPE_ALWAYS_RETRY      0x82
#define BIO_TYPE_MAYBE_RETRY       (0x83 | BIO_TYPE_FILTER)

static BIO_METHOD *method_tls_dump = NULL;
static BIO_METHOD *meth_mem = NULL;
static BIO_METHOD *meth_always_retry = NULL;
static BIO_METHOD *meth_maybe_retry = NULL;
static int retry_err = -1;

/* Note: Not thread safe! */
const BIO_METHOD *bio_f_tls_dump_filter(void)
{
    if (method_tls_dump == NULL) {
        method_tls_dump = BIO_meth_new(BIO_TYPE_TLS_DUMP_FILTER,
                                        "TLS dump filter");
        if (method_tls_dump == NULL
            || !BIO_meth_set_write(method_tls_dump, tls_dump_write)
            || !BIO_meth_set_read(method_tls_dump, tls_dump_read)
            || !BIO_meth_set_puts(method_tls_dump, tls_dump_puts)
            || !BIO_meth_set_gets(method_tls_dump, tls_dump_gets)
            || !BIO_meth_set_ctrl(method_tls_dump, tls_dump_ctrl)
            || !BIO_meth_set_create(method_tls_dump, tls_dump_new)
            || !BIO_meth_set_destroy(method_tls_dump, tls_dump_free))
            return NULL;
    }
    return method_tls_dump;
}

void bio_f_tls_dump_filter_free(void)
{
    BIO_meth_free(method_tls_dump);
}

static int tls_dump_new(BIO *bio)
{
    BIO_set_init(bio, 1);
    return 1;
}

static int tls_dump_free(BIO *bio)
{
    BIO_set_init(bio, 0);

    return 1;
}

static void copy_flags(BIO *bio)
{
    int flags;
    BIO *next = BIO_next(bio);

    flags = BIO_test_flags(next, BIO_FLAGS_SHOULD_RETRY | BIO_FLAGS_RWS);
    BIO_clear_flags(bio, BIO_FLAGS_SHOULD_RETRY | BIO_FLAGS_RWS);
    BIO_set_flags(bio, flags);
}

#define RECORD_CONTENT_TYPE     0
#define RECORD_VERSION_HI       1
#define RECORD_VERSION_LO       2
#define RECORD_EPOCH_HI         3
#define RECORD_EPOCH_LO         4
#define RECORD_SEQUENCE_START   5
#define RECORD_SEQUENCE_END     10
#define RECORD_LEN_HI           11
#define RECORD_LEN_LO           12

#define MSG_TYPE                0
#define MSG_LEN_HI              1
#define MSG_LEN_MID             2
#define MSG_LEN_LO              3
#define MSG_SEQ_HI              4
#define MSG_SEQ_LO              5
#define MSG_FRAG_OFF_HI         6
#define MSG_FRAG_OFF_MID        7
#define MSG_FRAG_OFF_LO         8
#define MSG_FRAG_LEN_HI         9
#define MSG_FRAG_LEN_MID        10
#define MSG_FRAG_LEN_LO         11


static void dump_data(const char *data, int len)
{
    int rem, i, content, reclen, msglen, fragoff, fraglen, epoch;
    unsigned char *rec;

    printf("---- START OF PACKET ----\n");

    rem = len;
    rec = (unsigned char *)data;

    while (rem > 0) {
        if (rem != len)
            printf("*\n");
        printf("*---- START OF RECORD ----\n");
        if (rem < DTLS1_RT_HEADER_LENGTH) {
            printf("*---- RECORD TRUNCATED ----\n");
            break;
        }
        content = rec[RECORD_CONTENT_TYPE];
        printf("** Record Content-type: %d\n", content);
        printf("** Record Version: %02x%02x\n",
               rec[RECORD_VERSION_HI], rec[RECORD_VERSION_LO]);
        epoch = (rec[RECORD_EPOCH_HI] << 8) | rec[RECORD_EPOCH_LO];
        printf("** Record Epoch: %d\n", epoch);
        printf("** Record Sequence: ");
        for (i = RECORD_SEQUENCE_START; i <= RECORD_SEQUENCE_END; i++)
            printf("%02x", rec[i]);
        reclen = (rec[RECORD_LEN_HI] << 8) | rec[RECORD_LEN_LO];
        printf("\n** Record Length: %d\n", reclen);

        /* Now look at message */
        rec += DTLS1_RT_HEADER_LENGTH;
        rem -= DTLS1_RT_HEADER_LENGTH;
        if (content == SSL3_RT_HANDSHAKE) {
            printf("**---- START OF HANDSHAKE MESSAGE FRAGMENT ----\n");
            if (epoch > 0) {
                printf("**---- HANDSHAKE MESSAGE FRAGMENT ENCRYPTED ----\n");
            } else if (rem < DTLS1_HM_HEADER_LENGTH
                    || reclen < DTLS1_HM_HEADER_LENGTH) {
                printf("**---- HANDSHAKE MESSAGE FRAGMENT TRUNCATED ----\n");
            } else {
                printf("*** Message Type: %d\n", rec[MSG_TYPE]);
                msglen = (rec[MSG_LEN_HI] << 16) | (rec[MSG_LEN_MID] << 8)
                         | rec[MSG_LEN_LO];
                printf("*** Message Length: %d\n", msglen);
                printf("*** Message sequence: %d\n",
                       (rec[MSG_SEQ_HI] << 8) | rec[MSG_SEQ_LO]);
                fragoff = (rec[MSG_FRAG_OFF_HI] << 16)
                          | (rec[MSG_FRAG_OFF_MID] << 8)
                          | rec[MSG_FRAG_OFF_LO];
                printf("*** Message Fragment offset: %d\n", fragoff);
                fraglen = (rec[MSG_FRAG_LEN_HI] << 16)
                          | (rec[MSG_FRAG_LEN_MID] << 8)
                          | rec[MSG_FRAG_LEN_LO];
                printf("*** Message Fragment len: %d\n", fraglen);
                if (fragoff + fraglen > msglen)
                    printf("***---- HANDSHAKE MESSAGE FRAGMENT INVALID ----\n");
                else if (reclen < fraglen)
                    printf("**---- HANDSHAKE MESSAGE FRAGMENT TRUNCATED ----\n");
                else
                    printf("**---- END OF HANDSHAKE MESSAGE FRAGMENT ----\n");
            }
        }
        if (rem < reclen) {
            printf("*---- RECORD TRUNCATED ----\n");
            rem = 0;
        } else {
            rec += reclen;
            rem -= reclen;
            printf("*---- END OF RECORD ----\n");
        }
    }
    printf("---- END OF PACKET ----\n\n");
    fflush(stdout);
}

static int tls_dump_read(BIO *bio, char *out, int outl)
{
    int ret;
    BIO *next = BIO_next(bio);

    ret = BIO_read(next, out, outl);
    copy_flags(bio);

    if (ret > 0) {
        dump_data(out, ret);
    }

    return ret;
}

static int tls_dump_write(BIO *bio, const char *in, int inl)
{
    int ret;
    BIO *next = BIO_next(bio);

    ret = BIO_write(next, in, inl);
    copy_flags(bio);

    return ret;
}

static long tls_dump_ctrl(BIO *bio, int cmd, long num, void *ptr)
{
    long ret;
    BIO *next = BIO_next(bio);

    if (next == NULL)
        return 0;

    switch (cmd) {
    case BIO_CTRL_DUP:
        ret = 0L;
        break;
    default:
        ret = BIO_ctrl(next, cmd, num, ptr);
        break;
    }
    return ret;
}

static int tls_dump_gets(BIO *bio, char *buf, int size)
{
    /* We don't support this - not needed anyway */
    return -1;
}

static int tls_dump_puts(BIO *bio, const char *str)
{
    return tls_dump_write(bio, str, strlen(str));
}


struct mempacket_st {
    unsigned char *data;
    int len;
    unsigned int num;
    unsigned int type;
};

static void mempacket_free(MEMPACKET *pkt)
{
    if (pkt->data != NULL)
        OPENSSL_free(pkt->data);
    OPENSSL_free(pkt);
}

typedef struct mempacket_test_ctx_st {
    STACK_OF(MEMPACKET) *pkts;
    uint16_t epoch;
    unsigned int currrec;
    unsigned int currpkt;
    unsigned int lastpkt;
    unsigned int injected;
    unsigned int noinject;
    unsigned int dropepoch;
    int droprec;
    int duprec;
} MEMPACKET_TEST_CTX;

static int mempacket_test_new(BIO *bi);
static int mempacket_test_free(BIO *a);
static int mempacket_test_read(BIO *b, char *out, int outl);
static int mempacket_test_write(BIO *b, const char *in, int inl);
static long mempacket_test_ctrl(BIO *b, int cmd, long num, void *ptr);
static int mempacket_test_gets(BIO *bp, char *buf, int size);
static int mempacket_test_puts(BIO *bp, const char *str);

const BIO_METHOD *bio_s_mempacket_test(void)
{
    if (meth_mem == NULL) {
        if (!TEST_ptr(meth_mem = BIO_meth_new(BIO_TYPE_MEMPACKET_TEST,
                                              "Mem Packet Test"))
            || !TEST_true(BIO_meth_set_write(meth_mem, mempacket_test_write))
            || !TEST_true(BIO_meth_set_read(meth_mem, mempacket_test_read))
            || !TEST_true(BIO_meth_set_puts(meth_mem, mempacket_test_puts))
            || !TEST_true(BIO_meth_set_gets(meth_mem, mempacket_test_gets))
            || !TEST_true(BIO_meth_set_ctrl(meth_mem, mempacket_test_ctrl))
            || !TEST_true(BIO_meth_set_create(meth_mem, mempacket_test_new))
            || !TEST_true(BIO_meth_set_destroy(meth_mem, mempacket_test_free)))
            return NULL;
    }
    return meth_mem;
}

void bio_s_mempacket_test_free(void)
{
    BIO_meth_free(meth_mem);
}

static int mempacket_test_new(BIO *bio)
{
    MEMPACKET_TEST_CTX *ctx;

    if (!TEST_ptr(ctx = OPENSSL_zalloc(sizeof(*ctx))))
        return 0;
    if (!TEST_ptr(ctx->pkts = sk_MEMPACKET_new_null())) {
        OPENSSL_free(ctx);
        return 0;
    }
    ctx->dropepoch = 0;
    ctx->droprec = -1;
    BIO_set_init(bio, 1);
    BIO_set_data(bio, ctx);
    return 1;
}

static int mempacket_test_free(BIO *bio)
{
    MEMPACKET_TEST_CTX *ctx = BIO_get_data(bio);

    sk_MEMPACKET_pop_free(ctx->pkts, mempacket_free);
    OPENSSL_free(ctx);
    BIO_set_data(bio, NULL);
    BIO_set_init(bio, 0);
    return 1;
}

/* Record Header values */
#define EPOCH_HI        3
#define EPOCH_LO        4
#define RECORD_SEQUENCE 10
#define RECORD_LEN_HI   11
#define RECORD_LEN_LO   12

#define STANDARD_PACKET                 0

static int mempacket_test_read(BIO *bio, char *out, int outl)
{
    MEMPACKET_TEST_CTX *ctx = BIO_get_data(bio);
    MEMPACKET *thispkt;
    unsigned char *rec;
    int rem;
    unsigned int seq, offset, len, epoch;

    BIO_clear_retry_flags(bio);
    if ((thispkt = sk_MEMPACKET_value(ctx->pkts, 0)) == NULL
        || thispkt->num != ctx->currpkt) {
        /* Probably run out of data */
        BIO_set_retry_read(bio);
        return -1;
    }
    (void)sk_MEMPACKET_shift(ctx->pkts);
    ctx->currpkt++;

    if (outl > thispkt->len)
        outl = thispkt->len;

    if (thispkt->type != INJECT_PACKET_IGNORE_REC_SEQ
            && (ctx->injected || ctx->droprec >= 0)) {
        /*
         * Overwrite the record sequence number. We strictly number them in
         * the order received. Since we are actually a reliable transport
         * we know that there won't be any re-ordering. We overwrite to deal
         * with any packets that have been injected
         */
        for (rem = thispkt->len, rec = thispkt->data; rem > 0; rem -= len) {
            if (rem < DTLS1_RT_HEADER_LENGTH)
                return -1;
            epoch = (rec[EPOCH_HI] << 8) | rec[EPOCH_LO];
            if (epoch != ctx->epoch) {
                ctx->epoch = epoch;
                ctx->currrec = 0;
            }
            seq = ctx->currrec;
            offset = 0;
            do {
                rec[RECORD_SEQUENCE - offset] = seq & 0xFF;
                seq >>= 8;
                offset++;
            } while (seq > 0);

            len = ((rec[RECORD_LEN_HI] << 8) | rec[RECORD_LEN_LO])
                  + DTLS1_RT_HEADER_LENGTH;
            if (rem < (int)len)
                return -1;
            if (ctx->droprec == (int)ctx->currrec && ctx->dropepoch == epoch) {
                if (rem > (int)len)
                    memmove(rec, rec + len, rem - len);
                outl -= len;
                ctx->droprec = -1;
                if (outl == 0)
                    BIO_set_retry_read(bio);
            } else {
                rec += len;
            }

            ctx->currrec++;
        }
    }

    memcpy(out, thispkt->data, outl);
    mempacket_free(thispkt);
    return outl;
}

/*
 * Look for records from different epochs in the last datagram and swap them
 * around
 */
int mempacket_swap_epoch(BIO *bio)
{
    MEMPACKET_TEST_CTX *ctx = BIO_get_data(bio);
    MEMPACKET *thispkt;
    int rem, len, prevlen = 0, pktnum;
    unsigned char *rec, *prevrec = NULL, *tmp;
    unsigned int epoch;
    int numpkts = sk_MEMPACKET_num(ctx->pkts);

    if (numpkts <= 0)
        return 0;

    /*
     * If there are multiple packets we only look in the last one. This should
     * always be the one where any epoch change occurs.
     */
    thispkt = sk_MEMPACKET_value(ctx->pkts, numpkts - 1);
    if (thispkt == NULL)
        return 0;

    for (rem = thispkt->len, rec = thispkt->data; rem > 0; rem -= len, rec += len) {
        if (rem < DTLS1_RT_HEADER_LENGTH)
            return 0;
        epoch = (rec[EPOCH_HI] << 8) | rec[EPOCH_LO];
        len = ((rec[RECORD_LEN_HI] << 8) | rec[RECORD_LEN_LO])
                + DTLS1_RT_HEADER_LENGTH;
        if (rem < len)
            return 0;

        /* Assumes the epoch change does not happen on the first record */
        if (epoch != ctx->epoch) {
            if (prevrec == NULL)
                return 0;

            /*
             * We found 2 records with different epochs. Take a copy of the
             * earlier record
             */
            tmp = OPENSSL_malloc(prevlen);
            if (tmp == NULL)
                return 0;

            memcpy(tmp, prevrec, prevlen);
            /*
             * Move everything from this record onwards, including any trailing
             * records, and overwrite the earlier record
             */
            memmove(prevrec, rec, rem);
            thispkt->len -= prevlen;
            pktnum = thispkt->num;

            /*
             * Create a new packet for the earlier record that we took out and
             * add it to the end of the packet list.
             */
            thispkt = OPENSSL_malloc(sizeof(*thispkt));
            if (thispkt == NULL) {
                OPENSSL_free(tmp);
                return 0;
            }
            thispkt->type = INJECT_PACKET;
            thispkt->data = tmp;
            thispkt->len = prevlen;
            thispkt->num = pktnum + 1;
            if (sk_MEMPACKET_insert(ctx->pkts, thispkt, numpkts) <= 0) {
                OPENSSL_free(tmp);
                OPENSSL_free(thispkt);
                return 0;
            }

            return 1;
        }
        prevrec = rec;
        prevlen = len;
    }

    return 0;
}

/* Move packet from position s to position d in the list (d < s) */
int mempacket_move_packet(BIO *bio, int d, int s)
{
    MEMPACKET_TEST_CTX *ctx = BIO_get_data(bio);
    MEMPACKET *thispkt;
    int numpkts = sk_MEMPACKET_num(ctx->pkts);
    int i;

    if (d >= s)
        return 0;

    /* We need at least s + 1 packets to be able to swap them */
    if (numpkts <= s)
        return 0;

    /* Get the packet at position s */
    thispkt = sk_MEMPACKET_value(ctx->pkts, s);
    if (thispkt == NULL)
        return 0;

    /* Remove and re-add it */
    if (sk_MEMPACKET_delete(ctx->pkts, s) != thispkt)
        return 0;

    thispkt->num -= (s - d);
    if (sk_MEMPACKET_insert(ctx->pkts, thispkt, d) <= 0)
        return 0;

    /* Increment the packet numbers for moved packets */
    for (i = d + 1; i <= s; i++) {
        thispkt = sk_MEMPACKET_value(ctx->pkts, i);
        thispkt->num++;
    }
    return 1;
}

int mempacket_dup_last_packet(BIO *bio)
{
    MEMPACKET_TEST_CTX *ctx = BIO_get_data(bio);
    MEMPACKET *thispkt, *duppkt;
    int numpkts = sk_MEMPACKET_num(ctx->pkts);

    /* We can only duplicate a packet if there is at least 1 pending */
    if (numpkts <= 0)
        return 0;

    /* Get the last packet */
    thispkt = sk_MEMPACKET_value(ctx->pkts, numpkts - 1);
    if (thispkt == NULL)
        return 0;

    duppkt = OPENSSL_malloc(sizeof(*duppkt));
    if (duppkt == NULL)
        return 0;

    *duppkt = *thispkt;
    duppkt->data = OPENSSL_memdup(thispkt->data, thispkt->len);
    if (duppkt->data == NULL) {
        mempacket_free(duppkt);
        return 0;
    }
    duppkt->num++;
    if (sk_MEMPACKET_insert(ctx->pkts, duppkt, numpkts) <= 0) {
        mempacket_free(duppkt);
        return 0;
    }

    return 1;
}

int mempacket_test_inject(BIO *bio, const char *in, int inl, int pktnum,
                          int type)
{
    MEMPACKET_TEST_CTX *ctx = BIO_get_data(bio);
    MEMPACKET *thispkt = NULL, *looppkt, *nextpkt, *allpkts[3];
    int i, duprec;
    const unsigned char *inu = (const unsigned char *)in;
    size_t len = ((inu[RECORD_LEN_HI] << 8) | inu[RECORD_LEN_LO])
                 + DTLS1_RT_HEADER_LENGTH;

    if (ctx == NULL)
        return -1;

    if ((size_t)inl < len)
        return -1;

    if ((size_t)inl == len)
        duprec = 0;
    else
        duprec = ctx->duprec > 0;

    /* We don't support arbitrary injection when duplicating records */
    if (duprec && pktnum != -1)
        return -1;

    /* We only allow injection before we've started writing any data */
    if (pktnum >= 0) {
        if (ctx->noinject)
            return -1;
        ctx->injected  = 1;
    } else {
        ctx->noinject = 1;
    }

    for (i = 0; i < (duprec ? 3 : 1); i++) {
        if (!TEST_ptr(allpkts[i] = OPENSSL_malloc(sizeof(*thispkt))))
            goto err;
        thispkt = allpkts[i];

        if (!TEST_ptr(thispkt->data = OPENSSL_malloc(inl)))
            goto err;
        /*
         * If we are duplicating the packet, we duplicate it three times. The
         * first two times we drop the first record if there are more than one.
         * In this way we know that libssl will not be able to make progress
         * until it receives the last packet, and hence will be forced to
         * buffer these records.
         */
        if (duprec && i != 2) {
            memcpy(thispkt->data, in + len, inl - len);
            thispkt->len = inl - len;
        } else {
            memcpy(thispkt->data, in, inl);
            thispkt->len = inl;
        }
        thispkt->num = (pktnum >= 0) ? (unsigned int)pktnum : ctx->lastpkt + i;
        thispkt->type = type;
    }

    for (i = 0; i < sk_MEMPACKET_num(ctx->pkts); i++) {
        if (!TEST_ptr(looppkt = sk_MEMPACKET_value(ctx->pkts, i)))
            goto err;
        /* Check if we found the right place to insert this packet */
        if (looppkt->num > thispkt->num) {
            if (sk_MEMPACKET_insert(ctx->pkts, thispkt, i) == 0)
                goto err;
            /* If we're doing up front injection then we're done */
            if (pktnum >= 0)
                return inl;
            /*
             * We need to do some accounting on lastpkt. We increment it first,
             * but it might now equal the value of injected packets, so we need
             * to skip over those
             */
            ctx->lastpkt++;
            do {
                i++;
                nextpkt = sk_MEMPACKET_value(ctx->pkts, i);
                if (nextpkt != NULL && nextpkt->num == ctx->lastpkt)
                    ctx->lastpkt++;
                else
                    return inl;
            } while(1);
        } else if (looppkt->num == thispkt->num) {
            if (!ctx->noinject) {
                /* We injected two packets with the same packet number! */
                goto err;
            }
            ctx->lastpkt++;
            thispkt->num++;
        }
    }
    /*
     * We didn't find any packets with a packet number equal to or greater than
     * this one, so we just add it onto the end
     */
    for (i = 0; i < (duprec ? 3 : 1); i++) {
        thispkt = allpkts[i];
        if (!sk_MEMPACKET_push(ctx->pkts, thispkt))
            goto err;

        if (pktnum < 0)
            ctx->lastpkt++;
    }

    return inl;

 err:
    for (i = 0; i < (ctx->duprec > 0 ? 3 : 1); i++)
        mempacket_free(allpkts[i]);
    return -1;
}

static int mempacket_test_write(BIO *bio, const char *in, int inl)
{
    return mempacket_test_inject(bio, in, inl, -1, STANDARD_PACKET);
}

static long mempacket_test_ctrl(BIO *bio, int cmd, long num, void *ptr)
{
    long ret = 1;
    MEMPACKET_TEST_CTX *ctx = BIO_get_data(bio);
    MEMPACKET *thispkt;

    switch (cmd) {
    case BIO_CTRL_EOF:
        ret = (long)(sk_MEMPACKET_num(ctx->pkts) == 0);
        break;
    case BIO_CTRL_GET_CLOSE:
        ret = BIO_get_shutdown(bio);
        break;
    case BIO_CTRL_SET_CLOSE:
        BIO_set_shutdown(bio, (int)num);
        break;
    case BIO_CTRL_WPENDING:
        ret = 0L;
        break;
    case BIO_CTRL_PENDING:
        thispkt = sk_MEMPACKET_value(ctx->pkts, 0);
        if (thispkt == NULL)
            ret = 0;
        else
            ret = thispkt->len;
        break;
    case BIO_CTRL_FLUSH:
        ret = 1;
        break;
    case MEMPACKET_CTRL_SET_DROP_EPOCH:
        ctx->dropepoch = (unsigned int)num;
        break;
    case MEMPACKET_CTRL_SET_DROP_REC:
        ctx->droprec = (int)num;
        break;
    case MEMPACKET_CTRL_GET_DROP_REC:
        ret = ctx->droprec;
        break;
    case MEMPACKET_CTRL_SET_DUPLICATE_REC:
        ctx->duprec = (int)num;
        break;
    case BIO_CTRL_RESET:
    case BIO_CTRL_DUP:
    case BIO_CTRL_PUSH:
    case BIO_CTRL_POP:
    default:
        ret = 0;
        break;
    }
    return ret;
}

static int mempacket_test_gets(BIO *bio, char *buf, int size)
{
    /* We don't support this - not needed anyway */
    return -1;
}

static int mempacket_test_puts(BIO *bio, const char *str)
{
    return mempacket_test_write(bio, str, strlen(str));
}

static int always_retry_new(BIO *bi);
static int always_retry_free(BIO *a);
static int always_retry_read(BIO *b, char *out, int outl);
static int always_retry_write(BIO *b, const char *in, int inl);
static long always_retry_ctrl(BIO *b, int cmd, long num, void *ptr);
static int always_retry_gets(BIO *bp, char *buf, int size);
static int always_retry_puts(BIO *bp, const char *str);

const BIO_METHOD *bio_s_always_retry(void)
{
    if (meth_always_retry == NULL) {
        if (!TEST_ptr(meth_always_retry = BIO_meth_new(BIO_TYPE_ALWAYS_RETRY,
                                                       "Always Retry"))
            || !TEST_true(BIO_meth_set_write(meth_always_retry,
                                             always_retry_write))
            || !TEST_true(BIO_meth_set_read(meth_always_retry,
                                            always_retry_read))
            || !TEST_true(BIO_meth_set_puts(meth_always_retry,
                                            always_retry_puts))
            || !TEST_true(BIO_meth_set_gets(meth_always_retry,
                                            always_retry_gets))
            || !TEST_true(BIO_meth_set_ctrl(meth_always_retry,
                                            always_retry_ctrl))
            || !TEST_true(BIO_meth_set_create(meth_always_retry,
                                              always_retry_new))
            || !TEST_true(BIO_meth_set_destroy(meth_always_retry,
                                               always_retry_free)))
            return NULL;
    }
    return meth_always_retry;
}

void bio_s_always_retry_free(void)
{
    BIO_meth_free(meth_always_retry);
}

static int always_retry_new(BIO *bio)
{
    BIO_set_init(bio, 1);
    return 1;
}

static int always_retry_free(BIO *bio)
{
    BIO_set_data(bio, NULL);
    BIO_set_init(bio, 0);
    return 1;
}

void set_always_retry_err_val(int err)
{
    retry_err = err;
}

static int always_retry_read(BIO *bio, char *out, int outl)
{
    BIO_set_retry_read(bio);
    return retry_err;
}

static int always_retry_write(BIO *bio, const char *in, int inl)
{
    BIO_set_retry_write(bio);
    return retry_err;
}

static long always_retry_ctrl(BIO *bio, int cmd, long num, void *ptr)
{
    long ret = 1;

    switch (cmd) {
    case BIO_CTRL_FLUSH:
        BIO_set_retry_write(bio);
        /* fall through */
    case BIO_CTRL_EOF:
    case BIO_CTRL_RESET:
    case BIO_CTRL_DUP:
    case BIO_CTRL_PUSH:
    case BIO_CTRL_POP:
    default:
        ret = 0;
        break;
    }
    return ret;
}

static int always_retry_gets(BIO *bio, char *buf, int size)
{
    BIO_set_retry_read(bio);
    return retry_err;
}

static int always_retry_puts(BIO *bio, const char *str)
{
    BIO_set_retry_write(bio);
    return retry_err;
}

struct maybe_retry_data_st {
    unsigned int retrycnt;
};

static int maybe_retry_new(BIO *bi);
static int maybe_retry_free(BIO *a);
static int maybe_retry_write(BIO *b, const char *in, int inl);
static long maybe_retry_ctrl(BIO *b, int cmd, long num, void *ptr);

const BIO_METHOD *bio_s_maybe_retry(void)
{
    if (meth_maybe_retry == NULL) {
        if (!TEST_ptr(meth_maybe_retry = BIO_meth_new(BIO_TYPE_MAYBE_RETRY,
                                                      "Maybe Retry"))
            || !TEST_true(BIO_meth_set_write(meth_maybe_retry,
                                             maybe_retry_write))
            || !TEST_true(BIO_meth_set_ctrl(meth_maybe_retry,
                                            maybe_retry_ctrl))
            || !TEST_true(BIO_meth_set_create(meth_maybe_retry,
                                              maybe_retry_new))
            || !TEST_true(BIO_meth_set_destroy(meth_maybe_retry,
                                               maybe_retry_free)))
            return NULL;
    }
    return meth_maybe_retry;
}

void bio_s_maybe_retry_free(void)
{
    BIO_meth_free(meth_maybe_retry);
}

static int maybe_retry_new(BIO *bio)
{
    struct maybe_retry_data_st *data = OPENSSL_zalloc(sizeof(*data));

    if (data == NULL)
        return 0;

    BIO_set_data(bio, data);
    BIO_set_init(bio, 1);
    return 1;
}

static int maybe_retry_free(BIO *bio)
{
    struct maybe_retry_data_st *data = BIO_get_data(bio);

    OPENSSL_free(data);
    BIO_set_data(bio, NULL);
    BIO_set_init(bio, 0);
    return 1;
}

static int maybe_retry_write(BIO *bio, const char *in, int inl)
{
    struct maybe_retry_data_st *data = BIO_get_data(bio);

    if (data == NULL)
        return -1;

    if (data->retrycnt == 0) {
        BIO_set_retry_write(bio);
        return -1;
    }
    data->retrycnt--;

    return BIO_write(BIO_next(bio), in, inl);
}

static long maybe_retry_ctrl(BIO *bio, int cmd, long num, void *ptr)
{
    struct maybe_retry_data_st *data = BIO_get_data(bio);

    if (data == NULL)
        return 0;

    switch (cmd) {
    case MAYBE_RETRY_CTRL_SET_RETRY_AFTER_CNT:
        data->retrycnt = num;
        return 1;

    case BIO_CTRL_FLUSH:
        if (data->retrycnt == 0) {
            BIO_set_retry_write(bio);
            return -1;
        }
        data->retrycnt--;
        /* fall through */
    default:
        return BIO_ctrl(BIO_next(bio), cmd, num, ptr);
    }
}

int create_ssl_ctx_pair(OSSL_LIB_CTX *libctx, const SSL_METHOD *sm,
                        const SSL_METHOD *cm, int min_proto_version,
                        int max_proto_version, SSL_CTX **sctx, SSL_CTX **cctx,
                        char *certfile, char *privkeyfile)
{
    SSL_CTX *serverctx = NULL;
    SSL_CTX *clientctx = NULL;

    if (sctx != NULL) {
        if (*sctx != NULL)
            serverctx = *sctx;
        else if (!TEST_ptr(serverctx = SSL_CTX_new_ex(libctx, NULL, sm))
            || !TEST_true(SSL_CTX_set_options(serverctx,
                                              SSL_OP_ALLOW_CLIENT_RENEGOTIATION)))
            goto err;
    }

    if (cctx != NULL) {
        if (*cctx != NULL)
            clientctx = *cctx;
        else if (!TEST_ptr(clientctx = SSL_CTX_new_ex(libctx, NULL, cm)))
            goto err;
    }

#if !defined(OPENSSL_NO_TLS1_3) \
    && defined(OPENSSL_NO_EC) \
    && defined(OPENSSL_NO_DH)
    /*
     * There are no usable built-in TLSv1.3 groups if ec and dh are both
     * disabled
     */
    if (max_proto_version == 0
            && (sm == TLS_server_method() || cm == TLS_client_method()))
        max_proto_version = TLS1_2_VERSION;
#endif

    if (serverctx != NULL
            && ((min_proto_version > 0
                 && !TEST_true(SSL_CTX_set_min_proto_version(serverctx,
                                                            min_proto_version)))
                || (max_proto_version > 0
                    && !TEST_true(SSL_CTX_set_max_proto_version(serverctx,
                                                                max_proto_version)))))
        goto err;
    if (clientctx != NULL
        && ((min_proto_version > 0
             && !TEST_true(SSL_CTX_set_min_proto_version(clientctx,
                                                         min_proto_version)))
            || (max_proto_version > 0
                && !TEST_true(SSL_CTX_set_max_proto_version(clientctx,
                                                            max_proto_version)))))
        goto err;

    if (serverctx != NULL && certfile != NULL && privkeyfile != NULL) {
        if (!TEST_int_eq(SSL_CTX_use_certificate_file(serverctx, certfile,
                                                      SSL_FILETYPE_PEM), 1)
                || !TEST_int_eq(SSL_CTX_use_PrivateKey_file(serverctx,
                                                            privkeyfile,
                                                            SSL_FILETYPE_PEM), 1)
                || !TEST_int_eq(SSL_CTX_check_private_key(serverctx), 1))
            goto err;
    }

    if (sctx != NULL)
        *sctx = serverctx;
    if (cctx != NULL)
        *cctx = clientctx;
    return 1;

 err:
    if (sctx != NULL && *sctx == NULL)
        SSL_CTX_free(serverctx);
    if (cctx != NULL && *cctx == NULL)
        SSL_CTX_free(clientctx);
    return 0;
}

#define MAXLOOPS    1000000

#if defined(OSSL_USE_SOCKETS)

int wait_until_sock_readable(int sock)
{
    fd_set readfds;
    struct timeval timeout;
    int width;

    width = sock + 1;
    FD_ZERO(&readfds);
    openssl_fdset(sock, &readfds);
    timeout.tv_sec = 10; /* give up after 10 seconds */
    timeout.tv_usec = 0;

    select(width, &readfds, NULL, NULL, &timeout);

    return FD_ISSET(sock, &readfds);
}

int create_test_sockets(int *cfdp, int *sfdp, int socktype, BIO_ADDR *saddr)
{
    struct sockaddr_in sin;
    const char *host = "127.0.0.1";
    int cfd_connected = 0, ret = 0;
    socklen_t slen = sizeof(sin);
    int afd = -1, cfd = -1, sfd = -1;

    memset ((char *) &sin, 0, sizeof(sin));
    sin.sin_family = AF_INET;
    sin.sin_addr.s_addr = inet_addr(host);

    afd = BIO_socket(AF_INET, socktype,
                     socktype == SOCK_STREAM ? IPPROTO_TCP : IPPROTO_UDP, 0);
    if (afd == INVALID_SOCKET)
        return 0;

    if (bind(afd, (struct sockaddr*)&sin, sizeof(sin)) < 0)
        goto out;

    if (getsockname(afd, (struct sockaddr*)&sin, &slen) < 0)
        goto out;

    if (saddr != NULL
            && !BIO_ADDR_rawmake(saddr, sin.sin_family, &sin.sin_addr,
                                 sizeof(sin.sin_addr), sin.sin_port))
        goto out;

    if (socktype == SOCK_STREAM && listen(afd, 1) < 0)
        goto out;

    cfd = BIO_socket(AF_INET, socktype,
                     socktype == SOCK_STREAM ? IPPROTO_TCP : IPPROTO_UDP, 0);
    if (cfd == INVALID_SOCKET)
        goto out;

    if (!BIO_socket_nbio(afd, 1))
        goto out;

    /*
     * If a DGRAM socket then we don't call "accept" or "connect" - so act like
     * we already called them.
     */
    if (socktype == SOCK_DGRAM) {
        cfd_connected = 1;
        sfd = afd;
        afd = -1;
    }

    while (sfd == -1 || !cfd_connected) {
        sfd = accept(afd, NULL, 0);
        if (sfd == -1 && errno != EAGAIN)
            goto out;

        if (!cfd_connected && connect(cfd, (struct sockaddr*)&sin, sizeof(sin)) < 0)
            goto out;
        else
            cfd_connected = 1;
    }

    if (!BIO_socket_nbio(cfd, 1) || !BIO_socket_nbio(sfd, 1))
        goto out;
    ret = 1;
    *cfdp = cfd;
    *sfdp = sfd;
    goto success;

out:
    if (cfd != -1)
        close(cfd);
    if (sfd != -1)
        close(sfd);
success:
    if (afd != -1)
        close(afd);
    return ret;
}

int create_ssl_objects2(SSL_CTX *serverctx, SSL_CTX *clientctx, SSL **sssl,
                          SSL **cssl, int sfd, int cfd)
{
    SSL *serverssl = NULL, *clientssl = NULL;
    BIO *s_to_c_bio = NULL, *c_to_s_bio = NULL;
    BIO_POLL_DESCRIPTOR rdesc = {0}, wdesc = {0};

    if (*sssl != NULL)
        serverssl = *sssl;
    else if (!TEST_ptr(serverssl = SSL_new(serverctx)))
        goto error;
    if (*cssl != NULL)
        clientssl = *cssl;
    else if (!TEST_ptr(clientssl = SSL_new(clientctx)))
        goto error;

    if (!TEST_ptr(s_to_c_bio = BIO_new_socket(sfd, BIO_NOCLOSE))
            || !TEST_ptr(c_to_s_bio = BIO_new_socket(cfd, BIO_NOCLOSE)))
        goto error;

    if (!TEST_false(SSL_get_rpoll_descriptor(clientssl, &rdesc)
        || !TEST_false(SSL_get_wpoll_descriptor(clientssl, &wdesc))))
        goto error;

    SSL_set_bio(clientssl, c_to_s_bio, c_to_s_bio);
    SSL_set_bio(serverssl, s_to_c_bio, s_to_c_bio);

    if (!TEST_true(SSL_get_rpoll_descriptor(clientssl, &rdesc))
        || !TEST_true(SSL_get_wpoll_descriptor(clientssl, &wdesc))
        || !TEST_int_eq(rdesc.type, BIO_POLL_DESCRIPTOR_TYPE_SOCK_FD)
        || !TEST_int_eq(wdesc.type, BIO_POLL_DESCRIPTOR_TYPE_SOCK_FD)
        || !TEST_int_eq(rdesc.value.fd, cfd)
        || !TEST_int_eq(wdesc.value.fd, cfd))
        goto error;

    if (!TEST_true(SSL_get_rpoll_descriptor(serverssl, &rdesc))
        || !TEST_true(SSL_get_wpoll_descriptor(serverssl, &wdesc))
        || !TEST_int_eq(rdesc.type, BIO_POLL_DESCRIPTOR_TYPE_SOCK_FD)
        || !TEST_int_eq(wdesc.type, BIO_POLL_DESCRIPTOR_TYPE_SOCK_FD)
        || !TEST_int_eq(rdesc.value.fd, sfd)
        || !TEST_int_eq(wdesc.value.fd, sfd))
        goto error;

    *sssl = serverssl;
    *cssl = clientssl;
    return 1;

 error:
    SSL_free(serverssl);
    SSL_free(clientssl);
    BIO_free(s_to_c_bio);
    BIO_free(c_to_s_bio);
    return 0;
}

#else

int wait_until_sock_readable(int sock)
{
    return 0;
}

#endif /* defined(OSSL_USE_SOCKETS) */

/*
 * NOTE: Transfers control of the BIOs - this function will free them on error
 */
int create_ssl_objects(SSL_CTX *serverctx, SSL_CTX *clientctx, SSL **sssl,
                          SSL **cssl, BIO *s_to_c_fbio, BIO *c_to_s_fbio)
{
    SSL *serverssl = NULL, *clientssl = NULL;
    BIO *s_to_c_bio = NULL, *c_to_s_bio = NULL;

    if (*sssl != NULL)
        serverssl = *sssl;
    else if (!TEST_ptr(serverssl = SSL_new(serverctx)))
        goto error;
    if (*cssl != NULL)
        clientssl = *cssl;
    else if (!TEST_ptr(clientssl = SSL_new(clientctx)))
        goto error;

    if (SSL_is_dtls(clientssl)) {
        if (!TEST_ptr(s_to_c_bio = BIO_new(bio_s_mempacket_test()))
                || !TEST_ptr(c_to_s_bio = BIO_new(bio_s_mempacket_test())))
            goto error;
    } else {
        if (!TEST_ptr(s_to_c_bio = BIO_new(BIO_s_mem()))
                || !TEST_ptr(c_to_s_bio = BIO_new(BIO_s_mem())))
            goto error;
    }

    if (s_to_c_fbio != NULL
            && !TEST_ptr(s_to_c_bio = BIO_push(s_to_c_fbio, s_to_c_bio)))
        goto error;
    if (c_to_s_fbio != NULL
            && !TEST_ptr(c_to_s_bio = BIO_push(c_to_s_fbio, c_to_s_bio)))
        goto error;

    /* Set Non-blocking IO behaviour */
    BIO_set_mem_eof_return(s_to_c_bio, -1);
    BIO_set_mem_eof_return(c_to_s_bio, -1);

    /* Up ref these as we are passing them to two SSL objects */
    if (!BIO_up_ref(s_to_c_bio))
        goto error;
    if (!BIO_up_ref(c_to_s_bio)) {
        BIO_free(s_to_c_bio);
        goto error;
    }

    SSL_set_bio(serverssl, c_to_s_bio, s_to_c_bio);
    SSL_set_bio(clientssl, s_to_c_bio, c_to_s_bio);
    *sssl = serverssl;
    *cssl = clientssl;
    return 1;

 error:
    SSL_free(serverssl);
    SSL_free(clientssl);
    BIO_free(s_to_c_bio);
    BIO_free(c_to_s_bio);
    BIO_free(s_to_c_fbio);
    BIO_free(c_to_s_fbio);

    return 0;
}

/*
 * Create an SSL connection, but does not read any post-handshake
 * NewSessionTicket messages.
 * If |read| is set and we're using DTLS then we will attempt to SSL_read on
 * the connection once we've completed one half of it, to ensure any retransmits
 * get triggered.
 * We stop the connection attempt (and return a failure value) if either peer
 * has SSL_get_error() return the value in the |want| parameter. The connection
 * attempt could be restarted by a subsequent call to this function.
 */
int create_bare_ssl_connection_ex(SSL *serverssl, SSL *clientssl, int want,
                                  int read, int listen, int *cm_count, int *sm_count)
{
    int retc = -1, rets = -1, err, abortctr = 0, ret = 0;
    int clienterr = 0, servererr = 0;
    int isdtls = SSL_is_dtls(serverssl);
    int icm_count = 0, ism_count = 0;
#ifndef OPENSSL_NO_SOCK
    BIO_ADDR *peer = NULL;

    if (listen) {
        if (!isdtls) {
            TEST_error("DTLSv1_listen requested for non-DTLS object\n");
            return 0;
        }
        peer = BIO_ADDR_new();
        if (!TEST_ptr(peer))
            return 0;
    }
#else
    if (listen) {
        TEST_error("DTLSv1_listen requested in a no-sock build\n");
        return 0;
    }
#endif

    do {
        err = SSL_ERROR_WANT_WRITE;
        while (!clienterr && retc <= 0 && err == SSL_ERROR_WANT_WRITE) {
            retc = SSL_connect(clientssl);
            if (retc <= 0)
                err = SSL_get_error(clientssl, retc);
            icm_count++;
        }

        if (!clienterr && retc <= 0 && err != SSL_ERROR_WANT_READ) {
            TEST_info("SSL_connect() failed %d, %d", retc, err);
            if (want != SSL_ERROR_SSL)
                TEST_openssl_errors();
            clienterr = 1;
        }
        if (want != SSL_ERROR_NONE && err == want)
            goto err;

        err = SSL_ERROR_WANT_WRITE;
        while (!servererr && rets <= 0 && err == SSL_ERROR_WANT_WRITE) {
#ifndef OPENSSL_NO_SOCK
            if (listen) {
                rets = DTLSv1_listen(serverssl, peer);
                if (rets < 0) {
                    err = SSL_ERROR_SSL;
                } else if (rets == 0) {
                    err = SSL_ERROR_WANT_READ;
                } else {
                    /* Success - stop listening and call SSL_accept from now on */
                    listen = 0;
                    rets = 0;
                }
                ism_count++;
            } else
#endif
            {
                rets = SSL_accept(serverssl);
                if (rets <= 0)
                    err = SSL_get_error(serverssl, rets);
                ism_count++;
            }
        }

        if (!servererr && rets <= 0
                && err != SSL_ERROR_WANT_READ
                && err != SSL_ERROR_WANT_X509_LOOKUP) {
            TEST_info("SSL_accept() failed %d, %d", rets, err);
            if (want != SSL_ERROR_SSL)
                TEST_openssl_errors();
            servererr = 1;
        }
        if (want != SSL_ERROR_NONE && err == want)
            goto err;
        if (clienterr && servererr)
            goto err;
        if (isdtls && read) {
            unsigned char buf[20];

            /* Trigger any retransmits that may be appropriate */
            if (rets > 0 && retc <= 0) {
                if (SSL_read(serverssl, buf, sizeof(buf)) > 0) {
                    /* We don't expect this to succeed! */
                    TEST_info("Unexpected SSL_read() success!");
                    goto err;
                }
                ism_count++;
            }
            if (retc > 0 && rets <= 0) {
                if (SSL_read(clientssl, buf, sizeof(buf)) > 0) {
                    /* We don't expect this to succeed! */
                    TEST_info("Unexpected SSL_read() success!");
                    goto err;
                }
                icm_count++;
            }
        }
        if (++abortctr == MAXLOOPS) {
            TEST_info("No progress made");
            goto err;
        }
        if (isdtls && abortctr <= 50 && (abortctr % 10) == 0) {
            /*
             * It looks like we're just spinning. Pause for a short period to
             * give the DTLS timer a chance to do something. We only do this for
             * the first few times to prevent hangs.
             */
            OSSL_sleep(50);
        }
    } while (retc <=0 || rets <= 0);

    ret = 1;
 err:
    if (cm_count != NULL)
        *cm_count = icm_count;
    if (sm_count != NULL)
        *sm_count = ism_count;
#ifndef OPENSSL_NO_SOCK
    BIO_ADDR_free(peer);
#endif
    return ret;
}

int create_bare_ssl_connection(SSL *serverssl, SSL *clientssl, int want,
                               int read, int listen)
{
    return create_bare_ssl_connection_ex(serverssl, clientssl, want, read,
                                         listen, NULL, NULL);
}

/*
 * Create an SSL connection including any post handshake NewSessionTicket
 * messages.
 */
int create_ssl_connection_ex(SSL *serverssl, SSL *clientssl, int want,
                             int *cm_count, int *sm_count)
{
    int i;
    unsigned char buf;
    size_t readbytes;

    if (!create_bare_ssl_connection_ex(serverssl, clientssl, want, 1, 0,
                                       cm_count, sm_count))
        return 0;

    /*
     * We attempt to read some data on the client side which we expect to fail.
     * This will ensure we have received the NewSessionTicket in TLSv1.3 where
     * appropriate. We do this twice because there are 2 NewSessionTickets.
     */
    for (i = 0; i < 2; i++) {
        if (SSL_read_ex(clientssl, &buf, sizeof(buf), &readbytes) > 0) {
            if (!TEST_ulong_eq(readbytes, 0))
                return 0;
        } else if (!TEST_int_eq(SSL_get_error(clientssl, 0),
                                SSL_ERROR_WANT_READ)) {
            return 0;
        }
        if (cm_count != NULL)
            (*cm_count)++;
    }

    return 1;
}

int create_ssl_connection(SSL *serverssl, SSL *clientssl, int want)
{
   return create_ssl_connection_ex(serverssl, clientssl, want, NULL, NULL);
}

void shutdown_ssl_connection(SSL *serverssl, SSL *clientssl)
{
    SSL_shutdown(clientssl);
    SSL_shutdown(serverssl);
    SSL_free(serverssl);
    SSL_free(clientssl);
}

SSL_SESSION *create_a_psk(SSL *ssl, size_t mdsize)
{
    const SSL_CIPHER *cipher = NULL;
    const unsigned char key[SHA384_DIGEST_LENGTH] = {
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a,
        0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15,
        0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20,
        0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b,
        0x2c, 0x2d, 0x2e, 0x2f
    };
    SSL_SESSION *sess = NULL;

    if (mdsize == SHA384_DIGEST_LENGTH) {
        cipher = SSL_CIPHER_find(ssl, TLS13_AES_256_GCM_SHA384_BYTES);
    } else if (mdsize == SHA256_DIGEST_LENGTH) {
        /*
         * Any ciphersuite using SHA256 will do - it will be compatible with
         * the actual ciphersuite selected as long as it too is based on SHA256
         */
        cipher = SSL_CIPHER_find(ssl, TLS13_AES_128_GCM_SHA256_BYTES);
    } else {
        /* Should not happen */
        return NULL;
    }
    sess = SSL_SESSION_new();
    if (!TEST_ptr(sess)
            || !TEST_ptr(cipher)
            || !TEST_true(SSL_SESSION_set1_master_key(sess, key, mdsize))
            || !TEST_true(SSL_SESSION_set_cipher(sess, cipher))
            || !TEST_true(
                    SSL_SESSION_set_protocol_version(sess,
                                                     TLS1_3_VERSION))) {
        SSL_SESSION_free(sess);
        return NULL;
    }
    return sess;
}

#define NUM_EXTRA_CERTS 40

int ssl_ctx_add_large_cert_chain(OSSL_LIB_CTX *libctx, SSL_CTX *sctx,
                                 const char *cert_file)
{
    BIO *certbio = NULL;
    X509 *chaincert = NULL;
    int certlen;
    int ret = 0;
    int i;

    if (!TEST_ptr(certbio = BIO_new_file(cert_file, "r")))
        goto end;

    if (!TEST_ptr(chaincert = X509_new_ex(libctx, NULL)))
        goto end;

    if (PEM_read_bio_X509(certbio, &chaincert, NULL, NULL) == NULL)
        goto end;
    BIO_free(certbio);
    certbio = NULL;

    /*
     * We assume the supplied certificate is big enough so that if we add
     * NUM_EXTRA_CERTS it will make the overall message large enough. The
     * default buffer size is requested to be 16k, but due to the way BUF_MEM
     * works, it ends up allocating a little over 21k (16 * 4/3). So, in this
     * test we need to have a message larger than that.
     */
    certlen = i2d_X509(chaincert, NULL);
    OPENSSL_assert(certlen * NUM_EXTRA_CERTS >
                   (SSL3_RT_MAX_PLAIN_LENGTH * 4) / 3);
    for (i = 0; i < NUM_EXTRA_CERTS; i++) {
        if (!X509_up_ref(chaincert))
            goto end;
        if (!SSL_CTX_add_extra_chain_cert(sctx, chaincert)) {
            X509_free(chaincert);
            goto end;
        }
    }

    ret = 1;
 end:
    BIO_free(certbio);
    X509_free(chaincert);
    return ret;
}

ENGINE *load_dasync(void)
{
#if !defined(OPENSSL_NO_TLS1_2) && !defined(OPENSSL_NO_DYNAMIC_ENGINE)
    ENGINE *e;

    if (!TEST_ptr(e = ENGINE_by_id("dasync")))
        return NULL;

    if (!TEST_true(ENGINE_init(e))) {
        ENGINE_free(e);
        return NULL;
    }

    if (!TEST_true(ENGINE_register_ciphers(e))) {
        ENGINE_free(e);
        return NULL;
    }

    return e;
#else
    return NULL;
#endif
}
