// "License": Public Domain
// I, Mathias Panzenböck, place this file hereby into the public domain. Use it at your own risk for whatever you like.
// In case there are jurisdictions that don't support putting things in the public domain you can also consider it to
// be "dual licensed" under the BSD, MIT and Apache licenses, if you want to. This code is trivial anyway. Consider it
// an example on how to get the endian conversion functions on different platforms.

// updates from https://github.com/mikepb/endian.h/issues/4

#ifndef ENDIAN_H
#define ENDIAN_H

#if (defined(_WIN16) || defined(_WIN32) || defined(_WIN64)) && !defined(__WINDOWS__)

#    define __WINDOWS__

#endif

#if defined(HAVE_ENDIAN_H) || \
    defined(__linux__) || \
    defined(__GNU__) || \
    defined(__HAIKU__) || \
    defined(__illumos__) || \
    defined(__NetBSD__) || \
    defined(__OpenBSD__) || \
    defined(__CYGWIN__) || \
    defined(__MSYS__) || \
    defined(__EMSCRIPTEN__) || \
    defined(__wasi__) || \
    defined(__wasm__)

#if defined(__NetBSD__)
#define _NETBSD_SOURCE 1
#endif

# include <endian.h>

#elif defined(HAVE_SYS_ENDIAN_H) || \
    defined(__FreeBSD__) || \
    defined(__DragonFly__)

# include <sys/endian.h>

#elif defined(__APPLE__)
#    define __BYTE_ORDER    BYTE_ORDER
#    define __BIG_ENDIAN    BIG_ENDIAN
#    define __LITTLE_ENDIAN LITTLE_ENDIAN
#    define __PDP_ENDIAN    PDP_ENDIAN

#    if !defined(_POSIX_C_SOURCE)
#        include <libkern/OSByteOrder.h>

#        define htobe16(x) OSSwapHostToBigInt16(x)
#        define htole16(x) OSSwapHostToLittleInt16(x)
#        define be16toh(x) OSSwapBigToHostInt16(x)
#        define le16toh(x) OSSwapLittleToHostInt16(x)

#        define htobe32(x) OSSwapHostToBigInt32(x)
#        define htole32(x) OSSwapHostToLittleInt32(x)
#        define be32toh(x) OSSwapBigToHostInt32(x)
#        define le32toh(x) OSSwapLittleToHostInt32(x)

#        define htobe64(x) OSSwapHostToBigInt64(x)
#        define htole64(x) OSSwapHostToLittleInt64(x)
#        define be64toh(x) OSSwapBigToHostInt64(x)
#        define le64toh(x) OSSwapLittleToHostInt64(x)
#    else
#        if BYTE_ORDER == LITTLE_ENDIAN
#            define htobe16(x) __builtin_bswap16(x)
#            define htole16(x) (x)
#            define be16toh(x) __builtin_bswap16(x)
#            define le16toh(x) (x)

#            define htobe32(x) __builtin_bswap32(x)
#            define htole32(x) (x)
#            define be32toh(x) __builtin_bswap32(x)
#            define le32toh(x) (x)

#            define htobe64(x) __builtin_bswap64(x)
#            define htole64(x) (x)
#            define be64toh(x) __builtin_bswap64(x)
#            define le64toh(x) (x)
#        elif BYTE_ORDER == BIG_ENDIAN
#            define htobe16(x) (x)
#            define htole16(x) __builtin_bswap16(x)
#            define be16toh(x) (x)
#            define le16toh(x) __builtin_bswap16(x)

#            define htobe32(x) (x)
#            define htole32(x) __builtin_bswap32(x)
#            define be32toh(x) (x)
#            define le32toh(x) __builtin_bswap32(x)

#            define htobe64(x) (x)
#            define htole64(x) __builtin_bswap64(x)
#            define be64toh(x) (x)
#            define le64toh(x) __builtin_bswap64(x)
#        else
#            error byte order not supported
#        endif
#    endif

#elif defined(__WINDOWS__)

#    if defined(_MSC_VER) && !defined(__clang__)
#        include <stdlib.h>
#        define B_SWAP_16(x) _byteswap_ushort(x)
#        define B_SWAP_32(x) _byteswap_ulong(x)
#        define B_SWAP_64(x) _byteswap_uint64(x)
#    else
#        define B_SWAP_16(x) __builtin_bswap16(x)
#        define B_SWAP_32(x) __builtin_bswap32(x)
#        define B_SWAP_64(x) __builtin_bswap64(x)
#    endif

# if defined(__MINGW32__) || defined(HAVE_SYS_PARAM_H)
#   include <sys/param.h>
# endif

#    ifndef BIG_ENDIAN
#        ifdef __BIG_ENDIAN
#            define BIG_ENDIAN __BIG_ENDIAN
#        elif defined(__ORDER_BIG_ENDIAN__)
#            define BIG_ENDIAN __ORDER_BIG_ENDIAN__
#        else
#            define BIG_ENDIAN 4321
#        endif
#    endif

#    ifndef LITTLE_ENDIAN
#        ifdef __LITTLE_ENDIAN
#            define LITTLE_ENDIAN __LITTLE_ENDIAN
#        elif defined(__ORDER_LITTLE_ENDIAN__)
#            define LITTLE_ENDIAN __ORDER_LITTLE_ENDIAN__
#        else
#            define LITTLE_ENDIAN 1234
#        endif
#    endif

#    ifndef BYTE_ORDER
#        ifdef __BYTE_ORDER
#            define BYTE_ORDER __BYTE_ORDER
#        elif defined(__BYTE_ORDER__)
#            define BYTE_ORDER __BYTE_ORDER__
#        else
             /* assume LE on Windows if nothing was defined */
#            define BYTE_ORDER LITTLE_ENDIAN
#        endif
#    endif

#    if BYTE_ORDER == LITTLE_ENDIAN

#        define htobe16(x) B_SWAP_16(x)
#        define htole16(x) (x)
#        define be16toh(x) B_SWAP_16(x)
#        define le16toh(x) (x)

#        define htobe32(x) B_SWAP_32(x)
#        define htole32(x) (x)
#        define be32toh(x) B_SWAP_32(x)
#        define le32toh(x) (x)

#        define htobe64(x) B_SWAP_64(x)
#        define htole64(x) (x)
#        define be64toh(x) B_SWAP_64(x)
#        define le64toh(x) (x)

#    elif BYTE_ORDER == BIG_ENDIAN

#        define htobe16(x) (x)
#        define htole16(x) B_SWAP_16(x)
#        define be16toh(x) (x)
#        define le16toh(x) B_SWAP_16(x)

#        define htobe32(x) (x)
#        define htole32(x) B_SWAP_32(x)
#        define be32toh(x) (x)
#        define le32toh(x) B_SWAP_32(x)

#        define htobe64(x) (x)
#        define htole64(x) B_SWAP_64(x)
#        define be64toh(x) (x)
#        define le64toh(x) B_SWAP_64(x)

#    else

#        error byte order not supported

#    endif

#elif defined(__QNXNTO__)

#    include <gulliver.h>

#    define __LITTLE_ENDIAN 1234
#    define __BIG_ENDIAN    4321
#    define __PDP_ENDIAN    3412

#    if defined(__BIGENDIAN__)

#        define __BYTE_ORDER __BIG_ENDIAN

#        define htobe16(x) (x)
#        define htobe32(x) (x)
#        define htobe64(x) (x)

#        define htole16(x) ENDIAN_SWAP16(x)
#        define htole32(x) ENDIAN_SWAP32(x)
#        define htole64(x) ENDIAN_SWAP64(x)

#    elif defined(__LITTLEENDIAN__)

#        define __BYTE_ORDER __LITTLE_ENDIAN

#        define htole16(x) (x)
#        define htole32(x) (x)
#        define htole64(x) (x)

#        define htobe16(x) ENDIAN_SWAP16(x)
#        define htobe32(x) ENDIAN_SWAP32(x)
#        define htobe64(x) ENDIAN_SWAP64(x)

#    else

#        error byte order not supported

#    endif

#    define be16toh(x) ENDIAN_BE16(x)
#    define be32toh(x) ENDIAN_BE32(x)
#    define be64toh(x) ENDIAN_BE64(x)
#    define le16toh(x) ENDIAN_LE16(x)
#    define le32toh(x) ENDIAN_LE32(x)
#    define le64toh(x) ENDIAN_LE64(x)

#else

#    error platform not supported

#endif

#endif
