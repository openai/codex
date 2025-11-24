/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <stdlib.h>
#include "bio_local.h"

#ifndef OPENSSL_NO_SOCK
# define SOCKET_PROTOCOL IPPROTO_TCP
# ifdef SO_MAXCONN
#  define MAX_LISTEN  SO_MAXCONN
# elif defined(SOMAXCONN)
#  define MAX_LISTEN  SOMAXCONN
# else
#  define MAX_LISTEN  32
# endif
# if defined(OPENSSL_SYS_WINDOWS)
static int wsa_init_done = 0;
# endif

# if defined __TANDEM
#  include <unistd.h>
#  include <sys/time.h> /* select */
# elif defined _WIN32
#  include <winsock.h> /* for type fd_set */
# else
#  include <unistd.h>
#  if defined __VMS
#   include <sys/socket.h>
#  elif defined _HPUX_SOURCE
#   include <sys/time.h>
#  else
#   include <sys/select.h>
#  endif
# endif
# include "internal/sockets.h" /* for openssl_fdset() */

# ifndef OPENSSL_NO_DEPRECATED_1_1_0
int BIO_get_host_ip(const char *str, unsigned char *ip)
{
    BIO_ADDRINFO *res = NULL;
    int ret = 0;

    if (BIO_sock_init() != 1)
        return 0;               /* don't generate another error code here */

    if (BIO_lookup(str, NULL, BIO_LOOKUP_CLIENT, AF_INET, SOCK_STREAM, &res)) {
        size_t l;

        if (BIO_ADDRINFO_family(res) != AF_INET) {
            ERR_raise(ERR_LIB_BIO, BIO_R_GETHOSTBYNAME_ADDR_IS_NOT_AF_INET);
        } else if (BIO_ADDR_rawaddress(BIO_ADDRINFO_address(res), NULL, &l)) {
            /*
             * Because only AF_INET addresses will reach this far, we can assert
             * that l should be 4
             */
            if (ossl_assert(l == 4))
                ret = BIO_ADDR_rawaddress(BIO_ADDRINFO_address(res), ip, &l);
        }
        BIO_ADDRINFO_free(res);
    } else {
        ERR_add_error_data(2, "host=", str);
    }

    return ret;
}

int BIO_get_port(const char *str, unsigned short *port_ptr)
{
    BIO_ADDRINFO *res = NULL;
    int ret = 0;

    if (str == NULL) {
        ERR_raise(ERR_LIB_BIO, BIO_R_NO_PORT_DEFINED);
        return 0;
    }

    if (BIO_sock_init() != 1)
        return 0;               /* don't generate another error code here */

    if (BIO_lookup(NULL, str, BIO_LOOKUP_CLIENT, AF_INET, SOCK_STREAM, &res)) {
        if (BIO_ADDRINFO_family(res) != AF_INET) {
            ERR_raise(ERR_LIB_BIO, BIO_R_ADDRINFO_ADDR_IS_NOT_AF_INET);
        } else {
            *port_ptr = ntohs(BIO_ADDR_rawport(BIO_ADDRINFO_address(res)));
            ret = 1;
        }
        BIO_ADDRINFO_free(res);
    } else {
        ERR_add_error_data(2, "host=", str);
    }

    return ret;
}
# endif

int BIO_sock_error(int sock)
{
    int j = 0, i;
    socklen_t size = sizeof(j);

    /*
     * Note: under Windows the third parameter is of type (char *) whereas
     * under other systems it is (void *) if you don't have a cast it will
     * choke the compiler: if you do have a cast then you can either go for
     * (char *) or (void *).
     */
    i = getsockopt(sock, SOL_SOCKET, SO_ERROR, (void *)&j, &size);
    if (i < 0)
        return get_last_socket_error();
    else
        return j;
}

# ifndef OPENSSL_NO_DEPRECATED_1_1_0
struct hostent *BIO_gethostbyname(const char *name)
{
    /*
     * Caching gethostbyname() results forever is wrong, so we have to let
     * the true gethostbyname() worry about this
     */
    return gethostbyname(name);
}
# endif

# ifdef BIO_HAVE_WSAMSG
LPFN_WSARECVMSG bio_WSARecvMsg;
LPFN_WSASENDMSG bio_WSASendMsg;
# endif

int BIO_sock_init(void)
{
# ifdef OPENSSL_SYS_WINDOWS
    static struct WSAData wsa_state;

    if (!wsa_init_done) {
        wsa_init_done = 1;
        memset(&wsa_state, 0, sizeof(wsa_state));
        /*
         * Not making wsa_state available to the rest of the code is formally
         * wrong. But the structures we use are [believed to be] invariable
         * among Winsock DLLs, while API availability is [expected to be]
         * probed at run-time with DSO_global_lookup.
         */
        if (WSAStartup(0x0202, &wsa_state) != 0) {
            ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                           "calling wsastartup()");
            ERR_raise(ERR_LIB_BIO, BIO_R_WSASTARTUP);
            return -1;
        }

        /*
         * On Windows, some socket functions are not exposed as a prototype.
         * Instead, their function pointers must be loaded via this elaborate
         * process...
         */
#  ifdef BIO_HAVE_WSAMSG
        {
            GUID id_WSARecvMsg = WSAID_WSARECVMSG;
            GUID id_WSASendMsg = WSAID_WSASENDMSG;
            DWORD len_out = 0;
            SOCKET s;

            s = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
            if (s != INVALID_SOCKET) {
                if (WSAIoctl(s, SIO_GET_EXTENSION_FUNCTION_POINTER,
                             &id_WSARecvMsg, sizeof(id_WSARecvMsg),
                             &bio_WSARecvMsg, sizeof(bio_WSARecvMsg),
                             &len_out, NULL, NULL) != 0
                    || len_out != sizeof(bio_WSARecvMsg))
                    bio_WSARecvMsg = NULL;

                if (WSAIoctl(s, SIO_GET_EXTENSION_FUNCTION_POINTER,
                             &id_WSASendMsg, sizeof(id_WSASendMsg),
                             &bio_WSASendMsg, sizeof(bio_WSASendMsg),
                             &len_out, NULL, NULL) != 0
                    || len_out != sizeof(bio_WSASendMsg))
                    bio_WSASendMsg = NULL;

                closesocket(s);
            }
        }
#  endif
    }
# endif                         /* OPENSSL_SYS_WINDOWS */
# ifdef WATT32
    extern int _watt_do_exit;
    _watt_do_exit = 0;          /* don't make sock_init() call exit() */
    if (sock_init())
        return -1;
# endif

    return 1;
}

void bio_sock_cleanup_int(void)
{
# ifdef OPENSSL_SYS_WINDOWS
    if (wsa_init_done) {
        wsa_init_done = 0;
        WSACleanup();
    }
# endif
}

int BIO_socket_ioctl(int fd, long type, void *arg)
{
    int i;

#  ifdef __DJGPP__
    i = ioctlsocket(fd, type, (char *)arg);
#  else
#   if defined(OPENSSL_SYS_VMS)
    /*-
     * 2011-02-18 SMS.
     * VMS ioctl() can't tolerate a 64-bit "void *arg", but we
     * observe that all the consumers pass in an "unsigned long *",
     * so we arrange a local copy with a short pointer, and use
     * that, instead.
     */
#    if __INITIAL_POINTER_SIZE == 64
#     define ARG arg_32p
#     pragma pointer_size save
#     pragma pointer_size 32
    unsigned long arg_32;
    unsigned long *arg_32p;
#     pragma pointer_size restore
    arg_32p = &arg_32;
    arg_32 = *((unsigned long *)arg);
#    else                       /* __INITIAL_POINTER_SIZE == 64 */
#     define ARG arg
#    endif                      /* __INITIAL_POINTER_SIZE == 64 [else] */
#   else                        /* defined(OPENSSL_SYS_VMS) */
#    define ARG arg
#   endif                       /* defined(OPENSSL_SYS_VMS) [else] */

    i = ioctlsocket(fd, type, ARG);
#  endif                        /* __DJGPP__ */
    if (i < 0)
        ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                       "calling ioctlsocket()");
    return i;
}

# ifndef OPENSSL_NO_DEPRECATED_1_1_0
int BIO_get_accept_socket(char *host, int bind_mode)
{
    int s = INVALID_SOCKET;
    char *h = NULL, *p = NULL;
    BIO_ADDRINFO *res = NULL;

    if (!BIO_parse_hostserv(host, &h, &p, BIO_PARSE_PRIO_SERV))
        return INVALID_SOCKET;

    if (BIO_sock_init() != 1)
        goto err;

    if (BIO_lookup(h, p, BIO_LOOKUP_SERVER, AF_UNSPEC, SOCK_STREAM, &res) != 0)
        goto err;

    if ((s = BIO_socket(BIO_ADDRINFO_family(res), BIO_ADDRINFO_socktype(res),
                        BIO_ADDRINFO_protocol(res), 0)) == INVALID_SOCKET) {
        s = INVALID_SOCKET;
        goto err;
    }

    if (!BIO_listen(s, BIO_ADDRINFO_address(res),
                    bind_mode ? BIO_SOCK_REUSEADDR : 0)) {
        BIO_closesocket(s);
        s = INVALID_SOCKET;
    }

 err:
    BIO_ADDRINFO_free(res);
    OPENSSL_free(h);
    OPENSSL_free(p);

    return s;
}

int BIO_accept(int sock, char **ip_port)
{
    BIO_ADDR res;
    int ret = -1;

    ret = BIO_accept_ex(sock, &res, 0);
    if (ret == (int)INVALID_SOCKET) {
        if (BIO_sock_should_retry(ret)) {
            ret = -2;
            goto end;
        }
        ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                       "calling accept()");
        ERR_raise(ERR_LIB_BIO, BIO_R_ACCEPT_ERROR);
        goto end;
    }

    if (ip_port != NULL) {
        char *host = BIO_ADDR_hostname_string(&res, 1);
        char *port = BIO_ADDR_service_string(&res, 1);
        if (host != NULL && port != NULL) {
            *ip_port = OPENSSL_zalloc(strlen(host) + strlen(port) + 2);
        } else {
            *ip_port = NULL;
            ERR_raise(ERR_LIB_BIO, ERR_R_BIO_LIB);
        }

        if (*ip_port == NULL) {
            BIO_closesocket(ret);
            ret = (int)INVALID_SOCKET;
        } else {
            strcpy(*ip_port, host);
            strcat(*ip_port, ":");
            strcat(*ip_port, port);
        }
        OPENSSL_free(host);
        OPENSSL_free(port);
    }

 end:
    return ret;
}
# endif

int BIO_set_tcp_ndelay(int s, int on)
{
    int ret = 0;
# if defined(TCP_NODELAY) && (defined(IPPROTO_TCP) || defined(SOL_TCP))
    int opt;

#  ifdef SOL_TCP
    opt = SOL_TCP;
#  else
#   ifdef IPPROTO_TCP
    opt = IPPROTO_TCP;
#   endif
#  endif

    ret = setsockopt(s, opt, TCP_NODELAY, (char *)&on, sizeof(on));
# endif
    return (ret == 0);
}

int BIO_socket_nbio(int s, int mode)
{
    int ret = -1;
    int l;

    l = mode;
# ifdef FIONBIO
    l = mode;

    ret = BIO_socket_ioctl(s, FIONBIO, &l);
# elif defined(F_GETFL) && defined(F_SETFL) && (defined(O_NONBLOCK) || defined(FNDELAY))
    /* make sure this call always pushes an error level; BIO_socket_ioctl() does so, so we do too. */

    l = fcntl(s, F_GETFL, 0);
    if (l == -1) {
        ERR_raise_data(ERR_LIB_SYS, get_last_sys_error(),
                       "calling fcntl()");
        ret = -1;
    } else {
#  if defined(O_NONBLOCK)
        l &= ~O_NONBLOCK;
#  else
        l &= ~FNDELAY; /* BSD4.x */
#  endif
        if (mode) {
#  if defined(O_NONBLOCK)
            l |= O_NONBLOCK;
#  else
            l |= FNDELAY; /* BSD4.x */
#  endif
        }
        ret = fcntl(s, F_SETFL, l);

        if (ret < 0) {
            ERR_raise_data(ERR_LIB_SYS, get_last_sys_error(),
                           "calling fcntl()");
        }
    }
# else
    /* make sure this call always pushes an error level; BIO_socket_ioctl() does so, so we do too. */
    ERR_raise(ERR_LIB_BIO, ERR_R_PASSED_INVALID_ARGUMENT);
# endif

    return (ret == 0);
}

int BIO_sock_info(int sock,
                  enum BIO_sock_info_type type, union BIO_sock_info_u *info)
{
    switch (type) {
    case BIO_SOCK_INFO_ADDRESS:
        {
            socklen_t addr_len;
            int ret = 0;
            addr_len = sizeof(*info->addr);
            ret = getsockname(sock, BIO_ADDR_sockaddr_noconst(info->addr),
                              &addr_len);
            if (ret == -1) {
                ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                               "calling getsockname()");
                ERR_raise(ERR_LIB_BIO, BIO_R_GETSOCKNAME_ERROR);
                return 0;
            }
            if ((size_t)addr_len > sizeof(*info->addr)) {
                ERR_raise(ERR_LIB_BIO, BIO_R_GETSOCKNAME_TRUNCATED_ADDRESS);
                return 0;
            }
        }
        break;
    default:
        ERR_raise(ERR_LIB_BIO, BIO_R_UNKNOWN_INFO_TYPE);
        return 0;
    }
    return 1;
}

/*
 * Wait on fd at most until max_time; succeed immediately if max_time == 0.
 * If for_read == 0 then assume to wait for writing, else wait for reading.
 * Returns -1 on error, 0 on timeout, and 1 on success.
 */
int BIO_socket_wait(int fd, int for_read, time_t max_time)
{
# if defined(OPENSSL_SYS_WINDOWS) || !defined(POLLIN)
    fd_set confds;
    struct timeval tv;
    time_t now;

#  ifdef _WIN32
    if ((SOCKET)fd == INVALID_SOCKET)
#  else
    if (fd < 0 || fd >= FD_SETSIZE)
#  endif
        return -1;
    if (max_time == 0)
        return 1;

    now = time(NULL);
    if (max_time < now)
        return 0;

    FD_ZERO(&confds);
    openssl_fdset(fd, &confds);
    tv.tv_usec = 0;
    tv.tv_sec = (long)(max_time - now); /* might overflow */
    return select(fd + 1, for_read ? &confds : NULL,
                  for_read ? NULL : &confds, NULL, &tv);
# else
    struct pollfd confds;
    time_t now;

    if (fd < 0)
        return -1;
    if (max_time == 0)
        return 1;

    now = time(NULL);
    if (max_time < now)
        return 0;

    confds.fd = fd;
    confds.events = for_read ? POLLIN : POLLOUT;
    return poll(&confds, 1, (int)(max_time - now) * 1000);
# endif
}
#endif /* !defined(OPENSSL_NO_SOCK) */
