/*
 * Copyright 1995-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <errno.h>
#include "bio_local.h"
#include "internal/bio_tfo.h"
#include "internal/cryptlib.h"
#include "internal/ktls.h"

#ifndef OPENSSL_NO_SOCK

# include <openssl/bio.h>

# ifdef WATT32
/* Watt-32 uses same names */
#  undef sock_write
#  undef sock_read
#  undef sock_puts
#  define sock_write SockWrite
#  define sock_read  SockRead
#  define sock_puts  SockPuts
# endif

struct bss_sock_st {
    BIO_ADDR tfo_peer;
    int tfo_first;
#ifndef OPENSSL_NO_KTLS
    unsigned char ktls_record_type;
#endif
};

static int sock_write(BIO *h, const char *buf, int num);
static int sock_read(BIO *h, char *buf, int size);
static int sock_puts(BIO *h, const char *str);
static long sock_ctrl(BIO *h, int cmd, long arg1, void *arg2);
static int sock_new(BIO *h);
static int sock_free(BIO *data);
int BIO_sock_should_retry(int s);

static const BIO_METHOD methods_sockp = {
    BIO_TYPE_SOCKET,
    "socket",
    bwrite_conv,
    sock_write,
    bread_conv,
    sock_read,
    sock_puts,
    NULL,                       /* sock_gets,         */
    sock_ctrl,
    sock_new,
    sock_free,
    NULL,                       /* sock_callback_ctrl */
};

const BIO_METHOD *BIO_s_socket(void)
{
    return &methods_sockp;
}

BIO *BIO_new_socket(int fd, int close_flag)
{
    BIO *ret;

    ret = BIO_new(BIO_s_socket());
    if (ret == NULL)
        return NULL;
    BIO_set_fd(ret, fd, close_flag);
# ifndef OPENSSL_NO_KTLS
    {
        /*
         * The new socket is created successfully regardless of ktls_enable.
         * ktls_enable doesn't change any functionality of the socket, except
         * changing the setsockopt to enable the processing of ktls_start.
         * Thus, it is not a problem to call it for non-TLS sockets.
         */
        ktls_enable(fd);
    }
# endif
    return ret;
}

static int sock_new(BIO *bi)
{
    bi->init = 0;
    bi->num = 0;
    bi->flags = 0;
    bi->ptr = OPENSSL_zalloc(sizeof(struct bss_sock_st));
    if (bi->ptr == NULL)
        return 0;
    return 1;
}

static int sock_free(BIO *a)
{
    if (a == NULL)
        return 0;
    if (a->shutdown) {
        if (a->init) {
            BIO_closesocket(a->num);
        }
        a->init = 0;
        a->flags = 0;
    }
    OPENSSL_free(a->ptr);
    a->ptr = NULL;
    return 1;
}

static int sock_read(BIO *b, char *out, int outl)
{
    int ret = 0;

    if (out != NULL) {
        clear_socket_error();
# ifndef OPENSSL_NO_KTLS
        if (BIO_get_ktls_recv(b))
            ret = ktls_read_record(b->num, out, outl);
        else
# endif
            ret = readsocket(b->num, out, outl);
        BIO_clear_retry_flags(b);
        if (ret <= 0) {
            if (BIO_sock_should_retry(ret))
                BIO_set_retry_read(b);
            else if (ret == 0)
                b->flags |= BIO_FLAGS_IN_EOF;
        }
    }
    return ret;
}

static int sock_write(BIO *b, const char *in, int inl)
{
    int ret = 0;
# if !defined(OPENSSL_NO_KTLS) || defined(OSSL_TFO_SENDTO)
    struct bss_sock_st *data = (struct bss_sock_st *)b->ptr;
# endif

    clear_socket_error();
# ifndef OPENSSL_NO_KTLS
    if (BIO_should_ktls_ctrl_msg_flag(b)) {
        unsigned char record_type = data->ktls_record_type;
        ret = ktls_send_ctrl_message(b->num, record_type, in, inl);
        if (ret >= 0) {
            ret = inl;
            BIO_clear_ktls_ctrl_msg_flag(b);
        }
    } else
# endif
# if defined(OSSL_TFO_SENDTO)
    if (data->tfo_first) {
        struct bss_sock_st *data = (struct bss_sock_st *)b->ptr;
        socklen_t peerlen = BIO_ADDR_sockaddr_size(&data->tfo_peer);

        ret = sendto(b->num, in, inl, OSSL_TFO_SENDTO,
                     BIO_ADDR_sockaddr(&data->tfo_peer), peerlen);
        data->tfo_first = 0;
    } else
# endif
        ret = writesocket(b->num, in, inl);
    BIO_clear_retry_flags(b);
    if (ret <= 0) {
        if (BIO_sock_should_retry(ret))
            BIO_set_retry_write(b);
    }
    return ret;
}

static long sock_ctrl(BIO *b, int cmd, long num, void *ptr)
{
    long ret = 1;
    int *ip;
    struct bss_sock_st *data = (struct bss_sock_st *)b->ptr;
# ifndef OPENSSL_NO_KTLS
    ktls_crypto_info_t *crypto_info;
# endif

    switch (cmd) {
    case BIO_C_SET_FD:
        /* minimal sock_free() */
        if (b->shutdown) {
            if (b->init)
                BIO_closesocket(b->num);
            b->flags = 0;
        }
        b->num = *((int *)ptr);
        b->shutdown = (int)num;
        b->init = 1;
        data->tfo_first = 0;
        memset(&data->tfo_peer, 0, sizeof(data->tfo_peer));
        break;
    case BIO_C_GET_FD:
        if (b->init) {
            ip = (int *)ptr;
            if (ip != NULL)
                *ip = b->num;
            ret = b->num;
        } else
            ret = -1;
        break;
    case BIO_CTRL_GET_CLOSE:
        ret = b->shutdown;
        break;
    case BIO_CTRL_SET_CLOSE:
        b->shutdown = (int)num;
        break;
    case BIO_CTRL_DUP:
    case BIO_CTRL_FLUSH:
        ret = 1;
        break;
    case BIO_CTRL_GET_RPOLL_DESCRIPTOR:
    case BIO_CTRL_GET_WPOLL_DESCRIPTOR:
        {
            BIO_POLL_DESCRIPTOR *pd = ptr;

            if (!b->init) {
                ret = 0;
                break;
            }

            pd->type        = BIO_POLL_DESCRIPTOR_TYPE_SOCK_FD;
            pd->value.fd    = b->num;
        }
        break;
# ifndef OPENSSL_NO_KTLS
    case BIO_CTRL_SET_KTLS:
        crypto_info = (ktls_crypto_info_t *)ptr;
        ret = ktls_start(b->num, crypto_info, num);
        if (ret)
            BIO_set_ktls_flag(b, num);
        break;
    case BIO_CTRL_GET_KTLS_SEND:
        return BIO_should_ktls_flag(b, 1) != 0;
    case BIO_CTRL_GET_KTLS_RECV:
        return BIO_should_ktls_flag(b, 0) != 0;
    case BIO_CTRL_SET_KTLS_TX_SEND_CTRL_MSG:
        BIO_set_ktls_ctrl_msg_flag(b);
        data->ktls_record_type = (unsigned char)num;
        ret = 0;
        break;
    case BIO_CTRL_CLEAR_KTLS_TX_CTRL_MSG:
        BIO_clear_ktls_ctrl_msg_flag(b);
        ret = 0;
        break;
    case BIO_CTRL_SET_KTLS_TX_ZEROCOPY_SENDFILE:
        ret = ktls_enable_tx_zerocopy_sendfile(b->num);
        if (ret)
            BIO_set_ktls_zerocopy_sendfile_flag(b);
        break;
# endif
    case BIO_CTRL_EOF:
        ret = (b->flags & BIO_FLAGS_IN_EOF) != 0;
        break;
    case BIO_C_GET_CONNECT:
        if (ptr != NULL && num == 2) {
            const char **pptr = (const char **)ptr;

            *pptr = (const char *)&data->tfo_peer;
        } else {
            ret = 0;
        }
        break;
    case BIO_C_SET_CONNECT:
        if (ptr != NULL && num == 2) {
            ret = BIO_ADDR_make(&data->tfo_peer,
                                BIO_ADDR_sockaddr((const BIO_ADDR *)ptr));
            if (ret)
                data->tfo_first = 1;
        } else {
            ret = 0;
        }
        break;
    default:
        ret = 0;
        break;
    }
    return ret;
}

static int sock_puts(BIO *bp, const char *str)
{
    int n, ret;

    n = strlen(str);
    ret = sock_write(bp, str, n);
    return ret;
}

int BIO_sock_should_retry(int i)
{
    int err;

    if ((i == 0) || (i == -1)) {
        err = get_last_socket_error();

        return BIO_sock_non_fatal_error(err);
    }
    return 0;
}

int BIO_sock_non_fatal_error(int err)
{
    switch (err) {
# if defined(OPENSSL_SYS_WINDOWS)
#  if defined(WSAEWOULDBLOCK)
    case WSAEWOULDBLOCK:
#  endif
# endif

# ifdef EWOULDBLOCK
#  ifdef WSAEWOULDBLOCK
#   if WSAEWOULDBLOCK != EWOULDBLOCK
    case EWOULDBLOCK:
#   endif
#  else
    case EWOULDBLOCK:
#  endif
# endif

# if defined(ENOTCONN)
    case ENOTCONN:
# endif

# ifdef EINTR
    case EINTR:
# endif

# ifdef EAGAIN
#  if EWOULDBLOCK != EAGAIN
    case EAGAIN:
#  endif
# endif

# ifdef EPROTO
    case EPROTO:
# endif

# ifdef EINPROGRESS
    case EINPROGRESS:
# endif

# ifdef EALREADY
    case EALREADY:
# endif
        return 1;
    default:
        break;
    }
    return 0;
}

#endif                          /* #ifndef OPENSSL_NO_SOCK */
