/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/crypto.h>
#include <string.h>

#include "crypto/punycode.h"
#include "internal/nelem.h"
#include "internal/packet.h"
#include "testutil.h"


static const struct puny_test {
    unsigned int raw[50];
    const char *encoded;
} puny_cases[] = {
    { /* Test of 4 byte codepoint using smileyface emoji */
        { 0x1F600
        },
        "e28h"
    },
    /* Test cases from RFC 3492 */
    {   /* Arabic (Egyptian) */
        { 0x0644, 0x064A, 0x0647, 0x0645, 0x0627, 0x0628, 0x062A, 0x0643, 0x0644,
          0x0645, 0x0648, 0x0634, 0x0639, 0x0631, 0x0628, 0x064A, 0x061F
        },
        "egbpdaj6bu4bxfgehfvwxn"
    },
    {   /* Chinese (simplified) */
        { 0x4ED6, 0x4EEC, 0x4E3A, 0x4EC0, 0x4E48, 0x4E0D, 0x8BF4, 0x4E2D, 0x6587
        },
        "ihqwcrb4cv8a8dqg056pqjye"
    },
    {   /* Chinese (traditional) */
        { 0x4ED6, 0x5011, 0x7232, 0x4EC0, 0x9EBD, 0x4E0D, 0x8AAA, 0x4E2D, 0x6587
        },
        "ihqwctvzc91f659drss3x8bo0yb"
    },
    {    /* Czech: Pro<ccaron>prost<ecaron>nemluv<iacute><ccaron>esky */
        { 0x0050, 0x0072, 0x006F, 0x010D, 0x0070, 0x0072, 0x006F, 0x0073, 0x0074,
          0x011B, 0x006E, 0x0065, 0x006D, 0x006C, 0x0075, 0x0076, 0x00ED, 0x010D,
          0x0065, 0x0073, 0x006B, 0x0079
       },
        "Proprostnemluvesky-uyb24dma41a"
    },
    {   /* Hebrew */
        { 0x05DC, 0x05DE, 0x05D4, 0x05D4, 0x05DD, 0x05E4, 0x05E9, 0x05D5, 0x05D8,
          0x05DC, 0x05D0, 0x05DE, 0x05D3, 0x05D1, 0x05E8, 0x05D9, 0x05DD, 0x05E2,
          0x05D1, 0x05E8, 0x05D9, 0x05EA
        },
        "4dbcagdahymbxekheh6e0a7fei0b"
    },
    {   /* Hindi (Devanagari) */
        { 0x092F, 0x0939, 0x0932, 0x094B, 0x0917, 0x0939, 0x093F, 0x0928, 0x094D,
          0x0926, 0x0940, 0x0915, 0x094D, 0x092F, 0x094B, 0x0902, 0x0928, 0x0939,
          0x0940, 0x0902, 0x092C, 0x094B, 0x0932, 0x0938, 0x0915, 0x0924, 0x0947,
          0x0939, 0x0948, 0x0902
        },
        "i1baa7eci9glrd9b2ae1bj0hfcgg6iyaf8o0a1dig0cd"
    },
    {   /* Japanese (kanji and hiragana) */
        { 0x306A, 0x305C, 0x307F, 0x3093, 0x306A, 0x65E5, 0x672C, 0x8A9E, 0x3092,
          0x8A71, 0x3057, 0x3066, 0x304F, 0x308C, 0x306A, 0x3044, 0x306E, 0x304B
        },
        "n8jok5ay5dzabd5bym9f0cm5685rrjetr6pdxa"
    },
    {   /* Korean (Hangul syllables) */
        { 0xC138, 0xACC4, 0xC758, 0xBAA8, 0xB4E0, 0xC0AC, 0xB78C, 0xB4E4, 0xC774,
          0xD55C, 0xAD6D, 0xC5B4, 0xB97C, 0xC774, 0xD574, 0xD55C, 0xB2E4, 0xBA74,
          0xC5BC, 0xB9C8, 0xB098, 0xC88B, 0xC744, 0xAE4C
        },
        "989aomsvi5e83db1d2a355cv1e0vak1dwrv93d5xbh15a0dt30a5jpsd879ccm6fea98c"
    },
    {   /* Russian (Cyrillic) */
        { 0x043F, 0x043E, 0x0447, 0x0435, 0x043C, 0x0443, 0x0436, 0x0435, 0x043E,
          0x043D, 0x0438, 0x043D, 0x0435, 0x0433, 0x043E, 0x0432, 0x043E, 0x0440,
          0x044F, 0x0442, 0x043F, 0x043E, 0x0440, 0x0443, 0x0441, 0x0441, 0x043A,
          0x0438
        },
        "b1abfaaepdrnnbgefbaDotcwatmq2g4l"
    },
    {   /* Spanish */
        { 0x0050, 0x006F, 0x0072, 0x0071, 0x0075, 0x00E9, 0x006E, 0x006F, 0x0070,
          0x0075, 0x0065, 0x0064, 0x0065, 0x006E, 0x0073, 0x0069, 0x006D, 0x0070,
          0x006C, 0x0065, 0x006D, 0x0065, 0x006E, 0x0074, 0x0065, 0x0068, 0x0061,
          0x0062, 0x006C, 0x0061, 0x0072, 0x0065, 0x006E, 0x0045, 0x0073, 0x0070,
          0x0061, 0x00F1, 0x006F, 0x006C
        },
        "PorqunopuedensimplementehablarenEspaol-fmd56a"
    },
    {   /* Vietnamese */
        { 0x0054, 0x1EA1, 0x0069, 0x0073, 0x0061, 0x006F, 0x0068, 0x1ECD, 0x006B,
          0x0068, 0x00F4, 0x006E, 0x0067, 0x0074, 0x0068, 0x1EC3, 0x0063, 0x0068,
          0x1EC9, 0x006E, 0x00F3, 0x0069, 0x0074, 0x0069, 0x1EBF, 0x006E, 0x0067,
          0x0056, 0x0069, 0x1EC7, 0x0074
        },
        "TisaohkhngthchnitingVit-kjcr8268qyxafd2f1b9g"
    },
    {   /* Japanese: 3<nen>B<gumi><kinpachi><sensei> */
        { 0x0033, 0x5E74, 0x0042, 0x7D44, 0x91D1, 0x516B, 0x5148, 0x751F
        },
        "3B-ww4c5e180e575a65lsy2b"
    },
    {   /* Japanese: <amuro><namie>-with-SUPER-MONKEYS */
        { 0x5B89, 0x5BA4, 0x5948, 0x7F8E, 0x6075, 0x002D, 0x0077, 0x0069, 0x0074,
          0x0068, 0x002D, 0x0053, 0x0055, 0x0050, 0x0045, 0x0052, 0x002D, 0x004D,
          0x004F, 0x004E, 0x004B, 0x0045, 0x0059, 0x0053
        },
        "-with-SUPER-MONKEYS-pc58ag80a8qai00g7n9n"
    },
    {   /* Japanese: Hello-Another-Way-<sorezore><no><basho> */
        { 0x0048, 0x0065, 0x006C, 0x006C, 0x006F, 0x002D, 0x0041, 0x006E, 0x006F,
          0x0074, 0x0068, 0x0065, 0x0072, 0x002D, 0x0057, 0x0061, 0x0079, 0x002D,
          0x305D, 0x308C, 0x305E, 0x308C, 0x306E, 0x5834, 0x6240
        },
        "Hello-Another-Way--fc4qua05auwb3674vfr0b"
    },
    {   /* Japanese: <hitotsu><yane><no><shita>2 */
        { 0x3072, 0x3068, 0x3064, 0x5C4B, 0x6839, 0x306E, 0x4E0B, 0x0032
        },
        "2-u9tlzr9756bt3uc0v"
    },
    {   /* Japanese: Maji<de>Koi<suru>5<byou><mae> */
        { 0x004D, 0x0061, 0x006A, 0x0069, 0x3067, 0x004B, 0x006F, 0x0069, 0x3059,
          0x308B, 0x0035, 0x79D2, 0x524D
        },
        "MajiKoi5-783gue6qz075azm5e"
    },
    {   /* Japanese: <pafii>de<runba> */
        { 0x30D1, 0x30D5, 0x30A3, 0x30FC, 0x0064, 0x0065, 0x30EB, 0x30F3, 0x30D0
        },
        "de-jg4avhby1noc0d"
    },
    {   /* Japanese: <sono><supiido><de> */
        { 0x305D, 0x306E, 0x30B9, 0x30D4, 0x30FC, 0x30C9, 0x3067
        },
        "d9juau41awczczp"
    },
    {   /* -> $1.00 <- */
        { 0x002D, 0x003E, 0x0020, 0x0024, 0x0031, 0x002E, 0x0030, 0x0030, 0x0020,
          0x003C, 0x002D
        },
        "-> $1.00 <--"
    }
};

static int test_punycode(int n)
{
    const struct puny_test *tc = puny_cases + n;
    unsigned int buffer[50];
    unsigned int bsize = OSSL_NELEM(buffer);
    size_t i;

    if (!TEST_true(ossl_punycode_decode(tc->encoded, strlen(tc->encoded),
                                        buffer, &bsize)))
        return 0;
    for (i = 0; i < OSSL_NELEM(tc->raw); i++)
        if (tc->raw[i] == 0)
            break;
    if (!TEST_mem_eq(buffer, bsize * sizeof(*buffer),
                     tc->raw, i * sizeof(*tc->raw)))
        return 0;
    return 1;
}

static const struct bad_decode_test {
    size_t outlen;
    const char input[20];
} bad_decode_tests[] = {
    { 20, "xn--e-*" },   /* bad digit '*' */
    { 10, "xn--e-999" }, /* loop > enc_len */
    { 20, "xn--e-999999999" }, /* Too big */
    { 20, {'x', 'n', '-', '-', (char)0x80, '-' } }, /* Not basic */
    { 20, "xn--e-Oy65t" }, /* codepoint > 0x10FFFF */
};

static int test_a2ulabel_bad_decode(int tst)
{
    char out[20];

    return TEST_int_eq(ossl_a2ulabel(bad_decode_tests[tst].input, out, bad_decode_tests[tst].outlen), -1);
}

static int test_a2ulabel(void)
{
    char out[50];
    char in[530] = { 0 };

    /*
     * The punycode being passed in and parsed is malformed but we're not
     * verifying that behaviour here.
     */
    if (!TEST_int_eq(ossl_a2ulabel("xn--a.b.c", out, 1), 0)
            || !TEST_int_eq(ossl_a2ulabel("xn--a.b.c", out, 7), 1))
        return 0;
    /* Test for an off by one on the buffer size works */
    if (!TEST_int_eq(ossl_a2ulabel("xn--a.b.c", out, 6), 0)
            || !TEST_int_eq(ossl_a2ulabel("xn--a.b.c", out, 7), 1)
            || !TEST_str_eq(out,"\xc2\x80.b.c"))
        return 0;

    /* Test 4 byte smiley face */
    if (!TEST_int_eq(ossl_a2ulabel("xn--e28h.com", out, 10), 1))
        return 0;

    /* Test that we dont overflow the fixed internal buffer of 512 bytes when the starting bytes are copied */
    strcpy(in, "xn--");
    memset(in + 4, 'e', 513);
    memcpy(in + 517, "-3ya", 4);
    if (!TEST_int_eq(ossl_a2ulabel(in, out, 50), -1))
        return 0;

    return 1;
}

static int test_puny_overrun(void)
{
    static const unsigned int out[] = {
        0x0033, 0x5E74, 0x0042, 0x7D44, 0x91D1, 0x516B, 0x5148, 0x751F
    };
    static const char *in = "3B-ww4c5e180e575a65lsy2b";
    unsigned int buf[OSSL_NELEM(out)];
    unsigned int bsize = OSSL_NELEM(buf) - 1;

    if (!TEST_false(ossl_punycode_decode(in, strlen(in), buf, &bsize))) {
        if (TEST_mem_eq(buf, bsize * sizeof(*buf), out, sizeof(out)))
            TEST_error("CRITICAL: buffer overrun detected!");
        return 0;
    }
    return 1;
}

static int test_dotted_overflow(void)
{
    static const char string[] = "a.a.a.a.a.a.a.a.a.a.a.a.a.a.a.a.a.a.a.a.a.a";
    const size_t num_reps = OSSL_NELEM(string) / 2;
    WPACKET p;
    BUF_MEM *in;
    char *out = NULL;
    size_t i;
    int res = 0;

    /* Create out input punycode string */
    if (!TEST_ptr(in = BUF_MEM_new()))
        return 0;
    if (!TEST_true(WPACKET_init_len(&p, in, 0))) {
        BUF_MEM_free(in);
        return 0;
    }
    for (i = 0; i < num_reps; i++) {
        if (i > 1 && !TEST_true(WPACKET_put_bytes_u8(&p, '.')))
            goto err;
        if (!TEST_true(WPACKET_memcpy(&p, "xn--a", sizeof("xn--a") - 1)))
            goto err;
    }
    if (!TEST_true(WPACKET_put_bytes_u8(&p, '\0')))
            goto err;
    if (!TEST_ptr(out = OPENSSL_malloc(in->length)))
        goto err;

    /* Test the decode into an undersized buffer */
    memset(out, 0x7f, in->length - 1);
    if (!TEST_int_le(ossl_a2ulabel(in->data, out, num_reps), 0)
            || !TEST_int_eq(out[num_reps], 0x7f))
        goto err;

    /* Test the decode works into a full size buffer */
    if (!TEST_int_gt(ossl_a2ulabel(in->data, out, in->length), 0)
            || !TEST_size_t_eq(strlen(out), num_reps * 3))
        goto err;

    res = 1;
 err:
    WPACKET_cleanup(&p);
    BUF_MEM_free(in);
    OPENSSL_free(out);
    return res;
}

int setup_tests(void)
{
    ADD_ALL_TESTS(test_punycode, OSSL_NELEM(puny_cases));
    ADD_TEST(test_dotted_overflow);
    ADD_TEST(test_a2ulabel);
    ADD_TEST(test_puny_overrun);
    ADD_ALL_TESTS(test_a2ulabel_bad_decode, OSSL_NELEM(bad_decode_tests));
    return 1;
}
