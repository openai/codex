/*
 * Copyright 2018-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "../testutil.h"
#include "internal/nelem.h"
#include "tu_local.h"
#include "output.h"


static int used[100] = { 0 };

int test_skip_common_options(void)
{
    OPTION_CHOICE_DEFAULT o;

    while ((o = (OPTION_CHOICE_DEFAULT)opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_TEST_CASES:
           break;
        default:
        case OPT_ERR:
            return 0;
        }
    }
    return 1;
}

size_t test_get_argument_count(void)
{
    return opt_num_rest();
}

char *test_get_argument(size_t n)
{
    char **argv = opt_rest();

    OPENSSL_assert(n < sizeof(used));
    if ((int)n >= opt_num_rest() || argv == NULL)
        return NULL;
    used[n] = 1;
    return argv[n];
}

void opt_check_usage(void)
{
    int i;
    char **argv = opt_rest();
    int n, arg_count = opt_num_rest();

    if (arg_count > (int)OSSL_NELEM(used))
        n = (int)OSSL_NELEM(used);
    else
        n = arg_count;
    for (i = 0; i < n; i++) {
        if (used[i] == 0)
            test_printf_stderr("Warning ignored command-line argument %d: %s\n",
                               i, argv[i]);
    }
    if (i < arg_count)
        test_printf_stderr("Warning arguments %d and later unchecked\n", i);
}

int opt_printf_stderr(const char *fmt, ...)
{
    va_list ap;
    int ret;

    va_start(ap, fmt);
    ret = test_vprintf_stderr(fmt, ap);
    va_end(ap);
    return ret;
}

