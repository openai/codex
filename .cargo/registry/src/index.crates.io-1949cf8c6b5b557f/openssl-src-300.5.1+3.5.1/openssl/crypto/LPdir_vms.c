/*
 * Copyright 2004-2016 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * This file is dual-licensed and is also available under the following
 * terms:
 *
 * Copyright (c) 2004, Richard Levitte <richard@levitte.org>
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
 * ``AS IS'' AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
 * LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
 * A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
 * OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
 * SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
 * LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
 * DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
 * THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
 * (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
 * OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */

#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <descrip.h>
#include <namdef.h>
#include <rmsdef.h>
#include <libfildef.h>
#include <lib$routines.h>
#include <strdef.h>
#include <str$routines.h>
#include <stsdef.h>
#ifndef LPDIR_H
# include "LPdir.h"
#endif
#include "vms_rms.h"

/* Some compiler options hide EVMSERR. */
#ifndef EVMSERR
# define EVMSERR        65535   /* error for non-translatable VMS errors */
#endif

struct LP_dir_context_st {
    unsigned long VMS_context;
    char filespec[NAMX_MAXRSS + 1];
    char result[NAMX_MAXRSS + 1];
    struct dsc$descriptor_d filespec_dsc;
    struct dsc$descriptor_d result_dsc;
};

const char *LP_find_file(LP_DIR_CTX **ctx, const char *directory)
{
    int status;
    char *p, *r;
    size_t l;
    unsigned long flags = 0;

/* Arrange 32-bit pointer to (copied) string storage, if needed. */
#if __INITIAL_POINTER_SIZE == 64
# pragma pointer_size save
# pragma pointer_size 32
    char *ctx_filespec_32p;
# pragma pointer_size restore
    char ctx_filespec_32[NAMX_MAXRSS + 1];
#endif                          /* __INITIAL_POINTER_SIZE == 64 */

#ifdef NAML$C_MAXRSS
    flags |= LIB$M_FIL_LONG_NAMES;
#endif

    if (ctx == NULL || directory == NULL) {
        errno = EINVAL;
        return 0;
    }

    errno = 0;
    if (*ctx == NULL) {
        size_t filespeclen = strlen(directory);
        char *filespec = NULL;

        if (filespeclen == 0) {
            errno = ENOENT;
            return 0;
        }

        /* MUST be a VMS directory specification!  Let's estimate if it is. */
        if (directory[filespeclen - 1] != ']'
            && directory[filespeclen - 1] != '>'
            && directory[filespeclen - 1] != ':') {
            errno = EINVAL;
            return 0;
        }

        filespeclen += 4;       /* "*.*;" */

        if (filespeclen > NAMX_MAXRSS) {
            errno = ENAMETOOLONG;
            return 0;
        }

        *ctx = malloc(sizeof(**ctx));
        if (*ctx == NULL) {
            errno = ENOMEM;
            return 0;
        }
        memset(*ctx, 0, sizeof(**ctx));

        strcpy((*ctx)->filespec, directory);
        strcat((*ctx)->filespec, "*.*;");

/* Arrange 32-bit pointer to (copied) string storage, if needed. */
#if __INITIAL_POINTER_SIZE == 64
# define CTX_FILESPEC ctx_filespec_32p
        /* Copy the file name to storage with a 32-bit pointer. */
        ctx_filespec_32p = ctx_filespec_32;
        strcpy(ctx_filespec_32p, (*ctx)->filespec);
#else                           /* __INITIAL_POINTER_SIZE == 64 */
# define CTX_FILESPEC (*ctx)->filespec
#endif                          /* __INITIAL_POINTER_SIZE == 64 [else] */

        (*ctx)->filespec_dsc.dsc$w_length = filespeclen;
        (*ctx)->filespec_dsc.dsc$b_dtype = DSC$K_DTYPE_T;
        (*ctx)->filespec_dsc.dsc$b_class = DSC$K_CLASS_S;
        (*ctx)->filespec_dsc.dsc$a_pointer = CTX_FILESPEC;
    }

    (*ctx)->result_dsc.dsc$w_length = 0;
    (*ctx)->result_dsc.dsc$b_dtype = DSC$K_DTYPE_T;
    (*ctx)->result_dsc.dsc$b_class = DSC$K_CLASS_D;
    (*ctx)->result_dsc.dsc$a_pointer = 0;

    status = lib$find_file(&(*ctx)->filespec_dsc, &(*ctx)->result_dsc,
                           &(*ctx)->VMS_context, 0, 0, 0, &flags);

    if (status == RMS$_NMF) {
        errno = 0;
        vaxc$errno = status;
        return NULL;
    }

    if (!$VMS_STATUS_SUCCESS(status)) {
        errno = EVMSERR;
        vaxc$errno = status;
        return NULL;
    }

    /*
     * Quick, cheap and dirty way to discard any device and directory, since
     * we only want file names
     */
    l = (*ctx)->result_dsc.dsc$w_length;
    p = (*ctx)->result_dsc.dsc$a_pointer;
    r = p;
    for (; *p; p++) {
        if (*p == '^' && p[1] != '\0') { /* Take care of ODS-5 escapes */
            p++;
        } else if (*p == ':' || *p == '>' || *p == ']') {
            l -= p + 1 - r;
            r = p + 1;
        } else if (*p == ';') {
            l = p - r;
            break;
        }
    }

    strncpy((*ctx)->result, r, l);
    (*ctx)->result[l] = '\0';
    str$free1_dx(&(*ctx)->result_dsc);

    return (*ctx)->result;
}

int LP_find_file_end(LP_DIR_CTX **ctx)
{
    if (ctx != NULL && *ctx != NULL) {
        int status = lib$find_file_end(&(*ctx)->VMS_context);

        free(*ctx);

        if (!$VMS_STATUS_SUCCESS(status)) {
            errno = EVMSERR;
            vaxc$errno = status;
            return 0;
        }
        return 1;
    }
    errno = EINVAL;
    return 0;
}
