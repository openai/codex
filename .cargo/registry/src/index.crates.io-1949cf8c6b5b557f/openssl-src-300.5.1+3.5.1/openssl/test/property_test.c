/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2019, Oracle and/or its affiliates.  All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdarg.h>
#include <openssl/evp.h>
#include "testutil.h"
#include "internal/nelem.h"
#include "internal/property.h"
#include "../crypto/property/property_local.h"

/*
 * We make our OSSL_PROVIDER for testing purposes.  All we really need is
 * a pointer.  We know that as long as we don't try to use the method
 * cache flush functions, the provider pointer is merely a pointer being
 * passed around, and used as a tag of sorts.
 */
struct ossl_provider_st {
    int x;
};

static int add_property_names(const char *n, ...)
{
    va_list args;
    int res = 1;

    va_start(args, n);
    do {
        if (!TEST_int_ne(ossl_property_name(NULL, n, 1), 0))
            res = 0;
    } while ((n = va_arg(args, const char *)) != NULL);
    va_end(args);
    return res;
}

static int up_ref(void *p)
{
    return 1;
}

static void down_ref(void *p)
{
}

static int test_property_string(void)
{
    OSSL_LIB_CTX *ctx;
    OSSL_METHOD_STORE *store = NULL;
    int res = 0;
    OSSL_PROPERTY_IDX i, j;

    /*-
     * Use our own library context because we depend on ordering from a
     * pristine state.
     */
    if (TEST_ptr(ctx = OSSL_LIB_CTX_new())
        && TEST_ptr(store = ossl_method_store_new(ctx))
        && TEST_int_eq(ossl_property_name(ctx, "fnord", 0), 0)
        && TEST_int_ne(ossl_property_name(ctx, "fnord", 1), 0)
        && TEST_int_ne(ossl_property_name(ctx, "name", 1), 0)
        /* Pre loaded names */
        && TEST_str_eq(ossl_property_name_str(ctx, 1), "provider")
        && TEST_str_eq(ossl_property_name_str(ctx, 2), "version")
        && TEST_str_eq(ossl_property_name_str(ctx, 3), "fips")
        && TEST_str_eq(ossl_property_name_str(ctx, 4), "output")
        && TEST_str_eq(ossl_property_name_str(ctx, 5), "input")
        && TEST_str_eq(ossl_property_name_str(ctx, 6), "structure")
        /* The names we added */
        && TEST_str_eq(ossl_property_name_str(ctx, 7), "fnord")
        && TEST_str_eq(ossl_property_name_str(ctx, 8), "name")
        /* Out of range */
        && TEST_ptr_null(ossl_property_name_str(ctx, 0))
        && TEST_ptr_null(ossl_property_name_str(ctx, 9))
        /* Property value checks */
        && TEST_int_eq(ossl_property_value(ctx, "fnord", 0), 0)
        && TEST_int_ne(i = ossl_property_value(ctx, "no", 0), 0)
        && TEST_int_ne(j = ossl_property_value(ctx, "yes", 0), 0)
        && TEST_int_ne(i, j)
        && TEST_int_eq(ossl_property_value(ctx, "yes", 1), j)
        && TEST_int_eq(ossl_property_value(ctx, "no", 1), i)
        && TEST_int_ne(i = ossl_property_value(ctx, "illuminati", 1), 0)
        && TEST_int_eq(j = ossl_property_value(ctx, "fnord", 1), i + 1)
        && TEST_int_eq(ossl_property_value(ctx, "fnord", 1), j)
        /* Pre loaded values */
        && TEST_str_eq(ossl_property_value_str(ctx, 1), "yes")
        && TEST_str_eq(ossl_property_value_str(ctx, 2), "no")
        /* The value we added */
        && TEST_str_eq(ossl_property_value_str(ctx, 3), "illuminati")
        && TEST_str_eq(ossl_property_value_str(ctx, 4), "fnord")
        /* Out of range */
        && TEST_ptr_null(ossl_property_value_str(ctx, 0))
        && TEST_ptr_null(ossl_property_value_str(ctx, 5))
        /* Check name and values are distinct */
        && TEST_int_eq(ossl_property_value(ctx, "cold", 0), 0)
        && TEST_int_ne(ossl_property_name(ctx, "fnord", 0),
                       ossl_property_value(ctx, "fnord", 0)))
        res = 1;
    ossl_method_store_free(store);
    OSSL_LIB_CTX_free(ctx);
    return res;
}

static const struct {
    const char *defn;
    const char *query;
    int e;
} parser_tests[] = {
    { "", "sky=blue", -1 },
    { "", "sky!=blue", 1 },
    { "groan", "", 0 },
    { "cold=yes", "cold=yes", 1 },
    { "cold=yes", "cold", 1 },
    { "cold=yes", "cold!=no", 1 },
    { "groan", "groan=yes", 1 },
    { "groan", "groan=no", -1 },
    { "groan", "groan!=yes", -1 },
    { "cold=no", "cold", -1 },
    { "cold=no", "?cold", 0 },
    { "cold=no", "cold=no", 1 },
    { "groan", "cold", -1 },
    { "groan", "cold=no", 1 },
    { "groan", "cold!=yes", 1 },
    { "groan=blue", "groan=yellow", -1 },
    { "groan=blue", "?groan=yellow", 0 },
    { "groan=blue", "groan!=yellow", 1 },
    { "groan=blue", "?groan!=yellow", 1 },
    { "today=monday, tomorrow=3", "today!=2", 1 },
    { "today=monday, tomorrow=3", "today!='monday'", -1 },
    { "today=monday, tomorrow=3", "tomorrow=3", 1 },
    { "n=0x3", "n=3", 1 },
    { "n=0x3", "n=-3", -1 },
    { "n=0x33", "n=51", 1 },
    { "n=0x123456789abcdef", "n=0x123456789abcdef", 1 },
    { "n=0x7fffffffffffffff", "n=0x7fffffffffffffff", 1 },   /* INT64_MAX */
    { "n=9223372036854775807", "n=9223372036854775807", 1 }, /* INT64_MAX */
    { "n=0777777777777777777777", "n=0777777777777777777777", 1 }, /* INT64_MAX */
    { "n=033", "n=27", 1 },
    { "n=0", "n=00", 1 },
    { "n=0x0", "n=0", 1 },
    { "n=0, sky=blue", "?n=0, sky=blue", 2 },
    { "n=1, sky=blue", "?n=0, sky=blue", 1 },
};

static int test_property_parse(int n)
{
    OSSL_METHOD_STORE *store;
    OSSL_PROPERTY_LIST *p = NULL, *q = NULL;
    int r = 0;

    if (TEST_ptr(store = ossl_method_store_new(NULL))
        && add_property_names("sky", "groan", "cold", "today", "tomorrow", "n",
                              NULL)
        && TEST_ptr(p = ossl_parse_property(NULL, parser_tests[n].defn))
        && TEST_ptr(q = ossl_parse_query(NULL, parser_tests[n].query, 0))
        && TEST_int_eq(ossl_property_match_count(q, p), parser_tests[n].e))
        r = 1;
    ossl_property_free(p);
    ossl_property_free(q);
    ossl_method_store_free(store);
    return r;
}

static int test_property_query_value_create(void)
{
    OSSL_METHOD_STORE *store;
    OSSL_PROPERTY_LIST *p = NULL, *q = NULL, *o = NULL;
    int r = 0;

    /* The property value used here must not be used in other test cases */
    if (TEST_ptr(store = ossl_method_store_new(NULL))
        && add_property_names("wood", NULL)
        && TEST_ptr(p = ossl_parse_query(NULL, "wood=oak", 0)) /* undefined */
        && TEST_ptr(q = ossl_parse_query(NULL, "wood=oak", 1)) /* creates */
        && TEST_ptr(o = ossl_parse_query(NULL, "wood=oak", 0)) /* defined */
        && TEST_int_eq(ossl_property_match_count(q, p), -1)
        && TEST_int_eq(ossl_property_match_count(q, o), 1))
        r = 1;
    ossl_property_free(o);
    ossl_property_free(p);
    ossl_property_free(q);
    ossl_method_store_free(store);
    return r;
}

static const struct {
    int query;
    const char *ps;
} parse_error_tests[] = {
    { 0, "n=1, n=1" },          /* duplicate name */
    { 0, "n=1, a=hi, n=1" },    /* duplicate name */
    { 1, "n=1, a=bye, ?n=0" },  /* duplicate name */
    { 0, "a=abc,#@!, n=1" },    /* non-ASCII character located */
    { 1, "a='Hello" },          /* Unterminated string */
    { 0, "a=\"World" },         /* Unterminated string */
    { 0, "a=_abd_" },           /* Unquoted string not starting with alphabetic */
    { 1, "a=2, n=012345678" },  /* Bad octal digit */
    { 0, "n=0x28FG, a=3" },     /* Bad hex digit */
    { 0, "n=145d, a=2" },       /* Bad decimal digit */
    { 0, "n=0x8000000000000000, a=3" },     /* Hex overflow */
    { 0, "n=922337203000000000d, a=2" },    /* Decimal overflow */
    { 0, "a=2, n=1000000000000000000000" }, /* Octal overflow */
    { 1, "@='hello'" },         /* Invalid name */
    { 1, "n0123456789012345678901234567890123456789"
         "0123456789012345678901234567890123456789"
         "0123456789012345678901234567890123456789"
         "0123456789012345678901234567890123456789=yes" }, /* Name too long */
    { 0, ".n=3" },              /* Invalid name */
    { 1, "fnord.fnord.=3" }     /* Invalid name */
};

static int test_property_parse_error(int n)
{
    OSSL_METHOD_STORE *store;
    OSSL_PROPERTY_LIST *p = NULL;
    int r = 0;
    const char *ps;

    if (!TEST_ptr(store = ossl_method_store_new(NULL))
        || !add_property_names("a", "n", NULL))
        goto err;
    ps = parse_error_tests[n].ps;
    if (parse_error_tests[n].query) {
        if (!TEST_ptr_null(p = ossl_parse_query(NULL, ps, 1)))
            goto err;
    } else if (!TEST_ptr_null(p = ossl_parse_property(NULL, ps))) {
        goto err;
    }
    r = 1;
 err:
    ossl_property_free(p);
    ossl_method_store_free(store);
    return r;
}

static const struct {
    const char *q_global;
    const char *q_local;
    const char *prop;
} merge_tests[] = {
    { "", "colour=blue", "colour=blue" },
    { "colour=blue", "", "colour=blue" },
    { "colour=red", "colour=blue", "colour=blue" },
    { "clouds=pink, urn=red", "urn=blue, colour=green",
        "urn=blue, colour=green, clouds=pink" },
    { "pot=gold", "urn=blue", "pot=gold, urn=blue" },
    { "night", "day", "day=yes, night=yes" },
    { "day", "night", "day=yes, night=yes" },
    { "", "", "" },
    /*
     * The following four leave 'day' unspecified in the query, and will match
     * any definition
     */
    { "day=yes", "-day", "day=no" },
    { "day=yes", "-day", "day=yes" },
    { "day=yes", "-day", "day=arglebargle" },
    { "day=yes", "-day", "pot=sesquioxidizing" },
    { "day, night", "-night, day", "day=yes, night=no" },
    { "-day", "day=yes", "day=yes" },
};

static int test_property_merge(int n)
{
    OSSL_METHOD_STORE *store;
    OSSL_PROPERTY_LIST *q_global = NULL, *q_local = NULL;
    OSSL_PROPERTY_LIST *q_combined = NULL, *prop = NULL;
    int r = 0;

    if (TEST_ptr(store = ossl_method_store_new(NULL))
        && add_property_names("colour", "urn", "clouds", "pot", "day", "night",
                              NULL)
        && TEST_ptr(prop = ossl_parse_property(NULL, merge_tests[n].prop))
        && TEST_ptr(q_global = ossl_parse_query(NULL, merge_tests[n].q_global,
                                                0))
        && TEST_ptr(q_local = ossl_parse_query(NULL, merge_tests[n].q_local, 0))
        && TEST_ptr(q_combined = ossl_property_merge(q_local, q_global))
        && TEST_int_ge(ossl_property_match_count(q_combined, prop), 0))
        r = 1;
    ossl_property_free(q_global);
    ossl_property_free(q_local);
    ossl_property_free(q_combined);
    ossl_property_free(prop);
    ossl_method_store_free(store);
    return r;
}

static int test_property_defn_cache(void)
{
    OSSL_METHOD_STORE *store;
    OSSL_PROPERTY_LIST *red = NULL, *blue = NULL, *blue2 = NULL;
    int r;

    r = TEST_ptr(store = ossl_method_store_new(NULL))
        && add_property_names("red", "blue", NULL)
        && TEST_ptr(red = ossl_parse_property(NULL, "red"))
        && TEST_ptr(blue = ossl_parse_property(NULL, "blue"))
        && TEST_ptr_ne(red, blue)
        && TEST_true(ossl_prop_defn_set(NULL, "red", &red));

    if (!r)  {
        ossl_property_free(red);
        red = NULL;
        ossl_property_free(blue);
        blue = NULL;
    }

    r = r && TEST_true(ossl_prop_defn_set(NULL, "blue", &blue));
    if (!r) {
        ossl_property_free(blue);
        blue = NULL;
    }

    r = r && TEST_ptr_eq(ossl_prop_defn_get(NULL, "red"), red)
        && TEST_ptr_eq(ossl_prop_defn_get(NULL, "blue"), blue)
        && TEST_ptr(blue2 = ossl_parse_property(NULL, "blue"))
        && TEST_ptr_ne(blue2, blue)
        && TEST_true(ossl_prop_defn_set(NULL, "blue", &blue2));
    if (!r) {
        ossl_property_free(blue2);
        blue2 = NULL;
    }

    r = r && TEST_ptr_eq(blue2, blue)
        && TEST_ptr_eq(ossl_prop_defn_get(NULL, "blue"), blue);

    ossl_method_store_free(store);
    return r;
}

static const struct {
    const char *defn;
    const char *query;
    int e;
} definition_tests[] = {
    { "alpha", "alpha=yes", 1 },
    { "alpha=no", "alpha", -1 },
    { "alpha=1", "alpha=1", 1 },
    { "alpha=2", "alpha=1",-1 },
    { "alpha", "omega", -1 },
    { "alpha", "?omega", 0 },
    { "alpha", "?omega=1", 0 },
    { "alpha", "?omega=no", 1 },
    { "alpha", "?omega=yes", 0 },
    { "alpha, omega", "?omega=yes", 1 },
    { "alpha, omega", "?omega=no", 0 }
};

static int test_definition_compares(int n)
{
    OSSL_METHOD_STORE *store;
    OSSL_PROPERTY_LIST *d = NULL, *q = NULL;
    int r;

    r = TEST_ptr(store = ossl_method_store_new(NULL))
        && add_property_names("alpha", "omega", NULL)
        && TEST_ptr(d = ossl_parse_property(NULL, definition_tests[n].defn))
        && TEST_ptr(q = ossl_parse_query(NULL, definition_tests[n].query, 0))
        && TEST_int_eq(ossl_property_match_count(q, d), definition_tests[n].e);

    ossl_property_free(d);
    ossl_property_free(q);
    ossl_method_store_free(store);
    return r;
}

static int test_register_deregister(void)
{
    static const struct {
        int nid;
        const char *prop;
        char *impl;
    } impls[] = {
        { 6, "position=1", "a" },
        { 6, "position=2", "b" },
        { 6, "position=3", "c" },
        { 6, "position=4", "d" },
    };
    size_t i;
    int ret = 0;
    OSSL_METHOD_STORE *store;
    OSSL_PROVIDER prov = { 1 };

    if (!TEST_ptr(store = ossl_method_store_new(NULL))
        || !add_property_names("position", NULL))
        goto err;

    for (i = 0; i < OSSL_NELEM(impls); i++)
        if (!TEST_true(ossl_method_store_add(store, &prov, impls[i].nid,
                                             impls[i].prop, impls[i].impl,
                                             &up_ref, &down_ref))) {
            TEST_note("iteration %zd", i + 1);
            goto err;
        }

    /* Deregister in a different order to registration */
    for (i = 0; i < OSSL_NELEM(impls); i++) {
        const size_t j = (1 + i * 3) % OSSL_NELEM(impls);
        int nid = impls[j].nid;
        void *impl = impls[j].impl;

        if (!TEST_true(ossl_method_store_remove(store, nid, impl))
            || !TEST_false(ossl_method_store_remove(store, nid, impl))) {
            TEST_note("iteration %zd, position %zd", i + 1, j + 1);
            goto err;
        }
    }

    if (TEST_false(ossl_method_store_remove(store, impls[0].nid, impls[0].impl)))
        ret = 1;
err:
    ossl_method_store_free(store);
    return ret;
}

static int test_property(void)
{
    static OSSL_PROVIDER fake_provider1 = { 1 };
    static OSSL_PROVIDER fake_provider2 = { 2 };
    static const OSSL_PROVIDER *fake_prov1 = &fake_provider1;
    static const OSSL_PROVIDER *fake_prov2 = &fake_provider2;
    static const struct {
        const OSSL_PROVIDER **prov;
        int nid;
        const char *prop;
        char *impl;
    } impls[] = {
        { &fake_prov1, 1, "fast=no, colour=green", "a" },
        { &fake_prov1, 1, "fast, colour=blue", "b" },
        { &fake_prov1, 1, "", "-" },
        { &fake_prov2, 9, "sky=blue, furry", "c" },
        { &fake_prov2, 3, NULL, "d" },
        { &fake_prov2, 6, "sky.colour=blue, sky=green, old.data", "e" },
    };
    static struct {
        const OSSL_PROVIDER **prov;
        int nid;
        const char *prop;
        char *expected;
    } queries[] = {
        { &fake_prov1, 1, "fast", "b" },
        { &fake_prov1, 1, "fast=yes", "b" },
        { &fake_prov1, 1, "fast=no, colour=green", "a" },
        { &fake_prov1, 1, "colour=blue, fast", "b" },
        { &fake_prov1, 1, "colour=blue", "b" },
        { &fake_prov2, 9, "furry", "c" },
        { &fake_prov2, 6, "sky.colour=blue", "e" },
        { &fake_prov2, 6, "old.data", "e" },
        { &fake_prov2, 9, "furry=yes, sky=blue", "c" },
        { &fake_prov1, 1, "", "a" },
        { &fake_prov2, 3, "", "d" },
    };
    OSSL_METHOD_STORE *store;
    size_t i;
    int ret = 0;
    void *result;

    if (!TEST_ptr(store = ossl_method_store_new(NULL))
        || !add_property_names("fast", "colour", "sky", "furry", NULL))
        goto err;

    for (i = 0; i < OSSL_NELEM(impls); i++)
        if (!TEST_true(ossl_method_store_add(store, *impls[i].prov,
                                             impls[i].nid, impls[i].prop,
                                             impls[i].impl,
                                             &up_ref, &down_ref))) {
            TEST_note("iteration %zd", i + 1);
            goto err;
        }
    /*
     * The first check of queries is with NULL given as provider.  All
     * queries are expected to succeed.
     */
    for (i = 0; i < OSSL_NELEM(queries); i++) {
        const OSSL_PROVIDER *nullprov = NULL;
        OSSL_PROPERTY_LIST *pq = NULL;

        if (!TEST_true(ossl_method_store_fetch(store,
                                               queries[i].nid, queries[i].prop,
                                               &nullprov, &result))
            || !TEST_str_eq((char *)result, queries[i].expected)) {
            TEST_note("iteration %zd", i + 1);
            ossl_property_free(pq);
            goto err;
        }
        ossl_property_free(pq);
    }
    /*
     * The second check of queries is with &address1 given as provider.
     */
    for (i = 0; i < OSSL_NELEM(queries); i++) {
        OSSL_PROPERTY_LIST *pq = NULL;

        result = NULL;
        if (queries[i].prov == &fake_prov1) {
            if (!TEST_true(ossl_method_store_fetch(store,
                                                   queries[i].nid,
                                                   queries[i].prop,
                                                   &fake_prov1, &result))
                || !TEST_ptr_eq(fake_prov1, &fake_provider1)
                || !TEST_str_eq((char *)result, queries[i].expected)) {
                TEST_note("iteration %zd", i + 1);
                ossl_property_free(pq);
                goto err;
            }
        } else {
            if (!TEST_false(ossl_method_store_fetch(store,
                                                    queries[i].nid,
                                                    queries[i].prop,
                                                    &fake_prov1, &result))
                || !TEST_ptr_eq(fake_prov1, &fake_provider1)
                || !TEST_ptr_null(result)) {
                TEST_note("iteration %zd", i + 1);
                ossl_property_free(pq);
                goto err;
            }
        }
        ossl_property_free(pq);
    }
    /*
     * The third check of queries is with &address2 given as provider.
     */
    for (i = 0; i < OSSL_NELEM(queries); i++) {
        OSSL_PROPERTY_LIST *pq = NULL;

        result = NULL;
        if (queries[i].prov == &fake_prov2) {
            if (!TEST_true(ossl_method_store_fetch(store,
                                                   queries[i].nid,
                                                   queries[i].prop,
                                                   &fake_prov2, &result))
                || !TEST_ptr_eq(fake_prov2, &fake_provider2)
                || !TEST_str_eq((char *)result, queries[i].expected)) {
                TEST_note("iteration %zd", i + 1);
                ossl_property_free(pq);
                goto err;
            }
        } else {
            if (!TEST_false(ossl_method_store_fetch(store,
                                                    queries[i].nid,
                                                    queries[i].prop,
                                                    &fake_prov2, &result))
                || !TEST_ptr_eq(fake_prov2, &fake_provider2)
                || !TEST_ptr_null(result)) {
                TEST_note("iteration %zd", i + 1);
                ossl_property_free(pq);
                goto err;
            }
        }
        ossl_property_free(pq);
    }
    ret = 1;
err:
    ossl_method_store_free(store);
    return ret;
}

static int test_query_cache_stochastic(void)
{
    const int max = 10000, tail = 10;
    OSSL_METHOD_STORE *store;
    int i, res = 0;
    char buf[50];
    void *result;
    int errors = 0;
    int v[10001];
    OSSL_PROVIDER prov = { 1 };

    if (!TEST_ptr(store = ossl_method_store_new(NULL))
        || !add_property_names("n", NULL))
        goto err;

    for (i = 1; i <= max; i++) {
        v[i] = 2 * i;
        BIO_snprintf(buf, sizeof(buf), "n=%d\n", i);
        if (!TEST_true(ossl_method_store_add(store, &prov, i, buf, "abc",
                                             &up_ref, &down_ref))
                || !TEST_true(ossl_method_store_cache_set(store, &prov, i,
                                                          buf, v + i,
                                                          &up_ref, &down_ref))
                || !TEST_true(ossl_method_store_cache_set(store, &prov, i,
                                                          "n=1234", "miss",
                                                          &up_ref, &down_ref))) {
            TEST_note("iteration %d", i);
            goto err;
        }
    }
    for (i = 1; i <= max; i++) {
        BIO_snprintf(buf, sizeof(buf), "n=%d\n", i);
        if (!ossl_method_store_cache_get(store, NULL, i, buf, &result)
            || result != v + i)
            errors++;
    }
    /* There is a tiny probability that this will fail when it shouldn't */
    res = TEST_int_gt(errors, tail) && TEST_int_lt(errors, max - tail);

err:
    ossl_method_store_free(store);
    return res;
}

static int test_fips_mode(void)
{
    int ret = 0;
    OSSL_LIB_CTX *ctx = NULL;

    if (!TEST_ptr(ctx = OSSL_LIB_CTX_new()))
        goto err;

    ret = TEST_true(EVP_set_default_properties(ctx, "default=yes,fips=yes"))
          && TEST_true(EVP_default_properties_is_fips_enabled(ctx))
          && TEST_true(EVP_set_default_properties(ctx, "fips=no,default=yes"))
          && TEST_false(EVP_default_properties_is_fips_enabled(ctx))
          && TEST_true(EVP_set_default_properties(ctx, "fips=no"))
          && TEST_false(EVP_default_properties_is_fips_enabled(ctx))
          && TEST_true(EVP_set_default_properties(ctx, "fips!=no"))
          && TEST_true(EVP_default_properties_is_fips_enabled(ctx))
          && TEST_true(EVP_set_default_properties(ctx, "fips=no"))
          && TEST_false(EVP_default_properties_is_fips_enabled(ctx))
          && TEST_true(EVP_set_default_properties(ctx, "fips=no,default=yes"))
          && TEST_true(EVP_default_properties_enable_fips(ctx, 1))
          && TEST_true(EVP_default_properties_is_fips_enabled(ctx))
          && TEST_true(EVP_default_properties_enable_fips(ctx, 0))
          && TEST_false(EVP_default_properties_is_fips_enabled(ctx));
err:
    OSSL_LIB_CTX_free(ctx);
    return ret;
}

static struct {
    const char *in;
    const char *out;
} to_string_tests[] = {
    { "fips=yes", "fips=yes" },
    { "fips!=yes", "fips!=yes" },
    { "fips = yes", "fips=yes" },
    { "fips", "fips=yes" },
    { "fips=no", "fips=no" },
    { "-fips", "-fips" },
    { "?fips=yes", "?fips=yes" },
    { "fips=yes,provider=fips", "fips=yes,provider=fips" },
    { "fips = yes , provider = fips", "fips=yes,provider=fips" },
    { "fips=yes,provider!=fips", "fips=yes,provider!=fips" },
    { "fips=yes,?provider=fips", "fips=yes,?provider=fips" },
    { "fips=yes,-provider", "fips=yes,-provider" },
      /* foo is an unknown internal name */
    { "foo=yes,fips=yes", "fips=yes"},
    { "", "" },
    { "fips=3", "fips=3" },
    { "fips=-3", "fips=-3" },
    { "provider='foo bar'", "provider='foo bar'" },
    { "provider=\"foo bar'\"", "provider=\"foo bar'\"" },
    { "provider=abc***", "provider='abc***'" },
    { NULL, "" }
};

static int test_property_list_to_string(int i)
{
    OSSL_PROPERTY_LIST *pl = NULL;
    int ret = 0;
    size_t bufsize;
    char *buf = NULL;

    if (to_string_tests[i].in != NULL
            && !TEST_ptr(pl = ossl_parse_query(NULL, to_string_tests[i].in, 1)))
        goto err;
    bufsize = ossl_property_list_to_string(NULL, pl, NULL, 0);
    if (!TEST_size_t_gt(bufsize, 0))
        goto err;
    buf = OPENSSL_malloc(bufsize);
    if (!TEST_ptr(buf)
            || !TEST_size_t_eq(ossl_property_list_to_string(NULL, pl, buf,
                                                            bufsize),
                               bufsize)
            || !TEST_str_eq(to_string_tests[i].out, buf)
            || !TEST_size_t_eq(bufsize, strlen(to_string_tests[i].out) + 1))
        goto err;

    ret = 1;
 err:
    OPENSSL_free(buf);
    ossl_property_free(pl);
    return ret;
}

int setup_tests(void)
{
    ADD_TEST(test_property_string);
    ADD_TEST(test_property_query_value_create);
    ADD_ALL_TESTS(test_property_parse, OSSL_NELEM(parser_tests));
    ADD_ALL_TESTS(test_property_parse_error, OSSL_NELEM(parse_error_tests));
    ADD_ALL_TESTS(test_property_merge, OSSL_NELEM(merge_tests));
    ADD_TEST(test_property_defn_cache);
    ADD_ALL_TESTS(test_definition_compares, OSSL_NELEM(definition_tests));
    ADD_TEST(test_register_deregister);
    ADD_TEST(test_property);
    ADD_TEST(test_query_cache_stochastic);
    ADD_TEST(test_fips_mode);
    ADD_ALL_TESTS(test_property_list_to_string, OSSL_NELEM(to_string_tests));
    return 1;
}
