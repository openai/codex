/*
 * Copyright 2015-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * This is the same detection used in cryptlib to set up the thread local
 * storage that we depend on, so just copy that
 */
#if defined(_WIN32) && !defined(OPENSSL_NO_ASYNC)
#include <openssl/async.h>
# define ASYNC_WIN
# define ASYNC_ARCH

# include <windows.h>
# include "internal/cryptlib.h"

typedef struct async_fibre_st {
    LPVOID fibre;
    int converted;
} async_fibre;

# define async_fibre_swapcontext(o,n,r) \
        (SwitchToFiber((n)->fibre), 1)

# if defined(_WIN32_WINNT) && _WIN32_WINNT >= 0x600
#   define async_fibre_makecontext(c) \
        ((c)->fibre = CreateFiberEx(0, 0, FIBER_FLAG_FLOAT_SWITCH, \
                                    async_start_func_win, 0))
# else
#   define async_fibre_makecontext(c) \
        ((c)->fibre = CreateFiber(0, async_start_func_win, 0))
# endif

# define async_fibre_free(f)             (DeleteFiber((f)->fibre))
# define async_local_init()              1
# define async_local_deinit()

int async_fibre_init_dispatcher(async_fibre *fibre);
VOID CALLBACK async_start_func_win(PVOID unused);

#endif
