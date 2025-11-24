/*
 * Copyright 1995-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#define OSSL_FORCE_ERR_STATE

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/crypto.h>
#include <openssl/buffer.h>
#include <openssl/err.h>
#include "err_local.h"

#define ERR_PRINT_BUF_SIZE 4096
void ERR_print_errors_cb(int (*cb) (const char *str, size_t len, void *u),
                         void *u)
{
    CRYPTO_THREAD_ID tid = CRYPTO_THREAD_get_current_id();
    unsigned long l;
    const char *file, *data, *func;
    int line, flags;

    while ((l = ERR_get_error_all(&file, &line, &func, &data, &flags)) != 0) {
        char buf[ERR_PRINT_BUF_SIZE] = "";
        char *hex = NULL;
        int offset;

        if ((flags & ERR_TXT_STRING) == 0)
            data = "";

        hex = ossl_buf2hexstr_sep((const unsigned char *)&tid, sizeof(tid), '\0');
        BIO_snprintf(buf, sizeof(buf), "%s:", hex == NULL ? "<null>" : hex);
        offset = strlen(buf);
        ossl_err_string_int(l, func, buf + offset, sizeof(buf) - offset);
        offset += strlen(buf + offset);
        BIO_snprintf(buf + offset, sizeof(buf) - offset, ":%s:%d:%s\n",
                     file, line, data);
        OPENSSL_free(hex);
        if (cb(buf, strlen(buf), u) <= 0)
            break;              /* abort outputting the error report */
    }
}

/* auxiliary function for incrementally reporting texts via the error queue */
static void put_error(int lib, const char *func, int reason,
                      const char *file, int line)
{
    ERR_new();
    ERR_set_debug(file, line, func);
    ERR_set_error(lib, reason, NULL /* no data here, so fmt is NULL */);
}

#define TYPICAL_MAX_OUTPUT_BEFORE_DATA 100
#define MAX_DATA_LEN (ERR_PRINT_BUF_SIZE - TYPICAL_MAX_OUTPUT_BEFORE_DATA)
void ERR_add_error_txt(const char *separator, const char *txt)
{
    const char *file = NULL;
    int line;
    const char *func = NULL;
    const char *data = NULL;
    int flags;
    unsigned long err = ERR_peek_last_error();

    if (separator == NULL)
        separator = "";
    if (err == 0)
        put_error(ERR_LIB_NONE, NULL, 0, "", 0);

    do {
        size_t available_len, data_len;
        const char *curr = txt, *next = txt;
        const char *leading_separator = separator;
        int trailing_separator = 0;
        char *tmp;

        ERR_peek_last_error_all(&file, &line, &func, &data, &flags);
        if ((flags & ERR_TXT_STRING) == 0) {
            data = "";
            leading_separator = "";
        }
        data_len = strlen(data);

        /* workaround for limit of ERR_print_errors_cb() */
        if (data_len >= MAX_DATA_LEN
                || strlen(separator) >= (size_t)(MAX_DATA_LEN - data_len))
            available_len = 0;
        else
            available_len = MAX_DATA_LEN - data_len - strlen(separator) - 1;
        /* MAX_DATA_LEN > available_len >= 0 */

        if (*separator == '\0') {
            const size_t len_next = strlen(next);

            if (len_next <= available_len) {
                next += len_next;
                curr = NULL; /* no need to split */
            } else {
                next += available_len;
                curr = next; /* will split at this point */
            }
        } else {
            while (*next != '\0' && (size_t)(next - txt) <= available_len) {
                curr = next;
                next = strstr(curr, separator);
                if (next != NULL) {
                    next += strlen(separator);
                    trailing_separator = *next == '\0';
                } else {
                    next = curr + strlen(curr);
                }
            }
            if ((size_t)(next - txt) <= available_len)
                curr = NULL; /* the above loop implies *next == '\0' */
        }
        if (curr != NULL) {
            /* split error msg at curr since error data would get too long */
            if (curr != txt) {
                tmp = OPENSSL_strndup(txt, curr - txt);
                if (tmp == NULL)
                    return;
                ERR_add_error_data(2, separator, tmp);
                OPENSSL_free(tmp);
            }
            put_error(ERR_GET_LIB(err), func, err, file, line);
            txt = curr;
        } else {
            if (trailing_separator) {
                tmp = OPENSSL_strndup(txt, next - strlen(separator) - txt);
                if (tmp == NULL)
                    return;
                /* output txt without the trailing separator */
                ERR_add_error_data(2, leading_separator, tmp);
                OPENSSL_free(tmp);
            } else {
                ERR_add_error_data(2, leading_separator, txt);
            }
            txt = next; /* finished */
        }
    } while (*txt != '\0');
}

void ERR_add_error_mem_bio(const char *separator, BIO *bio)
{
    if (bio != NULL) {
        char *str;
        long len = BIO_get_mem_data(bio, &str);

        if (len > 0) {
            if (str[len - 1] != '\0') {
                if (BIO_write(bio, "", 1) <= 0)
                    return;

                len = BIO_get_mem_data(bio, &str);
            }
            if (len > 1)
                ERR_add_error_txt(separator, str);
        }
    }
}

static int print_bio(const char *str, size_t len, void *bp)
{
    return BIO_write((BIO *)bp, str, len);
}

void ERR_print_errors(BIO *bp)
{
    ERR_print_errors_cb(print_bio, bp);
}

#ifndef OPENSSL_NO_STDIO
void ERR_print_errors_fp(FILE *fp)
{
    BIO *bio = BIO_new_fp(fp, BIO_NOCLOSE);
    if (bio == NULL)
        return;

    ERR_print_errors_cb(print_bio, bio);
    BIO_free(bio);
}
#endif
