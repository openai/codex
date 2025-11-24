#include <sys/poll.h>
#include <openssl/ssl.h>
#include <uv.h>
#include <assert.h>
#ifdef USE_QUIC
# include <sys/time.h>
#endif

typedef struct app_conn_st APP_CONN;
typedef struct upper_write_op_st UPPER_WRITE_OP;
typedef struct lower_write_op_st LOWER_WRITE_OP;

typedef void (app_connect_cb)(APP_CONN *conn, int status, void *arg);
typedef void (app_write_cb)(APP_CONN *conn, int status, void *arg);
typedef void (app_read_cb)(APP_CONN *conn, void *buf, size_t buf_len, void *arg);

#ifdef USE_QUIC
static void set_timer(APP_CONN *conn);
#else
static void tcp_connect_done(uv_connect_t *tcp_connect, int status);
#endif
static void net_connect_fail_close_done(uv_handle_t *handle);
static int handshake_ssl(APP_CONN *conn);
static void flush_write_buf(APP_CONN *conn);
static void set_rx(APP_CONN *conn);
static int try_write(APP_CONN *conn, UPPER_WRITE_OP *op);
static void handle_pending_writes(APP_CONN *conn);
static int write_deferred(APP_CONN *conn, const void *buf, size_t buf_len, app_write_cb *cb, void *arg);
static void teardown_continued(uv_handle_t *handle);
static int setup_ssl(APP_CONN *conn, const char *hostname);

#ifdef USE_QUIC
static inline int timeval_to_ms(const struct timeval *t)
{
    return t->tv_sec*1000 + t->tv_usec/1000;
}
#endif

/*
 * Structure to track an application-level write request. Only created
 * if SSL_write does not accept the data immediately, typically because
 * it is in WANT_READ.
 */
struct upper_write_op_st {
    struct upper_write_op_st   *prev, *next;
    const uint8_t              *buf;
    size_t                      buf_len, written;
    APP_CONN                   *conn;
    app_write_cb               *cb;
    void                       *cb_arg;
};

/*
 * Structure to track a network-level write request.
 */
struct lower_write_op_st {
#ifdef USE_QUIC
    uv_udp_send_t   w;
#else
    uv_write_t      w;
#endif
    uv_buf_t        b;
    uint8_t        *buf;
    APP_CONN       *conn;
};

/*
 * Application connection object.
 */
struct app_conn_st {
    SSL_CTX        *ctx;
    SSL            *ssl;
    BIO            *net_bio;
#ifdef USE_QUIC
    uv_udp_t        udp;
    uv_timer_t      timer;
#else
    uv_stream_t    *stream;
    uv_tcp_t        tcp;
    uv_connect_t    tcp_connect;
#endif
    app_connect_cb *app_connect_cb;   /* called once handshake is done */
    void           *app_connect_arg;
    app_read_cb    *app_read_cb;      /* application's on-RX callback */
    void           *app_read_arg;
    const char     *hostname;
    char            init_handshake, done_handshake, closed;
    char           *teardown_done;

    UPPER_WRITE_OP *pending_upper_write_head, *pending_upper_write_tail;
};

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
 * SSL_CTX. An outgoing TCP connection is started and the callback is called
 * asynchronously when the TLS handshake is complete.
 *
 * hostname is a string like "openssl.org" used for certificate validation.
 */

APP_CONN *new_conn(SSL_CTX *ctx, const char *hostname,
                   struct sockaddr *sa, socklen_t sa_len,
                   app_connect_cb *cb, void *arg)
{
    int rc;
    APP_CONN *conn = NULL;

    conn = calloc(1, sizeof(APP_CONN));
    if (!conn)
        return NULL;

#ifdef USE_QUIC
    uv_udp_init(uv_default_loop(), &conn->udp);
    conn->udp.data = conn;

    uv_timer_init(uv_default_loop(), &conn->timer);
    conn->timer.data = conn;
#else
    uv_tcp_init(uv_default_loop(), &conn->tcp);
    conn->tcp.data = conn;

    conn->stream            = (uv_stream_t *)&conn->tcp;
#endif

    conn->app_connect_cb    = cb;
    conn->app_connect_arg   = arg;
#ifdef USE_QUIC
    rc = uv_udp_connect(&conn->udp, sa);
#else
    conn->tcp_connect.data  = conn;
    rc = uv_tcp_connect(&conn->tcp_connect, &conn->tcp, sa, tcp_connect_done);
#endif
    if (rc < 0) {
#ifdef USE_QUIC
        uv_close((uv_handle_t *)&conn->udp, net_connect_fail_close_done);
#else
        uv_close((uv_handle_t *)&conn->tcp, net_connect_fail_close_done);
#endif
        return NULL;
    }

    conn->ctx       = ctx;
    conn->hostname  = hostname;

#ifdef USE_QUIC
    rc = setup_ssl(conn, hostname);
    if (rc < 0) {
        uv_close((uv_handle_t *)&conn->udp, net_connect_fail_close_done);
        return NULL;
    }
#endif

    return conn;
}

/*
 * The application wants to start reading from the SSL stream.
 * The callback is called whenever data is available.
 */
int app_read_start(APP_CONN *conn, app_read_cb *cb, void *arg)
{
    conn->app_read_cb  = cb;
    conn->app_read_arg = arg;
    set_rx(conn);
    return 0;
}

/*
 * The application wants to write. The callback is called once the
 * write is complete. The callback should free the buffer.
 */
int app_write(APP_CONN *conn, const void *buf, size_t buf_len, app_write_cb *cb, void *arg)
{
    write_deferred(conn, buf, buf_len, cb, arg);
    handle_pending_writes(conn);
    return buf_len;
}

/*
 * The application wants to close the connection and free bookkeeping
 * structures.
 */
void teardown(APP_CONN *conn)
{
    char teardown_done = 0;

    if (conn == NULL)
        return;

    BIO_free_all(conn->net_bio);
    SSL_free(conn->ssl);

#ifndef USE_QUIC
    uv_cancel((uv_req_t *)&conn->tcp_connect);
#endif

    conn->teardown_done = &teardown_done;
#ifdef USE_QUIC
    uv_close((uv_handle_t *)&conn->udp, teardown_continued);
    uv_close((uv_handle_t *)&conn->timer, teardown_continued);
#else
    uv_close((uv_handle_t *)conn->stream, teardown_continued);
#endif

    /* Just wait synchronously until teardown completes. */
#ifdef USE_QUIC
    while (teardown_done < 2)
#else
    while (!teardown_done)
#endif
        uv_run(uv_default_loop(), UV_RUN_DEFAULT);
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
 * Internal implementation functions.
 */
static void enqueue_upper_write_op(APP_CONN *conn, UPPER_WRITE_OP *op)
{
    op->prev = conn->pending_upper_write_tail;
    if (op->prev)
        op->prev->next = op;

    conn->pending_upper_write_tail = op;
    if (conn->pending_upper_write_head == NULL)
        conn->pending_upper_write_head = op;
}

static void dequeue_upper_write_op(APP_CONN *conn)
{
    if (conn->pending_upper_write_head == NULL)
        return;

    if (conn->pending_upper_write_head->next == NULL) {
        conn->pending_upper_write_head = NULL;
        conn->pending_upper_write_tail = NULL;
    } else {
        conn->pending_upper_write_head = conn->pending_upper_write_head->next;
        conn->pending_upper_write_head->prev = NULL;
    }
}

static void net_read_alloc(uv_handle_t *handle,
                           size_t suggested_size, uv_buf_t *buf)
{
#ifdef USE_QUIC
    if (suggested_size < 1472)
        suggested_size = 1472;
#endif

    buf->base = malloc(suggested_size);
    buf->len  = suggested_size;
}

static void on_rx_push(APP_CONN *conn)
{
    int srd, rc;
    int buf_len = 4096;

    do {
        if (!conn->app_read_cb)
            return;

        void *buf = malloc(buf_len);
        if (!buf)
            return;

        srd = SSL_read(conn->ssl, buf, buf_len);
        flush_write_buf(conn);
        if (srd <= 0) {
            rc = SSL_get_error(conn->ssl, srd);
            if (rc == SSL_ERROR_WANT_READ) {
                free(buf);
                return;
            }
        }

        conn->app_read_cb(conn, buf, srd, conn->app_read_arg);
    } while (srd == buf_len);
}

static void net_error(APP_CONN *conn)
{
    conn->closed = 1;
    set_rx(conn);

    if (conn->app_read_cb)
        conn->app_read_cb(conn, NULL, 0, conn->app_read_arg);
}

static void handle_pending_writes(APP_CONN *conn)
{
    int rc;

    if (conn->pending_upper_write_head == NULL)
        return;

    do {
        UPPER_WRITE_OP *op = conn->pending_upper_write_head;
        rc = try_write(conn, op);
        if (rc <= 0)
            break;

        dequeue_upper_write_op(conn);
        free(op);
    } while (conn->pending_upper_write_head != NULL);

    set_rx(conn);
}

#ifdef USE_QUIC
static void net_read_done(uv_udp_t *stream, ssize_t nr, const uv_buf_t *buf,
                          const struct sockaddr *addr, unsigned int flags)
#else
static void net_read_done(uv_stream_t *stream, ssize_t nr, const uv_buf_t *buf)
#endif
{
    int rc;
    APP_CONN *conn = (APP_CONN *)stream->data;

    if (nr < 0) {
        free(buf->base);
        net_error(conn);
        return;
    }

    if (nr > 0) {
        int wr = BIO_write(conn->net_bio, buf->base, nr);
        assert(wr == nr);
    }

    free(buf->base);

    if (!conn->done_handshake) {
        rc = handshake_ssl(conn);
        if (rc < 0) {
            fprintf(stderr, "handshake error: %d\n", rc);
            return;
        }

        if (!conn->done_handshake)
            return;
    }

    handle_pending_writes(conn);
    on_rx_push(conn);
}

static void set_rx(APP_CONN *conn)
{
#ifdef USE_QUIC
    if (!conn->closed)
        uv_udp_recv_start(&conn->udp, net_read_alloc, net_read_done);
    else
        uv_udp_recv_stop(&conn->udp);
#else
    if (!conn->closed && (conn->app_read_cb || (!conn->done_handshake && conn->init_handshake) || conn->pending_upper_write_head != NULL))
        uv_read_start(conn->stream, net_read_alloc, net_read_done);
    else
        uv_read_stop(conn->stream);
#endif
}

#ifdef USE_QUIC
static void net_write_done(uv_udp_send_t *req, int status)
#else
static void net_write_done(uv_write_t *req, int status)
#endif
{
    LOWER_WRITE_OP *op = (LOWER_WRITE_OP *)req->data;
    APP_CONN *conn = op->conn;

    if (status < 0) {
        fprintf(stderr, "UV write failed %d\n", status);
        return;
    }

    free(op->buf);
    free(op);

    flush_write_buf(conn);
}

static void flush_write_buf(APP_CONN *conn)
{
    int rc, rd;
    LOWER_WRITE_OP *op;
    uint8_t *buf;

    buf = malloc(4096);
    if (!buf)
        return;

    rd = BIO_read(conn->net_bio, buf, 4096);
    if (rd <= 0) {
        free(buf);
        return;
    }

    op = calloc(1, sizeof(LOWER_WRITE_OP));
    if (!op)
        return;

    op->buf     = buf;
    op->conn    = conn;
    op->w.data  = op;
    op->b.base  = (char *)buf;
    op->b.len   = rd;

#ifdef USE_QUIC
    rc = uv_udp_send(&op->w, &conn->udp, &op->b, 1, NULL, net_write_done);
#else
    rc = uv_write(&op->w, conn->stream, &op->b, 1, net_write_done);
#endif
    if (rc < 0) {
        free(buf);
        free(op);
        fprintf(stderr, "UV write failed\n");
        return;
    }
}

static void handshake_done_ssl(APP_CONN *conn)
{
#ifdef USE_QUIC
    set_timer(conn);
#endif
    conn->app_connect_cb(conn, 0, conn->app_connect_arg);
}

static int handshake_ssl(APP_CONN *conn)
{
    int rc, rcx;

    conn->init_handshake = 1;

    rc = SSL_do_handshake(conn->ssl);
    if (rc > 0) {
        conn->done_handshake = 1;
        handshake_done_ssl(conn);
        set_rx(conn);
        return 0;
    }

    flush_write_buf(conn);
    rcx = SSL_get_error(conn->ssl, rc);
    if (rcx == SSL_ERROR_WANT_READ) {
        set_rx(conn);
        return 0;
    }

    fprintf(stderr, "Handshake error: %d\n", rcx);
    return -rcx;
}

static int setup_ssl(APP_CONN *conn, const char *hostname)
{
    BIO *internal_bio = NULL, *net_bio = NULL;
    SSL *ssl = NULL;
#ifdef USE_QUIC
    static const unsigned char alpn[] = {5, 'd', 'u', 'm', 'm', 'y'};
#endif

    ssl = SSL_new(conn->ctx);
    if (!ssl)
        return -1;

    SSL_set_connect_state(ssl);

#ifdef USE_QUIC
    if (BIO_new_bio_dgram_pair(&internal_bio, 0, &net_bio, 0) <= 0) {
        SSL_free(ssl);
        return -1;
    }
#else
    if (BIO_new_bio_pair(&internal_bio, 0, &net_bio, 0) <= 0) {
        SSL_free(ssl);
        return -1;
    }
#endif

    SSL_set_bio(ssl, internal_bio, internal_bio);

    if (SSL_set1_host(ssl, hostname) <= 0) {
        SSL_free(ssl);
        return -1;
    }

    if (SSL_set_tlsext_host_name(ssl, hostname) <= 0) {
        SSL_free(ssl);
        return -1;
    }

#ifdef USE_QUIC
    /* Configure ALPN, which is required for QUIC. */
    if (SSL_set_alpn_protos(ssl, alpn, sizeof(alpn))) {
        /* Note: SSL_set_alpn_protos returns 1 for failure. */
        SSL_free(ssl);
        return -1;
    }
#endif

    conn->net_bio             = net_bio;
    conn->ssl                 = ssl;
    return handshake_ssl(conn);
}

#ifndef USE_QUIC
static void tcp_connect_done(uv_connect_t *tcp_connect, int status)
{
    int rc;
    APP_CONN *conn = (APP_CONN *)tcp_connect->data;

    if (status < 0) {
        uv_stop(uv_default_loop());
        return;
    }

    rc = setup_ssl(conn, conn->hostname);
    if (rc < 0) {
        fprintf(stderr, "cannot init SSL\n");
        uv_stop(uv_default_loop());
        return;
    }
}
#endif

static void net_connect_fail_close_done(uv_handle_t *handle)
{
    APP_CONN *conn = (APP_CONN *)handle->data;

    free(conn);
}

#ifdef USE_QUIC

static void timer_done(uv_timer_t *timer)
{
    APP_CONN *conn = (APP_CONN *)timer->data;

    SSL_handle_events(conn->ssl);
    handle_pending_writes(conn);
    flush_write_buf(conn);
    set_rx(conn);
    set_timer(conn); /* repeat timer */
}

static void set_timer(APP_CONN *conn)
{
    struct timeval tv;
    int ms, is_infinite;

    if (!SSL_get_event_timeout(conn->ssl, &tv, &is_infinite))
        return;

    ms = is_infinite ? -1 : timeval_to_ms(&tv);
    if (ms > 0)
        uv_timer_start(&conn->timer, timer_done, ms, 0);
}

#endif

static int try_write(APP_CONN *conn, UPPER_WRITE_OP *op)
{
    int rc, rcx;
    size_t written = op->written;

    while (written < op->buf_len) {
        rc = SSL_write(conn->ssl, op->buf + written, op->buf_len - written);
        if (rc <= 0) {
            rcx = SSL_get_error(conn->ssl, rc);
            if (rcx == SSL_ERROR_WANT_READ) {
                op->written = written;
                return 0;
            } else {
                if (op->cb != NULL)
                    op->cb(conn, -rcx, op->cb_arg);
                return 1; /* op should be freed */
            }
        }

        written += rc;
    }

    if (op->cb != NULL)
        op->cb(conn, 0, op->cb_arg);

    flush_write_buf(conn);
    return 1; /* op should be freed */
}

static int write_deferred(APP_CONN *conn, const void *buf, size_t buf_len, app_write_cb *cb, void *arg)
{
    UPPER_WRITE_OP *op = calloc(1, sizeof(UPPER_WRITE_OP));
    if (!op)
        return -1;

    op->buf     = buf;
    op->buf_len = buf_len;
    op->conn    = conn;
    op->cb      = cb;
    op->cb_arg  = arg;

    enqueue_upper_write_op(conn, op);
    set_rx(conn);
    flush_write_buf(conn);
    return buf_len;
}

static void teardown_continued(uv_handle_t *handle)
{
    APP_CONN *conn = (APP_CONN *)handle->data;
    UPPER_WRITE_OP *op, *next_op;
    char *teardown_done = conn->teardown_done;

#ifdef USE_QUIC
    if (++*teardown_done < 2)
        return;
#endif

    for (op=conn->pending_upper_write_head; op; op=next_op) {
        next_op = op->next;
        free(op);
    }

    free(conn);
#ifndef USE_QUIC
    *teardown_done = 1;
#endif
}

/*
 * ============================================================================
 * Example driver for the above code. This is just to demonstrate that the code
 * works and is not intended to be representative of a real application.
 */
static void post_read(APP_CONN *conn, void *buf, size_t buf_len, void *arg)
{
    if (!buf_len) {
        free(buf);
        uv_stop(uv_default_loop());
        return;
    }

    fwrite(buf, 1, buf_len, stdout);
    free(buf);
}

static void post_write_get(APP_CONN *conn, int status, void *arg)
{
    if (status < 0) {
        fprintf(stderr, "write failed: %d\n", status);
        return;
    }

    app_read_start(conn, post_read, NULL);
}

char tx_msg[300];
int mlen;

static void post_connect(APP_CONN *conn, int status, void *arg)
{
    int wr;

    if (status < 0) {
        fprintf(stderr, "failed to connect: %d\n", status);
        uv_stop(uv_default_loop());
        return;
    }

    wr = app_write(conn, tx_msg, mlen, post_write_get, NULL);
    if (wr < mlen) {
        fprintf(stderr, "error writing request");
        return;
    }
}

int main(int argc, char **argv)
{
    int rc = 1;
    SSL_CTX *ctx = NULL;
    APP_CONN *conn = NULL;
    struct addrinfo hints = {0}, *result = NULL;

    if (argc < 3) {
        fprintf(stderr, "usage: %s host port\n", argv[0]);
        goto fail;
    }

    mlen = snprintf(tx_msg, sizeof(tx_msg),
                    "GET / HTTP/1.0\r\nHost: %s\r\n\r\n", argv[1]);

    ctx = create_ssl_ctx();
    if (!ctx)
        goto fail;

    hints.ai_family     = AF_INET;
    hints.ai_socktype   = SOCK_STREAM;
    hints.ai_flags      = AI_PASSIVE;
    rc = getaddrinfo(argv[1], argv[2], &hints, &result);
    if (rc < 0) {
        fprintf(stderr, "cannot resolve\n");
        goto fail;
    }

    conn = new_conn(ctx, argv[1], result->ai_addr, result->ai_addrlen, post_connect, NULL);
    if (!conn)
        goto fail;

    uv_run(uv_default_loop(), UV_RUN_DEFAULT);

    rc = 0;
fail:
    teardown(conn);
    freeaddrinfo(result);
    uv_loop_close(uv_default_loop());
    teardown_ctx(ctx);
}
