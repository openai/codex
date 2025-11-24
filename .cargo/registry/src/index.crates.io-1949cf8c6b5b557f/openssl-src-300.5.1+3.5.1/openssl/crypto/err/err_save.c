/*
 * Copyright 2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#define OSSL_FORCE_ERR_STATE

#include <openssl/err.h>
#include "err_local.h"

/*
 * Save and restore error state.
 * We are using CRYPTO_zalloc(.., NULL, 0) instead of OPENSSL_malloc() in
 * these functions to prevent mem alloc error loop.
 */

ERR_STATE *OSSL_ERR_STATE_new(void)
{
    return CRYPTO_zalloc(sizeof(ERR_STATE), NULL, 0);
}

void OSSL_ERR_STATE_save(ERR_STATE *es)
{
    size_t i;
    ERR_STATE *thread_es;

    if (es == NULL)
        return;

    for (i = 0; i < ERR_NUM_ERRORS; i++)
        err_clear(es, i, 1);

    thread_es = ossl_err_get_state_int();
    if (thread_es == NULL)
        return;

    memcpy(es, thread_es, sizeof(*es));
    /* Taking over the pointers, just clear the thread state. */
    memset(thread_es, 0, sizeof(*thread_es));
}

void OSSL_ERR_STATE_save_to_mark(ERR_STATE *es)
{
    size_t i, j, count;
    int top;
    ERR_STATE *thread_es;

    if (es == NULL)
        return;

    thread_es = ossl_err_get_state_int();
    if (thread_es == NULL) {
        for (i = 0; i < ERR_NUM_ERRORS; ++i)
            err_clear(es, i, 1);

        es->top = es->bottom = 0;
        return;
    }

    /* Determine number of errors we are going to move. */
    for (count = 0, top = thread_es->top;
         thread_es->bottom != top
         && thread_es->err_marks[top] == 0;
         ++count)
        top = top > 0 ? top - 1 : ERR_NUM_ERRORS - 1;

    /* Move the errors, preserving order. */
    for (i = 0, j = top; i < count; ++i) {
        j = (j + 1) % ERR_NUM_ERRORS;

        err_clear(es, i, 1);

        /* Move the error entry to the given ERR_STATE. */
        es->err_flags[i]        = thread_es->err_flags[j];
        es->err_marks[i]        = 0;
        es->err_buffer[i]       = thread_es->err_buffer[j];
        es->err_data[i]         = thread_es->err_data[j];
        es->err_data_size[i]    = thread_es->err_data_size[j];
        es->err_data_flags[i]   = thread_es->err_data_flags[j];
        es->err_file[i]         = thread_es->err_file[j];
        es->err_line[i]         = thread_es->err_line[j];
        es->err_func[i]         = thread_es->err_func[j];

        thread_es->err_flags[j]      = 0;
        thread_es->err_buffer[j]     = 0;
        thread_es->err_data[j]       = NULL;
        thread_es->err_data_size[j]  = 0;
        thread_es->err_data_flags[j] = 0;
        thread_es->err_file[j]       = NULL;
        thread_es->err_line[j]       = 0;
        thread_es->err_func[j]       = NULL;
    }

    if (i > 0) {
        thread_es->top = top;
        /* If we moved anything, es's stack always starts at [0]. */
        es->top     = i - 1;
        es->bottom  = ERR_NUM_ERRORS - 1;
    } else {
        /* Didn't move anything - empty stack */
        es->top = es->bottom = 0;
    }

    /* Erase extra space as a precaution. */
    for (; i < ERR_NUM_ERRORS; ++i)
        err_clear(es, i, 1);
}

void OSSL_ERR_STATE_restore(const ERR_STATE *es)
{
    size_t i;
    ERR_STATE *thread_es;

    if (es == NULL || es->bottom == es->top)
        return;

    thread_es = ossl_err_get_state_int();
    if (thread_es == NULL)
        return;

    for (i = (size_t)es->bottom; i != (size_t)es->top;) {
        size_t top;

        i = (i + 1) % ERR_NUM_ERRORS;
        if ((es->err_flags[i] & ERR_FLAG_CLEAR) != 0)
            continue;

        err_get_slot(thread_es);
        top = thread_es->top;
        err_clear(thread_es, top, 0);

        thread_es->err_flags[top] = es->err_flags[i];
        thread_es->err_buffer[top] = es->err_buffer[i];

        err_set_debug(thread_es, top, es->err_file[i], es->err_line[i],
                      es->err_func[i]);

        if (es->err_data[i] != NULL && es->err_data_size[i] != 0) {
            void *data;
            size_t data_sz = es->err_data_size[i];

            data = CRYPTO_malloc(data_sz, NULL, 0);
            if (data != NULL) {
                memcpy(data, es->err_data[i], data_sz);
                err_set_data(thread_es, top, data, data_sz,
                             es->err_data_flags[i] | ERR_TXT_MALLOCED);
            }
        } else {
            err_clear_data(thread_es, top, 0);
        }
    }
}
