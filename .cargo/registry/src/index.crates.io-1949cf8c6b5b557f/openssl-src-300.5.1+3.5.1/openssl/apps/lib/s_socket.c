/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* socket-related functions used by s_client and s_server */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <signal.h>
#include <openssl/opensslconf.h>

/*
 * With IPv6, it looks like Digital has mixed up the proper order of
 * recursive header file inclusion, resulting in the compiler complaining
 * that u_int isn't defined, but only if _POSIX_C_SOURCE is defined, which is
 * needed to have fileno() declared correctly...  So let's define u_int
 */
#if defined(OPENSSL_SYS_VMS_DECC) && !defined(__U_INT)
# define __U_INT
typedef unsigned int u_int;
#endif

#ifdef _WIN32
# include <process.h>

/* MSVC renamed some POSIX functions to have an underscore prefix. */
# ifdef _MSC_VER
#  define getpid _getpid
# endif
#endif

#ifndef OPENSSL_NO_SOCK

# include "internal/e_os.h"
# include "apps.h"
# include "s_apps.h"
# include "internal/sockets.h" /* for openssl_fdset() */

# include <openssl/bio.h>
# include <openssl/err.h>

/* Keep track of our peer's address for the cookie callback */
BIO_ADDR *ourpeer = NULL;

/*
 * init_client - helper routine to set up socket communication
 * @sock: pointer to storage of resulting socket.
 * @host: the hostname or path (for AF_UNIX) to connect to.
 * @port: the port to connect to (ignored for AF_UNIX).
 * @bindhost: source host or path (for AF_UNIX).
 * @bindport: source port (ignored for AF_UNIX).
 * @family: desired socket family, may be AF_INET, AF_INET6, AF_UNIX or
 *  AF_UNSPEC
 * @type: socket type, must be SOCK_STREAM or SOCK_DGRAM
 * @protocol: socket protocol, e.g. IPPROTO_TCP or IPPROTO_UDP (or 0 for any)
 * @tfo: flag to enable TCP Fast Open
 * @doconn: whether we should call BIO_connect() on the socket
 * @ba_ret: BIO_ADDR for the remote peer, to be freed by caller
 *
 * This will create a socket and use it to connect to a host:port, or if
 * family == AF_UNIX, to the path found in host.
 *
 * If the host has more than one address, it will try them one by one until
 * a successful connection is established.  The resulting socket will be
 * found in *sock on success, it will be given INVALID_SOCKET otherwise.
 *
 * Returns 1 on success, 0 on failure.
 */
int init_client(int *sock, const char *host, const char *port,
                const char *bindhost, const char *bindport,
                int family, int type, int protocol, int tfo, int doconn,
                BIO_ADDR **ba_ret)
{
    BIO_ADDRINFO *res = NULL;
    BIO_ADDRINFO *bindaddr = NULL;
    const BIO_ADDRINFO *ai = NULL;
    const BIO_ADDRINFO *bi = NULL;
    int found = 0;
    int ret;
    int options = 0;

    if (BIO_sock_init() != 1)
        return 0;

    ret = BIO_lookup_ex(host, port, BIO_LOOKUP_CLIENT, family, type, protocol,
                        &res);
    if (ret == 0) {
        ERR_print_errors(bio_err);
        return 0;
    }

    if (bindhost != NULL || bindport != NULL) {
        ret = BIO_lookup_ex(bindhost, bindport, BIO_LOOKUP_CLIENT,
                            family, type, protocol, &bindaddr);
        if (ret == 0) {
            ERR_print_errors (bio_err);
            goto out;
        }
    }

    ret = 0;
    for (ai = res; ai != NULL; ai = BIO_ADDRINFO_next(ai)) {
        /* Admittedly, these checks are quite paranoid, we should not get
         * anything in the BIO_ADDRINFO chain that we haven't
         * asked for. */
        OPENSSL_assert((family == AF_UNSPEC
                        || family == BIO_ADDRINFO_family(ai))
                       && (type == 0 || type == BIO_ADDRINFO_socktype(ai))
                       && (protocol == 0
                           || protocol == BIO_ADDRINFO_protocol(ai)));

        if (bindaddr != NULL) {
            for (bi = bindaddr; bi != NULL; bi = BIO_ADDRINFO_next(bi)) {
                if (BIO_ADDRINFO_family(bi) == BIO_ADDRINFO_family(ai))
                    break;
            }
            if (bi == NULL)
                continue;
            ++found;
        }

        *sock = BIO_socket(BIO_ADDRINFO_family(ai), BIO_ADDRINFO_socktype(ai),
                           BIO_ADDRINFO_protocol(ai), 0);
        if (*sock == INVALID_SOCKET) {
            /* Maybe the kernel doesn't support the socket family, even if
             * BIO_lookup() added it in the returned result...
             */
            continue;
        }

        if (bi != NULL) {
            if (!BIO_bind(*sock, BIO_ADDRINFO_address(bi),
                          BIO_SOCK_REUSEADDR)) {
                BIO_closesocket(*sock);
                *sock = INVALID_SOCKET;
                break;
            }
        }

#ifndef OPENSSL_NO_SCTP
        if (protocol == IPPROTO_SCTP) {
            /*
             * For SCTP we have to set various options on the socket prior to
             * connecting. This is done automatically by BIO_new_dgram_sctp().
             * We don't actually need the created BIO though so we free it again
             * immediately.
             */
            BIO *tmpbio = BIO_new_dgram_sctp(*sock, BIO_NOCLOSE);

            if (tmpbio == NULL) {
                ERR_print_errors(bio_err);
                return 0;
            }
            BIO_free(tmpbio);
        }
#endif
        if (BIO_ADDRINFO_protocol(ai) == IPPROTO_TCP) {
            options |= BIO_SOCK_NODELAY;
            if (tfo)
                options |= BIO_SOCK_TFO;
        }

        if (doconn && !BIO_connect(*sock, BIO_ADDRINFO_address(ai), options)) {
            BIO_closesocket(*sock);
            *sock = INVALID_SOCKET;
            continue;
        }

        /* Save the address */
        if (tfo || !doconn) {
            if (ba_ret == NULL) {
                BIO_printf(bio_err, "Internal error\n");
                BIO_closesocket(*sock);
                *sock = INVALID_SOCKET;
                goto out;
            }

            *ba_ret = BIO_ADDR_dup(BIO_ADDRINFO_address(ai));
        }

        /* Success, don't try any more addresses */
        break;
    }

    if (*sock == INVALID_SOCKET) {
        if (bindaddr != NULL && !found) {
            BIO_printf(bio_err, "Can't bind %saddress for %s%s%s\n",
#ifdef AF_INET6
                       BIO_ADDRINFO_family(res) == AF_INET6 ? "IPv6 " :
#endif
                       BIO_ADDRINFO_family(res) == AF_INET ? "IPv4 " :
                       BIO_ADDRINFO_family(res) == AF_UNIX ? "unix " : "",
                       bindhost != NULL ? bindhost : "",
                       bindport != NULL ? ":" : "",
                       bindport != NULL ? bindport : "");
            ERR_clear_error();
            ret = 0;
        }
        ERR_print_errors(bio_err);
    } else {
        char *hostname = NULL;

        hostname = BIO_ADDR_hostname_string(BIO_ADDRINFO_address(ai), 1);
        if (hostname != NULL) {
            BIO_printf(bio_err, "Connecting to %s\n", hostname);
            OPENSSL_free(hostname);
        }
        /* Remove any stale errors from previous connection attempts */
        ERR_clear_error();
        ret = 1;
    }
out:
    if (bindaddr != NULL) {
        BIO_ADDRINFO_free (bindaddr);
    }
    BIO_ADDRINFO_free(res);
    return ret;
}

void get_sock_info_address(int asock, char **hostname, char **service)
{
    union BIO_sock_info_u info;

    if (hostname != NULL)
        *hostname = NULL;
    if (service != NULL)
        *service = NULL;

    if ((info.addr = BIO_ADDR_new()) != NULL
            && BIO_sock_info(asock, BIO_SOCK_INFO_ADDRESS, &info)) {
        if (hostname != NULL)
            *hostname = BIO_ADDR_hostname_string(info.addr, 1);
        if (service != NULL)
            *service = BIO_ADDR_service_string(info.addr, 1);
    }
    BIO_ADDR_free(info.addr);
}

int report_server_accept(BIO *out, int asock, int with_address, int with_pid)
{
    int success = 1;

    if (BIO_printf(out, "ACCEPT") <= 0)
        return 0;
    if (with_address) {
        char *hostname, *service;

        get_sock_info_address(asock, &hostname, &service);
        success = hostname != NULL && service != NULL;
        if (success)
            success = BIO_printf(out,
                                 strchr(hostname, ':') == NULL
                                 ? /* IPv4 */ " %s:%s"
                                 : /* IPv6 */ " [%s]:%s",
                                 hostname, service) > 0;
        else
            (void)BIO_printf(out, "unknown:error\n");
        OPENSSL_free(hostname);
        OPENSSL_free(service);
    }
    if (with_pid)
        success *= BIO_printf(out, " PID=%d", getpid()) > 0;
    success *= BIO_printf(out, "\n") > 0;
    (void)BIO_flush(out);

    return success;
}

/*
 * do_server - helper routine to perform a server operation
 * @accept_sock: pointer to storage of resulting socket.
 * @host: the hostname or path (for AF_UNIX) to connect to.
 * @port: the port to connect to (ignored for AF_UNIX).
 * @family: desired socket family, may be AF_INET, AF_INET6, AF_UNIX or
 *  AF_UNSPEC
 * @type: socket type, must be SOCK_STREAM or SOCK_DGRAM
 * @cb: pointer to a function that receives the accepted socket and
 *  should perform the communication with the connecting client.
 * @context: pointer to memory that's passed verbatim to the cb function.
 * @naccept: number of times an incoming connect should be accepted.  If -1,
 *  unlimited number.
 *
 * This will create a socket and use it to listen to a host:port, or if
 * family == AF_UNIX, to the path found in host, then start accepting
 * incoming connections and run cb on the resulting socket.
 *
 * 0 on failure, something other on success.
 */
int do_server(int *accept_sock, const char *host, const char *port,
              int family, int type, int protocol, do_server_cb cb,
              unsigned char *context, int naccept, BIO *bio_s_out,
              int tfo)
{
    int asock = 0;
    int sock;
    int i;
    BIO_ADDRINFO *res = NULL;
    const BIO_ADDRINFO *next;
    int sock_family, sock_type, sock_protocol, sock_port;
    const BIO_ADDR *sock_address;
    int sock_family_fallback = AF_UNSPEC;
    const BIO_ADDR *sock_address_fallback = NULL;
    int sock_options = BIO_SOCK_REUSEADDR;
    int ret = 0;

    if (BIO_sock_init() != 1)
        return 0;

    if (!BIO_lookup_ex(host, port, BIO_LOOKUP_SERVER, family, type, protocol,
                       &res)) {
        ERR_print_errors(bio_err);
        return 0;
    }

    /* Admittedly, these checks are quite paranoid, we should not get
     * anything in the BIO_ADDRINFO chain that we haven't asked for */
    OPENSSL_assert((family == AF_UNSPEC || family == BIO_ADDRINFO_family(res))
                   && (type == 0 || type == BIO_ADDRINFO_socktype(res))
                   && (protocol == 0 || protocol == BIO_ADDRINFO_protocol(res)));

    sock_family = BIO_ADDRINFO_family(res);
    sock_type = BIO_ADDRINFO_socktype(res);
    sock_protocol = BIO_ADDRINFO_protocol(res);
    sock_address = BIO_ADDRINFO_address(res);
    next = BIO_ADDRINFO_next(res);
    if (tfo && sock_type == SOCK_STREAM)
        sock_options |= BIO_SOCK_TFO;
#ifdef AF_INET6
    if (sock_family == AF_INET6)
        sock_options |= BIO_SOCK_V6_ONLY;
    if (next != NULL
            && BIO_ADDRINFO_socktype(next) == sock_type
            && BIO_ADDRINFO_protocol(next) == sock_protocol) {
        if (sock_family == AF_INET
                && BIO_ADDRINFO_family(next) == AF_INET6) {
            /* In case AF_INET6 is returned but not supported by the
             * kernel, retry with the first detected address family */
            sock_family_fallback = sock_family;
            sock_address_fallback = sock_address;
            sock_family = AF_INET6;
            sock_address = BIO_ADDRINFO_address(next);
        } else if (sock_family == AF_INET6
                   && BIO_ADDRINFO_family(next) == AF_INET) {
            sock_options &= ~BIO_SOCK_V6_ONLY;
        }
    }
#endif

    asock = BIO_socket(sock_family, sock_type, sock_protocol, 0);
    if (asock == INVALID_SOCKET && sock_family_fallback != AF_UNSPEC) {
        asock = BIO_socket(sock_family_fallback, sock_type, sock_protocol, 0);
        sock_address = sock_address_fallback;
    }
    if (asock == INVALID_SOCKET
        || !BIO_listen(asock, sock_address, sock_options)) {
        BIO_ADDRINFO_free(res);
        ERR_print_errors(bio_err);
        if (asock != INVALID_SOCKET)
            BIO_closesocket(asock);
        goto end;
    }

#ifndef OPENSSL_NO_SCTP
    if (protocol == IPPROTO_SCTP) {
        /*
         * For SCTP we have to set various options on the socket prior to
         * accepting. This is done automatically by BIO_new_dgram_sctp().
         * We don't actually need the created BIO though so we free it again
         * immediately.
         */
        BIO *tmpbio = BIO_new_dgram_sctp(asock, BIO_NOCLOSE);

        if (tmpbio == NULL) {
            BIO_closesocket(asock);
            ERR_print_errors(bio_err);
            goto end;
        }
        BIO_free(tmpbio);
    }
#endif

    sock_port = BIO_ADDR_rawport(sock_address);

    BIO_ADDRINFO_free(res);
    res = NULL;

    if (!report_server_accept(bio_s_out, asock, sock_port == 0, 0)) {
        BIO_closesocket(asock);
        ERR_print_errors(bio_err);
        goto end;
    }

    if (accept_sock != NULL)
        *accept_sock = asock;
    for (;;) {
        char sink[64];
        struct timeval timeout;
        fd_set readfds;

        if (type == SOCK_STREAM) {
            BIO_ADDR_free(ourpeer);
            ourpeer = BIO_ADDR_new();
            if (ourpeer == NULL) {
                BIO_closesocket(asock);
                ERR_print_errors(bio_err);
                goto end;
            }
            do {
                sock = BIO_accept_ex(asock, ourpeer, 0);
            } while (sock < 0 && BIO_sock_should_retry(sock));
            if (sock < 0) {
                ERR_print_errors(bio_err);
                BIO_closesocket(asock);
                break;
            }

            if (naccept != -1)
                naccept--;
            if (naccept == 0)
                BIO_closesocket(asock);

            BIO_set_tcp_ndelay(sock, 1);
            i = (*cb)(sock, type, protocol, context);

            /*
             * If we ended with an alert being sent, but still with data in the
             * network buffer to be read, then calling BIO_closesocket() will
             * result in a TCP-RST being sent. On some platforms (notably
             * Windows) then this will result in the peer immediately abandoning
             * the connection including any buffered alert data before it has
             * had a chance to be read. Shutting down the sending side first,
             * and then closing the socket sends TCP-FIN first followed by
             * TCP-RST. This seems to allow the peer to read the alert data.
             */
            shutdown(sock, 1); /* SHUT_WR */
            /*
             * We just said we have nothing else to say, but it doesn't mean
             * that the other side has nothing. It's even recommended to
             * consume incoming data. [In testing context this ensures that
             * alerts are passed on...]
             */
            timeout.tv_sec = 0;
            timeout.tv_usec = 500000;  /* some extreme round-trip */
            do {
                FD_ZERO(&readfds);
                openssl_fdset(sock, &readfds);
            } while (select(sock + 1, &readfds, NULL, NULL, &timeout) > 0
                     && readsocket(sock, sink, sizeof(sink)) > 0);

            BIO_closesocket(sock);
        } else {
            if (naccept != -1)
                naccept--;

            i = (*cb)(asock, type, protocol, context);
        }

        if (i < 0 || naccept == 0) {
            BIO_closesocket(asock);
            ret = i;
            break;
        }
    }
 end:
# ifdef AF_UNIX
    if (family == AF_UNIX)
        unlink(host);
# endif
    BIO_ADDR_free(ourpeer);
    ourpeer = NULL;
    return ret;
}

void do_ssl_shutdown(SSL *ssl)
{
    int ret;

    do {
        /* We only do unidirectional shutdown */
        ret = SSL_shutdown(ssl);
        if (ret < 0) {
            switch (SSL_get_error(ssl, ret)) {
            case SSL_ERROR_WANT_READ:
            case SSL_ERROR_WANT_WRITE:
            case SSL_ERROR_WANT_ASYNC:
            case SSL_ERROR_WANT_ASYNC_JOB:
                /* We just do busy waiting. Nothing clever */
                continue;
            }
            ret = 0;
        }
    } while (ret < 0);
}

#endif  /* OPENSSL_NO_SOCK */
