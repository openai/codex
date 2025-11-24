/*
 * Copyright 2004-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#define APPLINK_STDIN   1
#define APPLINK_STDOUT  2
#define APPLINK_STDERR  3
#define APPLINK_FPRINTF 4
#define APPLINK_FGETS   5
#define APPLINK_FREAD   6
#define APPLINK_FWRITE  7
#define APPLINK_FSETMOD 8
#define APPLINK_FEOF    9
#define APPLINK_FCLOSE  10      /* should not be used */

#define APPLINK_FOPEN   11      /* solely for completeness */
#define APPLINK_FSEEK   12
#define APPLINK_FTELL   13
#define APPLINK_FFLUSH  14
#define APPLINK_FERROR  15
#define APPLINK_CLEARERR 16
#define APPLINK_FILENO  17      /* to be used with below */

#define APPLINK_OPEN    18      /* formally can't be used, as flags can vary */
#define APPLINK_READ    19
#define APPLINK_WRITE   20
#define APPLINK_LSEEK   21
#define APPLINK_CLOSE   22
#define APPLINK_MAX     22      /* always same as last macro */

#ifndef APPMACROS_ONLY

/*
 * Normally, do not define APPLINK_NO_INCLUDES.  Define it if you are using
 * symbol preprocessing and do not want the preprocessing to affect the
 * following included header files.  You will need to put these
 * include lines somewhere in the file that is including applink.c.
 */
# ifndef APPLINK_NO_INCLUDES
#  include <stdio.h>
#  include <io.h>
#  include <fcntl.h>
# endif

# ifdef __BORLANDC__
   /* _lseek in <io.h> is a function-like macro so we can't take its address */
#  undef _lseek
#  define _lseek lseek
# endif

static void *app_stdin(void)
{
    return stdin;
}

static void *app_stdout(void)
{
    return stdout;
}

static void *app_stderr(void)
{
    return stderr;
}

static int app_feof(FILE *fp)
{
    return feof(fp);
}

static int app_ferror(FILE *fp)
{
    return ferror(fp);
}

static void app_clearerr(FILE *fp)
{
    clearerr(fp);
}

static int app_fileno(FILE *fp)
{
    return _fileno(fp);
}

static int app_fsetmod(FILE *fp, char mod)
{
    return _setmode(_fileno(fp), mod == 'b' ? _O_BINARY : _O_TEXT);
}

#ifdef __cplusplus
extern "C" {
#endif

__declspec(dllexport)
void **
# if defined(__BORLANDC__)
/*
 * __stdcall appears to be the only way to get the name
 * decoration right with Borland C. Otherwise it works
 * purely incidentally, as we pass no parameters.
 */
__stdcall
# else
__cdecl
# endif
OPENSSL_Applink(void)
{
    static int once = 1;
    static void *OPENSSL_ApplinkTable[APPLINK_MAX + 1] =
        { (void *)APPLINK_MAX };

    if (once) {
        OPENSSL_ApplinkTable[APPLINK_STDIN] = app_stdin;
        OPENSSL_ApplinkTable[APPLINK_STDOUT] = app_stdout;
        OPENSSL_ApplinkTable[APPLINK_STDERR] = app_stderr;
        OPENSSL_ApplinkTable[APPLINK_FPRINTF] = fprintf;
        OPENSSL_ApplinkTable[APPLINK_FGETS] = fgets;
        OPENSSL_ApplinkTable[APPLINK_FREAD] = fread;
        OPENSSL_ApplinkTable[APPLINK_FWRITE] = fwrite;
        OPENSSL_ApplinkTable[APPLINK_FSETMOD] = app_fsetmod;
        OPENSSL_ApplinkTable[APPLINK_FEOF] = app_feof;
        OPENSSL_ApplinkTable[APPLINK_FCLOSE] = fclose;

        OPENSSL_ApplinkTable[APPLINK_FOPEN] = fopen;
        OPENSSL_ApplinkTable[APPLINK_FSEEK] = fseek;
        OPENSSL_ApplinkTable[APPLINK_FTELL] = ftell;
        OPENSSL_ApplinkTable[APPLINK_FFLUSH] = fflush;
        OPENSSL_ApplinkTable[APPLINK_FERROR] = app_ferror;
        OPENSSL_ApplinkTable[APPLINK_CLEARERR] = app_clearerr;
        OPENSSL_ApplinkTable[APPLINK_FILENO] = app_fileno;

        OPENSSL_ApplinkTable[APPLINK_OPEN] = _open;
        OPENSSL_ApplinkTable[APPLINK_READ] = _read;
        OPENSSL_ApplinkTable[APPLINK_WRITE] = _write;
        OPENSSL_ApplinkTable[APPLINK_LSEEK] = _lseek;
        OPENSSL_ApplinkTable[APPLINK_CLOSE] = _close;

        once = 0;
    }

    return OPENSSL_ApplinkTable;
}

#ifdef __cplusplus
}
#endif
#endif
