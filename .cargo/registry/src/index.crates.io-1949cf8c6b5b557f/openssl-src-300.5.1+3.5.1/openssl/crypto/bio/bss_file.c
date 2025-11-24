/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#if defined(__linux) || defined(__sun) || defined(__hpux)
/*
 * Following definition aliases fopen to fopen64 on above mentioned
 * platforms. This makes it possible to open and sequentially access files
 * larger than 2GB from 32-bit application. It does not allow one to traverse
 * them beyond 2GB with fseek/ftell, but on the other hand *no* 32-bit
 * platform permits that, not with fseek/ftell. Not to mention that breaking
 * 2GB limit for seeking would require surgery to *our* API. But sequential
 * access suffices for practical cases when you can run into large files,
 * such as fingerprinting, so we can let API alone. For reference, the list
 * of 32-bit platforms which allow for sequential access of large files
 * without extra "magic" comprise *BSD, Darwin, IRIX...
 */
# ifndef _FILE_OFFSET_BITS
#  define _FILE_OFFSET_BITS 64
# endif
#endif

#include <stdio.h>
#include <errno.h>
#include "bio_local.h"
#include <openssl/err.h>

#if !defined(OPENSSL_NO_STDIO)

static int file_write(BIO *h, const char *buf, int num);
static int file_read(BIO *h, char *buf, int size);
static int file_puts(BIO *h, const char *str);
static int file_gets(BIO *h, char *str, int size);
static long file_ctrl(BIO *h, int cmd, long arg1, void *arg2);
static int file_new(BIO *h);
static int file_free(BIO *data);
static const BIO_METHOD methods_filep = {
    BIO_TYPE_FILE,
    "FILE pointer",
    bwrite_conv,
    file_write,
    bread_conv,
    file_read,
    file_puts,
    file_gets,
    file_ctrl,
    file_new,
    file_free,
    NULL,                      /* file_callback_ctrl */
};

BIO *BIO_new_file(const char *filename, const char *mode)
{
    BIO  *ret;
    FILE *file = openssl_fopen(filename, mode);
    int fp_flags = BIO_CLOSE;

    if (strchr(mode, 'b') == NULL)
        fp_flags |= BIO_FP_TEXT;

    if (file == NULL) {
        ERR_raise_data(ERR_LIB_SYS, get_last_sys_error(),
                       "calling fopen(%s, %s)",
                       filename, mode);
        if (errno == ENOENT
#ifdef ENXIO
            || errno == ENXIO
#endif
            )
            ERR_raise(ERR_LIB_BIO, BIO_R_NO_SUCH_FILE);
        else
            ERR_raise(ERR_LIB_BIO, ERR_R_SYS_LIB);
        return NULL;
    }
    if ((ret = BIO_new(BIO_s_file())) == NULL) {
        fclose(file);
        return NULL;
    }

    /* we did fopen -> we disengage UPLINK */
    BIO_clear_flags(ret, BIO_FLAGS_UPLINK_INTERNAL);
    BIO_set_fp(ret, file, fp_flags);
    return ret;
}

BIO *BIO_new_fp(FILE *stream, int close_flag)
{
    BIO *ret;

    if ((ret = BIO_new(BIO_s_file())) == NULL)
        return NULL;

    /* redundant flag, left for documentation purposes */
    BIO_set_flags(ret, BIO_FLAGS_UPLINK_INTERNAL);
    BIO_set_fp(ret, stream, close_flag);
    return ret;
}

const BIO_METHOD *BIO_s_file(void)
{
    return &methods_filep;
}

static int file_new(BIO *bi)
{
    bi->init = 0;
    bi->num = 0;
    bi->ptr = NULL;
    bi->flags = BIO_FLAGS_UPLINK_INTERNAL; /* default to UPLINK */
    return 1;
}

static int file_free(BIO *a)
{
    if (a == NULL)
        return 0;
    if (a->shutdown) {
        if ((a->init) && (a->ptr != NULL)) {
            if (a->flags & BIO_FLAGS_UPLINK_INTERNAL)
                UP_fclose(a->ptr);
            else
                fclose(a->ptr);
            a->ptr = NULL;
            a->flags = BIO_FLAGS_UPLINK_INTERNAL;
        }
        a->init = 0;
    }
    return 1;
}

static int file_read(BIO *b, char *out, int outl)
{
    int ret = 0;

    if (b->init && (out != NULL)) {
        if (b->flags & BIO_FLAGS_UPLINK_INTERNAL)
            ret = UP_fread(out, 1, (int)outl, b->ptr);
        else
            ret = fread(out, 1, (int)outl, (FILE *)b->ptr);
        if (ret == 0
            && (b->flags & BIO_FLAGS_UPLINK_INTERNAL
                ? UP_ferror((FILE *)b->ptr) : ferror((FILE *)b->ptr))) {
            ERR_raise_data(ERR_LIB_SYS, get_last_sys_error(),
                           "calling fread()");
            ERR_raise(ERR_LIB_BIO, ERR_R_SYS_LIB);
            ret = -1;
        }
    }
    return ret;
}

static int file_write(BIO *b, const char *in, int inl)
{
    int ret = 0;

    if (b->init && (in != NULL)) {
        if (b->flags & BIO_FLAGS_UPLINK_INTERNAL)
            ret = UP_fwrite(in, (int)inl, 1, b->ptr);
        else
            ret = fwrite(in, (int)inl, 1, (FILE *)b->ptr);
        if (ret)
            ret = inl;
        /* ret=fwrite(in,1,(int)inl,(FILE *)b->ptr); */
        /*
         * according to Tim Hudson <tjh@openssl.org>, the commented out
         * version above can cause 'inl' write calls under some stupid stdio
         * implementations (VMS)
         */
    }
    return ret;
}

static long file_ctrl(BIO *b, int cmd, long num, void *ptr)
{
    long ret = 1;
    FILE *fp = (FILE *)b->ptr;
    FILE **fpp;
    char p[4];
    int st;

    switch (cmd) {
    case BIO_C_FILE_SEEK:
    case BIO_CTRL_RESET:
        if (b->flags & BIO_FLAGS_UPLINK_INTERNAL)
            ret = (long)UP_fseek(b->ptr, num, 0);
        else
            ret = (long)fseek(fp, num, 0);
        break;
    case BIO_CTRL_EOF:
        if (b->flags & BIO_FLAGS_UPLINK_INTERNAL)
            ret = (long)UP_feof(fp);
        else
            ret = (long)feof(fp);
        break;
    case BIO_C_FILE_TELL:
    case BIO_CTRL_INFO:
        if (b->flags & BIO_FLAGS_UPLINK_INTERNAL)
            ret = UP_ftell(b->ptr);
        else
            ret = ftell(fp);
        break;
    case BIO_C_SET_FILE_PTR:
        file_free(b);
        b->shutdown = (int)num & BIO_CLOSE;
        b->ptr = ptr;
        b->init = 1;
# if BIO_FLAGS_UPLINK_INTERNAL!=0
#  if defined(__MINGW32__) && defined(__MSVCRT__) && !defined(_IOB_ENTRIES)
#   define _IOB_ENTRIES 20
#  endif
        /* Safety net to catch purely internal BIO_set_fp calls */
#  if (defined(_MSC_VER) && _MSC_VER>=1900) || defined(__BORLANDC__)
        if (ptr == stdin || ptr == stdout || ptr == stderr)
            BIO_clear_flags(b, BIO_FLAGS_UPLINK_INTERNAL);
#  elif defined(_IOB_ENTRIES)
        if ((size_t)ptr >= (size_t)stdin &&
            (size_t)ptr < (size_t)(stdin + _IOB_ENTRIES))
            BIO_clear_flags(b, BIO_FLAGS_UPLINK_INTERNAL);
#  endif
# endif
# ifdef UP_fsetmod
        if (b->flags & BIO_FLAGS_UPLINK_INTERNAL)
            UP_fsetmod(b->ptr, (char)((num & BIO_FP_TEXT) ? 't' : 'b'));
        else
# endif
        {
# if defined(OPENSSL_SYS_WINDOWS)
            int fd = _fileno((FILE *)ptr);
            if (num & BIO_FP_TEXT)
                _setmode(fd, _O_TEXT);
            else
                _setmode(fd, _O_BINARY);
# elif defined(OPENSSL_SYS_MSDOS)
            int fd = fileno((FILE *)ptr);
            /* Set correct text/binary mode */
            if (num & BIO_FP_TEXT)
                _setmode(fd, _O_TEXT);
            /* Dangerous to set stdin/stdout to raw (unless redirected) */
            else {
                if (fd == STDIN_FILENO || fd == STDOUT_FILENO) {
                    if (isatty(fd) <= 0)
                        _setmode(fd, _O_BINARY);
                } else
                    _setmode(fd, _O_BINARY);
            }
# elif defined(OPENSSL_SYS_WIN32_CYGWIN)
            int fd = fileno((FILE *)ptr);
            if (!(num & BIO_FP_TEXT))
                setmode(fd, O_BINARY);
# endif
        }
        break;
    case BIO_C_SET_FILENAME:
        file_free(b);
        b->shutdown = (int)num & BIO_CLOSE;
        if (num & BIO_FP_APPEND) {
            if (num & BIO_FP_READ)
                OPENSSL_strlcpy(p, "a+", sizeof(p));
            else
                OPENSSL_strlcpy(p, "a", sizeof(p));
        } else if ((num & BIO_FP_READ) && (num & BIO_FP_WRITE))
            OPENSSL_strlcpy(p, "r+", sizeof(p));
        else if (num & BIO_FP_WRITE)
            OPENSSL_strlcpy(p, "w", sizeof(p));
        else if (num & BIO_FP_READ)
            OPENSSL_strlcpy(p, "r", sizeof(p));
        else {
            ERR_raise(ERR_LIB_BIO, BIO_R_BAD_FOPEN_MODE);
            ret = 0;
            break;
        }
# if defined(OPENSSL_SYS_MSDOS) || defined(OPENSSL_SYS_WINDOWS)
        if (!(num & BIO_FP_TEXT))
            OPENSSL_strlcat(p, "b", sizeof(p));
        else
            OPENSSL_strlcat(p, "t", sizeof(p));
# elif defined(OPENSSL_SYS_WIN32_CYGWIN)
        if (!(num & BIO_FP_TEXT))
            OPENSSL_strlcat(p, "b", sizeof(p));
# endif
        fp = openssl_fopen(ptr, p);
        if (fp == NULL) {
            ERR_raise_data(ERR_LIB_SYS, get_last_sys_error(),
                           "calling fopen(%s, %s)",
                           ptr, p);
            ERR_raise(ERR_LIB_BIO, ERR_R_SYS_LIB);
            ret = 0;
            break;
        }
        b->ptr = fp;
        b->init = 1;
        /* we did fopen -> we disengage UPLINK */
        BIO_clear_flags(b, BIO_FLAGS_UPLINK_INTERNAL);
        break;
    case BIO_C_GET_FILE_PTR:
        /* the ptr parameter is actually a FILE ** in this case. */
        if (ptr != NULL) {
            fpp = (FILE **)ptr;
            *fpp = (FILE *)b->ptr;
        }
        break;
    case BIO_CTRL_GET_CLOSE:
        ret = (long)b->shutdown;
        break;
    case BIO_CTRL_SET_CLOSE:
        b->shutdown = (int)num;
        break;
    case BIO_CTRL_FLUSH:
        st = b->flags & BIO_FLAGS_UPLINK_INTERNAL
                ? UP_fflush(b->ptr) : fflush((FILE *)b->ptr);
        if (st == EOF) {
            ERR_raise_data(ERR_LIB_SYS, get_last_sys_error(),
                           "calling fflush()");
            ERR_raise(ERR_LIB_BIO, ERR_R_SYS_LIB);
            ret = 0;
        }
        break;
    case BIO_CTRL_DUP:
        ret = 1;
        break;

    case BIO_CTRL_WPENDING:
    case BIO_CTRL_PENDING:
    case BIO_CTRL_PUSH:
    case BIO_CTRL_POP:
    default:
        ret = 0;
        break;
    }
    return ret;
}

static int file_gets(BIO *bp, char *buf, int size)
{
    int ret = 0;

    buf[0] = '\0';
    if (bp->flags & BIO_FLAGS_UPLINK_INTERNAL) {
        if (!UP_fgets(buf, size, bp->ptr))
            goto err;
    } else {
        if (!fgets(buf, size, (FILE *)bp->ptr))
            goto err;
    }
    if (buf[0] != '\0')
        ret = strlen(buf);
 err:
    return ret;
}

static int file_puts(BIO *bp, const char *str)
{
    int n, ret;

    n = strlen(str);
    ret = file_write(bp, str, n);
    return ret;
}

#else

static int file_write(BIO *b, const char *in, int inl)
{
    return -1;
}
static int file_read(BIO *b, char *out, int outl)
{
    return -1;
}
static int file_puts(BIO *bp, const char *str)
{
    return -1;
}
static int file_gets(BIO *bp, char *buf, int size)
{
    return 0;
}
static long file_ctrl(BIO *b, int cmd, long num, void *ptr)
{
    return 0;
}
static int file_new(BIO *bi)
{
    return 0;
}
static int file_free(BIO *a)
{
    return 0;
}

static const BIO_METHOD methods_filep = {
    BIO_TYPE_FILE,
    "FILE pointer",
    bwrite_conv,
    file_write,
    bread_conv,
    file_read,
    file_puts,
    file_gets,
    file_ctrl,
    file_new,
    file_free,
    NULL,                      /* file_callback_ctrl */
};

const BIO_METHOD *BIO_s_file(void)
{
    return &methods_filep;
}

BIO *BIO_new_file(const char *filename, const char *mode)
{
    return NULL;
}

#endif                         /* OPENSSL_NO_STDIO */
