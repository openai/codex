/*
 * Copyright 2005-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef _GNU_SOURCE
# define _GNU_SOURCE
#endif

#include <stdio.h>
#include <errno.h>

#include "internal/time.h"
#include "bio_local.h"
#ifndef OPENSSL_NO_DGRAM

# ifndef OPENSSL_NO_SCTP
#  include <netinet/sctp.h>
#  include <fcntl.h>
#  define OPENSSL_SCTP_DATA_CHUNK_TYPE            0x00
#  define OPENSSL_SCTP_FORWARD_CUM_TSN_CHUNK_TYPE 0xc0
# endif

# if defined(OPENSSL_SYS_LINUX) && !defined(IP_MTU)
#  define IP_MTU      14        /* linux is lame */
# endif

# if OPENSSL_USE_IPV6 && !defined(IPPROTO_IPV6)
#  define IPPROTO_IPV6 41       /* windows is lame */
# endif

# if defined(__FreeBSD__) && defined(IN6_IS_ADDR_V4MAPPED)
/* Standard definition causes type-punning problems. */
#  undef IN6_IS_ADDR_V4MAPPED
#  define s6_addr32 __u6_addr.__u6_addr32
#  define IN6_IS_ADDR_V4MAPPED(a)               \
        (((a)->s6_addr32[0] == 0) &&          \
         ((a)->s6_addr32[1] == 0) &&          \
         ((a)->s6_addr32[2] == htonl(0x0000ffff)))
# endif

/* Determine what method to use for BIO_sendmmsg and BIO_recvmmsg. */
# define M_METHOD_NONE       0
# define M_METHOD_RECVMMSG   1
# define M_METHOD_RECVMSG    2
# define M_METHOD_RECVFROM   3
# define M_METHOD_WSARECVMSG 4

# if defined(__GLIBC__) && defined(__GLIBC_PREREQ)
#  if !(__GLIBC_PREREQ(2, 14))
#   undef NO_RECVMMSG
    /*
     * Some old glibc versions may have recvmmsg and MSG_WAITFORONE flag, but
     * not sendmmsg. We need both so force this to be disabled on these old
     * versions
     */
#   define NO_RECVMMSG
#  endif
# endif
# if defined(__GNU__)
   /* GNU/Hurd does not have IP_PKTINFO yet */
   #undef NO_RECVMSG
   #define NO_RECVMSG
# endif
# if (defined(__ANDROID_API__) && __ANDROID_API__ < 21) || defined(_AIX)
#  undef NO_RECVMMSG
#  define NO_RECVMMSG
# endif
# if !defined(M_METHOD)
#  if defined(OPENSSL_SYS_WINDOWS) && defined(BIO_HAVE_WSAMSG) && !defined(NO_WSARECVMSG)
#   define M_METHOD  M_METHOD_WSARECVMSG
#  elif !defined(OPENSSL_SYS_WINDOWS) && defined(MSG_WAITFORONE) && !defined(NO_RECVMMSG)
#   define M_METHOD  M_METHOD_RECVMMSG
#  elif !defined(OPENSSL_SYS_WINDOWS) && defined(CMSG_LEN) && !defined(NO_RECVMSG)
#   define M_METHOD  M_METHOD_RECVMSG
#  elif !defined(NO_RECVFROM)
#   define M_METHOD  M_METHOD_RECVFROM
#  else
#   define M_METHOD  M_METHOD_NONE
#  endif
# endif

# if defined(OPENSSL_SYS_WINDOWS)
#  define BIO_CMSG_SPACE(x) WSA_CMSG_SPACE(x)
#  define BIO_CMSG_FIRSTHDR(x) WSA_CMSG_FIRSTHDR(x)
#  define BIO_CMSG_NXTHDR(x, y) WSA_CMSG_NXTHDR(x, y)
#  define BIO_CMSG_DATA(x) WSA_CMSG_DATA(x)
#  define BIO_CMSG_LEN(x) WSA_CMSG_LEN(x)
#  define MSGHDR_TYPE WSAMSG
#  define CMSGHDR_TYPE WSACMSGHDR
# else
#  define MSGHDR_TYPE struct msghdr
#  define CMSGHDR_TYPE struct cmsghdr
#  define BIO_CMSG_SPACE(x) CMSG_SPACE(x)
#  define BIO_CMSG_FIRSTHDR(x) CMSG_FIRSTHDR(x)
#  define BIO_CMSG_NXTHDR(x, y) CMSG_NXTHDR(x, y)
#  define BIO_CMSG_DATA(x) CMSG_DATA(x)
#  define BIO_CMSG_LEN(x) CMSG_LEN(x)
# endif

# if   M_METHOD == M_METHOD_RECVMMSG   \
    || M_METHOD == M_METHOD_RECVMSG    \
    || M_METHOD == M_METHOD_WSARECVMSG
#  if defined(__APPLE__)
    /*
     * CMSG_SPACE is not a constant expression on OSX even though POSIX
     * says it's supposed to be. This should be adequate.
     */
#   define BIO_CMSG_ALLOC_LEN   64
#  else
#   if defined(IPV6_PKTINFO)
#     define BIO_CMSG_ALLOC_LEN_1   BIO_CMSG_SPACE(sizeof(struct in6_pktinfo))
#   else
#     define BIO_CMSG_ALLOC_LEN_1   0
#   endif
#   if defined(IP_PKTINFO)
#     define BIO_CMSG_ALLOC_LEN_2   BIO_CMSG_SPACE(sizeof(struct in_pktinfo))
#   else
#     define BIO_CMSG_ALLOC_LEN_2   0
#   endif
#   if defined(IP_RECVDSTADDR)
#     define BIO_CMSG_ALLOC_LEN_3   BIO_CMSG_SPACE(sizeof(struct in_addr))
#   else
#     define BIO_CMSG_ALLOC_LEN_3   0
#   endif
#   define BIO_MAX(X,Y) ((X) > (Y) ? (X) : (Y))
#   define BIO_CMSG_ALLOC_LEN                                        \
        BIO_MAX(BIO_CMSG_ALLOC_LEN_1,                                \
                BIO_MAX(BIO_CMSG_ALLOC_LEN_2, BIO_CMSG_ALLOC_LEN_3))
#  endif
#  if (defined(IP_PKTINFO) || defined(IP_RECVDSTADDR)) && defined(IPV6_RECVPKTINFO)
#   define SUPPORT_LOCAL_ADDR
#  endif
# endif

# define BIO_MSG_N(array, stride, n) (*(BIO_MSG *)((char *)(array) + (n)*(stride)))

static int dgram_write(BIO *h, const char *buf, int num);
static int dgram_read(BIO *h, char *buf, int size);
static int dgram_puts(BIO *h, const char *str);
static long dgram_ctrl(BIO *h, int cmd, long arg1, void *arg2);
static int dgram_new(BIO *h);
static int dgram_free(BIO *data);
static int dgram_clear(BIO *bio);
static int dgram_sendmmsg(BIO *b, BIO_MSG *msg,
                          size_t stride, size_t num_msg,
                          uint64_t flags, size_t *num_processed);
static int dgram_recvmmsg(BIO *b, BIO_MSG *msg,
                          size_t stride, size_t num_msg,
                          uint64_t flags, size_t *num_processed);

# ifndef OPENSSL_NO_SCTP
static int dgram_sctp_write(BIO *h, const char *buf, int num);
static int dgram_sctp_read(BIO *h, char *buf, int size);
static int dgram_sctp_puts(BIO *h, const char *str);
static long dgram_sctp_ctrl(BIO *h, int cmd, long arg1, void *arg2);
static int dgram_sctp_new(BIO *h);
static int dgram_sctp_free(BIO *data);
static int dgram_sctp_wait_for_dry(BIO *b);
static int dgram_sctp_msg_waiting(BIO *b);
#  ifdef SCTP_AUTHENTICATION_EVENT
static void dgram_sctp_handle_auth_free_key_event(BIO *b, union sctp_notification
                                                  *snp);
#  endif
# endif

static int BIO_dgram_should_retry(int s);

static const BIO_METHOD methods_dgramp = {
    BIO_TYPE_DGRAM,
    "datagram socket",
    bwrite_conv,
    dgram_write,
    bread_conv,
    dgram_read,
    dgram_puts,
    NULL,                       /* dgram_gets,         */
    dgram_ctrl,
    dgram_new,
    dgram_free,
    NULL,                       /* dgram_callback_ctrl */
    dgram_sendmmsg,
    dgram_recvmmsg,
};

# ifndef OPENSSL_NO_SCTP
static const BIO_METHOD methods_dgramp_sctp = {
    BIO_TYPE_DGRAM_SCTP,
    "datagram sctp socket",
    bwrite_conv,
    dgram_sctp_write,
    bread_conv,
    dgram_sctp_read,
    dgram_sctp_puts,
    NULL,                       /* dgram_gets,         */
    dgram_sctp_ctrl,
    dgram_sctp_new,
    dgram_sctp_free,
    NULL,                       /* dgram_callback_ctrl */
    NULL,                       /* sendmmsg */
    NULL,                       /* recvmmsg */
};
# endif

typedef struct bio_dgram_data_st {
    BIO_ADDR peer;
    BIO_ADDR local_addr;
    unsigned int connected;
    unsigned int _errno;
    unsigned int mtu;
    OSSL_TIME next_timeout;
    OSSL_TIME socket_timeout;
    unsigned int peekmode;
    char local_addr_enabled;
} bio_dgram_data;

# ifndef OPENSSL_NO_SCTP
typedef struct bio_dgram_sctp_save_message_st {
    BIO *bio;
    char *data;
    int length;
} bio_dgram_sctp_save_message;

/*
 * Note: bio_dgram_data must be first here
 * as we use dgram_ctrl for underlying dgram operations
 * which will cast this struct to a bio_dgram_data
 */
typedef struct bio_dgram_sctp_data_st {
    bio_dgram_data dgram;
    struct bio_dgram_sctp_sndinfo sndinfo;
    struct bio_dgram_sctp_rcvinfo rcvinfo;
    struct bio_dgram_sctp_prinfo prinfo;
    BIO_dgram_sctp_notification_handler_fn handle_notifications;
    void *notification_context;
    int in_handshake;
    int ccs_rcvd;
    int ccs_sent;
    int save_shutdown;
    int peer_auth_tested;
} bio_dgram_sctp_data;
# endif

const BIO_METHOD *BIO_s_datagram(void)
{
    return &methods_dgramp;
}

BIO *BIO_new_dgram(int fd, int close_flag)
{
    BIO *ret;

    ret = BIO_new(BIO_s_datagram());
    if (ret == NULL)
        return NULL;
    BIO_set_fd(ret, fd, close_flag);
    return ret;
}

static int dgram_new(BIO *bi)
{
    bio_dgram_data *data = OPENSSL_zalloc(sizeof(*data));

    if (data == NULL)
        return 0;
    bi->ptr = data;
    return 1;
}

static int dgram_free(BIO *a)
{
    bio_dgram_data *data;

    if (a == NULL)
        return 0;
    if (!dgram_clear(a))
        return 0;

    data = (bio_dgram_data *)a->ptr;
    OPENSSL_free(data);

    return 1;
}

static int dgram_clear(BIO *a)
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
    return 1;
}

static void dgram_adjust_rcv_timeout(BIO *b)
{
# if defined(SO_RCVTIMEO)
    bio_dgram_data *data = (bio_dgram_data *)b->ptr;
    OSSL_TIME timeleft;

    /* Is a timer active? */
    if (!ossl_time_is_zero(data->next_timeout)) {
        /* Read current socket timeout */
#  ifdef OPENSSL_SYS_WINDOWS
        int timeout;
        int sz = sizeof(timeout);

        if (getsockopt(b->num, SOL_SOCKET, SO_RCVTIMEO,
                       (void *)&timeout, &sz) < 0)
            ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                           "calling getsockopt()");
        else
            data->socket_timeout = ossl_ms2time(timeout);
#  else
        struct timeval tv;
        socklen_t sz = sizeof(tv);

        if (getsockopt(b->num, SOL_SOCKET, SO_RCVTIMEO, &tv, &sz) < 0)
            ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                           "calling getsockopt()");
        else
            data->socket_timeout = ossl_time_from_timeval(tv);
#  endif

        /* Calculate time left until timer expires */
        timeleft = ossl_time_subtract(data->next_timeout, ossl_time_now());
        if (ossl_time_compare(timeleft, ossl_ticks2time(OSSL_TIME_US)) < 0)
            timeleft = ossl_ticks2time(OSSL_TIME_US);

        /*
         * Adjust socket timeout if next handshake message timer will expire
         * earlier.
         */
        if (ossl_time_is_zero(data->socket_timeout)
            || ossl_time_compare(data->socket_timeout, timeleft) >= 0) {
#  ifdef OPENSSL_SYS_WINDOWS
            timeout = (int)ossl_time2ms(timeleft);
            if (setsockopt(b->num, SOL_SOCKET, SO_RCVTIMEO,
                           (void *)&timeout, sizeof(timeout)) < 0)
                ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                               "calling setsockopt()");
#  else
            tv = ossl_time_to_timeval(timeleft);
            if (setsockopt(b->num, SOL_SOCKET, SO_RCVTIMEO, &tv,
                           sizeof(tv)) < 0)
                ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                               "calling setsockopt()");
#  endif
        }
    }
# endif
}

static void dgram_update_local_addr(BIO *b)
{
    bio_dgram_data *data = (bio_dgram_data *)b->ptr;
    socklen_t addr_len = sizeof(data->local_addr);

    if (getsockname(b->num, &data->local_addr.sa, &addr_len) < 0)
        /*
         * This should not be possible, but zero-initialize and return
         * anyway.
         */
        BIO_ADDR_clear(&data->local_addr);
}

# if M_METHOD == M_METHOD_RECVMMSG || M_METHOD == M_METHOD_RECVMSG || M_METHOD == M_METHOD_WSARECVMSG
static int dgram_get_sock_family(BIO *b)
{
    bio_dgram_data *data = (bio_dgram_data *)b->ptr;
    return data->local_addr.sa.sa_family;
}
# endif

static void dgram_reset_rcv_timeout(BIO *b)
{
# if defined(SO_RCVTIMEO)
    bio_dgram_data *data = (bio_dgram_data *)b->ptr;

    /* Is a timer active? */
    if (!ossl_time_is_zero(data->next_timeout)) {
#  ifdef OPENSSL_SYS_WINDOWS
        int timeout = (int)ossl_time2ms(data->socket_timeout);

        if (setsockopt(b->num, SOL_SOCKET, SO_RCVTIMEO,
                       (void *)&timeout, sizeof(timeout)) < 0)
            ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                           "calling setsockopt()");
#  else
        struct timeval tv = ossl_time_to_timeval(data->socket_timeout);

        if (setsockopt(b->num, SOL_SOCKET, SO_RCVTIMEO, &tv, sizeof(tv)) < 0)
            ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                           "calling setsockopt()");
#  endif
    }
# endif
}

static int dgram_read(BIO *b, char *out, int outl)
{
    int ret = 0;
    bio_dgram_data *data = (bio_dgram_data *)b->ptr;
    int flags = 0;

    BIO_ADDR peer;
    socklen_t len = sizeof(peer);

    if (out != NULL) {
        clear_socket_error();
        BIO_ADDR_clear(&peer);
        dgram_adjust_rcv_timeout(b);
        if (data->peekmode)
            flags = MSG_PEEK;
        ret = recvfrom(b->num, out, outl, flags,
                       BIO_ADDR_sockaddr_noconst(&peer), &len);

        if (!data->connected && ret >= 0)
            BIO_ctrl(b, BIO_CTRL_DGRAM_SET_PEER, 0, &peer);

        BIO_clear_retry_flags(b);
        if (ret < 0) {
            if (BIO_dgram_should_retry(ret)) {
                BIO_set_retry_read(b);
                data->_errno = get_last_socket_error();
            }
        }

        dgram_reset_rcv_timeout(b);
    }
    return ret;
}

static int dgram_write(BIO *b, const char *in, int inl)
{
    int ret;
    bio_dgram_data *data = (bio_dgram_data *)b->ptr;
    clear_socket_error();

    if (data->connected)
        ret = writesocket(b->num, in, inl);
    else {
        int peerlen = BIO_ADDR_sockaddr_size(&data->peer);

        ret = sendto(b->num, in, inl, 0,
                     BIO_ADDR_sockaddr(&data->peer), peerlen);
    }

    BIO_clear_retry_flags(b);
    if (ret <= 0) {
        if (BIO_dgram_should_retry(ret)) {
            BIO_set_retry_write(b);
            data->_errno = get_last_socket_error();
        }
    }
    return ret;
}

static long dgram_get_mtu_overhead(bio_dgram_data *data)
{
    long ret;

    switch (BIO_ADDR_family(&data->peer)) {
    case AF_INET:
        /*
         * Assume this is UDP - 20 bytes for IP, 8 bytes for UDP
         */
        ret = 28;
        break;
# if OPENSSL_USE_IPV6
    case AF_INET6:
        {
#  ifdef IN6_IS_ADDR_V4MAPPED
            struct in6_addr tmp_addr;
            if (BIO_ADDR_rawaddress(&data->peer, &tmp_addr, NULL)
                && IN6_IS_ADDR_V4MAPPED(&tmp_addr))
                /*
                 * Assume this is UDP - 20 bytes for IP, 8 bytes for UDP
                 */
                ret = 28;
            else
#  endif
            /*
             * Assume this is UDP - 40 bytes for IP, 8 bytes for UDP
             */
            ret = 48;
        }
        break;
# endif
    default:
        /* We don't know. Go with the historical default */
        ret = 28;
        break;
    }
    return ret;
}

/* Enables appropriate destination address reception option on the socket. */
# if defined(SUPPORT_LOCAL_ADDR)
static int enable_local_addr(BIO *b, int enable) {
    int af = dgram_get_sock_family(b);

    if (af == AF_INET) {
#  if defined(IP_PKTINFO)
        /* IP_PKTINFO is preferred */
        if (setsockopt(b->num, IPPROTO_IP, IP_PKTINFO,
                       (void *)&enable, sizeof(enable)) < 0)
            return 0;

        return 1;

#  elif defined(IP_RECVDSTADDR)
        /* Fall back to IP_RECVDSTADDR */

        if (setsockopt(b->num, IPPROTO_IP, IP_RECVDSTADDR,
                       &enable, sizeof(enable)) < 0)
            return 0;

        return 1;
#  endif
    }

#  if OPENSSL_USE_IPV6
    if (af == AF_INET6) {
#   if defined(IPV6_RECVPKTINFO)
        if (setsockopt(b->num, IPPROTO_IPV6, IPV6_RECVPKTINFO,
                       &enable, sizeof(enable)) < 0)
            return 0;

        return 1;
#   endif
    }
#  endif

    return 0;
}
# endif

static long dgram_ctrl(BIO *b, int cmd, long num, void *ptr)
{
    long ret = 1;
    int *ip;
    bio_dgram_data *data = NULL;
# ifndef __DJGPP__
    /* There are currently no cases where this is used on djgpp/watt32. */
    int sockopt_val = 0;
# endif
    int d_errno;
# if defined(OPENSSL_SYS_LINUX) && (defined(IP_MTU_DISCOVER) || defined(IP_MTU))
    socklen_t sockopt_len;      /* assume that system supporting IP_MTU is
                                 * modern enough to define socklen_t */
    socklen_t addr_len;
    BIO_ADDR addr;
# endif
    struct sockaddr_storage ss;
    socklen_t ss_len = sizeof(ss);

    data = (bio_dgram_data *)b->ptr;

    switch (cmd) {
    case BIO_CTRL_RESET:
        num = 0;
        ret = 0;
        break;
    case BIO_CTRL_INFO:
        ret = 0;
        break;
    case BIO_C_SET_FD:
        dgram_clear(b);
        b->num = *((int *)ptr);
        b->shutdown = (int)num;
        b->init = 1;
        dgram_update_local_addr(b);
        if (getpeername(b->num, (struct sockaddr *)&ss, &ss_len) == 0) {
            BIO_ADDR_make(&data->peer, BIO_ADDR_sockaddr((BIO_ADDR *)&ss));
            data->connected = 1;
        }
# if defined(SUPPORT_LOCAL_ADDR)
        if (data->local_addr_enabled) {
            if (enable_local_addr(b, 1) < 1)
                data->local_addr_enabled = 0;
        }
# endif
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
    case BIO_CTRL_DUP:
    case BIO_CTRL_FLUSH:
        ret = 1;
        break;
    case BIO_CTRL_DGRAM_CONNECT:
        BIO_ADDR_make(&data->peer, BIO_ADDR_sockaddr((BIO_ADDR *)ptr));
        break;
        /* (Linux)kernel sets DF bit on outgoing IP packets */
    case BIO_CTRL_DGRAM_MTU_DISCOVER:
# if defined(OPENSSL_SYS_LINUX) && defined(IP_MTU_DISCOVER) && defined(IP_PMTUDISC_DO)
        addr_len = (socklen_t) sizeof(addr);
        BIO_ADDR_clear(&addr);
        if (getsockname(b->num, &addr.sa, &addr_len) < 0) {
            ret = 0;
            break;
        }
        switch (addr.sa.sa_family) {
        case AF_INET:
            sockopt_val = IP_PMTUDISC_DO;
            if ((ret = setsockopt(b->num, IPPROTO_IP, IP_MTU_DISCOVER,
                                  &sockopt_val, sizeof(sockopt_val))) < 0)
                ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                               "calling setsockopt()");
            break;
#  if OPENSSL_USE_IPV6 && defined(IPV6_MTU_DISCOVER) && defined(IPV6_PMTUDISC_DO)
        case AF_INET6:
            sockopt_val = IPV6_PMTUDISC_DO;
            if ((ret = setsockopt(b->num, IPPROTO_IPV6, IPV6_MTU_DISCOVER,
                                  &sockopt_val, sizeof(sockopt_val))) < 0)
                ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                               "calling setsockopt()");
            break;
#  endif
        default:
            ret = -1;
            break;
        }
# else
        ret = -1;
# endif
        break;
    case BIO_CTRL_DGRAM_QUERY_MTU:
# if defined(OPENSSL_SYS_LINUX) && defined(IP_MTU)
        addr_len = (socklen_t) sizeof(addr);
        BIO_ADDR_clear(&addr);
        if (getsockname(b->num, &addr.sa, &addr_len) < 0) {
            ret = 0;
            break;
        }
        sockopt_len = sizeof(sockopt_val);
        switch (addr.sa.sa_family) {
        case AF_INET:
            if ((ret =
                 getsockopt(b->num, IPPROTO_IP, IP_MTU, (void *)&sockopt_val,
                            &sockopt_len)) < 0 || sockopt_val < 0) {
                ret = 0;
            } else {
                /*
                 * we assume that the transport protocol is UDP and no IP
                 * options are used.
                 */
                data->mtu = sockopt_val - 8 - 20;
                ret = data->mtu;
            }
            break;
#  if OPENSSL_USE_IPV6 && defined(IPV6_MTU)
        case AF_INET6:
            if ((ret =
                 getsockopt(b->num, IPPROTO_IPV6, IPV6_MTU,
                            (void *)&sockopt_val, &sockopt_len)) < 0
                || sockopt_val < 0) {
                ret = 0;
            } else {
                /*
                 * we assume that the transport protocol is UDP and no IPV6
                 * options are used.
                 */
                data->mtu = sockopt_val - 8 - 40;
                ret = data->mtu;
            }
            break;
#  endif
        default:
            ret = 0;
            break;
        }
# else
        ret = 0;
# endif
        break;
    case BIO_CTRL_DGRAM_GET_FALLBACK_MTU:
        ret = -dgram_get_mtu_overhead(data);
        switch (BIO_ADDR_family(&data->peer)) {
        case AF_INET:
            ret += 576;
            break;
# if OPENSSL_USE_IPV6
        case AF_INET6:
            {
#  ifdef IN6_IS_ADDR_V4MAPPED
                struct in6_addr tmp_addr;
                if (BIO_ADDR_rawaddress(&data->peer, &tmp_addr, NULL)
                    && IN6_IS_ADDR_V4MAPPED(&tmp_addr))
                    ret += 576;
                else
#  endif
                    ret += 1280;
            }
            break;
# endif
        default:
            ret += 576;
            break;
        }
        break;
    case BIO_CTRL_DGRAM_GET_MTU:
        return data->mtu;
    case BIO_CTRL_DGRAM_SET_MTU:
        data->mtu = num;
        ret = num;
        break;
    case BIO_CTRL_DGRAM_SET_CONNECTED:
        if (ptr != NULL) {
            data->connected = 1;
            BIO_ADDR_make(&data->peer, BIO_ADDR_sockaddr((BIO_ADDR *)ptr));
        } else {
            data->connected = 0;
            BIO_ADDR_clear(&data->peer);
        }
        break;
    case BIO_CTRL_DGRAM_GET_PEER:
        ret = BIO_ADDR_sockaddr_size(&data->peer);
        /* FIXME: if num < ret, we will only return part of an address.
           That should bee an error, no? */
        if (num == 0 || num > ret)
            num = ret;
        memcpy(ptr, &data->peer, (ret = num));
        break;
    case BIO_CTRL_DGRAM_SET_PEER:
        BIO_ADDR_make(&data->peer, BIO_ADDR_sockaddr((BIO_ADDR *)ptr));
        break;
    case BIO_CTRL_DGRAM_DETECT_PEER_ADDR:
        {
            BIO_ADDR xaddr, *p = &data->peer;
            socklen_t xaddr_len = sizeof(xaddr.sa);

            if (BIO_ADDR_family(p) == AF_UNSPEC) {
                if (getpeername(b->num, (void *)&xaddr.sa, &xaddr_len) == 0
                    && BIO_ADDR_family(&xaddr) != AF_UNSPEC) {
                    p = &xaddr;
                } else {
                    ret = 0;
                    break;
                }
            }

            ret = BIO_ADDR_sockaddr_size(p);
            if (num == 0 || num > ret)
                num = ret;

            memcpy(ptr, p, (ret = num));
        }
        break;
    case BIO_C_SET_NBIO:
        if (!BIO_socket_nbio(b->num, num != 0))
            ret = 0;
        break;
    case BIO_CTRL_DGRAM_SET_NEXT_TIMEOUT:
        data->next_timeout = ossl_time_from_timeval(*(struct timeval *)ptr);
        break;
# if defined(SO_RCVTIMEO)
    case BIO_CTRL_DGRAM_SET_RECV_TIMEOUT:
#  ifdef OPENSSL_SYS_WINDOWS
        {
            struct timeval *tv = (struct timeval *)ptr;
            int timeout = tv->tv_sec * 1000 + tv->tv_usec / 1000;

            if ((ret = setsockopt(b->num, SOL_SOCKET, SO_RCVTIMEO,
                                  (void *)&timeout, sizeof(timeout))) < 0)
                ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                               "calling setsockopt()");
        }
#  else
        if ((ret = setsockopt(b->num, SOL_SOCKET, SO_RCVTIMEO, ptr,
                              sizeof(struct timeval))) < 0)
            ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                           "calling setsockopt()");
#  endif
        break;
    case BIO_CTRL_DGRAM_GET_RECV_TIMEOUT:
        {
#  ifdef OPENSSL_SYS_WINDOWS
            int sz = 0;
            int timeout;
            struct timeval *tv = (struct timeval *)ptr;

            sz = sizeof(timeout);
            if ((ret = getsockopt(b->num, SOL_SOCKET, SO_RCVTIMEO,
                                  (void *)&timeout, &sz)) < 0) {
                ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                               "calling getsockopt()");
            } else {
                tv->tv_sec = timeout / 1000;
                tv->tv_usec = (timeout % 1000) * 1000;
                ret = sizeof(*tv);
            }
#  else
            socklen_t sz = sizeof(struct timeval);
            if ((ret = getsockopt(b->num, SOL_SOCKET, SO_RCVTIMEO,
                                  ptr, &sz)) < 0) {
                ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                               "calling getsockopt()");
            } else {
                OPENSSL_assert((size_t)sz <= sizeof(struct timeval));
                ret = (int)sz;
            }
#  endif
        }
        break;
# endif
# if defined(SO_SNDTIMEO)
    case BIO_CTRL_DGRAM_SET_SEND_TIMEOUT:
#  ifdef OPENSSL_SYS_WINDOWS
        {
            struct timeval *tv = (struct timeval *)ptr;
            int timeout = tv->tv_sec * 1000 + tv->tv_usec / 1000;

            if ((ret = setsockopt(b->num, SOL_SOCKET, SO_SNDTIMEO,
                                  (void *)&timeout, sizeof(timeout))) < 0)
                ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                               "calling setsockopt()");
        }
#  else
        if ((ret = setsockopt(b->num, SOL_SOCKET, SO_SNDTIMEO, ptr,
                              sizeof(struct timeval))) < 0)
            ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                           "calling setsockopt()");
#  endif
        break;
    case BIO_CTRL_DGRAM_GET_SEND_TIMEOUT:
        {
#  ifdef OPENSSL_SYS_WINDOWS
            int sz = 0;
            int timeout;
            struct timeval *tv = (struct timeval *)ptr;

            sz = sizeof(timeout);
            if ((ret = getsockopt(b->num, SOL_SOCKET, SO_SNDTIMEO,
                                  (void *)&timeout, &sz)) < 0) {
                ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                               "calling getsockopt()");
            } else {
                tv->tv_sec = timeout / 1000;
                tv->tv_usec = (timeout % 1000) * 1000;
                ret = sizeof(*tv);
            }
#  else
            socklen_t sz = sizeof(struct timeval);

            if ((ret = getsockopt(b->num, SOL_SOCKET, SO_SNDTIMEO,
                                  ptr, &sz)) < 0) {
                ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                               "calling getsockopt()");
            } else {
                OPENSSL_assert((size_t)sz <= sizeof(struct timeval));
                ret = (int)sz;
            }
#  endif
        }
        break;
# endif
    case BIO_CTRL_DGRAM_GET_SEND_TIMER_EXP:
        /* fall-through */
    case BIO_CTRL_DGRAM_GET_RECV_TIMER_EXP:
# ifdef OPENSSL_SYS_WINDOWS
        d_errno = (data->_errno == WSAETIMEDOUT);
# else
        d_errno = (data->_errno == EAGAIN);
# endif
        if (d_errno) {
            ret = 1;
            data->_errno = 0;
        } else
            ret = 0;
        break;
# ifdef EMSGSIZE
    case BIO_CTRL_DGRAM_MTU_EXCEEDED:
        if (data->_errno == EMSGSIZE) {
            ret = 1;
            data->_errno = 0;
        } else
            ret = 0;
        break;
# endif
    case BIO_CTRL_DGRAM_SET_DONT_FRAG:
        switch (data->peer.sa.sa_family) {
        case AF_INET:
# if defined(IP_DONTFRAG)
            sockopt_val = num ? 1 : 0;
            if ((ret = setsockopt(b->num, IPPROTO_IP, IP_DONTFRAG,
                                  &sockopt_val, sizeof(sockopt_val))) < 0)
                ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                               "calling setsockopt()");
# elif defined(OPENSSL_SYS_LINUX) && defined(IP_MTU_DISCOVER) && defined (IP_PMTUDISC_PROBE)
            sockopt_val = num ? IP_PMTUDISC_PROBE : IP_PMTUDISC_DONT;
            if ((ret = setsockopt(b->num, IPPROTO_IP, IP_MTU_DISCOVER,
                                  &sockopt_val, sizeof(sockopt_val))) < 0)
                ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                               "calling setsockopt()");
# elif defined(OPENSSL_SYS_WINDOWS) && defined(IP_DONTFRAGMENT)
            sockopt_val = num ? 1 : 0;
            if ((ret = setsockopt(b->num, IPPROTO_IP, IP_DONTFRAGMENT,
                                  (const char *)&sockopt_val,
                                  sizeof(sockopt_val))) < 0)
                ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                               "calling setsockopt()");
# else
            ret = -1;
# endif
            break;
# if OPENSSL_USE_IPV6
        case AF_INET6:
#  if defined(IPV6_DONTFRAG)
            sockopt_val = num ? 1 : 0;
            if ((ret = setsockopt(b->num, IPPROTO_IPV6, IPV6_DONTFRAG,
                                  (const void *)&sockopt_val,
                                  sizeof(sockopt_val))) < 0)
                ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                               "calling setsockopt()");

#  elif defined(OPENSSL_SYS_LINUX) && defined(IPV6_MTUDISCOVER)
            sockopt_val = num ? IP_PMTUDISC_PROBE : IP_PMTUDISC_DONT;
            if ((ret = setsockopt(b->num, IPPROTO_IPV6, IPV6_MTU_DISCOVER,
                                  &sockopt_val, sizeof(sockopt_val))) < 0)
                ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                               "calling setsockopt()");
#  else
            ret = -1;
#  endif
            break;
# endif
        default:
            ret = -1;
            break;
        }
        break;
    case BIO_CTRL_DGRAM_GET_MTU_OVERHEAD:
        ret = dgram_get_mtu_overhead(data);
        break;

    /*
     * BIO_CTRL_DGRAM_SCTP_SET_IN_HANDSHAKE is used here for compatibility
     * reasons. When BIO_CTRL_DGRAM_SET_PEEK_MODE was first defined its value
     * was incorrectly clashing with BIO_CTRL_DGRAM_SCTP_SET_IN_HANDSHAKE. The
     * value has been updated to a non-clashing value. However to preserve
     * binary compatibility we now respond to both the old value and the new one
     */
    case BIO_CTRL_DGRAM_SCTP_SET_IN_HANDSHAKE:
    case BIO_CTRL_DGRAM_SET_PEEK_MODE:
        data->peekmode = (unsigned int)num;
        break;

    case BIO_CTRL_DGRAM_GET_LOCAL_ADDR_CAP:
# if defined(SUPPORT_LOCAL_ADDR)
        ret = 1;
# else
        ret = 0;
# endif
        break;

    case BIO_CTRL_DGRAM_SET_LOCAL_ADDR_ENABLE:
# if defined(SUPPORT_LOCAL_ADDR)
        num = num > 0;
        if (num != data->local_addr_enabled) {
            if (enable_local_addr(b, num) < 1) {
                ret = 0;
                break;
            }

            data->local_addr_enabled = (char)num;
        }
# else
        ret = 0;
# endif
        break;

    case BIO_CTRL_DGRAM_GET_LOCAL_ADDR_ENABLE:
        *(int *)ptr = data->local_addr_enabled;
        break;

    case BIO_CTRL_DGRAM_GET_EFFECTIVE_CAPS:
        ret = (long)(BIO_DGRAM_CAP_HANDLES_DST_ADDR
                     | BIO_DGRAM_CAP_HANDLES_SRC_ADDR
                     | BIO_DGRAM_CAP_PROVIDES_DST_ADDR
                     | BIO_DGRAM_CAP_PROVIDES_SRC_ADDR);
        break;

    case BIO_CTRL_GET_RPOLL_DESCRIPTOR:
    case BIO_CTRL_GET_WPOLL_DESCRIPTOR:
        {
            BIO_POLL_DESCRIPTOR *pd = ptr;

            pd->type        = BIO_POLL_DESCRIPTOR_TYPE_SOCK_FD;
            pd->value.fd    = b->num;
        }
        break;

    default:
        ret = 0;
        break;
    }
    /* Normalize if error */
    if (ret < 0)
        ret = -1;
    return ret;
}

static int dgram_puts(BIO *bp, const char *str)
{
    int n, ret;

    n = strlen(str);
    ret = dgram_write(bp, str, n);
    return ret;
}

# if M_METHOD == M_METHOD_WSARECVMSG
static void translate_msg_win(BIO *b, WSAMSG *mh, WSABUF *iov,
                              unsigned char *control, BIO_MSG *msg)
{
    iov->len = msg->data_len;
    iov->buf = msg->data;

    /* Windows requires namelen to be set exactly */
    mh->name = msg->peer != NULL ? &msg->peer->sa : NULL;
    if (msg->peer != NULL && dgram_get_sock_family(b) == AF_INET)
        mh->namelen = sizeof(struct sockaddr_in);
#  if OPENSSL_USE_IPV6
    else if (msg->peer != NULL && dgram_get_sock_family(b) == AF_INET6)
        mh->namelen = sizeof(struct sockaddr_in6);
#  endif
    else
        mh->namelen = 0;

    /*
     * When local address reception (IP_PKTINFO, etc.) is enabled, on Windows
     * this causes WSARecvMsg to fail if the control buffer is too small to hold
     * the structure, or if no control buffer is passed. So we need to give it
     * the control buffer even if we aren't actually going to examine the
     * result.
     */
    mh->lpBuffers       = iov;
    mh->dwBufferCount   = 1;
    mh->Control.len     = BIO_CMSG_ALLOC_LEN;
    mh->Control.buf     = control;
    mh->dwFlags         = 0;
}
# endif

# if M_METHOD == M_METHOD_RECVMMSG || M_METHOD == M_METHOD_RECVMSG
/* Translates a BIO_MSG to a msghdr and iovec. */
static void translate_msg(BIO *b, struct msghdr *mh, struct iovec *iov,
                          unsigned char *control, BIO_MSG *msg)
{
    bio_dgram_data *data;

    iov->iov_base = msg->data;
    iov->iov_len  = msg->data_len;

    data = (bio_dgram_data *)b->ptr;
    if (data->connected == 0) {
        /* macOS requires msg_namelen be 0 if msg_name is NULL */
        mh->msg_name = msg->peer != NULL ? &msg->peer->sa : NULL;
        if (msg->peer != NULL && dgram_get_sock_family(b) == AF_INET)
            mh->msg_namelen = sizeof(struct sockaddr_in);
#  if OPENSSL_USE_IPV6
        else if (msg->peer != NULL && dgram_get_sock_family(b) == AF_INET6)
            mh->msg_namelen = sizeof(struct sockaddr_in6);
#  endif
        else
            mh->msg_namelen = 0;
    } else {
        mh->msg_name = NULL;
        mh->msg_namelen = 0;
    }

    mh->msg_iov         = iov;
    mh->msg_iovlen      = 1;
    mh->msg_control     = msg->local != NULL ? control : NULL;
    mh->msg_controllen  = msg->local != NULL ? BIO_CMSG_ALLOC_LEN : 0;
    mh->msg_flags       = 0;
}
# endif

# if M_METHOD == M_METHOD_RECVMMSG || M_METHOD == M_METHOD_RECVMSG || M_METHOD == M_METHOD_WSARECVMSG
/* Extracts destination address from the control buffer. */
static int extract_local(BIO *b, MSGHDR_TYPE *mh, BIO_ADDR *local) {
#  if defined(IP_PKTINFO) || defined(IP_RECVDSTADDR) || defined(IPV6_PKTINFO)
    CMSGHDR_TYPE *cmsg;
    int af = dgram_get_sock_family(b);

    for (cmsg = BIO_CMSG_FIRSTHDR(mh); cmsg != NULL;
         cmsg = BIO_CMSG_NXTHDR(mh, cmsg)) {
        if (af == AF_INET) {
            if (cmsg->cmsg_level != IPPROTO_IP)
                continue;

#   if defined(IP_PKTINFO)
            if (cmsg->cmsg_type != IP_PKTINFO)
                continue;

            local->s_in.sin_addr =
                ((struct in_pktinfo *)BIO_CMSG_DATA(cmsg))->ipi_addr;

#   elif defined(IP_RECVDSTADDR)
            if (cmsg->cmsg_type != IP_RECVDSTADDR)
                continue;

            local->s_in.sin_addr = *(struct in_addr *)BIO_CMSG_DATA(cmsg);
#   endif

#   if defined(IP_PKTINFO) || defined(IP_RECVDSTADDR)
            {
                bio_dgram_data *data = b->ptr;

                local->s_in.sin_family = AF_INET;
                local->s_in.sin_port   = data->local_addr.s_in.sin_port;
            }
            return 1;
#   endif
        }
#   if OPENSSL_USE_IPV6
        else if (af == AF_INET6) {
            if (cmsg->cmsg_level != IPPROTO_IPV6)
                continue;

#    if defined(IPV6_RECVPKTINFO)
            if (cmsg->cmsg_type != IPV6_PKTINFO)
                continue;

            {
                bio_dgram_data *data = b->ptr;

                local->s_in6.sin6_addr     =
                    ((struct in6_pktinfo *)BIO_CMSG_DATA(cmsg))->ipi6_addr;
                local->s_in6.sin6_family   = AF_INET6;
                local->s_in6.sin6_port     = data->local_addr.s_in6.sin6_port;
                local->s_in6.sin6_scope_id =
                    data->local_addr.s_in6.sin6_scope_id;
                local->s_in6.sin6_flowinfo = 0;
            }
            return 1;
#    endif
        }
#   endif
    }
#  endif

    return 0;
}

static int pack_local(BIO *b, MSGHDR_TYPE *mh, const BIO_ADDR *local) {
    int af = dgram_get_sock_family(b);
#  if defined(IP_PKTINFO) || defined(IP_RECVDSTADDR) || defined(IPV6_PKTINFO)
    CMSGHDR_TYPE *cmsg;
    bio_dgram_data *data = b->ptr;
#  endif

    if (af == AF_INET) {
#  if defined(IP_PKTINFO)
        struct in_pktinfo *info;

#   if defined(OPENSSL_SYS_WINDOWS)
        cmsg = (CMSGHDR_TYPE *)mh->Control.buf;
#   else
        cmsg = (CMSGHDR_TYPE *)mh->msg_control;
#   endif

        cmsg->cmsg_len   = BIO_CMSG_LEN(sizeof(struct in_pktinfo));
        cmsg->cmsg_level = IPPROTO_IP;
        cmsg->cmsg_type  = IP_PKTINFO;

        info = (struct in_pktinfo *)BIO_CMSG_DATA(cmsg);
#   if !defined(OPENSSL_SYS_WINDOWS) && !defined(OPENSSL_SYS_CYGWIN) && !defined(__FreeBSD__) && !defined(__QNX__)
        info->ipi_spec_dst      = local->s_in.sin_addr;
#   endif
        info->ipi_addr.s_addr   = 0;
        info->ipi_ifindex       = 0;

        /*
         * We cannot override source port using this API, therefore
         * ensure the application specified a source port of 0
         * or the one we are bound to. (Better to error than silently
         * ignore this.)
         */
        if (local->s_in.sin_port != 0
            && data->local_addr.s_in.sin_port != local->s_in.sin_port) {
            ERR_raise(ERR_LIB_BIO, BIO_R_PORT_MISMATCH);
            return 0;
        }

#   if defined(OPENSSL_SYS_WINDOWS)
        mh->Control.len = BIO_CMSG_SPACE(sizeof(struct in_pktinfo));
#   else
        mh->msg_controllen = BIO_CMSG_SPACE(sizeof(struct in_pktinfo));
#   endif
        return 1;

#  elif defined(IP_SENDSRCADDR)
        struct in_addr *info;

        /*
         * At least FreeBSD is very pedantic about using IP_SENDSRCADDR when we
         * are not bound to 0.0.0.0 or ::, even if the address matches what we
         * bound to. Support this by not packing the structure if the address
         * matches our understanding of our local address. IP_SENDSRCADDR is a
         * BSD thing, so we don't need an explicit test for BSD here.
         */
        if (local->s_in.sin_addr.s_addr == data->local_addr.s_in.sin_addr.s_addr) {
            mh->msg_control    = NULL;
            mh->msg_controllen = 0;
            return 1;
        }

        cmsg = (struct cmsghdr *)mh->msg_control;
        cmsg->cmsg_len   = BIO_CMSG_LEN(sizeof(struct in_addr));
        cmsg->cmsg_level = IPPROTO_IP;
        cmsg->cmsg_type  = IP_SENDSRCADDR;

        info = (struct in_addr *)BIO_CMSG_DATA(cmsg);
        *info = local->s_in.sin_addr;

        /* See comment above. */
        if (local->s_in.sin_port != 0
            && data->local_addr.s_in.sin_port != local->s_in.sin_port) {
            ERR_raise(ERR_LIB_BIO, BIO_R_PORT_MISMATCH);
            return 0;
        }

        mh->msg_controllen = BIO_CMSG_SPACE(sizeof(struct in_addr));
        return 1;
#  endif
    }
#  if OPENSSL_USE_IPV6
    else if (af == AF_INET6) {
#   if defined(IPV6_PKTINFO)
        struct in6_pktinfo *info;

#    if defined(OPENSSL_SYS_WINDOWS)
        cmsg = (CMSGHDR_TYPE *)mh->Control.buf;
#    else
        cmsg = (CMSGHDR_TYPE *)mh->msg_control;
#    endif
        cmsg->cmsg_len   = BIO_CMSG_LEN(sizeof(struct in6_pktinfo));
        cmsg->cmsg_level = IPPROTO_IPV6;
        cmsg->cmsg_type  = IPV6_PKTINFO;

        info = (struct in6_pktinfo *)BIO_CMSG_DATA(cmsg);
        info->ipi6_addr     = local->s_in6.sin6_addr;
        info->ipi6_ifindex  = 0;

        /*
         * See comment above, but also applies to the other fields
         * in sockaddr_in6.
         */
        if (local->s_in6.sin6_port != 0
            && data->local_addr.s_in6.sin6_port != local->s_in6.sin6_port) {
            ERR_raise(ERR_LIB_BIO, BIO_R_PORT_MISMATCH);
            return 0;
        }

        if (local->s_in6.sin6_scope_id != 0
            && data->local_addr.s_in6.sin6_scope_id != local->s_in6.sin6_scope_id) {
            ERR_raise(ERR_LIB_BIO, BIO_R_PORT_MISMATCH);
            return 0;
        }

#    if defined(OPENSSL_SYS_WINDOWS)
        mh->Control.len = BIO_CMSG_SPACE(sizeof(struct in6_pktinfo));
#    else
        mh->msg_controllen = BIO_CMSG_SPACE(sizeof(struct in6_pktinfo));
#    endif
        return 1;
#   endif
    }
#  endif

    return 0;
}
# endif

/*
 * Converts flags passed to BIO_sendmmsg or BIO_recvmmsg to syscall flags. You
 * should mask out any system flags returned by this function you cannot support
 * in a particular circumstance. Currently no flags are defined.
 */
# if M_METHOD != M_METHOD_NONE
static int translate_flags(uint64_t flags) {
    return 0;
}
# endif

static int dgram_sendmmsg(BIO *b, BIO_MSG *msg, size_t stride,
                          size_t num_msg, uint64_t flags, size_t *num_processed)
{
# if M_METHOD != M_METHOD_NONE && M_METHOD != M_METHOD_RECVMSG
    int ret;
# endif
# if M_METHOD == M_METHOD_RECVMMSG
#  define BIO_MAX_MSGS_PER_CALL   64
    int sysflags;
    bio_dgram_data *data = (bio_dgram_data *)b->ptr;
    size_t i;
    struct mmsghdr mh[BIO_MAX_MSGS_PER_CALL];
    struct iovec iov[BIO_MAX_MSGS_PER_CALL];
    unsigned char control[BIO_MAX_MSGS_PER_CALL][BIO_CMSG_ALLOC_LEN];
    int have_local_enabled = data->local_addr_enabled;
# elif M_METHOD == M_METHOD_RECVMSG
    int sysflags;
    bio_dgram_data *data = (bio_dgram_data *)b->ptr;
    ossl_ssize_t l;
    struct msghdr mh;
    struct iovec iov;
    unsigned char control[BIO_CMSG_ALLOC_LEN];
    int have_local_enabled = data->local_addr_enabled;
# elif M_METHOD == M_METHOD_WSARECVMSG
    bio_dgram_data *data = (bio_dgram_data *)b->ptr;
    int have_local_enabled = data->local_addr_enabled;
    WSAMSG wmsg;
    WSABUF wbuf;
    DWORD num_bytes_sent = 0;
    unsigned char control[BIO_CMSG_ALLOC_LEN];
# endif
# if M_METHOD == M_METHOD_RECVFROM || M_METHOD == M_METHOD_WSARECVMSG
    int sysflags;
# endif

    if (num_msg == 0) {
        *num_processed = 0;
        return 1;
    }

    if (num_msg > OSSL_SSIZE_MAX)
        num_msg = OSSL_SSIZE_MAX;

# if M_METHOD != M_METHOD_NONE
    sysflags = translate_flags(flags);
# endif

# if M_METHOD == M_METHOD_RECVMMSG
    /*
     * In the sendmmsg/recvmmsg case, we need to allocate our translated struct
     * msghdr and struct iovec on the stack to support multithreaded use. Thus
     * we place a fixed limit on the number of messages per call, in the
     * expectation that we will be called again if there were more messages to
     * be sent.
     */
    if (num_msg > BIO_MAX_MSGS_PER_CALL)
        num_msg = BIO_MAX_MSGS_PER_CALL;

    for (i = 0; i < num_msg; ++i) {
        translate_msg(b, &mh[i].msg_hdr, &iov[i],
                      control[i], &BIO_MSG_N(msg, stride, i));

        /* If local address was requested, it must have been enabled */
        if (BIO_MSG_N(msg, stride, i).local != NULL) {
            if (!have_local_enabled) {
                ERR_raise(ERR_LIB_BIO, BIO_R_LOCAL_ADDR_NOT_AVAILABLE);
                *num_processed = 0;
                return 0;
            }

            if (pack_local(b, &mh[i].msg_hdr,
                           BIO_MSG_N(msg, stride, i).local) < 1) {
                ERR_raise(ERR_LIB_BIO, BIO_R_LOCAL_ADDR_NOT_AVAILABLE);
                *num_processed = 0;
                return 0;
            }
        }
    }

    /* Do the batch */
    ret = sendmmsg(b->num, mh, num_msg, sysflags);
    if (ret < 0) {
        ERR_raise(ERR_LIB_SYS, get_last_socket_error());
        *num_processed = 0;
        return 0;
    }

    for (i = 0; i < (size_t)ret; ++i) {
        BIO_MSG_N(msg, stride, i).data_len = mh[i].msg_len;
        BIO_MSG_N(msg, stride, i).flags    = 0;
    }

    *num_processed = (size_t)ret;
    return 1;

# elif M_METHOD == M_METHOD_RECVMSG
    /*
     * If sendmsg is available, use it.
     */
    translate_msg(b, &mh, &iov, control, msg);

    if (msg->local != NULL) {
        if (!have_local_enabled) {
            ERR_raise(ERR_LIB_BIO, BIO_R_LOCAL_ADDR_NOT_AVAILABLE);
            *num_processed = 0;
            return 0;
        }

        if (pack_local(b, &mh, msg->local) < 1) {
            ERR_raise(ERR_LIB_BIO, BIO_R_LOCAL_ADDR_NOT_AVAILABLE);
            *num_processed = 0;
            return 0;
        }
    }

    l = sendmsg(b->num, &mh, sysflags);
    if (l < 0) {
        ERR_raise(ERR_LIB_SYS, get_last_socket_error());
        *num_processed = 0;
        return 0;
    }

    msg->data_len   = (size_t)l;
    msg->flags      = 0;
    *num_processed  = 1;
    return 1;

# elif M_METHOD == M_METHOD_WSARECVMSG || M_METHOD == M_METHOD_RECVFROM
#  if M_METHOD == M_METHOD_WSARECVMSG
    if (bio_WSASendMsg != NULL) {
        /* WSASendMsg-based implementation for Windows. */
        translate_msg_win(b, &wmsg, &wbuf, control, msg);

        if (msg[0].local != NULL) {
            if (!have_local_enabled) {
                ERR_raise(ERR_LIB_BIO, BIO_R_LOCAL_ADDR_NOT_AVAILABLE);
                *num_processed = 0;
                return 0;
            }

            if (pack_local(b, &wmsg, msg[0].local) < 1) {
                ERR_raise(ERR_LIB_BIO, BIO_R_LOCAL_ADDR_NOT_AVAILABLE);
                *num_processed = 0;
                return 0;
            }
        }

        ret = WSASendMsg((SOCKET)b->num, &wmsg, 0, &num_bytes_sent, NULL, NULL);
        if (ret < 0) {
            ERR_raise(ERR_LIB_SYS, get_last_socket_error());
            *num_processed = 0;
            return 0;
        }

        msg[0].data_len = num_bytes_sent;
        msg[0].flags    = 0;
        *num_processed  = 1;
        return 1;
    }
#  endif

    /*
     * Fallback to sendto and send a single message.
     */
    if (msg[0].local != NULL) {
        /*
         * We cannot set the local address if using sendto
         * so fail in this case
         */
        ERR_raise(ERR_LIB_BIO, BIO_R_LOCAL_ADDR_NOT_AVAILABLE);
        *num_processed = 0;
        return 0;
    }

    ret = sendto(b->num, msg[0].data,
#  if defined(OPENSSL_SYS_WINDOWS)
                 (int)msg[0].data_len,
#  else
                 msg[0].data_len,
#  endif
                 sysflags,
                 msg[0].peer != NULL ? BIO_ADDR_sockaddr(msg[0].peer) : NULL,
                 msg[0].peer != NULL ? BIO_ADDR_sockaddr_size(msg[0].peer) : 0);
    if (ret <= 0) {
        ERR_raise(ERR_LIB_SYS, get_last_socket_error());
        *num_processed = 0;
        return 0;
    }

    msg[0].data_len = ret;
    msg[0].flags    = 0;
    *num_processed  = 1;
    return 1;

# else
    ERR_raise(ERR_LIB_BIO, BIO_R_UNSUPPORTED_METHOD);
    *num_processed = 0;
    return 0;
# endif
}

static int dgram_recvmmsg(BIO *b, BIO_MSG *msg,
                          size_t stride, size_t num_msg,
                          uint64_t flags, size_t *num_processed)
{
# if M_METHOD != M_METHOD_NONE && M_METHOD != M_METHOD_RECVMSG
    int ret;
# endif
# if M_METHOD == M_METHOD_RECVMMSG
    int sysflags;
    bio_dgram_data *data = (bio_dgram_data *)b->ptr;
    size_t i;
    struct mmsghdr mh[BIO_MAX_MSGS_PER_CALL];
    struct iovec iov[BIO_MAX_MSGS_PER_CALL];
    unsigned char control[BIO_MAX_MSGS_PER_CALL][BIO_CMSG_ALLOC_LEN];
    int have_local_enabled = data->local_addr_enabled;
# elif M_METHOD == M_METHOD_RECVMSG
    int sysflags;
    bio_dgram_data *data = (bio_dgram_data *)b->ptr;
    ossl_ssize_t l;
    struct msghdr mh;
    struct iovec iov;
    unsigned char control[BIO_CMSG_ALLOC_LEN];
    int have_local_enabled = data->local_addr_enabled;
# elif M_METHOD == M_METHOD_WSARECVMSG
    bio_dgram_data *data = (bio_dgram_data *)b->ptr;
    int have_local_enabled = data->local_addr_enabled;
    WSAMSG wmsg;
    WSABUF wbuf;
    DWORD num_bytes_received = 0;
    unsigned char control[BIO_CMSG_ALLOC_LEN];
# endif
# if M_METHOD == M_METHOD_RECVFROM || M_METHOD == M_METHOD_WSARECVMSG
    int sysflags;
    socklen_t slen;
# endif

    if (num_msg == 0) {
        *num_processed = 0;
        return 1;
    }

    if (num_msg > OSSL_SSIZE_MAX)
        num_msg = OSSL_SSIZE_MAX;

# if M_METHOD != M_METHOD_NONE
    sysflags = translate_flags(flags);
# endif

# if M_METHOD == M_METHOD_RECVMMSG
    /*
     * In the sendmmsg/recvmmsg case, we need to allocate our translated struct
     * msghdr and struct iovec on the stack to support multithreaded use. Thus
     * we place a fixed limit on the number of messages per call, in the
     * expectation that we will be called again if there were more messages to
     * be sent.
     */
    if (num_msg > BIO_MAX_MSGS_PER_CALL)
        num_msg = BIO_MAX_MSGS_PER_CALL;

    for (i = 0; i < num_msg; ++i) {
        translate_msg(b, &mh[i].msg_hdr, &iov[i],
                      control[i], &BIO_MSG_N(msg, stride, i));

        /* If local address was requested, it must have been enabled */
        if (BIO_MSG_N(msg, stride, i).local != NULL && !have_local_enabled) {
            ERR_raise(ERR_LIB_BIO, BIO_R_LOCAL_ADDR_NOT_AVAILABLE);
            *num_processed = 0;
            return 0;
        }
    }

    /* Do the batch */
    ret = recvmmsg(b->num, mh, num_msg, sysflags, NULL);
    if (ret < 0) {
        ERR_raise(ERR_LIB_SYS, get_last_socket_error());
        *num_processed = 0;
        return 0;
    }

    for (i = 0; i < (size_t)ret; ++i) {
        BIO_MSG_N(msg, stride, i).data_len = mh[i].msg_len;
        BIO_MSG_N(msg, stride, i).flags    = 0;
        /*
         * *(msg->peer) will have been filled in by recvmmsg;
         * for msg->local we parse the control data returned
         */
        if (BIO_MSG_N(msg, stride, i).local != NULL)
            if (extract_local(b, &mh[i].msg_hdr,
                              BIO_MSG_N(msg, stride, i).local) < 1)
                /*
                 * It appears BSDs do not support local addresses for
                 * loopback sockets. In this case, just clear the local
                 * address, as for OS X and Windows in some circumstances
                 * (see below).
                 */
                BIO_ADDR_clear(msg->local);
    }

    *num_processed = (size_t)ret;
    return 1;

# elif M_METHOD == M_METHOD_RECVMSG
    /*
     * If recvmsg is available, use it.
     */
    translate_msg(b, &mh, &iov, control, msg);

    /* If local address was requested, it must have been enabled */
    if (msg->local != NULL && !have_local_enabled) {
        /*
         * If we have done at least one message, we must return the
         * count; if we haven't done any, we can give an error code
         */
        ERR_raise(ERR_LIB_BIO, BIO_R_LOCAL_ADDR_NOT_AVAILABLE);
        *num_processed = 0;
        return 0;
    }

    l = recvmsg(b->num, &mh, sysflags);
    if (l < 0) {
        ERR_raise(ERR_LIB_SYS, get_last_socket_error());
        *num_processed = 0;
        return 0;
    }

    msg->data_len   = (size_t)l;
    msg->flags      = 0;

    if (msg->local != NULL)
        if (extract_local(b, &mh, msg->local) < 1)
            /*
             * OS X exhibits odd behaviour where it appears that if a packet is
             * sent before the receiving interface enables IP_PKTINFO, it will
             * sometimes not have any control data returned even if the
             * receiving interface enables IP_PKTINFO before calling recvmsg().
             * This appears to occur non-deterministically. Presumably, OS X
             * handles IP_PKTINFO at the time the packet is enqueued into a
             * socket's receive queue, rather than at the time recvmsg() is
             * called, unlike most other operating systems. Thus (if this
             * hypothesis is correct) there is a race between where IP_PKTINFO
             * is enabled by the process and when the kernel's network stack
             * queues the incoming message.
             *
             * We cannot return the local address if we do not have it, but this
             * is not a caller error either, so just return a zero address
             * structure. This is similar to how we handle Windows loopback
             * interfaces (see below). We enable this workaround for all
             * platforms, not just Apple, as this kind of quirk in OS networking
             * stacks seems to be common enough that failing hard if a local
             * address is not provided appears to be too brittle.
             */
            BIO_ADDR_clear(msg->local);

    *num_processed = 1;
    return 1;

# elif M_METHOD == M_METHOD_RECVFROM || M_METHOD == M_METHOD_WSARECVMSG
#  if M_METHOD == M_METHOD_WSARECVMSG
    if (bio_WSARecvMsg != NULL) {
        /* WSARecvMsg-based implementation for Windows. */
        translate_msg_win(b, &wmsg, &wbuf, control, msg);

        /* If local address was requested, it must have been enabled */
        if (msg[0].local != NULL && !have_local_enabled) {
            ERR_raise(ERR_LIB_BIO, BIO_R_LOCAL_ADDR_NOT_AVAILABLE);
            *num_processed = 0;
            return 0;
        }

        ret = WSARecvMsg((SOCKET)b->num, &wmsg, &num_bytes_received, NULL, NULL);
        if (ret < 0) {
            ERR_raise(ERR_LIB_SYS, get_last_socket_error());
            *num_processed = 0;
            return 0;
        }

        msg[0].data_len = num_bytes_received;
        msg[0].flags    = 0;
        if (msg[0].local != NULL)
            if (extract_local(b, &wmsg, msg[0].local) < 1)
                /*
                 * On Windows, loopback is not a "proper" interface and it works
                 * differently; packets are essentially short-circuited and
                 * don't go through all of the normal processing. A consequence
                 * of this is that packets sent from the local machine to the
                 * local machine _will not have IP_PKTINFO_ even if the
                 * IP_PKTINFO socket option is enabled. WSARecvMsg just sets
                 * Control.len to 0 on returning.
                 *
                 * This applies regardless of whether the loopback address,
                 * 127.0.0.1 is used, or a local interface address (e.g.
                 * 192.168.1.1); in both cases IP_PKTINFO will not be present.
                 *
                 * We report this condition by setting the local BIO_ADDR's
                 * family to 0.
                 */
                BIO_ADDR_clear(msg[0].local);

        *num_processed = 1;
        return 1;
    }
#  endif

    /*
     * Fallback to recvfrom and receive a single message.
     */
    if (msg[0].local != NULL) {
        /*
         * We cannot determine the local address if using recvfrom
         * so fail in this case
         */
        ERR_raise(ERR_LIB_BIO, BIO_R_LOCAL_ADDR_NOT_AVAILABLE);
        *num_processed = 0;
        return 0;
    }

    slen = sizeof(*msg[0].peer);
    ret = recvfrom(b->num, msg[0].data,
#  if defined(OPENSSL_SYS_WINDOWS)
                   (int)msg[0].data_len,
#  else
                   msg[0].data_len,
#  endif
                   sysflags,
                   msg[0].peer != NULL ? &msg[0].peer->sa : NULL,
                   msg[0].peer != NULL ? &slen : NULL);
    if (ret <= 0) {
        ERR_raise(ERR_LIB_SYS, get_last_socket_error());
        return 0;
    }

    msg[0].data_len = ret;
    msg[0].flags    = 0;
    *num_processed = 1;
    return 1;

# else
    ERR_raise(ERR_LIB_BIO, BIO_R_UNSUPPORTED_METHOD);
    *num_processed = 0;
    return 0;
# endif
}

# ifndef OPENSSL_NO_SCTP
const BIO_METHOD *BIO_s_datagram_sctp(void)
{
    return &methods_dgramp_sctp;
}

BIO *BIO_new_dgram_sctp(int fd, int close_flag)
{
    BIO *bio;
    int ret, optval = 20000;
    int auth_data = 0, auth_forward = 0;
    unsigned char *p;
    struct sctp_authchunk auth;
    struct sctp_authchunks *authchunks;
    socklen_t sockopt_len;
#  ifdef SCTP_AUTHENTICATION_EVENT
#   ifdef SCTP_EVENT
    struct sctp_event event;
#   else
    struct sctp_event_subscribe event;
#   endif
#  endif

    bio = BIO_new(BIO_s_datagram_sctp());
    if (bio == NULL)
        return NULL;
    BIO_set_fd(bio, fd, close_flag);

    /* Activate SCTP-AUTH for DATA and FORWARD-TSN chunks */
    auth.sauth_chunk = OPENSSL_SCTP_DATA_CHUNK_TYPE;
    ret =
        setsockopt(fd, IPPROTO_SCTP, SCTP_AUTH_CHUNK, &auth,
                   sizeof(struct sctp_authchunk));
    if (ret < 0) {
        BIO_vfree(bio);
        ERR_raise_data(ERR_LIB_BIO, ERR_R_SYS_LIB,
                       "Ensure SCTP AUTH chunks are enabled in kernel");
        return NULL;
    }
    auth.sauth_chunk = OPENSSL_SCTP_FORWARD_CUM_TSN_CHUNK_TYPE;
    ret =
        setsockopt(fd, IPPROTO_SCTP, SCTP_AUTH_CHUNK, &auth,
                   sizeof(struct sctp_authchunk));
    if (ret < 0) {
        BIO_vfree(bio);
        ERR_raise_data(ERR_LIB_BIO, ERR_R_SYS_LIB,
                       "Ensure SCTP AUTH chunks are enabled in kernel");
        return NULL;
    }

    /*
     * Test if activation was successful. When using accept(), SCTP-AUTH has
     * to be activated for the listening socket already, otherwise the
     * connected socket won't use it. Similarly with connect(): the socket
     * prior to connection must be activated for SCTP-AUTH
     */
    sockopt_len = (socklen_t) (sizeof(sctp_assoc_t) + 256 * sizeof(uint8_t));
    authchunks = OPENSSL_zalloc(sockopt_len);
    if (authchunks == NULL) {
        BIO_vfree(bio);
        return NULL;
    }
    ret = getsockopt(fd, IPPROTO_SCTP, SCTP_LOCAL_AUTH_CHUNKS, authchunks,
                   &sockopt_len);
    if (ret < 0) {
        OPENSSL_free(authchunks);
        BIO_vfree(bio);
        return NULL;
    }

    for (p = (unsigned char *)authchunks->gauth_chunks;
         p < (unsigned char *)authchunks + sockopt_len;
         p += sizeof(uint8_t)) {
        if (*p == OPENSSL_SCTP_DATA_CHUNK_TYPE)
            auth_data = 1;
        if (*p == OPENSSL_SCTP_FORWARD_CUM_TSN_CHUNK_TYPE)
            auth_forward = 1;
    }

    OPENSSL_free(authchunks);

    if (!auth_data || !auth_forward) {
        BIO_vfree(bio);
        ERR_raise_data(ERR_LIB_BIO, ERR_R_SYS_LIB,
                       "Ensure SCTP AUTH chunks are enabled on the "
                       "underlying socket");
        return NULL;
    }

#  ifdef SCTP_AUTHENTICATION_EVENT
#   ifdef SCTP_EVENT
    memset(&event, 0, sizeof(event));
    event.se_assoc_id = 0;
    event.se_type = SCTP_AUTHENTICATION_EVENT;
    event.se_on = 1;
    ret =
        setsockopt(fd, IPPROTO_SCTP, SCTP_EVENT, &event,
                   sizeof(struct sctp_event));
    if (ret < 0) {
        BIO_vfree(bio);
        return NULL;
    }
#   else
    sockopt_len = (socklen_t) sizeof(struct sctp_event_subscribe);
    ret = getsockopt(fd, IPPROTO_SCTP, SCTP_EVENTS, &event, &sockopt_len);
    if (ret < 0) {
        BIO_vfree(bio);
        return NULL;
    }

    event.sctp_authentication_event = 1;

    ret =
        setsockopt(fd, IPPROTO_SCTP, SCTP_EVENTS, &event,
                   sizeof(struct sctp_event_subscribe));
    if (ret < 0) {
        BIO_vfree(bio);
        return NULL;
    }
#   endif
#  endif

    /*
     * Disable partial delivery by setting the min size larger than the max
     * record size of 2^14 + 2048 + 13
     */
    ret =
        setsockopt(fd, IPPROTO_SCTP, SCTP_PARTIAL_DELIVERY_POINT, &optval,
                   sizeof(optval));
    if (ret < 0) {
        BIO_vfree(bio);
        return NULL;
    }

    return bio;
}

int BIO_dgram_is_sctp(BIO *bio)
{
    return (BIO_method_type(bio) == BIO_TYPE_DGRAM_SCTP);
}

static int dgram_sctp_new(BIO *bi)
{
    bio_dgram_sctp_data *data = NULL;

    bi->init = 0;
    bi->num = 0;
    if ((data = OPENSSL_zalloc(sizeof(*data))) == NULL)
        return 0;
#  ifdef SCTP_PR_SCTP_NONE
    data->prinfo.pr_policy = SCTP_PR_SCTP_NONE;
#  endif
    bi->ptr = data;

    bi->flags = 0;
    return 1;
}

static int dgram_sctp_free(BIO *a)
{
    bio_dgram_sctp_data *data;

    if (a == NULL)
        return 0;
    if (!dgram_clear(a))
        return 0;

    data = (bio_dgram_sctp_data *) a->ptr;
    if (data != NULL)
        OPENSSL_free(data);

    return 1;
}

#  ifdef SCTP_AUTHENTICATION_EVENT
void dgram_sctp_handle_auth_free_key_event(BIO *b,
                                           union sctp_notification *snp)
{
    int ret;
    struct sctp_authkey_event *authkeyevent = &snp->sn_auth_event;

    if (authkeyevent->auth_indication == SCTP_AUTH_FREE_KEY) {
        struct sctp_authkeyid authkeyid;

        /* delete key */
        authkeyid.scact_keynumber = authkeyevent->auth_keynumber;
        ret = setsockopt(b->num, IPPROTO_SCTP, SCTP_AUTH_DELETE_KEY,
                         &authkeyid, sizeof(struct sctp_authkeyid));
    }
}
#  endif

static int dgram_sctp_read(BIO *b, char *out, int outl)
{
    int ret = 0, n = 0, i, optval;
    socklen_t optlen;
    bio_dgram_sctp_data *data = (bio_dgram_sctp_data *) b->ptr;
    struct msghdr msg;
    struct iovec iov;
    struct cmsghdr *cmsg;
    char cmsgbuf[512];

    if (out != NULL) {
        clear_socket_error();

        do {
            memset(&data->rcvinfo, 0, sizeof(data->rcvinfo));
            iov.iov_base = out;
            iov.iov_len = outl;
            msg.msg_name = NULL;
            msg.msg_namelen = 0;
            msg.msg_iov = &iov;
            msg.msg_iovlen = 1;
            msg.msg_control = cmsgbuf;
            msg.msg_controllen = 512;
            msg.msg_flags = 0;
            n = recvmsg(b->num, &msg, 0);

            if (n <= 0) {
                if (n < 0)
                    ret = n;
                break;
            }

            if (msg.msg_controllen > 0) {
                for (cmsg = CMSG_FIRSTHDR(&msg); cmsg;
                     cmsg = CMSG_NXTHDR(&msg, cmsg)) {
                    if (cmsg->cmsg_level != IPPROTO_SCTP)
                        continue;
#  ifdef SCTP_RCVINFO
                    if (cmsg->cmsg_type == SCTP_RCVINFO) {
                        struct sctp_rcvinfo *rcvinfo;

                        rcvinfo = (struct sctp_rcvinfo *)CMSG_DATA(cmsg);
                        data->rcvinfo.rcv_sid = rcvinfo->rcv_sid;
                        data->rcvinfo.rcv_ssn = rcvinfo->rcv_ssn;
                        data->rcvinfo.rcv_flags = rcvinfo->rcv_flags;
                        data->rcvinfo.rcv_ppid = rcvinfo->rcv_ppid;
                        data->rcvinfo.rcv_tsn = rcvinfo->rcv_tsn;
                        data->rcvinfo.rcv_cumtsn = rcvinfo->rcv_cumtsn;
                        data->rcvinfo.rcv_context = rcvinfo->rcv_context;
                    }
#  endif
#  ifdef SCTP_SNDRCV
                    if (cmsg->cmsg_type == SCTP_SNDRCV) {
                        struct sctp_sndrcvinfo *sndrcvinfo;

                        sndrcvinfo =
                            (struct sctp_sndrcvinfo *)CMSG_DATA(cmsg);
                        data->rcvinfo.rcv_sid = sndrcvinfo->sinfo_stream;
                        data->rcvinfo.rcv_ssn = sndrcvinfo->sinfo_ssn;
                        data->rcvinfo.rcv_flags = sndrcvinfo->sinfo_flags;
                        data->rcvinfo.rcv_ppid = sndrcvinfo->sinfo_ppid;
                        data->rcvinfo.rcv_tsn = sndrcvinfo->sinfo_tsn;
                        data->rcvinfo.rcv_cumtsn = sndrcvinfo->sinfo_cumtsn;
                        data->rcvinfo.rcv_context = sndrcvinfo->sinfo_context;
                    }
#  endif
                }
            }

            if (msg.msg_flags & MSG_NOTIFICATION) {
                union sctp_notification snp;

                memcpy(&snp, out, sizeof(snp));
                if (snp.sn_header.sn_type == SCTP_SENDER_DRY_EVENT) {
#  ifdef SCTP_EVENT
                    struct sctp_event event;
#  else
                    struct sctp_event_subscribe event;
                    socklen_t eventsize;
#  endif

                    /* disable sender dry event */
#  ifdef SCTP_EVENT
                    memset(&event, 0, sizeof(event));
                    event.se_assoc_id = 0;
                    event.se_type = SCTP_SENDER_DRY_EVENT;
                    event.se_on = 0;
                    i = setsockopt(b->num, IPPROTO_SCTP, SCTP_EVENT, &event,
                                   sizeof(struct sctp_event));
                    if (i < 0) {
                        ret = i;
                        break;
                    }
#  else
                    eventsize = sizeof(struct sctp_event_subscribe);
                    i = getsockopt(b->num, IPPROTO_SCTP, SCTP_EVENTS, &event,
                                   &eventsize);
                    if (i < 0) {
                        ret = i;
                        break;
                    }

                    event.sctp_sender_dry_event = 0;

                    i = setsockopt(b->num, IPPROTO_SCTP, SCTP_EVENTS, &event,
                                   sizeof(struct sctp_event_subscribe));
                    if (i < 0) {
                        ret = i;
                        break;
                    }
#  endif
                }
#  ifdef SCTP_AUTHENTICATION_EVENT
                if (snp.sn_header.sn_type == SCTP_AUTHENTICATION_EVENT)
                    dgram_sctp_handle_auth_free_key_event(b, &snp);
#  endif

                if (data->handle_notifications != NULL)
                    data->handle_notifications(b, data->notification_context,
                                               (void *)out);

                memset(&snp, 0, sizeof(snp));
                memset(out, 0, outl);
            } else {
                ret += n;
            }
        }
        while ((msg.msg_flags & MSG_NOTIFICATION) && (msg.msg_flags & MSG_EOR)
               && (ret < outl));

        if (ret > 0 && !(msg.msg_flags & MSG_EOR)) {
            /* Partial message read, this should never happen! */

            /*
             * The buffer was too small, this means the peer sent a message
             * that was larger than allowed.
             */
            if (ret == outl)
                return -1;

            /*
             * Test if socket buffer can handle max record size (2^14 + 2048
             * + 13)
             */
            optlen = (socklen_t) sizeof(int);
            ret = getsockopt(b->num, SOL_SOCKET, SO_RCVBUF, &optval, &optlen);
            if (ret >= 0)
                OPENSSL_assert(optval >= 18445);

            /*
             * Test if SCTP doesn't partially deliver below max record size
             * (2^14 + 2048 + 13)
             */
            optlen = (socklen_t) sizeof(int);
            ret =
                getsockopt(b->num, IPPROTO_SCTP, SCTP_PARTIAL_DELIVERY_POINT,
                           &optval, &optlen);
            if (ret >= 0)
                OPENSSL_assert(optval >= 18445);

            /*
             * Partially delivered notification??? Probably a bug....
             */
            OPENSSL_assert(!(msg.msg_flags & MSG_NOTIFICATION));

            /*
             * Everything seems ok till now, so it's most likely a message
             * dropped by PR-SCTP.
             */
            memset(out, 0, outl);
            BIO_set_retry_read(b);
            return -1;
        }

        BIO_clear_retry_flags(b);
        if (ret < 0) {
            if (BIO_dgram_should_retry(ret)) {
                BIO_set_retry_read(b);
                data->dgram._errno = get_last_socket_error();
            }
        }

        /* Test if peer uses SCTP-AUTH before continuing */
        if (!data->peer_auth_tested) {
            int ii, auth_data = 0, auth_forward = 0;
            unsigned char *p;
            struct sctp_authchunks *authchunks;

            optlen =
                (socklen_t) (sizeof(sctp_assoc_t) + 256 * sizeof(uint8_t));
            authchunks = OPENSSL_malloc(optlen);
            if (authchunks == NULL)
                return -1;
            memset(authchunks, 0, optlen);
            ii = getsockopt(b->num, IPPROTO_SCTP, SCTP_PEER_AUTH_CHUNKS,
                            authchunks, &optlen);

            if (ii >= 0)
                for (p = (unsigned char *)authchunks->gauth_chunks;
                     p < (unsigned char *)authchunks + optlen;
                     p += sizeof(uint8_t)) {
                    if (*p == OPENSSL_SCTP_DATA_CHUNK_TYPE)
                        auth_data = 1;
                    if (*p == OPENSSL_SCTP_FORWARD_CUM_TSN_CHUNK_TYPE)
                        auth_forward = 1;
                }

            OPENSSL_free(authchunks);

            if (!auth_data || !auth_forward) {
                ERR_raise(ERR_LIB_BIO, BIO_R_CONNECT_ERROR);
                return -1;
            }

            data->peer_auth_tested = 1;
        }
    }
    return ret;
}

/*
 * dgram_sctp_write - send message on SCTP socket
 * @b: BIO to write to
 * @in: data to send
 * @inl: amount of bytes in @in to send
 *
 * Returns -1 on error or the sent amount of bytes on success
 */
static int dgram_sctp_write(BIO *b, const char *in, int inl)
{
    int ret;
    bio_dgram_sctp_data *data = (bio_dgram_sctp_data *) b->ptr;
    struct bio_dgram_sctp_sndinfo *sinfo = &(data->sndinfo);
    struct bio_dgram_sctp_prinfo *pinfo = &(data->prinfo);
    struct bio_dgram_sctp_sndinfo handshake_sinfo;
    struct iovec iov[1];
    struct msghdr msg;
    struct cmsghdr *cmsg;
#  if defined(SCTP_SNDINFO) && defined(SCTP_PRINFO)
    char cmsgbuf[CMSG_SPACE(sizeof(struct sctp_sndinfo)) +
                 CMSG_SPACE(sizeof(struct sctp_prinfo))];
    struct sctp_sndinfo *sndinfo;
    struct sctp_prinfo *prinfo;
#  else
    char cmsgbuf[CMSG_SPACE(sizeof(struct sctp_sndrcvinfo))];
    struct sctp_sndrcvinfo *sndrcvinfo;
#  endif

    clear_socket_error();

    /*
     * If we're send anything else than application data, disable all user
     * parameters and flags.
     */
    if (in[0] != 23) {
        memset(&handshake_sinfo, 0, sizeof(handshake_sinfo));
#  ifdef SCTP_SACK_IMMEDIATELY
        handshake_sinfo.snd_flags = SCTP_SACK_IMMEDIATELY;
#  endif
        sinfo = &handshake_sinfo;
    }

    /* We can only send a shutdown alert if the socket is dry */
    if (data->save_shutdown) {
        ret = BIO_dgram_sctp_wait_for_dry(b);
        if (ret < 0)
            return -1;
        if (ret == 0) {
            BIO_clear_retry_flags(b);
            BIO_set_retry_write(b);
            return -1;
        }
    }

    iov[0].iov_base = (char *)in;
    iov[0].iov_len = inl;
    msg.msg_name = NULL;
    msg.msg_namelen = 0;
    msg.msg_iov = iov;
    msg.msg_iovlen = 1;
    msg.msg_control = (caddr_t) cmsgbuf;
    msg.msg_controllen = 0;
    msg.msg_flags = 0;
#  if defined(SCTP_SNDINFO) && defined(SCTP_PRINFO)
    cmsg = (struct cmsghdr *)cmsgbuf;
    cmsg->cmsg_level = IPPROTO_SCTP;
    cmsg->cmsg_type = SCTP_SNDINFO;
    cmsg->cmsg_len = CMSG_LEN(sizeof(struct sctp_sndinfo));
    sndinfo = (struct sctp_sndinfo *)CMSG_DATA(cmsg);
    memset(sndinfo, 0, sizeof(*sndinfo));
    sndinfo->snd_sid = sinfo->snd_sid;
    sndinfo->snd_flags = sinfo->snd_flags;
    sndinfo->snd_ppid = sinfo->snd_ppid;
    sndinfo->snd_context = sinfo->snd_context;
    msg.msg_controllen += CMSG_SPACE(sizeof(struct sctp_sndinfo));

    cmsg =
        (struct cmsghdr *)&cmsgbuf[CMSG_SPACE(sizeof(struct sctp_sndinfo))];
    cmsg->cmsg_level = IPPROTO_SCTP;
    cmsg->cmsg_type = SCTP_PRINFO;
    cmsg->cmsg_len = CMSG_LEN(sizeof(struct sctp_prinfo));
    prinfo = (struct sctp_prinfo *)CMSG_DATA(cmsg);
    memset(prinfo, 0, sizeof(*prinfo));
    prinfo->pr_policy = pinfo->pr_policy;
    prinfo->pr_value = pinfo->pr_value;
    msg.msg_controllen += CMSG_SPACE(sizeof(struct sctp_prinfo));
#  else
    cmsg = (struct cmsghdr *)cmsgbuf;
    cmsg->cmsg_level = IPPROTO_SCTP;
    cmsg->cmsg_type = SCTP_SNDRCV;
    cmsg->cmsg_len = CMSG_LEN(sizeof(struct sctp_sndrcvinfo));
    sndrcvinfo = (struct sctp_sndrcvinfo *)CMSG_DATA(cmsg);
    memset(sndrcvinfo, 0, sizeof(*sndrcvinfo));
    sndrcvinfo->sinfo_stream = sinfo->snd_sid;
    sndrcvinfo->sinfo_flags = sinfo->snd_flags;
#   ifdef __FreeBSD__
    sndrcvinfo->sinfo_flags |= pinfo->pr_policy;
#   endif
    sndrcvinfo->sinfo_ppid = sinfo->snd_ppid;
    sndrcvinfo->sinfo_context = sinfo->snd_context;
    sndrcvinfo->sinfo_timetolive = pinfo->pr_value;
    msg.msg_controllen += CMSG_SPACE(sizeof(struct sctp_sndrcvinfo));
#  endif

    ret = sendmsg(b->num, &msg, 0);

    BIO_clear_retry_flags(b);
    if (ret <= 0) {
        if (BIO_dgram_should_retry(ret)) {
            BIO_set_retry_write(b);
            data->dgram._errno = get_last_socket_error();
        }
    }
    return ret;
}

static long dgram_sctp_ctrl(BIO *b, int cmd, long num, void *ptr)
{
    long ret = 1;
    bio_dgram_sctp_data *data = NULL;
    socklen_t sockopt_len = 0;
    struct sctp_authkeyid authkeyid;
    struct sctp_authkey *authkey = NULL;

    data = (bio_dgram_sctp_data *) b->ptr;

    switch (cmd) {
    case BIO_CTRL_DGRAM_QUERY_MTU:
        /*
         * Set to maximum (2^14) and ignore user input to enable transport
         * protocol fragmentation. Returns always 2^14.
         */
        data->dgram.mtu = 16384;
        ret = data->dgram.mtu;
        break;
    case BIO_CTRL_DGRAM_SET_MTU:
        /*
         * Set to maximum (2^14) and ignore input to enable transport
         * protocol fragmentation. Returns always 2^14.
         */
        data->dgram.mtu = 16384;
        ret = data->dgram.mtu;
        break;
    case BIO_CTRL_DGRAM_SET_CONNECTED:
    case BIO_CTRL_DGRAM_CONNECT:
        /* Returns always -1. */
        ret = -1;
        break;
    case BIO_CTRL_DGRAM_SET_NEXT_TIMEOUT:
        /*
         * SCTP doesn't need the DTLS timer Returns always 1.
         */
        break;
    case BIO_CTRL_DGRAM_GET_MTU_OVERHEAD:
        /*
         * We allow transport protocol fragmentation so this is irrelevant
         */
        ret = 0;
        break;
    case BIO_CTRL_DGRAM_SCTP_SET_IN_HANDSHAKE:
        if (num > 0)
            data->in_handshake = 1;
        else
            data->in_handshake = 0;

        ret =
            setsockopt(b->num, IPPROTO_SCTP, SCTP_NODELAY,
                       &data->in_handshake, sizeof(int));
        break;
    case BIO_CTRL_DGRAM_SCTP_ADD_AUTH_KEY:
        /*
         * New shared key for SCTP AUTH. Returns 0 on success, -1 otherwise.
         */

        /* Get active key */
        sockopt_len = sizeof(struct sctp_authkeyid);
        ret =
            getsockopt(b->num, IPPROTO_SCTP, SCTP_AUTH_ACTIVE_KEY, &authkeyid,
                       &sockopt_len);
        if (ret < 0)
            break;

        /* Add new key */
        sockopt_len = sizeof(struct sctp_authkey) + 64 * sizeof(uint8_t);
        authkey = OPENSSL_malloc(sockopt_len);
        if (authkey == NULL) {
            ret = -1;
            break;
        }
        memset(authkey, 0, sockopt_len);
        authkey->sca_keynumber = authkeyid.scact_keynumber + 1;
#  ifndef __FreeBSD__
        /*
         * This field is missing in FreeBSD 8.2 and earlier, and FreeBSD 8.3
         * and higher work without it.
         */
        authkey->sca_keylength = 64;
#  endif
        memcpy(&authkey->sca_key[0], ptr, 64 * sizeof(uint8_t));

        ret =
            setsockopt(b->num, IPPROTO_SCTP, SCTP_AUTH_KEY, authkey,
                       sockopt_len);
        OPENSSL_free(authkey);
        authkey = NULL;
        if (ret < 0)
            break;

        /* Reset active key */
        ret = setsockopt(b->num, IPPROTO_SCTP, SCTP_AUTH_ACTIVE_KEY,
                         &authkeyid, sizeof(struct sctp_authkeyid));
        if (ret < 0)
            break;

        break;
    case BIO_CTRL_DGRAM_SCTP_NEXT_AUTH_KEY:
        /* Returns 0 on success, -1 otherwise. */

        /* Get active key */
        sockopt_len = sizeof(struct sctp_authkeyid);
        ret =
            getsockopt(b->num, IPPROTO_SCTP, SCTP_AUTH_ACTIVE_KEY, &authkeyid,
                       &sockopt_len);
        if (ret < 0)
            break;

        /* Set active key */
        authkeyid.scact_keynumber = authkeyid.scact_keynumber + 1;
        ret = setsockopt(b->num, IPPROTO_SCTP, SCTP_AUTH_ACTIVE_KEY,
                         &authkeyid, sizeof(struct sctp_authkeyid));
        if (ret < 0)
            break;

        /*
         * CCS has been sent, so remember that and fall through to check if
         * we need to deactivate an old key
         */
        data->ccs_sent = 1;
        /* fall-through */

    case BIO_CTRL_DGRAM_SCTP_AUTH_CCS_RCVD:
        /* Returns 0 on success, -1 otherwise. */

        /*
         * Has this command really been called or is this just a
         * fall-through?
         */
        if (cmd == BIO_CTRL_DGRAM_SCTP_AUTH_CCS_RCVD)
            data->ccs_rcvd = 1;

        /*
         * CSS has been both, received and sent, so deactivate an old key
         */
        if (data->ccs_rcvd == 1 && data->ccs_sent == 1) {
            /* Get active key */
            sockopt_len = sizeof(struct sctp_authkeyid);
            ret =
                getsockopt(b->num, IPPROTO_SCTP, SCTP_AUTH_ACTIVE_KEY,
                           &authkeyid, &sockopt_len);
            if (ret < 0)
                break;

            /*
             * Deactivate key or delete second last key if
             * SCTP_AUTHENTICATION_EVENT is not available.
             */
            authkeyid.scact_keynumber = authkeyid.scact_keynumber - 1;
#  ifdef SCTP_AUTH_DEACTIVATE_KEY
            sockopt_len = sizeof(struct sctp_authkeyid);
            ret = setsockopt(b->num, IPPROTO_SCTP, SCTP_AUTH_DEACTIVATE_KEY,
                             &authkeyid, sockopt_len);
            if (ret < 0)
                break;
#  endif
#  ifndef SCTP_AUTHENTICATION_EVENT
            if (authkeyid.scact_keynumber > 0) {
                authkeyid.scact_keynumber = authkeyid.scact_keynumber - 1;
                ret = setsockopt(b->num, IPPROTO_SCTP, SCTP_AUTH_DELETE_KEY,
                                 &authkeyid, sizeof(struct sctp_authkeyid));
                if (ret < 0)
                    break;
            }
#  endif

            data->ccs_rcvd = 0;
            data->ccs_sent = 0;
        }
        break;
    case BIO_CTRL_DGRAM_SCTP_GET_SNDINFO:
        /* Returns the size of the copied struct. */
        if (num > (long)sizeof(struct bio_dgram_sctp_sndinfo))
            num = sizeof(struct bio_dgram_sctp_sndinfo);

        memcpy(ptr, &(data->sndinfo), num);
        ret = num;
        break;
    case BIO_CTRL_DGRAM_SCTP_SET_SNDINFO:
        /* Returns the size of the copied struct. */
        if (num > (long)sizeof(struct bio_dgram_sctp_sndinfo))
            num = sizeof(struct bio_dgram_sctp_sndinfo);

        memcpy(&(data->sndinfo), ptr, num);
        break;
    case BIO_CTRL_DGRAM_SCTP_GET_RCVINFO:
        /* Returns the size of the copied struct. */
        if (num > (long)sizeof(struct bio_dgram_sctp_rcvinfo))
            num = sizeof(struct bio_dgram_sctp_rcvinfo);

        memcpy(ptr, &data->rcvinfo, num);

        ret = num;
        break;
    case BIO_CTRL_DGRAM_SCTP_SET_RCVINFO:
        /* Returns the size of the copied struct. */
        if (num > (long)sizeof(struct bio_dgram_sctp_rcvinfo))
            num = sizeof(struct bio_dgram_sctp_rcvinfo);

        memcpy(&(data->rcvinfo), ptr, num);
        break;
    case BIO_CTRL_DGRAM_SCTP_GET_PRINFO:
        /* Returns the size of the copied struct. */
        if (num > (long)sizeof(struct bio_dgram_sctp_prinfo))
            num = sizeof(struct bio_dgram_sctp_prinfo);

        memcpy(ptr, &(data->prinfo), num);
        ret = num;
        break;
    case BIO_CTRL_DGRAM_SCTP_SET_PRINFO:
        /* Returns the size of the copied struct. */
        if (num > (long)sizeof(struct bio_dgram_sctp_prinfo))
            num = sizeof(struct bio_dgram_sctp_prinfo);

        memcpy(&(data->prinfo), ptr, num);
        break;
    case BIO_CTRL_DGRAM_SCTP_SAVE_SHUTDOWN:
        /* Returns always 1. */
        if (num > 0)
            data->save_shutdown = 1;
        else
            data->save_shutdown = 0;
        break;
    case BIO_CTRL_DGRAM_SCTP_WAIT_FOR_DRY:
        return dgram_sctp_wait_for_dry(b);
    case BIO_CTRL_DGRAM_SCTP_MSG_WAITING:
        return dgram_sctp_msg_waiting(b);

    default:
        /*
         * Pass to default ctrl function to process SCTP unspecific commands
         */
        ret = dgram_ctrl(b, cmd, num, ptr);
        break;
    }
    return ret;
}

int BIO_dgram_sctp_notification_cb(BIO *b,
                BIO_dgram_sctp_notification_handler_fn handle_notifications,
                void *context)
{
    bio_dgram_sctp_data *data = (bio_dgram_sctp_data *) b->ptr;

    if (handle_notifications != NULL) {
        data->handle_notifications = handle_notifications;
        data->notification_context = context;
    } else
        return -1;

    return 0;
}

/*
 * BIO_dgram_sctp_wait_for_dry - Wait for SCTP SENDER_DRY event
 * @b: The BIO to check for the dry event
 *
 * Wait until the peer confirms all packets have been received, and so that
 * our kernel doesn't have anything to send anymore.  This is only received by
 * the peer's kernel, not the application.
 *
 * Returns:
 * -1 on error
 *  0 when not dry yet
 *  1 when dry
 */
int BIO_dgram_sctp_wait_for_dry(BIO *b)
{
    return (int)BIO_ctrl(b, BIO_CTRL_DGRAM_SCTP_WAIT_FOR_DRY, 0, NULL);
}

static int dgram_sctp_wait_for_dry(BIO *b)
{
    int is_dry = 0;
    int sockflags = 0;
    int n, ret;
    union sctp_notification snp;
    struct msghdr msg;
    struct iovec iov;
#  ifdef SCTP_EVENT
    struct sctp_event event;
#  else
    struct sctp_event_subscribe event;
    socklen_t eventsize;
#  endif
    bio_dgram_sctp_data *data = (bio_dgram_sctp_data *) b->ptr;

    /* set sender dry event */
#  ifdef SCTP_EVENT
    memset(&event, 0, sizeof(event));
    event.se_assoc_id = 0;
    event.se_type = SCTP_SENDER_DRY_EVENT;
    event.se_on = 1;
    ret =
        setsockopt(b->num, IPPROTO_SCTP, SCTP_EVENT, &event,
                   sizeof(struct sctp_event));
#  else
    eventsize = sizeof(struct sctp_event_subscribe);
    ret = getsockopt(b->num, IPPROTO_SCTP, SCTP_EVENTS, &event, &eventsize);
    if (ret < 0)
        return -1;

    event.sctp_sender_dry_event = 1;

    ret =
        setsockopt(b->num, IPPROTO_SCTP, SCTP_EVENTS, &event,
                   sizeof(struct sctp_event_subscribe));
#  endif
    if (ret < 0)
        return -1;

    /* peek for notification */
    memset(&snp, 0, sizeof(snp));
    iov.iov_base = (char *)&snp;
    iov.iov_len = sizeof(union sctp_notification);
    msg.msg_name = NULL;
    msg.msg_namelen = 0;
    msg.msg_iov = &iov;
    msg.msg_iovlen = 1;
    msg.msg_control = NULL;
    msg.msg_controllen = 0;
    msg.msg_flags = 0;

    n = recvmsg(b->num, &msg, MSG_PEEK);
    if (n <= 0) {
        if ((n < 0) && (get_last_socket_error() != EAGAIN)
            && (get_last_socket_error() != EWOULDBLOCK))
            return -1;
        else
            return 0;
    }

    /* if we find a notification, process it and try again if necessary */
    while (msg.msg_flags & MSG_NOTIFICATION) {
        memset(&snp, 0, sizeof(snp));
        iov.iov_base = (char *)&snp;
        iov.iov_len = sizeof(union sctp_notification);
        msg.msg_name = NULL;
        msg.msg_namelen = 0;
        msg.msg_iov = &iov;
        msg.msg_iovlen = 1;
        msg.msg_control = NULL;
        msg.msg_controllen = 0;
        msg.msg_flags = 0;

        n = recvmsg(b->num, &msg, 0);
        if (n <= 0) {
            if ((n < 0) && (get_last_socket_error() != EAGAIN)
                && (get_last_socket_error() != EWOULDBLOCK))
                return -1;
            else
                return is_dry;
        }

        if (snp.sn_header.sn_type == SCTP_SENDER_DRY_EVENT) {
            is_dry = 1;

            /* disable sender dry event */
#  ifdef SCTP_EVENT
            memset(&event, 0, sizeof(event));
            event.se_assoc_id = 0;
            event.se_type = SCTP_SENDER_DRY_EVENT;
            event.se_on = 0;
            ret =
                setsockopt(b->num, IPPROTO_SCTP, SCTP_EVENT, &event,
                           sizeof(struct sctp_event));
#  else
            eventsize = (socklen_t) sizeof(struct sctp_event_subscribe);
            ret =
                getsockopt(b->num, IPPROTO_SCTP, SCTP_EVENTS, &event,
                           &eventsize);
            if (ret < 0)
                return -1;

            event.sctp_sender_dry_event = 0;

            ret =
                setsockopt(b->num, IPPROTO_SCTP, SCTP_EVENTS, &event,
                           sizeof(struct sctp_event_subscribe));
#  endif
            if (ret < 0)
                return -1;
        }
#  ifdef SCTP_AUTHENTICATION_EVENT
        if (snp.sn_header.sn_type == SCTP_AUTHENTICATION_EVENT)
            dgram_sctp_handle_auth_free_key_event(b, &snp);
#  endif

        if (data->handle_notifications != NULL)
            data->handle_notifications(b, data->notification_context,
                                       (void *)&snp);

        /* found notification, peek again */
        memset(&snp, 0, sizeof(snp));
        iov.iov_base = (char *)&snp;
        iov.iov_len = sizeof(union sctp_notification);
        msg.msg_name = NULL;
        msg.msg_namelen = 0;
        msg.msg_iov = &iov;
        msg.msg_iovlen = 1;
        msg.msg_control = NULL;
        msg.msg_controllen = 0;
        msg.msg_flags = 0;

        /* if we have seen the dry already, don't wait */
        if (is_dry) {
            sockflags = fcntl(b->num, F_GETFL, 0);
            fcntl(b->num, F_SETFL, O_NONBLOCK);
        }

        n = recvmsg(b->num, &msg, MSG_PEEK);

        if (is_dry) {
            fcntl(b->num, F_SETFL, sockflags);
        }

        if (n <= 0) {
            if ((n < 0) && (get_last_socket_error() != EAGAIN)
                && (get_last_socket_error() != EWOULDBLOCK))
                return -1;
            else
                return is_dry;
        }
    }

    /* read anything else */
    return is_dry;
}

int BIO_dgram_sctp_msg_waiting(BIO *b)
{
    return (int)BIO_ctrl(b, BIO_CTRL_DGRAM_SCTP_MSG_WAITING, 0, NULL);
}

static int dgram_sctp_msg_waiting(BIO *b)
{
    int n, sockflags;
    union sctp_notification snp;
    struct msghdr msg;
    struct iovec iov;
    bio_dgram_sctp_data *data = (bio_dgram_sctp_data *) b->ptr;

    /* Check if there are any messages waiting to be read */
    do {
        memset(&snp, 0, sizeof(snp));
        iov.iov_base = (char *)&snp;
        iov.iov_len = sizeof(union sctp_notification);
        msg.msg_name = NULL;
        msg.msg_namelen = 0;
        msg.msg_iov = &iov;
        msg.msg_iovlen = 1;
        msg.msg_control = NULL;
        msg.msg_controllen = 0;
        msg.msg_flags = 0;

        sockflags = fcntl(b->num, F_GETFL, 0);
        fcntl(b->num, F_SETFL, O_NONBLOCK);
        n = recvmsg(b->num, &msg, MSG_PEEK);
        fcntl(b->num, F_SETFL, sockflags);

        /* if notification, process and try again */
        if (n > 0 && (msg.msg_flags & MSG_NOTIFICATION)) {
#  ifdef SCTP_AUTHENTICATION_EVENT
            if (snp.sn_header.sn_type == SCTP_AUTHENTICATION_EVENT)
                dgram_sctp_handle_auth_free_key_event(b, &snp);
#  endif

            memset(&snp, 0, sizeof(snp));
            iov.iov_base = (char *)&snp;
            iov.iov_len = sizeof(union sctp_notification);
            msg.msg_name = NULL;
            msg.msg_namelen = 0;
            msg.msg_iov = &iov;
            msg.msg_iovlen = 1;
            msg.msg_control = NULL;
            msg.msg_controllen = 0;
            msg.msg_flags = 0;
            n = recvmsg(b->num, &msg, 0);

            if (data->handle_notifications != NULL)
                data->handle_notifications(b, data->notification_context,
                                           (void *)&snp);
        }

    } while (n > 0 && (msg.msg_flags & MSG_NOTIFICATION));

    /* Return 1 if there is a message to be read, return 0 otherwise. */
    if (n > 0)
        return 1;
    else
        return 0;
}

static int dgram_sctp_puts(BIO *bp, const char *str)
{
    int n, ret;

    n = strlen(str);
    ret = dgram_sctp_write(bp, str, n);
    return ret;
}
# endif

static int BIO_dgram_should_retry(int i)
{
    int err;

    if ((i == 0) || (i == -1)) {
        err = get_last_socket_error();

# if defined(OPENSSL_SYS_WINDOWS)
        /*
         * If the socket return value (i) is -1 and err is unexpectedly 0 at
         * this point, the error code was overwritten by another system call
         * before this error handling is called.
         */
# endif

        return BIO_dgram_non_fatal_error(err);
    }
    return 0;
}

int BIO_dgram_non_fatal_error(int err)
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

#endif
