/*
 * Copyright 2019-2022 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2019, Oracle and/or its affiliates.  All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/params.h>
#include "testutil.h"

/* On machines that dont support <inttypes.h> just disable the tests */
#if !defined(OPENSSL_NO_INTTYPES_H)

# ifdef OPENSSL_SYS_VMS
#  define strtoumax strtoull
#  define strtoimax strtoll
# endif

typedef struct {
    OSSL_PARAM *param;
    int32_t i32;
    int64_t i64;
    uint32_t u32;
    uint64_t u64;
    double d;
    int valid_i32, valid_i64, valid_u32, valid_u64, valid_d;
    void *ref, *datum;
    size_t size;
} PARAM_CONVERSION;

static int param_conversion_load_stanza(PARAM_CONVERSION *pc, const STANZA *s)
{

    static int32_t datum_i32, ref_i32;
    static int64_t datum_i64, ref_i64;
    static uint32_t datum_u32, ref_u32;
    static uint64_t datum_u64, ref_u64;
    static double datum_d, ref_d;
    static OSSL_PARAM params[] = {
        OSSL_PARAM_int32("int32",   &datum_i32),
        OSSL_PARAM_int64("int64",   &datum_i64),
        OSSL_PARAM_uint32("uint32", &datum_u32),
        OSSL_PARAM_uint64("uint64", &datum_u64),
        OSSL_PARAM_double("double", &datum_d),
        OSSL_PARAM_END
    };
    int def_i32 = 0, def_i64 = 0, def_u32 = 0, def_u64 = 0, def_d = 0;
    const PAIR *pp = s->pairs;
    const char *type = NULL;
    char *p;
    int i;

    memset(pc, 0, sizeof(*pc));

    for (i = 0; i < s->numpairs; i++, pp++) {
        p = "";
        if (OPENSSL_strcasecmp(pp->key, "type") == 0) {
            if (type != NULL) {
                TEST_info("Line %d: multiple type lines", s->curr);
                return 0;
            }
            pc->param = OSSL_PARAM_locate(params, type = pp->value);
            if (pc->param == NULL) {
                TEST_info("Line %d: unknown type line", s->curr);
                return 0;
            }
        } else if (OPENSSL_strcasecmp(pp->key, "int32") == 0) {
            if (def_i32++) {
                TEST_info("Line %d: multiple int32 lines", s->curr);
                return 0;
            }
            if (OPENSSL_strcasecmp(pp->value, "invalid") != 0) {
                pc->valid_i32 = 1;
                pc->i32 = (int32_t)strtoimax(pp->value, &p, 10);
            }
        } else if (OPENSSL_strcasecmp(pp->key, "int64") == 0) {
            if (def_i64++) {
                TEST_info("Line %d: multiple int64 lines", s->curr);
                return 0;
            }
            if (OPENSSL_strcasecmp(pp->value, "invalid") != 0) {
                pc->valid_i64 = 1;
                pc->i64 = (int64_t)strtoimax(pp->value, &p, 10);
            }
        } else if (OPENSSL_strcasecmp(pp->key, "uint32") == 0) {
            if (def_u32++) {
                TEST_info("Line %d: multiple uint32 lines", s->curr);
                return 0;
            }
            if (OPENSSL_strcasecmp(pp->value, "invalid") != 0) {
                pc->valid_u32 = 1;
                pc->u32 = (uint32_t)strtoumax(pp->value, &p, 10);
            }
        } else if (OPENSSL_strcasecmp(pp->key, "uint64") == 0) {
            if (def_u64++) {
                TEST_info("Line %d: multiple uint64 lines", s->curr);
                return 0;
            }
            if (OPENSSL_strcasecmp(pp->value, "invalid") != 0) {
                pc->valid_u64 = 1;
                pc->u64 = (uint64_t)strtoumax(pp->value, &p, 10);
            }
        } else if (OPENSSL_strcasecmp(pp->key, "double") == 0) {
            if (def_d++) {
                TEST_info("Line %d: multiple double lines", s->curr);
                return 0;
            }
            if (OPENSSL_strcasecmp(pp->value, "invalid") != 0) {
                pc->valid_d = 1;
                pc->d = strtod(pp->value, &p);
            }
        } else {
            TEST_info("Line %d: unknown keyword %s", s->curr, pp->key);
            return 0;
        }
        if (*p != '\0') {
            TEST_info("Line %d: extra characters at end '%s' for %s",
                      s->curr, p, pp->key);
            return 0;
        }
    }

    if (!TEST_ptr(type)) {
        TEST_info("Line %d: type not found", s->curr);
        return 0;
    }

    if (OPENSSL_strcasecmp(type, "int32") == 0) {
        if (!TEST_true(def_i32) || !TEST_true(pc->valid_i32)) {
            TEST_note("errant int32 on line %d", s->curr);
            return 0;
        }
        datum_i32 = ref_i32 = pc->i32;
        pc->datum = &datum_i32;
        pc->ref = &ref_i32;
        pc->size = sizeof(ref_i32);
    } else if (OPENSSL_strcasecmp(type, "int64") == 0) {
        if (!TEST_true(def_i64) || !TEST_true(pc->valid_i64)) {
            TEST_note("errant int64 on line %d", s->curr);
            return 0;
        }
        datum_i64 = ref_i64 = pc->i64;
        pc->datum = &datum_i64;
        pc->ref = &ref_i64;
        pc->size = sizeof(ref_i64);
    } else if (OPENSSL_strcasecmp(type, "uint32") == 0) {
        if (!TEST_true(def_u32) || !TEST_true(pc->valid_u32)) {
            TEST_note("errant uint32 on line %d", s->curr);
            return 0;
        }
        datum_u32 = ref_u32 = pc->u32;
        pc->datum = &datum_u32;
        pc->ref = &ref_u32;
        pc->size = sizeof(ref_u32);
    } else if (OPENSSL_strcasecmp(type, "uint64") == 0) {
        if (!TEST_true(def_u64) || !TEST_true(pc->valid_u64)) {
            TEST_note("errant uint64 on line %d", s->curr);
            return 0;
        }
        datum_u64 = ref_u64 = pc->u64;
        pc->datum = &datum_u64;
        pc->ref = &ref_u64;
        pc->size = sizeof(ref_u64);
    } else if (OPENSSL_strcasecmp(type, "double") == 0) {
        if (!TEST_true(def_d) || !TEST_true(pc->valid_d)) {
            TEST_note("errant double on line %d", s->curr);
            return 0;
        }
        datum_d = ref_d = pc->d;
        pc->datum = &datum_d;
        pc->ref = &ref_d;
        pc->size = sizeof(ref_d);
    } else {
        TEST_error("type unknown at line %d", s->curr);
        return 0;
    }
    return 1;
}

static int param_conversion_test(const PARAM_CONVERSION *pc, int line)
{
    int32_t i32;
    int64_t i64;
    uint32_t u32;
    uint64_t u64;
    double d;

    if (!pc->valid_i32) {
        if (!TEST_false(OSSL_PARAM_get_int32(pc->param, &i32))
                || !TEST_ulong_ne(ERR_get_error(), 0)) {
            TEST_note("unexpected valid conversion to int32 on line %d", line);
            return 0;
        }
    } else {
        if (!TEST_true(OSSL_PARAM_get_int32(pc->param, &i32))
            || !TEST_true(i32 == pc->i32)) {
            TEST_note("unexpected conversion to int32 on line %d", line);
            return 0;
        }
        memset(pc->datum, 44, pc->size);
        if (!TEST_true(OSSL_PARAM_set_int32(pc->param, i32))
            || !TEST_mem_eq(pc->datum, pc->size, pc->ref, pc->size)) {
            TEST_note("unexpected valid conversion from int32 on line %d",
                      line);
            return 0;
        }
    }

    if (!pc->valid_i64) {
        if (!TEST_false(OSSL_PARAM_get_int64(pc->param, &i64))
                || !TEST_ulong_ne(ERR_get_error(), 0)) {
            TEST_note("unexpected valid conversion to int64 on line %d", line);
            return 0;
        }
    } else {
        if (!TEST_true(OSSL_PARAM_get_int64(pc->param, &i64))
            || !TEST_true(i64 == pc->i64)) {
            TEST_note("unexpected conversion to int64 on line %d", line);
            return 0;
        }
        memset(pc->datum, 44, pc->size);
        if (!TEST_true(OSSL_PARAM_set_int64(pc->param, i64))
            || !TEST_mem_eq(pc->datum, pc->size, pc->ref, pc->size)) {
            TEST_note("unexpected valid conversion from int64 on line %d",
                      line);
            return 0;
        }
    }

    if (!pc->valid_u32) {
        if (!TEST_false(OSSL_PARAM_get_uint32(pc->param, &u32))
                || !TEST_ulong_ne(ERR_get_error(), 0)) {
            TEST_note("unexpected valid conversion to uint32 on line %d", line);
            return 0;
        }
    } else {
        if (!TEST_true(OSSL_PARAM_get_uint32(pc->param, &u32))
            || !TEST_true(u32 == pc->u32)) {
            TEST_note("unexpected conversion to uint32 on line %d", line);
            return 0;
        }
        memset(pc->datum, 44, pc->size);
        if (!TEST_true(OSSL_PARAM_set_uint32(pc->param, u32))
            || !TEST_mem_eq(pc->datum, pc->size, pc->ref, pc->size)) {
            TEST_note("unexpected valid conversion from uint32 on line %d",
                      line);
            return 0;
        }
    }

    if (!pc->valid_u64) {
        if (!TEST_false(OSSL_PARAM_get_uint64(pc->param, &u64))
                || !TEST_ulong_ne(ERR_get_error(), 0)) {
            TEST_note("unexpected valid conversion to uint64 on line %d", line);
            return 0;
        }
    } else {
        if (!TEST_true(OSSL_PARAM_get_uint64(pc->param, &u64))
            || !TEST_true(u64 == pc->u64)) {
            TEST_note("unexpected conversion to uint64 on line %d", line);
            return 0;
        }
        memset(pc->datum, 44, pc->size);
        if (!TEST_true(OSSL_PARAM_set_uint64(pc->param, u64))
            || !TEST_mem_eq(pc->datum, pc->size, pc->ref, pc->size)) {
            TEST_note("unexpected valid conversion from uint64 on line %d",
                      line);
            return 0;
        }
    }

    if (!pc->valid_d) {
        if (!TEST_false(OSSL_PARAM_get_double(pc->param, &d))
                || !TEST_ulong_ne(ERR_get_error(), 0)) {
            TEST_note("unexpected valid conversion to double on line %d", line);
            return 0;
        }
    } else {
        if (!TEST_true(OSSL_PARAM_get_double(pc->param, &d))) {
            TEST_note("unable to convert to double on line %d", line);
            return 0;
        }
        /*
         * Check for not a number (NaN) without using the libm functions.
         * When d is a NaN, the standard requires d == d to be false.
         * It's less clear if d != d should be true even though it generally is.
         * Hence we use the equality test and a not.
         */
        if (!(d == d)) {
            /*
             * We've encountered a NaN so check it's really meant to be a NaN.
             * We ignore the case where the two values are both different NaN,
             * that's not resolvable without knowing the underlying format
             * or using libm functions.
             */
            if (!TEST_false(pc->d == pc->d)) {
                TEST_note("unexpected NaN on line %d", line);
                return 0;
            }
        } else if (!TEST_true(d == pc->d)) {
            TEST_note("unexpected conversion to double on line %d", line);
            return 0;
        }
        memset(pc->datum, 44, pc->size);
        if (!TEST_true(OSSL_PARAM_set_double(pc->param, d))
            || !TEST_mem_eq(pc->datum, pc->size, pc->ref, pc->size)) {
            TEST_note("unexpected valid conversion from double on line %d",
                      line);
            return 0;
        }
    }

    return 1;
}

static int run_param_file_tests(int i)
{
    STANZA *s;
    PARAM_CONVERSION pc;
    const char *testfile = test_get_argument(i);
    int res = 1;

    if (!TEST_ptr(s = OPENSSL_zalloc(sizeof(*s))))
        return 0;
    if (!test_start_file(s, testfile)) {
        OPENSSL_free(s);
        return 0;
    }

    while (!BIO_eof(s->fp)) {
        if (!test_readstanza(s)) {
            res = 0;
            goto end;
        }
        if (s->numpairs != 0)
            if (!param_conversion_load_stanza(&pc, s)
                || !param_conversion_test(&pc, s->curr))
                res = 0;
        test_clearstanza(s);
    }
end:
    test_end_file(s);
    OPENSSL_free(s);
    return res;
}

#endif /* OPENSSL_NO_INTTYPES_H */

OPT_TEST_DECLARE_USAGE("file...\n")

int setup_tests(void)
{
    size_t n;

    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    n = test_get_argument_count();
    if (n == 0)
        return 0;

#if !defined(OPENSSL_NO_INTTYPES_H)
    ADD_ALL_TESTS(run_param_file_tests, n);
#endif /* OPENSSL_NO_INTTYPES_H */

    return 1;
}
