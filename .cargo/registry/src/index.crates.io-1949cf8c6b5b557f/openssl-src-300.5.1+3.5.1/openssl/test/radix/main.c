/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

OPT_TEST_DECLARE_USAGE("cert_file key_file\n")

/*
 * A RADIX test suite binding must define:
 *
 *   static SCRIPT_INFO *const scripts[];
 *
 *   int bindings_process_init(size_t node_idx, size_t process_idx);
 *   void bindings_process_finish(int testresult);
 *   int bindings_adjust_terp_config(TERP_CONFIG *cfg);
 *
 */
static int test_script(int idx)
{
    SCRIPT_INFO *script_info = scripts[idx];
    int testresult;
    TERP_CONFIG cfg = {0};

    if (!TEST_true(bindings_process_init(0, 0)))
        return 0;

    cfg.debug_bio = bio_err;

    if (!TEST_true(bindings_adjust_terp_config(&cfg)))
        return 0;

    testresult = TERP_run(script_info, &cfg);

    if (!bindings_process_finish(testresult))
        testresult = 0;

    return testresult;
}

int setup_tests(void)
{
    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    cert_file = test_get_argument(0);
    if (cert_file == NULL)
        cert_file = "test/certs/servercert.pem";

    key_file = test_get_argument(1);
    if (key_file == NULL)
        key_file = "test/certs/serverkey.pem";

    ADD_ALL_TESTS(test_script, OSSL_NELEM(scripts));
    return 1;
}
