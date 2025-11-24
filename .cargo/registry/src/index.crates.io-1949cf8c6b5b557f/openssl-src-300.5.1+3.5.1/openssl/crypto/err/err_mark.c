/*
 * Copyright 2003-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#define OSSL_FORCE_ERR_STATE

#include <openssl/err.h>
#include "err_local.h"

int ERR_set_mark(void)
{
    ERR_STATE *es;

    es = ossl_err_get_state_int();
    if (es == NULL)
        return 0;

    if (es->bottom == es->top)
        return 0;
    es->err_marks[es->top]++;
    return 1;
}

int ERR_pop(void)
{
    ERR_STATE *es;

    es = ossl_err_get_state_int();
    if (es == NULL || es->bottom == es->top)
        return 0;

    err_clear(es, es->top, 0);
    es->top = es->top > 0 ? es->top - 1 : ERR_NUM_ERRORS - 1;
    return 1;
}

int ERR_pop_to_mark(void)
{
    ERR_STATE *es;

    es = ossl_err_get_state_int();
    if (es == NULL)
        return 0;

    while (es->bottom != es->top
           && es->err_marks[es->top] == 0) {
        err_clear(es, es->top, 0);
        es->top = es->top > 0 ? es->top - 1 : ERR_NUM_ERRORS - 1;
    }

    if (es->bottom == es->top)
        return 0;
    es->err_marks[es->top]--;
    return 1;
}

int ERR_count_to_mark(void)
{
    ERR_STATE *es;
    int count = 0, top;

    es = ossl_err_get_state_int();
    if (es == NULL)
        return 0;

    top = es->top;
    while (es->bottom != top
           && es->err_marks[top] == 0) {
        ++count;
        top = top > 0 ? top - 1 : ERR_NUM_ERRORS - 1;
    }

    return count;
}

int ERR_clear_last_mark(void)
{
    ERR_STATE *es;
    int top;

    es = ossl_err_get_state_int();
    if (es == NULL)
        return 0;

    top = es->top;
    while (es->bottom != top
           && es->err_marks[top] == 0) {
        top = top > 0 ? top - 1 : ERR_NUM_ERRORS - 1;
    }

    if (es->bottom == top)
        return 0;
    es->err_marks[top]--;
    return 1;
}

