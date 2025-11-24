/*
 * Copyright 2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "opt.h"
#include <openssl/ui.h>
#include "apps_ui.h"

/* This function is defined here due to visibility of bio_err */
int opt_printf_stderr(const char *fmt, ...)
{
    va_list ap;
    int ret;

    va_start(ap, fmt);
    ret = BIO_vprintf(bio_err, fmt, ap);
    va_end(ap);
    return ret;
}

