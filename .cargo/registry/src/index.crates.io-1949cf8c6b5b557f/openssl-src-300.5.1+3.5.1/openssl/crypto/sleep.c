/*
 * Copyright 2022-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/crypto.h>
#include "internal/e_os.h"

/* system-specific variants defining OSSL_sleep() */
#if defined(OPENSSL_SYS_UNIX) || defined(__DJGPP__)

# if defined(OPENSSL_USE_USLEEP)                        \
    || defined(__DJGPP__)                               \
    || (defined(__TANDEM) && defined(_REENTRANT))

/*
 * usleep() was made obsolete by POSIX.1-2008, and nanosleep()
 * should be used instead.  However, nanosleep() isn't implemented
 * on the platforms given above, so we still use it for those.
 * Also, OPENSSL_USE_USLEEP can be defined to enable the use of
 * usleep, if it turns out that nanosleep() is unavailable.
 */

#  include <unistd.h>
void OSSL_sleep(uint64_t millis)
{
    unsigned int s = (unsigned int)(millis / 1000);
    unsigned int us = (unsigned int)((millis % 1000) * 1000);

    if (s > 0)
        sleep(s);
    /*
     * On NonStop with the PUT thread model, thread context switch is
     * cooperative, with usleep() being a "natural" context switch point.
     * We avoid checking us > 0 here, to allow that context switch to
     * happen.
     */
    usleep(us);
}

# elif defined(__TANDEM) && !defined(_REENTRANT)

#  include <cextdecs.h(PROCESS_DELAY_)>
void OSSL_sleep(uint64_t millis)
{
    /* HPNS does not support usleep for non threaded apps */
    PROCESS_DELAY_(millis * 1000);
}

# else

/* nanosleep is defined by POSIX.1-2001 */
#  include <time.h>
void OSSL_sleep(uint64_t millis)
{
    struct timespec ts;

    ts.tv_sec = (long int) (millis / 1000);
    ts.tv_nsec = (long int) (millis % 1000) * 1000000ul;
    nanosleep(&ts, NULL);
}

# endif
#elif defined(_WIN32) && !defined(OPENSSL_SYS_UEFI)
# include <windows.h>

void OSSL_sleep(uint64_t millis)
{
    /*
     * Windows' Sleep() takes a DWORD argument, which is smaller than
     * a uint64_t, so we need to limit it to 49 days, which should be enough.
     */
    DWORD limited_millis = (DWORD)-1;

    if (millis < limited_millis)
        limited_millis = (DWORD)millis;
    Sleep(limited_millis);
}

#else
/* Fallback to a busy wait */
# include "internal/time.h"

static void ossl_sleep_secs(uint64_t secs)
{
    /*
     * sleep() takes an unsigned int argument, which is smaller than
     * a uint64_t, so it needs to be limited to 136 years which
     * should be enough even for Sleeping Beauty.
     */
    unsigned int limited_secs = UINT_MAX;

    if (secs < limited_secs)
        limited_secs = (unsigned int)secs;
    sleep(limited_secs);
}

static void ossl_sleep_millis(uint64_t millis)
{
    const OSSL_TIME finish
        = ossl_time_add(ossl_time_now(), ossl_ms2time(millis));

    while (ossl_time_compare(ossl_time_now(), finish) < 0)
        /* busy wait */ ;
}

void OSSL_sleep(uint64_t millis)
{
    ossl_sleep_secs(millis / 1000);
    ossl_sleep_millis(millis % 1000);
}
#endif /* defined(OPENSSL_SYS_UNIX) || defined(__DJGPP__) */
