#include <sys/poll.h>
#include <openssl/ssl.h>

/*
 * Demo 2: Client — Managed Connection — Nonblocking
 * ==============================================================
 *
 * This is an example of (part of) an application which uses libssl in an
 * asynchronous, nonblocking fashion. The functions show all interactions with
 * libssl the application makes, and would hypothetically be linked into a
 * larger application.
 *
 * In this example, libssl still makes syscalls directly using an fd, which is
 * configured in nonblocking mode. As such, the application can still be
 * abstracted from the details of what that fd is (is it a TCP socket? is it a
 * UDP socket?); this code passes the application an fd and the application
 * simply calls back into this code when poll()/etc. indicates it is ready.
 */
typedef struct app_conn_st {
    SSL *ssl;
    BIO *ssl_bio;
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
    ctx = SSL_CTX_new(OSSL_QUIC_client_thread_method());
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
 * hostname is a string like "openssl.org:443" or "[::1]:443".
 */
APP_CONN *new_conn(SSL_CTX *ctx, const char *hostname)
{
    APP_CONN *conn;
    BIO *out, *buf;
    SSL *ssl = NULL;
    const char *bare_hostname;
#ifdef USE_QUIC
    static const unsigned char alpn[] = {5, 'd', 'u', 'm', 'm', 'y'};
#endif

    conn = calloc(1, sizeof(APP_CONN));
    if (conn == NULL)
        return NULL;

    out = BIO_new_ssl_connect(ctx);
    if (out == NULL) {
        free(conn);
        return NULL;
    }

    if (BIO_get_ssl(out, &ssl) == 0) {
        BIO_free_all(out);
        free(conn);
        return NULL;
    }

    buf = BIO_new(BIO_f_buffer());
    if (buf == NULL) {
        BIO_free_all(out);
        free(conn);
        return NULL;
    }

    BIO_push(out, buf);

    if (BIO_set_conn_hostname(out, hostname) == 0) {
        BIO_free_all(out);
        free(conn);
        return NULL;
    }

    /* Returns the parsed hostname extracted from the hostname:port string. */
    bare_hostname = BIO_get_conn_hostname(out);
    if (bare_hostname == NULL) {
        BIO_free_all(out);
        free(conn);
        return NULL;
    }

    /* Tell the SSL object the hostname to check certificates against. */
    if (SSL_set1_host(ssl, bare_hostname) <= 0) {
        BIO_free_all(out);
        free(conn);
        return NULL;
    }

#ifdef USE_QUIC
    /* Configure ALPN, which is required for QUIC. */
    if (SSL_set_alpn_protos(ssl, alpn, sizeof(alpn))) {
        /* Note: SSL_set_alpn_protos returns 1 for failure. */
        BIO_free_all(out);
        free(conn);
        return NULL;
    }
#endif

    /* Make the BIO nonblocking. */
    BIO_set_nbio(out, 1);

    conn->ssl_bio = out;
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
    int l;

    conn->tx_need_rx = 0;

    l = BIO_write(conn->ssl_bio, buf, buf_len);
    if (l <= 0) {
        if (BIO_should_retry(conn->ssl_bio)) {
            conn->tx_need_rx = BIO_should_read(conn->ssl_bio);
            return -2;
        } else {
            return -1;
        }
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
    int l;

    conn->rx_need_tx = 0;

    l = BIO_read(conn->ssl_bio, buf, buf_len);
    if (l <= 0) {
        if (BIO_should_retry(conn->ssl_bio)) {
            conn->rx_need_tx = BIO_should_write(conn->ssl_bio);
            return -2;
        } else {
            return -1;
        }
    }

    return l;
}

/*
 * The application wants to know a fd it can poll on to determine when the
 * SSL state machine needs to be pumped.
 */
int get_conn_fd(APP_CONN *conn)
{
#ifdef USE_QUIC
    BIO_POLL_DESCRIPTOR d;

    if (!BIO_get_rpoll_descriptor(conn->ssl_bio, &d))
        return -1;

    return d.value.fd;
#else
    return BIO_get_fd(conn->ssl_bio, NULL);
#endif
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
int main(int argc, char **argv)
{
    static char tx_msg[384], host_port[300];
    const char *tx_p = tx_msg;
    char rx_buf[2048];
    int res = 1, l, tx_len;
    int timeout = 2000 /* ms */;
    APP_CONN *conn = NULL;
    SSL_CTX *ctx = NULL;

    if (argc < 3) {
        fprintf(stderr, "usage: %s host port\n", argv[0]);
        goto fail;
    }

    snprintf(host_port, sizeof(host_port), "%s:%s", argv[1], argv[2]);
    tx_len = snprintf(tx_msg, sizeof(tx_msg),
                      "GET / HTTP/1.0\r\nHost: %s\r\n\r\n", argv[1]);

    ctx = create_ssl_ctx();
    if (ctx == NULL) {
        fprintf(stderr, "cannot create SSL context\n");
        goto fail;
    }

    conn = new_conn(ctx, host_port);
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
            struct pollfd pfd = {0};
            pfd.fd = get_conn_fd(conn);
            pfd.events = get_conn_pending_tx(conn);
            if (poll(&pfd, 1, timeout) == 0) {
                fprintf(stderr, "tx timeout\n");
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
            struct pollfd pfd = {0};
            pfd.fd = get_conn_fd(conn);
            pfd.events = get_conn_pending_rx(conn);
            if (poll(&pfd, 1, timeout) == 0) {
                fprintf(stderr, "rx timeout\n");
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
    return res;
}
