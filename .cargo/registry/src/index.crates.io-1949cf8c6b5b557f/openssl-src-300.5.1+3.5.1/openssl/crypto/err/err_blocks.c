/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#define OSSL_FORCE_ERR_STATE

#include <string.h>
#include <openssl/err.h>
#include "err_local.h"

void ERR_new(void)
{
    ERR_STATE *es;

    es = ossl_err_get_state_int();
    if (es == NULL)
        return;

    /* Allocate a slot */
    err_get_slot(es);
    err_clear(es, es->top, 0);
}

void ERR_set_debug(const char *file, int line, const char *func)
{
    ERR_STATE *es;

    es = ossl_err_get_state_int();
    if (es == NULL)
        return;

    err_set_debug(es, es->top, file, line, func);
}

void ERR_set_error(int lib, int reason, const char *fmt, ...)
{
    va_list args;

    va_start(args, fmt);
    ERR_vset_error(lib, reason, fmt, args);
    va_end(args);
}

void ERR_vset_error(int lib, int reason, const char *fmt, va_list args)
{
    ERR_STATE *es;
    char *buf = NULL;
    size_t buf_size = 0;
    unsigned long flags = 0;
    size_t i;

    es = ossl_err_get_state_int();
    if (es == NULL)
        return;
    i = es->top;

    if (fmt != NULL) {
        int printed_len = 0;
        char *rbuf = NULL;

        buf = es->err_data[i];
        buf_size = es->err_data_size[i];

        /*
         * To protect the string we just grabbed from tampering by other
         * functions we may call, or to protect them from freeing a pointer
         * that may no longer be valid at that point, we clear away the
         * data pointer and the flags.  We will set them again at the end
         * of this function.
         */
        es->err_data[i] = NULL;
        es->err_data_flags[i] = 0;

        /*
         * Try to maximize the space available.  If that fails, we use what
         * we have.
         */
        if (buf_size < ERR_MAX_DATA_SIZE
            && (rbuf = OPENSSL_realloc(buf, ERR_MAX_DATA_SIZE)) != NULL) {
            buf = rbuf;
            buf_size = ERR_MAX_DATA_SIZE;
        }

        if (buf != NULL) {
            printed_len = BIO_vsnprintf(buf, buf_size, fmt, args);
        }
        if (printed_len < 0)
            printed_len = 0;
        if (buf != NULL)
            buf[printed_len] = '\0';

        /*
         * Try to reduce the size, but only if we maximized above.  If that
         * fails, we keep what we have.
         * (According to documentation, realloc leaves the old buffer untouched
         * if it fails)
         */
        if ((rbuf = OPENSSL_realloc(buf, printed_len + 1)) != NULL) {
            buf = rbuf;
            buf_size = printed_len + 1;
            buf[printed_len] = '\0';
        }

        if (buf != NULL)
            flags = ERR_TXT_MALLOCED | ERR_TXT_STRING;
    }

    err_clear_data(es, es->top, 0);
    err_set_error(es, es->top, lib, reason);
    if (fmt != NULL)
        err_set_data(es, es->top, buf, buf_size, flags);
}
