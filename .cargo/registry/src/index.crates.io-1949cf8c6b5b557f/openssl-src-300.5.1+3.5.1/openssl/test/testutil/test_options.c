/*
 * Copyright 2018-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "../testutil.h"
#include "tu_local.h"

/* An overridable list of command line options */
const OPTIONS *test_get_options(void)
{
    static const OPTIONS default_options[] = {
        OPT_TEST_OPTIONS_DEFAULT_USAGE,
        { NULL }
    };
    return default_options;
}
