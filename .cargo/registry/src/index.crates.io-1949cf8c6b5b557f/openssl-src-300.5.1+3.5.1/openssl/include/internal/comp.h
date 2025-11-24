/*
 * Copyright 2016-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#ifndef _INTERNAL_COMP_H
#define	_INTERNAL_COMP_H

#include <openssl/comp.h>

void ossl_comp_zlib_cleanup(void);
void ossl_comp_brotli_cleanup(void);
void ossl_comp_zstd_cleanup(void);

struct ssl_comp_st {
    int id;
    const char *name;
    COMP_METHOD *method;
};

#endif
