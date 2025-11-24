/*
 * Copyright 2016-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "../testutil.h"
#include "output.h"
#include "tu_local.h"


int main(int argc, char *argv[])
{
    int ret = EXIT_FAILURE;
    int setup_res;

    test_open_streams();

    if (!global_init()) {
        test_printf_stderr("Global init failed - aborting\n");
        return ret;
    }

    if (!setup_test_framework(argc, argv))
        goto end;

    if ((setup_res = setup_tests()) > 0) {
        ret = run_tests(argv[0]);
        cleanup_tests();
        opt_check_usage();
    } else if (setup_res == 0) {
        opt_help(test_get_options());
    }
end:
    ret = pulldown_test_framework(ret);
    test_close_streams();
    return ret;
}
