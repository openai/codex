/*
 * Copyright 2016-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * This header file should be included by internal code that needs to use APIs
 * that have been deprecated for public use, but where those symbols will still
 * be available internally. For example the EVP and provider code needs to use
 * low level APIs that are otherwise deprecated.
 *
 * This header *must* be the first OpenSSL header included by a source file.
 */

#ifndef OSSL_INTERNAL_DEPRECATED_H
# define OSSL_INTERNAL_DEPRECATED_H
# pragma once

# include <openssl/configuration.h>

# undef OPENSSL_NO_DEPRECATED
# define OPENSSL_SUPPRESS_DEPRECATED

# include <openssl/macros.h>

#endif
