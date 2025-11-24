/*
 * Copyright 2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/bio.h>
#include "internal/e_os.h"
#include "internal/sockets.h"
#include "internal/bio_tfo.h"
#include "testutil.h"

/* If OS support is added in crypto/bio/bio_tfo.h, add it here */
#if defined(OPENSSL_SYS_LINUX)
# define GOOD_OS 1
#elif defined(__FreeBSD__)
# define GOOD_OS 1
#elif defined(OPENSSL_SYS_MACOSX)
# define GOOD_OS 1
#else
# ifdef GOOD_OS
#  undef GOOD_OS
# endif
#endif

#if !defined(OPENSSL_NO_TFO) && defined(GOOD_OS)

/*
 * This test is to ensure that if TCP Fast Open is configured, that socket
 * connections will still work. These tests are able to detect if TCP Fast
 * Open works, but the tests will pass as long as the socket connects.
 *
 * The first test function tests the socket interface as implemented as BIOs.
 *
 * The second test functions tests the socket interface as implemented as fds.
 *
 * The tests are run 5 times. The first time is without TFO.
 * The second test will create the TCP fast open cookie,
 * this can be seen in `ip tcp_metrics` and in /proc/net/netstat/ on Linux.
 * e.g. on Linux 4.15.0-135-generic:
 * $ grep '^TcpExt:' /proc/net/netstat | cut -d ' ' -f 84-90 | column -t
 * The third attempt will use the cookie and actually do TCP fast open.
 * The 4th time is client-TFO only, the 5th time is server-TFO only.
 */

#  define SOCKET_DATA "FooBar"
#  define SOCKET_DATA_LEN sizeof(SOCKET_DATA)

static int test_bio_tfo(int idx)
{
    BIO *cbio = NULL;
    BIO *abio = NULL;
    BIO *sbio = NULL;
    int ret = 0;
    int sockerr = 0;
    const char *port;
    int server_tfo = 0;
    int client_tfo = 0;
    size_t bytes;
    char read_buffer[20];

    switch (idx) {
    default:
    case 0:
        break;
    case 1:
    case 2:
        server_tfo = 1;
        client_tfo = 1;
        break;
    case 3:
        client_tfo = 1;
        break;
    case 4:
        server_tfo = 1;
        break;
    }

    /* ACCEPT SOCKET */
    if (!TEST_ptr(abio = BIO_new_accept("localhost:0"))
            || !TEST_true(BIO_set_nbio_accept(abio, 1))
            || !TEST_true(BIO_set_tfo_accept(abio, server_tfo))
            || !TEST_int_gt(BIO_do_accept(abio), 0)
            || !TEST_ptr(port = BIO_get_accept_port(abio))) {
        sockerr = get_last_socket_error();
        goto err;
    }

    /* Note: first BIO_do_accept will basically do the bind/listen */

    /* CLIENT SOCKET */
    if (!TEST_ptr(cbio = BIO_new_connect("localhost"))
            || !TEST_long_gt(BIO_set_conn_port(cbio, port), 0)
            || !TEST_long_gt(BIO_set_nbio(cbio, 1), 0)
            || !TEST_long_gt(BIO_set_tfo(cbio, client_tfo), 0)) {
        sockerr = get_last_socket_error();
        goto err;
    }

    /* FIRST ACCEPT: no connection should be established */
    if (BIO_do_accept(abio) <= 0) {
        if (!BIO_should_retry(abio)) {
            sockerr = get_last_socket_error();
            BIO_printf(bio_err, "Error: failed without EAGAIN\n");
            goto err;
        }
    } else {
        sbio = BIO_pop(abio);
        BIO_printf(bio_err, "Error: accepted unknown connection\n");
        goto err;
    }

    /* CONNECT ATTEMPT: different behavior based on TFO support */
    if (BIO_do_connect(cbio) <= 0) {
        sockerr = get_last_socket_error();
        if (sockerr == EOPNOTSUPP) {
            BIO_printf(bio_err, "Skip: TFO not enabled/supported for client\n");
            goto success;
        } else if (sockerr != EINPROGRESS) {
            BIO_printf(bio_err, "Error: failed without EINPROGRESSn");
            goto err;
        }
    }

    /* macOS needs some time for this to happen, so put in a select */
    if (!TEST_int_ge(BIO_wait(abio, time(NULL) + 2, 0), 0)) {
        sockerr = get_last_socket_error();
        BIO_printf(bio_err, "Error: socket wait failed\n");
        goto err;
    }

    /* SECOND ACCEPT: if TFO is supported, this will still fail until data is sent */
    if (BIO_do_accept(abio) <= 0) {
        if (!BIO_should_retry(abio)) {
            sockerr = get_last_socket_error();
            BIO_printf(bio_err, "Error: failed without EAGAIN\n");
            goto err;
        }
    } else {
        if (idx == 0)
            BIO_printf(bio_err, "Success: non-TFO connection accepted without data\n");
        else if (idx == 1)
            BIO_printf(bio_err, "Ignore: connection accepted before data, possibly no TFO cookie, or TFO may not be enabled\n");
        else if (idx == 4)
            BIO_printf(bio_err, "Success: connection accepted before data, client TFO is disabled\n");
        else
            BIO_printf(bio_err, "Warning: connection accepted before data, TFO may not be enabled\n");
        sbio = BIO_pop(abio);
        goto success;
    }

    /* SEND DATA: this should establish the actual TFO connection */
    if (!TEST_true(BIO_write_ex(cbio, SOCKET_DATA, SOCKET_DATA_LEN, &bytes))) {
        sockerr = get_last_socket_error();
        goto err;
    }

    /* macOS needs some time for this to happen, so put in a select */
    if (!TEST_int_ge(BIO_wait(abio, time(NULL) + 2, 0), 0)) {
        sockerr = get_last_socket_error();
        BIO_printf(bio_err, "Error: socket wait failed\n");
        goto err;
    }

    /* FINAL ACCEPT: if TFO is enabled, socket should be accepted at *this* point */
    if (BIO_do_accept(abio) <= 0) {
        sockerr = get_last_socket_error();
        BIO_printf(bio_err, "Error: socket not accepted\n");
        goto err;
    }
    BIO_printf(bio_err, "Success: Server accepted socket after write\n");
    if (!TEST_ptr(sbio = BIO_pop(abio))
            || !TEST_true(BIO_read_ex(sbio, read_buffer, sizeof(read_buffer), &bytes))
            || !TEST_size_t_eq(bytes, SOCKET_DATA_LEN)
            || !TEST_strn_eq(read_buffer, SOCKET_DATA, SOCKET_DATA_LEN)) {
        sockerr = get_last_socket_error();
        goto err;
    }

success:
    sockerr = 0;
    ret = 1;

err:
    if (sockerr != 0) {
        const char *errstr = strerror(sockerr);

        if (errstr != NULL)
            BIO_printf(bio_err, "last errno: %d=%s\n", sockerr, errstr);
    }
    BIO_free(cbio);
    BIO_free(abio);
    BIO_free(sbio);
    return ret;
}

static int test_fd_tfo(int idx)
{
    struct sockaddr_storage sstorage;
    socklen_t slen;
    struct addrinfo *ai = NULL;
    struct addrinfo hints;
    int ret = 0;
    int cfd = -1; /* client socket */
    int afd = -1; /* accept socket */
    int sfd = -1; /* server accepted socket */
    BIO_ADDR *baddr = NULL;
    char read_buffer[20];
    int bytes_read;
    int server_flags = BIO_SOCK_NONBLOCK;
    int client_flags = BIO_SOCK_NONBLOCK;
    int sockerr = 0;
    unsigned short port;
    void *addr;
    size_t addrlen;

    switch (idx) {
    default:
    case 0:
        break;
    case 1:
    case 2:
        server_flags |= BIO_SOCK_TFO;
        client_flags |= BIO_SOCK_TFO;
        break;
    case 3:
        client_flags |= BIO_SOCK_TFO;
        break;
    case 4:
        server_flags |= BIO_SOCK_TFO;
        break;
    }

    /* ADDRESS SETUP */
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;
    if (!TEST_int_eq(getaddrinfo(NULL, "0", &hints, &ai), 0))
        goto err;

    switch (ai->ai_family) {
        case AF_INET:
            port = ((struct sockaddr_in *)ai->ai_addr)->sin_port;
            addr = &((struct sockaddr_in *)ai->ai_addr)->sin_addr;
            addrlen = sizeof(((struct sockaddr_in *)ai->ai_addr)->sin_addr);
            BIO_printf(bio_err, "Using IPv4\n");
            break;
        case AF_INET6:
            port = ((struct sockaddr_in6 *)ai->ai_addr)->sin6_port;
            addr = &((struct sockaddr_in6 *)ai->ai_addr)->sin6_addr;
            addrlen = sizeof(((struct sockaddr_in6 *)ai->ai_addr)->sin6_addr);
            BIO_printf(bio_err, "Using IPv6\n");
            break;
        default:
            BIO_printf(bio_err, "Unknown address family %d\n", ai->ai_family);
            goto err;
    }

    if (!TEST_ptr(baddr = BIO_ADDR_new())
            || !TEST_true(BIO_ADDR_rawmake(baddr, ai->ai_family, addr, addrlen, port)))
        goto err;

    /* ACCEPT SOCKET */

    if (!TEST_int_ge(afd = BIO_socket(ai->ai_family, ai->ai_socktype, ai->ai_protocol, 0), 0)
            || !TEST_true(BIO_listen(afd, baddr, server_flags)))
        goto err;

    /* UPDATE ADDRESS WITH PORT */
    slen = sizeof(sstorage);
    if (!TEST_int_ge(getsockname(afd, (struct sockaddr *)&sstorage, &slen), 0))
        goto err;

    switch (sstorage.ss_family) {
        case AF_INET:
            port = ((struct sockaddr_in *)&sstorage)->sin_port;
            addr = &((struct sockaddr_in *)&sstorage)->sin_addr;
            addrlen = sizeof(((struct sockaddr_in *)&sstorage)->sin_addr);
            break;
        case AF_INET6:
            port = ((struct sockaddr_in6 *)&sstorage)->sin6_port;
            addr = &((struct sockaddr_in6 *)&sstorage)->sin6_addr;
            addrlen = sizeof(((struct sockaddr_in6 *)&sstorage)->sin6_addr);
            break;
        default:
            goto err;
    }

    if(!TEST_true(BIO_ADDR_rawmake(baddr, sstorage.ss_family, addr, addrlen, port)))
        goto err;

    /* CLIENT SOCKET */
    if (!TEST_int_ge(cfd = BIO_socket(ai->ai_family, ai->ai_socktype, ai->ai_protocol, 0), 0))
        goto err;

    /* FIRST ACCEPT: no connection should be established */
    sfd = BIO_accept_ex(afd, NULL, 0);
    if (sfd == -1) {
        sockerr = get_last_socket_error();
        /* Note: Windows would hit WSAEWOULDBLOCK */
        if (sockerr != EAGAIN) {
            BIO_printf(bio_err, "Error: failed without EAGAIN\n");
            goto err;
        }
    } else {
        BIO_printf(bio_err, "Error: accepted unknown connection\n");
        goto err;
    }

    /* CONNECT ATTEMPT: different behavior based on TFO support */
    if (!BIO_connect(cfd, baddr, client_flags)) {
        sockerr = get_last_socket_error();
        if (sockerr == EOPNOTSUPP) {
            BIO_printf(bio_err, "Skip: TFO not enabled/supported for client\n");
            goto success;
        } else {
            /* Note: Windows would hit WSAEWOULDBLOCK */
            if (sockerr != EINPROGRESS) {
                BIO_printf(bio_err, "Error: failed without EINPROGRESS\n");
                goto err;
            }
        }
    }

    /* macOS needs some time for this to happen, so put in a select */
    if (!TEST_int_ge(BIO_socket_wait(afd, 1, time(NULL) + 2), 0)) {
        sockerr = get_last_socket_error();
        BIO_printf(bio_err, "Error: socket wait failed\n");
        goto err;
    }

    /* SECOND ACCEPT: if TFO is supported, this will still fail until data is sent */
    sfd = BIO_accept_ex(afd, NULL, 0);
    if (sfd == -1) {
        sockerr = get_last_socket_error();
        /* Note: Windows would hit WSAEWOULDBLOCK */
        if (sockerr != EAGAIN) {
            BIO_printf(bio_err, "Error: failed without EAGAIN\n");
            goto err;
        }
    } else {
        if (idx == 0)
            BIO_printf(bio_err, "Success: non-TFO connection accepted without data\n");
        else if (idx == 1)
            BIO_printf(bio_err, "Ignore: connection accepted before data, possibly no TFO cookie, or TFO may not be enabled\n");
        else if (idx == 4)
            BIO_printf(bio_err, "Success: connection accepted before data, client TFO is disabled\n");
        else
            BIO_printf(bio_err, "Warning: connection accepted before data, TFO may not be enabled\n");
        goto success;
    }

    /* SEND DATA: this should establish the actual TFO connection */
#ifdef OSSL_TFO_SENDTO
    if (!TEST_int_ge(sendto(cfd, SOCKET_DATA, SOCKET_DATA_LEN, OSSL_TFO_SENDTO,
                            (struct sockaddr *)&sstorage, slen), 0)) {
        sockerr = get_last_socket_error();
        goto err;
    }
#else
    if (!TEST_int_ge(writesocket(cfd, SOCKET_DATA, SOCKET_DATA_LEN), 0)) {
        sockerr = get_last_socket_error();
        goto err;
    }
#endif

    /* macOS needs some time for this to happen, so put in a select */
    if (!TEST_int_ge(BIO_socket_wait(afd, 1, time(NULL) + 2), 0)) {
        sockerr = get_last_socket_error();
        BIO_printf(bio_err, "Error: socket wait failed\n");
        goto err;
    }

    /* FINAL ACCEPT: if TFO is enabled, socket should be accepted at *this* point */
    sfd = BIO_accept_ex(afd, NULL, 0);
    if (sfd == -1) {
        sockerr = get_last_socket_error();
        BIO_printf(bio_err, "Error: socket not accepted\n");
        goto err;
    }
    BIO_printf(bio_err, "Success: Server accepted socket after write\n");
    bytes_read = readsocket(sfd, read_buffer, sizeof(read_buffer));
    if (!TEST_int_eq(bytes_read, SOCKET_DATA_LEN)
            || !TEST_strn_eq(read_buffer, SOCKET_DATA, SOCKET_DATA_LEN)) {
        sockerr = get_last_socket_error();
        goto err;
    }

success:
    sockerr = 0;
    ret = 1;

err:
    if (sockerr != 0) {
        const char *errstr = strerror(sockerr);

        if (errstr != NULL)
            BIO_printf(bio_err, "last errno: %d=%s\n", sockerr, errstr);
    }
    if (ai != NULL)
        freeaddrinfo(ai);
    BIO_ADDR_free(baddr);
    BIO_closesocket(cfd);
    BIO_closesocket(sfd);
    BIO_closesocket(afd);
    return ret;
}
#endif

int setup_tests(void)
{
#if !defined(OPENSSL_NO_TFO) && defined(GOOD_OS)
    ADD_ALL_TESTS(test_bio_tfo, 5);
    ADD_ALL_TESTS(test_fd_tfo, 5);
#endif
    return 1;
}
