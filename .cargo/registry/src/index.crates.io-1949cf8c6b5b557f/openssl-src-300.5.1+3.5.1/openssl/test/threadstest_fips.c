/*
 * Copyright 2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#if defined(_WIN32)
# include <windows.h>
#endif

#include "testutil.h"
#include "threadstest.h"

static int success;

static void thread_fips_rand_fetch(void)
{
    EVP_MD *md;

    if (!TEST_true(md = EVP_MD_fetch(NULL, "SHA2-256", NULL)))
        success = 0;
    EVP_MD_free(md);
}

static int test_fips_rand_leak(void)
{
    thread_t thread;

    success = 1;

    if (!TEST_true(run_thread(&thread, thread_fips_rand_fetch)))
        return 0;
    if (!TEST_true(wait_for_thread(thread)))
        return 0;
    return TEST_true(success);
}

int setup_tests(void)
{
    /*
     * This test MUST be run first.  Once the default library context is set
     * up, this test will always pass.
     */
    ADD_TEST(test_fips_rand_leak);
    return 1;
}
