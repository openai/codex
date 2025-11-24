/*
 * Copyright 2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/rand.h>
#include "testutil.h"

/*
 * This needs to be in a test executable all by itself so that it can be
 * guaranteed to run before any generate calls have been made.
 */

static int test_rand_status(void)
{
    return TEST_true(RAND_status());
}

int setup_tests(void)
{
    ADD_TEST(test_rand_status);
    return 1;
}
