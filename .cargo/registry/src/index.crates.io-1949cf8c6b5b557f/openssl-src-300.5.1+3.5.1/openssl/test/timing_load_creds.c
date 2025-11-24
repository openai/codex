/*
 * Copyright 2020-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <stdlib.h>

#include <openssl/e_os2.h>

#ifdef OPENSSL_SYS_UNIX
# include <sys/stat.h>
# include <sys/resource.h>
# include <openssl/pem.h>
# include <openssl/x509.h>
# include <openssl/err.h>
# include <openssl/bio.h>
# include "internal/e_os.h"
# if defined(_POSIX_VERSION) && _POSIX_VERSION >= 200112L

# ifndef timersub
/* struct timeval * subtraction; a must be greater than or equal to b */
#  define timersub(a, b, res)                                         \
     do {                                                             \
         (res)->tv_sec = (a)->tv_sec - (b)->tv_sec;                   \
         if ((a)->tv_usec < (b)->tv_usec) {                           \
             (res)->tv_usec = (a)->tv_usec + 1000000 - (b)->tv_usec;  \
             --(res)->tv_sec;                                         \
         } else {                                                     \
             (res)->tv_usec = (a)->tv_usec - (b)->tv_usec;            \
         }                                                            \
     } while(0)
# endif

static char *prog;

static void readx509(const char *contents, int size)
{
    X509 *x = NULL;
    BIO *b = BIO_new_mem_buf(contents, size);

    if (b == NULL) {
        ERR_print_errors_fp(stderr);
        exit(EXIT_FAILURE);
    }
    PEM_read_bio_X509(b, &x, 0, NULL);
    if (x == NULL) {
        ERR_print_errors_fp(stderr);
        exit(EXIT_FAILURE);
    }
    X509_free(x);
    BIO_free(b);
}

static void readpkey(const char *contents, int size)
{
    BIO *b = BIO_new_mem_buf(contents, size);
    EVP_PKEY *pkey;

    if (b == NULL) {
        ERR_print_errors_fp(stderr);
        exit(EXIT_FAILURE);
    }
    pkey = PEM_read_bio_PrivateKey(b, NULL, NULL, NULL);
    if (pkey == NULL) {
        ERR_print_errors_fp(stderr);
        exit(EXIT_FAILURE);
    }

    EVP_PKEY_free(pkey);
    BIO_free(b);
}

static void print_timeval(const char *what, struct timeval *tp)
{
    printf("%s %d sec %d microsec\n", what, (int)tp->tv_sec, (int)tp->tv_usec);
}

static void usage(void)
{
    fprintf(stderr, "Usage: %s [flags] pem-file\n", prog);
    fprintf(stderr, "Flags, with the default being '-wc':\n");
    fprintf(stderr, "  -c #  Repeat count\n");
    fprintf(stderr, "  -d    Debugging output (minimal)\n");
    fprintf(stderr, "  -w<T> What to load T is a single character:\n");
    fprintf(stderr, "          c for cert\n");
    fprintf(stderr, "          p for private key\n");
    exit(EXIT_FAILURE);
}
# endif
#endif

int main(int ac, char **av)
{
#if defined(_POSIX_VERSION) && _POSIX_VERSION >= 200112L
    int i, debug = 0, count = 100, what = 'c';
    struct stat sb;
    FILE *fp;
    char *contents;
    struct rusage start, end, elapsed;
    struct timeval e_start, e_end, e_elapsed;

    /* Parse JCL. */
    prog = av[0];
    while ((i = getopt(ac, av, "c:dw:")) != EOF) {
        switch (i) {
        default:
            usage();
            break;
        case 'c':
            if ((count = atoi(optarg)) < 0)
                usage();
            break;
        case 'd':
            debug = 1;
            break;
        case 'w':
            if (optarg[1] != '\0')
                usage();
            switch (*optarg) {
            default:
                usage();
                break;
            case 'c':
            case 'p':
                what = *optarg;
                break;
            }
            break;
        }
    }
    ac -= optind;
    av += optind;

    /* Read input file. */
    if (av[0] == NULL)
        usage();
    if (stat(av[0], &sb) < 0) {
        perror(av[0]);
        exit(EXIT_FAILURE);
    }
    contents = OPENSSL_malloc(sb.st_size + 1);
    if (contents == NULL) {
        perror("malloc");
        exit(EXIT_FAILURE);
    }
    fp = fopen(av[0], "r");
    if ((long)fread(contents, 1, sb.st_size, fp) != sb.st_size) {
        perror("fread");
        exit(EXIT_FAILURE);
    }
    contents[sb.st_size] = '\0';
    fclose(fp);
    if (debug)
        printf(">%s<\n", contents);

    /* Try to prep system cache, etc. */
    for (i = 10; i > 0; i--) {
        switch (what) {
        case 'c':
            readx509(contents, (int)sb.st_size);
            break;
        case 'p':
            readpkey(contents, (int)sb.st_size);
            break;
        }
    }

    if (gettimeofday(&e_start, NULL) < 0) {
        perror("elapsed start");
        exit(EXIT_FAILURE);
    }
    if (getrusage(RUSAGE_SELF, &start) < 0) {
        perror("start");
        exit(EXIT_FAILURE);
    }
    for (i = count; i > 0; i--) {
        switch (what) {
        case 'c':
            readx509(contents, (int)sb.st_size);
            break;
        case 'p':
            readpkey(contents, (int)sb.st_size);
            break;
        }
    }
    if (getrusage(RUSAGE_SELF, &end) < 0) {
        perror("getrusage");
        exit(EXIT_FAILURE);
    }
    if (gettimeofday(&e_end, NULL) < 0) {
        perror("gettimeofday");
        exit(EXIT_FAILURE);
    }

    timersub(&end.ru_utime, &start.ru_stime, &elapsed.ru_stime);
    timersub(&end.ru_utime, &start.ru_utime, &elapsed.ru_utime);
    timersub(&e_end, &e_start, &e_elapsed);
    print_timeval("user     ", &elapsed.ru_utime);
    print_timeval("sys      ", &elapsed.ru_stime);
    if (debug)
        print_timeval("elapsed??", &e_elapsed);

    OPENSSL_free(contents);
    return EXIT_SUCCESS;
#else
    fprintf(stderr,
            "This tool is not supported on this platform for lack of POSIX1.2001 support\n");
    exit(EXIT_FAILURE);
#endif
}
