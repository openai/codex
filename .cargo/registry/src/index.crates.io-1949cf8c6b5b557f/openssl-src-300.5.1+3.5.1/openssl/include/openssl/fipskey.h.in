/*
 * {- join("\n * ", @autowarntext) -}
 *
 * Copyright 2020-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OPENSSL_FIPSKEY_H
# define OPENSSL_FIPSKEY_H
# pragma once

# ifdef  __cplusplus
extern "C" {
# endif

/*
 * The FIPS validation HMAC key, usable as an array initializer.
 */
#define FIPS_KEY_ELEMENTS \
    {- join(', ', map { "0x$_" } unpack("(A2)*", $config{FIPSKEY})) -}

/*
 * The FIPS validation key, as a string.
 */
#define FIPS_KEY_STRING "{- $config{FIPSKEY} -}"

/*
 * The FIPS provider vendor name, as a string.
 */
#define FIPS_VENDOR "{- $config{FIPS_VENDOR} -}"

# ifdef  __cplusplus
}
# endif

#endif
