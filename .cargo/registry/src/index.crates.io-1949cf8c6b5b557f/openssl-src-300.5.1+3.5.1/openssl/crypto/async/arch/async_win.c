/*
 * Copyright 2015-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* This must be the first #include file */
#include "../async_local.h"

#ifdef ASYNC_WIN

# include <windows.h>
# include "internal/cryptlib.h"

int ASYNC_is_capable(void)
{
    return 1;
}

int ASYNC_set_mem_functions(ASYNC_stack_alloc_fn alloc_fn,
                            ASYNC_stack_free_fn free_fn)
{
    return 0;
}

void ASYNC_get_mem_functions(ASYNC_stack_alloc_fn *alloc_fn,
                             ASYNC_stack_free_fn *free_fn)
{
    if (alloc_fn != NULL)
        *alloc_fn = NULL;
    if (free_fn != NULL)
        *free_fn = NULL;
}

void async_local_cleanup(void)
{
    async_ctx *ctx = async_get_ctx();
    if (ctx != NULL) {
        async_fibre *fibre = &ctx->dispatcher;
        if (fibre != NULL && fibre->fibre != NULL && fibre->converted) {
            ConvertFiberToThread();
            fibre->fibre = NULL;
        }
    }
}

int async_fibre_init_dispatcher(async_fibre *fibre)
{
# if defined(_WIN32_WINNT) && _WIN32_WINNT >= 0x600
    fibre->fibre = ConvertThreadToFiberEx(NULL, FIBER_FLAG_FLOAT_SWITCH);
# else
    fibre->fibre = ConvertThreadToFiber(NULL);
# endif
    if (fibre->fibre == NULL) {
        fibre->converted = 0;
        fibre->fibre = GetCurrentFiber();
        if (fibre->fibre == NULL)
            return 0;
    } else {
        fibre->converted = 1;
    }

    return 1;
}

VOID CALLBACK async_start_func_win(PVOID unused)
{
    async_start_func();
}

#endif
