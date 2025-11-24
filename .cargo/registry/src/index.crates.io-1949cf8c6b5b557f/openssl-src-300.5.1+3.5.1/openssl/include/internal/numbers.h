/*
 * Copyright 2015-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_INTERNAL_NUMBERS_H
# define OSSL_INTERNAL_NUMBERS_H
# pragma once

# include <limits.h>

# if (-1 & 3) == 0x03           /* Two's complement */

#  define __MAXUINT__(T) ((T) -1)
#  define __MAXINT__(T) ((T) ((((T) 1) << ((sizeof(T) * CHAR_BIT) - 1)) ^ __MAXUINT__(T)))
#  define __MININT__(T) (-__MAXINT__(T) - 1)

# elif (-1 & 3) == 0x02         /* Ones' complement */

#  define __MAXUINT__(T) (((T) -1) + 1)
#  define __MAXINT__(T) ((T) ((((T) 1) << ((sizeof(T) * CHAR_BIT) - 1)) ^ __MAXUINT__(T)))
#  define __MININT__(T) (-__MAXINT__(T))

# elif (-1 & 3) == 0x01         /* Sign/magnitude */

#  define __MAXINT__(T) ((T) (((((T) 1) << ((sizeof(T) * CHAR_BIT) - 2)) - 1) | (((T) 1) << ((sizeof(T) * CHAR_BIT) - 2))))
#  define __MAXUINT__(T) ((T) (__MAXINT__(T) | (((T) 1) << ((sizeof(T) * CHAR_BIT) - 1))))
#  define __MININT__(T) (-__MAXINT__(T))

# else

#  error "do not know the integer encoding on this architecture"

# endif

# ifndef INT8_MAX
#  define INT8_MIN __MININT__(int8_t)
#  define INT8_MAX __MAXINT__(int8_t)
#  define UINT8_MAX __MAXUINT__(uint8_t)
# endif

# ifndef INT16_MAX
#  define INT16_MIN __MININT__(int16_t)
#  define INT16_MAX __MAXINT__(int16_t)
#  define UINT16_MAX __MAXUINT__(uint16_t)
# endif

# ifndef INT32_MAX
#  define INT32_MIN __MININT__(int32_t)
#  define INT32_MAX __MAXINT__(int32_t)
#  define UINT32_MAX __MAXUINT__(uint32_t)
# endif

# ifndef INT64_MAX
#  define INT64_MIN __MININT__(int64_t)
#  define INT64_MAX __MAXINT__(int64_t)
#  define UINT64_MAX __MAXUINT__(uint64_t)
# endif

/*
 * 64-bit processor with LP64 ABI
 */
# ifdef SIXTY_FOUR_BIT_LONG
#  ifndef UINT32_C
#   define UINT32_C(c) (c)
#  endif
#  ifndef UINT64_C
#   define UINT64_C(c) (c##UL)
#  endif
# endif

/*
 * 64-bit processor other than LP64 ABI
 */
# ifdef SIXTY_FOUR_BIT
#  ifndef UINT32_C
#   define UINT32_C(c) (c##UL)
#  endif
#  ifndef UINT64_C
#   define UINT64_C(c) (c##ULL)
#  endif
# endif


# ifndef INT128_MAX
#  if defined(__SIZEOF_INT128__) && __SIZEOF_INT128__ == 16
typedef __int128_t int128_t;
typedef __uint128_t uint128_t;
#   define INT128_MIN __MININT__(int128_t)
#   define INT128_MAX __MAXINT__(int128_t)
#   define UINT128_MAX __MAXUINT__(uint128_t)
#  endif
# endif

# ifndef SIZE_MAX
#  define SIZE_MAX __MAXUINT__(size_t)
# endif

# ifndef OSSL_INTMAX_MAX
#  define OSSL_INTMAX_MIN __MININT__(ossl_intmax_t)
#  define OSSL_INTMAX_MAX __MAXINT__(ossl_intmax_t)
#  define OSSL_UINTMAX_MAX __MAXUINT__(ossl_uintmax_t)
# endif

#endif

