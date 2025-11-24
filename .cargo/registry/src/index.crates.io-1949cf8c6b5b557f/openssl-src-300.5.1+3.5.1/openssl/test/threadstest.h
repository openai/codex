/*
 * Copyright 2021-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#if defined(_WIN32)
# include <windows.h>
#endif

#include <string.h>
#include "testutil.h"

#if !defined(OPENSSL_THREADS) || defined(CRYPTO_TDEBUG)

typedef unsigned int thread_t;

static int run_thread(thread_t *t, void (*f)(void))
{
    f();
    return 1;
}

static int wait_for_thread(thread_t thread)
{
    return 1;
}

#elif defined(OPENSSL_SYS_WINDOWS)

typedef HANDLE thread_t;

static DWORD WINAPI thread_run(LPVOID arg)
{
    void (*f)(void);

    *(void **) (&f) = arg;

    f();
    return 0;
}

static int run_thread(thread_t *t, void (*f)(void))
{
    *t = CreateThread(NULL, 0, thread_run, *(void **) &f, 0, NULL);
    return *t != NULL;
}

static int wait_for_thread(thread_t thread)
{
    return WaitForSingleObject(thread, INFINITE) == 0;
}

#else

typedef pthread_t thread_t;

static void *thread_run(void *arg)
{
    void (*f)(void);

    *(void **) (&f) = arg;

    f();
    OPENSSL_thread_stop();
    return NULL;
}

static int run_thread(thread_t *t, void (*f)(void))
{
    return pthread_create(t, NULL, thread_run, *(void **) &f) == 0;
}

static int wait_for_thread(thread_t thread)
{
    return pthread_join(thread, NULL) == 0;
}

#endif

