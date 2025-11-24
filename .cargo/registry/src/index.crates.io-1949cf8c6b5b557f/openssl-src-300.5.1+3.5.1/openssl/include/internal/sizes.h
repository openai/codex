/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_INTERNAL_SIZES_H
# define OSSL_INTERNAL_SIZES_H
# pragma once

/*
 * Max sizes used to allocate buffers with a fixed sizes, for example for
 * stack allocations, structure fields, ...
 */
# define OSSL_MAX_NAME_SIZE           50 /* Algorithm name */
# define OSSL_MAX_PROPQUERY_SIZE     256 /* Property query strings */
# define OSSL_MAX_ALGORITHM_ID_SIZE  256 /* AlgorithmIdentifier DER */
# define OSSL_MAX_CODEC_STRUCT_SIZE   32 /* DATA_STRUCTURE name */

#endif
