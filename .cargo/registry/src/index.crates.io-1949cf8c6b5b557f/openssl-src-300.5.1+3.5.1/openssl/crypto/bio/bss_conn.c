/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
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
#include "internal/ktls.h"

#ifndef OPENSSL_NO_SOCK

typedef struct bio_connect_st {
    int state;
    int connect_family;
    int connect_sock_type;
    char *param_hostname;
    char *param_service;
    int connect_mode;
# ifndef OPENSSL_NO_KTLS
    unsigned char record_type;
# endif
    int tfo_first;

    BIO_ADDRINFO *addr_first;
    const BIO_ADDRINFO *addr_iter;
    /*
     * int socket; this will be kept in bio->num so that it is compatible
     * with the bss_sock bio
     */
    /*
     * called when the connection is initially made callback(BIO,state,ret);
     * The callback should return 'ret'.  state is for compatibility with the
     * ssl info_callback
     */
    BIO_info_cb *info_callback;
    /*
     * Used when connect_sock_type is SOCK_DGRAM. Owned by us; we forward
     * read/write(mmsg) calls to this if present.
     */
    BIO *dgram_bio;
} BIO_CONNECT;

static int conn_write(BIO *h, const char *buf, int num);
static int conn_read(BIO *h, char *buf, int size);
static int conn_puts(BIO *h, const char *str);
static int conn_gets(BIO *h, char *buf, int size);
static long conn_ctrl(BIO *h, int cmd, long arg1, void *arg2);
static int conn_new(BIO *h);
static int conn_free(BIO *data);
static long conn_callback_ctrl(BIO *h, int cmd, BIO_info_cb *);
static int conn_sendmmsg(BIO *h, BIO_MSG *m, size_t s, size_t n,
                         uint64_t f, size_t *mp);
static int conn_recvmmsg(BIO *h, BIO_MSG *m, size_t s, size_t n,
                         uint64_t f, size_t *mp);

static int conn_state(BIO *b, BIO_CONNECT *c);
static void conn_close_socket(BIO *data);
static BIO_CONNECT *BIO_CONNECT_new(void);
static void BIO_CONNECT_free(BIO_CONNECT *a);

#define BIO_CONN_S_BEFORE                1
#define BIO_CONN_S_GET_ADDR              2
#define BIO_CONN_S_CREATE_SOCKET         3
#define BIO_CONN_S_CONNECT               4
#define BIO_CONN_S_OK                    5
#define BIO_CONN_S_BLOCKED_CONNECT       6
#define BIO_CONN_S_CONNECT_ERROR         7

static const BIO_METHOD methods_connectp = {
    BIO_TYPE_CONNECT,
    "socket connect",
    bwrite_conv,
    conn_write,
    bread_conv,
    conn_read,
    conn_puts,
    conn_gets,
    conn_ctrl,
    conn_new,
    conn_free,
    conn_callback_ctrl,
    conn_sendmmsg,
    conn_recvmmsg,
};

static int conn_create_dgram_bio(BIO *b, BIO_CONNECT *c)
{
    if (c->connect_sock_type != SOCK_DGRAM)
        return 1;

#ifndef OPENSSL_NO_DGRAM
    c->dgram_bio = BIO_new_dgram(b->num, 0);
    if (c->dgram_bio == NULL)
        goto err;

    return 1;

err:
#endif
    c->state = BIO_CONN_S_CONNECT_ERROR;
    return 0;
}

static int conn_state(BIO *b, BIO_CONNECT *c)
{
    int ret = -1, i, opts;
    BIO_info_cb *cb = NULL;

    if (c->info_callback != NULL)
        cb = c->info_callback;

    for (;;) {
        switch (c->state) {
        case BIO_CONN_S_BEFORE:
            if (c->param_hostname == NULL && c->param_service == NULL) {
                ERR_raise_data(ERR_LIB_BIO,
                               BIO_R_NO_HOSTNAME_OR_SERVICE_SPECIFIED,
                               "hostname=%s service=%s",
                               c->param_hostname, c->param_service);
                goto exit_loop;
            }
            c->state = BIO_CONN_S_GET_ADDR;
            break;

        case BIO_CONN_S_GET_ADDR:
            {
                int family = AF_UNSPEC;
                switch (c->connect_family) {
                case BIO_FAMILY_IPV6:
                    if (1) { /* This is a trick we use to avoid bit rot.
                              * at least the "else" part will always be
                              * compiled.
                              */
#if OPENSSL_USE_IPV6
                        family = AF_INET6;
                    } else {
#endif
                        ERR_raise(ERR_LIB_BIO, BIO_R_UNAVAILABLE_IP_FAMILY);
                        goto exit_loop;
                    }
                    break;
                case BIO_FAMILY_IPV4:
                    family = AF_INET;
                    break;
                case BIO_FAMILY_IPANY:
                    family = AF_UNSPEC;
                    break;
                default:
                    ERR_raise(ERR_LIB_BIO, BIO_R_UNSUPPORTED_IP_FAMILY);
                    goto exit_loop;
                }
                if (BIO_lookup(c->param_hostname, c->param_service,
                               BIO_LOOKUP_CLIENT,
                               family, c->connect_sock_type,
                               &c->addr_first) == 0)
                    goto exit_loop;
            }
            if (c->addr_first == NULL) {
                ERR_raise(ERR_LIB_BIO, BIO_R_LOOKUP_RETURNED_NOTHING);
                goto exit_loop;
            }
            c->addr_iter = c->addr_first;
            c->state = BIO_CONN_S_CREATE_SOCKET;
            break;

        case BIO_CONN_S_CREATE_SOCKET:
            ret = BIO_socket(BIO_ADDRINFO_family(c->addr_iter),
                             BIO_ADDRINFO_socktype(c->addr_iter),
                             BIO_ADDRINFO_protocol(c->addr_iter), 0);
            if (ret == (int)INVALID_SOCKET) {
                ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                               "calling socket(%s, %s)",
                               c->param_hostname, c->param_service);
                ERR_raise(ERR_LIB_BIO, BIO_R_UNABLE_TO_CREATE_SOCKET);
                goto exit_loop;
            }
            b->num = ret;
            c->state = BIO_CONN_S_CONNECT;
            break;

        case BIO_CONN_S_CONNECT:
            BIO_clear_retry_flags(b);
            ERR_set_mark();

            opts = c->connect_mode;
            if (BIO_ADDRINFO_socktype(c->addr_iter) == SOCK_STREAM)
                opts |= BIO_SOCK_KEEPALIVE;

            ret = BIO_connect(b->num, BIO_ADDRINFO_address(c->addr_iter), opts);
            b->retry_reason = 0;
            if (ret == 0) {
                if (BIO_sock_should_retry(ret)) {
                    BIO_set_retry_special(b);
                    c->state = BIO_CONN_S_BLOCKED_CONNECT;
                    b->retry_reason = BIO_RR_CONNECT;
                    ERR_pop_to_mark();
                } else if ((c->addr_iter = BIO_ADDRINFO_next(c->addr_iter))
                           != NULL) {
                    /*
                     * if there are more addresses to try, do that first
                     */
                    BIO_closesocket(b->num);
                    c->state = BIO_CONN_S_CREATE_SOCKET;
                    ERR_pop_to_mark();
                    break;
                } else {
                    ERR_clear_last_mark();
                    ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                                   "calling connect(%s, %s)",
                                    c->param_hostname, c->param_service);
                    c->state = BIO_CONN_S_CONNECT_ERROR;
                    break;
                }
                goto exit_loop;
            } else {
                ERR_clear_last_mark();
                if (!conn_create_dgram_bio(b, c))
                    break;
                c->state = BIO_CONN_S_OK;
            }
            break;

        case BIO_CONN_S_BLOCKED_CONNECT:
            /* wait for socket being writable, before querying BIO_sock_error */
            if (BIO_socket_wait(b->num, 0, time(NULL)) == 0)
                break;
            i = BIO_sock_error(b->num);
            if (i != 0) {
                BIO_clear_retry_flags(b);
                if ((c->addr_iter = BIO_ADDRINFO_next(c->addr_iter)) != NULL) {
                    /*
                     * if there are more addresses to try, do that first
                     */
                    BIO_closesocket(b->num);
                    c->state = BIO_CONN_S_CREATE_SOCKET;
                    break;
                }
                ERR_raise_data(ERR_LIB_SYS, i,
                               "calling connect(%s, %s)",
                                c->param_hostname, c->param_service);
                ERR_raise(ERR_LIB_BIO, BIO_R_NBIO_CONNECT_ERROR);
                ret = 0;
                goto exit_loop;
            } else {
                if (!conn_create_dgram_bio(b, c))
                    break;
                c->state = BIO_CONN_S_OK;
# ifndef OPENSSL_NO_KTLS
                /*
                 * The new socket is created successfully regardless of ktls_enable.
                 * ktls_enable doesn't change any functionality of the socket, except
                 * changing the setsockopt to enable the processing of ktls_start.
                 * Thus, it is not a problem to call it for non-TLS sockets.
                 */
                ktls_enable(b->num);
# endif
            }
            break;

        case BIO_CONN_S_CONNECT_ERROR:
            ERR_raise(ERR_LIB_BIO, BIO_R_CONNECT_ERROR);
            ret = 0;
            goto exit_loop;

        case BIO_CONN_S_OK:
            ret = 1;
            goto exit_loop;
        default:
            /* abort(); */
            goto exit_loop;
        }

        if (cb != NULL) {
            if ((ret = cb((BIO *)b, c->state, ret)) == 0)
                goto end;
        }
    }

    /* Loop does not exit */
 exit_loop:
    if (cb != NULL)
        ret = cb((BIO *)b, c->state, ret);
 end:
    return ret;
}

static BIO_CONNECT *BIO_CONNECT_new(void)
{
    BIO_CONNECT *ret;

    if ((ret = OPENSSL_zalloc(sizeof(*ret))) == NULL)
        return NULL;
    ret->state = BIO_CONN_S_BEFORE;
    ret->connect_family = BIO_FAMILY_IPANY;
    ret->connect_sock_type = SOCK_STREAM;
    return ret;
}

static void BIO_CONNECT_free(BIO_CONNECT *a)
{
    if (a == NULL)
        return;
    OPENSSL_free(a->param_hostname);
    OPENSSL_free(a->param_service);
    BIO_ADDRINFO_free(a->addr_first);
    OPENSSL_free(a);
}

const BIO_METHOD *BIO_s_connect(void)
{
    return &methods_connectp;
}

static int conn_new(BIO *bi)
{
    bi->init = 0;
    bi->num = (int)INVALID_SOCKET;
    bi->flags = 0;
    if ((bi->ptr = (char *)BIO_CONNECT_new()) == NULL)
        return 0;
    else
        return 1;
}

static void conn_close_socket(BIO *bio)
{
    BIO_CONNECT *c;

    c = (BIO_CONNECT *)bio->ptr;
    if (bio->num != (int)INVALID_SOCKET) {
        /* Only do a shutdown if things were established */
        if (c->state == BIO_CONN_S_OK)
            shutdown(bio->num, 2);
        BIO_closesocket(bio->num);
        bio->num = (int)INVALID_SOCKET;
    }
}

static int conn_free(BIO *a)
{
    BIO_CONNECT *data;

    if (a == NULL)
        return 0;
    data = (BIO_CONNECT *)a->ptr;

    BIO_free(data->dgram_bio);

    if (a->shutdown) {
        conn_close_socket(a);
        BIO_CONNECT_free(data);
        a->ptr = NULL;
        a->flags = 0;
        a->init = 0;
    }
    return 1;
}

static int conn_read(BIO *b, char *out, int outl)
{
    int ret = 0;
    BIO_CONNECT *data;

    data = (BIO_CONNECT *)b->ptr;
    if (data->state != BIO_CONN_S_OK) {
        ret = conn_state(b, data);
        if (ret <= 0)
            return ret;
    }

    if (data->dgram_bio != NULL) {
        BIO_clear_retry_flags(b);
        ret = BIO_read(data->dgram_bio, out, outl);
        BIO_set_flags(b, BIO_get_retry_flags(data->dgram_bio));
        return ret;
    }

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

static int conn_write(BIO *b, const char *in, int inl)
{
    int ret;
    BIO_CONNECT *data;

    data = (BIO_CONNECT *)b->ptr;
    if (data->state != BIO_CONN_S_OK) {
        ret = conn_state(b, data);
        if (ret <= 0)
            return ret;
    }

    if (data->dgram_bio != NULL) {
        BIO_clear_retry_flags(b);
        ret = BIO_write(data->dgram_bio, in, inl);
        BIO_set_flags(b, BIO_get_retry_flags(data->dgram_bio));
        return ret;
    }

    clear_socket_error();
# ifndef OPENSSL_NO_KTLS
    if (BIO_should_ktls_ctrl_msg_flag(b)) {
        ret = ktls_send_ctrl_message(b->num, data->record_type, in, inl);
        if (ret >= 0) {
            ret = inl;
            BIO_clear_ktls_ctrl_msg_flag(b);
        }
    } else
# endif
# if defined(OSSL_TFO_SENDTO)
    if (data->tfo_first) {
        int peerlen = BIO_ADDRINFO_sockaddr_size(data->addr_iter);

        ret = sendto(b->num, in, inl, OSSL_TFO_SENDTO,
                     BIO_ADDRINFO_sockaddr(data->addr_iter), peerlen);
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

static long conn_ctrl(BIO *b, int cmd, long num, void *ptr)
{
    BIO *dbio;
    int *ip;
    const char **pptr = NULL;
    long ret = 1;
    BIO_CONNECT *data;
    const BIO_ADDR *dg_addr;
# ifndef OPENSSL_NO_KTLS
    ktls_crypto_info_t *crypto_info;
# endif

    data = (BIO_CONNECT *)b->ptr;

    switch (cmd) {
    case BIO_CTRL_RESET:
        ret = 0;
        data->state = BIO_CONN_S_BEFORE;
        conn_close_socket(b);
        BIO_ADDRINFO_free(data->addr_first);
        data->addr_first = NULL;
        b->flags = 0;
        break;
    case BIO_C_DO_STATE_MACHINE:
        /* use this one to start the connection */
        if (data->state != BIO_CONN_S_OK)
            ret = (long)conn_state(b, data);
        else
            ret = 1;
        break;
    case BIO_C_GET_CONNECT:
        if (ptr != NULL) {
            pptr = (const char **)ptr;
            if (num == 0) {
                *pptr = data->param_hostname;
            } else if (num == 1) {
                *pptr = data->param_service;
            } else if (num == 2) {
                *pptr = (const char *)BIO_ADDRINFO_address(data->addr_iter);
            } else if (num == 3) {
                switch (BIO_ADDRINFO_family(data->addr_iter)) {
# if OPENSSL_USE_IPV6
                case AF_INET6:
                    ret = BIO_FAMILY_IPV6;
                    break;
# endif
                case AF_INET:
                    ret = BIO_FAMILY_IPV4;
                    break;
                case 0:
                    ret = data->connect_family;
                    break;
                default:
                    ret = -1;
                    break;
                }
            } else if (num == 4) {
                ret = data->connect_mode;
            } else {
                ret = 0;
            }
        } else {
            ret = 0;
        }
        break;
    case BIO_C_SET_CONNECT:
        if (ptr != NULL) {
            b->init = 1;
            if (num == 0) { /* BIO_set_conn_hostname */
                char *hold_service = data->param_service;
                /* We affect the hostname regardless.  However, the input
                 * string might contain a host:service spec, so we must
                 * parse it, which might or might not affect the service
                 */

                OPENSSL_free(data->param_hostname);
                data->param_hostname = NULL;
                ret = BIO_parse_hostserv(ptr,
                                         &data->param_hostname,
                                         &data->param_service,
                                         BIO_PARSE_PRIO_HOST);
                if (hold_service != data->param_service)
                    OPENSSL_free(hold_service);
            } else if (num == 1) { /* BIO_set_conn_port */
                OPENSSL_free(data->param_service);
                if ((data->param_service = OPENSSL_strdup(ptr)) == NULL)
                    ret = 0;
            } else if (num == 2) { /* BIO_set_conn_address */
                const BIO_ADDR *addr = (const BIO_ADDR *)ptr;
                char *host = BIO_ADDR_hostname_string(addr, 1);
                char *service = BIO_ADDR_service_string(addr, 1);

                ret = host != NULL && service != NULL;
                if (ret) {
                    OPENSSL_free(data->param_hostname);
                    data->param_hostname = host;
                    OPENSSL_free(data->param_service);
                    data->param_service = service;
                    BIO_ADDRINFO_free(data->addr_first);
                    data->addr_first = NULL;
                    data->addr_iter = NULL;
                } else {
                    OPENSSL_free(host);
                    OPENSSL_free(service);
                }
            } else if (num == 3) { /* BIO_set_conn_ip_family */
                data->connect_family = *(int *)ptr;
            } else {
                ret = 0;
            }
        }
        break;
    case BIO_C_SET_SOCK_TYPE:
        if ((num != SOCK_STREAM && num != SOCK_DGRAM)
            || data->state >= BIO_CONN_S_GET_ADDR) {
            ret = 0;
            break;
        }

        data->connect_sock_type = (int)num;
        ret = 1;
        break;
    case BIO_C_GET_SOCK_TYPE:
        ret = data->connect_sock_type;
        break;
    case BIO_C_GET_DGRAM_BIO:
        if (data->dgram_bio != NULL) {
            *(BIO **)ptr = data->dgram_bio;
            ret = 1;
        } else {
            ret = 0;
        }
        break;
    case BIO_CTRL_DGRAM_GET_PEER:
    case BIO_CTRL_DGRAM_DETECT_PEER_ADDR:
        if (data->state != BIO_CONN_S_OK)
            conn_state(b, data); /* best effort */

        if (data->state >= BIO_CONN_S_CREATE_SOCKET
            && data->addr_iter != NULL
            && (dg_addr = BIO_ADDRINFO_address(data->addr_iter)) != NULL) {

            ret = BIO_ADDR_sockaddr_size(dg_addr);
            if (num == 0 || num > ret)
                num = ret;

            memcpy(ptr, dg_addr, num);
            ret = num;
        } else {
            ret = 0;
        }

        break;
    case BIO_CTRL_GET_RPOLL_DESCRIPTOR:
    case BIO_CTRL_GET_WPOLL_DESCRIPTOR:
        {
            BIO_POLL_DESCRIPTOR *pd = ptr;

            if (data->state != BIO_CONN_S_OK)
                conn_state(b, data); /* best effort */

            if (data->state >= BIO_CONN_S_CREATE_SOCKET) {
                pd->type        = BIO_POLL_DESCRIPTOR_TYPE_SOCK_FD;
                pd->value.fd    = b->num;
            } else {
                ret = 0;
            }
        }
        break;
    case BIO_C_SET_NBIO:
        if (num != 0)
            data->connect_mode |= BIO_SOCK_NONBLOCK;
        else
            data->connect_mode &= ~BIO_SOCK_NONBLOCK;

        if (data->dgram_bio != NULL)
            ret = BIO_set_nbio(data->dgram_bio, num);

        break;
#if defined(TCP_FASTOPEN) && !defined(OPENSSL_NO_TFO)
    case BIO_C_SET_TFO:
        if (num != 0) {
            data->connect_mode |= BIO_SOCK_TFO;
            data->tfo_first = 1;
        } else {
            data->connect_mode &= ~BIO_SOCK_TFO;
            data->tfo_first = 0;
        }
        break;
#endif
    case BIO_C_SET_CONNECT_MODE:
        data->connect_mode = (int)num;
        if (num & BIO_SOCK_TFO)
            data->tfo_first = 1;
        else
            data->tfo_first = 0;
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
    case BIO_CTRL_PENDING:
    case BIO_CTRL_WPENDING:
        ret = 0;
        break;
    case BIO_CTRL_FLUSH:
        break;
    case BIO_CTRL_DUP:
        {
            dbio = (BIO *)ptr;
            if (data->param_hostname)
                BIO_set_conn_hostname(dbio, data->param_hostname);
            if (data->param_service)
                BIO_set_conn_port(dbio, data->param_service);
            BIO_set_conn_ip_family(dbio, data->connect_family);
            BIO_set_conn_mode(dbio, data->connect_mode);
            /*
             * FIXME: the cast of the function seems unlikely to be a good
             * idea
             */
            (void)BIO_set_info_callback(dbio, data->info_callback);
        }
        break;
    case BIO_CTRL_SET_CALLBACK:
        ret = 0; /* use callback ctrl */
        break;
    case BIO_CTRL_GET_CALLBACK:
        {
            BIO_info_cb **fptr;

            fptr = (BIO_info_cb **)ptr;
            *fptr = data->info_callback;
        }
        break;
    case BIO_CTRL_EOF:
        ret = (b->flags & BIO_FLAGS_IN_EOF) != 0;
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
        data->record_type = num;
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
    default:
        ret = 0;
        break;
    }
    return ret;
}

static long conn_callback_ctrl(BIO *b, int cmd, BIO_info_cb *fp)
{
    long ret = 1;
    BIO_CONNECT *data;

    data = (BIO_CONNECT *)b->ptr;

    switch (cmd) {
    case BIO_CTRL_SET_CALLBACK:
        {
            data->info_callback = fp;
        }
        break;
    default:
        ret = 0;
        break;
    }
    return ret;
}

static int conn_puts(BIO *bp, const char *str)
{
    int n, ret;

    n = strlen(str);
    ret = conn_write(bp, str, n);
    return ret;
}

int conn_gets(BIO *bio, char *buf, int size)
{
    BIO_CONNECT *data;
    char *ptr = buf;
    int ret = 0;

    if (buf == NULL) {
        ERR_raise(ERR_LIB_BIO, ERR_R_PASSED_NULL_PARAMETER);
        return -1;
    }
    if (size <= 0) {
        ERR_raise(ERR_LIB_BIO, BIO_R_INVALID_ARGUMENT);
        return -1;
    }
    *buf = '\0';

    if (bio == NULL || bio->ptr == NULL) {
        ERR_raise(ERR_LIB_BIO, ERR_R_PASSED_NULL_PARAMETER);
        return -1;
    }
    data = (BIO_CONNECT *)bio->ptr;
    if (data->state != BIO_CONN_S_OK) {
        ret = conn_state(bio, data);
        if (ret <= 0)
            return ret;
    }

    if (data->dgram_bio != NULL) {
        ERR_raise(ERR_LIB_BIO, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return -1;
    }

    clear_socket_error();
    while (size-- > 1) {
# ifndef OPENSSL_NO_KTLS
        if (BIO_get_ktls_recv(bio))
            ret = ktls_read_record(bio->num, ptr, 1);
        else
# endif
            ret = readsocket(bio->num, ptr, 1);
        BIO_clear_retry_flags(bio);
        if (ret <= 0) {
            if (BIO_sock_should_retry(ret))
                BIO_set_retry_read(bio);
            else if (ret == 0)
                bio->flags |= BIO_FLAGS_IN_EOF;
            break;
        }
        if (*ptr++ == '\n')
            break;
    }
    *ptr = '\0';
    return ret > 0 || (bio->flags & BIO_FLAGS_IN_EOF) != 0 ? ptr - buf : ret;
}

static int conn_sendmmsg(BIO *bio, BIO_MSG *msg, size_t stride, size_t num_msgs,
                         uint64_t flags, size_t *msgs_processed)
{
    int ret;
    BIO_CONNECT *data;

    if (bio == NULL) {
        *msgs_processed = 0;
        ERR_raise(ERR_LIB_BIO, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    data = (BIO_CONNECT *)bio->ptr;
    if (data->state != BIO_CONN_S_OK) {
        ret = conn_state(bio, data);
        if (ret <= 0) {
            *msgs_processed = 0;
            return 0;
        }
    }

    if (data->dgram_bio == NULL) {
        *msgs_processed = 0;
        ERR_raise(ERR_LIB_BIO, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }

    return BIO_sendmmsg(data->dgram_bio, msg, stride, num_msgs,
                        flags, msgs_processed);
}

static int conn_recvmmsg(BIO *bio, BIO_MSG *msg, size_t stride, size_t num_msgs,
                         uint64_t flags, size_t *msgs_processed)
{
    int ret;
    BIO_CONNECT *data;

    if (bio == NULL) {
        *msgs_processed = 0;
        ERR_raise(ERR_LIB_BIO, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    data = (BIO_CONNECT *)bio->ptr;
    if (data->state != BIO_CONN_S_OK) {
        ret = conn_state(bio, data);
        if (ret <= 0) {
            *msgs_processed = 0;
            return 0;
        }
    }

    if (data->dgram_bio == NULL) {
        *msgs_processed = 0;
        ERR_raise(ERR_LIB_BIO, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }

    return BIO_recvmmsg(data->dgram_bio, msg, stride, num_msgs,
                        flags, msgs_processed);
}

BIO *BIO_new_connect(const char *str)
{
    BIO *ret;

    ret = BIO_new(BIO_s_connect());
    if (ret == NULL)
        return NULL;
    if (BIO_set_conn_hostname(ret, str))
        return ret;
    BIO_free(ret);
    return NULL;
}

#endif
