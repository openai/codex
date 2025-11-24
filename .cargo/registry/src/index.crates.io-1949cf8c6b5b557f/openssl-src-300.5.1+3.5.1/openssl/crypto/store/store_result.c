/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/e_os.h"
#include <string.h>

#include <openssl/core.h>
#include <openssl/core_names.h>
#include <openssl/core_object.h>
#include <openssl/err.h>
#include <openssl/pkcs12.h>
#include <openssl/provider.h>
#include <openssl/decoder.h>
#include <openssl/store.h>
#include "internal/provider.h"
#include "internal/passphrase.h"
#include "crypto/decoder.h"
#include "crypto/evp.h"
#include "crypto/x509.h"
#include "store_local.h"

#ifndef OSSL_OBJECT_PKCS12
/*
 * The object abstraction doesn't know PKCS#12, but we want to indicate
 * it anyway, so we create our own.  Since the public macros use positive
 * numbers, negative ones should be fine.  They must never slip out from
 * this translation unit anyway.
 */
# define OSSL_OBJECT_PKCS12 -1
#endif

/*
 * ossl_store_handle_load_result() is initially written to be a companion
 * to our 'file:' scheme provider implementation, but has been made generic
 * to serve others as well.
 *
 * This result handler takes any object abstraction (see provider-object(7))
 * and does the best it can with it.  If the object is passed by value (not
 * by reference), the contents are currently expected to be DER encoded.
 * If an object type is specified, that will be respected; otherwise, this
 * handler will guess the contents, by trying the following in order:
 *
 * 1.  Decode it into an EVP_PKEY, using OSSL_DECODER.
 * 2.  Decode it into an X.509 certificate, using d2i_X509 / d2i_X509_AUX.
 * 3.  Decode it into an X.509 CRL, using d2i_X509_CRL.
 * 4.  Decode it into a PKCS#12 structure, using d2i_PKCS12 (*).
 *
 * For the 'file:' scheme implementation, this is division of labor.  Since
 * the libcrypto <-> provider interface currently doesn't support certain
 * structures as first class objects, they must be unpacked from DER here
 * rather than in the provider.  The current exception is asymmetric keys,
 * which can reside within the provider boundary, most of all thanks to
 * OSSL_FUNC_keymgmt_load(), which allows loading the key material by
 * reference.
 */

struct extracted_param_data_st {
    int object_type;
    const char *data_type;
    const char *input_type;
    const char *data_structure;
    const char *utf8_data;
    const void *octet_data;
    size_t octet_data_size;
    const void *ref;
    size_t ref_size;
    const char *desc;
};

static int try_name(struct extracted_param_data_st *, OSSL_STORE_INFO **);
static int try_key(struct extracted_param_data_st *, OSSL_STORE_INFO **,
                   OSSL_STORE_CTX *, const OSSL_PROVIDER *,
                   OSSL_LIB_CTX *, const char *);
static int try_cert(struct extracted_param_data_st *, OSSL_STORE_INFO **,
                    OSSL_LIB_CTX *, const char *);
static int try_crl(struct extracted_param_data_st *, OSSL_STORE_INFO **,
                   OSSL_LIB_CTX *, const char *);
static int try_pkcs12(struct extracted_param_data_st *, OSSL_STORE_INFO **,
                      OSSL_STORE_CTX *, OSSL_LIB_CTX *, const char *);

int ossl_store_handle_load_result(const OSSL_PARAM params[], void *arg)
{
    struct ossl_load_result_data_st *cbdata = arg;
    OSSL_STORE_INFO **v = &cbdata->v;
    OSSL_STORE_CTX *ctx = cbdata->ctx;
    const OSSL_PROVIDER *provider =
        OSSL_STORE_LOADER_get0_provider(ctx->fetched_loader);
    OSSL_LIB_CTX *libctx = ossl_provider_libctx(provider);
    const char *propq = ctx->properties;
    const OSSL_PARAM *p;
    struct extracted_param_data_st helper_data;

    memset(&helper_data, 0, sizeof(helper_data));
    helper_data.object_type = OSSL_OBJECT_UNKNOWN;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_OBJECT_PARAM_TYPE)) != NULL
        && !OSSL_PARAM_get_int(p, &helper_data.object_type))
        return 0;
    p = OSSL_PARAM_locate_const(params, OSSL_OBJECT_PARAM_DATA_TYPE);
    if (p != NULL
        && !OSSL_PARAM_get_utf8_string_ptr(p, &helper_data.data_type))
        return 0;
    p = OSSL_PARAM_locate_const(params, OSSL_OBJECT_PARAM_DATA);
    if (p != NULL
        && !OSSL_PARAM_get_octet_string_ptr(p, &helper_data.octet_data,
                                            &helper_data.octet_data_size)
        && !OSSL_PARAM_get_utf8_string_ptr(p, &helper_data.utf8_data))
        return 0;
    p = OSSL_PARAM_locate_const(params, OSSL_OBJECT_PARAM_DATA_STRUCTURE);
    if (p != NULL
        && !OSSL_PARAM_get_utf8_string_ptr(p, &helper_data.data_structure))
        return 0;
    p = OSSL_PARAM_locate_const(params, OSSL_OBJECT_PARAM_INPUT_TYPE);
    if (p != NULL
        && !OSSL_PARAM_get_utf8_string_ptr(p, &helper_data.input_type))
        return 0;
    p = OSSL_PARAM_locate_const(params, OSSL_OBJECT_PARAM_REFERENCE);
    if (p != NULL && !OSSL_PARAM_get_octet_string_ptr(p, &helper_data.ref,
                                                      &helper_data.ref_size))
        return 0;
    p = OSSL_PARAM_locate_const(params, OSSL_OBJECT_PARAM_DESC);
    if (p != NULL && !OSSL_PARAM_get_utf8_string_ptr(p, &helper_data.desc))
        return 0;

    /*
     * The helper functions return 0 on actual errors, otherwise 1, even if
     * they didn't fill out |*v|.
     */
    ERR_set_mark();
    if (*v == NULL && !try_name(&helper_data, v))
        goto err;
    ERR_pop_to_mark();
    ERR_set_mark();
    if (*v == NULL && !try_key(&helper_data, v, ctx, provider, libctx, propq))
        goto err;
    ERR_pop_to_mark();
    ERR_set_mark();
    if (*v == NULL && !try_cert(&helper_data, v, libctx, propq))
        goto err;
    ERR_pop_to_mark();
    ERR_set_mark();
    if (*v == NULL && !try_crl(&helper_data, v, libctx, propq))
        goto err;
    ERR_pop_to_mark();
    ERR_set_mark();
    if (*v == NULL && !try_pkcs12(&helper_data, v, ctx, libctx, propq))
        goto err;
    ERR_pop_to_mark();

    if (*v == NULL) {
        const char *hint = "";

        if (!OSSL_PROVIDER_available(libctx, "default"))
            hint = ":maybe need to load the default provider?";
        if (provider != NULL)
            ERR_raise_data(ERR_LIB_OSSL_STORE, ERR_R_UNSUPPORTED, "provider=%s%s",
                           OSSL_PROVIDER_get0_name(provider), hint);
        else if (hint[0] != '\0')
            ERR_raise_data(ERR_LIB_OSSL_STORE, ERR_R_UNSUPPORTED, "%s", hint);
        else
            ERR_raise(ERR_LIB_OSSL_STORE, ERR_R_UNSUPPORTED);
    }

    return (*v != NULL);
 err:
    ERR_clear_last_mark();
    return 0;
}

static int try_name(struct extracted_param_data_st *data, OSSL_STORE_INFO **v)
{
    if (data->object_type == OSSL_OBJECT_NAME) {
        char *newname = NULL, *newdesc = NULL;

        if (data->utf8_data == NULL)
            return 0;
        if ((newname = OPENSSL_strdup(data->utf8_data)) == NULL
            || (data->desc != NULL
                && (newdesc = OPENSSL_strdup(data->desc)) == NULL)
            || (*v = OSSL_STORE_INFO_new_NAME(newname)) == NULL) {
            OPENSSL_free(newname);
            OPENSSL_free(newdesc);
            return 0;
        }
        OSSL_STORE_INFO_set0_NAME_description(*v, newdesc);
    }
    return 1;
}

/*
 * For the rest of the object types, the provider code may not know what
 * type of data it gave us, so we may need to figure that out on our own.
 * Therefore, we do check for OSSL_OBJECT_UNKNOWN everywhere below, and
 * only return 0 on error if the object type is known.
 */

static EVP_PKEY *try_key_ref(struct extracted_param_data_st *data,
                             OSSL_STORE_CTX *ctx,
                             const OSSL_PROVIDER *provider,
                             OSSL_LIB_CTX *libctx, const char *propq)
{
    EVP_PKEY *pk = NULL;
    EVP_KEYMGMT *keymgmt = NULL;
    void *keydata = NULL;
    int try_fallback = 2;

    /* If we have an object reference, we must have a data type */
    if (data->data_type == NULL)
        return 0;

    keymgmt = EVP_KEYMGMT_fetch(libctx, data->data_type, propq);
    ERR_set_mark();
    while (keymgmt != NULL && keydata == NULL && try_fallback-- > 0) {
        /*
         * There are two possible cases
         *
         * 1.  The keymgmt is from the same provider as the loader,
         *     so we can use evp_keymgmt_load()
         * 2.  The keymgmt is from another provider, then we must
         *     do the export/import dance.
         */
        if (EVP_KEYMGMT_get0_provider(keymgmt) == provider) {
            /* no point trying fallback here */
            try_fallback = 0;
            keydata = evp_keymgmt_load(keymgmt, data->ref, data->ref_size);
        } else {
            struct evp_keymgmt_util_try_import_data_st import_data;
            OSSL_FUNC_store_export_object_fn *export_object =
                ctx->fetched_loader->p_export_object;

            import_data.keymgmt = keymgmt;
            import_data.keydata = NULL;
            import_data.selection = OSSL_KEYMGMT_SELECT_ALL;

            if (export_object != NULL) {
                /*
                 * No need to check for errors here, the value of
                 * |import_data.keydata| is as much an indicator.
                 */
                (void)export_object(ctx->loader_ctx,
                                    data->ref, data->ref_size,
                                    &evp_keymgmt_util_try_import,
                                    &import_data);
            }

            keydata = import_data.keydata;
        }

        if (keydata == NULL && try_fallback > 0) {
            EVP_KEYMGMT_free(keymgmt);
            keymgmt = evp_keymgmt_fetch_from_prov((OSSL_PROVIDER *)provider,
                                                  data->data_type, propq);
            if (keymgmt != NULL) {
                ERR_pop_to_mark();
                ERR_set_mark();
            }
        }
    }
    if (keydata != NULL) {
        ERR_pop_to_mark();
        pk = evp_keymgmt_util_make_pkey(keymgmt, keydata);
    } else {
        ERR_clear_last_mark();
    }
    EVP_KEYMGMT_free(keymgmt);

    return pk;
}

static EVP_PKEY *try_key_value(struct extracted_param_data_st *data,
                               OSSL_STORE_CTX *ctx,
                               OSSL_PASSPHRASE_CALLBACK *cb, void *cbarg,
                               OSSL_LIB_CTX *libctx, const char *propq,
                               int *harderr)
{
    EVP_PKEY *pk = NULL;
    OSSL_DECODER_CTX *decoderctx = NULL;
    const unsigned char *pdata = data->octet_data;
    size_t pdatalen = data->octet_data_size;
    int selection = 0;

    switch (ctx->expected_type) {
    case 0:
        break;
    case OSSL_STORE_INFO_PARAMS:
        selection = OSSL_KEYMGMT_SELECT_ALL_PARAMETERS;
        break;
    case OSSL_STORE_INFO_PUBKEY:
        selection =
            OSSL_KEYMGMT_SELECT_PUBLIC_KEY
            | OSSL_KEYMGMT_SELECT_ALL_PARAMETERS;
        break;
    case OSSL_STORE_INFO_PKEY:
        selection = OSSL_KEYMGMT_SELECT_ALL;
        break;
    default:
        return NULL;
    }

    decoderctx =
        OSSL_DECODER_CTX_new_for_pkey(&pk, data->input_type, data->data_structure,
                                      data->data_type, selection, libctx,
                                      propq);
    (void)OSSL_DECODER_CTX_set_passphrase_cb(decoderctx, cb, cbarg);

    /* No error if this couldn't be decoded */
    (void)OSSL_DECODER_from_data(decoderctx, &pdata, &pdatalen);

    /* Save the hard error state. */
    *harderr = ossl_decoder_ctx_get_harderr(decoderctx);
    OSSL_DECODER_CTX_free(decoderctx);

    return pk;
}

typedef OSSL_STORE_INFO *store_info_new_fn(EVP_PKEY *);

static EVP_PKEY *try_key_value_legacy(struct extracted_param_data_st *data,
                                      store_info_new_fn **store_info_new,
                                      OSSL_STORE_CTX *ctx,
                                      OSSL_PASSPHRASE_CALLBACK *cb, void *cbarg,
                                      OSSL_LIB_CTX *libctx, const char *propq)
{
    EVP_PKEY *pk = NULL;
    const unsigned char *der = data->octet_data, *derp;
    long der_len = (long)data->octet_data_size;

    /* Try PUBKEY first, that's a real easy target */
    if (ctx->expected_type == 0
        || ctx->expected_type == OSSL_STORE_INFO_PUBKEY) {
        derp = der;
        pk = d2i_PUBKEY_ex(NULL, &derp, der_len, libctx, propq);

        if (pk != NULL)
            *store_info_new = OSSL_STORE_INFO_new_PUBKEY;
    }

    /* Try private keys next */
    if (pk == NULL
        && (ctx->expected_type == 0
            || ctx->expected_type == OSSL_STORE_INFO_PKEY)) {
        unsigned char *new_der = NULL;
        X509_SIG *p8 = NULL;
        PKCS8_PRIV_KEY_INFO *p8info = NULL;

        /* See if it's an encrypted PKCS#8 and decrypt it. */
        derp = der;
        p8 = d2i_X509_SIG(NULL, &derp, der_len);

        if (p8 != NULL) {
            char pbuf[PEM_BUFSIZE];
            size_t plen = 0;

            if (!cb(pbuf, sizeof(pbuf), &plen, NULL, cbarg)) {
                ERR_raise(ERR_LIB_OSSL_STORE, OSSL_STORE_R_BAD_PASSWORD_READ);
            } else {
                const X509_ALGOR *alg = NULL;
                const ASN1_OCTET_STRING *oct = NULL;
                int len = 0;

                X509_SIG_get0(p8, &alg, &oct);

                /*
                 * No need to check the returned value, |new_der|
                 * will be NULL on error anyway.
                 */
                PKCS12_pbe_crypt(alg, pbuf, plen,
                                 oct->data, oct->length,
                                 &new_der, &len, 0);
                der_len = len;
                der = new_der;
            }
            X509_SIG_free(p8);
        }

        /*
         * If the encrypted PKCS#8 couldn't be decrypted,
         * |der| is NULL
         */
        if (der != NULL) {
            /* Try to unpack an unencrypted PKCS#8, that's easy */
            derp = der;
            p8info = d2i_PKCS8_PRIV_KEY_INFO(NULL, &derp, der_len);

            if (p8info != NULL) {
                pk = EVP_PKCS82PKEY_ex(p8info, libctx, propq);
                PKCS8_PRIV_KEY_INFO_free(p8info);
            }
        }

        if (pk != NULL)
            *store_info_new = OSSL_STORE_INFO_new_PKEY;

        OPENSSL_free(new_der);
    }

    return pk;
}

static int try_key(struct extracted_param_data_st *data, OSSL_STORE_INFO **v,
                   OSSL_STORE_CTX *ctx, const OSSL_PROVIDER *provider,
                   OSSL_LIB_CTX *libctx, const char *propq)
{
    store_info_new_fn *store_info_new = NULL;
    int harderr = 0;

    if (data->object_type == OSSL_OBJECT_UNKNOWN
        || data->object_type == OSSL_OBJECT_PKEY) {
        EVP_PKEY *pk = NULL;

        /* Prefer key by reference than key by value */
        if (data->object_type == OSSL_OBJECT_PKEY && data->ref != NULL) {
            pk = try_key_ref(data, ctx, provider, libctx, propq);

            /*
             * If for some reason we couldn't get a key, it's an error.
             * It indicates that while decoders could make a key reference,
             * the keymgmt somehow couldn't handle it, or doesn't have a
             * OSSL_FUNC_keymgmt_load function.
             */
            if (pk == NULL)
                return 0;
        } else if (data->octet_data != NULL) {
            OSSL_PASSPHRASE_CALLBACK *cb = ossl_pw_passphrase_callback_dec;
            void *cbarg = &ctx->pwdata;

            pk = try_key_value(data, ctx, cb, cbarg, libctx, propq, &harderr);

            /*
             * Desperate last maneuver, in case the decoders don't support
             * the data we have, then we try on our own to at least get an
             * engine provided legacy key.
             * This is the same as der2key_decode() does, but in a limited
             * way and within the walls of libcrypto.
             */
            if (pk == NULL && harderr == 0)
                pk = try_key_value_legacy(data, &store_info_new, ctx,
                                          cb, cbarg, libctx, propq);
        }

        if (pk != NULL) {
            data->object_type = OSSL_OBJECT_PKEY;

            if (store_info_new == NULL) {
                /*
                 * We determined the object type for OSSL_STORE_INFO, which
                 * makes an explicit difference between an EVP_PKEY with just
                 * (domain) parameters and an EVP_PKEY with actual key
                 * material.
                 * The logic is that an EVP_PKEY with actual key material
                 * always has the public half.
                 */
                if (evp_keymgmt_util_has(pk, OSSL_KEYMGMT_SELECT_PRIVATE_KEY))
                    store_info_new = OSSL_STORE_INFO_new_PKEY;
                else if (evp_keymgmt_util_has(pk,
                                              OSSL_KEYMGMT_SELECT_PUBLIC_KEY))
                    store_info_new = OSSL_STORE_INFO_new_PUBKEY;
                else
                    store_info_new = OSSL_STORE_INFO_new_PARAMS;
            }
            *v = store_info_new(pk);
        }

        if (*v == NULL)
            EVP_PKEY_free(pk);
    }

    return harderr == 0;
}

static int try_cert(struct extracted_param_data_st *data, OSSL_STORE_INFO **v,
                    OSSL_LIB_CTX *libctx, const char *propq)
{
    if (data->object_type == OSSL_OBJECT_UNKNOWN
        || data->object_type == OSSL_OBJECT_CERT) {
        /*
         * In most cases, we can try to interpret the serialized
         * data as a trusted cert (X509 + X509_AUX) and fall back
         * to reading it as a normal cert (just X509), but if
         * |data_type| (the PEM name) specifically declares it as a
         * trusted cert, then no fallback should be engaged.
         * |ignore_trusted| tells if the fallback can be used (1)
         * or not (0).
         */
        int ignore_trusted = 1;
        X509 *cert = X509_new_ex(libctx, propq);

        if (cert == NULL)
            return 0;

        /* If we have a data type, it should be a PEM name */
        if (data->data_type != NULL
            && (OPENSSL_strcasecmp(data->data_type, PEM_STRING_X509_TRUSTED) == 0))
            ignore_trusted = 0;

        if (d2i_X509_AUX(&cert, (const unsigned char **)&data->octet_data,
                         data->octet_data_size) == NULL
            && (!ignore_trusted
                || d2i_X509(&cert, (const unsigned char **)&data->octet_data,
                            data->octet_data_size) == NULL)) {
            X509_free(cert);
            cert = NULL;
        }

        if (cert != NULL) {
            /* We determined the object type */
            data->object_type = OSSL_OBJECT_CERT;
            *v = OSSL_STORE_INFO_new_CERT(cert);
            if (*v == NULL)
                X509_free(cert);
        }
    }

    return 1;
}

static int try_crl(struct extracted_param_data_st *data, OSSL_STORE_INFO **v,
                   OSSL_LIB_CTX *libctx, const char *propq)
{
    if (data->object_type == OSSL_OBJECT_UNKNOWN
        || data->object_type == OSSL_OBJECT_CRL) {
        X509_CRL *crl;

        crl = d2i_X509_CRL(NULL, (const unsigned char **)&data->octet_data,
                           data->octet_data_size);

        if (crl != NULL)
            /* We determined the object type */
            data->object_type = OSSL_OBJECT_CRL;

        if (crl != NULL && !ossl_x509_crl_set0_libctx(crl, libctx, propq)) {
            X509_CRL_free(crl);
            crl = NULL;
        }

        if (crl != NULL)
            *v = OSSL_STORE_INFO_new_CRL(crl);
        if (*v == NULL)
            X509_CRL_free(crl);
    }

    return 1;
}

static int try_pkcs12(struct extracted_param_data_st *data, OSSL_STORE_INFO **v,
                      OSSL_STORE_CTX *ctx,
                      OSSL_LIB_CTX *libctx, const char *propq)
{
    int ok = 1;

    /* There is no specific object type for PKCS12 */
    if (data->object_type == OSSL_OBJECT_UNKNOWN) {
        /* Initial parsing */
        PKCS12 *p12;

        p12 = d2i_PKCS12(NULL, (const unsigned char **)&data->octet_data,
                         data->octet_data_size);

        if (p12 != NULL) {
            char *pass = NULL;
            char tpass[PEM_BUFSIZE + 1];
            size_t tpass_len;
            EVP_PKEY *pkey = NULL;
            X509 *cert = NULL;
            STACK_OF(X509) *chain = NULL;

            data->object_type = OSSL_OBJECT_PKCS12;

            ok = 0;              /* Assume decryption or parse error */

            if (!PKCS12_mac_present(p12)
                || PKCS12_verify_mac(p12, NULL, 0)) {
                pass = NULL;
            } else if (PKCS12_verify_mac(p12, "", 0)) {
                pass = "";
            } else {
                static char prompt_info[] = "PKCS12 import pass phrase";
                OSSL_PARAM pw_params[] = {
                    OSSL_PARAM_utf8_string(OSSL_PASSPHRASE_PARAM_INFO,
                                           prompt_info,
                                           sizeof(prompt_info) - 1),
                    OSSL_PARAM_END
                };

                if (!ossl_pw_get_passphrase(tpass, sizeof(tpass) - 1,
                                            &tpass_len,
                                            pw_params, 0, &ctx->pwdata)) {
                    ERR_raise(ERR_LIB_OSSL_STORE,
                              OSSL_STORE_R_PASSPHRASE_CALLBACK_ERROR);
                    goto p12_end;
                }
                pass = tpass;
                /*
                 * ossl_pw_get_passphrase() does not NUL terminate but
                 * we must do it for PKCS12_parse()
                 */
                pass[tpass_len] = '\0';
                if (!PKCS12_verify_mac(p12, pass, tpass_len)) {
                    ERR_raise_data(ERR_LIB_OSSL_STORE,
                                   OSSL_STORE_R_ERROR_VERIFYING_PKCS12_MAC,
                                   tpass_len == 0 ? "empty password" :
                                   "maybe wrong password");
                    goto p12_end;
                }
            }

            if (PKCS12_parse(p12, pass, &pkey, &cert, &chain)) {
                STACK_OF(OSSL_STORE_INFO) *infos = NULL;
                OSSL_STORE_INFO *osi_pkey = NULL;
                OSSL_STORE_INFO *osi_cert = NULL;
                OSSL_STORE_INFO *osi_ca = NULL;

                ok = 1;          /* Parsing went through correctly! */

                if ((infos = sk_OSSL_STORE_INFO_new_null()) != NULL) {
                    if (pkey != NULL) {
                        if ((osi_pkey = OSSL_STORE_INFO_new_PKEY(pkey)) != NULL
                            /* clearing pkey here avoids case distinctions */
                            && (pkey = NULL) == NULL
                            && sk_OSSL_STORE_INFO_push(infos, osi_pkey) != 0)
                            osi_pkey = NULL;
                        else
                            ok = 0;
                    }
                    if (ok && cert != NULL) {
                        if ((osi_cert = OSSL_STORE_INFO_new_CERT(cert)) != NULL
                            /* clearing cert here avoids case distinctions */
                            && (cert = NULL) == NULL
                            && sk_OSSL_STORE_INFO_push(infos, osi_cert) != 0)
                            osi_cert = NULL;
                        else
                            ok = 0;
                    }
                    while (ok && sk_X509_num(chain) > 0) {
                        X509 *ca = sk_X509_value(chain, 0);

                        if ((osi_ca = OSSL_STORE_INFO_new_CERT(ca)) != NULL
                            && sk_X509_shift(chain) != NULL
                            && sk_OSSL_STORE_INFO_push(infos, osi_ca) != 0)
                            osi_ca = NULL;
                        else
                            ok = 0;
                    }
                }
                EVP_PKEY_free(pkey);
                X509_free(cert);
                OSSL_STACK_OF_X509_free(chain);
                OSSL_STORE_INFO_free(osi_pkey);
                OSSL_STORE_INFO_free(osi_cert);
                OSSL_STORE_INFO_free(osi_ca);
                if (!ok) {
                    sk_OSSL_STORE_INFO_pop_free(infos, OSSL_STORE_INFO_free);
                    infos = NULL;
                }
                ctx->cached_info = infos;
            }
         p12_end:
            OPENSSL_cleanse(tpass, sizeof(tpass));
            PKCS12_free(p12);
        }
        *v = sk_OSSL_STORE_INFO_shift(ctx->cached_info);
    }

    return ok;
}
