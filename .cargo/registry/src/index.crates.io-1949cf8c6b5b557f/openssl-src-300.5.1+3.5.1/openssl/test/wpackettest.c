/*
 * Copyright 2016-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/buffer.h>
#include <openssl/rand.h>
#include "internal/packet_quic.h"
#include "testutil.h"

static const unsigned char simple1[] = { 0xff };
static const unsigned char simple2[] = { 0x01, 0xff };
static const unsigned char simple3[] = { 0x00, 0x00, 0x00, 0x01, 0xff };
static const unsigned char nestedsub[] = { 0x03, 0xff, 0x01, 0xff };
static const unsigned char seqsub[] = { 0x01, 0xff, 0x01, 0xff };
static const unsigned char empty[] = { 0x00 };
static const unsigned char alloc[] = { 0x02, 0xfe, 0xff };
static const unsigned char submem[] = { 0x03, 0x02, 0xfe, 0xff };
static const unsigned char fixed[] = { 0xff, 0xff, 0xff };
static const unsigned char simpleder[] = {
    0xfc, 0x04, 0x00, 0x01, 0x02, 0x03, 0xff, 0xfe, 0xfd
};

#ifndef OPENSSL_NO_QUIC

/* QUIC sub-packet with 4-byte length prefix, containing a 1-byte vlint */
static const unsigned char quic1[] = { 0x80, 0x00, 0x00, 0x01, 0x09 };
/* QUIC sub-packet with 1-byte length prefix, containing a 1-byte vlint */
static const unsigned char quic2[] = { 0x01, 0x09 };
/* QUIC sub-packet with 2-byte length prefix, containing a 2-byte vlint */
static const unsigned char quic3[] = { 0x40, 0x02, 0x40, 0x41 };
/* QUIC sub-packet with 8-byte length prefix, containing a 4-byte vlint */
static const unsigned char quic4[] = {
    0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04,
    0x80, 0x01, 0x3c, 0x6a
};
/* QUIC sub-packet with 8-byte length prefix, containing a 8-byte vlint */
static const unsigned char quic5[] = {
    0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08,
    0xef, 0x77, 0x21, 0x3f, 0x3f, 0x50, 0x5b, 0xa5
};
/* QUIC sub-packet, length known up-front */
static const unsigned char quic6[] = { 0x03, 0x55, 0x66, 0x77 };
/* Nested and sequential sub-packets with length prefixes */
static const unsigned char quic7[] = {
    0x07, 0x80, 0x00, 0x00, 0x08, 0x65, 0x14, 0x40, 0x01, 0x05,
    0x40, 0x01, 0x11, 0x40, 0x01, 0x12, 0x40, 0x01, 0x13
};

#endif

static BUF_MEM *buf;

static int cleanup(WPACKET *pkt)
{
    WPACKET_cleanup(pkt);
    return 0;
}

static int test_WPACKET_init(void)
{
    WPACKET pkt;
    int i;
    size_t written;
    unsigned char sbuf[3];

    if (!TEST_true(WPACKET_init(&pkt, buf))
            || !TEST_true(WPACKET_put_bytes_u8(&pkt, 0xff))
                /* Closing a top level WPACKET should fail */
            || !TEST_false(WPACKET_close(&pkt))
                /* Finishing a top level WPACKET should succeed */
            || !TEST_true(WPACKET_finish(&pkt))
                /*
                 * Can't call close or finish on a WPACKET that's already
                 * finished.
                 */
            || !TEST_false(WPACKET_close(&pkt))
            || !TEST_false(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_mem_eq(buf->data, written, simple1, sizeof(simple1)))
        return cleanup(&pkt);

    /* Now try with a one byte length prefix */
    if (!TEST_true(WPACKET_init_len(&pkt, buf, 1))
            || !TEST_true(WPACKET_put_bytes_u8(&pkt, 0xff))
            || !TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_mem_eq(buf->data, written, simple2, sizeof(simple2)))
        return cleanup(&pkt);

    /* And a longer length prefix */
    if (!TEST_true(WPACKET_init_len(&pkt, buf, 4))
            || !TEST_true(WPACKET_put_bytes_u8(&pkt, 0xff))
            || !TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_mem_eq(buf->data, written, simple3, sizeof(simple3)))
        return cleanup(&pkt);

    if (!TEST_true(WPACKET_init_len(&pkt, buf, 1)))
        return cleanup(&pkt);
    for (i = 1; i < 257; i++) {
        /*
         * Putting more bytes in than fit for the size of the length prefix
         * should fail
         */
        if (!TEST_int_eq(WPACKET_put_bytes_u8(&pkt, 0xff), i < 256))
            return cleanup(&pkt);
    }
    if (!TEST_true(WPACKET_finish(&pkt)))
        return cleanup(&pkt);

    /* Test initialising from a fixed size buffer */
    if (!TEST_true(WPACKET_init_static_len(&pkt, sbuf, sizeof(sbuf), 0))
                /* Adding 3 bytes should succeed */
            || !TEST_true(WPACKET_put_bytes_u24(&pkt, 0xffffff))
                /* Adding 1 more byte should fail */
            || !TEST_false(WPACKET_put_bytes_u8(&pkt, 0xff))
                /* Finishing the top level WPACKET should succeed */
            || !TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_mem_eq(sbuf, written, fixed, sizeof(sbuf))
                /* Initialise with 1 len byte */
            || !TEST_true(WPACKET_init_static_len(&pkt, sbuf, sizeof(sbuf), 1))
                /* Adding 2 bytes should succeed */
            || !TEST_true(WPACKET_put_bytes_u16(&pkt, 0xfeff))
                /* Adding 1 more byte should fail */
            || !TEST_false(WPACKET_put_bytes_u8(&pkt, 0xff))
            || !TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_mem_eq(sbuf, written, alloc, sizeof(alloc)))
        return cleanup(&pkt);

    return 1;
}

static int test_WPACKET_set_max_size(void)
{
    WPACKET pkt;
    size_t written;

    if (!TEST_true(WPACKET_init(&pkt, buf))
                /*
                 * No previous lenbytes set so we should be ok to set the max
                 * possible max size
                 */
            || !TEST_true(WPACKET_set_max_size(&pkt, SIZE_MAX))
                /* We should be able to set it smaller too */
            || !TEST_true(WPACKET_set_max_size(&pkt, SIZE_MAX -1))
                /* And setting it bigger again should be ok */
            || !TEST_true(WPACKET_set_max_size(&pkt, SIZE_MAX))
            || !TEST_true(WPACKET_finish(&pkt)))
        return cleanup(&pkt);

    if (!TEST_true(WPACKET_init_len(&pkt, buf, 1))
                /*
                 * Should fail because we already consumed 1 byte with the
                 * length
                 */
            || !TEST_false(WPACKET_set_max_size(&pkt, 0))
                /*
                 * Max size can't be bigger than biggest that will fit in
                 * lenbytes
                 */
            || !TEST_false(WPACKET_set_max_size(&pkt, 0x0101))
                /* It can be the same as the maximum possible size */
            || !TEST_true(WPACKET_set_max_size(&pkt, 0x0100))
                /* Or it can be less */
            || !TEST_true(WPACKET_set_max_size(&pkt, 0x01))
                /* Should fail because packet is already filled */
            || !TEST_false(WPACKET_put_bytes_u8(&pkt, 0xff))
                /* You can't put in more bytes than max size */
            || !TEST_true(WPACKET_set_max_size(&pkt, 0x02))
            || !TEST_true(WPACKET_put_bytes_u8(&pkt, 0xff))
            || !TEST_false(WPACKET_put_bytes_u8(&pkt, 0xff))
            || !TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_mem_eq(buf->data, written, simple2, sizeof(simple2)))
        return cleanup(&pkt);

    return 1;
}

static int test_WPACKET_start_sub_packet(void)
{
    WPACKET pkt;
    size_t written;
    size_t len;

    if (!TEST_true(WPACKET_init(&pkt, buf))
            || !TEST_true(WPACKET_start_sub_packet(&pkt))
            || !TEST_true(WPACKET_put_bytes_u8(&pkt, 0xff))
                /* Can't finish because we have a sub packet */
            || !TEST_false(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_close(&pkt))
                /* Sub packet is closed so can't close again */
            || !TEST_false(WPACKET_close(&pkt))
                /* Now a top level so finish should succeed */
            || !TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_mem_eq(buf->data, written, simple1, sizeof(simple1)))
        return cleanup(&pkt);

   /* Single sub-packet with length prefix */
    if (!TEST_true(WPACKET_init(&pkt, buf))
            || !TEST_true(WPACKET_start_sub_packet_u8(&pkt))
            || !TEST_true(WPACKET_put_bytes_u8(&pkt, 0xff))
            || !TEST_true(WPACKET_close(&pkt))
            || !TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_mem_eq(buf->data, written, simple2, sizeof(simple2)))
        return cleanup(&pkt);

    /* Nested sub-packets with length prefixes */
    if (!TEST_true(WPACKET_init(&pkt, buf))
            || !TEST_true(WPACKET_start_sub_packet_u8(&pkt))
            || !TEST_true(WPACKET_put_bytes_u8(&pkt, 0xff))
            || !TEST_true(WPACKET_start_sub_packet_u8(&pkt))
            || !TEST_true(WPACKET_put_bytes_u8(&pkt, 0xff))
            || !TEST_true(WPACKET_get_length(&pkt, &len))
            || !TEST_size_t_eq(len, 1)
            || !TEST_true(WPACKET_close(&pkt))
            || !TEST_true(WPACKET_get_length(&pkt, &len))
            || !TEST_size_t_eq(len, 3)
            || !TEST_true(WPACKET_close(&pkt))
            || !TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_mem_eq(buf->data, written, nestedsub, sizeof(nestedsub)))
        return cleanup(&pkt);

    /* Sequential sub-packets with length prefixes */
    if (!TEST_true(WPACKET_init(&pkt, buf))
            || !TEST_true(WPACKET_start_sub_packet_u8(&pkt))
            || !TEST_true(WPACKET_put_bytes_u8(&pkt, 0xff))
            || !TEST_true(WPACKET_close(&pkt))
            || !TEST_true(WPACKET_start_sub_packet_u8(&pkt))
            || !TEST_true(WPACKET_put_bytes_u8(&pkt, 0xff))
            || !TEST_true(WPACKET_close(&pkt))
            || !TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_mem_eq(buf->data, written, seqsub, sizeof(seqsub)))
        return cleanup(&pkt);

    /* Nested sub-packets with lengths filled before finish */
    if (!TEST_true(WPACKET_init(&pkt, buf))
            || !TEST_true(WPACKET_start_sub_packet_u8(&pkt))
            || !TEST_true(WPACKET_put_bytes_u8(&pkt, 0xff))
            || !TEST_true(WPACKET_start_sub_packet_u8(&pkt))
            || !TEST_true(WPACKET_put_bytes_u8(&pkt, 0xff))
            || !TEST_true(WPACKET_get_length(&pkt, &len))
            || !TEST_size_t_eq(len, 1)
            || !TEST_true(WPACKET_close(&pkt))
            || !TEST_true(WPACKET_get_length(&pkt, &len))
            || !TEST_size_t_eq(len, 3)
            || !TEST_true(WPACKET_close(&pkt))
            || !TEST_true(WPACKET_fill_lengths(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_mem_eq(buf->data, written, nestedsub, sizeof(nestedsub))
            || !TEST_true(WPACKET_finish(&pkt)))
        return cleanup(&pkt);

    return 1;
}


static int test_WPACKET_set_flags(void)
{
    WPACKET pkt;
    size_t written;

    /* Set packet to be non-zero length */
    if (!TEST_true(WPACKET_init(&pkt, buf))
            || !TEST_true(WPACKET_set_flags(&pkt, WPACKET_FLAGS_NON_ZERO_LENGTH))
                /* Should fail because of zero length */
            || !TEST_false(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_put_bytes_u8(&pkt, 0xff))
            || !TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_mem_eq(buf->data, written, simple1, sizeof(simple1)))
        return cleanup(&pkt);

    /* Repeat above test in a sub-packet */
    if (!TEST_true(WPACKET_init(&pkt, buf))
            || !TEST_true(WPACKET_start_sub_packet(&pkt))
            || !TEST_true(WPACKET_set_flags(&pkt, WPACKET_FLAGS_NON_ZERO_LENGTH))
                /* Should fail because of zero length */
            || !TEST_false(WPACKET_close(&pkt))
            || !TEST_true(WPACKET_put_bytes_u8(&pkt, 0xff))
            || !TEST_true(WPACKET_close(&pkt))
            || !TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_mem_eq(buf->data, written, simple1, sizeof(simple1)))
        return cleanup(&pkt);

    /* Set packet to abandon non-zero length */
    if (!TEST_true(WPACKET_init_len(&pkt, buf, 1))
            || !TEST_true(WPACKET_set_flags(&pkt, WPACKET_FLAGS_ABANDON_ON_ZERO_LENGTH))
            || !TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_size_t_eq(written, 0))
        return cleanup(&pkt);

    /* Repeat above test but only abandon a sub-packet */
    if (!TEST_true(WPACKET_init_len(&pkt, buf, 1))
            || !TEST_true(WPACKET_start_sub_packet_u8(&pkt))
            || !TEST_true(WPACKET_set_flags(&pkt, WPACKET_FLAGS_ABANDON_ON_ZERO_LENGTH))
            || !TEST_true(WPACKET_close(&pkt))
            || !TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_mem_eq(buf->data, written, empty, sizeof(empty)))
        return cleanup(&pkt);

    /* And repeat with a non empty sub-packet */
    if (!TEST_true(WPACKET_init(&pkt, buf))
            || !TEST_true(WPACKET_start_sub_packet_u8(&pkt))
            || !TEST_true(WPACKET_set_flags(&pkt, WPACKET_FLAGS_ABANDON_ON_ZERO_LENGTH))
            || !TEST_true(WPACKET_put_bytes_u8(&pkt, 0xff))
            || !TEST_true(WPACKET_close(&pkt))
            || !TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_mem_eq(buf->data, written, simple2, sizeof(simple2)))
        return cleanup(&pkt);
    return 1;
}

static int test_WPACKET_allocate_bytes(void)
{
    WPACKET pkt;
    size_t written;
    unsigned char *bytes;

    if (!TEST_true(WPACKET_init_len(&pkt, buf, 1))
            || !TEST_true(WPACKET_allocate_bytes(&pkt, 2, &bytes)))
        return cleanup(&pkt);
    bytes[0] = 0xfe;
    bytes[1] = 0xff;
    if (!TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_mem_eq(buf->data, written, alloc, sizeof(alloc)))
        return cleanup(&pkt);

    /* Repeat with WPACKET_sub_allocate_bytes */
    if (!TEST_true(WPACKET_init_len(&pkt, buf, 1))
            || !TEST_true(WPACKET_sub_allocate_bytes_u8(&pkt, 2, &bytes)))
        return cleanup(&pkt);
    bytes[0] = 0xfe;
    bytes[1] = 0xff;
    if (!TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_mem_eq(buf->data, written, submem, sizeof(submem)))
        return cleanup(&pkt);

    return 1;
}

static int test_WPACKET_memcpy(void)
{
    WPACKET pkt;
    size_t written;
    const unsigned char bytes[] = { 0xfe, 0xff };

    if (!TEST_true(WPACKET_init_len(&pkt, buf, 1))
            || !TEST_true(WPACKET_memcpy(&pkt, bytes, sizeof(bytes)))
            || !TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_mem_eq(buf->data, written, alloc, sizeof(alloc)))
        return cleanup(&pkt);

    /* Repeat with WPACKET_sub_memcpy() */
    if (!TEST_true(WPACKET_init_len(&pkt, buf, 1))
            || !TEST_true(WPACKET_sub_memcpy_u8(&pkt, bytes, sizeof(bytes)))
            || !TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_mem_eq(buf->data, written, submem, sizeof(submem)))
        return cleanup(&pkt);

    return 1;
}

static int test_WPACKET_init_der(void)
{
    WPACKET pkt;
    unsigned char sbuf[1024];
    unsigned char testdata[] = { 0x00, 0x01, 0x02, 0x03 };
    unsigned char testdata2[259]  = { 0x82, 0x01, 0x00 };
    size_t written[2];
    size_t size1, size2;
    int flags = WPACKET_FLAGS_ABANDON_ON_ZERO_LENGTH;
    int i;

    /* Test initialising for writing DER */
    if (!TEST_true(WPACKET_init_der(&pkt, sbuf, sizeof(sbuf)))
            || !TEST_true(WPACKET_put_bytes_u24(&pkt, 0xfffefd))
               /* Test writing data in a length prefixed sub-packet */
            || !TEST_true(WPACKET_start_sub_packet(&pkt))
            || !TEST_true(WPACKET_memcpy(&pkt, testdata, sizeof(testdata)))
            || !TEST_true(WPACKET_close(&pkt))
            || !TEST_true(WPACKET_put_bytes_u8(&pkt, 0xfc))
            /* this sub-packet is empty, and should render zero bytes */
            || (!TEST_true(WPACKET_start_sub_packet(&pkt))
                || !TEST_true(WPACKET_set_flags(&pkt, flags))
                || !TEST_true(WPACKET_get_total_written(&pkt, &size1))
                || !TEST_true(WPACKET_close(&pkt))
                || !TEST_true(WPACKET_get_total_written(&pkt, &size2))
                || !TEST_size_t_eq(size1, size2))
            || !TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written[0]))
            || !TEST_mem_eq(WPACKET_get_curr(&pkt), written[0], simpleder,
                            sizeof(simpleder)))
        return cleanup(&pkt);

    /* Generate random packet data for test */
    if (!TEST_int_gt(RAND_bytes(&testdata2[3], sizeof(testdata2) - 3), 0))
        return 0;

    /*
     * Test with a sub-packet that has 2 length bytes. We do 2 passes - first
     * with a NULL buffer, just to calculate lengths, and a second pass with a
     * real buffer to actually generate a packet
     */
    for (i = 0; i < 2; i++) {
        if (i == 0) {
            if (!TEST_true(WPACKET_init_null_der(&pkt)))
                return 0;
        } else {
            if (!TEST_true(WPACKET_init_der(&pkt, sbuf, sizeof(sbuf))))
                return 0;
        }
        if (!TEST_true(WPACKET_start_sub_packet(&pkt))
            || !TEST_true(WPACKET_memcpy(&pkt, &testdata2[3],
                                         sizeof(testdata2) - 3))
            || !TEST_true(WPACKET_close(&pkt))
            || !TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written[i])))
        return cleanup(&pkt);
    }

    /*
     * Check that the size calculated in the first pass equals the size of the
     * packet actually generated in the second pass. Also check the generated
     * packet looks as we expect it to.
     */
    if (!TEST_size_t_eq(written[0], written[1])
            || !TEST_mem_eq(WPACKET_get_curr(&pkt), written[1], testdata2,
                            sizeof(testdata2)))
        return 0;

    return 1;
}

#ifndef OPENSSL_NO_QUIC

static int test_WPACKET_quic(void)
{
    WPACKET pkt;
    size_t written, len;
    unsigned char *bytes;

    /* QUIC sub-packet with 4-byte length prefix, containing a 1-byte vlint */
    if (!TEST_true(WPACKET_init(&pkt, buf))
            || !TEST_true(WPACKET_start_quic_sub_packet(&pkt))
            || !TEST_true(WPACKET_quic_write_vlint(&pkt, 0x09))
                /* Can't finish because we have a sub packet */
            || !TEST_false(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_close(&pkt))
                /* Sub packet is closed so can't close again */
            || !TEST_false(WPACKET_close(&pkt))
                /* Now a top level so finish should succeed */
            || !TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_mem_eq(buf->data, written, quic1, sizeof(quic1)))
        return cleanup(&pkt);

    /* QUIC sub-packet with 1-byte length prefix, containing a 1-byte vlint */
    if (!TEST_true(WPACKET_init(&pkt, buf))
            || !TEST_true(WPACKET_start_quic_sub_packet_bound(&pkt, OSSL_QUIC_VLINT_1B_MAX))
            || !TEST_true(WPACKET_quic_write_vlint(&pkt, 0x09))
            || !TEST_false(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_close(&pkt))
            || !TEST_false(WPACKET_close(&pkt))
            || !TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_mem_eq(buf->data, written, quic2, sizeof(quic2)))
        return cleanup(&pkt);

    /* QUIC sub-packet with 2-byte length prefix, containing a 2-byte vlint */
    if (!TEST_true(WPACKET_init(&pkt, buf))
            || !TEST_true(WPACKET_start_quic_sub_packet_bound(&pkt, OSSL_QUIC_VLINT_2B_MIN))
            || !TEST_true(WPACKET_quic_write_vlint(&pkt, 0x41))
            || !TEST_false(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_close(&pkt))
            || !TEST_false(WPACKET_close(&pkt))
            || !TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_mem_eq(buf->data, written, quic3, sizeof(quic3)))
        return cleanup(&pkt);

    /* QUIC sub-packet with 8-byte length prefix, containing a 4-byte vlint */
    if (!TEST_true(WPACKET_init(&pkt, buf))
            || !TEST_true(WPACKET_start_quic_sub_packet_bound(&pkt, OSSL_QUIC_VLINT_8B_MIN))
            || !TEST_true(WPACKET_quic_write_vlint(&pkt, 0x13c6a))
            || !TEST_false(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_close(&pkt))
            || !TEST_false(WPACKET_close(&pkt))
            || !TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_mem_eq(buf->data, written, quic4, sizeof(quic4)))
        return cleanup(&pkt);

    /* QUIC sub-packet with 8-byte length prefix, containing a 8-byte vlint */
    if (!TEST_true(WPACKET_init(&pkt, buf))
            || !TEST_true(WPACKET_start_quic_sub_packet_bound(&pkt, OSSL_QUIC_VLINT_8B_MIN))
            || !TEST_true(WPACKET_quic_write_vlint(&pkt, 0x2f77213f3f505ba5ULL))
            || !TEST_false(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_close(&pkt))
            || !TEST_false(WPACKET_close(&pkt))
            || !TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_mem_eq(buf->data, written, quic5, sizeof(quic5)))
        return cleanup(&pkt);

    /* QUIC sub-packet, length known up-front */
    if (!TEST_true(WPACKET_init(&pkt, buf))
            || !TEST_true(WPACKET_quic_sub_allocate_bytes(&pkt, 3, &bytes)))
        return cleanup(&pkt);

    bytes[0] = 0x55;
    bytes[1] = 0x66;
    bytes[2] = 0x77;

    if (!TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_mem_eq(buf->data, written, quic6, sizeof(quic6)))
        return cleanup(&pkt);

    /* Nested and sequential sub-packets with length prefixes */
    if (!TEST_true(WPACKET_init(&pkt, buf))
            || !TEST_true(WPACKET_quic_write_vlint(&pkt, 0x07))
            || !TEST_true(WPACKET_get_length(&pkt, &len))
            || !TEST_size_t_eq(len, 1)
            || !TEST_true(WPACKET_start_quic_sub_packet_bound(&pkt, OSSL_QUIC_VLINT_4B_MIN))
            || !TEST_true(WPACKET_quic_write_vlint(&pkt, 0x2514))
            || !TEST_true(WPACKET_get_length(&pkt, &len))
            || !TEST_size_t_eq(len, 2)
            || !TEST_true(WPACKET_start_quic_sub_packet_bound(&pkt, OSSL_QUIC_VLINT_2B_MIN))
            || !TEST_true(WPACKET_quic_write_vlint(&pkt, 0x05))
            || !TEST_true(WPACKET_get_length(&pkt, &len))
            || !TEST_size_t_eq(len, 1)
            || !TEST_true(WPACKET_close(&pkt))
            || !TEST_true(WPACKET_start_quic_sub_packet_bound(&pkt, OSSL_QUIC_VLINT_2B_MIN))
            || !TEST_true(WPACKET_quic_write_vlint(&pkt, 0x11))
            || !TEST_true(WPACKET_close(&pkt))
            || !TEST_true(WPACKET_get_length(&pkt, &len))
            || !TEST_size_t_eq(len, 8)
            || !TEST_true(WPACKET_close(&pkt))
            || !TEST_true(WPACKET_start_quic_sub_packet_bound(&pkt, OSSL_QUIC_VLINT_2B_MIN))
            || !TEST_true(WPACKET_quic_write_vlint(&pkt, 0x12))
            || !TEST_true(WPACKET_close(&pkt))
            || !TEST_true(WPACKET_start_quic_sub_packet_bound(&pkt, OSSL_QUIC_VLINT_2B_MIN))
            || !TEST_true(WPACKET_quic_write_vlint(&pkt, 0x13))
            || !TEST_true(WPACKET_close(&pkt))
            || !TEST_true(WPACKET_finish(&pkt))
            || !TEST_true(WPACKET_get_total_written(&pkt, &written))
            || !TEST_mem_eq(buf->data, written, quic7, sizeof(quic7)))
        return cleanup(&pkt);

    /* Trying to encode a value above OSSL_QUIC_VLINT_MAX should fail */
    if (!TEST_true(WPACKET_init(&pkt, buf))
            || !TEST_false(WPACKET_quic_write_vlint(&pkt, OSSL_QUIC_VLINT_MAX+1))
            || !TEST_true(WPACKET_quic_write_vlint(&pkt, OSSL_QUIC_VLINT_MAX)))
            return cleanup(&pkt);

    WPACKET_cleanup(&pkt);
    return 1;
}

static int test_WPACKET_quic_vlint_random(void)
{
    size_t i, written;
    uint64_t expected, actual = 0;
    unsigned char rand_data[9];
    WPACKET pkt;
    PACKET read_pkt = {0};

    for (i = 0; i < 10000; ++i) {
        if (!TEST_int_gt(RAND_bytes(rand_data, sizeof(rand_data)), 0))
            return cleanup(&pkt);

        memcpy(&expected, rand_data, sizeof(expected));

        /*
         * Ensure that all size classes get tested with equal probability.
         */
        switch (rand_data[8] & 3) {
            case 0:
                expected &= OSSL_QUIC_VLINT_1B_MAX;
                break;
            case 1:
                expected &= OSSL_QUIC_VLINT_2B_MAX;
                break;
            case 2:
                expected &= OSSL_QUIC_VLINT_4B_MAX;
                break;
            case 3:
                expected &= OSSL_QUIC_VLINT_8B_MAX;
                break;
        }

        if (!TEST_true(WPACKET_init(&pkt, buf))
                || !TEST_true(WPACKET_quic_write_vlint(&pkt, expected))
                || !TEST_true(WPACKET_get_total_written(&pkt, &written)))
            return cleanup(&pkt);

        if (!TEST_true(PACKET_buf_init(&read_pkt, (unsigned char *)buf->data, written))
                || !TEST_true(PACKET_get_quic_vlint(&read_pkt, &actual))
                || !TEST_uint64_t_eq(expected, actual))
            return cleanup(&pkt);

        WPACKET_cleanup(&pkt);
    }

    WPACKET_cleanup(&pkt);
    return 1;
}

#endif

int setup_tests(void)
{
    if (!TEST_ptr(buf = BUF_MEM_new()))
            return 0;

    ADD_TEST(test_WPACKET_init);
    ADD_TEST(test_WPACKET_set_max_size);
    ADD_TEST(test_WPACKET_start_sub_packet);
    ADD_TEST(test_WPACKET_set_flags);
    ADD_TEST(test_WPACKET_allocate_bytes);
    ADD_TEST(test_WPACKET_memcpy);
    ADD_TEST(test_WPACKET_init_der);
#ifndef OPENSSL_NO_QUIC
    ADD_TEST(test_WPACKET_quic);
    ADD_TEST(test_WPACKET_quic_vlint_random);
#endif
    return 1;
}

void cleanup_tests(void)
{
    BUF_MEM_free(buf);
}
