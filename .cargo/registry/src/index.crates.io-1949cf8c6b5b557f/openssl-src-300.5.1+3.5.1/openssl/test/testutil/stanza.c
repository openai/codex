/*
 * Copyright 2017-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include <assert.h>
#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <ctype.h>

#include "internal/nelem.h"
#include "../testutil.h"
#include "tu_local.h"

int test_start_file(STANZA *s, const char *testfile)
{
    TEST_info("Reading %s", testfile);
    set_test_title(testfile);
    memset(s, 0, sizeof(*s));
    if (!TEST_ptr(s->fp = BIO_new_file(testfile, "r")))
        return 0;
    s->test_file = testfile;
    return 1;
}

int test_end_file(STANZA *s)
{
    TEST_info("Completed %d tests with %d errors and %d skipped",
              s->numtests, s->errors, s->numskip);
    BIO_free(s->fp);
    return 1;
}

/*
 * Read a PEM block.  Return 1 if okay, 0 on error.
 */
static int read_key(STANZA *s)
{
    char tmpbuf[128];

    if (s->key == NULL) {
        if (!TEST_ptr(s->key = BIO_new(BIO_s_mem())))
            return 0;
    } else if (!TEST_int_gt(BIO_reset(s->key), 0)) {
        return 0;
    }

    /* Read to PEM end line and place content in memory BIO */
    while (BIO_gets(s->fp, tmpbuf, sizeof(tmpbuf))) {
        s->curr++;
        if (!TEST_int_gt(BIO_puts(s->key, tmpbuf), 0))
            return 0;
        if (HAS_PREFIX(tmpbuf, "-----END"))
            return 1;
    }
    TEST_error("Can't find key end");
    return 0;
}


/*
 * Delete leading and trailing spaces from a string
 */
static char *strip_spaces(char *p)
{
    char *q;

    /* Skip over leading spaces */
    while (*p && isspace((unsigned char)*p))
        p++;
    if (*p == '\0')
        return NULL;

    for (q = p + strlen(p) - 1; q != p && isspace((unsigned char)*q); )
        *q-- = '\0';
    return *p ? p : NULL;
}

/*
 * Read next test stanza; return 1 if found, 0 on EOF or error.
 */
int test_readstanza(STANZA *s)
{
    PAIR *pp = s->pairs;
    char *p, *equals, *key;
    const char *value;
    static char buff[131072];

    for (s->numpairs = 0; BIO_gets(s->fp, buff, sizeof(buff)); ) {
        s->curr++;
        if (!TEST_ptr(p = strchr(buff, '\n'))) {
            TEST_info("Line %d too long", s->curr);
            return 0;
        }
        *p = '\0';

        /* Blank line marks end of tests. */
        if (buff[0] == '\0')
            break;

        /* Lines starting with a pound sign are ignored. */
        if (buff[0] == '#')
            continue;

        /* Parse into key=value */
        if (!TEST_ptr(equals = strchr(buff, '='))) {
            TEST_info("Missing = at line %d\n", s->curr);
            return 0;
        }
        *equals++ = '\0';
        if (!TEST_ptr(key = strip_spaces(buff))) {
            TEST_info("Empty field at line %d\n", s->curr);
            return 0;
        }
        if ((value = strip_spaces(equals)) == NULL)
            value = "";

        if (strcmp(key, "Title") == 0) {
            TEST_info("Starting \"%s\" tests at line %d", value, s->curr);
            continue;
        }

        if (s->numpairs == 0)
            s->start = s->curr;

        if (strcmp(key, "PrivateKey") == 0
                || strcmp(key, "PublicKey") == 0
                || strcmp(key, "ParamKey") == 0) {
            if (!read_key(s))
                return 0;
        }

        if (!TEST_int_lt(s->numpairs++, TESTMAXPAIRS)
                || !TEST_ptr(pp->key = OPENSSL_strdup(key))
                || !TEST_ptr(pp->value = OPENSSL_strdup(value)))
            return 0;
        pp++;
    }

    /* If we read anything, return ok. */
    return 1;
}

void test_clearstanza(STANZA *s)
{
    PAIR *pp = s->pairs;
    int i = s->numpairs;

    for ( ; --i >= 0; pp++) {
        OPENSSL_free(pp->key);
        OPENSSL_free(pp->value);
    }
    s->numpairs = 0;
}
