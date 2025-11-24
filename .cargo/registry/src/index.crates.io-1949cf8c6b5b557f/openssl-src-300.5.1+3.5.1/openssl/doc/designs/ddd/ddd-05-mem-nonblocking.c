#include <sys/poll.h>
#include <openssl/ssl.h>

/*
 * Demo 5: Client — Client Uses Memory BIO — Nonblocking
 * =====================================================
 *
 * This is an example of (part of) an application which uses libssl in an
 * asynchronous, nonblocking fashion. The application passes memory BIOs to
 * OpenSSL, meaning that it controls both when data is read/written from an SSL
 * object on the decrypted side but also when encrypted data from the network is
 * shunted to/from OpenSSL. In this way OpenSSL is used as a pure state machine
 * which does not make its own network I/O calls. OpenSSL never sees or creates
 * any file descriptor for a network socket. The functions below show all
 * interactions with libssl the application makes, and would hypothetically be
 * linked into a larger application.
 */
typedef struct app_conn_st {
    SSL *ssl;
    BIO *ssl_bio, *net_bio;
    int rx_need_tx, tx_need_rx;
} APP_CONN;

/*
 * The application is initializing and wants an SSL_CTX which it will use for
 * some number of outgoing connections, which it creates in subsequent calls to
 * new_conn. The application may also call this function multiple times to
 * create multiple SSL_CTX.
 */
SSL_CTX *create_ssl_ctx(void)
{
    SSL_CTX *ctx;

#ifdef USE_QUIC
    ctx = SSL_CTX_new(OSSL_QUIC_client_method());
#else
    ctx = SSL_CTX_new(TLS_client_method());
#endif
    if (ctx == NULL)
        return NULL;

    /* Enable trust chain verification. */
    SSL_CTX_set_verify(ctx, SSL_VERIFY_PEER, NULL);

    /* Load default root CA store. */
    if (SSL_CTX_set_default_verify_paths(ctx) == 0) {
        SSL_CTX_free(ctx);
        return NULL;
    }

    return ctx;
}

/*
 * The application wants to create a new outgoing connection using a given
 * SSL_CTX.
 *
 * hostname is a string like "openssl.org" used for certificate validation.
 */
APP_CONN *new_conn(SSL_CTX *ctx, const char *bare_hostname)
{
    BIO *ssl_bio, *internal_bio, *net_bio;
    APP_CONN *conn;
    SSL *ssl;
#ifdef USE_QUIC
    static const unsigned char alpn[] = {5, 'd', 'u', 'm', 'm', 'y'};
#endif

    conn = calloc(1, sizeof(APP_CONN));
    if (conn == NULL)
        return NULL;

    ssl = conn->ssl = SSL_new(ctx);
    if (ssl == NULL) {
        free(conn);
        return NULL;
    }

    SSL_set_connect_state(ssl); /* cannot fail */

#ifdef USE_QUIC
    if (BIO_new_bio_dgram_pair(&internal_bio, 0, &net_bio, 0) <= 0) {
#else
    if (BIO_new_bio_pair(&internal_bio, 0, &net_bio, 0) <= 0) {
#endif
        SSL_free(ssl);
        free(conn);
        return NULL;
    }

    SSL_set_bio(ssl, internal_bio, internal_bio);

    if (SSL_set1_host(ssl, bare_hostname) <= 0) {
        SSL_free(ssl);
        free(conn);
        return NULL;
    }

    if (SSL_set_tlsext_host_name(ssl, bare_hostname) <= 0) {
        SSL_free(ssl);
        free(conn);
        return NULL;
    }

    ssl_bio = BIO_new(BIO_f_ssl());
    if (ssl_bio == NULL) {
        SSL_free(ssl);
        free(conn);
        return NULL;
    }

    if (BIO_set_ssl(ssl_bio, ssl, BIO_CLOSE) <= 0) {
        SSL_free(ssl);
        BIO_free(ssl_bio);
        return NULL;
    }

#ifdef USE_QUIC
    /* Configure ALPN, which is required for QUIC. */
    if (SSL_set_alpn_protos(ssl, alpn, sizeof(alpn))) {
        /* Note: SSL_set_alpn_protos returns 1 for failure. */
        SSL_free(ssl);
        BIO_free(ssl_bio);
        return NULL;
    }
#endif

    conn->ssl_bio   = ssl_bio;
    conn->net_bio   = net_bio;
    return conn;
}

/*
 * Non-blocking transmission.
 *
 * Returns -1 on error. Returns -2 if the function would block (corresponds to
 * EWOULDBLOCK).
 */
int tx(APP_CONN *conn, const void *buf, int buf_len)
{
    int rc, l;

    l = BIO_write(conn->ssl_bio, buf, buf_len);
    if (l <= 0) {
        rc = SSL_get_error(conn->ssl, l);
        switch (rc) {
            case SSL_ERROR_WANT_READ:
                conn->tx_need_rx = 1;
            case SSL_ERROR_WANT_CONNECT:
            case SSL_ERROR_WANT_WRITE:
                return -2;
            default:
                return -1;
        }
    } else {
        conn->tx_need_rx = 0;
    }

    return l;
}

/*
 * Non-blocking reception.
 *
 * Returns -1 on error. Returns -2 if the function would block (corresponds to
 * EWOULDBLOCK).
 */
int rx(APP_CONN *conn, void *buf, int buf_len)
{
    int rc, l;

    l = BIO_read(conn->ssl_bio, buf, buf_len);
    if (l <= 0) {
        rc = SSL_get_error(conn->ssl, l);
        switch (rc) {
            case SSL_ERROR_WANT_WRITE:
                conn->rx_need_tx = 1;
            case SSL_ERROR_WANT_READ:
                return -2;
            default:
                return -1;
        }
    } else {
        conn->rx_need_tx = 0;
    }

    return l;
}

/*
 * Called to get data which has been enqueued for transmission to the network
 * by OpenSSL. For QUIC, this always outputs a single datagram.
 *
 * IMPORTANT (QUIC): If buf_len is inadequate to hold the datagram, it is truncated
 * (similar to read(2)). A buffer size of at least 1472 must be used by default
 * to guarantee this does not occur.
 */
int read_net_tx(APP_CONN *conn, void *buf, int buf_len)
{
    return BIO_read(conn->net_bio, buf, buf_len);
}

/*
 * Called to feed data which has been received from the network to OpenSSL.
 *
 * QUIC: buf must contain the entirety of a single datagram. It will be consumed
 * entirely (return value == buf_len) or not at all.
 */
int write_net_rx(APP_CONN *conn, const void *buf, int buf_len)
{
    return BIO_write(conn->net_bio, buf, buf_len);
}

/*
 * Determine how much data can be written to the network RX BIO.
 */
size_t net_rx_space(APP_CONN *conn)
{
    return BIO_ctrl_get_write_guarantee(conn->net_bio);
}

/*
 * Determine how much data is currently queued for transmission in the network
 * TX BIO.
 */
size_t net_tx_avail(APP_CONN *conn)
{
    return BIO_ctrl_pending(conn->net_bio);
}

/*
 * These functions returns zero or more of:
 *
 *   POLLIN:    The SSL state machine is interested in socket readability events.
 *
 *   POLLOUT:   The SSL state machine is interested in socket writeability events.
 *
 *   POLLERR:   The SSL state machine is interested in socket error events.
 *
 * get_conn_pending_tx returns events which may cause SSL_write to make
 * progress and get_conn_pending_rx returns events which may cause SSL_read
 * to make progress.
 */
int get_conn_pending_tx(APP_CONN *conn)
{
#ifdef USE_QUIC
    return (SSL_net_read_desired(conn->ssl) ? POLLIN : 0)
           | (SSL_net_write_desired(conn->ssl) ? POLLOUT : 0)
           | POLLERR;
#else
    return (conn->tx_need_rx ? POLLIN : 0) | POLLOUT | POLLERR;
#endif
}

int get_conn_pending_rx(APP_CONN *conn)
{
#ifdef USE_QUIC
    return get_conn_pending_tx(conn);
#else
    return (conn->rx_need_tx ? POLLOUT : 0) | POLLIN | POLLERR;
#endif
}

/*
 * The application wants to close the connection and free bookkeeping
 * structures.
 */
void teardown(APP_CONN *conn)
{
    BIO_free_all(conn->ssl_bio);
    BIO_free_all(conn->net_bio);
    free(conn);
}

/*
 * The application is shutting down and wants to free a previously
 * created SSL_CTX.
 */
void teardown_ctx(SSL_CTX *ctx)
{
    SSL_CTX_free(ctx);
}

/*
 * ============================================================================
 * Example driver for the above code. This is just to demonstrate that the code
 * works and is not intended to be representative of a real application.
 */
#include <sys/types.h>
#include <sys/socket.h>
#include <sys/signal.h>
#include <netdb.h>
#include <unistd.h>
#include <fcntl.h>
#include <errno.h>

static int pump(APP_CONN *conn, int fd, int events, int timeout)
{
    int l, l2;
    char buf[2048]; /* QUIC: would need to be changed if < 1472 */
    size_t wspace;
    struct pollfd pfd = {0};

    pfd.fd = fd;
    pfd.events = (events & (POLLIN | POLLERR));
    if (net_rx_space(conn) == 0)
        pfd.events &= ~POLLIN;
    if (net_tx_avail(conn) > 0)
        pfd.events |= POLLOUT;

    if ((pfd.events & (POLLIN|POLLOUT)) == 0)
        return 1;

    if (poll(&pfd, 1, timeout) == 0)
        return -1;

    if (pfd.revents & POLLIN) {
        while ((wspace = net_rx_space(conn)) > 0) {
            l = read(fd, buf, wspace > sizeof(buf) ? sizeof(buf) : wspace);
            if (l <= 0) {
                switch (errno) {
                    case EAGAIN:
                        goto stop;
                    default:
                        if (l == 0) /* EOF */
                            goto stop;

                        fprintf(stderr, "error on read: %d\n", errno);
                        return -1;
                }
                break;
            }
            l2 = write_net_rx(conn, buf, l);
            if (l2 < l)
                fprintf(stderr, "short write %d %d\n", l2, l);
        } stop:;
    }

    if (pfd.revents & POLLOUT) {
        for (;;) {
            l = read_net_tx(conn, buf, sizeof(buf));
            if (l <= 0)
                break;
            l2 = write(fd, buf, l);
            if (l2 < l)
                fprintf(stderr, "short read %d %d\n", l2, l);
        }
    }

    return 1;
}

int main(int argc, char **argv)
{
    int rc, fd = -1, res = 1;
    static char tx_msg[300];
    const char *tx_p = tx_msg;
    char rx_buf[2048];
    int l, tx_len;
    int timeout = 2000 /* ms */;
    APP_CONN *conn = NULL;
    struct addrinfo hints = {0}, *result = NULL;
    SSL_CTX *ctx = NULL;

    if (argc < 3) {
        fprintf(stderr, "usage: %s host port\n", argv[0]);
        goto fail;
    }

    tx_len = snprintf(tx_msg, sizeof(tx_msg),
                      "GET / HTTP/1.0\r\nHost: %s\r\n\r\n",
                      argv[1]);

    ctx = create_ssl_ctx();
    if (ctx == NULL) {
        fprintf(stderr, "cannot create SSL context\n");
        goto fail;
    }

    hints.ai_family     = AF_INET;
    hints.ai_socktype   = SOCK_STREAM;
    hints.ai_flags      = AI_PASSIVE;
    rc = getaddrinfo(argv[1], argv[2], &hints, &result);
    if (rc < 0) {
        fprintf(stderr, "cannot resolve\n");
        goto fail;
    }

    signal(SIGPIPE, SIG_IGN);

#ifdef USE_QUIC
    fd = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
#else
    fd = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
#endif
    if (fd < 0) {
        fprintf(stderr, "cannot create socket\n");
        goto fail;
    }

    rc = connect(fd, result->ai_addr, result->ai_addrlen);
    if (rc < 0) {
        fprintf(stderr, "cannot connect\n");
        goto fail;
    }

    rc = fcntl(fd, F_SETFL, O_NONBLOCK);
    if (rc < 0) {
        fprintf(stderr, "cannot make socket nonblocking\n");
        goto fail;
    }

    conn = new_conn(ctx, argv[1]);
    if (conn == NULL) {
        fprintf(stderr, "cannot establish connection\n");
        goto fail;
    }

    /* TX */
    while (tx_len != 0) {
        l = tx(conn, tx_p, tx_len);
        if (l > 0) {
            tx_p += l;
            tx_len -= l;
        } else if (l == -1) {
            fprintf(stderr, "tx error\n");
        } else if (l == -2) {
            if (pump(conn, fd, get_conn_pending_tx(conn), timeout) != 1) {
                fprintf(stderr, "pump error\n");
                goto fail;
            }
        }
    }

    /* RX */
    for (;;) {
        l = rx(conn, rx_buf, sizeof(rx_buf));
        if (l > 0) {
            fwrite(rx_buf, 1, l, stdout);
        } else if (l == -1) {
            break;
        } else if (l == -2) {
            if (pump(conn, fd, get_conn_pending_rx(conn), timeout) != 1) {
                fprintf(stderr, "pump error\n");
                goto fail;
            }
        }
    }

    res = 0;
fail:
    if (conn != NULL)
        teardown(conn);
    if (ctx != NULL)
        teardown_ctx(ctx);
    if (result != NULL)
        freeaddrinfo(result);
    return res;
}
