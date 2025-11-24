/*
 * Copyright 2001-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/crypto.h>
#include <openssl/opensslconf.h>

#if defined(OPENSSL_SYS_WIN32) || defined(OPENSSL_SYS_VXWORKS) || defined(OPENSSL_SYS_UEFI) || defined(__wasi__)

int OPENSSL_issetugid(void)
{
    return 0;
}

#elif defined(__OpenBSD__) || (defined(__FreeBSD__) && __FreeBSD__ > 2) || defined(__DragonFly__) || (defined(__GLIBC__) && defined(__FreeBSD_kernel__))

# include <unistd.h>

int OPENSSL_issetugid(void)
{
    return issetugid();
}

#else

# include <unistd.h>
# include <sys/types.h>

# if defined(__GLIBC__) && defined(__GLIBC_PREREQ)
#  if __GLIBC_PREREQ(2, 16)
#   include <sys/auxv.h>
#   define OSSL_IMPLEMENT_GETAUXVAL
#  endif
# elif defined(__ANDROID_API__)
/* see https://developer.android.google.cn/ndk/guides/cpu-features */
#  if __ANDROID_API__ >= 18
#   include <sys/auxv.h>
#   define OSSL_IMPLEMENT_GETAUXVAL
#  endif
# endif

int OPENSSL_issetugid(void)
{
# ifdef OSSL_IMPLEMENT_GETAUXVAL
    return getauxval(AT_SECURE) != 0;
# else
    return getuid() != geteuid() || getgid() != getegid();
# endif
}
#endif
