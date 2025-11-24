/*
 * Copyright 2020-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <string.h>
#include <stdlib.h>

#include "internal/nelem.h"

#include <openssl/pkcs12.h>
#include <openssl/x509.h>
#include <openssl/x509v3.h>
#include <openssl/pem.h>

#include "../testutil.h"


/* -------------------------------------------------------------------------
 * PKCS#12 Test structures
 */

/* Holds a set of Attributes */
typedef struct pkcs12_attr {
    char *oid;
    char *value;
} PKCS12_ATTR;


/* Holds encryption parameters */
typedef struct pkcs12_enc {
    int         nid;
    const char *pass;
    int         iter;
} PKCS12_ENC;

/* Set of variables required for constructing the PKCS#12 structure */
typedef struct pkcs12_builder {
    const char *filename;
    int success;
    BIO *p12bio;
    STACK_OF(PKCS7) *safes;
    int safe_idx;
    STACK_OF(PKCS12_SAFEBAG) *bags;
    int bag_idx;
} PKCS12_BUILDER;


/* -------------------------------------------------------------------------
 * PKCS#12 Test function declarations
 */

/* Global settings */
void PKCS12_helper_set_write_files(int enable);
void PKCS12_helper_set_legacy(int enable);
void PKCS12_helper_set_libctx(OSSL_LIB_CTX *libctx);
void PKCS12_helper_set_propq(const char *propq);

/* Allocate and initialise a PKCS#12 builder object */
PKCS12_BUILDER *new_pkcs12_builder(const char *filename);

/* Finalise and free the PKCS#12 builder object, returning the success/fail flag */
int end_pkcs12_builder(PKCS12_BUILDER *pb);

/* Encode/build functions */
void start_pkcs12(PKCS12_BUILDER *pb);
void end_pkcs12(PKCS12_BUILDER *pb);
void end_pkcs12_with_mac(PKCS12_BUILDER *pb, const PKCS12_ENC *mac);

void start_contentinfo(PKCS12_BUILDER *pb);
void end_contentinfo(PKCS12_BUILDER *pb);
void end_contentinfo_encrypted(PKCS12_BUILDER *pb, const PKCS12_ENC *enc);

void add_certbag(PKCS12_BUILDER *pb, const unsigned char *bytes, int len,
                 const PKCS12_ATTR *attrs);
void add_keybag(PKCS12_BUILDER *pb, const unsigned char *bytes, int len,
                const PKCS12_ATTR *attrs, const PKCS12_ENC *enc);
void add_secretbag(PKCS12_BUILDER *pb, int secret_nid, const char *secret,
                   const PKCS12_ATTR *attrs);
void add_extra_attr(PKCS12_BUILDER *pb);

/* Decode/check functions */
void start_check_pkcs12(PKCS12_BUILDER *pb);
void start_check_pkcs12_with_mac(PKCS12_BUILDER *pb, const PKCS12_ENC *mac);
void start_check_pkcs12_file(PKCS12_BUILDER *pb);
void start_check_pkcs12_file_with_mac(PKCS12_BUILDER *pb, const PKCS12_ENC *mac);
void end_check_pkcs12(PKCS12_BUILDER *pb);

void start_check_contentinfo(PKCS12_BUILDER *pb);
void start_check_contentinfo_encrypted(PKCS12_BUILDER *pb, const PKCS12_ENC *enc);
void end_check_contentinfo(PKCS12_BUILDER *pb);

void check_certbag(PKCS12_BUILDER *pb, const unsigned char *bytes, int len,
                   const PKCS12_ATTR *attrs);
void check_keybag(PKCS12_BUILDER *pb, const unsigned char *bytes, int len,
                  const PKCS12_ATTR *attrs, const PKCS12_ENC *enc);
void check_secretbag(PKCS12_BUILDER *pb, int secret_nid, const char *secret,
                     const PKCS12_ATTR *attrs);

