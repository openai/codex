/*
 * Copyright 2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stddef.h>
#include <openssl/params.h>

/*
 * Extract the parameter into an allocated buffer.
 * Any existing allocation in *out is cleared and freed.
 *
 * Returns 1 on success, 0 on failure and -1  if there are no matching params.
 *
 * *out and *out_len are guaranteed to be untouched if this function
 * doesn't return success.
 */
int ossl_param_get1_octet_string(const OSSL_PARAM *params, const char *name,
                                 unsigned char **out, size_t *out_len);
/*
 * Concatenate all of the matching params together.
 * *out will point to an allocated buffer on successful return.
 * Any existing allocation in *out is cleared and freed.
 *
 * Passing 0 for maxsize means unlimited size output.
 *
 * Returns 1 on success, 0 on failure and -1 if there are no matching params.
 *
 * *out and *out_len are guaranteed to be untouched if this function
 * doesn't return success.
 */
int ossl_param_get1_concat_octet_string(const OSSL_PARAM *params, const char *name,
                                        unsigned char **out, size_t *out_len,
                                        size_t maxsize);
