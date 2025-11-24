/*
 * Copyright 1995-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#define OPENSSL_SUPPRESS_DEPRECATED

#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include "bio_local.h"
#include "internal/cryptlib.h"
#include <openssl/err.h>

long BIO_debug_callback_ex(BIO *bio, int cmd, const char *argp, size_t len,
                           int argi, long argl, int ret, size_t *processed)
{
    BIO *b;
    char buf[256];
    char *p;
    int left;
    size_t l = 0;
    BIO_MMSG_CB_ARGS *args;
    long ret_ = ret;

    if (processed != NULL)
        l = *processed;

    left = BIO_snprintf(buf, sizeof(buf), "BIO[%p]: ", (void *)bio);

    /* Ignore errors and continue printing the other information. */
    if (left < 0)
        left = 0;
    p = buf + left;
    left = sizeof(buf) - left;

    switch (cmd) {
    case BIO_CB_FREE:
        BIO_snprintf(p, left, "Free - %s\n", bio->method->name);
        break;
    case BIO_CB_READ:
        if (bio->method->type & BIO_TYPE_DESCRIPTOR)
            BIO_snprintf(p, left, "read(%d,%zu) - %s fd=%d\n",
                         bio->num, len,
                         bio->method->name, bio->num);
        else
            BIO_snprintf(p, left, "read(%d,%zu) - %s\n",
                    bio->num, len, bio->method->name);
        break;
    case BIO_CB_WRITE:
        if (bio->method->type & BIO_TYPE_DESCRIPTOR)
            BIO_snprintf(p, left, "write(%d,%zu) - %s fd=%d\n",
                         bio->num, len,
                         bio->method->name, bio->num);
        else
            BIO_snprintf(p, left, "write(%d,%zu) - %s\n",
                         bio->num, len, bio->method->name);
        break;
    case BIO_CB_PUTS:
        BIO_snprintf(p, left, "puts() - %s\n", bio->method->name);
        break;
    case BIO_CB_GETS:
        BIO_snprintf(p, left, "gets(%zu) - %s\n", len,
                     bio->method->name);
        break;
    case BIO_CB_CTRL:
        BIO_snprintf(p, left, "ctrl(%d) - %s\n", argi,
                     bio->method->name);
        break;
    case BIO_CB_RECVMMSG:
        args = (BIO_MMSG_CB_ARGS *)argp;
        BIO_snprintf(p, left, "recvmmsg(%zu) - %s",
                     args->num_msg, bio->method->name);
        break;
    case BIO_CB_SENDMMSG:
        args = (BIO_MMSG_CB_ARGS *)argp;
        BIO_snprintf(p, left, "sendmmsg(%zu) - %s",
                     args->num_msg, bio->method->name);
        break;
    case BIO_CB_RETURN | BIO_CB_READ:
        BIO_snprintf(p, left, "read return %d processed: %zu\n", ret, l);
        break;
    case BIO_CB_RETURN | BIO_CB_WRITE:
        BIO_snprintf(p, left, "write return %d processed: %zu\n", ret, l);
        break;
    case BIO_CB_RETURN | BIO_CB_GETS:
        BIO_snprintf(p, left, "gets return %d processed: %zu\n", ret, l);
        break;
    case BIO_CB_RETURN | BIO_CB_PUTS:
        BIO_snprintf(p, left, "puts return %d processed: %zu\n", ret, l);
        break;
    case BIO_CB_RETURN | BIO_CB_CTRL:
        BIO_snprintf(p, left, "ctrl return %d\n", ret);
        break;
    case BIO_CB_RETURN | BIO_CB_RECVMMSG:
        BIO_snprintf(p, left, "recvmmsg processed: %zu\n", len);
        ret_ = (long)len;
        break;
    case BIO_CB_RETURN | BIO_CB_SENDMMSG:
        BIO_snprintf(p, left, "sendmmsg processed: %zu\n", len);
        ret_ = (long)len;
        break;
    default:
        BIO_snprintf(p, left, "bio callback - unknown type (%d)\n", cmd);
        break;
    }

    b = (BIO *)bio->cb_arg;
    if (b != NULL)
        BIO_write(b, buf, strlen(buf));
#if !defined(OPENSSL_NO_STDIO)
    else
        fputs(buf, stderr);
#endif
    return ret_;
}

#ifndef OPENSSL_NO_DEPRECATED_3_0
long BIO_debug_callback(BIO *bio, int cmd, const char *argp,
                        int argi, long argl, long ret)
{
    size_t processed = 0;

    if (ret > 0)
        processed = (size_t)ret;
    BIO_debug_callback_ex(bio, cmd, argp, (size_t)argi,
                          argi, argl, ret > 0 ? 1 : (int)ret, &processed);
    return ret;
}
#endif
