/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Very basic HTTP server */

#if !defined(_POSIX_C_SOURCE) && defined(OPENSSL_SYS_VMS)
/*
 * On VMS, you need to define this to get the declaration of fileno().  The
 * value 2 is to make sure no function defined in POSIX-2 is left undefined.
 */
# define _POSIX_C_SOURCE 2
#endif

#include <ctype.h>
#include "internal/e_os.h"
#include "http_server.h"
#include "internal/sockets.h" /* for openssl_fdset() */

#include <openssl/err.h>
#include <openssl/trace.h>
#include <openssl/rand.h>
#include "s_apps.h"
#include "log.h"

#define HTTP_PREFIX "HTTP/"
#define HTTP_VERSION_PATT "1." /* allow 1.x */
#define HTTP_PREFIX_VERSION HTTP_PREFIX""HTTP_VERSION_PATT
#define HTTP_1_0 HTTP_PREFIX_VERSION"0" /* "HTTP/1.0" */
#define HTTP_VERSION_STR " "HTTP_PREFIX_VERSION

#define log_HTTP(prog, level, text) \
    trace_log_message(OSSL_TRACE_CATEGORY_HTTP, prog, level, "%s", text)
#define log_HTTP1(prog, level, fmt, arg) \
    trace_log_message(OSSL_TRACE_CATEGORY_HTTP, prog, level, fmt, arg)
#define log_HTTP2(prog, level, fmt, arg1, arg2) \
    trace_log_message(OSSL_TRACE_CATEGORY_HTTP, prog, level, fmt, arg1, arg2)
#define log_HTTP3(prog, level, fmt, a1, a2, a3)                        \
    trace_log_message(OSSL_TRACE_CATEGORY_HTTP, prog, level, fmt, a1, a2, a3)

#ifdef HTTP_DAEMON
int n_responders = 0; /* run multiple responder processes, set by ocsp.c */
int acfd = (int)INVALID_SOCKET;

void socket_timeout(int signum)
{
    if (acfd != (int)INVALID_SOCKET)
        (void)shutdown(acfd, SHUT_RD);
}

static void killall(int ret, pid_t *kidpids)
{
    int i;

    for (i = 0; i < n_responders; ++i)
        if (kidpids[i] != 0)
            (void)kill(kidpids[i], SIGTERM);
    OPENSSL_free(kidpids);
    OSSL_sleep(1000);
    exit(ret);
}

static int termsig = 0;

static void noteterm(int sig)
{
    termsig = sig;
}

/*
 * Loop spawning up to `multi` child processes, only child processes return
 * from this function.  The parent process loops until receiving a termination
 * signal, kills extant children and exits without returning.
 */
void spawn_loop(const char *prog)
{
    pid_t *kidpids = NULL;
    int status;
    int procs = 0;
    int i;

    openlog(prog, LOG_PID, LOG_DAEMON);

    if (setpgid(0, 0)) {
        log_HTTP1(prog, LOG_CRIT,
                  "error detaching from parent process group: %s",
                  strerror(errno));
        exit(1);
    }
    kidpids = app_malloc(n_responders * sizeof(*kidpids), "child PID array");
    for (i = 0; i < n_responders; ++i)
        kidpids[i] = 0;

    signal(SIGINT, noteterm);
    signal(SIGTERM, noteterm);

    while (termsig == 0) {
        pid_t fpid;

        /*
         * Wait for a child to replace when we're at the limit.
         * Slow down if a child exited abnormally or waitpid() < 0
         */
        while (termsig == 0 && procs >= n_responders) {
            if ((fpid = waitpid(-1, &status, 0)) > 0) {
                for (i = 0; i < procs; ++i) {
                    if (kidpids[i] == fpid) {
                        kidpids[i] = 0;
                        --procs;
                        break;
                    }
                }
                if (i >= n_responders) {
                    log_HTTP1(prog, LOG_CRIT,
                              "internal error: no matching child slot for pid: %ld",
                              (long)fpid);
                    killall(1, kidpids);
                }
                if (status != 0) {
                    if (WIFEXITED(status)) {
                        log_HTTP2(prog, LOG_WARNING,
                                  "child process: %ld, exit status: %d",
                                  (long)fpid, WEXITSTATUS(status));
                    } else if (WIFSIGNALED(status)) {
                        char *dumped = "";

# ifdef WCOREDUMP
                        if (WCOREDUMP(status))
                            dumped = " (core dumped)";
# endif
                        log_HTTP3(prog, LOG_WARNING,
                                  "child process: %ld, term signal %d%s",
                                  (long)fpid, WTERMSIG(status), dumped);
                    }
                    OSSL_sleep(1000);
                }
                break;
            } else if (errno != EINTR) {
                log_HTTP1(prog, LOG_CRIT,
                          "waitpid() failed: %s", strerror(errno));
                killall(1, kidpids);
            }
        }
        if (termsig)
            break;

        switch (fpid = fork()) {
        case -1: /* error */
            /* System critically low on memory, pause and try again later */
            OSSL_sleep(30000);
            break;
        case 0: /* child */
            OPENSSL_free(kidpids);
            signal(SIGINT, SIG_DFL);
            signal(SIGTERM, SIG_DFL);
            if (termsig)
                _exit(0);
            if (RAND_poll() <= 0) {
                log_HTTP(prog, LOG_CRIT, "RAND_poll() failed");
                _exit(1);
            }
            return;
        default:            /* parent */
            for (i = 0; i < n_responders; ++i) {
                if (kidpids[i] == 0) {
                    kidpids[i] = fpid;
                    procs++;
                    break;
                }
            }
            if (i >= n_responders) {
                log_HTTP(prog, LOG_CRIT,
                         "internal error: no free child slots");
                killall(1, kidpids);
            }
            break;
        }
    }

    /* The loop above can only break on termsig */
    log_HTTP1(prog, LOG_INFO, "terminating on signal: %d", termsig);
    killall(0, kidpids);
}
#endif

#ifndef OPENSSL_NO_SOCK
BIO *http_server_init(const char *prog, const char *port, int verb)
{
    BIO *acbio = NULL, *bufbio;
    int asock;
    int port_num;
    char name[40];

    BIO_snprintf(name, sizeof(name), "*:%s", port); /* port may be "0" */
    if (verb >= 0 && !log_set_verbosity(prog, verb))
        return NULL;
    bufbio = BIO_new(BIO_f_buffer());
    if (bufbio == NULL)
        goto err;
    acbio = BIO_new(BIO_s_accept());
    if (acbio == NULL
        || BIO_set_accept_ip_family(acbio, BIO_FAMILY_IPANY) <= 0 /* IPv4/6 */
        || BIO_set_bind_mode(acbio, BIO_BIND_REUSEADDR) <= 0
        || BIO_set_accept_name(acbio, name) <= 0) {
        log_HTTP(prog, LOG_ERR, "error setting up accept BIO");
        goto err;
    }

    BIO_set_accept_bios(acbio, bufbio);
    bufbio = NULL;
    if (BIO_do_accept(acbio) <= 0) {
        log_HTTP1(prog, LOG_ERR, "error setting accept on port %s", port);
        goto err;
    }

    /* Report back what address and port are used */
    BIO_get_fd(acbio, &asock);
    port_num = report_server_accept(bio_out, asock, 1, 1);
    if (port_num == 0) {
        log_HTTP(prog, LOG_ERR, "error printing ACCEPT string");
        goto err;
    }

    return acbio;

 err:
    ERR_print_errors(bio_err);
    BIO_free_all(acbio);
    BIO_free(bufbio);
    return NULL;
}

/*
 * Decode %xx URL-decoding in-place. Ignores malformed sequences.
 */
static int urldecode(char *p)
{
    unsigned char *out = (unsigned char *)p;
    unsigned char *save = out;

    for (; *p; p++) {
        if (*p != '%') {
            *out++ = *p;
        } else if (isxdigit(_UC(p[1])) && isxdigit(_UC(p[2]))) {
            /* Don't check, can't fail because of ixdigit() call. */
            *out++ = (OPENSSL_hexchar2int(p[1]) << 4)
                | OPENSSL_hexchar2int(p[2]);
            p += 2;
        } else {
            return -1;
        }
    }
    *out = '\0';
    return (int)(out - save);
}

/* if *pcbio != NULL, continue given connected session, else accept new */
/* if found_keep_alive != NULL, return this way connection persistence state */
int http_server_get_asn1_req(const ASN1_ITEM *it, ASN1_VALUE **preq,
                             char **ppath, BIO **pcbio, BIO *acbio,
                             int *found_keep_alive,
                             const char *prog, int accept_get, int timeout)
{
    BIO *cbio = *pcbio, *getbio = NULL, *b64 = NULL;
    int len;
    char reqbuf[2048], inbuf[2048];
    char *meth, *url, *end;
    ASN1_VALUE *req;
    int ret = 0;

    *preq = NULL;
    if (ppath != NULL)
        *ppath = NULL;

    if (cbio == NULL) {
        char *port;

        get_sock_info_address(BIO_get_fd(acbio, NULL), NULL, &port);
        if (port == NULL) {
            log_HTTP(prog, LOG_ERR, "cannot get port listening on");
            goto fatal;
        }
        log_HTTP1(prog, LOG_DEBUG,
                  "awaiting new connection on port %s ...", port);
        OPENSSL_free(port);

        if (BIO_do_accept(acbio) <= 0)
            /* Connection loss before accept() is routine, ignore silently */
            return ret;

        *pcbio = cbio = BIO_pop(acbio);
    } else {
        log_HTTP(prog, LOG_DEBUG, "awaiting next request ...");
    }
    if (cbio == NULL) {
        /* Cannot call http_server_send_status(..., cbio, ...) */
        ret = -1;
        goto out;
    }

# ifdef HTTP_DAEMON
    if (timeout > 0) {
        (void)BIO_get_fd(cbio, &acfd);
        alarm(timeout);
    }
# endif

    /* Read the request line. */
    len = BIO_gets(cbio, reqbuf, sizeof(reqbuf));
    if (len == 0)
        return ret;
    ret = 1;
    if (len < 0) {
        log_HTTP(prog, LOG_WARNING, "request line read error");
        (void)http_server_send_status(prog, cbio, 400, "Bad Request");
        goto out;
    }

    if (((end = strchr(reqbuf, '\r')) != NULL && end[1] == '\n')
            || (end = strchr(reqbuf, '\n')) != NULL)
        *end = '\0';
    if (log_get_verbosity() < LOG_TRACE)
        trace_log_message(-1, prog, LOG_INFO,
                          "received request, 1st line: %s", reqbuf);
    log_HTTP(prog, LOG_TRACE, "received request header:");
    log_HTTP1(prog, LOG_TRACE, "%s", reqbuf);
    if (end == NULL) {
        log_HTTP(prog, LOG_WARNING,
                 "cannot parse HTTP header: missing end of line");
        (void)http_server_send_status(prog, cbio, 400, "Bad Request");
        goto out;
    }

    url = meth = reqbuf;
    if ((accept_get && CHECK_AND_SKIP_PREFIX(url, "GET "))
            || CHECK_AND_SKIP_PREFIX(url, "POST ")) {

        /* Expecting (GET|POST) {sp} /URL {sp} HTTP/1.x */
        url[-1] = '\0';
        while (*url == ' ')
            url++;
        if (*url != '/') {
            log_HTTP2(prog, LOG_WARNING,
                      "invalid %s -- URL does not begin with '/': %s",
                      meth, url);
            (void)http_server_send_status(prog, cbio, 400, "Bad Request");
            goto out;
        }
        url++;

        /* Splice off the HTTP version identifier. */
        for (end = url; *end != '\0'; end++)
            if (*end == ' ')
                break;
        if (!HAS_PREFIX(end, HTTP_VERSION_STR)) {
            log_HTTP2(prog, LOG_WARNING,
                      "invalid %s -- bad HTTP/version string: %s",
                      meth, end + 1);
            (void)http_server_send_status(prog, cbio, 400, "Bad Request");
            goto out;
        }
        *end = '\0';
        /* above HTTP 1.0, connection persistence is the default */
        if (found_keep_alive != NULL)
            *found_keep_alive = end[sizeof(HTTP_VERSION_STR) - 1] > '0';

        /*-
         * Skip "GET / HTTP..." requests often used by load-balancers.
         * 'url' was incremented above to point to the first byte *after*
         * the leading slash, so in case 'GET / ' it is now an empty string.
         */
        if (strlen(meth) == 3 && url[0] == '\0') {
            (void)http_server_send_status(prog, cbio, 200, "OK");
            goto out;
        }

        len = urldecode(url);
        if (len < 0) {
            log_HTTP2(prog, LOG_WARNING,
                      "invalid %s request -- bad URL encoding: %s", meth, url);
            (void)http_server_send_status(prog, cbio, 400, "Bad Request");
            goto out;
        }
        if (strlen(meth) == 3) { /* GET */
            if ((getbio = BIO_new_mem_buf(url, len)) == NULL
                || (b64 = BIO_new(BIO_f_base64())) == NULL) {
                log_HTTP1(prog, LOG_ERR,
                          "could not allocate base64 bio with size = %d", len);
                goto fatal;
            }
            BIO_set_flags(b64, BIO_FLAGS_BASE64_NO_NL);
            getbio = BIO_push(b64, getbio);
        }
    } else {
        log_HTTP2(prog, LOG_WARNING,
                  "HTTP request does not begin with %sPOST: %s",
                  accept_get ? "GET or " : "", reqbuf);
        (void)http_server_send_status(prog, cbio, 400, "Bad Request");
        goto out;
    }

    /* chop any further/duplicate leading or trailing '/' */
    while (*url == '/')
        url++;
    while (end >= url + 2 && end[-2] == '/' && end[-1] == '/')
        end--;
    *end = '\0';

    /* Read and skip past the headers. */
    for (;;) {
        char *key, *value;

        len = BIO_gets(cbio, inbuf, sizeof(inbuf));
        if (len <= 0) {
            log_HTTP(prog, LOG_WARNING, "error reading HTTP header");
            (void)http_server_send_status(prog, cbio, 400, "Bad Request");
            goto out;
        }

        if (((end = strchr(inbuf, '\r')) != NULL && end[1] == '\n')
            || (end = strchr(inbuf, '\n')) != NULL)
            *end = '\0';
        log_HTTP1(prog, LOG_TRACE, "%s", *inbuf == '\0' ?
                  " " /* workaround for "" getting ignored */ : inbuf);
        if (end == NULL) {
            log_HTTP(prog, LOG_WARNING,
                     "error parsing HTTP header: missing end of line");
            (void)http_server_send_status(prog, cbio, 400, "Bad Request");
            goto out;
        }

        if (inbuf[0] == '\0')
            break;

        key = inbuf;
        value = strchr(key, ':');
        if (value == NULL) {
            log_HTTP(prog, LOG_WARNING,
                     "error parsing HTTP header: missing ':'");
            (void)http_server_send_status(prog, cbio, 400, "Bad Request");
            goto out;
        }
        *(value++) = '\0';
        while (*value == ' ')
            value++;
        /* https://tools.ietf.org/html/rfc7230#section-6.3 Persistence */
        if (found_keep_alive != NULL
            && OPENSSL_strcasecmp(key, "Connection") == 0) {
            if (OPENSSL_strcasecmp(value, "keep-alive") == 0)
                *found_keep_alive = 1;
            else if (OPENSSL_strcasecmp(value, "close") == 0)
                *found_keep_alive = 0;
        }
    }

# ifdef HTTP_DAEMON
    /* Clear alarm before we close the client socket */
    alarm(0);
    timeout = 0;
# endif

    /* Try to read and parse request */
    req = ASN1_item_d2i_bio(it, getbio != NULL ? getbio : cbio, NULL);
    if (req == NULL) {
        log_HTTP(prog, LOG_WARNING,
                 "error parsing DER-encoded request content");
        (void)http_server_send_status(prog, cbio, 400, "Bad Request");
    } else if (ppath != NULL && (*ppath = OPENSSL_strdup(url)) == NULL) {
        log_HTTP1(prog, LOG_ERR,
                  "out of memory allocating %zu bytes", strlen(url) + 1);
        ASN1_item_free(req, it);
        goto fatal;
    }

    *preq = req;

 out:
    BIO_free_all(getbio);
# ifdef HTTP_DAEMON
    if (timeout > 0)
        alarm(0);
    acfd = (int)INVALID_SOCKET;
# endif
    return ret;

 fatal:
    (void)http_server_send_status(prog, cbio, 500, "Internal Server Error");
    if (ppath != NULL) {
        OPENSSL_free(*ppath);
        *ppath = NULL;
    }
    BIO_free_all(cbio);
    *pcbio = NULL;
    ret = -1;
    goto out;
}

/* assumes that cbio does not do an encoding that changes the output length */
int http_server_send_asn1_resp(const char *prog, BIO *cbio, int keep_alive,
                               const char *content_type,
                               const ASN1_ITEM *it, const ASN1_VALUE *resp)
{
    char buf[200], *p;
    int ret = BIO_snprintf(buf, sizeof(buf), HTTP_1_0" 200 OK\r\n%s"
                           "Content-type: %s\r\n"
                           "Content-Length: %d\r\n",
                           keep_alive ? "Connection: keep-alive\r\n" : "",
                           content_type,
                           ASN1_item_i2d(resp, NULL, it));

    if (ret < 0 || (size_t)ret >= sizeof(buf))
        return 0;
    if (log_get_verbosity() < LOG_TRACE && (p = strchr(buf, '\r')) != NULL)
        trace_log_message(-1, prog, LOG_INFO,
                          "sending response, 1st line: %.*s", (int)(p - buf),
                          buf);
    log_HTTP1(prog, LOG_TRACE, "sending response header:\n%s", buf);

    ret = BIO_printf(cbio, "%s\r\n", buf) > 0
        && ASN1_item_i2d_bio(it, cbio, resp) > 0;

    (void)BIO_flush(cbio);
    return ret;
}

int http_server_send_status(const char *prog, BIO *cbio,
                            int status, const char *reason)
{
    char buf[200];
    int ret = BIO_snprintf(buf, sizeof(buf), HTTP_1_0" %d %s\r\n\r\n",
                           /* This implicitly cancels keep-alive */
                           status, reason);

    if (ret < 0 || (size_t)ret >= sizeof(buf))
        return 0;
    log_HTTP1(prog, LOG_TRACE, "sending response header:\n%s", buf);

    ret = BIO_printf(cbio, "%s\r\n", buf) > 0;
    (void)BIO_flush(cbio);
    return ret;
}
#endif
