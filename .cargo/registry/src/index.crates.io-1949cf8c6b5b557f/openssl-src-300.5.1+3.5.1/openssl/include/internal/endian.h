/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_INTERNAL_ENDIAN_H
# define OSSL_INTERNAL_ENDIAN_H
# pragma once

/*
 * IS_LITTLE_ENDIAN and IS_BIG_ENDIAN can be used to detect the endianness
 * at compile time. To use it, DECLARE_IS_ENDIAN must be used to declare
 * a variable.
 *
 * L_ENDIAN and B_ENDIAN can be used at preprocessor time. They can be set
 * in the configarion using the lib_cppflags variable. If neither is
 * set, it will fall back to code works with either endianness.
 */

# if defined(__BYTE_ORDER__) && defined(__ORDER_LITTLE_ENDIAN__)
#  define DECLARE_IS_ENDIAN const int ossl_is_little_endian = __BYTE_ORDER__ == __ORDER_LITTLE_ENDIAN__
#  define IS_LITTLE_ENDIAN (ossl_is_little_endian)
#  define IS_BIG_ENDIAN (!ossl_is_little_endian)
#  if defined(L_ENDIAN) && (__BYTE_ORDER__ != __ORDER_LITTLE_ENDIAN__)
#   error "L_ENDIAN defined on a big endian machine"
#  endif
#  if defined(B_ENDIAN) && (__BYTE_ORDER__ == __ORDER_LITTLE_ENDIAN__)
#   error "B_ENDIAN defined on a little endian machine"
#  endif
#  if !defined(L_ENDIAN) && (__BYTE_ORDER__ == __ORDER_LITTLE_ENDIAN__)
#   define L_ENDIAN
#  endif
#  if !defined(B_ENDIAN) && (__BYTE_ORDER__ != __ORDER_LITTLE_ENDIAN__)
#   define B_ENDIAN
#  endif
# else
#  define DECLARE_IS_ENDIAN \
    const union { \
        long one; \
        char little; \
    } ossl_is_endian = { 1 }

#  define IS_LITTLE_ENDIAN (ossl_is_endian.little != 0)
#  define IS_BIG_ENDIAN    (ossl_is_endian.little == 0)
# endif

#endif
