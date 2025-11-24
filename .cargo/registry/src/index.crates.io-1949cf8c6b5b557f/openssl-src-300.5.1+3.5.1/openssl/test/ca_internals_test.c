/*
 * Copyright 2021-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include "apps.h"
#include "testutil.h"
#include "crypto/asn1.h"

#define binname "ca_internals_test"

char *default_config_file = NULL;

static int test_do_updatedb(void)
{
    CA_DB *db = NULL;
    time_t testdateutc;
    int rv;
    size_t argc = test_get_argument_count();
    BIO *bio_tmp;
    char *testdate;
    char *indexfile;
    int need64bit;
    int have64bit;

    if (argc != 4) {
        TEST_error("Usage: %s: do_updatedb dbfile testdate need64bit\n", binname);
        TEST_error("       testdate format: ASN1-String\n");
        return 0;
    }

    /*
     * if the test will only work with 64bit time_t and
     * the build only supports 32, assume the test as success
     */
    need64bit = (int)strtol(test_get_argument(3), NULL, 0);
    have64bit = sizeof(time_t) > sizeof(uint32_t);
    if (need64bit && !have64bit) {
        BIO_printf(bio_out, "skipping test (need64bit: %i, have64bit: %i)",
            need64bit, have64bit);
        return 1;
    }

    testdate = test_get_argument(2);
    testdateutc = test_asn1_string_to_time_t(testdate);
    if (TEST_time_t_lt(testdateutc, 0)) {
        return 0;
    }

    indexfile = test_get_argument(1);
    db = load_index(indexfile, NULL);
    if (TEST_ptr_null(db)) {
        return 0;
    }

    bio_tmp = bio_err;
    bio_err = bio_out;
    rv = do_updatedb(db, &testdateutc);
    bio_err = bio_tmp;

    if (rv > 0) {
        if (!TEST_true(save_index(indexfile, "new", db)))
            goto end;

        if (!TEST_true(rotate_index(indexfile, "new", "old")))
            goto end;
    }
end:
    free_index(db);
    return 1;
}

int setup_tests(void)
{
    char *command = test_get_argument(0);

    if (test_get_argument_count() < 1) {
        TEST_error("%s: no command specified for testing\n", binname);
        return 0;
    }

    if (strcmp(command, "do_updatedb") == 0)
        return test_do_updatedb();

    TEST_error("%s: command '%s' is not supported for testing\n", binname, command);
    return 0;
}

