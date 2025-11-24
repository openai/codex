#include <sys/poll.h>
#include <openssl/ssl.h>

/*
 * Demo 4: Client — Client Creates FD — Nonblocking
 * ================================================
 *
 * This is an example of (part of) an application which uses libssl in an
 * asynchronous, nonblocking fashion. The client is responsible for creating the
 * socket and passing it to libssl. The functions show all interactions with
 * libssl the application makes, and would hypothetically be linked into a
 * larger application.
 */
typedef struct app_conn_st {
    SSL *ssl;
    int fd;
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
APP_CONN *new_conn(SSL_CTX *ctx, int fd, const char *bare_hostname)
{
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

    if (SSL_set_fd(ssl, fd) <= 0) {
        SSL_free(ssl);
        free(conn);
        return NULL;
    }

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

#ifdef USE_QUIC
    /* Configure ALPN, which is required for QUIC. */
    if (SSL_set_alpn_protos(ssl, alpn, sizeof(alpn))) {
        /* Note: SSL_set_alpn_protos returns 1 for failure. */
        SSL_free(ssl);
        free(conn);
        return NULL;
    }
#endif

    conn->fd = fd;
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

    conn->tx_need_rx = 0;

    l = SSL_write(conn->ssl, buf, buf_len);
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

    conn->rx_need_tx = 0;

    l = SSL_read(conn->ssl, buf, buf_len);
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
    }

    return l;
}

/*
 * The application wants to know a fd it can poll on to determine when the
 * SSL state machine needs to be pumped.
 *
 * If the fd returned has:
 *
 *   POLLIN:    SSL_read *may* return data;
 *              if application does not want to read yet, it should call pump().
 *
 *   POLLOUT:   SSL_write *may* accept data
 *
 *   POLLERR:   An application should call pump() if it is not likely to call
 *              SSL_read or SSL_write soon.
 *
 */
int get_conn_fd(APP_CONN *conn)
{
    return conn->fd;
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
    return get_conn_pending_tx(conn);
}

#ifdef USE_QUIC
/*
 * Returns the number of milliseconds after which some call to libssl must be
 * made. Any call (SSL_read/SSL_write/SSL_pump) will do. Returns -1 if there is
 * no need for such a call. This may change after the next call
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
    SSL_shutdown(conn->ssl);
    SSL_free(conn->ssl);
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
#ifdef USE_QUIC
# include <sys/time.h>
#endif
#include <netdb.h>
#include <unistd.h>
#include <fcntl.h>

#ifdef USE_QUIC

static inline void ms_to_timeval(struct timeval *t, int ms)
{
    t->tv_sec   = ms < 0 ? -1 : ms/1000;
    t->tv_usec  = ms < 0 ? 0 : (ms%1000)*1000;
}

static inline int timeval_to_ms(const struct timeval *t)
{
    return t->tv_sec*1000 + t->tv_usec/1000;
}

#endif

int main(int argc, char **argv)
{
    int rc, fd = -1, res = 1;
    static char tx_msg[300];
    const char *tx_p = tx_msg;
    char rx_buf[2048];
    int l, tx_len;
#ifdef USE_QUIC
    struct timeval timeout;
#else
    int timeout = 2000 /* ms */;
#endif
    APP_CONN *conn = NULL;
    struct addrinfo hints = {0}, *result = NULL;
    SSL_CTX *ctx = NULL;

#ifdef USE_QUIC
    ms_to_timeval(&timeout, 2000);
#endif

    if (argc < 3) {
        fprintf(stderr, "usage: %s host port\n", argv[0]);
        goto fail;
    }

    tx_len = snprintf(tx_msg, sizeof(tx_msg),
                      "GET / HTTP/1.0\r\nHost: %s\r\n\r\n", argv[1]);

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

    conn = new_conn(ctx, fd, argv[1]);
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
            goto fail;
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
    if (result != NULL)
        freeaddrinfo(result);
    return res;
}
