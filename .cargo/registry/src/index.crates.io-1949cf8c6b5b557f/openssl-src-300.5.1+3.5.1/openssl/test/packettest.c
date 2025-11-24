/*
 * Copyright 2015-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/packet_quic.h"
#include "testutil.h"

#define BUF_LEN 255

static unsigned char smbuf[BUF_LEN + 1];

static int test_PACKET_remaining(void)
{
    PACKET pkt;

    if (!TEST_true(PACKET_buf_init(&pkt, smbuf, BUF_LEN))
            || !TEST_size_t_eq(PACKET_remaining(&pkt), BUF_LEN)
            || !TEST_true(PACKET_forward(&pkt, BUF_LEN - 1))
            || !TEST_size_t_eq(PACKET_remaining(&pkt), 1)
            || !TEST_true(PACKET_forward(&pkt, 1))
            || !TEST_size_t_eq(PACKET_remaining(&pkt), 0))
        return 0;

    return 1;
}

static int test_PACKET_end(void)
{
    PACKET pkt;

    if (!TEST_true(PACKET_buf_init(&pkt, smbuf, BUF_LEN))
            || !TEST_size_t_eq(PACKET_remaining(&pkt), BUF_LEN)
            || !TEST_ptr_eq(PACKET_end(&pkt), smbuf + BUF_LEN)
            || !TEST_true(PACKET_forward(&pkt, BUF_LEN - 1))
            || !TEST_ptr_eq(PACKET_end(&pkt), smbuf + BUF_LEN)
            || !TEST_true(PACKET_forward(&pkt, 1))
            || !TEST_ptr_eq(PACKET_end(&pkt), smbuf + BUF_LEN))
        return 0;

    return 1;
}

static int test_PACKET_get_1(void)
{
    unsigned int i = 0;
    PACKET pkt;

    if (!TEST_true(PACKET_buf_init(&pkt, smbuf, BUF_LEN))
            || !TEST_true(PACKET_get_1(&pkt, &i))
            || !TEST_uint_eq(i, 0x02)
            || !TEST_true(PACKET_forward(&pkt, BUF_LEN - 2))
            || !TEST_true(PACKET_get_1(&pkt, &i))
            || !TEST_uint_eq(i, 0xfe)
            || !TEST_false(PACKET_get_1(&pkt, &i)))
        return 0;

    return 1;
}

static int test_PACKET_get_4(void)
{
    unsigned long i = 0;
    PACKET pkt;

    if (!TEST_true(PACKET_buf_init(&pkt, smbuf, BUF_LEN))
            || !TEST_true(PACKET_get_4(&pkt, &i))
            || !TEST_ulong_eq(i, 0x08060402UL)
            || !TEST_true(PACKET_forward(&pkt, BUF_LEN - 8))
            || !TEST_true(PACKET_get_4(&pkt, &i))
            || !TEST_ulong_eq(i, 0xfefcfaf8UL)
            || !TEST_false(PACKET_get_4(&pkt, &i)))
        return 0;

    return 1;
}

static int test_PACKET_get_net_2(void)
{
    unsigned int i = 0;
    PACKET pkt;

    if (!TEST_true(PACKET_buf_init(&pkt, smbuf, BUF_LEN))
            || !TEST_true(PACKET_get_net_2(&pkt, &i))
            || !TEST_uint_eq(i, 0x0204)
            || !TEST_true(PACKET_forward(&pkt, BUF_LEN - 4))
            || !TEST_true(PACKET_get_net_2(&pkt, &i))
            || !TEST_uint_eq(i, 0xfcfe)
            || !TEST_false(PACKET_get_net_2(&pkt, &i)))
        return 0;

    return 1;
}

static int test_PACKET_get_net_3(void)
{
    unsigned long i = 0;
    PACKET pkt;

    if (!TEST_true(PACKET_buf_init(&pkt, smbuf, BUF_LEN))
            || !TEST_true(PACKET_get_net_3(&pkt, &i))
            || !TEST_ulong_eq(i, 0x020406UL)
            || !TEST_true(PACKET_forward(&pkt, BUF_LEN - 6))
            || !TEST_true(PACKET_get_net_3(&pkt, &i))
            || !TEST_ulong_eq(i, 0xfafcfeUL)
            || !TEST_false(PACKET_get_net_3(&pkt, &i)))
        return 0;

    return 1;
}

static int test_PACKET_get_net_4(void)
{
    unsigned long i = 0;
    PACKET pkt;

    if (!TEST_true(PACKET_buf_init(&pkt, smbuf, BUF_LEN))
            || !TEST_true(PACKET_get_net_4(&pkt, &i))
            || !TEST_ulong_eq(i, 0x02040608UL)
            || !TEST_true(PACKET_forward(&pkt, BUF_LEN - 8))
            || !TEST_true(PACKET_get_net_4(&pkt, &i))
            || !TEST_ulong_eq(i, 0xf8fafcfeUL)
            || !TEST_false(PACKET_get_net_4(&pkt, &i)))
        return 0;

    return 1;
}

static int test_PACKET_get_sub_packet(void)
{
    PACKET pkt, subpkt;
    unsigned long i = 0;

    if (!TEST_true(PACKET_buf_init(&pkt, smbuf, BUF_LEN))
            || !TEST_true(PACKET_get_sub_packet(&pkt, &subpkt, 4))
            || !TEST_true(PACKET_get_net_4(&subpkt, &i))
            || !TEST_ulong_eq(i, 0x02040608UL)
            || !TEST_size_t_eq(PACKET_remaining(&subpkt), 0)
            || !TEST_true(PACKET_forward(&pkt, BUF_LEN - 8))
            || !TEST_true(PACKET_get_sub_packet(&pkt, &subpkt, 4))
            || !TEST_true(PACKET_get_net_4(&subpkt, &i))
            || !TEST_ulong_eq(i, 0xf8fafcfeUL)
            || !TEST_size_t_eq(PACKET_remaining(&subpkt), 0)
            || !TEST_false(PACKET_get_sub_packet(&pkt, &subpkt, 4)))
        return 0;

    return 1;
}

static int test_PACKET_get_bytes(void)
{
    const unsigned char *bytes = NULL;
    PACKET pkt;

    if (!TEST_true(PACKET_buf_init(&pkt, smbuf, BUF_LEN))
            || !TEST_true(PACKET_get_bytes(&pkt, &bytes, 4))
            || !TEST_uchar_eq(bytes[0], 2)
            || !TEST_uchar_eq(bytes[1], 4)
            || !TEST_uchar_eq(bytes[2], 6)
            || !TEST_uchar_eq(bytes[3], 8)
            || !TEST_size_t_eq(PACKET_remaining(&pkt), BUF_LEN -4)
            || !TEST_true(PACKET_forward(&pkt, BUF_LEN - 8))
            || !TEST_true(PACKET_get_bytes(&pkt, &bytes, 4))
            || !TEST_uchar_eq(bytes[0], 0xf8)
            || !TEST_uchar_eq(bytes[1], 0xfa)
            || !TEST_uchar_eq(bytes[2], 0xfc)
            || !TEST_uchar_eq(bytes[3], 0xfe)
            || !TEST_false(PACKET_remaining(&pkt)))
        return 0;

    return 1;
}

static int test_PACKET_copy_bytes(void)
{
    unsigned char bytes[4];
    PACKET pkt;

    if (!TEST_true(PACKET_buf_init(&pkt, smbuf, BUF_LEN))
            || !TEST_true(PACKET_copy_bytes(&pkt, bytes, 4))
            || !TEST_char_eq(bytes[0], 2)
            || !TEST_char_eq(bytes[1], 4)
            || !TEST_char_eq(bytes[2], 6)
            || !TEST_char_eq(bytes[3], 8)
            || !TEST_size_t_eq(PACKET_remaining(&pkt), BUF_LEN - 4)
            || !TEST_true(PACKET_forward(&pkt, BUF_LEN - 8))
            || !TEST_true(PACKET_copy_bytes(&pkt, bytes, 4))
            || !TEST_uchar_eq(bytes[0], 0xf8)
            || !TEST_uchar_eq(bytes[1], 0xfa)
            || !TEST_uchar_eq(bytes[2], 0xfc)
            || !TEST_uchar_eq(bytes[3], 0xfe)
            || !TEST_false(PACKET_remaining(&pkt)))
        return 0;

    return 1;
}

static int test_PACKET_copy_all(void)
{
    unsigned char tmp[BUF_LEN];
    PACKET pkt;
    size_t len;

    if (!TEST_true(PACKET_buf_init(&pkt, smbuf, BUF_LEN))
            || !TEST_true(PACKET_copy_all(&pkt, tmp, BUF_LEN, &len))
            || !TEST_size_t_eq(len, BUF_LEN)
            || !TEST_mem_eq(smbuf, BUF_LEN, tmp, BUF_LEN)
            || !TEST_size_t_eq(PACKET_remaining(&pkt), BUF_LEN)
            || !TEST_false(PACKET_copy_all(&pkt, tmp, BUF_LEN - 1, &len)))
        return 0;

    return 1;
}

static int test_PACKET_memdup(void)
{
    unsigned char *data = NULL;
    size_t len;
    PACKET pkt;
    int result = 0;

    if (!TEST_true(PACKET_buf_init(&pkt, smbuf, BUF_LEN))
            || !TEST_true(PACKET_memdup(&pkt, &data, &len))
            || !TEST_size_t_eq(len, BUF_LEN)
            || !TEST_mem_eq(data, len, PACKET_data(&pkt), len)
            || !TEST_true(PACKET_forward(&pkt, 10))
            || !TEST_true(PACKET_memdup(&pkt, &data, &len))
            || !TEST_size_t_eq(len, BUF_LEN - 10)
            || !TEST_mem_eq(data, len, PACKET_data(&pkt), len))
        goto end;
    result = 1;
end:
    OPENSSL_free(data);
    return result;
}

static int test_PACKET_strndup(void)
{
    char buf1[10], buf2[10];
    char *data = NULL;
    PACKET pkt;
    int result = 0;

    memset(buf1, 'x', 10);
    memset(buf2, 'y', 10);
    buf2[5] = '\0';

    if (!TEST_true(PACKET_buf_init(&pkt, (unsigned char*)buf1, 10))
            || !TEST_true(PACKET_strndup(&pkt, &data))
            || !TEST_size_t_eq(strlen(data), 10)
            || !TEST_strn_eq(data, buf1, 10)
            || !TEST_true(PACKET_buf_init(&pkt, (unsigned char*)buf2, 10))
            || !TEST_true(PACKET_strndup(&pkt, &data))
            || !TEST_size_t_eq(strlen(data), 5)
            || !TEST_str_eq(data, buf2))
        goto end;

    result = 1;
end:
    OPENSSL_free(data);
    return result;
}

static int test_PACKET_contains_zero_byte(void)
{
    char buf1[10], buf2[10];
    PACKET pkt;

    memset(buf1, 'x', 10);
    memset(buf2, 'y', 10);
    buf2[5] = '\0';

    if (!TEST_true(PACKET_buf_init(&pkt, (unsigned char*)buf1, 10))
            || !TEST_false(PACKET_contains_zero_byte(&pkt))
            || !TEST_true(PACKET_buf_init(&pkt, (unsigned char*)buf2, 10))
            || !TEST_true(PACKET_contains_zero_byte(&pkt)))
        return 0;

    return 1;
}

static int test_PACKET_forward(void)
{
    const unsigned char *byte = NULL;
    PACKET pkt;

    if (!TEST_true(PACKET_buf_init(&pkt, smbuf, BUF_LEN))
            || !TEST_true(PACKET_forward(&pkt, 1))
            || !TEST_true(PACKET_get_bytes(&pkt, &byte, 1))
            || !TEST_uchar_eq(byte[0], 4)
            || !TEST_true(PACKET_forward(&pkt, BUF_LEN - 3))
            || !TEST_true(PACKET_get_bytes(&pkt, &byte, 1))
            || !TEST_uchar_eq(byte[0], 0xfe))
        return 0;

    return 1;
}

static int test_PACKET_buf_init(void)
{
    unsigned char buf1[BUF_LEN] = { 0 };
    PACKET pkt;

    /* Also tests PACKET_remaining() */
    if (!TEST_true(PACKET_buf_init(&pkt, buf1, 4))
            || !TEST_size_t_eq(PACKET_remaining(&pkt), 4)
            || !TEST_true(PACKET_buf_init(&pkt, buf1, BUF_LEN))
            || !TEST_size_t_eq(PACKET_remaining(&pkt), BUF_LEN)
            || !TEST_false(PACKET_buf_init(&pkt, buf1, -1)))
        return 0;

    return 1;
}

static int test_PACKET_null_init(void)
{
    PACKET pkt;

    PACKET_null_init(&pkt);
    if (!TEST_size_t_eq(PACKET_remaining(&pkt), 0)
            || !TEST_false(PACKET_forward(&pkt, 1)))
        return 0;

    return 1;
}

static int test_PACKET_equal(void)
{
    PACKET pkt;

    if (!TEST_true(PACKET_buf_init(&pkt, smbuf, 4))
            || !TEST_true(PACKET_equal(&pkt, smbuf, 4))
            || !TEST_false(PACKET_equal(&pkt, smbuf + 1, 4))
            || !TEST_true(PACKET_buf_init(&pkt, smbuf, BUF_LEN))
            || !TEST_true(PACKET_equal(&pkt, smbuf, BUF_LEN))
            || !TEST_false(PACKET_equal(&pkt, smbuf, BUF_LEN - 1))
            || !TEST_false(PACKET_equal(&pkt, smbuf, BUF_LEN + 1))
            || !TEST_false(PACKET_equal(&pkt, smbuf, 0)))
        return 0;

    return 1;
}

static int test_PACKET_get_length_prefixed_1(void)
{
    unsigned char buf1[BUF_LEN];
    const size_t len = 16;
    unsigned int i;
    PACKET pkt, short_pkt, subpkt;

    memset(&subpkt, 0, sizeof(subpkt));
    buf1[0] = (unsigned char)len;
    for (i = 1; i < BUF_LEN; i++)
        buf1[i] = (i * 2) & 0xff;

    if (!TEST_true(PACKET_buf_init(&pkt, buf1, BUF_LEN))
            || !TEST_true(PACKET_buf_init(&short_pkt, buf1, len))
            || !TEST_true(PACKET_get_length_prefixed_1(&pkt, &subpkt))
            || !TEST_size_t_eq(PACKET_remaining(&subpkt), len)
            || !TEST_true(PACKET_get_net_2(&subpkt, &i))
            || !TEST_uint_eq(i, 0x0204)
            || !TEST_false(PACKET_get_length_prefixed_1(&short_pkt, &subpkt))
            || !TEST_size_t_eq(PACKET_remaining(&short_pkt), len))
        return 0;

    return 1;
}

static int test_PACKET_get_length_prefixed_2(void)
{
    unsigned char buf1[1024];
    const size_t len = 516;  /* 0x0204 */
    unsigned int i;
    PACKET pkt, short_pkt, subpkt;

    memset(&subpkt, 0, sizeof(subpkt));
    for (i = 1; i <= 1024; i++)
        buf1[i - 1] = (i * 2) & 0xff;

    if (!TEST_true(PACKET_buf_init(&pkt, buf1, 1024))
            || !TEST_true(PACKET_buf_init(&short_pkt, buf1, len))
            || !TEST_true(PACKET_get_length_prefixed_2(&pkt, &subpkt))
            || !TEST_size_t_eq(PACKET_remaining(&subpkt), len)
            || !TEST_true(PACKET_get_net_2(&subpkt, &i))
            || !TEST_uint_eq(i, 0x0608)
            || !TEST_false(PACKET_get_length_prefixed_2(&short_pkt, &subpkt))
            || !TEST_size_t_eq(PACKET_remaining(&short_pkt), len))
        return 0;

    return 1;
}

static int test_PACKET_get_length_prefixed_3(void)
{
    unsigned char buf1[1024];
    const size_t len = 516;  /* 0x000204 */
    unsigned int i;
    PACKET pkt, short_pkt, subpkt;

    memset(&subpkt, 0, sizeof(subpkt));
    for (i = 0; i < 1024; i++)
        buf1[i] = (i * 2) & 0xff;

    if (!TEST_true(PACKET_buf_init(&pkt, buf1, 1024))
            || !TEST_true(PACKET_buf_init(&short_pkt, buf1, len))
            || !TEST_true(PACKET_get_length_prefixed_3(&pkt, &subpkt))
            || !TEST_size_t_eq(PACKET_remaining(&subpkt), len)
            || !TEST_true(PACKET_get_net_2(&subpkt, &i))
            || !TEST_uint_eq(i, 0x0608)
            || !TEST_false(PACKET_get_length_prefixed_3(&short_pkt, &subpkt))
            || !TEST_size_t_eq(PACKET_remaining(&short_pkt), len))
        return 0;

    return 1;
}

static int test_PACKET_as_length_prefixed_1(void)
{
    unsigned char buf1[BUF_LEN];
    const size_t len = 16;
    unsigned int i;
    PACKET pkt, exact_pkt, subpkt;

    memset(&subpkt, 0, sizeof(subpkt));
    buf1[0] = (unsigned char)len;
    for (i = 1; i < BUF_LEN; i++)
        buf1[i] = (i * 2) & 0xff;

    if (!TEST_true(PACKET_buf_init(&pkt, buf1, BUF_LEN))
            || !TEST_true(PACKET_buf_init(&exact_pkt, buf1, len + 1))
            || !TEST_false(PACKET_as_length_prefixed_1(&pkt, &subpkt))
            || !TEST_size_t_eq(PACKET_remaining(&pkt), BUF_LEN)
            || !TEST_true(PACKET_as_length_prefixed_1(&exact_pkt, &subpkt))
            || !TEST_size_t_eq(PACKET_remaining(&exact_pkt), 0)
            || !TEST_size_t_eq(PACKET_remaining(&subpkt), len))
        return 0;

    return 1;
}

static int test_PACKET_as_length_prefixed_2(void)
{
    unsigned char buf[1024];
    const size_t len = 516;  /* 0x0204 */
    unsigned int i;
    PACKET pkt, exact_pkt, subpkt;

    memset(&subpkt, 0, sizeof(subpkt));
    for (i = 1; i <= 1024; i++)
        buf[i-1] = (i * 2) & 0xff;

    if (!TEST_true(PACKET_buf_init(&pkt, buf, 1024))
            || !TEST_true(PACKET_buf_init(&exact_pkt, buf, len + 2))
            || !TEST_false(PACKET_as_length_prefixed_2(&pkt, &subpkt))
            || !TEST_size_t_eq(PACKET_remaining(&pkt), 1024)
            || !TEST_true(PACKET_as_length_prefixed_2(&exact_pkt, &subpkt))
            || !TEST_size_t_eq(PACKET_remaining(&exact_pkt), 0)
            || !TEST_size_t_eq(PACKET_remaining(&subpkt), len))
        return 0;

    return 1;
}

#ifndef OPENSSL_NO_QUIC

static int test_PACKET_get_quic_vlint(void)
{
    struct quic_test_case {
        unsigned char buf[16];
        size_t expected_read_count;
        uint64_t value;
    };

    static const struct quic_test_case cases[] = {
        { {0x00}, 1, 0  },
        { {0x01}, 1, 1  },
        { {0x3e}, 1, 62 },
        { {0x3f}, 1, 63 },
        { {0x40,0x00}, 2, 0 },
        { {0x40,0x01}, 2, 1 },
        { {0x40,0x02}, 2, 2 },
        { {0x40,0xff}, 2, 255 },
        { {0x41,0x00}, 2, 256 },
        { {0x7f,0xfe}, 2, 16382 },
        { {0x7f,0xff}, 2, 16383 },
        { {0x80,0x00,0x00,0x00}, 4, 0 },
        { {0x80,0x00,0x00,0x01}, 4, 1 },
        { {0x80,0x00,0x01,0x02}, 4, 258 },
        { {0x80,0x18,0x49,0x65}, 4, 1591653 },
        { {0xbe,0x18,0x49,0x65}, 4, 1041779045 },
        { {0xbf,0xff,0xff,0xff}, 4, 1073741823 },
        { {0xc0,0x00,0x00,0x00,0x00,0x00,0x00,0x00}, 8, 0 },
        { {0xc0,0x00,0x00,0x00,0x00,0x00,0x01,0x02}, 8, 258 },
        { {0xfd,0x1f,0x59,0x8d,0xc9,0xf8,0x71,0x8a}, 8, 4404337426105397642 },
    };

    PACKET pkt;
    size_t i;
    uint64_t v;

    for (i = 0; i < OSSL_NELEM(cases); ++i) {
        memset(&pkt, 0, sizeof(pkt));
        v = 55;

        if (!TEST_true(PACKET_buf_init(&pkt, cases[i].buf, sizeof(cases[i].buf)))
                || !TEST_true(PACKET_get_quic_vlint(&pkt, &v))
                || !TEST_uint64_t_eq(v, cases[i].value)
                || !TEST_size_t_eq(PACKET_remaining(&pkt),
                                   sizeof(cases[i].buf) - cases[i].expected_read_count)
           )
            return 0;
    }

    return 1;
}

static int test_PACKET_get_quic_length_prefixed(void)
{
    struct quic_test_case {
        unsigned char buf[16];
        size_t enclen, len;
        int fail;
    };

    static const struct quic_test_case cases[] = {
        /* success cases */
        { {0x00}, 1, 0, 0 },
        { {0x01}, 1, 1, 0 },
        { {0x02}, 1, 2, 0 },
        { {0x03}, 1, 3, 0 },
        { {0x04}, 1, 4, 0 },
        { {0x05}, 1, 5, 0 },

        /* failure cases */
        { {0x10}, 1, 0, 1 },
        { {0x3f}, 1, 0, 1 },
    };

    size_t i;
    PACKET pkt, subpkt = {0};

    for (i = 0; i < OSSL_NELEM(cases); ++i) {
        memset(&pkt, 0, sizeof(pkt));

        if (!TEST_true(PACKET_buf_init(&pkt, cases[i].buf,
                                       cases[i].fail
                                         ? sizeof(cases[i].buf)
                                         : cases[i].enclen + cases[i].len)))
            return 0;

        if (!TEST_int_eq(PACKET_get_quic_length_prefixed(&pkt, &subpkt), !cases[i].fail))
            return 0;

        if (cases[i].fail) {
            if (!TEST_ptr_eq(pkt.curr, cases[i].buf))
                return 0;
            continue;
        }

        if (!TEST_ptr_eq(subpkt.curr, cases[i].buf + cases[i].enclen))
            return 0;

        if (!TEST_size_t_eq(subpkt.remaining, cases[i].len))
            return 0;
    }

    return 1;
}

#endif

int setup_tests(void)
{
    unsigned int i;

    for (i = 1; i <= BUF_LEN; i++)
        smbuf[i - 1] = (i * 2) & 0xff;

    ADD_TEST(test_PACKET_buf_init);
    ADD_TEST(test_PACKET_null_init);
    ADD_TEST(test_PACKET_remaining);
    ADD_TEST(test_PACKET_end);
    ADD_TEST(test_PACKET_equal);
    ADD_TEST(test_PACKET_get_1);
    ADD_TEST(test_PACKET_get_4);
    ADD_TEST(test_PACKET_get_net_2);
    ADD_TEST(test_PACKET_get_net_3);
    ADD_TEST(test_PACKET_get_net_4);
    ADD_TEST(test_PACKET_get_sub_packet);
    ADD_TEST(test_PACKET_get_bytes);
    ADD_TEST(test_PACKET_copy_bytes);
    ADD_TEST(test_PACKET_copy_all);
    ADD_TEST(test_PACKET_memdup);
    ADD_TEST(test_PACKET_strndup);
    ADD_TEST(test_PACKET_contains_zero_byte);
    ADD_TEST(test_PACKET_forward);
    ADD_TEST(test_PACKET_get_length_prefixed_1);
    ADD_TEST(test_PACKET_get_length_prefixed_2);
    ADD_TEST(test_PACKET_get_length_prefixed_3);
    ADD_TEST(test_PACKET_as_length_prefixed_1);
    ADD_TEST(test_PACKET_as_length_prefixed_2);
#ifndef OPENSSL_NO_QUIC
    ADD_TEST(test_PACKET_get_quic_vlint);
    ADD_TEST(test_PACKET_get_quic_length_prefixed);
#endif
    return 1;
}
