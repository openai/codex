/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/e_os.h"
#include "internal/cryptlib.h"
#include "internal/thread_once.h"
#include <openssl/crypto.h>
#include <openssl/x509.h>

#if defined(_WIN32)

static char x509_private_dir[MAX_PATH + 1]; 
static char *x509_private_dirptr = NULL;

static char x509_cert_area[MAX_PATH + 1];
static char *x509_cert_areaptr = NULL;

static char x509_cert_dir[MAX_PATH + 1];
static char *x509_cert_dirptr = NULL;

static char x509_cert_file[MAX_PATH + 1];
static char *x509_cert_fileptr = NULL;

static void get_windows_default_path(char *pathname, const char *suffix)
{
    char *ossldir;

    ossldir = ossl_get_openssldir();

    if (ossldir == NULL)
        return;

    OPENSSL_strlcpy(pathname, ossldir, MAX_PATH - 1);
    if (MAX_PATH - strlen(pathname) > strlen(suffix))
        strcat(pathname, suffix);
}

static CRYPTO_ONCE openssldir_setup_init = CRYPTO_ONCE_STATIC_INIT;
DEFINE_RUN_ONCE_STATIC(do_openssldir_setup)
{
    get_windows_default_path(x509_private_dir, "\\private");
    if (strlen(x509_private_dir) > 0)
        x509_private_dirptr = x509_private_dir;

    get_windows_default_path(x509_cert_area, "\\");
    if (strlen(x509_cert_area) > 0)
        x509_cert_areaptr = x509_cert_area;

    get_windows_default_path(x509_cert_dir, "\\certs");
    if (strlen(x509_cert_dir) > 0)
        x509_cert_dirptr = x509_cert_dir;

    get_windows_default_path(x509_cert_file, "\\cert.pem");
    if (strlen(x509_cert_file) > 0)
        x509_cert_fileptr = x509_cert_file;

    return 1;
}
#endif

const char *X509_get_default_private_dir(void)
{
#if defined (_WIN32)
    RUN_ONCE(&openssldir_setup_init, do_openssldir_setup);
    return x509_private_dirptr;
#else
    return X509_PRIVATE_DIR;
#endif
}

const char *X509_get_default_cert_area(void)
{
#if defined (_WIN32)
    RUN_ONCE(&openssldir_setup_init, do_openssldir_setup);
    return x509_cert_areaptr;
#else
    return X509_CERT_AREA;
#endif
}

const char *X509_get_default_cert_dir(void)
{
#if defined (_WIN32)
    RUN_ONCE(&openssldir_setup_init, do_openssldir_setup);
    return x509_cert_dirptr;
#else
    return X509_CERT_DIR;
#endif
}

const char *X509_get_default_cert_file(void)
{
#if defined (_WIN32)
    RUN_ONCE(&openssldir_setup_init, do_openssldir_setup);
    return x509_cert_fileptr;
#else
    return X509_CERT_FILE;
#endif
}

const char *X509_get_default_cert_dir_env(void)
{
    return X509_CERT_DIR_EVP;
}

const char *X509_get_default_cert_file_env(void)
{
    return X509_CERT_FILE_EVP;
}
