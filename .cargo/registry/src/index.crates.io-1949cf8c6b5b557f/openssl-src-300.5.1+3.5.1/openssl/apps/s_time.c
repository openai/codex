/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <openssl/opensslconf.h>

#ifndef OPENSSL_NO_SOCK

#include "apps.h"
#include "progs.h"
#include <openssl/x509.h>
#include <openssl/ssl.h>
#include <openssl/pem.h>
#include "s_apps.h"
#include <openssl/err.h>
#include "internal/sockets.h"
#if !defined(OPENSSL_SYS_MSDOS)
# include <unistd.h>
#endif

#define SSL_CONNECT_NAME        "localhost:4433"

#define SECONDS 30
#define SECONDSSTR "30"

static SSL *doConnection(SSL *scon, const char *host, SSL_CTX *ctx);

/*
 * Define a HTTP get command globally.
 * Also define the size of the command, this is two bytes less than
 * the size of the string because the %s is replaced by the URL.
 */
static const char fmt_http_get_cmd[] = "GET %s HTTP/1.0\r\n\r\n";
static const size_t fmt_http_get_cmd_size = sizeof(fmt_http_get_cmd) - 2;

typedef enum OPTION_choice {
    OPT_COMMON,
    OPT_CONNECT, OPT_CIPHER, OPT_CIPHERSUITES, OPT_CERT, OPT_NAMEOPT, OPT_KEY,
    OPT_CAPATH, OPT_CAFILE, OPT_CASTORE,
    OPT_NOCAPATH, OPT_NOCAFILE, OPT_NOCASTORE,
    OPT_NEW, OPT_REUSE, OPT_BUGS, OPT_VERIFY, OPT_TIME, OPT_SSL3,
    OPT_WWW, OPT_TLS1, OPT_TLS1_1, OPT_TLS1_2, OPT_TLS1_3,
    OPT_PROV_ENUM
} OPTION_CHOICE;

const OPTIONS s_time_options[] = {
    OPT_SECTION("General"),
    {"help", OPT_HELP, '-', "Display this summary"},

    OPT_SECTION("Connection"),
    {"connect", OPT_CONNECT, 's',
     "Where to connect as post:port (default is " SSL_CONNECT_NAME ")"},
    {"new", OPT_NEW, '-', "Just time new connections"},
    {"reuse", OPT_REUSE, '-', "Just time connection reuse"},
    {"bugs", OPT_BUGS, '-', "Turn on SSL bug compatibility"},
    {"cipher", OPT_CIPHER, 's', "TLSv1.2 and below cipher list to be used"},
    {"ciphersuites", OPT_CIPHERSUITES, 's',
     "Specify TLSv1.3 ciphersuites to be used"},
#ifndef OPENSSL_NO_SSL3
    {"ssl3", OPT_SSL3, '-', "Just use SSLv3"},
#endif
#ifndef OPENSSL_NO_TLS1
    {"tls1", OPT_TLS1, '-', "Just use TLSv1.0"},
#endif
#ifndef OPENSSL_NO_TLS1_1
    {"tls1_1", OPT_TLS1_1, '-', "Just use TLSv1.1"},
#endif
#ifndef OPENSSL_NO_TLS1_2
    {"tls1_2", OPT_TLS1_2, '-', "Just use TLSv1.2"},
#endif
#ifndef OPENSSL_NO_TLS1_3
    {"tls1_3", OPT_TLS1_3, '-', "Just use TLSv1.3"},
#endif
    {"verify", OPT_VERIFY, 'p',
     "Turn on peer certificate verification, set depth"},
    {"time", OPT_TIME, 'p', "Seconds to collect data, default " SECONDSSTR},
    {"www", OPT_WWW, 's', "Fetch specified page from the site"},

    OPT_SECTION("Certificate"),
    {"nameopt", OPT_NAMEOPT, 's', "Certificate subject/issuer name printing options"},
    {"cert", OPT_CERT, '<', "Cert file to use, PEM format assumed"},
    {"key", OPT_KEY, '<', "File with key, PEM; default is -cert file"},
    {"cafile", OPT_CAFILE, '<', "PEM format file of CA's"},
    {"CAfile", OPT_CAFILE, '<', "PEM format file of CA's"},
    {"CApath", OPT_CAPATH, '/', "PEM format directory of CA's"},
    {"CAstore", OPT_CASTORE, ':', "URI to store of CA's"},
    {"no-CAfile", OPT_NOCAFILE, '-',
     "Do not load the default certificates file"},
    {"no-CApath", OPT_NOCAPATH, '-',
     "Do not load certificates from the default certificates directory"},
    {"no-CAstore", OPT_NOCASTORE, '-',
     "Do not load certificates from the default certificates store URI"},

    OPT_PROV_OPTIONS,
    {NULL}
};

#define START   0
#define STOP    1

static double tm_Time_F(int s)
{
    return app_tminterval(s, 1);
}

int s_time_main(int argc, char **argv)
{
    char buf[1024 * 8];
    SSL *scon = NULL;
    SSL_CTX *ctx = NULL;
    const SSL_METHOD *meth = NULL;
    char *CApath = NULL, *CAfile = NULL, *CAstore = NULL;
    char *cipher = NULL, *ciphersuites = NULL;
    char *www_path = NULL;
    char *host = SSL_CONNECT_NAME, *certfile = NULL, *keyfile = NULL, *prog;
    double totalTime = 0.0;
    int noCApath = 0, noCAfile = 0, noCAstore = 0;
    int maxtime = SECONDS, nConn = 0, perform = 3, ret = 1, i, st_bugs = 0;
    long bytes_read = 0, finishtime = 0;
    OPTION_CHOICE o;
    int min_version = 0, max_version = 0, ver, buf_len, fd;
    size_t buf_size;

    meth = TLS_client_method();

    prog = opt_init(argc, argv, s_time_options);
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_EOF:
        case OPT_ERR:
 opthelp:
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            goto end;
        case OPT_HELP:
            opt_help(s_time_options);
            ret = 0;
            goto end;
        case OPT_CONNECT:
            host = opt_arg();
            break;
        case OPT_REUSE:
            perform = 2;
            break;
        case OPT_NEW:
            perform = 1;
            break;
        case OPT_VERIFY:
            verify_args.depth = opt_int_arg();
            BIO_printf(bio_err, "%s: verify depth is %d\n",
                       prog, verify_args.depth);
            break;
        case OPT_CERT:
            certfile = opt_arg();
            break;
        case OPT_NAMEOPT:
            if (!set_nameopt(opt_arg()))
                goto end;
            break;
        case OPT_KEY:
            keyfile = opt_arg();
            break;
        case OPT_CAPATH:
            CApath = opt_arg();
            break;
        case OPT_CAFILE:
            CAfile = opt_arg();
            break;
        case OPT_NOCAPATH:
            noCApath = 1;
            break;
        case OPT_NOCAFILE:
            noCAfile = 1;
            break;
        case OPT_CASTORE:
            CAstore = opt_arg();
            break;
        case OPT_NOCASTORE:
            noCAstore = 1;
            break;
        case OPT_CIPHER:
            cipher = opt_arg();
            break;
        case OPT_CIPHERSUITES:
            ciphersuites = opt_arg();
            break;
        case OPT_BUGS:
            st_bugs = 1;
            break;
        case OPT_TIME:
            maxtime = opt_int_arg();
            break;
        case OPT_WWW:
            www_path = opt_arg();
            buf_size = strlen(www_path) + fmt_http_get_cmd_size;
            if (buf_size > sizeof(buf)) {
                BIO_printf(bio_err, "%s: -www option is too long\n", prog);
                goto end;
            }
            break;
        case OPT_SSL3:
            min_version = SSL3_VERSION;
            max_version = SSL3_VERSION;
            break;
        case OPT_TLS1:
            min_version = TLS1_VERSION;
            max_version = TLS1_VERSION;
            break;
        case OPT_TLS1_1:
            min_version = TLS1_1_VERSION;
            max_version = TLS1_1_VERSION;
            break;
        case OPT_TLS1_2:
            min_version = TLS1_2_VERSION;
            max_version = TLS1_2_VERSION;
            break;
        case OPT_TLS1_3:
            min_version = TLS1_3_VERSION;
            max_version = TLS1_3_VERSION;
            break;
        case OPT_PROV_CASES:
            if (!opt_provider(o))
                goto end;
            break;
        }
    }

    /* No extra arguments. */
    if (!opt_check_rest_arg(NULL))
        goto opthelp;

    if (cipher == NULL)
        cipher = getenv("SSL_CIPHER");

    if ((ctx = SSL_CTX_new(meth)) == NULL)
        goto end;

    SSL_CTX_set_quiet_shutdown(ctx, 1);
    if (SSL_CTX_set_min_proto_version(ctx, min_version) == 0)
        goto end;
    if (SSL_CTX_set_max_proto_version(ctx, max_version) == 0)
        goto end;

    if (st_bugs)
        SSL_CTX_set_options(ctx, SSL_OP_ALL);
    if (cipher != NULL && !SSL_CTX_set_cipher_list(ctx, cipher))
        goto end;
    if (ciphersuites != NULL && !SSL_CTX_set_ciphersuites(ctx, ciphersuites))
        goto end;
    if (!set_cert_stuff(ctx, certfile, keyfile))
        goto end;

    if (!ctx_set_verify_locations(ctx, CAfile, noCAfile, CApath, noCApath,
                                  CAstore, noCAstore)) {
        ERR_print_errors(bio_err);
        goto end;
    }
    if (!(perform & 1))
        goto next;
    printf("Collecting connection statistics for %d seconds\n", maxtime);

    /* Loop and time how long it takes to make connections */

    bytes_read = 0;
    finishtime = (long)time(NULL) + maxtime;
    tm_Time_F(START);
    for (;;) {
        if (finishtime < (long)time(NULL))
            break;

        if ((scon = doConnection(NULL, host, ctx)) == NULL)
            goto end;

        if (www_path != NULL) {
            buf_len = BIO_snprintf(buf, sizeof(buf), fmt_http_get_cmd,
                                   www_path);
            if (buf_len <= 0 || SSL_write(scon, buf, buf_len) <= 0)
                goto end;
            while ((i = SSL_read(scon, buf, sizeof(buf))) > 0)
                bytes_read += i;
        }
        SSL_set_shutdown(scon, SSL_SENT_SHUTDOWN | SSL_RECEIVED_SHUTDOWN);
        BIO_closesocket(SSL_get_fd(scon));

        nConn += 1;
        if (SSL_session_reused(scon)) {
            ver = 'r';
        } else {
            ver = SSL_version(scon);
            if (ver == TLS1_VERSION)
                ver = 't';
            else if (ver == SSL3_VERSION)
                ver = '3';
            else
                ver = '*';
        }
        fputc(ver, stdout);
        fflush(stdout);

        SSL_free(scon);
        scon = NULL;
    }
    totalTime += tm_Time_F(STOP); /* Add the time for this iteration */

    printf
        ("\n\n%d connections in %.2fs; %.2f connections/user sec, bytes read %ld\n",
         nConn, totalTime, ((double)nConn / totalTime), bytes_read);
    printf
        ("%d connections in %ld real seconds, %ld bytes read per connection\n",
         nConn, (long)time(NULL) - finishtime + maxtime,
         nConn > 0 ? bytes_read / nConn : 0l);

    /*
     * Now loop and time connections using the same session id over and over
     */

 next:
    if (!(perform & 2)) {
        ret = 0;
        goto end;
    }
    printf("\n\nNow timing with session id reuse.\n");

    /* Get an SSL object so we can reuse the session id */
    if ((scon = doConnection(NULL, host, ctx)) == NULL) {
        BIO_printf(bio_err, "Unable to get connection\n");
        goto end;
    }

    if (www_path != NULL) {
        buf_len = BIO_snprintf(buf, sizeof(buf), fmt_http_get_cmd, www_path);
        if (buf_len <= 0 || SSL_write(scon, buf, buf_len) <= 0)
            goto end;
        while (SSL_read(scon, buf, sizeof(buf)) > 0)
            continue;
    }
    SSL_set_shutdown(scon, SSL_SENT_SHUTDOWN | SSL_RECEIVED_SHUTDOWN);
    if ((fd = SSL_get_fd(scon)) >= 0)
        BIO_closesocket(fd);

    nConn = 0;
    totalTime = 0.0;

    finishtime = (long)time(NULL) + maxtime;

    printf("starting\n");
    bytes_read = 0;
    tm_Time_F(START);

    for (;;) {
        if (finishtime < (long)time(NULL))
            break;

        if ((doConnection(scon, host, ctx)) == NULL)
            goto end;

        if (www_path != NULL) {
            buf_len = BIO_snprintf(buf, sizeof(buf), fmt_http_get_cmd,
                                   www_path);
            if (buf_len <= 0 || SSL_write(scon, buf, buf_len) <= 0)
                goto end;
            while ((i = SSL_read(scon, buf, sizeof(buf))) > 0)
                bytes_read += i;
        }
        SSL_set_shutdown(scon, SSL_SENT_SHUTDOWN | SSL_RECEIVED_SHUTDOWN);
        if ((fd = SSL_get_fd(scon)) >= 0)
            BIO_closesocket(fd);

        nConn += 1;
        if (SSL_session_reused(scon)) {
            ver = 'r';
        } else {
            ver = SSL_version(scon);
            if (ver == TLS1_VERSION)
                ver = 't';
            else if (ver == SSL3_VERSION)
                ver = '3';
            else
                ver = '*';
        }
        fputc(ver, stdout);
        fflush(stdout);
    }
    totalTime += tm_Time_F(STOP); /* Add the time for this iteration */

    printf
        ("\n\n%d connections in %.2fs; %.2f connections/user sec, bytes read %ld\n",
         nConn, totalTime, ((double)nConn / totalTime), bytes_read);
    if (nConn > 0)
        printf
            ("%d connections in %ld real seconds, %ld bytes read per connection\n",
             nConn, (long)time(NULL) - finishtime + maxtime, bytes_read / nConn);
    else
        printf("0 connections in %ld real seconds\n",
               (long)time(NULL) - finishtime + maxtime);
    ret = 0;

 end:
    SSL_free(scon);
    SSL_CTX_free(ctx);
    return ret;
}

/*-
 * doConnection - make a connection
 */
static SSL *doConnection(SSL *scon, const char *host, SSL_CTX *ctx)
{
    BIO *conn;
    SSL *serverCon;
    int i;

    if ((conn = BIO_new(BIO_s_connect())) == NULL)
        return NULL;

    if (BIO_set_conn_hostname(conn, host) <= 0
            || BIO_set_conn_mode(conn, BIO_SOCK_NODELAY) <= 0) {
        BIO_free(conn);
        return NULL;
    }

    if (scon == NULL) {
        serverCon = SSL_new(ctx);
        if (serverCon == NULL) {
            BIO_free(conn);
            return NULL;
        }
    } else {
        serverCon = scon;
        SSL_set_connect_state(serverCon);
    }

    SSL_set_bio(serverCon, conn, conn);

    /* ok, lets connect */
    i = SSL_connect(serverCon);
    if (i <= 0) {
        BIO_printf(bio_err, "ERROR\n");
        if (verify_args.error != X509_V_OK)
            BIO_printf(bio_err, "verify error:%s\n",
                       X509_verify_cert_error_string(verify_args.error));
        else
            ERR_print_errors(bio_err);
        if (scon == NULL)
            SSL_free(serverCon);
        return NULL;
    }

#if defined(SOL_SOCKET) && defined(SO_LINGER)
    {
        struct linger no_linger;
        int fd;

        no_linger.l_onoff  = 1;
        no_linger.l_linger = 0;
        fd = SSL_get_fd(serverCon);
        if (fd >= 0)
            (void)setsockopt(fd, SOL_SOCKET, SO_LINGER, (char*)&no_linger,
                             sizeof(no_linger));
    }
#endif

    return serverCon;
}
#endif /* OPENSSL_NO_SOCK */
