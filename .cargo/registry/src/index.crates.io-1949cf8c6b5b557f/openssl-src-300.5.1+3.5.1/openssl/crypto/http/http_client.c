/*
 * Copyright 2001-2025 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright Siemens AG 2018-2020
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/e_os.h"
#include <stdio.h>
#include <stdlib.h>
#include "crypto/ctype.h"
#include <string.h>
#include <openssl/asn1.h>
#include <openssl/evp.h>
#include <openssl/err.h>
#include <openssl/httperr.h>
#include <openssl/cmperr.h>
#include <openssl/buffer.h>
#include <openssl/http.h>
#include <openssl/trace.h>
#include "internal/sockets.h"
#include "internal/common.h" /* for ossl_assert() */

#define HTTP_PREFIX "HTTP/"
#define HTTP_VERSION_PATT "1." /* allow 1.x */
#define HTTP_VERSION_STR_LEN sizeof(HTTP_VERSION_PATT) /* == strlen("1.0") */
#define HTTP_PREFIX_VERSION HTTP_PREFIX""HTTP_VERSION_PATT
#define HTTP_1_0 HTTP_PREFIX_VERSION"0" /* "HTTP/1.0" */
#define HTTP_LINE1_MINLEN (sizeof(HTTP_PREFIX_VERSION "x 200\n") - 1)
#define HTTP_VERSION_MAX_REDIRECTIONS 50

#define HTTP_STATUS_CODE_OK                200
#define HTTP_STATUS_CODE_MOVED_PERMANENTLY 301
#define HTTP_STATUS_CODE_FOUND             302
#define HTTP_STATUS_CODES_NONFATAL_ERROR   400

/* Stateful HTTP request code, supporting blocking and non-blocking I/O */

/* Opaque HTTP request status structure */

struct ossl_http_req_ctx_st {
    int state;                  /* Current I/O state */
    unsigned char *buf;         /* Buffer to write request or read response */
    int buf_size;               /* Buffer size */
    int free_wbio;              /* wbio allocated internally, free with ctx */
    BIO *wbio;                  /* BIO to write/send request to */
    BIO *rbio;                  /* BIO to read/receive response from */
    OSSL_HTTP_bio_cb_t upd_fn;  /* Optional BIO update callback used for TLS */
    void *upd_arg;              /* Optional arg for update callback function */
    int use_ssl;                /* Use HTTPS */
    char *proxy;                /* Optional proxy name or URI */
    char *server;               /* Optional server hostname */
    char *port;                 /* Optional server port */
    BIO *mem;                   /* Mem BIO holding request header or response */
    BIO *req;                   /* BIO holding the request provided by caller */
    int method_POST;            /* HTTP method is POST (else GET) */
    int text;                   /* Request content type is (likely) text */
    char *expected_ct;          /* Optional expected Content-Type */
    int expect_asn1;            /* Response content must be ASN.1-encoded */
    unsigned char *pos;         /* Current position sending data */
    long len_to_send;           /* Number of bytes still to send */
    size_t resp_len;            /* Length of response */
    size_t max_resp_len;        /* Maximum length of response, or 0 */
    int keep_alive;             /* Persistent conn. 0=no, 1=prefer, 2=require */
    time_t max_time;            /* Maximum end time of current transfer, or 0 */
    time_t max_total_time;      /* Maximum end time of total transfer, or 0 */
    char *redirection_url;      /* Location obtained from HTTP status 301/302 */
    size_t max_hdr_lines;       /* Max. number of response header lines, or 0 */
};

/* HTTP client OSSL_HTTP_REQ_CTX_nbio() internal states, in typical order */

#define OHS_NOREAD         0x1000 /* If set no reading should be performed */
#define OHS_ERROR          (0 | OHS_NOREAD) /* Error condition */
#define OHS_ADD_HEADERS    (1 | OHS_NOREAD) /* Adding header lines to request */
#define OHS_WRITE_INIT     (2 | OHS_NOREAD) /* 1st call: ready to start send */
#define OHS_WRITE_HDR1     (3 | OHS_NOREAD) /* Request header to be sent */
#define OHS_WRITE_HDR      (4 | OHS_NOREAD) /* Request header being sent */
#define OHS_WRITE_REQ      (5 | OHS_NOREAD) /* Request content (body) being sent */
#define OHS_FLUSH          (6 | OHS_NOREAD) /* Request being flushed */

#define OHS_FIRSTLINE       1 /* First line of response being read */
#define OHS_HEADERS         2 /* MIME headers of response being read */
#define OHS_HEADERS_ERROR   3 /* MIME headers of response being read after fatal error */
#define OHS_REDIRECT        4 /* MIME headers being read, expecting Location */
#define OHS_ASN1_HEADER     5 /* ASN1 sequence header (tag+length) being read */
#define OHS_ASN1_CONTENT    6 /* ASN1 content octets being read */
#define OHS_ASN1_DONE       7 /* ASN1 content read completed */
#define OHS_STREAM          8 /* HTTP content stream to be read by caller */
#define OHS_ERROR_CONTENT   9 /* response content (body) being read after fatal error */

/* Low-level HTTP API implementation */

OSSL_HTTP_REQ_CTX *OSSL_HTTP_REQ_CTX_new(BIO *wbio, BIO *rbio, int buf_size)
{
    OSSL_HTTP_REQ_CTX *rctx;

    if (wbio == NULL || rbio == NULL) {
        ERR_raise(ERR_LIB_HTTP, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }

    if ((rctx = OPENSSL_zalloc(sizeof(*rctx))) == NULL)
        return NULL;
    rctx->state = OHS_ERROR;
    rctx->buf_size = buf_size > 0 ? buf_size : OSSL_HTTP_DEFAULT_MAX_LINE_LEN;
    rctx->buf = OPENSSL_malloc(rctx->buf_size);
    rctx->wbio = wbio;
    rctx->rbio = rbio;
    rctx->max_hdr_lines = OSSL_HTTP_DEFAULT_MAX_RESP_HDR_LINES;
    if (rctx->buf == NULL) {
        OPENSSL_free(rctx);
        return NULL;
    }
    rctx->max_resp_len = OSSL_HTTP_DEFAULT_MAX_RESP_LEN;
    /* everything else is 0, e.g. rctx->len_to_send, or NULL, e.g. rctx->mem  */
    return rctx;
}

void OSSL_HTTP_REQ_CTX_free(OSSL_HTTP_REQ_CTX *rctx)
{
    if (rctx == NULL)
        return;
    /*
     * Use BIO_free_all() because bio_update_fn may prepend or append to cbio.
     * This also frees any (e.g., SSL/TLS) BIOs linked with bio and,
     * like BIO_reset(bio), calls SSL_shutdown() to notify/alert the peer.
     */
    if (rctx->free_wbio)
        BIO_free_all(rctx->wbio);
    /* do not free rctx->rbio */
    BIO_free(rctx->mem);
    BIO_free(rctx->req);
    OPENSSL_free(rctx->buf);
    OPENSSL_free(rctx->proxy);
    OPENSSL_free(rctx->server);
    OPENSSL_free(rctx->port);
    OPENSSL_free(rctx->expected_ct);
    OPENSSL_free(rctx);
}

BIO *OSSL_HTTP_REQ_CTX_get0_mem_bio(const OSSL_HTTP_REQ_CTX *rctx)
{
    if (rctx == NULL) {
        ERR_raise(ERR_LIB_HTTP, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }
    return rctx->mem;
}

size_t OSSL_HTTP_REQ_CTX_get_resp_len(const OSSL_HTTP_REQ_CTX *rctx)
{
    if (rctx == NULL) {
        ERR_raise(ERR_LIB_HTTP, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    return rctx->resp_len;
}

void OSSL_HTTP_REQ_CTX_set_max_response_length(OSSL_HTTP_REQ_CTX *rctx,
                                               unsigned long len)
{
    if (rctx == NULL) {
        ERR_raise(ERR_LIB_HTTP, ERR_R_PASSED_NULL_PARAMETER);
        return;
    }
    rctx->max_resp_len = len != 0 ? (size_t)len : OSSL_HTTP_DEFAULT_MAX_RESP_LEN;
}

/*
 * Create request line using |rctx| and |path| (or "/" in case |path| is NULL).
 * Server name (and optional port) must be given if and only if
 * a plain HTTP proxy is used and |path| does not begin with 'http://'.
 */
int OSSL_HTTP_REQ_CTX_set_request_line(OSSL_HTTP_REQ_CTX *rctx, int method_POST,
                                       const char *server, const char *port,
                                       const char *path)
{
    if (rctx == NULL) {
        ERR_raise(ERR_LIB_HTTP, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    BIO_free(rctx->mem);
    if ((rctx->mem = BIO_new(BIO_s_mem())) == NULL)
        return 0;

    rctx->method_POST = method_POST != 0;
    if (BIO_printf(rctx->mem, "%s ", rctx->method_POST ? "POST" : "GET") <= 0)
        return 0;

    if (server != NULL) { /* HTTP (but not HTTPS) proxy is used */
        /*
         * Section 5.1.2 of RFC 1945 states that the absoluteURI form is only
         * allowed when using a proxy
         */
        if (BIO_printf(rctx->mem, OSSL_HTTP_PREFIX"%s", server) <= 0)
            return 0;
        if (port != NULL && BIO_printf(rctx->mem, ":%s", port) <= 0)
            return 0;
    }

    /* Make sure path includes a forward slash (abs_path) */
    if (path == NULL)  {
        path = "/";
    } else if (HAS_PREFIX(path, "http://")) { /* absoluteURI for proxy use */
        if (server != NULL) {
            ERR_raise(ERR_LIB_HTTP, ERR_R_PASSED_INVALID_ARGUMENT);
            return 0;
        }
    } else if (path[0] != '/' && BIO_printf(rctx->mem, "/") <= 0) {
        return 0;
    }
    /*
     * Add (the rest of) the path and the HTTP version,
     * which is fixed to 1.0 for straightforward implementation of keep-alive
     */
    if (BIO_printf(rctx->mem, "%s "HTTP_1_0"\r\n", path) <= 0)
        return 0;

    rctx->resp_len = 0;
    rctx->state = OHS_ADD_HEADERS;
    return 1;
}

int OSSL_HTTP_REQ_CTX_add1_header(OSSL_HTTP_REQ_CTX *rctx,
                                  const char *name, const char *value)
{
    if (rctx == NULL || name == NULL) {
        ERR_raise(ERR_LIB_HTTP, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (rctx->mem == NULL) {
        ERR_raise(ERR_LIB_HTTP, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }

    if (BIO_puts(rctx->mem, name) <= 0)
        return 0;
    if (value != NULL) {
        if (BIO_write(rctx->mem, ": ", 2) != 2)
            return 0;
        if (BIO_puts(rctx->mem, value) <= 0)
            return 0;
    }
    return BIO_write(rctx->mem, "\r\n", 2) == 2;
}

int OSSL_HTTP_REQ_CTX_set_expected(OSSL_HTTP_REQ_CTX *rctx,
                                   const char *content_type, int asn1,
                                   int timeout, int keep_alive)
{
    if (rctx == NULL) {
        ERR_raise(ERR_LIB_HTTP, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (keep_alive != 0
            && rctx->state != OHS_ERROR && rctx->state != OHS_ADD_HEADERS) {
        /* Cannot anymore set keep-alive in request header */
        ERR_raise(ERR_LIB_HTTP, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }

    OPENSSL_free(rctx->expected_ct);
    rctx->expected_ct = NULL;
    if (content_type != NULL
            && (rctx->expected_ct = OPENSSL_strdup(content_type)) == NULL)
        return 0;

    rctx->expect_asn1 = asn1;
    if (timeout >= 0)
        rctx->max_time = timeout > 0 ? time(NULL) + timeout : 0;
    else /* take over any |overall_timeout| arg of OSSL_HTTP_open(), else 0 */
        rctx->max_time = rctx->max_total_time;
    rctx->keep_alive = keep_alive;
    return 1;
}

static int set1_content(OSSL_HTTP_REQ_CTX *rctx,
                        const char *content_type, BIO *req)
{
    long req_len = 0;
#ifndef OPENSSL_NO_STDIO
    FILE *fp = NULL;
#endif

    if (rctx == NULL || (req == NULL && content_type != NULL)) {
        ERR_raise(ERR_LIB_HTTP, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if (rctx->keep_alive != 0
            && !OSSL_HTTP_REQ_CTX_add1_header(rctx, "Connection", "keep-alive"))
        return 0;

    BIO_free(rctx->req);
    rctx->req = NULL;
    if (req == NULL)
        return 1;
    if (!rctx->method_POST) {
        ERR_raise(ERR_LIB_HTTP, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }

    if (content_type == NULL) {
        rctx->text = 1; /* assuming request to be text by default, used just for tracing */
    } else {
        if (HAS_CASE_PREFIX(content_type, "text/"))
            rctx->text = 1;
        if (BIO_printf(rctx->mem, "Content-Type: %s\r\n", content_type) <= 0)
            return 0;
    }

    /*
     * BIO_CTRL_INFO yields the data length at least for memory BIOs, but for
     * file-based BIOs it gives the current position, which is not what we need.
     */
    if (BIO_method_type(req) == BIO_TYPE_FILE) {
#ifndef OPENSSL_NO_STDIO
        if (BIO_get_fp(req, &fp) == 1 && fseek(fp, 0, SEEK_END) == 0) {
            req_len = ftell(fp);
            (void)fseek(fp, 0, SEEK_SET);
        } else {
            fp = NULL;
        }
#endif
    } else {
        req_len = BIO_ctrl(req, BIO_CTRL_INFO, 0, NULL);
        /*
         * Streaming BIOs likely will not support querying the size at all,
         * and we assume we got a correct value if req_len > 0.
         */
    }
    if ((
#ifndef OPENSSL_NO_STDIO
         fp != NULL /* definitely correct req_len */ ||
#endif
         req_len > 0)
            && BIO_printf(rctx->mem, "Content-Length: %ld\r\n", req_len) < 0)
        return 0;

    if (!BIO_up_ref(req))
        return 0;
    rctx->req = req;
    return 1;
}

int OSSL_HTTP_REQ_CTX_set1_req(OSSL_HTTP_REQ_CTX *rctx, const char *content_type,
                               const ASN1_ITEM *it, const ASN1_VALUE *req)
{
    BIO *mem = NULL;
    int res = 1;

    if (req != NULL)
        res = (mem = ASN1_item_i2d_mem_bio(it, req)) != NULL;
    res = res && set1_content(rctx, content_type, mem);
    BIO_free(mem);
    return res;
}

void OSSL_HTTP_REQ_CTX_set_max_response_hdr_lines(OSSL_HTTP_REQ_CTX *rctx,
                                                  size_t count)
{
    if (rctx == NULL) {
        ERR_raise(ERR_LIB_HTTP, ERR_R_PASSED_NULL_PARAMETER);
        return;
    }
    rctx->max_hdr_lines = count;
}

static int add1_headers(OSSL_HTTP_REQ_CTX *rctx,
                        const STACK_OF(CONF_VALUE) *headers, const char *host)
{
    int i;
    int add_host = host != NULL && *host != '\0';
    CONF_VALUE *hdr;

    for (i = 0; i < sk_CONF_VALUE_num(headers); i++) {
        hdr = sk_CONF_VALUE_value(headers, i);
        if (add_host && OPENSSL_strcasecmp("host", hdr->name) == 0)
            add_host = 0;
        if (!OSSL_HTTP_REQ_CTX_add1_header(rctx, hdr->name, hdr->value))
            return 0;
    }

    if (add_host && !OSSL_HTTP_REQ_CTX_add1_header(rctx, "Host", host))
        return 0;
    return 1;
}

/* Create OSSL_HTTP_REQ_CTX structure using the values provided. */
static OSSL_HTTP_REQ_CTX *http_req_ctx_new(int free_wbio, BIO *wbio, BIO *rbio,
                                           OSSL_HTTP_bio_cb_t bio_update_fn,
                                           void *arg, int use_ssl,
                                           const char *proxy,
                                           const char *server, const char *port,
                                           int buf_size, int overall_timeout)
{
    OSSL_HTTP_REQ_CTX *rctx = OSSL_HTTP_REQ_CTX_new(wbio, rbio, buf_size);

    if (rctx == NULL)
        return NULL;
    rctx->free_wbio = free_wbio;
    rctx->upd_fn = bio_update_fn;
    rctx->upd_arg = arg;
    rctx->use_ssl = use_ssl;
    if (proxy != NULL
            && (rctx->proxy = OPENSSL_strdup(proxy)) == NULL)
        goto err;
    if (server != NULL
            && (rctx->server = OPENSSL_strdup(server)) == NULL)
        goto err;
    if (port != NULL
            && (rctx->port = OPENSSL_strdup(port)) == NULL)
        goto err;
    rctx->max_total_time =
        overall_timeout > 0 ? time(NULL) + overall_timeout : 0;
    return rctx;

 err:
    OSSL_HTTP_REQ_CTX_free(rctx);
    return NULL;
}

/*
 * Parse first HTTP response line. This should be like this: "HTTP/1.0 200 OK".
 * We need to obtain the status code and (optional) informational message.
 * Return any received HTTP response status code, or 0 on fatal error.
 */

static int parse_http_line1(char *line, int *found_keep_alive)
{
    int i, retcode;
    char *code, *reason, *end;

    if (!CHECK_AND_SKIP_PREFIX(line, HTTP_PREFIX_VERSION))
        goto err;
    /* above HTTP 1.0, connection persistence is the default */
    *found_keep_alive = *line > '0';

    /* Skip to first whitespace (past protocol info) */
    for (code = line; *code != '\0' && !ossl_isspace(*code); code++)
        continue;
    if (*code == '\0')
        goto err;

    /* Skip past whitespace to start of response code */
    while (*code != '\0' && ossl_isspace(*code))
        code++;
    if (*code == '\0')
        goto err;

    /* Find end of response code: first whitespace after start of code */
    for (reason = code; *reason != '\0' && !ossl_isspace(*reason); reason++)
        continue;

    if (*reason == '\0')
        goto err;

    /* Set end of response code and start of message */
    *reason++ = '\0';

    /* Attempt to parse numeric code */
    retcode = strtoul(code, &end, 10);
    if (*end != '\0')
        goto err;

    /* Skip over any leading whitespace in message */
    while (*reason != '\0' && ossl_isspace(*reason))
        reason++;

    if (*reason != '\0') {
        /*
         * Finally zap any trailing whitespace in message (include CRLF)
         */

        /* chop any trailing whitespace from reason */
        /* We know reason has a non-whitespace character so this is OK */
        for (end = reason + strlen(reason) - 1; ossl_isspace(*end); end--)
            *end = '\0';
    }

    switch (retcode) {
    case HTTP_STATUS_CODE_OK:
    case HTTP_STATUS_CODE_MOVED_PERMANENTLY:
    case HTTP_STATUS_CODE_FOUND:
        return retcode;
    default:
        if (retcode < HTTP_STATUS_CODES_NONFATAL_ERROR) {
            ERR_raise_data(ERR_LIB_HTTP, HTTP_R_STATUS_CODE_UNSUPPORTED, "code=%s", code);
            if (*reason != '\0')
                ERR_add_error_data(2, ", reason=", reason);
        } /* must return content normally if status >= 400 */
        return retcode;
    }

 err:
    for (i = 0; i < 60 && line[i] != '\0'; i++)
        if (!ossl_isprint(line[i]))
            line[i] = ' ';
    line[i] = '\0';
    ERR_raise_data(ERR_LIB_HTTP, HTTP_R_HEADER_PARSE_ERROR, "content=%s", line);
    return 0;
}

static int check_max_len(const char *desc, size_t max_len, size_t len)
{
    if (max_len != 0 && len > max_len) {
        ERR_raise_data(ERR_LIB_HTTP, HTTP_R_MAX_RESP_LEN_EXCEEDED,
                       "%s length=%zu, max=%zu", desc, len, max_len);
        return 0;
    }
    return 1;
}

static int check_set_resp_len(const char *desc, OSSL_HTTP_REQ_CTX *rctx, size_t len)
{
    if (!check_max_len(desc, rctx->max_resp_len, len))
        return 0;
    if (rctx->resp_len != 0 && rctx->resp_len != len) {
        ERR_raise_data(ERR_LIB_HTTP, HTTP_R_INCONSISTENT_CONTENT_LENGTH,
                       "%s length=%zu, Content-Length=%zu", desc, len, rctx->resp_len);
        return 0;
    }
    rctx->resp_len = len;
    return 1;
}

static int may_still_retry(time_t max_time, int *ptimeout)
{
    time_t time_diff, now = time(NULL);

    if (max_time != 0) {
        if (max_time < now) {
            ERR_raise(ERR_LIB_HTTP, HTTP_R_RETRY_TIMEOUT);
            return 0;
        }
        time_diff = max_time - now;
        *ptimeout = time_diff > INT_MAX ? INT_MAX : (int)time_diff;
    }
    return 1;
}

/*
 * Try exchanging request and response via HTTP on (non-)blocking BIO in rctx.
 * Returns 1 on success, 0 on error or redirection, -1 on BIO_should_retry.
 */
int OSSL_HTTP_REQ_CTX_nbio(OSSL_HTTP_REQ_CTX *rctx)
{
    int i, found_expected_ct = 0, found_keep_alive = 0;
    int got_text = 1;
    long n;
    size_t resp_len = 0;
    const unsigned char *p;
    char *buf, *key, *value, *line_end = NULL;
    size_t resp_hdr_lines = 0;

    if (rctx == NULL) {
        ERR_raise(ERR_LIB_HTTP, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (rctx->mem == NULL || rctx->wbio == NULL || rctx->rbio == NULL) {
        ERR_raise(ERR_LIB_HTTP, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }

    rctx->redirection_url = NULL;
 next_io:
    buf = (char *)rctx->buf;
    if ((rctx->state & OHS_NOREAD) == 0) {
        if (rctx->expect_asn1 && (rctx->state == OHS_ASN1_HEADER
                                  || rctx->state == OHS_ASN1_CONTENT)) {
            n = BIO_read(rctx->rbio, buf, rctx->buf_size);
        } else { /* read one text line */
            (void)ERR_set_mark();
            n = BIO_gets(rctx->rbio, buf, rctx->buf_size);
            if (n == -2) { /* some BIOs, such as SSL, do not support "gets" */
                (void)ERR_pop_to_mark();
                n = BIO_get_line(rctx->rbio, buf, rctx->buf_size);
            } else {
                (void)ERR_clear_last_mark();
            }
        }
        if (n <= 0) {
            if (rctx->state == OHS_ERROR_CONTENT) {
                if (OSSL_TRACE_ENABLED(HTTP))
                    OSSL_TRACE(HTTP, "]\n"); /* end of error response content */
                /* in addition, throw error on inconsistent length: */
                (void)check_set_resp_len("error response content", rctx, resp_len);
                return 0;
            }
            if (BIO_should_retry(rctx->rbio))
                return -1;
            ERR_raise(ERR_LIB_HTTP, HTTP_R_FAILED_READING_DATA);
            return 0;
        }

        /* Write data to memory BIO */
        if (BIO_write(rctx->mem, buf, n) != n)
            return 0;
    }

    switch (rctx->state) {
    case OHS_ERROR:
    default:
        return 0;

    case OHS_ADD_HEADERS:
        /* Last operation was adding headers: need a final \r\n */
        if (BIO_write(rctx->mem, "\r\n", 2) != 2) {
            rctx->state = OHS_ERROR;
            return 0;
        }
        rctx->state = OHS_WRITE_INIT;

        /* fall through */
    case OHS_WRITE_INIT:
        rctx->len_to_send = BIO_get_mem_data(rctx->mem, &rctx->pos);
        rctx->state = OHS_WRITE_HDR1;

        /* fall through */
    case OHS_WRITE_HDR1:
    case OHS_WRITE_HDR:
        /* Copy some chunk of data from rctx->mem to rctx->wbio */
    case OHS_WRITE_REQ:
        /* Copy some chunk of data from rctx->req to rctx->wbio */

        if (rctx->len_to_send > 0) {
            size_t sz;

            if (!BIO_write_ex(rctx->wbio, rctx->pos, rctx->len_to_send, &sz)) {
                if (BIO_should_retry(rctx->wbio))
                    return -1;
                rctx->state = OHS_ERROR;
                return 0;
            }
            if (OSSL_TRACE_ENABLED(HTTP)) {
                if (rctx->state == OHS_WRITE_HDR1)
                    OSSL_TRACE(HTTP, "Sending request header: [\n");
                /* for request headers, this usually traces several lines at once: */
                OSSL_TRACE_STRING(HTTP, rctx->state != OHS_WRITE_REQ || rctx->text,
                                  rctx->state != OHS_WRITE_REQ, rctx->pos, sz);
                OSSL_TRACE(HTTP, "]\n"); /* end of request header or content */
            }
            if (rctx->state == OHS_WRITE_HDR1)
                rctx->state = OHS_WRITE_HDR;
            rctx->pos += sz;
            rctx->len_to_send -= sz;
            goto next_io;
        }
        if (rctx->state == OHS_WRITE_HDR) {
            (void)BIO_reset(rctx->mem);
            rctx->state = OHS_WRITE_REQ;
        }
        if (rctx->req != NULL && !BIO_eof(rctx->req)) {
            if (OSSL_TRACE_ENABLED(HTTP))
                OSSL_TRACE1(HTTP, "Sending request content (likely %s)\n",
                            rctx->text ? "text" : "ASN.1");
            n = BIO_read(rctx->req, rctx->buf, rctx->buf_size);
            if (n <= 0) {
                if (BIO_should_retry(rctx->req))
                    return -1;
                ERR_raise(ERR_LIB_HTTP, HTTP_R_FAILED_READING_DATA);
                return 0;
            }
            rctx->pos = rctx->buf;
            rctx->len_to_send = n;
            goto next_io;
        }
        rctx->state = OHS_FLUSH;

        /* fall through */
    case OHS_FLUSH:

        i = BIO_flush(rctx->wbio);

        if (i > 0) {
            rctx->state = OHS_FIRSTLINE;
            goto next_io;
        }

        if (BIO_should_retry(rctx->wbio))
            return -1;

        rctx->state = OHS_ERROR;
        return 0;

        /* State machine could be broken up at this point and bulky code sections factorized out. */

    case OHS_FIRSTLINE:
    case OHS_HEADERS:
    case OHS_HEADERS_ERROR:
    case OHS_REDIRECT:
    case OHS_ERROR_CONTENT:

        /* Attempt to read a line in */
 next_line:
        /*
         * Due to strange memory BIO behavior with BIO_gets we have to check
         * there's a complete line in there before calling BIO_gets or we'll
         * just get a partial read.
         */
        n = BIO_get_mem_data(rctx->mem, &p);
        if (n <= 0 || memchr(p, '\n', n) == 0) {
            if (n >= rctx->buf_size) {
                rctx->state = OHS_ERROR;
                return 0;
            }
            goto next_io;
        }
        n = BIO_gets(rctx->mem, buf, rctx->buf_size);

        if (n <= 0) {
            if (BIO_should_retry(rctx->mem))
                goto next_io;
            rctx->state = OHS_ERROR;
            return 0;
        }

        if (rctx->state == OHS_ERROR_CONTENT) {
            resp_len += n;
            if (!check_max_len("error response content", rctx->max_resp_len, resp_len))
                return 0;
            if (OSSL_TRACE_ENABLED(HTTP)) /* dump response content line */
                OSSL_TRACE_STRING(HTTP, got_text, 1, (unsigned char *)buf, n);
            goto next_line;
        }

        resp_hdr_lines++;
        if (rctx->max_hdr_lines != 0 && rctx->max_hdr_lines < resp_hdr_lines) {
            ERR_raise(ERR_LIB_HTTP, HTTP_R_RESPONSE_TOO_MANY_HDRLINES);
            rctx->state = OHS_ERROR;
            return 0;
        }

        /* Don't allow excessive lines */
        if (n == rctx->buf_size) {
            ERR_raise(ERR_LIB_HTTP, HTTP_R_RESPONSE_LINE_TOO_LONG);
            rctx->state = OHS_ERROR;
            return 0;
        }

        if (OSSL_TRACE_ENABLED(HTTP)) {
            /* dump all response header line */
            if (rctx->state == OHS_FIRSTLINE)
                OSSL_TRACE(HTTP, "Receiving response header: [\n");
            OSSL_TRACE_STRING(HTTP, 1, 1, (unsigned char *)buf, n);
        }

        /* First line in response header */
        if (rctx->state == OHS_FIRSTLINE) {
            i = parse_http_line1(buf, &found_keep_alive);
            switch (i) {
            case HTTP_STATUS_CODE_OK:
                rctx->state = OHS_HEADERS;
                goto next_line;
            case HTTP_STATUS_CODE_MOVED_PERMANENTLY:
            case HTTP_STATUS_CODE_FOUND: /* i.e., moved temporarily */
                if (!rctx->method_POST) { /* method is GET */
                    rctx->state = OHS_REDIRECT;
                    goto next_line;
                }
                ERR_raise(ERR_LIB_HTTP, HTTP_R_REDIRECTION_NOT_ENABLED);
                /* redirection is not supported/recommended for POST */
                /* fall through */
            default:
                /* must return content if status >= 400 */
                rctx->state = i < HTTP_STATUS_CODES_NONFATAL_ERROR
                    ? OHS_HEADERS_ERROR : OHS_HEADERS;
                goto next_line; /* continue parsing, also on HTTP error */
            }
        }
        key = buf;
        value = strchr(key, ':');
        if (value != NULL) {
            *(value++) = '\0';
            while (ossl_isspace(*value))
                value++;
            line_end = strchr(value, '\r');
            if (line_end == NULL)
                line_end = strchr(value, '\n');
            if (line_end != NULL)
                *line_end = '\0';
        }
        if (value != NULL && line_end != NULL) {
            if (rctx->state == OHS_REDIRECT
                    && OPENSSL_strcasecmp(key, "Location") == 0) {
                rctx->redirection_url = value;
                if (OSSL_TRACE_ENABLED(HTTP))
                    OSSL_TRACE(HTTP, "]\n");
                /* stop reading due to redirect */
                (void)BIO_reset(rctx->rbio);
                return 0;
            }
            if (OPENSSL_strcasecmp(key, "Content-Type") == 0) {
                got_text = HAS_CASE_PREFIX(value, "text/");
                if (rctx->state == OHS_HEADERS
                    && rctx->expected_ct != NULL) {
                    const char *semicolon;

                    if (OPENSSL_strcasecmp(rctx->expected_ct, value) != 0
                        /* ignore past ';' unless expected_ct contains ';' */
                        && (strchr(rctx->expected_ct, ';') != NULL
                            || (semicolon = strchr(value, ';')) == NULL
                            || (size_t)(semicolon - value) != strlen(rctx->expected_ct)
                            || OPENSSL_strncasecmp(rctx->expected_ct, value,
                                                   semicolon - value) != 0)) {
                        ERR_raise_data(ERR_LIB_HTTP,
                                       HTTP_R_UNEXPECTED_CONTENT_TYPE,
                                       "expected=%s, actual=%s",
                                       rctx->expected_ct, value);
                        return 0;
                    }
                    found_expected_ct = 1;
                }
            }

            /* https://tools.ietf.org/html/rfc7230#section-6.3 Persistence */
            if (OPENSSL_strcasecmp(key, "Connection") == 0) {
                if (OPENSSL_strcasecmp(value, "keep-alive") == 0)
                    found_keep_alive = 1;
                else if (OPENSSL_strcasecmp(value, "close") == 0)
                    found_keep_alive = 0;
            } else if (OPENSSL_strcasecmp(key, "Content-Length") == 0) {
                size_t content_len = (size_t)strtoul(value, &line_end, 10);

                if (line_end == value || *line_end != '\0') {
                    ERR_raise_data(ERR_LIB_HTTP,
                                   HTTP_R_ERROR_PARSING_CONTENT_LENGTH,
                                   "input=%s", value);
                    return 0;
                }
                if (!check_set_resp_len("response content-length", rctx, content_len))
                    return 0;
            }
        }

        /* Look for blank line indicating end of headers */
        for (p = rctx->buf; *p != '\0'; p++) {
            if (*p != '\r' && *p != '\n')
                break;
        }
        if (*p != '\0') /* not end of headers or not end of error reponse content */
            goto next_line;

        /* Found blank line(s) indicating end of headers */
        if (OSSL_TRACE_ENABLED(HTTP))
            OSSL_TRACE(HTTP, "]\n"); /* end of response header */

        if (rctx->keep_alive != 0 /* do not let server initiate keep_alive */
                && !found_keep_alive /* otherwise there is no change */) {
            if (rctx->keep_alive == 2) {
                rctx->keep_alive = 0;
                ERR_raise(ERR_LIB_HTTP, HTTP_R_SERVER_CANCELED_CONNECTION);
                return 0;
            }
            rctx->keep_alive = 0;
        }

        if (rctx->state == OHS_HEADERS_ERROR) {
            rctx->state = OHS_ERROR_CONTENT;
            if (OSSL_TRACE_ENABLED(HTTP)) {
                OSSL_TRACE1(HTTP, "Receiving error response content (likely %s): [\n",
                            got_text ? "text" : "ASN.1");
                goto next_line;
            }
            /* discard response content when trace not enabled */
            (void)BIO_reset(rctx->rbio);
            return 0;
        }

        if (rctx->expected_ct != NULL && !found_expected_ct) {
            ERR_raise_data(ERR_LIB_HTTP, HTTP_R_MISSING_CONTENT_TYPE,
                           "expected=%s", rctx->expected_ct);
            return 0;
        }
        if (rctx->state == OHS_REDIRECT) {
            /* http status code indicated redirect but there was no Location */
            ERR_raise(ERR_LIB_HTTP, HTTP_R_MISSING_REDIRECT_LOCATION);
            return 0;
        }

        /* Note: in non-error situations cannot trace response content */
        if (!rctx->expect_asn1) {
            if (OSSL_TRACE_ENABLED(HTTP))
                OSSL_TRACE(HTTP, "Receiving response text content\n");
            rctx->state = OHS_STREAM;
            return 1;
        }

        if (OSSL_TRACE_ENABLED(HTTP))
            OSSL_TRACE(HTTP, "Receiving response ASN.1 content\n");
        rctx->state = OHS_ASN1_HEADER;

        /* Fall thru */
    case OHS_ASN1_HEADER:
        /*
         * Now reading ASN1 header: can read at least 2 bytes which is enough
         * for ASN1 SEQUENCE header and either length field or at least the
         * length of the length field.
         */
        n = BIO_get_mem_data(rctx->mem, &p);
        if (n < 2)
            goto next_io;

        /* Check it is an ASN1 SEQUENCE */
        if (*p++ != (V_ASN1_SEQUENCE | V_ASN1_CONSTRUCTED)) {
            ERR_raise(ERR_LIB_HTTP, HTTP_R_MISSING_ASN1_ENCODING);
            return 0;
        }

        /* Check out length field */
        if ((*p & 0x80) != 0) {
            /*
             * If MSB set on initial length octet we can now always read 6
             * octets: make sure we have them.
             */
            if (n < 6)
                goto next_io;
            n = *p & 0x7F;
            /* Not NDEF or excessive length */
            if (n == 0 || (n > 4)) {
                ERR_raise(ERR_LIB_HTTP, HTTP_R_ERROR_PARSING_ASN1_LENGTH);
                return 0;
            }
            p++;
            resp_len = 0;
            for (i = 0; i < n; i++) {
                resp_len <<= 8;
                resp_len |= *p++;
            }
            resp_len += n + 2;
        } else {
            resp_len = *p + 2;
        }
        if (!check_set_resp_len("ASN.1 DER content", rctx, resp_len))
            return 0;

        if (OSSL_TRACE_ENABLED(HTTP))
            OSSL_TRACE1(HTTP, "Expected response ASN.1 DER content length: %zd\n", resp_len);
        rctx->state = OHS_ASN1_CONTENT;

        /* Fall thru */
    case OHS_ASN1_CONTENT:
        n = BIO_get_mem_data(rctx->mem, NULL);
        if (n < 0 || (size_t)n < rctx->resp_len)
            goto next_io;

        if (OSSL_TRACE_ENABLED(HTTP))
            OSSL_TRACE(HTTP, "Finished receiving response ASN.1 content\n");
        rctx->state = OHS_ASN1_DONE;
        return 1;
    }
}

int OSSL_HTTP_REQ_CTX_nbio_d2i(OSSL_HTTP_REQ_CTX *rctx,
                               ASN1_VALUE **pval, const ASN1_ITEM *it)
{
    const unsigned char *p;
    int rv;

    *pval = NULL;
    if ((rv = OSSL_HTTP_REQ_CTX_nbio(rctx)) != 1)
        return rv;
    *pval = ASN1_item_d2i(NULL, &p, BIO_get_mem_data(rctx->mem, &p), it);
    return *pval != NULL;

}

#ifndef OPENSSL_NO_SOCK

static const char *explict_or_default_port(const char *hostserv, const char *port, int use_ssl)
{
    if (port == NULL) {
        char *service = NULL;

        if (!BIO_parse_hostserv(hostserv, NULL, &service, BIO_PARSE_PRIO_HOST))
            return NULL;
        if (service == NULL) /* implicit port */
            port = use_ssl ? OSSL_HTTPS_PORT : OSSL_HTTP_PORT;
        OPENSSL_free(service);
    } /* otherwise take the explicitly given port */
    return port;
}

/* set up a new connection BIO, to HTTP server or to HTTP(S) proxy if given */
static BIO *http_new_bio(const char *server /* optionally includes ":port" */,
                         const char *server_port /* explicit server port */,
                         int use_ssl,
                         const char *proxy /* optionally includes ":port" */,
                         const char *proxy_port /* explicit proxy port */)
{
    const char *host = server;
    const char *port = server_port;
    BIO *cbio;

    if (!ossl_assert(server != NULL))
        return NULL;

    if (proxy != NULL) {
        host = proxy;
        port = proxy_port;
    }

    port = explict_or_default_port(host, port, use_ssl);

    cbio = BIO_new_connect(host /* optionally includes ":port" */);
    if (cbio == NULL)
        goto end;
    if (port != NULL)
        (void)BIO_set_conn_port(cbio, port);

 end:
    return cbio;
}
#endif /* OPENSSL_NO_SOCK */

/* Exchange request and response via HTTP on (non-)blocking BIO */
BIO *OSSL_HTTP_REQ_CTX_exchange(OSSL_HTTP_REQ_CTX *rctx)
{
    int rv;

    if (rctx == NULL) {
        ERR_raise(ERR_LIB_HTTP, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }

    for (;;) {
        rv = OSSL_HTTP_REQ_CTX_nbio(rctx);
        if (rv != -1)
            break;
        /* BIO_should_retry was true */
        /* will not actually wait if rctx->max_time == 0 */
        if (BIO_wait(rctx->rbio, rctx->max_time, 100 /* milliseconds */) <= 0)
            return NULL;
    }

    if (rv == 0) {
        if (rctx->redirection_url == NULL) { /* an error occurred */
            if (rctx->len_to_send > 0)
                ERR_raise(ERR_LIB_HTTP, HTTP_R_ERROR_SENDING);
            else
                ERR_raise(ERR_LIB_HTTP, HTTP_R_ERROR_RECEIVING);
        }
        return NULL;
    }
    return rctx->state == OHS_STREAM ? rctx->rbio : rctx->mem;
}

int OSSL_HTTP_is_alive(const OSSL_HTTP_REQ_CTX *rctx)
{
    return rctx != NULL && rctx->keep_alive != 0;
}

/* High-level HTTP API implementation */

/* Initiate an HTTP session using bio, else use given server, proxy, etc. */
OSSL_HTTP_REQ_CTX *OSSL_HTTP_open(const char *server, const char *port,
                                  const char *proxy, const char *no_proxy,
                                  int use_ssl, BIO *bio, BIO *rbio,
                                  OSSL_HTTP_bio_cb_t bio_update_fn, void *arg,
                                  int buf_size, int overall_timeout)
{
    BIO *cbio; /* == bio if supplied, used as connection BIO if rbio is NULL */
    OSSL_HTTP_REQ_CTX *rctx = NULL;

    if (use_ssl && bio_update_fn == NULL) {
        ERR_raise(ERR_LIB_HTTP, HTTP_R_TLS_NOT_ENABLED);
        return NULL;
    }
    if (rbio != NULL && (bio == NULL || bio_update_fn != NULL)) {
        ERR_raise(ERR_LIB_HTTP, ERR_R_PASSED_INVALID_ARGUMENT);
        return NULL;
    }

    if (bio != NULL) {
        cbio = bio;
        if (proxy != NULL || no_proxy != NULL) {
            ERR_raise(ERR_LIB_HTTP, ERR_R_PASSED_INVALID_ARGUMENT);
            return NULL;
        }
    } else {
#ifndef OPENSSL_NO_SOCK
        char *proxy_host = NULL, *proxy_port = NULL;

        if (server == NULL) {
            ERR_raise(ERR_LIB_HTTP, ERR_R_PASSED_NULL_PARAMETER);
            return NULL;
        }
        if (port != NULL && *port == '\0')
            port = NULL;
        proxy = OSSL_HTTP_adapt_proxy(proxy, no_proxy, server, use_ssl);
        if (proxy != NULL
            && !OSSL_HTTP_parse_url(proxy, NULL /* use_ssl */, NULL /* user */,
                                    &proxy_host, &proxy_port, NULL /* num */,
                                    NULL /* path */, NULL, NULL))
            return NULL;
        cbio = http_new_bio(server, port, use_ssl, proxy_host, proxy_port);
        OPENSSL_free(proxy_host);
        OPENSSL_free(proxy_port);
        if (cbio == NULL)
            return NULL;
#else
        ERR_raise(ERR_LIB_HTTP, HTTP_R_SOCK_NOT_SUPPORTED);
        return NULL;
#endif
    }

    (void)ERR_set_mark(); /* prepare removing any spurious libssl errors */
    if (rbio == NULL && BIO_do_connect_retry(cbio, overall_timeout, -1) <= 0) {
        if (bio == NULL) /* cbio was not provided by caller */
            BIO_free_all(cbio);
        goto end;
    }
    /* now overall_timeout is guaranteed to be >= 0 */

    /* adapt in order to fix callback design flaw, see #17088 */
    /* callback can be used to wrap or prepend TLS session */
    if (bio_update_fn != NULL) {
        BIO *orig_bio = cbio;

        cbio = (*bio_update_fn)(cbio, arg, 1 /* connect */, use_ssl != 0);
        if (cbio == NULL) {
            if (bio == NULL) /* cbio was not provided by caller */
                BIO_free_all(orig_bio);
            goto end;
        }
    }

    rctx = http_req_ctx_new(bio == NULL, cbio, rbio != NULL ? rbio : cbio,
                            bio_update_fn, arg, use_ssl, proxy, server, port,
                            buf_size, overall_timeout);

 end:
    if (rctx != NULL)
        /* remove any spurious error queue entries by ssl_add_cert_chain() */
        (void)ERR_pop_to_mark();
    else
        (void)ERR_clear_last_mark();

    return rctx;
}

int OSSL_HTTP_set1_request(OSSL_HTTP_REQ_CTX *rctx, const char *path,
                           const STACK_OF(CONF_VALUE) *headers,
                           const char *content_type, BIO *req,
                           const char *expected_content_type, int expect_asn1,
                           size_t max_resp_len, int timeout, int keep_alive)
{
    int use_http_proxy;

    if (rctx == NULL) {
        ERR_raise(ERR_LIB_HTTP, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    use_http_proxy = rctx->proxy != NULL && !rctx->use_ssl;
    if (use_http_proxy && rctx->server == NULL) {
        ERR_raise(ERR_LIB_HTTP, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    rctx->max_resp_len = max_resp_len; /* allows for 0: indefinite */

    return OSSL_HTTP_REQ_CTX_set_request_line(rctx, req != NULL,
                                              use_http_proxy ? rctx->server
                                              : NULL, rctx->port, path)
        && add1_headers(rctx, headers, rctx->server)
        && OSSL_HTTP_REQ_CTX_set_expected(rctx, expected_content_type,
                                          expect_asn1, timeout, keep_alive)
        && set1_content(rctx, content_type, req);
}

/*-
 * Exchange single HTTP request and response according to rctx.
 * If rctx->method_POST then use POST, else use GET and ignore content_type.
 * The redirection_url output (freed by caller) parameter is used only for GET.
 */
BIO *OSSL_HTTP_exchange(OSSL_HTTP_REQ_CTX *rctx, char **redirection_url)
{
    BIO *resp;

    if (rctx == NULL) {
        ERR_raise(ERR_LIB_HTTP, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }

    if (redirection_url != NULL)
        *redirection_url = NULL; /* do this beforehand to prevent dbl free */

    resp = OSSL_HTTP_REQ_CTX_exchange(rctx);
    if (resp == NULL) {
        if (rctx->redirection_url != NULL) {
            if (redirection_url == NULL)
                ERR_raise(ERR_LIB_HTTP, HTTP_R_REDIRECTION_NOT_ENABLED);
            else
                /* may be NULL if out of memory: */
                *redirection_url = OPENSSL_strdup(rctx->redirection_url);
        } else {
            char buf[200];
            unsigned long err = ERR_peek_error();
            int lib = ERR_GET_LIB(err);
            int reason = ERR_GET_REASON(err);

            if (lib == ERR_LIB_SSL || lib == ERR_LIB_HTTP
                    || (lib == ERR_LIB_BIO && reason == BIO_R_CONNECT_TIMEOUT)
                    || (lib == ERR_LIB_BIO && reason == BIO_R_CONNECT_ERROR)
#ifndef OPENSSL_NO_CMP
                    || (lib == ERR_LIB_CMP
                        && reason == CMP_R_POTENTIALLY_INVALID_CERTIFICATE)
#endif
                ) {
                if (rctx->server != NULL && *rctx->server != '\0') {
                    BIO_snprintf(buf, sizeof(buf), "server=http%s://%s%s%s",
                                 rctx->use_ssl ? "s" : "", rctx->server,
                                 rctx->port != NULL ? ":" : "",
                                 rctx->port != NULL ? rctx->port : "");
                    ERR_add_error_data(1, buf);
                }
                if (rctx->proxy != NULL)
                    ERR_add_error_data(2, " proxy=", rctx->proxy);
                if (err == 0) {
                    BIO_snprintf(buf, sizeof(buf), " peer has disconnected%s",
                                 rctx->use_ssl ? " violating the protocol" :
                                 ", likely because it requires the use of TLS");
                    ERR_add_error_data(1, buf);
                }
            }
        }
    }

    if (resp != NULL && !BIO_up_ref(resp))
        resp = NULL;
    return resp;
}

static int redirection_ok(int n_redir, const char *old_url, const char *new_url)
{
    if (n_redir >= HTTP_VERSION_MAX_REDIRECTIONS) {
        ERR_raise(ERR_LIB_HTTP, HTTP_R_TOO_MANY_REDIRECTIONS);
        return 0;
    }
    if (*new_url == '/') /* redirection to same server => same protocol */
        return 1;
    if (HAS_PREFIX(old_url, OSSL_HTTPS_NAME":") &&
        !HAS_PREFIX(new_url, OSSL_HTTPS_NAME":")) {
        ERR_raise(ERR_LIB_HTTP, HTTP_R_REDIRECTION_FROM_HTTPS_TO_HTTP);
        return 0;
    }
    return 1;
}

/* Get data via HTTP from server at given URL, potentially with redirection */
BIO *OSSL_HTTP_get(const char *url, const char *proxy, const char *no_proxy,
                   BIO *bio, BIO *rbio,
                   OSSL_HTTP_bio_cb_t bio_update_fn, void *arg,
                   int buf_size, const STACK_OF(CONF_VALUE) *headers,
                   const char *expected_ct, int expect_asn1,
                   size_t max_resp_len, int timeout)
{
    char *current_url;
    int n_redirs = 0;
    char *host;
    char *port;
    char *path;
    int use_ssl;
    BIO *resp = NULL;
    time_t max_time = timeout > 0 ? time(NULL) + timeout : 0;

    if (url == NULL) {
        ERR_raise(ERR_LIB_HTTP, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }
    if ((current_url = OPENSSL_strdup(url)) == NULL)
        return NULL;

    for (;;) {
        OSSL_HTTP_REQ_CTX *rctx;
        char *redirection_url;

        if (!OSSL_HTTP_parse_url(current_url, &use_ssl, NULL /* user */, &host,
                                 &port, NULL /* port_num */, &path, NULL, NULL))
            break;

        rctx = OSSL_HTTP_open(host, port, proxy, no_proxy,
                              use_ssl, bio, rbio, bio_update_fn, arg,
                              buf_size, timeout);
    new_rpath:
        redirection_url = NULL;
        if (rctx != NULL) {
            if (!OSSL_HTTP_set1_request(rctx, path, headers,
                                        NULL /* content_type */,
                                        NULL /* req */,
                                        expected_ct, expect_asn1, max_resp_len,
                                        -1 /* use same max time (timeout) */,
                                        0 /* no keep_alive */)) {
                OSSL_HTTP_REQ_CTX_free(rctx);
                rctx = NULL;
            } else {
                resp = OSSL_HTTP_exchange(rctx, &redirection_url);
            }
        }
        OPENSSL_free(path);
        if (resp == NULL && redirection_url != NULL) {
            if (redirection_ok(++n_redirs, current_url, redirection_url)
                    && may_still_retry(max_time, &timeout)) {
                (void)BIO_reset(bio);
                OPENSSL_free(current_url);
                current_url = redirection_url;
                if (*redirection_url == '/') { /* redirection to same server */
                    path = OPENSSL_strdup(redirection_url);
                    if (path == NULL) {
                        OPENSSL_free(host);
                        OPENSSL_free(port);
                        (void)OSSL_HTTP_close(rctx, 1);
                        BIO_free(resp);
                        OPENSSL_free(current_url);
                        return NULL;
                    }
                    goto new_rpath;
                }
                OPENSSL_free(host);
                OPENSSL_free(port);
                (void)OSSL_HTTP_close(rctx, 1);
                continue;
            }
            /* if redirection not allowed, ignore it */
            OPENSSL_free(redirection_url);
        }
        OPENSSL_free(host);
        OPENSSL_free(port);
        if (!OSSL_HTTP_close(rctx, resp != NULL)) {
            BIO_free(resp);
            resp = NULL;
        }
        break;
    }
    OPENSSL_free(current_url);
    return resp;
}

/* Exchange request and response over a connection managed via |prctx| */
BIO *OSSL_HTTP_transfer(OSSL_HTTP_REQ_CTX **prctx,
                        const char *server, const char *port,
                        const char *path, int use_ssl,
                        const char *proxy, const char *no_proxy,
                        BIO *bio, BIO *rbio,
                        OSSL_HTTP_bio_cb_t bio_update_fn, void *arg,
                        int buf_size, const STACK_OF(CONF_VALUE) *headers,
                        const char *content_type, BIO *req,
                        const char *expected_ct, int expect_asn1,
                        size_t max_resp_len, int timeout, int keep_alive)
{
    OSSL_HTTP_REQ_CTX *rctx = prctx == NULL ? NULL : *prctx;
    BIO *resp = NULL;

    if (rctx == NULL) {
        rctx = OSSL_HTTP_open(server, port, proxy, no_proxy,
                              use_ssl, bio, rbio, bio_update_fn, arg,
                              buf_size, timeout);
        timeout = -1; /* Already set during opening the connection */
    }
    if (rctx != NULL) {
        if (OSSL_HTTP_set1_request(rctx, path, headers, content_type, req,
                                   expected_ct, expect_asn1,
                                   max_resp_len, timeout, keep_alive))
            resp = OSSL_HTTP_exchange(rctx, NULL);
        if (resp == NULL || !OSSL_HTTP_is_alive(rctx)) {
            if (!OSSL_HTTP_close(rctx, resp != NULL)) {
                BIO_free(resp);
                resp = NULL;
            }
            rctx = NULL;
        }
    }
    if (prctx != NULL)
        *prctx = rctx;
    return resp;
}

int OSSL_HTTP_close(OSSL_HTTP_REQ_CTX *rctx, int ok)
{
    BIO *wbio;
    int ret = 1;

    /* callback can be used to finish TLS session and free its BIO */
    if (rctx != NULL && rctx->upd_fn != NULL) {
        wbio = (*rctx->upd_fn)(rctx->wbio, rctx->upd_arg,
                               0 /* disconnect */, ok);
        ret = wbio != NULL;
        if (ret)
            rctx->wbio = wbio;
    }
    OSSL_HTTP_REQ_CTX_free(rctx);
    return ret;
}

/* BASE64 encoder used for encoding basic proxy authentication credentials */
static char *base64encode(const void *buf, size_t len)
{
    int i;
    size_t outl;
    char *out;

    /* Calculate size of encoded data */
    outl = (len / 3);
    if (len % 3 > 0)
        outl++;
    outl <<= 2;
    out = OPENSSL_malloc(outl + 1);
    if (out == NULL)
        return 0;

    i = EVP_EncodeBlock((unsigned char *)out, buf, len);
    if (!ossl_assert(0 <= i && (size_t)i <= outl)) {
        OPENSSL_free(out);
        return NULL;
    }
    return out;
}

/*
 * Promote the given connection BIO using the CONNECT method for a TLS proxy.
 * This is typically called by an app, so bio_err and prog are used unless NULL
 * to print additional diagnostic information in a user-oriented way.
 */
int OSSL_HTTP_proxy_connect(BIO *bio, const char *server, const char *port,
                            const char *proxyuser, const char *proxypass,
                            int timeout, BIO *bio_err, const char *prog)
{
#undef BUF_SIZE
#define BUF_SIZE (8 * 1024)
    char *mbuf = OPENSSL_malloc(BUF_SIZE);
    char *mbufp;
    int read_len = 0;
    int ret = 0;
    BIO *fbio = BIO_new(BIO_f_buffer());
    int rv;
    time_t max_time = timeout > 0 ? time(NULL) + timeout : 0;

    if (bio == NULL || server == NULL
            || (bio_err != NULL && prog == NULL)) {
        ERR_raise(ERR_LIB_HTTP, ERR_R_PASSED_NULL_PARAMETER);
        goto end;
    }
    if (port == NULL || *port == '\0')
        port = OSSL_HTTPS_PORT;

    if (mbuf == NULL || fbio == NULL) {
        BIO_printf(bio_err /* may be NULL */, "%s: out of memory", prog);
        goto end;
    }
    BIO_push(fbio, bio);

    BIO_printf(fbio, "CONNECT %s:%s "HTTP_1_0"\r\n", server, port);

    /*
     * Workaround for broken proxies which would otherwise close
     * the connection when entering tunnel mode (e.g., Squid 2.6)
     */
    BIO_printf(fbio, "Proxy-Connection: Keep-Alive\r\n");

    /* Support for basic (base64) proxy authentication */
    if (proxyuser != NULL) {
        size_t len = strlen(proxyuser) + 1;
        char *proxyauth, *proxyauthenc = NULL;

        if (proxypass != NULL)
            len += strlen(proxypass);
        proxyauth = OPENSSL_malloc(len + 1);
        if (proxyauth == NULL)
            goto end;
        if (BIO_snprintf(proxyauth, len + 1, "%s:%s", proxyuser,
                         proxypass != NULL ? proxypass : "") != (int)len)
            goto proxy_end;
        proxyauthenc = base64encode(proxyauth, len);
        if (proxyauthenc != NULL) {
            BIO_printf(fbio, "Proxy-Authorization: Basic %s\r\n", proxyauthenc);
            OPENSSL_clear_free(proxyauthenc, strlen(proxyauthenc));
        }
    proxy_end:
        OPENSSL_clear_free(proxyauth, len);
        if (proxyauthenc == NULL)
            goto end;
    }

    /* Terminate the HTTP CONNECT request */
    BIO_printf(fbio, "\r\n");

    for (;;) {
        if (BIO_flush(fbio) != 0)
            break;
        /* potentially needs to be retried if BIO is non-blocking */
        if (!BIO_should_retry(fbio))
            break;
    }

    for (;;) {
        /* will not actually wait if timeout == 0 */
        rv = BIO_wait(fbio, max_time, 100 /* milliseconds */);
        if (rv <= 0) {
            BIO_printf(bio_err, "%s: HTTP CONNECT %s\n", prog,
                       rv == 0 ? "timed out" : "failed waiting for data");
            goto end;
        }

        /*-
         * The first line is the HTTP response.
         * According to RFC 7230, it is formatted exactly like this:
         * HTTP/d.d ddd reason text\r\n
         */
        read_len = BIO_gets(fbio, mbuf, BUF_SIZE);
        /* the BIO may not block, so we must wait for the 1st line to come in */
        if (read_len < (int)HTTP_LINE1_MINLEN)
            continue;

        /* Check for HTTP/1.x */
        mbufp = mbuf;
        if (!CHECK_AND_SKIP_PREFIX(mbufp, HTTP_PREFIX)) {
            ERR_raise(ERR_LIB_HTTP, HTTP_R_HEADER_PARSE_ERROR);
            BIO_printf(bio_err, "%s: HTTP CONNECT failed, non-HTTP response\n",
                       prog);
            /* Wrong protocol, not even HTTP, so stop reading headers */
            goto end;
        }
        if (!HAS_PREFIX(mbufp, HTTP_VERSION_PATT)) {
            ERR_raise(ERR_LIB_HTTP, HTTP_R_RECEIVED_WRONG_HTTP_VERSION);
            BIO_printf(bio_err,
                       "%s: HTTP CONNECT failed, bad HTTP version %.*s\n",
                       prog, (int)HTTP_VERSION_STR_LEN, mbufp);
            goto end;
        }
        mbufp += HTTP_VERSION_STR_LEN;

        /* RFC 7231 4.3.6: any 2xx status code is valid */
        if (!HAS_PREFIX(mbufp, " 2")) {
            if (ossl_isspace(*mbufp))
                mbufp++;
            /* chop any trailing whitespace */
            while (read_len > 0 && ossl_isspace(mbuf[read_len - 1]))
                read_len--;
            mbuf[read_len] = '\0';
            ERR_raise_data(ERR_LIB_HTTP, HTTP_R_CONNECT_FAILURE,
                           "reason=%s", mbufp);
            BIO_printf(bio_err, "%s: HTTP CONNECT failed, reason=%s\n",
                       prog, mbufp);
            goto end;
        }
        ret = 1;
        break;
    }

    /* Read past all following headers */
    do {
        /*
         * This does not necessarily catch the case when the full
         * HTTP response came in more than a single TCP message.
         */
        read_len = BIO_gets(fbio, mbuf, BUF_SIZE);
    } while (read_len > 2);

 end:
    if (fbio != NULL) {
        (void)BIO_flush(fbio);
        BIO_pop(fbio);
        BIO_free(fbio);
    }
    OPENSSL_free(mbuf);
    return ret;
#undef BUF_SIZE
}
