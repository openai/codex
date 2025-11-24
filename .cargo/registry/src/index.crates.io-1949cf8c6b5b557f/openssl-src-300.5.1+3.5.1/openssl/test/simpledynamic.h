/*
 * Copyright 2016-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_TEST_SIMPLEDYNAMIC_H
# define OSSL_TEST_SIMPLEDYNAMIC_H

# include "crypto/dso_conf.h"

# if defined(DSO_DLFCN) || defined(DSO_VMS)

#  include <dlfcn.h>

#  define SD_INIT       NULL
#  ifdef DSO_VMS
#   define SD_SHLIB     0
#   define SD_MODULE    0
#  else
#   define SD_SHLIB     (RTLD_GLOBAL|RTLD_LAZY)
#   define SD_MODULE    (RTLD_LOCAL|RTLD_NOW)
#  endif

typedef void *SD;
typedef void *SD_SYM;

# elif defined(DSO_WIN32)

#  include <windows.h>

#  define SD_INIT       0
#  define SD_SHLIB      0
#  define SD_MODULE     0

typedef HINSTANCE SD;
typedef void *SD_SYM;

# endif

# if defined(DSO_DLFCN) || defined(DSO_WIN32) || defined(DSO_VMS)
int sd_load(const char *filename, SD *sd, int type);
int sd_sym(SD sd, const char *symname, SD_SYM *sym);
int sd_close(SD lib);
const char *sd_error(void);
# endif

#endif
