/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#define OPENSSL_SUPPRESS_DEPRECATED

#include <stdio.h>
#include <errno.h>
#include <openssl/crypto.h>
#include "internal/numbers.h"
#include "bio_local.h"

/*
 * Helper macro for the callback to determine whether an operator expects a
 * len parameter or not
 */
#define HAS_LEN_OPER(o) ((o) == BIO_CB_READ || (o) == BIO_CB_WRITE \
                         || (o) == BIO_CB_GETS)

#ifndef OPENSSL_NO_DEPRECATED_3_0
# define HAS_CALLBACK(b) ((b)->callback != NULL || (b)->callback_ex != NULL)
#else
# define HAS_CALLBACK(b) ((b)->callback_ex != NULL)
#endif
/*
 * Helper function to work out whether to call the new style callback or the old
 * one, and translate between the two.
 *
 * This has a long return type for consistency with the old callback. Similarly
 * for the "long" used for "inret"
 */
static long bio_call_callback(BIO *b, int oper, const char *argp, size_t len,
                              int argi, long argl, long inret,
                              size_t *processed)
{
    long ret = inret;
#ifndef OPENSSL_NO_DEPRECATED_3_0
    int bareoper;

    if (b->callback_ex != NULL)
#endif
        return b->callback_ex(b, oper, argp, len, argi, argl, inret, processed);

#ifndef OPENSSL_NO_DEPRECATED_3_0
    /* Strip off any BIO_CB_RETURN flag */
    bareoper = oper & ~BIO_CB_RETURN;

    /*
     * We have an old style callback, so we will have to do nasty casts and
     * check for overflows.
     */
    if (HAS_LEN_OPER(bareoper)) {
        /* In this case |len| is set, and should be used instead of |argi| */
        if (len > INT_MAX)
            return -1;

        argi = (int)len;
    }

    if (inret > 0 && (oper & BIO_CB_RETURN) && bareoper != BIO_CB_CTRL) {
        if (*processed > INT_MAX)
            return -1;
        inret = *processed;
    }

    ret = b->callback(b, oper, argp, argi, argl, inret);

    if (ret > 0 && (oper & BIO_CB_RETURN) && bareoper != BIO_CB_CTRL) {
        *processed = (size_t)ret;
        ret = 1;
    }
#endif
    return ret;
}

BIO *BIO_new_ex(OSSL_LIB_CTX *libctx, const BIO_METHOD *method)
{
    BIO *bio = OPENSSL_zalloc(sizeof(*bio));

    if (bio == NULL)
        return NULL;

    bio->libctx = libctx;
    bio->method = method;
    bio->shutdown = 1;

    if (!CRYPTO_NEW_REF(&bio->references, 1))
        goto err;

    if (!CRYPTO_new_ex_data(CRYPTO_EX_INDEX_BIO, bio, &bio->ex_data))
        goto err;

    if (method->create != NULL && !method->create(bio)) {
        ERR_raise(ERR_LIB_BIO, ERR_R_INIT_FAIL);
        CRYPTO_free_ex_data(CRYPTO_EX_INDEX_BIO, bio, &bio->ex_data);
        goto err;
    }
    if (method->create == NULL)
        bio->init = 1;

    return bio;

err:
    CRYPTO_FREE_REF(&bio->references);
    OPENSSL_free(bio);
    return NULL;
}

BIO *BIO_new(const BIO_METHOD *method)
{
    return BIO_new_ex(NULL, method);
}

int BIO_free(BIO *a)
{
    int ret;

    if (a == NULL)
        return 0;

    if (CRYPTO_DOWN_REF(&a->references, &ret) <= 0)
        return 0;

    REF_PRINT_COUNT("BIO", ret, a);
    if (ret > 0)
        return 1;
    REF_ASSERT_ISNT(ret < 0);

    if (HAS_CALLBACK(a)) {
        ret = (int)bio_call_callback(a, BIO_CB_FREE, NULL, 0, 0, 0L, 1L, NULL);
        if (ret <= 0)
            return 0;
    }

    if ((a->method != NULL) && (a->method->destroy != NULL))
        a->method->destroy(a);

    CRYPTO_free_ex_data(CRYPTO_EX_INDEX_BIO, a, &a->ex_data);

    CRYPTO_FREE_REF(&a->references);

    OPENSSL_free(a);

    return 1;
}

void BIO_set_data(BIO *a, void *ptr)
{
    a->ptr = ptr;
}

void *BIO_get_data(BIO *a)
{
    return a->ptr;
}

void BIO_set_init(BIO *a, int init)
{
    a->init = init;
}

int BIO_get_init(BIO *a)
{
    return a->init;
}

void BIO_set_shutdown(BIO *a, int shut)
{
    a->shutdown = shut;
}

int BIO_get_shutdown(BIO *a)
{
    return a->shutdown;
}

void BIO_vfree(BIO *a)
{
    BIO_free(a);
}

int BIO_up_ref(BIO *a)
{
    int i;

    if (CRYPTO_UP_REF(&a->references, &i) <= 0)
        return 0;

    REF_PRINT_COUNT("BIO", i, a);
    REF_ASSERT_ISNT(i < 2);
    return i > 1;
}

void BIO_clear_flags(BIO *b, int flags)
{
    b->flags &= ~flags;
}

int BIO_test_flags(const BIO *b, int flags)
{
    return (b->flags & flags);
}

void BIO_set_flags(BIO *b, int flags)
{
    b->flags |= flags;
}

#ifndef OPENSSL_NO_DEPRECATED_3_0
BIO_callback_fn BIO_get_callback(const BIO *b)
{
    return b->callback;
}

void BIO_set_callback(BIO *b, BIO_callback_fn cb)
{
    b->callback = cb;
}
#endif

BIO_callback_fn_ex BIO_get_callback_ex(const BIO *b)
{
    return b->callback_ex;
}

void BIO_set_callback_ex(BIO *b, BIO_callback_fn_ex cb)
{
    b->callback_ex = cb;
}

void BIO_set_callback_arg(BIO *b, char *arg)
{
    b->cb_arg = arg;
}

char *BIO_get_callback_arg(const BIO *b)
{
    return b->cb_arg;
}

const char *BIO_method_name(const BIO *b)
{
    return b->method->name;
}

int BIO_method_type(const BIO *b)
{
    return b->method->type;
}

/*
 * This is essentially the same as BIO_read_ex() except that it allows
 * 0 or a negative value to indicate failure (retryable or not) in the return.
 * This is for compatibility with the old style BIO_read(), where existing code
 * may make assumptions about the return value that it might get.
 */
static int bio_read_intern(BIO *b, void *data, size_t dlen, size_t *readbytes)
{
    int ret;

    if (b == NULL) {
        ERR_raise(ERR_LIB_BIO, ERR_R_PASSED_NULL_PARAMETER);
        return -1;
    }
    if (b->method == NULL || b->method->bread == NULL) {
        ERR_raise(ERR_LIB_BIO, BIO_R_UNSUPPORTED_METHOD);
        return -2;
    }

    if (HAS_CALLBACK(b) &&
        ((ret = (int)bio_call_callback(b, BIO_CB_READ, data, dlen, 0, 0L, 1L,
                                       NULL)) <= 0))
        return ret;

    if (!b->init) {
        ERR_raise(ERR_LIB_BIO, BIO_R_UNINITIALIZED);
        return -1;
    }

    ret = b->method->bread(b, data, dlen, readbytes);

    if (ret > 0)
        b->num_read += (uint64_t)*readbytes;

    if (HAS_CALLBACK(b))
        ret = (int)bio_call_callback(b, BIO_CB_READ | BIO_CB_RETURN, data,
                                     dlen, 0, 0L, ret, readbytes);

    /* Shouldn't happen */
    if (ret > 0 && *readbytes > dlen) {
        ERR_raise(ERR_LIB_BIO, ERR_R_INTERNAL_ERROR);
        return -1;
    }

    return ret;
}

int BIO_read(BIO *b, void *data, int dlen)
{
    size_t readbytes;
    int ret;

    if (dlen < 0)
        return 0;

    ret = bio_read_intern(b, data, (size_t)dlen, &readbytes);

    if (ret > 0) {
        /* *readbytes should always be <= dlen */
        ret = (int)readbytes;
    }

    return ret;
}

int BIO_read_ex(BIO *b, void *data, size_t dlen, size_t *readbytes)
{
    return bio_read_intern(b, data, dlen, readbytes) > 0;
}

static int bio_write_intern(BIO *b, const void *data, size_t dlen,
                            size_t *written)
{
    size_t local_written;
    int ret;

    if (written != NULL)
        *written = 0;
    /*
     * b == NULL is not an error but just means that zero bytes are written.
     * Do not raise an error here.
     */
    if (b == NULL)
        return 0;

    if (b->method == NULL || b->method->bwrite == NULL) {
        ERR_raise(ERR_LIB_BIO, BIO_R_UNSUPPORTED_METHOD);
        return -2;
    }

    if (HAS_CALLBACK(b) &&
        ((ret = (int)bio_call_callback(b, BIO_CB_WRITE, data, dlen, 0, 0L, 1L,
                                       NULL)) <= 0))
        return ret;

    if (!b->init) {
        ERR_raise(ERR_LIB_BIO, BIO_R_UNINITIALIZED);
        return -1;
    }

    ret = b->method->bwrite(b, data, dlen, &local_written);

    if (ret > 0)
        b->num_write += (uint64_t)local_written;

    if (HAS_CALLBACK(b))
        ret = (int)bio_call_callback(b, BIO_CB_WRITE | BIO_CB_RETURN, data,
                                     dlen, 0, 0L, ret, &local_written);

    if (written != NULL)
        *written = local_written;
    return ret;
}

int BIO_write(BIO *b, const void *data, int dlen)
{
    size_t written;
    int ret;

    if (dlen <= 0)
        return 0;

    ret = bio_write_intern(b, data, (size_t)dlen, &written);

    if (ret > 0) {
        /* written should always be <= dlen */
        ret = (int)written;
    }

    return ret;
}

int BIO_write_ex(BIO *b, const void *data, size_t dlen, size_t *written)
{
    return bio_write_intern(b, data, dlen, written) > 0
        || (b != NULL && dlen == 0); /* order is important for *written */
}

int BIO_sendmmsg(BIO *b, BIO_MSG *msg,
                 size_t stride, size_t num_msg, uint64_t flags,
                 size_t *msgs_processed)
{
    size_t ret;
    BIO_MMSG_CB_ARGS args;

    if (b == NULL) {
        *msgs_processed = 0;
        ERR_raise(ERR_LIB_BIO, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if (b->method == NULL || b->method->bsendmmsg == NULL) {
        *msgs_processed = 0;
        ERR_raise(ERR_LIB_BIO, BIO_R_UNSUPPORTED_METHOD);
        return 0;
    }

    if (HAS_CALLBACK(b)) {
        args.msg            = msg;
        args.stride         = stride;
        args.num_msg        = num_msg;
        args.flags          = flags;
        args.msgs_processed = msgs_processed;

        ret = (size_t)bio_call_callback(b, BIO_CB_SENDMMSG, (void *)&args,
                                        0, 0, 0, 1, NULL);
        if (ret <= 0)
            return 0;
    }

    if (!b->init) {
        *msgs_processed = 0;
        ERR_raise(ERR_LIB_BIO, BIO_R_UNINITIALIZED);
        return 0;
    }

    ret = b->method->bsendmmsg(b, msg, stride, num_msg, flags, msgs_processed);

    if (HAS_CALLBACK(b))
        ret = (size_t)bio_call_callback(b, BIO_CB_SENDMMSG | BIO_CB_RETURN,
                                        (void *)&args, ret, 0, 0, ret, NULL);

    return ret;
}

int BIO_recvmmsg(BIO *b, BIO_MSG *msg,
                 size_t stride, size_t num_msg, uint64_t flags,
                 size_t *msgs_processed)
{
    size_t ret;
    BIO_MMSG_CB_ARGS args;

    if (b == NULL) {
        *msgs_processed = 0;
        ERR_raise(ERR_LIB_BIO, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if (b->method == NULL || b->method->brecvmmsg == NULL) {
        *msgs_processed = 0;
        ERR_raise(ERR_LIB_BIO, BIO_R_UNSUPPORTED_METHOD);
        return 0;
    }

    if (HAS_CALLBACK(b)) {
        args.msg            = msg;
        args.stride         = stride;
        args.num_msg        = num_msg;
        args.flags          = flags;
        args.msgs_processed = msgs_processed;

        ret = bio_call_callback(b, BIO_CB_RECVMMSG, (void *)&args,
                                0, 0, 0, 1, NULL);
        if (ret <= 0)
            return 0;
    }

    if (!b->init) {
        *msgs_processed = 0;
        ERR_raise(ERR_LIB_BIO, BIO_R_UNINITIALIZED);
        return 0;
    }

    ret = b->method->brecvmmsg(b, msg, stride, num_msg, flags, msgs_processed);

    if (HAS_CALLBACK(b))
        ret = (size_t)bio_call_callback(b, BIO_CB_RECVMMSG | BIO_CB_RETURN,
                                        (void *)&args, ret, 0, 0, ret, NULL);

    return ret;
}

int BIO_get_rpoll_descriptor(BIO *b, BIO_POLL_DESCRIPTOR *desc)
{
    return BIO_ctrl(b, BIO_CTRL_GET_RPOLL_DESCRIPTOR, 0, desc);
}

int BIO_get_wpoll_descriptor(BIO *b, BIO_POLL_DESCRIPTOR *desc)
{
    return BIO_ctrl(b, BIO_CTRL_GET_WPOLL_DESCRIPTOR, 0, desc);
}

int BIO_puts(BIO *b, const char *buf)
{
    int ret;
    size_t written = 0;

    if (b == NULL) {
        ERR_raise(ERR_LIB_BIO, ERR_R_PASSED_NULL_PARAMETER);
        return -1;
    }
    if (b->method == NULL || b->method->bputs == NULL) {
        ERR_raise(ERR_LIB_BIO, BIO_R_UNSUPPORTED_METHOD);
        return -2;
    }

    if (HAS_CALLBACK(b)) {
        ret = (int)bio_call_callback(b, BIO_CB_PUTS, buf, 0, 0, 0L, 1L, NULL);
        if (ret <= 0)
            return ret;
    }

    if (!b->init) {
        ERR_raise(ERR_LIB_BIO, BIO_R_UNINITIALIZED);
        return -1;
    }

    ret = b->method->bputs(b, buf);

    if (ret > 0) {
        b->num_write += (uint64_t)ret;
        written = ret;
        ret = 1;
    }

    if (HAS_CALLBACK(b))
        ret = (int)bio_call_callback(b, BIO_CB_PUTS | BIO_CB_RETURN, buf, 0, 0,
                                     0L, ret, &written);

    if (ret > 0) {
        if (written > INT_MAX) {
            ERR_raise(ERR_LIB_BIO, BIO_R_LENGTH_TOO_LONG);
            ret = -1;
        } else {
            ret = (int)written;
        }
    }

    return ret;
}

int BIO_gets(BIO *b, char *buf, int size)
{
    int ret;
    size_t readbytes = 0;

    if (b == NULL) {
        ERR_raise(ERR_LIB_BIO, ERR_R_PASSED_NULL_PARAMETER);
        return -1;
    }
    if (b->method == NULL || b->method->bgets == NULL) {
        ERR_raise(ERR_LIB_BIO, BIO_R_UNSUPPORTED_METHOD);
        return -2;
    }

    if (size < 0) {
        ERR_raise(ERR_LIB_BIO, BIO_R_INVALID_ARGUMENT);
        return -1;
    }

    if (HAS_CALLBACK(b)) {
        ret = (int)bio_call_callback(b, BIO_CB_GETS, buf, size, 0, 0L, 1, NULL);
        if (ret <= 0)
            return ret;
    }

    if (!b->init) {
        ERR_raise(ERR_LIB_BIO, BIO_R_UNINITIALIZED);
        return -1;
    }

    ret = b->method->bgets(b, buf, size);

    if (ret > 0) {
        readbytes = ret;
        ret = 1;
    }

    if (HAS_CALLBACK(b))
        ret = (int)bio_call_callback(b, BIO_CB_GETS | BIO_CB_RETURN, buf, size,
                                     0, 0L, ret, &readbytes);

    if (ret > 0) {
        /* Shouldn't happen */
        if (readbytes > (size_t)size)
            ret = -1;
        else
            ret = (int)readbytes;
    }

    return ret;
}

int BIO_get_line(BIO *bio, char *buf, int size)
{
    int ret = 0;
    char *ptr = buf;

    if (buf == NULL) {
        ERR_raise(ERR_LIB_BIO, ERR_R_PASSED_NULL_PARAMETER);
        return -1;
    }
    if (size <= 0) {
        ERR_raise(ERR_LIB_BIO, BIO_R_INVALID_ARGUMENT);
        return -1;
    }
    *buf = '\0';

    if (bio == NULL) {
        ERR_raise(ERR_LIB_BIO, ERR_R_PASSED_NULL_PARAMETER);
        return -1;
    }
    if (!bio->init) {
        ERR_raise(ERR_LIB_BIO, BIO_R_UNINITIALIZED);
        return -1;
    }

    while (size-- > 1 && (ret = BIO_read(bio, ptr, 1)) > 0)
        if (*ptr++ == '\n')
            break;
    *ptr = '\0';
    return ret > 0 || BIO_eof(bio) ? ptr - buf : ret;
}

int BIO_indent(BIO *b, int indent, int max)
{
    if (indent < 0)
        indent = 0;
    if (indent > max)
        indent = max;
    while (indent--)
        if (BIO_puts(b, " ") != 1)
            return 0;
    return 1;
}

long BIO_int_ctrl(BIO *b, int cmd, long larg, int iarg)
{
    int i;

    i = iarg;
    return BIO_ctrl(b, cmd, larg, (char *)&i);
}

void *BIO_ptr_ctrl(BIO *b, int cmd, long larg)
{
    void *p = NULL;

    if (BIO_ctrl(b, cmd, larg, (char *)&p) <= 0)
        return NULL;
    else
        return p;
}

long BIO_ctrl(BIO *b, int cmd, long larg, void *parg)
{
    long ret;

    if (b == NULL)
        return -1;
    if (b->method == NULL || b->method->ctrl == NULL) {
        ERR_raise(ERR_LIB_BIO, BIO_R_UNSUPPORTED_METHOD);
        return -2;
    }

    if (HAS_CALLBACK(b)) {
        ret = bio_call_callback(b, BIO_CB_CTRL, parg, 0, cmd, larg, 1L, NULL);
        if (ret <= 0)
            return ret;
    }

    ret = b->method->ctrl(b, cmd, larg, parg);

    if (HAS_CALLBACK(b))
        ret = bio_call_callback(b, BIO_CB_CTRL | BIO_CB_RETURN, parg, 0, cmd,
                                larg, ret, NULL);

    return ret;
}

long BIO_callback_ctrl(BIO *b, int cmd, BIO_info_cb *fp)
{
    long ret;

    if (b == NULL)
        return -2;
    if (b->method == NULL || b->method->callback_ctrl == NULL
            || cmd != BIO_CTRL_SET_CALLBACK) {
        ERR_raise(ERR_LIB_BIO, BIO_R_UNSUPPORTED_METHOD);
        return -2;
    }

    if (HAS_CALLBACK(b)) {
        ret = bio_call_callback(b, BIO_CB_CTRL, (void *)&fp, 0, cmd, 0, 1L,
                                NULL);
        if (ret <= 0)
            return ret;
    }

    ret = b->method->callback_ctrl(b, cmd, fp);

    if (HAS_CALLBACK(b))
        ret = bio_call_callback(b, BIO_CB_CTRL | BIO_CB_RETURN, (void *)&fp, 0,
                                cmd, 0, ret, NULL);

    return ret;
}

/*
 * It is unfortunate to duplicate in functions what the BIO_(w)pending macros
 * do; but those macros have inappropriate return type, and for interfacing
 * from other programming languages, C macros aren't much of a help anyway.
 */
size_t BIO_ctrl_pending(BIO *bio)
{
    long ret = BIO_ctrl(bio, BIO_CTRL_PENDING, 0, NULL);

    if (ret < 0)
        ret = 0;
#if LONG_MAX > SIZE_MAX
    if (ret > SIZE_MAX)
        ret = SIZE_MAX;
#endif
    return (size_t)ret;
}

size_t BIO_ctrl_wpending(BIO *bio)
{
    long ret = BIO_ctrl(bio, BIO_CTRL_WPENDING, 0, NULL);

    if (ret < 0)
        ret = 0;
#if LONG_MAX > SIZE_MAX
    if (ret > SIZE_MAX)
        ret = SIZE_MAX;
#endif
    return (size_t)ret;
}

/* put the 'bio' on the end of b's list of operators */
BIO *BIO_push(BIO *b, BIO *bio)
{
    BIO *lb;

    if (b == NULL)
        return bio;
    lb = b;
    while (lb->next_bio != NULL)
        lb = lb->next_bio;
    lb->next_bio = bio;
    if (bio != NULL)
        bio->prev_bio = lb;
    /* called to do internal processing */
    BIO_ctrl(b, BIO_CTRL_PUSH, 0, lb);
    return b;
}

/* Remove the first and return the rest */
BIO *BIO_pop(BIO *b)
{
    BIO *ret;

    if (b == NULL)
        return NULL;
    ret = b->next_bio;

    BIO_ctrl(b, BIO_CTRL_POP, 0, b);

    if (b->prev_bio != NULL)
        b->prev_bio->next_bio = b->next_bio;
    if (b->next_bio != NULL)
        b->next_bio->prev_bio = b->prev_bio;

    b->next_bio = NULL;
    b->prev_bio = NULL;
    return ret;
}

BIO *BIO_get_retry_BIO(BIO *bio, int *reason)
{
    BIO *b, *last;

    b = last = bio;
    for (;;) {
        if (!BIO_should_retry(b))
            break;
        last = b;
        b = b->next_bio;
        if (b == NULL)
            break;
    }
    if (reason != NULL)
        *reason = last->retry_reason;
    return last;
}

int BIO_get_retry_reason(BIO *bio)
{
    return bio->retry_reason;
}

void BIO_set_retry_reason(BIO *bio, int reason)
{
    bio->retry_reason = reason;
}

BIO *BIO_find_type(BIO *bio, int type)
{
    int mt, mask;

    if (bio == NULL) {
        ERR_raise(ERR_LIB_BIO, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }
    mask = type & BIO_TYPE_MASK;
    do {
        if (bio->method != NULL) {
            mt = bio->method->type;

            if (!mask) {
                if (mt & type)
                    return bio;
            } else if (mt == type) {
                return bio;
            }
        }
        bio = bio->next_bio;
    } while (bio != NULL);
    return NULL;
}

BIO *BIO_next(BIO *b)
{
    if (b == NULL)
        return NULL;
    return b->next_bio;
}

void BIO_set_next(BIO *b, BIO *next)
{
    b->next_bio = next;
}

void BIO_free_all(BIO *bio)
{
    BIO *b;
    int ref;

    while (bio != NULL) {
        b = bio;
        CRYPTO_GET_REF(&b->references, &ref);
        bio = bio->next_bio;
        BIO_free(b);
        /* Since ref count > 1, don't free anyone else. */
        if (ref > 1)
            break;
    }
}

BIO *BIO_dup_chain(BIO *in)
{
    BIO *ret = NULL, *eoc = NULL, *bio, *new_bio;

    for (bio = in; bio != NULL; bio = bio->next_bio) {
        if ((new_bio = BIO_new(bio->method)) == NULL)
            goto err;
#ifndef OPENSSL_NO_DEPRECATED_3_0
        new_bio->callback = bio->callback;
#endif
        new_bio->callback_ex = bio->callback_ex;
        new_bio->cb_arg = bio->cb_arg;
        new_bio->init = bio->init;
        new_bio->shutdown = bio->shutdown;
        new_bio->flags = bio->flags;

        /* This will let SSL_s_sock() work with stdin/stdout */
        new_bio->num = bio->num;

        if (BIO_dup_state(bio, (char *)new_bio) <= 0) {
            BIO_free(new_bio);
            goto err;
        }

        /* copy app data */
        if (!CRYPTO_dup_ex_data(CRYPTO_EX_INDEX_BIO, &new_bio->ex_data,
                                &bio->ex_data)) {
            BIO_free(new_bio);
            goto err;
        }

        if (ret == NULL) {
            eoc = new_bio;
            ret = eoc;
        } else {
            BIO_push(eoc, new_bio);
            eoc = new_bio;
        }
    }
    return ret;
 err:
    BIO_free_all(ret);

    return NULL;
}

void BIO_copy_next_retry(BIO *b)
{
    BIO_set_flags(b, BIO_get_retry_flags(b->next_bio));
    b->retry_reason = b->next_bio->retry_reason;
}

int BIO_set_ex_data(BIO *bio, int idx, void *data)
{
    return CRYPTO_set_ex_data(&(bio->ex_data), idx, data);
}

void *BIO_get_ex_data(const BIO *bio, int idx)
{
    return CRYPTO_get_ex_data(&(bio->ex_data), idx);
}

uint64_t BIO_number_read(BIO *bio)
{
    if (bio)
        return bio->num_read;
    return 0;
}

uint64_t BIO_number_written(BIO *bio)
{
    if (bio)
        return bio->num_write;
    return 0;
}

void bio_free_ex_data(BIO *bio)
{
    CRYPTO_free_ex_data(CRYPTO_EX_INDEX_BIO, bio, &bio->ex_data);
}

void bio_cleanup(void)
{
#ifndef OPENSSL_NO_SOCK
    bio_sock_cleanup_int();
    CRYPTO_THREAD_lock_free(bio_lookup_lock);
    bio_lookup_lock = NULL;
#endif
    CRYPTO_FREE_REF(&bio_type_count);
}

/* Internal variant of the below BIO_wait() not calling ERR_raise(...) */
static int bio_wait(BIO *bio, time_t max_time, unsigned int nap_milliseconds)
{
#ifndef OPENSSL_NO_SOCK
    int fd;
#endif
    long sec_diff;

    if (max_time == 0) /* no timeout */
        return 1;

#ifndef OPENSSL_NO_SOCK
    if (BIO_get_fd(bio, &fd) > 0) {
        int ret = BIO_socket_wait(fd, BIO_should_read(bio), max_time);

        if (ret != -1)
            return ret;
    }
#endif
    /* fall back to polling since no sockets are available */

    sec_diff = (long)(max_time - time(NULL)); /* might overflow */
    if (sec_diff < 0)
        return 0; /* clearly timeout */

    /* now take a nap at most the given number of milliseconds */
    if (sec_diff == 0) { /* we are below the 1 seconds resolution of max_time */
        if (nap_milliseconds > 1000)
            nap_milliseconds = 1000;
    } else { /* for sec_diff > 0, take min(sec_diff * 1000, nap_milliseconds) */
        if ((unsigned long)sec_diff * 1000 < nap_milliseconds)
            nap_milliseconds = (unsigned int)sec_diff * 1000;
    }
    OSSL_sleep(nap_milliseconds);
    return 1;
}

/*-
 * Wait on (typically socket-based) BIO at most until max_time.
 * Succeed immediately if max_time == 0.
 * If sockets are not available support polling: succeed after waiting at most
 * the number of nap_milliseconds in order to avoid a tight busy loop.
 * Call ERR_raise(ERR_LIB_BIO, ...) on timeout or error.
 * Returns -1 on error, 0 on timeout, and 1 on success.
 */
int BIO_wait(BIO *bio, time_t max_time, unsigned int nap_milliseconds)
{
    int rv = bio_wait(bio, max_time, nap_milliseconds);

    if (rv <= 0)
        ERR_raise(ERR_LIB_BIO,
                  rv == 0 ? BIO_R_TRANSFER_TIMEOUT : BIO_R_TRANSFER_ERROR);
    return rv;
}

/*
 * Connect via given BIO using BIO_do_connect() until success/timeout/error.
 * Parameter timeout == 0 means no timeout, < 0 means exactly one try.
 * For non-blocking and potentially even non-socket BIOs perform polling with
 * the given density: between polls sleep nap_milliseconds using BIO_wait()
 * in order to avoid a tight busy loop.
 * Returns -1 on error, 0 on timeout, and 1 on success.
 */
int BIO_do_connect_retry(BIO *bio, int timeout, int nap_milliseconds)
{
    int blocking = timeout <= 0;
    time_t max_time = timeout > 0 ? time(NULL) + timeout : 0;
    int rv;

    if (bio == NULL) {
        ERR_raise(ERR_LIB_BIO, ERR_R_PASSED_NULL_PARAMETER);
        return -1;
    }

    if (nap_milliseconds < 0)
        nap_milliseconds = 100;
    BIO_set_nbio(bio, !blocking);

 retry:
    ERR_set_mark();
    rv = BIO_do_connect(bio);

    if (rv <= 0) { /* could be timeout or retryable error or fatal error */
        int err = ERR_peek_last_error();
        int reason = ERR_GET_REASON(err);
        int do_retry = BIO_should_retry(bio); /* may be 1 only if !blocking */

        if (ERR_GET_LIB(err) == ERR_LIB_BIO) {
            switch (reason) {
            case ERR_R_SYS_LIB:
                /*
                 * likely retryable system error occurred, which may be
                 * EAGAIN (resource temporarily unavailable) some 40 secs after
                 * calling getaddrinfo(): Temporary failure in name resolution
                 * or a premature ETIMEDOUT, some 30 seconds after connect()
                 */
            case BIO_R_CONNECT_ERROR:
            case BIO_R_NBIO_CONNECT_ERROR:
                /* some likely retryable connection error occurred */
                (void)BIO_reset(bio); /* often needed to avoid retry failure */
                do_retry = 1;
                break;
            default:
                break;
            }
        }
        if (timeout >= 0 && do_retry) {
            ERR_pop_to_mark();
            /* will not actually wait if timeout == 0 (i.e., blocking BIO): */
            rv = bio_wait(bio, max_time, nap_milliseconds);
            if (rv > 0)
                goto retry;
            ERR_raise(ERR_LIB_BIO,
                      rv == 0 ? BIO_R_CONNECT_TIMEOUT : BIO_R_CONNECT_ERROR);
        } else {
            ERR_clear_last_mark();
            rv = -1;
            if (err == 0) /* missing error queue entry */
                /* workaround: general error */
                ERR_raise(ERR_LIB_BIO, BIO_R_CONNECT_ERROR);
        }
    } else {
        ERR_clear_last_mark();
    }

    return rv;
}
