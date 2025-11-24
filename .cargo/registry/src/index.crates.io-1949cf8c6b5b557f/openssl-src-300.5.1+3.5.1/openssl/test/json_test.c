/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <string.h>

#include "testutil.h"
#include "internal/json_enc.h"

struct helper {
    OSSL_JSON_ENC   j;
    int             init;
    uint32_t        flags;
    BIO             *mem_bio;
};

static int helper_ensure(struct helper *h)
{
    if (h->init)
        return 1;

    if (!TEST_ptr(h->mem_bio = BIO_new(BIO_s_mem())))
        return 0;

    if (!ossl_json_init(&h->j, h->mem_bio, h->flags)) {
        BIO_free_all(h->mem_bio);
        h->mem_bio = NULL;
        return 0;
    }

    h->init = 1;
    return 1;
}

static void helper_cleanup(struct helper *h)
{
    BIO_free_all(h->mem_bio);
    h->mem_bio = NULL;

    if (h->init) {
        ossl_json_cleanup(&h->j);
        h->init = 0;
    }
}

static void helper_set_flags(struct helper *h, uint32_t flags)
{
    helper_cleanup(h);
    h->flags = flags;
}

struct script_word {
    void        *p;
    uint64_t    u64;
    int64_t     i64;
    double      d;
    void        (*fp)(void);
};

#define OP_P(x)     { (x) },
#define OP_U64(x)   { NULL, (x) },
#define OP_I64(x)   { NULL, 0, (x) },
#define OP_D(x)     { NULL, 0, 0, (x) },
#define OP_FP(x)    { NULL, 0, 0, 0, (void (*)(void))(x) },

struct script_info {
    const char                  *name, *title;
    const struct script_word    *words;
    size_t                      num_words;
    const char                  *expected_output;
    size_t                      expected_output_len;
};

typedef const struct script_info *(*info_func)(void);

enum {
    OPK_END,
    OPK_CALL,           /* (OSSL_JSON_ENC *) */
    OPK_CALL_P,         /* (OSSL_JSON_ENC *, const void *) */
    OPK_CALL_I,         /* (OSSL_JSON_ENC *, int) */
    OPK_CALL_U64,       /* (OSSL_JSON_ENC *, uint64_t) */
    OPK_CALL_I64,       /* (OSSL_JSON_ENC *, int64_t) */
    OPK_CALL_D,         /* (OSSL_JSON_ENC *, double) */
    OPK_CALL_PZ,        /* (OSSL_JSON_ENC *, const void *, size_t) */
    OPK_ASSERT_ERROR,   /* (OSSL_JSON_ENC *, int expect_error) */
    OPK_INIT_FLAGS      /* (uint32_t flags) */
};

typedef void (*fp_type)(OSSL_JSON_ENC *);
typedef void (*fp_p_type)(OSSL_JSON_ENC *, const void *);
typedef void (*fp_i_type)(OSSL_JSON_ENC *, int);
typedef void (*fp_u64_type)(OSSL_JSON_ENC *, uint64_t);
typedef void (*fp_i64_type)(OSSL_JSON_ENC *, int64_t);
typedef void (*fp_d_type)(OSSL_JSON_ENC *, double);
typedef void (*fp_pz_type)(OSSL_JSON_ENC *, const void *, size_t);

#define OP_END()              OP_U64(OPK_END)
#define OP_CALL(f)            OP_U64(OPK_CALL)     OP_FP(f)
#define OP_CALL_P(f, x)       OP_U64(OPK_CALL_P)   OP_FP(f) OP_P  (x)
#define OP_CALL_I(f, x)       OP_U64(OPK_CALL_I)   OP_FP(f) OP_I64(x)
#define OP_CALL_U64(f, x)     OP_U64(OPK_CALL_U64) OP_FP(f) OP_U64(x)
#define OP_CALL_I64(f, x)     OP_U64(OPK_CALL_I64) OP_FP(f) OP_I64(x)
#define OP_CALL_D(f, x)       OP_U64(OPK_CALL_D)   OP_FP(f) OP_D  (x)
#define OP_CALL_PZ(f, x, xl)  OP_U64(OPK_CALL_PZ)  OP_FP(f) OP_P  (x) OP_U64(xl)
#define OP_ASSERT_ERROR(err)  OP_U64(OPK_ASSERT_ERROR) OP_U64(err)
#define OP_INIT_FLAGS(flags)  OP_U64(OPK_INIT_FLAGS)   OP_U64(flags)

#define OPJ_BEGIN_O()         OP_CALL(ossl_json_object_begin)
#define OPJ_END_O()           OP_CALL(ossl_json_object_end)
#define OPJ_BEGIN_A()         OP_CALL(ossl_json_array_begin)
#define OPJ_END_A()           OP_CALL(ossl_json_array_end)
#define OPJ_NULL()            OP_CALL(ossl_json_null)
#define OPJ_BOOL(x)           OP_CALL_I(ossl_json_bool, (x))
#define OPJ_U64(x)            OP_CALL_U64(ossl_json_u64, (x))
#define OPJ_I64(x)            OP_CALL_I64(ossl_json_i64, (x))
#define OPJ_KEY(x)            OP_CALL_P(ossl_json_key, (x))
#define OPJ_STR(x)            OP_CALL_P(ossl_json_str, (x))
#define OPJ_STR_LEN(x, xl)    OP_CALL_PZ(ossl_json_str_len, (x), (xl))
#define OPJ_STR_HEX(x, xl)    OP_CALL_PZ(ossl_json_str_hex, (x), (xl))

#define BEGIN_SCRIPT(name, title, flags)                                       \
    static const struct script_info *get_script_##name(void)                   \
    {                                                                          \
        static const char script_name[] = #name;                               \
        static const char script_title[] = #title;                             \
                                                                               \
        static const struct script_word script_words[] = {                     \
            OP_INIT_FLAGS(flags)

#define END_SCRIPT_EXPECTING(s, slen)                                          \
            OP_END()                                                           \
        };                                                                     \
        static const struct script_info script_info = {                        \
            script_name, script_title, script_words, OSSL_NELEM(script_words), \
            (s), (slen)                                                        \
        };                                                                     \
        return &script_info;                                                   \
    }

#ifdef OPENSSL_SYS_VMS
/*
 * The VMS C compiler recognises \u in strings, and emits a warning, which
 * stops the build.  Because we think we know what we're doing, we change that
 * particular message to be merely informational.
 */
# pragma message informational UCNNOMAP
#endif

#define END_SCRIPT_EXPECTING_S(s)   END_SCRIPT_EXPECTING(s, SIZE_MAX)
#define END_SCRIPT_EXPECTING_Q(s)   END_SCRIPT_EXPECTING(#s, sizeof(#s) - 1)

#define SCRIPT(name) get_script_##name,

BEGIN_SCRIPT(null, "serialize a single null", 0)
    OPJ_NULL()
END_SCRIPT_EXPECTING_Q(null)

BEGIN_SCRIPT(obj_empty, "serialize an empty object", 0)
    OPJ_BEGIN_O()
    OPJ_END_O()
END_SCRIPT_EXPECTING_Q({})

BEGIN_SCRIPT(array_empty, "serialize an empty array", 0)
    OPJ_BEGIN_A()
    OPJ_END_A()
END_SCRIPT_EXPECTING_Q([])

BEGIN_SCRIPT(bool_false, "serialize false", 0)
    OPJ_BOOL(0)
END_SCRIPT_EXPECTING_Q(false)

BEGIN_SCRIPT(bool_true, "serialize true", 0)
    OPJ_BOOL(1)
END_SCRIPT_EXPECTING_Q(true)

BEGIN_SCRIPT(u64_0, "serialize u64(0)", 0)
    OPJ_U64(0)
END_SCRIPT_EXPECTING_Q(0)

BEGIN_SCRIPT(u64_1, "serialize u64(1)", 0)
    OPJ_U64(1)
END_SCRIPT_EXPECTING_Q(1)

BEGIN_SCRIPT(u64_10, "serialize u64(10)", 0)
    OPJ_U64(10)
END_SCRIPT_EXPECTING_Q(10)

BEGIN_SCRIPT(u64_12345, "serialize u64(12345)", 0)
    OPJ_U64(12345)
END_SCRIPT_EXPECTING_Q(12345)

BEGIN_SCRIPT(u64_18446744073709551615, "serialize u64(18446744073709551615)", 0)
    OPJ_U64(18446744073709551615ULL)
END_SCRIPT_EXPECTING_Q(18446744073709551615)

BEGIN_SCRIPT(i64_0, "serialize i64(0)", 0)
    OPJ_I64(0)
END_SCRIPT_EXPECTING_Q(0)

BEGIN_SCRIPT(i64_1, "serialize i64(1)", 0)
    OPJ_I64(1)
END_SCRIPT_EXPECTING_Q(1)

BEGIN_SCRIPT(i64_2, "serialize i64(2)", 0)
    OPJ_I64(2)
END_SCRIPT_EXPECTING_Q(2)

BEGIN_SCRIPT(i64_10, "serialize i64(10)", 0)
    OPJ_I64(10)
END_SCRIPT_EXPECTING_Q(10)

BEGIN_SCRIPT(i64_12345, "serialize i64(12345)", 0)
    OPJ_I64(12345)
END_SCRIPT_EXPECTING_Q(12345)

BEGIN_SCRIPT(i64_9223372036854775807, "serialize i64(9223372036854775807)", 0)
    OPJ_I64(9223372036854775807LL)
END_SCRIPT_EXPECTING_Q(9223372036854775807)

BEGIN_SCRIPT(i64_m1, "serialize i64(-1)", 0)
    OPJ_I64(-1)
END_SCRIPT_EXPECTING_Q(-1)

BEGIN_SCRIPT(i64_m2, "serialize i64(-2)", 0)
    OPJ_I64(-2)
END_SCRIPT_EXPECTING_Q(-2)

BEGIN_SCRIPT(i64_m10, "serialize i64(-10)", 0)
    OPJ_I64(-10)
END_SCRIPT_EXPECTING_Q(-10)

BEGIN_SCRIPT(i64_m12345, "serialize i64(-12345)", 0)
    OPJ_I64(-12345)
END_SCRIPT_EXPECTING_Q(-12345)

BEGIN_SCRIPT(i64_m9223372036854775807, "serialize i64(-9223372036854775807)", 0)
    OPJ_I64(-9223372036854775807LL)
END_SCRIPT_EXPECTING_Q(-9223372036854775807)

BEGIN_SCRIPT(i64_m9223372036854775808, "serialize i64(-9223372036854775808)", 0)
    OPJ_I64(-9223372036854775807LL - 1LL)
END_SCRIPT_EXPECTING_Q(-9223372036854775808)

BEGIN_SCRIPT(str_empty, "serialize \"\"", 0)
    OPJ_STR("")
END_SCRIPT_EXPECTING_Q("")

BEGIN_SCRIPT(str_a, "serialize \"a\"", 0)
    OPJ_STR("a")
END_SCRIPT_EXPECTING_Q("a")

BEGIN_SCRIPT(str_abc, "serialize \"abc\"", 0)
    OPJ_STR("abc")
END_SCRIPT_EXPECTING_Q("abc")

BEGIN_SCRIPT(str_quote, "serialize with quote", 0)
    OPJ_STR("abc\"def")
END_SCRIPT_EXPECTING_Q("abc\"def")

BEGIN_SCRIPT(str_quote2, "serialize with quote", 0)
    OPJ_STR("abc\"\"def")
END_SCRIPT_EXPECTING_Q("abc\"\"def")

BEGIN_SCRIPT(str_escape, "serialize with various escapes", 0)
    OPJ_STR("abc\"\"de'f\r\n\t\b\f\\\x01\v\x7f\\")
END_SCRIPT_EXPECTING_Q("abc\"\"de'f\r\n\t\b\f\\\u0001\u000b\u007f\\")

BEGIN_SCRIPT(str_len, "length-signalled string", 0)
    OPJ_STR_LEN("abcdef", 6)
END_SCRIPT_EXPECTING_Q("abcdef")

BEGIN_SCRIPT(str_len0, "0-length-signalled string", 0)
    OPJ_STR_LEN("", 0)
END_SCRIPT_EXPECTING_Q("")

BEGIN_SCRIPT(str_len_nul, "string with NUL", 0)
    OPJ_STR_LEN("x\0y", 3)
END_SCRIPT_EXPECTING_Q("x\u0000y")

BEGIN_SCRIPT(hex_data0, "zero-length hex data", 0)
    OPJ_STR_HEX("", 0)
END_SCRIPT_EXPECTING_Q("")

BEGIN_SCRIPT(hex_data, "hex data", 0)
    OPJ_STR_HEX("\x00\x01\x5a\xfb\xff", 5)
END_SCRIPT_EXPECTING_Q("00015afbff")

BEGIN_SCRIPT(array_nest1, "serialize nested empty arrays", 0)
    OPJ_BEGIN_A()
    OPJ_BEGIN_A()
    OPJ_END_A()
    OPJ_END_A()
END_SCRIPT_EXPECTING_Q([[]])

BEGIN_SCRIPT(array_nest2, "serialize nested empty arrays", 0)
    OPJ_BEGIN_A()
    OPJ_BEGIN_A()
    OPJ_BEGIN_A()
    OPJ_END_A()
    OPJ_END_A()
    OPJ_END_A()
END_SCRIPT_EXPECTING_Q([[[]]])

BEGIN_SCRIPT(array_nest3, "serialize nested empty arrays", 0)
    OPJ_BEGIN_A()
    OPJ_BEGIN_A()
    OPJ_BEGIN_A()
    OPJ_END_A()
    OPJ_BEGIN_A()
    OPJ_END_A()
    OPJ_BEGIN_A()
    OPJ_END_A()
    OPJ_END_A()
    OPJ_BEGIN_A()
    OPJ_END_A()
    OPJ_END_A()
END_SCRIPT_EXPECTING_S("[[[],[],[]],[]]")

BEGIN_SCRIPT(array_nest4, "deep nested arrays", 0)
    OPJ_BEGIN_A()
    OPJ_BEGIN_A()
    OPJ_BEGIN_A()
    OPJ_BEGIN_A()
    OPJ_BEGIN_A()
    OPJ_BEGIN_A()
    OPJ_BEGIN_A()
    OPJ_BEGIN_A()
    OPJ_BEGIN_A()
    OPJ_BEGIN_A()
    OPJ_BEGIN_A()
    OPJ_BEGIN_A()
    OPJ_BEGIN_A()
    OPJ_BEGIN_A()
    OPJ_BEGIN_A()
    OPJ_BEGIN_A()
    OPJ_BEGIN_A()
    OPJ_BEGIN_A()
    OPJ_BEGIN_A()
    OPJ_BEGIN_A()
    OPJ_END_A()
    OPJ_END_A()
    OPJ_END_A()
    OPJ_END_A()
    OPJ_END_A()
    OPJ_END_A()
    OPJ_END_A()
    OPJ_END_A()
    OPJ_END_A()
    OPJ_END_A()
    OPJ_END_A()
    OPJ_END_A()
    OPJ_END_A()
    OPJ_END_A()
    OPJ_END_A()
    OPJ_END_A()
    OPJ_END_A()
    OPJ_END_A()
    OPJ_END_A()
    OPJ_NULL()
    OPJ_END_A()
END_SCRIPT_EXPECTING_S("[[[[[[[[[[[[[[[[[[[[]]]]]]]]]]]]]]]]]]],null]")

BEGIN_SCRIPT(obj_nontrivial1, "serialize nontrivial object", 0)
    OPJ_BEGIN_O()
    OPJ_KEY("")
    OPJ_NULL()
    OPJ_END_O()
END_SCRIPT_EXPECTING_S("{\"\":null}")

BEGIN_SCRIPT(obj_nontrivial2, "serialize nontrivial object", 0)
    OPJ_BEGIN_O()
    OPJ_KEY("")
    OPJ_NULL()
    OPJ_KEY("x")
    OPJ_NULL()
    OPJ_END_O()
END_SCRIPT_EXPECTING_S("{\"\":null,\"x\":null}")

BEGIN_SCRIPT(obj_nest1, "serialize nested objects", 0)
    OPJ_BEGIN_O()
    OPJ_KEY("")
    OPJ_BEGIN_O()
    OPJ_KEY("x")
    OPJ_U64(42)
    OPJ_END_O()
    OPJ_KEY("x")
    OPJ_BEGIN_A()
    OPJ_U64(42)
    OPJ_U64(101)
    OPJ_END_A()
    OPJ_KEY("y")
    OPJ_NULL()
    OPJ_KEY("z")
    OPJ_BEGIN_O()
    OPJ_KEY("z0")
    OPJ_I64(-1)
    OPJ_KEY("z1")
    OPJ_I64(-2)
    OPJ_END_O()
    OPJ_END_O()
END_SCRIPT_EXPECTING_S("{\"\":{\"x\":42},\"x\":[42,101],\"y\":null,\"z\":{\"z0\":-1,\"z1\":-2}}")

BEGIN_SCRIPT(err_obj_no_key, "error test: object item without key", 0)
    OPJ_BEGIN_O()
    OP_ASSERT_ERROR(0)
    OPJ_NULL()
    OP_ASSERT_ERROR(1)
    OPJ_END_O()
    OP_ASSERT_ERROR(1)
END_SCRIPT_EXPECTING_S("{")

BEGIN_SCRIPT(err_obj_multi_key, "error test: object item with repeated key", 0)
    OPJ_BEGIN_O()
    OPJ_KEY("x")
    OP_ASSERT_ERROR(0)
    OPJ_KEY("y")
    OP_ASSERT_ERROR(1)
    OPJ_NULL()
    OP_ASSERT_ERROR(1)
END_SCRIPT_EXPECTING_S("{\"x\":")

BEGIN_SCRIPT(err_obj_no_value, "error test: object item with no value", 0)
    OPJ_BEGIN_O()
    OPJ_KEY("x")
    OP_ASSERT_ERROR(0)
    OPJ_END_O()
    OP_ASSERT_ERROR(1)
END_SCRIPT_EXPECTING_S("{\"x\":")

BEGIN_SCRIPT(err_utf8, "error test: only basic ASCII supported", 0)
    OPJ_STR("\x80")
    OP_ASSERT_ERROR(0)
END_SCRIPT_EXPECTING_S("\"\\u0080\"")

BEGIN_SCRIPT(utf8_2, "test: valid UTF-8 2byte supported", 0)
    OPJ_STR("low=\xc2\x80, high=\xdf\xbf")
    OP_ASSERT_ERROR(0)
END_SCRIPT_EXPECTING_S("\"low=\xc2\x80, high=\xdf\xbf\"")

BEGIN_SCRIPT(utf8_3, "test: valid UTF-8 3byte supported", 0)
    OPJ_STR("low=\xe0\xa0\x80, high=\xef\xbf\xbf")
    OP_ASSERT_ERROR(0)
END_SCRIPT_EXPECTING_S("\"low=\xe0\xa0\x80, high=\xef\xbf\xbf\"")

BEGIN_SCRIPT(utf8_4, "test: valid UTF-8 4byte supported", 0)
    OPJ_STR("low=\xf0\x90\xbf\xbf, high=\xf4\x8f\xbf\xbf")
    OP_ASSERT_ERROR(0)
END_SCRIPT_EXPECTING_S("\"low=\xf0\x90\xbf\xbf, high=\xf4\x8f\xbf\xbf\"")

BEGIN_SCRIPT(ijson_int, "I-JSON: large integer", OSSL_JSON_FLAG_IJSON)
    OPJ_BEGIN_A()
    OPJ_U64(1)
    OPJ_I64(-1)
    OPJ_U64(9007199254740991)
    OPJ_U64(9007199254740992)
    OPJ_I64(-9007199254740991)
    OPJ_I64(-9007199254740992)
    OPJ_END_A()
END_SCRIPT_EXPECTING_S("[1,-1,9007199254740991,\"9007199254740992\",-9007199254740991,\"-9007199254740992\"]")

BEGIN_SCRIPT(multi_item, "multiple top level items", 0)
    OPJ_NULL()
    OPJ_NULL()
    OPJ_BEGIN_A()
    OPJ_END_A()
    OPJ_BEGIN_A()
    OPJ_END_A()
END_SCRIPT_EXPECTING_S("nullnull[][]")

BEGIN_SCRIPT(seq, "JSON-SEQ", OSSL_JSON_FLAG_SEQ)
    OPJ_NULL()
    OPJ_NULL()
    OPJ_NULL()
    OPJ_BEGIN_O()
    OPJ_KEY("x")
    OPJ_U64(1)
    OPJ_KEY("y")
    OPJ_BEGIN_O()
    OPJ_END_O()
    OPJ_END_O()
END_SCRIPT_EXPECTING_S("\x1Enull\n" "\x1Enull\n" "\x1Enull\n" "\x1E{\"x\":1,\"y\":{}}\n")

static const info_func scripts[] = {
    SCRIPT(null)
    SCRIPT(obj_empty)
    SCRIPT(array_empty)
    SCRIPT(bool_false)
    SCRIPT(bool_true)
    SCRIPT(u64_0)
    SCRIPT(u64_1)
    SCRIPT(u64_10)
    SCRIPT(u64_12345)
    SCRIPT(u64_18446744073709551615)
    SCRIPT(i64_0)
    SCRIPT(i64_1)
    SCRIPT(i64_2)
    SCRIPT(i64_10)
    SCRIPT(i64_12345)
    SCRIPT(i64_9223372036854775807)
    SCRIPT(i64_m1)
    SCRIPT(i64_m2)
    SCRIPT(i64_m10)
    SCRIPT(i64_m12345)
    SCRIPT(i64_m9223372036854775807)
    SCRIPT(i64_m9223372036854775808)
    SCRIPT(str_empty)
    SCRIPT(str_a)
    SCRIPT(str_abc)
    SCRIPT(str_quote)
    SCRIPT(str_quote2)
    SCRIPT(str_escape)
    SCRIPT(str_len)
    SCRIPT(str_len0)
    SCRIPT(str_len_nul)
    SCRIPT(hex_data0)
    SCRIPT(hex_data)
    SCRIPT(array_nest1)
    SCRIPT(array_nest2)
    SCRIPT(array_nest3)
    SCRIPT(array_nest4)
    SCRIPT(obj_nontrivial1)
    SCRIPT(obj_nontrivial2)
    SCRIPT(obj_nest1)
    SCRIPT(err_obj_no_key)
    SCRIPT(err_obj_multi_key)
    SCRIPT(err_obj_no_value)
    SCRIPT(err_utf8)
    SCRIPT(utf8_2)
    SCRIPT(utf8_3)
    SCRIPT(utf8_4)
    SCRIPT(ijson_int)
    SCRIPT(multi_item)
    SCRIPT(seq)
};

/* Test runner. */
static int run_script(const struct script_info *info)
{
    int ok = 0, asserted = -1;
    const struct script_word *words = info->words;
    size_t wp = 0;
    struct script_word w;
    struct helper h = {0};
    BUF_MEM *bufp = NULL;

    TEST_info("running script '%s' (%s)", info->name, info->title);

#define GET_WORD()  (w = words[wp++])
#define GET_U64()   (GET_WORD().u64)
#define GET_I64()   (GET_WORD().i64)
#define GET_FP()    (GET_WORD().fp)
#define GET_P()     (GET_WORD().p)

    for (;;)
        switch (GET_U64()) {
        case OPK_END:
            goto stop;
        case OPK_INIT_FLAGS:
            helper_set_flags(&h, (uint32_t)GET_U64());
            break;
        case OPK_CALL:
        {
            fp_type f = (fp_type)GET_FP();

            if (!TEST_true(helper_ensure(&h)))
                goto err;

            f(&h.j);
            break;
        }
        case OPK_CALL_I:
        {
            fp_i_type f = (fp_i_type)GET_FP();

            if (!TEST_true(helper_ensure(&h)))
                goto err;

            f(&h.j, (int)GET_I64());
            break;
        }
        case OPK_CALL_U64:
        {
            fp_u64_type f = (fp_u64_type)GET_FP();

            if (!TEST_true(helper_ensure(&h)))
                goto err;

            f(&h.j, GET_U64());
            break;
        }
        case OPK_CALL_I64:
        {
            fp_i64_type f = (fp_i64_type)GET_FP();

            if (!TEST_true(helper_ensure(&h)))
                goto err;

            f(&h.j, GET_I64());
            break;
        }
        case OPK_CALL_P:
        {
            fp_p_type f = (fp_p_type)GET_FP();

            if (!TEST_true(helper_ensure(&h)))
                goto err;

            f(&h.j, GET_P());
            break;
        }
        case OPK_CALL_PZ:
        {
            fp_pz_type f = (fp_pz_type)GET_FP();
            void *p;
            uint64_t u64;

            if (!TEST_true(helper_ensure(&h)))
                goto err;

            p   = GET_P();
            u64 = GET_U64();
            f(&h.j, p, (size_t)u64);
            break;
        }
        case OPK_ASSERT_ERROR:
        {
            if (!TEST_true(helper_ensure(&h)))
                goto err;

            asserted = (int)GET_U64();
            if (!TEST_int_eq(ossl_json_in_error(&h.j), asserted))
                goto err;

            break;
        }
#define OP_ASSERT_ERROR(err)  OP_U64(OPK_ASSERT_ERROR) OP_U64(err)

        default:
            TEST_error("unknown opcode");
            goto err;
        }
stop:

    if (!TEST_true(helper_ensure(&h)))
        goto err;

    if (!TEST_true(ossl_json_flush(&h.j)))
        goto err;

    /* Implicit error check if not done explicitly. */
    if (asserted < 0 && !TEST_false(ossl_json_in_error(&h.j)))
        goto err;

    if (!TEST_true(BIO_get_mem_ptr(h.mem_bio, &bufp)))
        goto err;

    if (!TEST_mem_eq(bufp->data, bufp->length,
                     info->expected_output,
                     info->expected_output_len == SIZE_MAX
                        ? strlen(info->expected_output)
                        : info->expected_output_len))
        goto err;

    ok = 1;
err:
    if (!ok)
        TEST_error("script '%s' failed", info->name);

    helper_cleanup(&h);
    return ok;
}

static int test_json_enc(void)
{
    int ok = 1;
    size_t i;

    for (i = 0; i < OSSL_NELEM(scripts); ++i)
        if (!TEST_true(run_script(scripts[i]())))
            ok = 0;

    return ok;
}

int setup_tests(void)
{
    ADD_TEST(test_json_enc);
    return 1;
}
