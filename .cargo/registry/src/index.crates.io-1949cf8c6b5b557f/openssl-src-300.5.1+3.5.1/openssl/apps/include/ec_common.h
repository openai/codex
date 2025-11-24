/*
 * Copyright 2020-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OPENSSL_NO_EC
static const char *point_format_options[] = {
    "uncompressed",
    "compressed",
    "hybrid",
    NULL
};

static const char *asn1_encoding_options[] = {
    "named_curve",
    "explicit",
    NULL
};
#endif
