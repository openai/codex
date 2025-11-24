/*
 * Copyright 2016-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <stdlib.h>
#include <errno.h>

#include "bio_local.h"
#include "internal/ktls.h"
#include "internal/bio_tfo.h"

#include <openssl/err.h>

#ifndef OPENSSL_NO_SOCK
# ifdef SO_MAXCONN
#  define MAX_LISTEN  SO_MAXCONN
# elif defined(SOMAXCONN)
#  define MAX_LISTEN  SOMAXCONN
# else
#  define MAX_LISTEN  32
# endif

/*-
 * BIO_socket - create a socket
 * @domain: the socket domain (AF_INET, AF_INET6, AF_UNIX, ...)
 * @socktype: the socket type (SOCK_STEAM, SOCK_DGRAM)
 * @protocol: the protocol to use (IPPROTO_TCP, IPPROTO_UDP)
 * @options: BIO socket options (currently unused)
 *
 * Creates a socket.  This should be called before calling any
 * of BIO_connect and BIO_listen.
 *
 * Returns the file descriptor on success or INVALID_SOCKET on failure.  On
 * failure errno is set, and a status is added to the OpenSSL error stack.
 */
int BIO_socket(int domain, int socktype, int protocol, int options)
{
    int sock = -1;

    if (BIO_sock_init() != 1)
        return INVALID_SOCKET;

    sock = socket(domain, socktype, protocol);
    if (sock == -1) {
        ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                       "calling socket()");
        ERR_raise(ERR_LIB_BIO, BIO_R_UNABLE_TO_CREATE_SOCKET);
        return INVALID_SOCKET;
    }

    return sock;
}

/*-
 * BIO_connect - connect to an address
 * @sock: the socket to connect with
 * @addr: the address to connect to
 * @options: BIO socket options
 *
 * Connects to the address using the given socket and options.
 *
 * Options can be a combination of the following:
 * - BIO_SOCK_KEEPALIVE: enable regularly sending keep-alive messages.
 * - BIO_SOCK_NONBLOCK: Make the socket non-blocking.
 * - BIO_SOCK_NODELAY: don't delay small messages.
 * - BIO_SOCK_TFO: use TCP Fast Open
 *
 * options holds BIO socket options that can be used
 * You should call this for every address returned by BIO_lookup
 * until the connection is successful.
 *
 * Returns 1 on success or 0 on failure.  On failure errno is set
 * and an error status is added to the OpenSSL error stack.
 */
int BIO_connect(int sock, const BIO_ADDR *addr, int options)
{
    const int on = 1;

    if (sock == -1) {
        ERR_raise(ERR_LIB_BIO, BIO_R_INVALID_SOCKET);
        return 0;
    }

    if (!BIO_socket_nbio(sock, (options & BIO_SOCK_NONBLOCK) != 0))
        return 0;

    if (options & BIO_SOCK_KEEPALIVE) {
        if (setsockopt(sock, SOL_SOCKET, SO_KEEPALIVE,
                       (const void *)&on, sizeof(on)) != 0) {
            ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                           "calling setsockopt()");
            ERR_raise(ERR_LIB_BIO, BIO_R_UNABLE_TO_KEEPALIVE);
            return 0;
        }
    }

    if (options & BIO_SOCK_NODELAY) {
        if (setsockopt(sock, IPPROTO_TCP, TCP_NODELAY,
                       (const void *)&on, sizeof(on)) != 0) {
            ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                           "calling setsockopt()");
            ERR_raise(ERR_LIB_BIO, BIO_R_UNABLE_TO_NODELAY);
            return 0;
        }
    }
    if (options & BIO_SOCK_TFO) {
# if defined(OSSL_TFO_CLIENT_FLAG)
#  if defined(OSSL_TFO_SYSCTL_CLIENT)
        int enabled = 0;
        size_t enabledlen = sizeof(enabled);

        /* Later FreeBSD */
        if (sysctlbyname(OSSL_TFO_SYSCTL_CLIENT, &enabled, &enabledlen, NULL, 0) < 0) {
            ERR_raise(ERR_LIB_BIO, BIO_R_TFO_NO_KERNEL_SUPPORT);
            return 0;
        }
        /* Need to check for client flag */
        if (!(enabled & OSSL_TFO_CLIENT_FLAG)) {
            ERR_raise(ERR_LIB_BIO, BIO_R_TFO_DISABLED);
            return 0;
        }
#  elif defined(OSSL_TFO_SYSCTL)
        int enabled = 0;
        size_t enabledlen = sizeof(enabled);

        /* macOS */
        if (sysctlbyname(OSSL_TFO_SYSCTL, &enabled, &enabledlen, NULL, 0) < 0) {
            ERR_raise(ERR_LIB_BIO, BIO_R_TFO_NO_KERNEL_SUPPORT);
            return 0;
        }
        /* Need to check for client flag */
        if (!(enabled & OSSL_TFO_CLIENT_FLAG)) {
            ERR_raise(ERR_LIB_BIO, BIO_R_TFO_DISABLED);
            return 0;
        }
#  endif
# endif
# if defined(OSSL_TFO_CONNECTX)
        sa_endpoints_t sae;

        memset(&sae, 0, sizeof(sae));
        sae.sae_dstaddr = BIO_ADDR_sockaddr(addr);
        sae.sae_dstaddrlen = BIO_ADDR_sockaddr_size(addr);
        if (connectx(sock, &sae, SAE_ASSOCID_ANY,
                     CONNECT_DATA_IDEMPOTENT | CONNECT_RESUME_ON_READ_WRITE,
                     NULL, 0, NULL, NULL) == -1) {
            if (!BIO_sock_should_retry(-1)) {
                ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                               "calling connectx()");
                ERR_raise(ERR_LIB_BIO, BIO_R_CONNECT_ERROR);
            }
            return 0;
        }
# endif
# if defined(OSSL_TFO_CLIENT_SOCKOPT)
        if (setsockopt(sock, IPPROTO_TCP, OSSL_TFO_CLIENT_SOCKOPT,
                       (const void *)&on, sizeof(on)) != 0) {
            ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                           "calling setsockopt()");
            ERR_raise(ERR_LIB_BIO, BIO_R_UNABLE_TO_TFO);
            return 0;
        }
# endif
# if defined(OSSL_TFO_DO_NOT_CONNECT)
        return 1;
# endif
    }

    if (connect(sock, BIO_ADDR_sockaddr(addr),
                BIO_ADDR_sockaddr_size(addr)) == -1) {
        if (!BIO_sock_should_retry(-1)) {
            ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                           "calling connect()");
            ERR_raise(ERR_LIB_BIO, BIO_R_CONNECT_ERROR);
        }
        return 0;
    }
# ifndef OPENSSL_NO_KTLS
    /*
     * The new socket is created successfully regardless of ktls_enable.
     * ktls_enable doesn't change any functionality of the socket, except
     * changing the setsockopt to enable the processing of ktls_start.
     * Thus, it is not a problem to call it for non-TLS sockets.
     */
    ktls_enable(sock);
# endif
    return 1;
}

/*-
 * BIO_bind - bind socket to address
 * @sock: the socket to set
 * @addr: local address to bind to
 * @options: BIO socket options
 *
 * Binds to the address using the given socket and options.
 *
 * Options can be a combination of the following:
 * - BIO_SOCK_REUSEADDR: Try to reuse the address and port combination
 *   for a recently closed port.
 *
 * When restarting the program it could be that the port is still in use.  If
 * you set to BIO_SOCK_REUSEADDR option it will try to reuse the port anyway.
 * It's recommended that you use this.
 */
int BIO_bind(int sock, const BIO_ADDR *addr, int options)
{
# ifndef OPENSSL_SYS_WINDOWS
    int on = 1;
# endif

    if (sock == -1) {
        ERR_raise(ERR_LIB_BIO, BIO_R_INVALID_SOCKET);
        return 0;
    }

# ifndef OPENSSL_SYS_WINDOWS
    /*
     * SO_REUSEADDR has different behavior on Windows than on
     * other operating systems, don't set it there.
     */
    if (options & BIO_SOCK_REUSEADDR) {
        if (setsockopt(sock, SOL_SOCKET, SO_REUSEADDR,
                       (const void *)&on, sizeof(on)) != 0) {
            ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                           "calling setsockopt()");
            ERR_raise(ERR_LIB_BIO, BIO_R_UNABLE_TO_REUSEADDR);
            return 0;
        }
    }
# endif

    if (bind(sock, BIO_ADDR_sockaddr(addr), BIO_ADDR_sockaddr_size(addr)) != 0) {
        ERR_raise_data(ERR_LIB_SYS, get_last_socket_error() /* may be 0 */,
                       "calling bind()");
        ERR_raise(ERR_LIB_BIO, BIO_R_UNABLE_TO_BIND_SOCKET);
        return 0;
    }

    return 1;
}

/*-
 * BIO_listen - Creates a listen socket
 * @sock: the socket to listen with
 * @addr: local address to bind to
 * @options: BIO socket options
 *
 * Binds to the address using the given socket and options, then
 * starts listening for incoming connections.
 *
 * Options can be a combination of the following:
 * - BIO_SOCK_KEEPALIVE: enable regularly sending keep-alive messages.
 * - BIO_SOCK_NONBLOCK: Make the socket non-blocking.
 * - BIO_SOCK_NODELAY: don't delay small messages.
 * - BIO_SOCK_REUSEADDR: Try to reuse the address and port combination
 *   for a recently closed port.
 * - BIO_SOCK_V6_ONLY: When creating an IPv6 socket, make it listen only
 *   for IPv6 addresses and not IPv4 addresses mapped to IPv6.
 * - BIO_SOCK_TFO: accept TCP fast open (set TCP_FASTOPEN)
 *
 * It's recommended that you set up both an IPv6 and IPv4 listen socket, and
 * then check both for new clients that connect to it.  You want to set up
 * the socket as non-blocking in that case since else it could hang.
 *
 * Not all operating systems support IPv4 addresses on an IPv6 socket, and for
 * others it's an option.  If you pass the BIO_LISTEN_V6_ONLY it will try to
 * create the IPv6 sockets to only listen for IPv6 connection.
 *
 * It could be that the first BIO_listen() call will listen to all the IPv6
 * and IPv4 addresses and that then trying to bind to the IPv4 address will
 * fail.  We can't tell the difference between already listening ourself to
 * it and someone else listening to it when failing and errno is EADDRINUSE, so
 * it's recommended to not give an error in that case if the first call was
 * successful.
 *
 * When restarting the program it could be that the port is still in use.  If
 * you set to BIO_SOCK_REUSEADDR option it will try to reuse the port anyway.
 * It's recommended that you use this.
 */
int BIO_listen(int sock, const BIO_ADDR *addr, int options)
{
    int on = 1;
    int socktype;
    socklen_t socktype_len = sizeof(socktype);

    if (sock == -1) {
        ERR_raise(ERR_LIB_BIO, BIO_R_INVALID_SOCKET);
        return 0;
    }

    if (getsockopt(sock, SOL_SOCKET, SO_TYPE,
                   (void *)&socktype, &socktype_len) != 0
        || socktype_len != sizeof(socktype)) {
        ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                       "calling getsockopt()");
        ERR_raise(ERR_LIB_BIO, BIO_R_GETTING_SOCKTYPE);
        return 0;
    }

    if (!BIO_socket_nbio(sock, (options & BIO_SOCK_NONBLOCK) != 0))
        return 0;

    if (options & BIO_SOCK_KEEPALIVE) {
        if (setsockopt(sock, SOL_SOCKET, SO_KEEPALIVE,
                       (const void *)&on, sizeof(on)) != 0) {
            ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                           "calling setsockopt()");
            ERR_raise(ERR_LIB_BIO, BIO_R_UNABLE_TO_KEEPALIVE);
            return 0;
        }
    }

    if (options & BIO_SOCK_NODELAY) {
        if (setsockopt(sock, IPPROTO_TCP, TCP_NODELAY,
                       (const void *)&on, sizeof(on)) != 0) {
            ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                           "calling setsockopt()");
            ERR_raise(ERR_LIB_BIO, BIO_R_UNABLE_TO_NODELAY);
            return 0;
        }
    }

  /* On OpenBSD it is always IPv6 only with IPv6 sockets thus read-only */
# if defined(IPV6_V6ONLY) && !defined(__OpenBSD__)
    if (BIO_ADDR_family(addr) == AF_INET6) {
        /*
         * Note: Windows default of IPV6_V6ONLY is ON, and Linux is OFF.
         * Therefore we always have to use setsockopt here.
         */
        on = options & BIO_SOCK_V6_ONLY ? 1 : 0;
        if (setsockopt(sock, IPPROTO_IPV6, IPV6_V6ONLY,
                       (const void *)&on, sizeof(on)) != 0) {
            ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                           "calling setsockopt()");
            ERR_raise(ERR_LIB_BIO, BIO_R_LISTEN_V6_ONLY);
            return 0;
        }
    }
# endif

    if (!BIO_bind(sock, addr, options))
        return 0;

    if (socktype != SOCK_DGRAM && listen(sock, MAX_LISTEN) == -1) {
        ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                       "calling listen()");
        ERR_raise(ERR_LIB_BIO, BIO_R_UNABLE_TO_LISTEN_SOCKET);
        return 0;
    }

# if defined(OSSL_TFO_SERVER_SOCKOPT)
    /*
     * Must do it explicitly after listen() for macOS, still
     * works fine on other OS's
     */
    if ((options & BIO_SOCK_TFO) && socktype != SOCK_DGRAM) {
        int q = OSSL_TFO_SERVER_SOCKOPT_VALUE;
#  if defined(OSSL_TFO_CLIENT_FLAG)
#   if defined(OSSL_TFO_SYSCTL_SERVER)
        int enabled = 0;
        size_t enabledlen = sizeof(enabled);

        /* Later FreeBSD */
        if (sysctlbyname(OSSL_TFO_SYSCTL_SERVER, &enabled, &enabledlen, NULL, 0) < 0) {
            ERR_raise(ERR_LIB_BIO, BIO_R_TFO_NO_KERNEL_SUPPORT);
            return 0;
        }
        /* Need to check for server flag */
        if (!(enabled & OSSL_TFO_SERVER_FLAG)) {
            ERR_raise(ERR_LIB_BIO, BIO_R_TFO_DISABLED);
            return 0;
        }
#   elif defined(OSSL_TFO_SYSCTL)
        int enabled = 0;
        size_t enabledlen = sizeof(enabled);

        /* Early FreeBSD, macOS */
        if (sysctlbyname(OSSL_TFO_SYSCTL, &enabled, &enabledlen, NULL, 0) < 0) {
            ERR_raise(ERR_LIB_BIO, BIO_R_TFO_NO_KERNEL_SUPPORT);
            return 0;
        }
        /* Need to check for server flag */
        if (!(enabled & OSSL_TFO_SERVER_FLAG)) {
            ERR_raise(ERR_LIB_BIO, BIO_R_TFO_DISABLED);
            return 0;
        }
#   endif
#  endif
        if (setsockopt(sock, IPPROTO_TCP, OSSL_TFO_SERVER_SOCKOPT,
                       (void *)&q, sizeof(q)) < 0) {
            ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                           "calling setsockopt()");
            ERR_raise(ERR_LIB_BIO, BIO_R_UNABLE_TO_TFO);
            return 0;
        }
    }
# endif

    return 1;
}

/*-
 * BIO_accept_ex - Accept new incoming connections
 * @sock: the listening socket
 * @addr: the BIO_ADDR to store the peer address in
 * @options: BIO socket options, applied on the accepted socket.
 *
 */
int BIO_accept_ex(int accept_sock, BIO_ADDR *addr_, int options)
{
    socklen_t len;
    int accepted_sock;
    BIO_ADDR locaddr;
    BIO_ADDR *addr = addr_ == NULL ? &locaddr : addr_;

    len = sizeof(*addr);
    accepted_sock = accept(accept_sock,
                           BIO_ADDR_sockaddr_noconst(addr), &len);
    if (accepted_sock == -1) {
        if (!BIO_sock_should_retry(accepted_sock)) {
            ERR_raise_data(ERR_LIB_SYS, get_last_socket_error(),
                           "calling accept()");
            ERR_raise(ERR_LIB_BIO, BIO_R_ACCEPT_ERROR);
        }
        return INVALID_SOCKET;
    }

    if (!BIO_socket_nbio(accepted_sock, (options & BIO_SOCK_NONBLOCK) != 0)) {
        closesocket(accepted_sock);
        return INVALID_SOCKET;
    }

    return accepted_sock;
}

/*-
 * BIO_closesocket - Close a socket
 * @sock: the socket to close
 */
int BIO_closesocket(int sock)
{
    if (sock < 0 || closesocket(sock) < 0)
        return 0;
    return 1;
}
#endif
