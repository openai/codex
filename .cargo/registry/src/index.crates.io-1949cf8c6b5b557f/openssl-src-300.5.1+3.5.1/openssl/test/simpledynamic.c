/*
 * Copyright 2016-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <stdlib.h>              /* For NULL */
#include <openssl/macros.h>      /* For NON_EMPTY_TRANSLATION_UNIT */
#include <openssl/e_os2.h>
#include "simpledynamic.h"

#if defined(DSO_DLFCN) || defined(DSO_VMS)

int sd_load(const char *filename, SD *lib, int type)
{
    int dl_flags = type;
#ifdef _AIX
    if (filename[strlen(filename) - 1] == ')')
        dl_flags |= RTLD_MEMBER;
#endif
    *lib = dlopen(filename, dl_flags);
    return *lib == NULL ? 0 : 1;
}

int sd_sym(SD lib, const char *symname, SD_SYM *sym)
{
    *sym = dlsym(lib, symname);
    return *sym != NULL;
}

int sd_close(SD lib)
{
    return dlclose(lib) != 0 ? 0 : 1;
}

const char *sd_error(void)
{
    return dlerror();
}

#elif defined(DSO_WIN32)

int sd_load(const char *filename, SD *lib, ossl_unused int type)
{
    *lib = LoadLibraryA(filename);
    return *lib == NULL ? 0 : 1;
}

int sd_sym(SD lib, const char *symname, SD_SYM *sym)
{
    *sym = (SD_SYM)GetProcAddress(lib, symname);
    return *sym != NULL;
}

int sd_close(SD lib)
{
    return FreeLibrary(lib) == 0 ? 0 : 1;
}

const char *sd_error(void)
{
    static char buffer[255];

    buffer[0] = '\0';
    FormatMessageA(FORMAT_MESSAGE_FROM_SYSTEM, NULL, GetLastError(), 0,
                   buffer, sizeof(buffer), NULL);
    return buffer;
}

#else

NON_EMPTY_TRANSLATION_UNIT

#endif
