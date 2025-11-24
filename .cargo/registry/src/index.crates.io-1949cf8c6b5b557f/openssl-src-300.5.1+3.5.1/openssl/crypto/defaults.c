/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <openssl/opensslv.h>
#include "internal/thread_once.h"
#include "internal/cryptlib.h"
#include "internal/e_os.h"

#if defined(_WIN32) && defined(OSSL_WINCTX)

# define TOSTR(x) #x
# define MAKESTR(x) TOSTR(x)
# define NOQUOTE(x) x
# if defined(OSSL_WINCTX)
#  define REGISTRY_KEY "SOFTWARE\\WOW6432Node\\OpenSSL" "-" MAKESTR(OPENSSL_VERSION_MAJOR) "." MAKESTR(OPENSSL_VERSION_MINOR) "-" MAKESTR(OSSL_WINCTX)
# endif

/**
 * @brief The directory where OpenSSL is installed.
 */
static char openssldir[MAX_PATH + 1];

/**
 * @brief The pointer to the openssldir buffer
 */
static char *openssldirptr = NULL;

/**
 * @brief The directory where OpenSSL engines are located.
 */

static char enginesdir[MAX_PATH + 1];

/**
 * @brief The pointer to the enginesdir buffer
 */
static char *enginesdirptr = NULL;

/**
 * @brief The directory where OpenSSL modules are located.
 */
static char modulesdir[MAX_PATH + 1];

/**
 * @brief The pointer to the modulesdir buffer
 */
static char *modulesdirptr = NULL;

/**
 * @brief Get the list of Windows registry directories.
 *
 * This function retrieves a list of Windows registry directories.
 *
 * @return A pointer to a char array containing the registry directories.
 */
static char *get_windows_regdirs(char *dst, DWORD dstsizebytes, LPCWSTR valuename)
{
    char *retval = NULL;
# ifdef REGISTRY_KEY
    DWORD keysizebytes;
    DWORD ktype;
    HKEY hkey;
    LSTATUS ret;
    DWORD index = 0;
    LPCWSTR tempstr = NULL;

    ret = RegOpenKeyEx(HKEY_LOCAL_MACHINE,
                       TEXT(REGISTRY_KEY), KEY_WOW64_32KEY,
                       KEY_QUERY_VALUE, &hkey);
    if (ret != ERROR_SUCCESS)
        goto out;

    /* Always use wide call so we can avoid extra encoding conversions on the output */
    ret = RegQueryValueExW(hkey, valuename, NULL, &ktype, NULL,
                           &keysizebytes);
    if (ret != ERROR_SUCCESS)
        goto out;
    if (ktype != REG_EXPAND_SZ && ktype != REG_SZ)
        goto out;
    if (keysizebytes > MAX_PATH * sizeof(WCHAR))
        goto out;

    /*
     * RegQueryValueExW does not guarantee the buffer is null terminated,
     * so we make space for one in the allocation
     */
    tempstr = OPENSSL_zalloc(keysizebytes + sizeof(WCHAR));

    if (tempstr == NULL)
        goto out;

    if (RegQueryValueExW(hkey, valuename,
                         NULL, &ktype, (LPBYTE)tempstr, &keysizebytes) != ERROR_SUCCESS)
        goto out;

    if (!WideCharToMultiByte(CP_UTF8, 0, tempstr, -1, dst, dstsizebytes,
                             NULL, NULL))
        goto out;

    retval = dst;
out:
    OPENSSL_free(tempstr);
    RegCloseKey(hkey);
# endif /* REGISTRY_KEY */
    return retval;
}

static CRYPTO_ONCE defaults_setup_init = CRYPTO_ONCE_STATIC_INIT;

/**
 * @brief Function to setup default values to run once.
 * Only used in Windows environments.  Does run time initialization
 * of openssldir/modulesdir/enginesdir from the registry
 */
DEFINE_RUN_ONCE_STATIC(do_defaults_setup)
{
    get_windows_regdirs(openssldir, sizeof(openssldir), L"OPENSSLDIR");
    get_windows_regdirs(enginesdir, sizeof(enginesdir), L"ENGINESDIR");
    get_windows_regdirs(modulesdir, sizeof(modulesdir), L"MODULESDIR");

    /*
     * Set our pointers only if the directories are fetched properly
     */
    if (strlen(openssldir) > 0)
        openssldirptr = openssldir;

    if (strlen(enginesdir) > 0)
        enginesdirptr = enginesdir;

    if (strlen(modulesdir) > 0)
        modulesdirptr = modulesdir;

    return 1;
}
#endif /* defined(_WIN32) && defined(OSSL_WINCTX) */

/**
 * @brief Get the directory where OpenSSL is installed.
 *
 * @return A pointer to a string containing the OpenSSL directory path.
 */
const char *ossl_get_openssldir(void)
{
#if defined(_WIN32) && defined (OSSL_WINCTX)
    if (!RUN_ONCE(&defaults_setup_init, do_defaults_setup))
        return NULL;
    return (const char *)openssldirptr;
# else
    return OPENSSLDIR;
#endif
}

/**
 * @brief Get the directory where OpenSSL engines are located.
 *
 * @return A pointer to a string containing the engines directory path.
 */
const char *ossl_get_enginesdir(void)
{
#if defined(_WIN32) && defined (OSSL_WINCTX)
    if (!RUN_ONCE(&defaults_setup_init, do_defaults_setup))
        return NULL;
    return (const char *)enginesdirptr;
#else
    return ENGINESDIR;
#endif
}

/**
 * @brief Get the directory where OpenSSL modules are located.
 *
 * @return A pointer to a string containing the modules directory path.
 */
const char *ossl_get_modulesdir(void)
{
#if defined(_WIN32) && defined(OSSL_WINCTX)
    if (!RUN_ONCE(&defaults_setup_init, do_defaults_setup))
        return NULL;
    return (const char *)modulesdirptr;
#else
    return MODULESDIR;
#endif
}

/**
 * @brief Get the build time defined windows installer context
 *
 * @return A char pointer to a string representing the windows install context
 */
const char *ossl_get_wininstallcontext(void)
{
#if defined(_WIN32) && defined (OSSL_WINCTX)
    return MAKESTR(OSSL_WINCTX);
#else
    return "Undefined";
#endif
}
