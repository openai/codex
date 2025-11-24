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

    /*
     * NOTE: QUIC cannot operate with a buffering BIO between the QUIC SSL
     * object in the network. In this case, the call to BIO_push() is not
     * supported by the QUIC SSL object and will be ignored, thus this code
     * works without removing this line. However, the buffering BIO is not
     * actually used as a result and should be removed when adapting code to use
     * QUIC.
     *
     * Setting a buffer as the underlying BIO on the QUIC SSL object using
     * SSL_set_bio() will not work, though BIO_s_dgram_pair is available for
     * buffering the input and output to the QUIC SSL object on the network side
     * if desired.
     */
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

#ifdef USE_QUIC
/*
 * Returns the number of milliseconds after which some call to libssl must be
 * made. Any call (BIO_read/BIO_write/BIO_pump) will do. Returns -1 if
 * there is no need for such a call. This may change after the next call
 * to libssl.
 */
static inline int timeval_to_ms(const struct timeval *t);

int get_conn_pump_timeout(APP_CONN *conn)
{
    struct timeval tv;
    int is_infinite;

    if (!SSL_get_event_timeout(conn->ssl, &tv, &is_infinite))
        return -1;

    return is_infinite ? -1 : timeval_to_ms(&tv);
}

/*
 * Called to advance internals of libssl state machines without having to
 * perform an application-level read/write.
 */
void pump(APP_CONN *conn)
{
    SSL_handle_events(conn->ssl);
}
#endif

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
#include <sys/time.h>

static inline void ms_to_timeval(struct timeval *t, int ms)
{
    t->tv_sec   = ms < 0 ? -1 : ms/1000;
    t->tv_usec  = ms < 0 ? 0 : (ms%1000)*1000;
}

static inline int timeval_to_ms(const struct timeval *t)
{
    return t->tv_sec*1000 + t->tv_usec/1000;
}

int main(int argc, char **argv)
{
    static char tx_msg[384], host_port[300];
    const char *tx_p = tx_msg;
    char rx_buf[2048];
    int res = 1, l, tx_len;
#ifdef USE_QUIC
    struct timeval timeout;
#else
    int timeout = 2000 /* ms */;
#endif
    APP_CONN *conn = NULL;
    SSL_CTX *ctx = NULL;

#ifdef USE_QUIC
    ms_to_timeval(&timeout, 2000);
#endif

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
#ifdef USE_QUIC
            struct timeval start, now, deadline, t;
#endif
            struct pollfd pfd = {0};

#ifdef USE_QUIC
            ms_to_timeval(&t, get_conn_pump_timeout(conn));
            if (t.tv_sec < 0 || timercmp(&t, &timeout, >))
                t = timeout;

            gettimeofday(&start, NULL);
            timeradd(&start, &timeout, &deadline);
#endif

            pfd.fd = get_conn_fd(conn);
            pfd.events = get_conn_pending_tx(conn);
#ifdef USE_QUIC
            if (poll(&pfd, 1, timeval_to_ms(&t)) == 0)
#else
            if (poll(&pfd, 1, timeout) == 0)
#endif
            {
#ifdef USE_QUIC
                pump(conn);

                gettimeofday(&now, NULL);
                if (timercmp(&now, &deadline, >=))
#endif
                {
                    fprintf(stderr, "tx timeout\n");
                    goto fail;
                }
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
#ifdef USE_QUIC
            struct timeval start, now, deadline, t;
#endif
            struct pollfd pfd = {0};

#ifdef USE_QUIC
            ms_to_timeval(&t, get_conn_pump_timeout(conn));
            if (t.tv_sec < 0 || timercmp(&t, &timeout, >))
                t = timeout;

            gettimeofday(&start, NULL);
            timeradd(&start, &timeout, &deadline);
#endif

            pfd.fd = get_conn_fd(conn);
            pfd.events = get_conn_pending_rx(conn);
#ifdef USE_QUIC
            if (poll(&pfd, 1, timeval_to_ms(&t)) == 0)
#else
            if (poll(&pfd, 1, timeout) == 0)
#endif
            {
#ifdef USE_QUIC
                pump(conn);

                gettimeofday(&now, NULL);
                if (timercmp(&now, &deadline, >=))
#endif
                {
                    fprintf(stderr, "rx timeout\n");
                    goto fail;
                }
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
