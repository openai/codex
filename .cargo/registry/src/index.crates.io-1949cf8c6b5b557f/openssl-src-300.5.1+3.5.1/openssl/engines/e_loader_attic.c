/*
 * Copyright 2016-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* THIS ENGINE IS FOR TESTING PURPOSES ONLY. */

/* This file has quite some overlap with providers/implementations/storemgmt/file_store.c */

/* We need to use some engine deprecated APIs */
#define OPENSSL_SUPPRESS_DEPRECATED

#include "internal/e_os.h" /* for stat */
#include <string.h>
#include <sys/stat.h>
#include <ctype.h>
#include <assert.h>

#include <openssl/bio.h>
#include <openssl/dsa.h>         /* For d2i_DSAPrivateKey */
#include <openssl/err.h>
#include <openssl/evp.h>
#include <openssl/pem.h>
#include <openssl/pkcs12.h>      /* For the PKCS8 stuff o.O */
#include <openssl/rsa.h>         /* For d2i_RSAPrivateKey */
#include <openssl/safestack.h>
#include <openssl/store.h>
#include <openssl/ui.h>
#include <openssl/engine.h>
#include <openssl/x509.h>        /* For the PKCS8 stuff o.O */
#include "internal/asn1.h"       /* For asn1_d2i_read_bio */
#include "internal/o_dir.h"
#include "internal/cryptlib.h"
#include "crypto/ctype.h"        /* For ossl_isdigit */
#include "crypto/pem.h"          /* For PVK and "blob" PEM headers */

#include "e_loader_attic_err.c"

DEFINE_STACK_OF(OSSL_STORE_INFO)

#ifndef S_ISDIR
# define S_ISDIR(a) (((a) & S_IFMT) == S_IFDIR)
#endif

/*-
 *  Password prompting
 *  ------------------
 */

static char *file_get_pass(const UI_METHOD *ui_method, char *pass,
                           size_t maxsize, const char *desc, const char *info,
                           void *data)
{
    UI *ui = UI_new();
    char *prompt = NULL;

    if (ui == NULL) {
        ATTICerr(0, ERR_R_UI_LIB);
        return NULL;
    }

    if (ui_method != NULL)
        UI_set_method(ui, ui_method);
    UI_add_user_data(ui, data);

    if ((prompt = UI_construct_prompt(ui, desc, info)) == NULL) {
        ATTICerr(0, ERR_R_UI_LIB);
        pass = NULL;
    } else if (UI_add_input_string(ui, prompt, UI_INPUT_FLAG_DEFAULT_PWD,
                                    pass, 0, maxsize - 1) <= 0) {
        ATTICerr(0, ERR_R_UI_LIB);
        pass = NULL;
    } else {
        switch (UI_process(ui)) {
        case -2:
            ATTICerr(0, ATTIC_R_UI_PROCESS_INTERRUPTED_OR_CANCELLED);
            pass = NULL;
            break;
        case -1:
            ATTICerr(0, ERR_R_UI_LIB);
            pass = NULL;
            break;
        default:
            break;
        }
    }

    OPENSSL_free(prompt);
    UI_free(ui);
    return pass;
}

struct pem_pass_data {
    const UI_METHOD *ui_method;
    void *data;
    const char *prompt_desc;
    const char *prompt_info;
};

static int file_fill_pem_pass_data(struct pem_pass_data *pass_data,
                                   const char *desc, const char *info,
                                   const UI_METHOD *ui_method, void *ui_data)
{
    if (pass_data == NULL)
        return 0;
    pass_data->ui_method = ui_method;
    pass_data->data = ui_data;
    pass_data->prompt_desc = desc;
    pass_data->prompt_info = info;
    return 1;
}

/* This is used anywhere a pem_password_cb is needed */
static int file_get_pem_pass(char *buf, int num, int w, void *data)
{
    struct pem_pass_data *pass_data = data;
    char *pass = file_get_pass(pass_data->ui_method, buf, num,
                               pass_data->prompt_desc, pass_data->prompt_info,
                               pass_data->data);

    return pass == NULL ? 0 : strlen(pass);
}

/*
 * Check if |str| ends with |suffix| preceded by a space, and if it does,
 * return the index of that space.  If there is no such suffix in |str|,
 * return -1.
 * For |str| == "FOO BAR" and |suffix| == "BAR", the returned value is 3.
 */
static int check_suffix(const char *str, const char *suffix)
{
    int str_len = strlen(str);
    int suffix_len = strlen(suffix) + 1;
    const char *p = NULL;

    if (suffix_len >= str_len)
        return -1;
    p = str + str_len - suffix_len;
    if (*p != ' '
        || strcmp(p + 1, suffix) != 0)
        return -1;
    return p - str;
}

/*
 * EMBEDDED is a special type of OSSL_STORE_INFO, specially for the file
 * handlers, so we define it internally.  This uses the possibility to
 * create an OSSL_STORE_INFO with a generic data pointer and arbitrary
 * type number.
 *
 * This is used by a FILE_HANDLER's try_decode function to signal that it
 * has decoded the incoming blob into a new blob, and that the attempted
 * decoding should be immediately restarted with the new blob, using the
 * new PEM name.
 */
/* Negative numbers are never used for public OSSL_STORE_INFO types */
#define STORE_INFO_EMBEDDED       -1

/* This is the embedded data */
struct embedded_st {
    BUF_MEM *blob;
    char *pem_name;
};

/* Helper functions */
static struct embedded_st *get0_EMBEDDED(OSSL_STORE_INFO *info)
{
    return OSSL_STORE_INFO_get0_data(STORE_INFO_EMBEDDED, info);
}

static void store_info_free(OSSL_STORE_INFO *info)
{
    struct embedded_st *data;

    if (info != NULL && (data = get0_EMBEDDED(info)) != NULL) {
        BUF_MEM_free(data->blob);
        OPENSSL_free(data->pem_name);
        OPENSSL_free(data);
    }
    OSSL_STORE_INFO_free(info);
}

static OSSL_STORE_INFO *new_EMBEDDED(const char *new_pem_name,
                                     BUF_MEM *embedded)
{
    OSSL_STORE_INFO *info = NULL;
    struct embedded_st *data = NULL;

    if ((data = OPENSSL_zalloc(sizeof(*data))) == NULL)
        return NULL;
    if ((info = OSSL_STORE_INFO_new(STORE_INFO_EMBEDDED, data)) == NULL) {
        ATTICerr(0, ERR_R_OSSL_STORE_LIB);
        OPENSSL_free(data);
        return NULL;
    }

    data->blob = embedded;
    data->pem_name =
        new_pem_name == NULL ? NULL : OPENSSL_strdup(new_pem_name);

    if (new_pem_name != NULL && data->pem_name == NULL) {
        store_info_free(info);
        info = NULL;
    }

    return info;
}

/*-
 *  The file scheme decoders
 *  ------------------------
 *
 *  Each possible data type has its own decoder, which either operates
 *  through a given PEM name, or attempts to decode to see if the blob
 *  it's given is decodable for its data type.  The assumption is that
 *  only the correct data type will match the content.
 */

/*-
 * The try_decode function is called to check if the blob of data can
 * be used by this handler, and if it can, decodes it into a supported
 * OpenSSL type and returns an OSSL_STORE_INFO with the decoded data.
 * Input:
 *    pem_name:     If this blob comes from a PEM file, this holds
 *                  the PEM name.  If it comes from another type of
 *                  file, this is NULL.
 *    pem_header:   If this blob comes from a PEM file, this holds
 *                  the PEM headers.  If it comes from another type of
 *                  file, this is NULL.
 *    blob:         The blob of data to match with what this handler
 *                  can use.
 *    len:          The length of the blob.
 *    handler_ctx:  For a handler marked repeatable, this pointer can
 *                  be used to create a context for the handler.  IT IS
 *                  THE HANDLER'S RESPONSIBILITY TO CREATE AND DESTROY
 *                  THIS CONTEXT APPROPRIATELY, i.e. create on first call
 *                  and destroy when about to return NULL.
 *    matchcount:   A pointer to an int to count matches for this data.
 *                  Usually becomes 0 (no match) or 1 (match!), but may
 *                  be higher in the (unlikely) event that the data matches
 *                  more than one possibility.  The int will always be
 *                  zero when the function is called.
 *    ui_method:    Application UI method for getting a password, pin
 *                  or any other interactive data.
 *    ui_data:      Application data to be passed to ui_method when
 *                  it's called.
 *    libctx:       The library context to be used if applicable
 *    propq:        The property query string for any algorithm fetches
 * Output:
 *    an OSSL_STORE_INFO
 */
typedef OSSL_STORE_INFO *(*file_try_decode_fn)(const char *pem_name,
                                               const char *pem_header,
                                               const unsigned char *blob,
                                               size_t len, void **handler_ctx,
                                               int *matchcount,
                                               const UI_METHOD *ui_method,
                                               void *ui_data, const char *uri,
                                               OSSL_LIB_CTX *libctx,
                                               const char *propq);
/*
 * The eof function should return 1 if there's no more data to be found
 * with the handler_ctx, otherwise 0.  This is only used when the handler is
 * marked repeatable.
 */
typedef int (*file_eof_fn)(void *handler_ctx);
/*
 * The destroy_ctx function is used to destroy the handler_ctx that was
 * initiated by a repeatable try_decode function.  This is only used when
 * the handler is marked repeatable.
 */
typedef void (*file_destroy_ctx_fn)(void **handler_ctx);

typedef struct file_handler_st {
    const char *name;
    file_try_decode_fn try_decode;
    file_eof_fn eof;
    file_destroy_ctx_fn destroy_ctx;

    /* flags */
    int repeatable;
} FILE_HANDLER;

/*
 * PKCS#12 decoder.  It operates by decoding all of the blob content,
 * extracting all the interesting data from it and storing them internally,
 * then serving them one piece at a time.
 */
static OSSL_STORE_INFO *try_decode_PKCS12(const char *pem_name,
                                          const char *pem_header,
                                          const unsigned char *blob,
                                          size_t len, void **pctx,
                                          int *matchcount,
                                          const UI_METHOD *ui_method,
                                          void *ui_data, const char *uri,
                                          OSSL_LIB_CTX *libctx,
                                          const char *propq)
{
    OSSL_STORE_INFO *store_info = NULL;
    STACK_OF(OSSL_STORE_INFO) *ctx = *pctx;

    if (ctx == NULL) {
        /* Initial parsing */
        PKCS12 *p12;

        if (pem_name != NULL)
            /* No match, there is no PEM PKCS12 tag */
            return NULL;

        if ((p12 = d2i_PKCS12(NULL, &blob, len)) != NULL) {
            char *pass = NULL;
            char tpass[PEM_BUFSIZE];
            EVP_PKEY *pkey = NULL;
            X509 *cert = NULL;
            STACK_OF(X509) *chain = NULL;

            *matchcount = 1;

            if (!PKCS12_mac_present(p12)
                || PKCS12_verify_mac(p12, "", 0)
                || PKCS12_verify_mac(p12, NULL, 0)) {
                pass = "";
            } else {
                if ((pass = file_get_pass(ui_method, tpass, PEM_BUFSIZE,
                                          "PKCS12 import", uri,
                                          ui_data)) == NULL) {
                    ATTICerr(0, ATTIC_R_PASSPHRASE_CALLBACK_ERROR);
                    goto p12_end;
                }
                if (!PKCS12_verify_mac(p12, pass, strlen(pass))) {
                    ATTICerr(0, ATTIC_R_ERROR_VERIFYING_PKCS12_MAC);
                    goto p12_end;
                }
            }

            if (PKCS12_parse(p12, pass, &pkey, &cert, &chain)) {
                OSSL_STORE_INFO *osi_pkey = NULL;
                OSSL_STORE_INFO *osi_cert = NULL;
                OSSL_STORE_INFO *osi_ca = NULL;
                int ok = 1;

                if ((ctx = sk_OSSL_STORE_INFO_new_null()) != NULL) {
                    if (pkey != NULL) {
                        if ((osi_pkey = OSSL_STORE_INFO_new_PKEY(pkey)) != NULL
                            /* clearing pkey here avoids case distinctions */
                            && (pkey = NULL) == NULL
                            && sk_OSSL_STORE_INFO_push(ctx, osi_pkey) != 0)
                            osi_pkey = NULL;
                        else
                            ok = 0;
                    }
                    if (ok && cert != NULL) {
                        if ((osi_cert = OSSL_STORE_INFO_new_CERT(cert)) != NULL
                            /* clearing cert here avoids case distinctions */
                            && (cert = NULL) == NULL
                            && sk_OSSL_STORE_INFO_push(ctx, osi_cert) != 0)
                            osi_cert = NULL;
                        else
                            ok = 0;
                    }
                    while (ok && sk_X509_num(chain) > 0) {
                        X509 *ca = sk_X509_value(chain, 0);

                        if ((osi_ca = OSSL_STORE_INFO_new_CERT(ca)) != NULL
                            && sk_X509_shift(chain) != NULL
                            && sk_OSSL_STORE_INFO_push(ctx, osi_ca) != 0)
                            osi_ca = NULL;
                        else
                            ok = 0;
                    }
                }
                EVP_PKEY_free(pkey);
                X509_free(cert);
                OSSL_STACK_OF_X509_free(chain);
                store_info_free(osi_pkey);
                store_info_free(osi_cert);
                store_info_free(osi_ca);
                if (!ok) {
                    sk_OSSL_STORE_INFO_pop_free(ctx, store_info_free);
                    ctx = NULL;
                }
                *pctx = ctx;
            }
        }
     p12_end:
        PKCS12_free(p12);
        if (ctx == NULL)
            return NULL;
    }

    *matchcount = 1;
    store_info = sk_OSSL_STORE_INFO_shift(ctx);
    return store_info;
}

static int eof_PKCS12(void *ctx_)
{
    STACK_OF(OSSL_STORE_INFO) *ctx = ctx_;

    return ctx == NULL || sk_OSSL_STORE_INFO_num(ctx) == 0;
}

static void destroy_ctx_PKCS12(void **pctx)
{
    STACK_OF(OSSL_STORE_INFO) *ctx = *pctx;

    sk_OSSL_STORE_INFO_pop_free(ctx, store_info_free);
    *pctx = NULL;
}

static FILE_HANDLER PKCS12_handler = {
    "PKCS12",
    try_decode_PKCS12,
    eof_PKCS12,
    destroy_ctx_PKCS12,
    1 /* repeatable */
};

/*
 * Encrypted PKCS#8 decoder.  It operates by just decrypting the given blob
 * into a new blob, which is returned as an EMBEDDED STORE_INFO.  The whole
 * decoding process will then start over with the new blob.
 */
static OSSL_STORE_INFO *try_decode_PKCS8Encrypted(const char *pem_name,
                                                  const char *pem_header,
                                                  const unsigned char *blob,
                                                  size_t len, void **pctx,
                                                  int *matchcount,
                                                  const UI_METHOD *ui_method,
                                                  void *ui_data,
                                                  const char *uri,
                                                  OSSL_LIB_CTX *libctx,
                                                  const char *propq)
{
    X509_SIG *p8 = NULL;
    char kbuf[PEM_BUFSIZE];
    char *pass = NULL;
    const X509_ALGOR *dalg = NULL;
    const ASN1_OCTET_STRING *doct = NULL;
    OSSL_STORE_INFO *store_info = NULL;
    BUF_MEM *mem = NULL;
    unsigned char *new_data = NULL;
    int new_data_len;

    if (pem_name != NULL) {
        if (strcmp(pem_name, PEM_STRING_PKCS8) != 0)
            return NULL;
        *matchcount = 1;
    }

    if ((p8 = d2i_X509_SIG(NULL, &blob, len)) == NULL)
        return NULL;

    *matchcount = 1;

    if ((mem = BUF_MEM_new()) == NULL) {
        ATTICerr(0, ERR_R_BUF_LIB);
        goto nop8;
    }

    if ((pass = file_get_pass(ui_method, kbuf, PEM_BUFSIZE,
                              "PKCS8 decrypt pass phrase", uri,
                              ui_data)) == NULL) {
        ATTICerr(0, ATTIC_R_BAD_PASSWORD_READ);
        goto nop8;
    }

    X509_SIG_get0(p8, &dalg, &doct);
    if (!PKCS12_pbe_crypt(dalg, pass, strlen(pass), doct->data, doct->length,
                          &new_data, &new_data_len, 0))
        goto nop8;

    mem->data = (char *)new_data;
    mem->max = mem->length = (size_t)new_data_len;
    X509_SIG_free(p8);
    p8 = NULL;

    store_info = new_EMBEDDED(PEM_STRING_PKCS8INF, mem);
    if (store_info == NULL) {
        ATTICerr(0, ERR_R_OSSL_STORE_LIB);
        goto nop8;
    }

    return store_info;
 nop8:
    X509_SIG_free(p8);
    BUF_MEM_free(mem);
    return NULL;
}

static FILE_HANDLER PKCS8Encrypted_handler = {
    "PKCS8Encrypted",
    try_decode_PKCS8Encrypted
};

/*
 * Private key decoder.  Decodes all sorts of private keys, both PKCS#8
 * encoded ones and old style PEM ones (with the key type is encoded into
 * the PEM name).
 */
static OSSL_STORE_INFO *try_decode_PrivateKey(const char *pem_name,
                                              const char *pem_header,
                                              const unsigned char *blob,
                                              size_t len, void **pctx,
                                              int *matchcount,
                                              const UI_METHOD *ui_method,
                                              void *ui_data, const char *uri,
                                              OSSL_LIB_CTX *libctx,
                                              const char *propq)
{
    OSSL_STORE_INFO *store_info = NULL;
    EVP_PKEY *pkey = NULL;
    const EVP_PKEY_ASN1_METHOD *ameth = NULL;

    if (pem_name != NULL) {
        if (strcmp(pem_name, PEM_STRING_PKCS8INF) == 0) {
            PKCS8_PRIV_KEY_INFO *p8inf =
                d2i_PKCS8_PRIV_KEY_INFO(NULL, &blob, len);

            *matchcount = 1;
            if (p8inf != NULL)
                pkey = EVP_PKCS82PKEY_ex(p8inf, libctx, propq);
            PKCS8_PRIV_KEY_INFO_free(p8inf);
        } else {
            int slen;
            int pkey_id;

            if ((slen = check_suffix(pem_name, "PRIVATE KEY")) > 0
                && (ameth = EVP_PKEY_asn1_find_str(NULL, pem_name,
                                                   slen)) != NULL
                && EVP_PKEY_asn1_get0_info(&pkey_id, NULL, NULL, NULL, NULL,
                                           ameth)) {
                *matchcount = 1;
                pkey = d2i_PrivateKey_ex(pkey_id, NULL, &blob, len,
                                         libctx, propq);
            }
        }
    } else {
        int i;
#ifndef OPENSSL_NO_ENGINE
        ENGINE *curengine = ENGINE_get_first();

        while (curengine != NULL) {
            ENGINE_PKEY_ASN1_METHS_PTR asn1meths =
                ENGINE_get_pkey_asn1_meths(curengine);

            if (asn1meths != NULL) {
                const int *nids = NULL;
                int nids_n = asn1meths(curengine, NULL, &nids, 0);

                for (i = 0; i < nids_n; i++) {
                    EVP_PKEY_ASN1_METHOD *ameth2 = NULL;
                    EVP_PKEY *tmp_pkey = NULL;
                    const unsigned char *tmp_blob = blob;
                    int pkey_id, pkey_flags;

                    if (!asn1meths(curengine, &ameth2, NULL, nids[i])
                        || !EVP_PKEY_asn1_get0_info(&pkey_id, NULL,
                                                    &pkey_flags, NULL, NULL,
                                                    ameth2)
                        || (pkey_flags & ASN1_PKEY_ALIAS) != 0)
                        continue;

                    ERR_set_mark(); /* prevent flooding error queue */
                    tmp_pkey = d2i_PrivateKey_ex(pkey_id, NULL,
                                                 &tmp_blob, len,
                                                 libctx, propq);
                    if (tmp_pkey != NULL) {
                        if (pkey != NULL)
                            EVP_PKEY_free(tmp_pkey);
                        else
                            pkey = tmp_pkey;
                        (*matchcount)++;
                    }
                    ERR_pop_to_mark();
                }
            }
            curengine = ENGINE_get_next(curengine);
        }
#endif

        for (i = 0; i < EVP_PKEY_asn1_get_count(); i++) {
            EVP_PKEY *tmp_pkey = NULL;
            const unsigned char *tmp_blob = blob;
            int pkey_id, pkey_flags;

            ameth = EVP_PKEY_asn1_get0(i);
            if (!EVP_PKEY_asn1_get0_info(&pkey_id, NULL, &pkey_flags, NULL,
                                         NULL, ameth)
                || (pkey_flags & ASN1_PKEY_ALIAS) != 0)
                continue;

            ERR_set_mark(); /* prevent flooding error queue */
            tmp_pkey = d2i_PrivateKey_ex(pkey_id, NULL, &tmp_blob, len,
                                         libctx, propq);
            if (tmp_pkey != NULL) {
                if (pkey != NULL)
                    EVP_PKEY_free(tmp_pkey);
                else
                    pkey = tmp_pkey;
                (*matchcount)++;
            }
            ERR_pop_to_mark();
        }

        if (*matchcount > 1) {
            EVP_PKEY_free(pkey);
            pkey = NULL;
        }
    }
    if (pkey == NULL)
        /* No match */
        return NULL;

    store_info = OSSL_STORE_INFO_new_PKEY(pkey);
    if (store_info == NULL)
        EVP_PKEY_free(pkey);

    return store_info;
}

static FILE_HANDLER PrivateKey_handler = {
    "PrivateKey",
    try_decode_PrivateKey
};

/*
 * Public key decoder.  Only supports SubjectPublicKeyInfo formatted keys.
 */
static OSSL_STORE_INFO *try_decode_PUBKEY(const char *pem_name,
                                          const char *pem_header,
                                          const unsigned char *blob,
                                          size_t len, void **pctx,
                                          int *matchcount,
                                          const UI_METHOD *ui_method,
                                          void *ui_data, const char *uri,
                                          OSSL_LIB_CTX *libctx,
                                          const char *propq)
{
    OSSL_STORE_INFO *store_info = NULL;
    EVP_PKEY *pkey = NULL;

    if (pem_name != NULL) {
        if (strcmp(pem_name, PEM_STRING_PUBLIC) != 0)
            /* No match */
            return NULL;
        *matchcount = 1;
    }

    if ((pkey = d2i_PUBKEY(NULL, &blob, len)) != NULL) {
        *matchcount = 1;
        store_info = OSSL_STORE_INFO_new_PUBKEY(pkey);
    }

    return store_info;
}

static FILE_HANDLER PUBKEY_handler = {
    "PUBKEY",
    try_decode_PUBKEY
};

/*
 * Key parameter decoder.
 */
static OSSL_STORE_INFO *try_decode_params(const char *pem_name,
                                          const char *pem_header,
                                          const unsigned char *blob,
                                          size_t len, void **pctx,
                                          int *matchcount,
                                          const UI_METHOD *ui_method,
                                          void *ui_data, const char *uri,
                                          OSSL_LIB_CTX *libctx,
                                          const char *propq)
{
    OSSL_STORE_INFO *store_info = NULL;
    EVP_PKEY *pkey = NULL;
    const EVP_PKEY_ASN1_METHOD *ameth = NULL;

    if (pem_name != NULL) {
        int slen;
        int pkey_id;

        if ((slen = check_suffix(pem_name, "PARAMETERS")) > 0
            && (ameth = EVP_PKEY_asn1_find_str(NULL, pem_name, slen)) != NULL
            && EVP_PKEY_asn1_get0_info(&pkey_id, NULL, NULL, NULL, NULL,
                                       ameth)) {
            *matchcount = 1;
            pkey = d2i_KeyParams(pkey_id, NULL, &blob, len);
        }
    } else {
        int i;

        for (i = 0; i < EVP_PKEY_asn1_get_count(); i++) {
            EVP_PKEY *tmp_pkey = NULL;
            const unsigned char *tmp_blob = blob;
            int pkey_id, pkey_flags;

            ameth = EVP_PKEY_asn1_get0(i);
            if (!EVP_PKEY_asn1_get0_info(&pkey_id, NULL, &pkey_flags, NULL,
                                         NULL, ameth)
                || (pkey_flags & ASN1_PKEY_ALIAS) != 0)
                continue;

            ERR_set_mark(); /* prevent flooding error queue */

            tmp_pkey = d2i_KeyParams(pkey_id, NULL, &tmp_blob, len);

            if (tmp_pkey != NULL) {
                if (pkey != NULL)
                    EVP_PKEY_free(tmp_pkey);
                else
                    pkey = tmp_pkey;
                (*matchcount)++;
            }
            ERR_pop_to_mark();
        }

        if (*matchcount > 1) {
            EVP_PKEY_free(pkey);
            pkey = NULL;
        }
    }
    if (pkey == NULL)
        /* No match */
        return NULL;

    store_info = OSSL_STORE_INFO_new_PARAMS(pkey);
    if (store_info == NULL)
        EVP_PKEY_free(pkey);

    return store_info;
}

static FILE_HANDLER params_handler = {
    "params",
    try_decode_params
};

/*
 * X.509 certificate decoder.
 */
static OSSL_STORE_INFO *try_decode_X509Certificate(const char *pem_name,
                                                   const char *pem_header,
                                                   const unsigned char *blob,
                                                   size_t len, void **pctx,
                                                   int *matchcount,
                                                   const UI_METHOD *ui_method,
                                                   void *ui_data,
                                                   const char *uri,
                                                   OSSL_LIB_CTX *libctx,
                                                   const char *propq)
{
    OSSL_STORE_INFO *store_info = NULL;
    X509 *cert = NULL;

    /*
     * In most cases, we can try to interpret the serialized data as a trusted
     * cert (X509 + X509_AUX) and fall back to reading it as a normal cert
     * (just X509), but if the PEM name specifically declares it as a trusted
     * cert, then no fallback should be engaged.  |ignore_trusted| tells if
     * the fallback can be used (1) or not (0).
     */
    int ignore_trusted = 1;

    if (pem_name != NULL) {
        if (strcmp(pem_name, PEM_STRING_X509_TRUSTED) == 0)
            ignore_trusted = 0;
        else if (strcmp(pem_name, PEM_STRING_X509_OLD) != 0
                 && strcmp(pem_name, PEM_STRING_X509) != 0)
            /* No match */
            return NULL;
        *matchcount = 1;
    }

    cert = X509_new_ex(libctx, propq);
    if (cert == NULL)
        return NULL;

    if ((d2i_X509_AUX(&cert, &blob, len)) != NULL
        || (ignore_trusted && (d2i_X509(&cert, &blob, len)) != NULL)) {
        *matchcount = 1;
        store_info = OSSL_STORE_INFO_new_CERT(cert);
    }

    if (store_info == NULL)
        X509_free(cert);

    return store_info;
}

static FILE_HANDLER X509Certificate_handler = {
    "X509Certificate",
    try_decode_X509Certificate
};

/*
 * X.509 CRL decoder.
 */
static OSSL_STORE_INFO *try_decode_X509CRL(const char *pem_name,
                                           const char *pem_header,
                                           const unsigned char *blob,
                                           size_t len, void **pctx,
                                           int *matchcount,
                                           const UI_METHOD *ui_method,
                                           void *ui_data, const char *uri,
                                           OSSL_LIB_CTX *libctx,
                                           const char *propq)
{
    OSSL_STORE_INFO *store_info = NULL;
    X509_CRL *crl = NULL;

    if (pem_name != NULL) {
        if (strcmp(pem_name, PEM_STRING_X509_CRL) != 0)
            /* No match */
            return NULL;
        *matchcount = 1;
    }

    if ((crl = d2i_X509_CRL(NULL, &blob, len)) != NULL) {
        *matchcount = 1;
        store_info = OSSL_STORE_INFO_new_CRL(crl);
    }

    if (store_info == NULL)
        X509_CRL_free(crl);

    return store_info;
}

static FILE_HANDLER X509CRL_handler = {
    "X509CRL",
    try_decode_X509CRL
};

/*
 * To finish it all off, we collect all the handlers.
 */
static const FILE_HANDLER *file_handlers[] = {
    &PKCS12_handler,
    &PKCS8Encrypted_handler,
    &X509Certificate_handler,
    &X509CRL_handler,
    &params_handler,
    &PUBKEY_handler,
    &PrivateKey_handler,
};


/*-
 *  The loader itself
 *  -----------------
 */

struct ossl_store_loader_ctx_st {
    char *uri;                   /* The URI we currently try to load */
    enum {
        is_raw = 0,
        is_pem,
        is_dir
    } type;
    int errcnt;
#define FILE_FLAG_SECMEM         (1<<0)
#define FILE_FLAG_ATTACHED       (1<<1)
    unsigned int flags;
    union {
        struct { /* Used with is_raw and is_pem */
            BIO *file;

            /*
             * The following are used when the handler is marked as
             * repeatable
             */
            const FILE_HANDLER *last_handler;
            void *last_handler_ctx;
        } file;
        struct { /* Used with is_dir */
            OPENSSL_DIR_CTX *ctx;
            int end_reached;

            /*
             * When a search expression is given, these are filled in.
             * |search_name| contains the file basename to look for.
             * The string is exactly 8 characters long.
             */
            char search_name[9];

            /*
             * The directory reading utility we have combines opening with
             * reading the first name.  To make sure we can detect the end
             * at the right time, we read early and cache the name.
             */
            const char *last_entry;
            int last_errno;
        } dir;
    } _;

    /* Expected object type.  May be unspecified */
    int expected_type;

    OSSL_LIB_CTX *libctx;
    char *propq;
};

static void OSSL_STORE_LOADER_CTX_free(OSSL_STORE_LOADER_CTX *ctx)
{
    if (ctx == NULL)
        return;

    OPENSSL_free(ctx->propq);
    OPENSSL_free(ctx->uri);
    if (ctx->type != is_dir) {
        if (ctx->_.file.last_handler != NULL) {
            ctx->_.file.last_handler->destroy_ctx(&ctx->_.file.last_handler_ctx);
            ctx->_.file.last_handler_ctx = NULL;
            ctx->_.file.last_handler = NULL;
        }
    }
    OPENSSL_free(ctx);
}

static int file_find_type(OSSL_STORE_LOADER_CTX *ctx)
{
    BIO *buff = NULL;
    char peekbuf[4096] = { 0, };

    if ((buff = BIO_new(BIO_f_buffer())) == NULL)
        return 0;

    ctx->_.file.file = BIO_push(buff, ctx->_.file.file);
    if (BIO_buffer_peek(ctx->_.file.file, peekbuf, sizeof(peekbuf) - 1) > 0) {
        peekbuf[sizeof(peekbuf) - 1] = '\0';
        if (strstr(peekbuf, "-----BEGIN ") != NULL)
            ctx->type = is_pem;
    }
    return 1;
}

static OSSL_STORE_LOADER_CTX *file_open_ex
    (const OSSL_STORE_LOADER *loader, const char *uri,
     OSSL_LIB_CTX *libctx, const char *propq,
     const UI_METHOD *ui_method, void *ui_data)
{
    OSSL_STORE_LOADER_CTX *ctx = NULL;
    struct stat st;
    struct {
        const char *path;
        unsigned int check_absolute:1;
    } path_data[2];
    size_t path_data_n = 0, i;
    const char *path, *p = uri, *q;

    /*
     * First step, just take the URI as is.
     */
    path_data[path_data_n].check_absolute = 0;
    path_data[path_data_n++].path = uri;

    /*
     * Second step, if the URI appears to start with the "file" scheme,
     * extract the path and make that the second path to check.
     * There's a special case if the URI also contains an authority, then
     * the full URI shouldn't be used as a path anywhere.
     */
    if (CHECK_AND_SKIP_CASE_PREFIX(p, "file:")) {
        q = p;
        if (CHECK_AND_SKIP_PREFIX(q, "//")) {
            path_data_n--;           /* Invalidate using the full URI */
            if (CHECK_AND_SKIP_CASE_PREFIX(q, "localhost/")
                    || CHECK_AND_SKIP_PREFIX(q, "/")) {
                p = q - 1;
            } else {
                ATTICerr(0, ATTIC_R_URI_AUTHORITY_UNSUPPORTED);
                return NULL;
            }
        }

        path_data[path_data_n].check_absolute = 1;
#ifdef _WIN32
        /* Windows "file:" URIs with a drive letter start with a '/' */
        if (p[0] == '/' && p[2] == ':' && p[3] == '/') {
            char c = tolower((unsigned char)p[1]);

            if (c >= 'a' && c <= 'z') {
                p++;
                /* We know it's absolute, so no need to check */
                path_data[path_data_n].check_absolute = 0;
            }
        }
#endif
        path_data[path_data_n++].path = p;
    }


    for (i = 0, path = NULL; path == NULL && i < path_data_n; i++) {
        /*
         * If the scheme "file" was an explicit part of the URI, the path must
         * be absolute.  So says RFC 8089
         */
        if (path_data[i].check_absolute && path_data[i].path[0] != '/') {
            ATTICerr(0, ATTIC_R_PATH_MUST_BE_ABSOLUTE);
            ERR_add_error_data(1, path_data[i].path);
            return NULL;
        }

        if (stat(path_data[i].path, &st) < 0) {
            ERR_raise_data(ERR_LIB_SYS, errno,
                           "calling stat(%s)",
                           path_data[i].path);
        } else {
            path = path_data[i].path;
        }
    }
    if (path == NULL) {
        return NULL;
    }

    /* Successfully found a working path */

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx == NULL)
        return NULL;
    ctx->uri = OPENSSL_strdup(uri);
    if (ctx->uri == NULL)
        goto err;

    if (S_ISDIR(st.st_mode)) {
        ctx->type = is_dir;
        ctx->_.dir.last_entry = OPENSSL_DIR_read(&ctx->_.dir.ctx, path);
        ctx->_.dir.last_errno = errno;
        if (ctx->_.dir.last_entry == NULL) {
            if (ctx->_.dir.last_errno != 0) {
                ERR_raise(ERR_LIB_SYS, ctx->_.dir.last_errno);
                goto err;
            }
            ctx->_.dir.end_reached = 1;
        }
    } else if ((ctx->_.file.file = BIO_new_file(path, "rb")) == NULL
               || !file_find_type(ctx)) {
        BIO_free_all(ctx->_.file.file);
        goto err;
    }
    if (propq != NULL) {
        ctx->propq = OPENSSL_strdup(propq);
        if (ctx->propq == NULL)
            goto err;
    }
    ctx->libctx = libctx;

    return ctx;
 err:
    OSSL_STORE_LOADER_CTX_free(ctx);
    return NULL;
}

static OSSL_STORE_LOADER_CTX *file_open
    (const OSSL_STORE_LOADER *loader, const char *uri,
     const UI_METHOD *ui_method, void *ui_data)
{
    return file_open_ex(loader, uri, NULL, NULL, ui_method, ui_data);
}

static OSSL_STORE_LOADER_CTX *file_attach
    (const OSSL_STORE_LOADER *loader, BIO *bp,
     OSSL_LIB_CTX *libctx, const char *propq,
     const UI_METHOD *ui_method, void *ui_data)
{
    OSSL_STORE_LOADER_CTX *ctx = NULL;

    if ((ctx = OPENSSL_zalloc(sizeof(*ctx))) == NULL
        || (propq != NULL && (ctx->propq = OPENSSL_strdup(propq)) == NULL)) {
        OSSL_STORE_LOADER_CTX_free(ctx);
        return NULL;
    }
    ctx->libctx = libctx;
    ctx->flags |= FILE_FLAG_ATTACHED;
    ctx->_.file.file = bp;
    if (!file_find_type(ctx)) {
        /* Safety measure */
        ctx->_.file.file = NULL;
        goto err;
    }
    return ctx;
err:
    OSSL_STORE_LOADER_CTX_free(ctx);
    return NULL;
}

static int file_ctrl(OSSL_STORE_LOADER_CTX *ctx, int cmd, va_list args)
{
    int ret = 1;

    switch (cmd) {
    case OSSL_STORE_C_USE_SECMEM:
        {
            int on = *(va_arg(args, int *));

            switch (on) {
            case 0:
                ctx->flags &= ~FILE_FLAG_SECMEM;
                break;
            case 1:
                ctx->flags |= FILE_FLAG_SECMEM;
                break;
            default:
                ATTICerr(0, ERR_R_PASSED_INVALID_ARGUMENT);
                ret = 0;
                break;
            }
        }
        break;
    default:
        break;
    }

    return ret;
}

static int file_expect(OSSL_STORE_LOADER_CTX *ctx, int expected)
{
    ctx->expected_type = expected;
    return 1;
}

static int file_find(OSSL_STORE_LOADER_CTX *ctx,
                     const OSSL_STORE_SEARCH *search)
{
    /*
     * If ctx == NULL, the library is looking to know if this loader supports
     * the given search type.
     */

    if (OSSL_STORE_SEARCH_get_type(search) == OSSL_STORE_SEARCH_BY_NAME) {
        unsigned long hash = 0;

        if (ctx == NULL)
            return 1;

        if (ctx->type != is_dir) {
            ATTICerr(0, ATTIC_R_SEARCH_ONLY_SUPPORTED_FOR_DIRECTORIES);
            return 0;
        }

        hash = X509_NAME_hash_ex(OSSL_STORE_SEARCH_get0_name(search),
                                 NULL, NULL, NULL);
        BIO_snprintf(ctx->_.dir.search_name, sizeof(ctx->_.dir.search_name),
                     "%08lx", hash);
        return 1;
    }

    if (ctx != NULL)
        ATTICerr(0, ATTIC_R_UNSUPPORTED_SEARCH_TYPE);
    return 0;
}

static OSSL_STORE_INFO *file_load_try_decode(OSSL_STORE_LOADER_CTX *ctx,
                                             const char *pem_name,
                                             const char *pem_header,
                                             unsigned char *data, size_t len,
                                             const UI_METHOD *ui_method,
                                             void *ui_data, int *matchcount)
{
    OSSL_STORE_INFO *result = NULL;
    BUF_MEM *new_mem = NULL;
    char *new_pem_name = NULL;
    int t = 0;

 again:
    {
        size_t i = 0;
        void *handler_ctx = NULL;
        const FILE_HANDLER **matching_handlers =
            OPENSSL_zalloc(sizeof(*matching_handlers)
                           * OSSL_NELEM(file_handlers));

        if (matching_handlers == NULL)
            goto err;

        *matchcount = 0;
        for (i = 0; i < OSSL_NELEM(file_handlers); i++) {
            const FILE_HANDLER *handler = file_handlers[i];
            int try_matchcount = 0;
            void *tmp_handler_ctx = NULL;
            OSSL_STORE_INFO *tmp_result;
            unsigned long err;

            ERR_set_mark();
            tmp_result =
                handler->try_decode(pem_name, pem_header, data, len,
                                    &tmp_handler_ctx, &try_matchcount,
                                    ui_method, ui_data, ctx->uri,
                                    ctx->libctx, ctx->propq);
            /* avoid flooding error queue with low-level ASN.1 parse errors */
            err = ERR_peek_last_error();
            if (ERR_GET_LIB(err) == ERR_LIB_ASN1
                    && ERR_GET_REASON(err) == ERR_R_NESTED_ASN1_ERROR)
                ERR_pop_to_mark();
            else
                ERR_clear_last_mark();

            if (try_matchcount > 0) {

                matching_handlers[*matchcount] = handler;

                if (handler_ctx)
                    handler->destroy_ctx(&handler_ctx);
                handler_ctx = tmp_handler_ctx;

                if ((*matchcount += try_matchcount) > 1) {
                    /* more than one match => ambiguous, kill any result */
                    store_info_free(result);
                    store_info_free(tmp_result);
                    if (handler->destroy_ctx != NULL)
                        handler->destroy_ctx(&handler_ctx);
                    handler_ctx = NULL;
                    tmp_result = NULL;
                    result = NULL;
                }
                if (result == NULL)
                    result = tmp_result;
                if (result == NULL) /* e.g., PKCS#12 file decryption error */
                    break;
            }
        }

        if (result != NULL
                && *matchcount == 1 && matching_handlers[0]->repeatable) {
            ctx->_.file.last_handler = matching_handlers[0];
            ctx->_.file.last_handler_ctx = handler_ctx;
        }

        OPENSSL_free(matching_handlers);
    }

 err:
    OPENSSL_free(new_pem_name);
    BUF_MEM_free(new_mem);

    if (result != NULL
        && (t = OSSL_STORE_INFO_get_type(result)) == STORE_INFO_EMBEDDED) {
        struct embedded_st *embedded = get0_EMBEDDED(result);

        /* "steal" the embedded data */
        pem_name = new_pem_name = embedded->pem_name;
        new_mem = embedded->blob;
        data = (unsigned char *)new_mem->data;
        len = new_mem->length;
        embedded->pem_name = NULL;
        embedded->blob = NULL;

        store_info_free(result);
        result = NULL;
        goto again;
    }

    return result;
}

static OSSL_STORE_INFO *file_load_try_repeat(OSSL_STORE_LOADER_CTX *ctx,
                                             const UI_METHOD *ui_method,
                                             void *ui_data)
{
    OSSL_STORE_INFO *result = NULL;
    int try_matchcount = 0;

    if (ctx->_.file.last_handler != NULL) {
        result =
            ctx->_.file.last_handler->try_decode(NULL, NULL, NULL, 0,
                                                 &ctx->_.file.last_handler_ctx,
                                                 &try_matchcount,
                                                 ui_method, ui_data, ctx->uri,
                                                 ctx->libctx, ctx->propq);

        if (result == NULL) {
            ctx->_.file.last_handler->destroy_ctx(&ctx->_.file.last_handler_ctx);
            ctx->_.file.last_handler_ctx = NULL;
            ctx->_.file.last_handler = NULL;
        }
    }
    return result;
}

static void pem_free_flag(void *pem_data, int secure, size_t num)
{
    if (secure)
        OPENSSL_secure_clear_free(pem_data, num);
    else
        OPENSSL_free(pem_data);
}
static int file_read_pem(BIO *bp, char **pem_name, char **pem_header,
                         unsigned char **data, long *len,
                         const UI_METHOD *ui_method, void *ui_data,
                         const char *uri, int secure)
{
    int i = secure
        ? PEM_read_bio_ex(bp, pem_name, pem_header, data, len,
                          PEM_FLAG_SECURE | PEM_FLAG_EAY_COMPATIBLE)
        : PEM_read_bio(bp, pem_name, pem_header, data, len);

    if (i <= 0)
        return 0;

    /*
     * 10 is the number of characters in "Proc-Type:", which
     * PEM_get_EVP_CIPHER_INFO() requires to be present.
     * If the PEM header has less characters than that, it's
     * not worth spending cycles on it.
     */
    if (strlen(*pem_header) > 10) {
        EVP_CIPHER_INFO cipher;
        struct pem_pass_data pass_data;

        if (!PEM_get_EVP_CIPHER_INFO(*pem_header, &cipher)
            || !file_fill_pem_pass_data(&pass_data, "PEM pass phrase", uri,
                                        ui_method, ui_data)
            || !PEM_do_header(&cipher, *data, len, file_get_pem_pass,
                              &pass_data)) {
            return 0;
        }
    }
    return 1;
}

static OSSL_STORE_INFO *file_try_read_msblob(BIO *bp, int *matchcount)
{
    OSSL_STORE_INFO *result = NULL;
    int ispub = -1;

    {
        unsigned int magic = 0, bitlen = 0;
        int isdss = 0;
        unsigned char peekbuf[16] = { 0, };
        const unsigned char *p = peekbuf;

        if (BIO_buffer_peek(bp, peekbuf, sizeof(peekbuf)) <= 0)
            return 0;
        if (ossl_do_blob_header(&p, sizeof(peekbuf), &magic, &bitlen,
                                 &isdss, &ispub) <= 0)
            return 0;
    }

    (*matchcount)++;

    {
        EVP_PKEY *tmp = ispub
            ? b2i_PublicKey_bio(bp)
            : b2i_PrivateKey_bio(bp);

        if (tmp == NULL
            || (result = OSSL_STORE_INFO_new_PKEY(tmp)) == NULL) {
            EVP_PKEY_free(tmp);
            return 0;
        }
    }

    return result;
}

static OSSL_STORE_INFO *file_try_read_PVK(BIO *bp, const UI_METHOD *ui_method,
                                          void *ui_data, const char *uri,
                                          int *matchcount)
{
    OSSL_STORE_INFO *result = NULL;

    {
        unsigned int saltlen = 0, keylen = 0;
        int isdss = -1;
        unsigned char peekbuf[24] = { 0, };
        const unsigned char *p = peekbuf;

        if (BIO_buffer_peek(bp, peekbuf, sizeof(peekbuf)) <= 0)
            return 0;
        if (!ossl_do_PVK_header(&p, sizeof(peekbuf), 0, &isdss, &saltlen, &keylen))
            return 0;
    }

    (*matchcount)++;

    {
        EVP_PKEY *tmp = NULL;
        struct pem_pass_data pass_data;

        if (!file_fill_pem_pass_data(&pass_data, "PVK pass phrase", uri,
                                     ui_method, ui_data)
            || (tmp = b2i_PVK_bio(bp, file_get_pem_pass, &pass_data)) == NULL
            || (result = OSSL_STORE_INFO_new_PKEY(tmp)) == NULL) {
            EVP_PKEY_free(tmp);
            return 0;
        }
    }

    return result;
}

static int file_read_asn1(BIO *bp, unsigned char **data, long *len)
{
    BUF_MEM *mem = NULL;

    if (asn1_d2i_read_bio(bp, &mem) < 0)
        return 0;

    *data = (unsigned char *)mem->data;
    *len = (long)mem->length;
    OPENSSL_free(mem);

    return 1;
}

static int file_name_to_uri(OSSL_STORE_LOADER_CTX *ctx, const char *name,
                            char **data)
{
    assert(name != NULL);
    assert(data != NULL);
    {
        const char *pathsep = ossl_ends_with_dirsep(ctx->uri) ? "" : "/";
        long calculated_length = strlen(ctx->uri) + strlen(pathsep)
            + strlen(name) + 1 /* \0 */;

        *data = OPENSSL_zalloc(calculated_length);
        if (*data == NULL)
            return 0;

        OPENSSL_strlcat(*data, ctx->uri, calculated_length);
        OPENSSL_strlcat(*data, pathsep, calculated_length);
        OPENSSL_strlcat(*data, name, calculated_length);
    }
    return 1;
}

static int file_name_check(OSSL_STORE_LOADER_CTX *ctx, const char *name)
{
    const char *p = NULL;
    size_t len = strlen(ctx->_.dir.search_name);

    /* If there are no search criteria, all names are accepted */
    if (ctx->_.dir.search_name[0] == '\0')
        return 1;

    /* If the expected type isn't supported, no name is accepted */
    if (ctx->expected_type != 0
        && ctx->expected_type != OSSL_STORE_INFO_CERT
        && ctx->expected_type != OSSL_STORE_INFO_CRL)
        return 0;

    /*
     * First, check the basename
     */
    if (OPENSSL_strncasecmp(name, ctx->_.dir.search_name, len) != 0
        || name[len] != '.')
        return 0;
    p = &name[len + 1];

    /*
     * Then, if the expected type is a CRL, check that the extension starts
     * with 'r'
     */
    if (*p == 'r') {
        p++;
        if (ctx->expected_type != 0
            && ctx->expected_type != OSSL_STORE_INFO_CRL)
            return 0;
    } else if (ctx->expected_type == OSSL_STORE_INFO_CRL) {
        return 0;
    }

    /*
     * Last, check that the rest of the extension is a decimal number, at
     * least one digit long.
     */
    if (!isdigit((unsigned char)*p))
        return 0;
    while (isdigit((unsigned char)*p))
        p++;

#ifdef __VMS
    /*
     * One extra step here, check for a possible generation number.
     */
    if (*p == ';')
        for (p++; *p != '\0'; p++)
            if (!ossl_isdigit(*p))
                break;
#endif

    /*
     * If we've reached the end of the string at this point, we've successfully
     * found a fitting file name.
     */
    return *p == '\0';
}

static int file_eof(OSSL_STORE_LOADER_CTX *ctx);
static int file_error(OSSL_STORE_LOADER_CTX *ctx);
static OSSL_STORE_INFO *file_load(OSSL_STORE_LOADER_CTX *ctx,
                                  const UI_METHOD *ui_method,
                                  void *ui_data)
{
    OSSL_STORE_INFO *result = NULL;

    ctx->errcnt = 0;

    if (ctx->type == is_dir) {
        do {
            char *newname = NULL;

            if (ctx->_.dir.last_entry == NULL) {
                if (!ctx->_.dir.end_reached) {
                    assert(ctx->_.dir.last_errno != 0);
                    ERR_raise(ERR_LIB_SYS, ctx->_.dir.last_errno);
                    ctx->errcnt++;
                }
                return NULL;
            }

            if (ctx->_.dir.last_entry[0] != '.'
                && file_name_check(ctx, ctx->_.dir.last_entry)
                && !file_name_to_uri(ctx, ctx->_.dir.last_entry, &newname))
                return NULL;

            /*
             * On the first call (with a NULL context), OPENSSL_DIR_read()
             * cares about the second argument.  On the following calls, it
             * only cares that it isn't NULL.  Therefore, we can safely give
             * it our URI here.
             */
            ctx->_.dir.last_entry = OPENSSL_DIR_read(&ctx->_.dir.ctx, ctx->uri);
            ctx->_.dir.last_errno = errno;
            if (ctx->_.dir.last_entry == NULL && ctx->_.dir.last_errno == 0)
                ctx->_.dir.end_reached = 1;

            if (newname != NULL
                && (result = OSSL_STORE_INFO_new_NAME(newname)) == NULL) {
                OPENSSL_free(newname);
                ATTICerr(0, ERR_R_OSSL_STORE_LIB);
                return NULL;
            }
        } while (result == NULL && !file_eof(ctx));
    } else {
        int matchcount = -1;

     again:
        result = file_load_try_repeat(ctx, ui_method, ui_data);
        if (result != NULL)
            return result;

        if (file_eof(ctx))
            return NULL;

        do {
            char *pem_name = NULL;      /* PEM record name */
            char *pem_header = NULL;    /* PEM record header */
            unsigned char *data = NULL; /* DER encoded data */
            long len = 0;               /* DER encoded data length */

            matchcount = -1;
            if (ctx->type == is_pem) {
                if (!file_read_pem(ctx->_.file.file, &pem_name, &pem_header,
                                   &data, &len, ui_method, ui_data, ctx->uri,
                                   (ctx->flags & FILE_FLAG_SECMEM) != 0)) {
                    ctx->errcnt++;
                    goto endloop;
                }
            } else {
                if ((result = file_try_read_msblob(ctx->_.file.file,
                                                   &matchcount)) != NULL
                    || (result = file_try_read_PVK(ctx->_.file.file,
                                                   ui_method, ui_data, ctx->uri,
                                                   &matchcount)) != NULL)
                    goto endloop;

                if (!file_read_asn1(ctx->_.file.file, &data, &len)) {
                    ctx->errcnt++;
                    goto endloop;
                }
            }

            result = file_load_try_decode(ctx, pem_name, pem_header, data, len,
                                          ui_method, ui_data, &matchcount);

            if (result != NULL)
                goto endloop;

            /*
             * If a PEM name matches more than one handler, the handlers are
             * badly coded.
             */
            if (!ossl_assert(pem_name == NULL || matchcount <= 1)) {
                ctx->errcnt++;
                goto endloop;
            }

            if (matchcount > 1) {
                ATTICerr(0, ATTIC_R_AMBIGUOUS_CONTENT_TYPE);
            } else if (matchcount == 1) {
                /*
                 * If there are other errors on the stack, they already show
                 * what the problem is.
                 */
                if (ERR_peek_error() == 0) {
                    ATTICerr(0, ATTIC_R_UNSUPPORTED_CONTENT_TYPE);
                    if (pem_name != NULL)
                        ERR_add_error_data(3, "PEM type is '", pem_name, "'");
                }
            }
            if (matchcount > 0)
                ctx->errcnt++;

         endloop:
            pem_free_flag(pem_name, (ctx->flags & FILE_FLAG_SECMEM) != 0, 0);
            pem_free_flag(pem_header, (ctx->flags & FILE_FLAG_SECMEM) != 0, 0);
            pem_free_flag(data, (ctx->flags & FILE_FLAG_SECMEM) != 0, len);
        } while (matchcount == 0 && !file_eof(ctx) && !file_error(ctx));

        /* We bail out on ambiguity */
        if (matchcount > 1) {
            store_info_free(result);
            return NULL;
        }

        if (result != NULL
            && ctx->expected_type != 0
            && ctx->expected_type != OSSL_STORE_INFO_get_type(result)) {
            store_info_free(result);
            goto again;
        }
    }

    return result;
}

static int file_error(OSSL_STORE_LOADER_CTX *ctx)
{
    return ctx->errcnt > 0;
}

static int file_eof(OSSL_STORE_LOADER_CTX *ctx)
{
    if (ctx->type == is_dir)
        return ctx->_.dir.end_reached;

    if (ctx->_.file.last_handler != NULL
        && !ctx->_.file.last_handler->eof(ctx->_.file.last_handler_ctx))
        return 0;
    return BIO_eof(ctx->_.file.file);
}

static int file_close(OSSL_STORE_LOADER_CTX *ctx)
{
    if ((ctx->flags & FILE_FLAG_ATTACHED) == 0) {
        if (ctx->type == is_dir)
            OPENSSL_DIR_end(&ctx->_.dir.ctx);
        else
            BIO_free_all(ctx->_.file.file);
    } else {
        /*
         * Because file_attach() called file_find_type(), we know that a
         * BIO_f_buffer() has been pushed on top of the regular BIO.
         */
        BIO *buff = ctx->_.file.file;

        /* Detach buff */
        (void)BIO_pop(ctx->_.file.file);
        /* Safety measure */
        ctx->_.file.file = NULL;

        BIO_free(buff);
    }
    OSSL_STORE_LOADER_CTX_free(ctx);
    return 1;
}

/*-
 * ENGINE management
 */

static const char *loader_attic_id = "loader_attic";
static const char *loader_attic_name = "'file:' loader";

static OSSL_STORE_LOADER *loader_attic = NULL;

static int loader_attic_init(ENGINE *e)
{
    return 1;
}


static int loader_attic_finish(ENGINE *e)
{
    return 1;
}


static int loader_attic_destroy(ENGINE *e)
{
    OSSL_STORE_LOADER *loader = OSSL_STORE_unregister_loader("file");

    if (loader == NULL)
        return 0;

    ERR_unload_ATTIC_strings();
    OSSL_STORE_LOADER_free(loader);
    return 1;
}

static int bind_loader_attic(ENGINE *e)
{

    /* Ensure the ATTIC error handling is set up on best effort basis */
    ERR_load_ATTIC_strings();

    if (/* Create the OSSL_STORE_LOADER */
        (loader_attic = OSSL_STORE_LOADER_new(e, "file")) == NULL
        || !OSSL_STORE_LOADER_set_open_ex(loader_attic, file_open_ex)
        || !OSSL_STORE_LOADER_set_open(loader_attic, file_open)
        || !OSSL_STORE_LOADER_set_attach(loader_attic, file_attach)
        || !OSSL_STORE_LOADER_set_ctrl(loader_attic, file_ctrl)
        || !OSSL_STORE_LOADER_set_expect(loader_attic, file_expect)
        || !OSSL_STORE_LOADER_set_find(loader_attic, file_find)
        || !OSSL_STORE_LOADER_set_load(loader_attic, file_load)
        || !OSSL_STORE_LOADER_set_eof(loader_attic, file_eof)
        || !OSSL_STORE_LOADER_set_error(loader_attic, file_error)
        || !OSSL_STORE_LOADER_set_close(loader_attic, file_close)
        /* Init the engine itself */
        || !ENGINE_set_id(e, loader_attic_id)
        || !ENGINE_set_name(e, loader_attic_name)
        || !ENGINE_set_destroy_function(e, loader_attic_destroy)
        || !ENGINE_set_init_function(e, loader_attic_init)
        || !ENGINE_set_finish_function(e, loader_attic_finish)
        /* Finally, register the method with libcrypto */
        || !OSSL_STORE_register_loader(loader_attic)) {
        OSSL_STORE_LOADER_free(loader_attic);
        loader_attic = NULL;
        ATTICerr(0, ATTIC_R_INIT_FAILED);
        return 0;
    }

    return 1;
}

#ifdef OPENSSL_NO_DYNAMIC_ENGINE
# error "Only allowed as dynamically shared object"
#endif

static int bind_helper(ENGINE *e, const char *id)
{
    if (id && (strcmp(id, loader_attic_id) != 0))
        return 0;
    if (!bind_loader_attic(e))
        return 0;
    return 1;
}

IMPLEMENT_DYNAMIC_CHECK_FN()
    IMPLEMENT_DYNAMIC_BIND_FN(bind_helper)
