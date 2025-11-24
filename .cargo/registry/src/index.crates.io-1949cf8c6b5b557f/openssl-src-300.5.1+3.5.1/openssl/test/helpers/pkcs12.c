/*
 * Copyright 2020-2024 The OpenSSL Project Authors. All Rights Reserved.
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
#include "pkcs12.h" /* from the same directory */

/* Set this to > 0 write test data to file */
static int write_files = 0;

static int legacy = 0;

static OSSL_LIB_CTX *test_ctx = NULL;
static const char *test_propq = NULL;

/* -------------------------------------------------------------------------
 * Local function declarations
 */

static int add_attributes(PKCS12_SAFEBAG *bag, const PKCS12_ATTR *attrs);

static void generate_p12(PKCS12_BUILDER *pb, const PKCS12_ENC *mac);
static int write_p12(PKCS12 *p12, const char *outfile);

static PKCS12 *from_bio_p12(BIO *bio, const PKCS12_ENC *mac);
static PKCS12 *read_p12(const char *infile, const PKCS12_ENC *mac);
static int check_p12_mac(PKCS12 *p12, const PKCS12_ENC *mac);
static int check_asn1_string(const ASN1_TYPE *av, const char *txt);
static int check_attrs(const STACK_OF(X509_ATTRIBUTE) *bag_attrs, const PKCS12_ATTR *attrs);


/* --------------------------------------------------------------------------
 * Global settings
 */

void PKCS12_helper_set_write_files(int enable)
{
    write_files = enable;
}

void PKCS12_helper_set_legacy(int enable)
{
    legacy = enable;
}

void PKCS12_helper_set_libctx(OSSL_LIB_CTX *libctx)
{
    test_ctx = libctx;
}

void PKCS12_helper_set_propq(const char *propq)
{
    test_propq = propq;
}


/* --------------------------------------------------------------------------
 * Test data load functions
 */

static X509 *load_cert_asn1(const unsigned char *bytes, int len)
{
    X509 *cert = NULL;

    cert = d2i_X509(NULL, &bytes, len);
    if (!TEST_ptr(cert))
        goto err;
err:
    return cert;
}

static EVP_PKEY *load_pkey_asn1(const unsigned char *bytes, int len)
{
    EVP_PKEY *pkey = NULL;

    pkey = d2i_AutoPrivateKey(NULL, &bytes, len);
    if (!TEST_ptr(pkey))
        goto err;
err:
    return pkey;
}

/* -------------------------------------------------------------------------
 * PKCS12 builder
 */

PKCS12_BUILDER *new_pkcs12_builder(const char *filename)
{
    PKCS12_BUILDER *pb = OPENSSL_malloc(sizeof(PKCS12_BUILDER));
    if (!TEST_ptr(pb))
        return NULL;

    pb->filename = filename;
    pb->success = 1;
    return pb;
}

int end_pkcs12_builder(PKCS12_BUILDER *pb)
{
    int result = pb->success;

    OPENSSL_free(pb);
    return result;
}


void start_pkcs12(PKCS12_BUILDER *pb)
{
    pb->safes = NULL;
}


void end_pkcs12(PKCS12_BUILDER *pb)
{
    if (!pb->success)
        return;
    generate_p12(pb, NULL);
}


void end_pkcs12_with_mac(PKCS12_BUILDER *pb, const PKCS12_ENC *mac)
{
    if (!pb->success)
        return;
    generate_p12(pb, mac);
}


/* Generate the PKCS12 encoding and write to memory bio */
static void generate_p12(PKCS12_BUILDER *pb, const PKCS12_ENC *mac)
{
    PKCS12 *p12;
    EVP_MD *md = NULL;

    if (!pb->success)
        return;

    pb->p12bio = BIO_new(BIO_s_mem());
    if (!TEST_ptr(pb->p12bio)) {
        pb->success = 0;
        return;
    }
    if (legacy)
        p12 = PKCS12_add_safes(pb->safes, 0);
    else
        p12 = PKCS12_add_safes_ex(pb->safes, 0, test_ctx, test_propq);
    if (!TEST_ptr(p12)) {
        pb->success = 0;
        goto err;
    }
    sk_PKCS7_pop_free(pb->safes, PKCS7_free);

    if (mac != NULL) {
        if (legacy)
            md = (EVP_MD *)EVP_get_digestbynid(mac->nid);
        else
            md = EVP_MD_fetch(test_ctx, OBJ_nid2sn(mac->nid), test_propq);

        if (!TEST_true(PKCS12_set_mac(p12, mac->pass, strlen(mac->pass),
                                      NULL, 0, mac->iter, md))) {
            pb->success = 0;
            goto err;
        }
    }
    i2d_PKCS12_bio(pb->p12bio, p12);

    /* Can write to file here for debug */
    if (write_files)
        write_p12(p12, pb->filename);
err:
    if (!legacy && md != NULL)
        EVP_MD_free(md);
    PKCS12_free(p12);
}


static int write_p12(PKCS12 *p12, const char *outfile)
{
    int ret = 0;
    BIO *out = BIO_new_file(outfile, "w");

    if (out == NULL)
        goto err;

    if (!TEST_int_eq(i2d_PKCS12_bio(out, p12), 1))
        goto err;
    ret = 1;
err:
    BIO_free(out);
    return ret;
}

static PKCS12 *from_bio_p12(BIO *bio, const PKCS12_ENC *mac)
{
    PKCS12 *p12 = NULL;

    /* Supply a p12 with library context/propq to the d2i decoder*/
    if (!legacy) {
        p12 = PKCS12_init_ex(NID_pkcs7_data, test_ctx, test_propq);
        if (!TEST_ptr(p12))
            goto err;
    }
    p12 = d2i_PKCS12_bio(bio, &p12);
    BIO_free(bio);
    if (!TEST_ptr(p12))
        goto err;
    if (mac == NULL) {
        if (!TEST_false(PKCS12_mac_present(p12)))
            goto err;
    } else {
        if (!check_p12_mac(p12, mac))
            goto err;
    }
    return p12;
err:
    PKCS12_free(p12);
    return NULL;
}


/* For use with existing files */
static PKCS12 *read_p12(const char *infile, const PKCS12_ENC *mac)
{
    PKCS12 *p12 = NULL;
    BIO *in = BIO_new_file(infile, "r");

    if (in == NULL)
        goto err;
    p12 = d2i_PKCS12_bio(in, NULL);
    BIO_free(in);
    if (!TEST_ptr(p12))
        goto err;
    if (mac == NULL) {
        if (!TEST_false(PKCS12_mac_present(p12)))
            goto err;
    } else {
        if (!check_p12_mac(p12, mac))
            goto err;
    }
    return p12;
err:
    PKCS12_free(p12);
    return NULL;
}

static int check_p12_mac(PKCS12 *p12, const PKCS12_ENC *mac)
{
    return TEST_true(PKCS12_mac_present(p12))
        && TEST_true(PKCS12_verify_mac(p12, mac->pass, strlen(mac->pass)));
}


/* -------------------------------------------------------------------------
 * PKCS7 content info builder
 */

void start_contentinfo(PKCS12_BUILDER *pb)
{
    pb->bags = NULL;
}


void end_contentinfo(PKCS12_BUILDER *pb)
{
    if (pb->success && pb->bags != NULL) {
        if (!TEST_true(PKCS12_add_safe(&pb->safes, pb->bags, -1, 0, NULL)))
            pb->success = 0;
    }
    sk_PKCS12_SAFEBAG_pop_free(pb->bags, PKCS12_SAFEBAG_free);
    pb->bags = NULL;
}


void end_contentinfo_encrypted(PKCS12_BUILDER *pb, const PKCS12_ENC *enc)
{
    if (pb->success && pb->bags != NULL) {
        if (legacy) {
            if (!TEST_true(PKCS12_add_safe(&pb->safes, pb->bags, enc->nid,
                                           enc->iter, enc->pass)))
                pb->success = 0;
        } else {
            if (!TEST_true(PKCS12_add_safe_ex(&pb->safes, pb->bags, enc->nid,
                                              enc->iter, enc->pass, test_ctx,
                                              test_propq)))
                pb->success = 0;
        }
    }
    sk_PKCS12_SAFEBAG_pop_free(pb->bags, PKCS12_SAFEBAG_free);
    pb->bags = NULL;
}


static STACK_OF(PKCS12_SAFEBAG) *decode_contentinfo(STACK_OF(PKCS7) *safes, int idx, const PKCS12_ENC *enc)
{
    STACK_OF(PKCS12_SAFEBAG) *bags = NULL;
    int bagnid;
    PKCS7 *p7 = sk_PKCS7_value(safes, idx);

    if (!TEST_ptr(p7))
        goto err;

    bagnid = OBJ_obj2nid(p7->type);
    if (enc) {
        if (!TEST_int_eq(bagnid, NID_pkcs7_encrypted))
            goto err;
        bags = PKCS12_unpack_p7encdata(p7, enc->pass, strlen(enc->pass));
    } else {
        if (!TEST_int_eq(bagnid, NID_pkcs7_data))
            goto err;
        bags = PKCS12_unpack_p7data(p7);
    }
    if (!TEST_ptr(bags))
        goto err;

    return bags;
err:
    return NULL;
}


/* -------------------------------------------------------------------------
 * PKCS12 safeBag/attribute builder
 */

static int add_attributes(PKCS12_SAFEBAG *bag, const PKCS12_ATTR *attr)
{
    int ret = 0;
    int attr_nid;
    const PKCS12_ATTR *p_attr = attr;
    STACK_OF(X509_ATTRIBUTE)* attrs = NULL;
    X509_ATTRIBUTE *x509_attr = NULL;

    if (attr == NULL)
        return 1;

    while (p_attr->oid != NULL) {
        TEST_info("Adding attribute %s = %s", p_attr->oid, p_attr->value);
        attr_nid = OBJ_txt2nid(p_attr->oid);

        if (attr_nid == NID_friendlyName) {
            if (!TEST_true(PKCS12_add_friendlyname(bag, p_attr->value, -1)))
                goto err;
        } else if (attr_nid == NID_localKeyID) {
            if (!TEST_true(PKCS12_add_localkeyid(bag, (unsigned char *)p_attr->value,
                                                 strlen(p_attr->value))))
                goto err;
        } else if (attr_nid == NID_oracle_jdk_trustedkeyusage) {
            attrs = (STACK_OF(X509_ATTRIBUTE)*)PKCS12_SAFEBAG_get0_attrs(bag);
            x509_attr = X509_ATTRIBUTE_create(attr_nid, V_ASN1_OBJECT, OBJ_txt2obj(p_attr->value, 0));
            X509at_add1_attr(&attrs, x509_attr);
            PKCS12_SAFEBAG_set0_attrs(bag, attrs);
            X509_ATTRIBUTE_free(x509_attr);
        } else {
            /* Custom attribute values limited to ASCII in these tests */
            if (!TEST_true(PKCS12_add1_attr_by_txt(bag, p_attr->oid, MBSTRING_ASC,
                                                   (unsigned char *)p_attr->value,
                                                   strlen(p_attr->value))))
                goto err;
        }
        p_attr++;
    }
    ret = 1;
err:
    return ret;
}

void add_certbag(PKCS12_BUILDER *pb, const unsigned char *bytes, int len,
                 const PKCS12_ATTR *attrs)
{
    PKCS12_SAFEBAG *bag = NULL;
    X509 *cert = NULL;
    char *name;

    if (!pb->success)
        return;

    cert = load_cert_asn1(bytes, len);
    if (!TEST_ptr(cert)) {
        pb->success = 0;
        return;
    }

    name = X509_NAME_oneline(X509_get_subject_name(cert), NULL, 0);
    TEST_info("Adding certificate <%s>", name);
    OPENSSL_free(name);

    bag = PKCS12_add_cert(&pb->bags, cert);
    if (!TEST_ptr(bag)) {
        pb->success = 0;
        goto err;
    }

    if (!TEST_true(add_attributes(bag, attrs))) {
        pb->success = 0;
        goto err;
    }
err:
    X509_free(cert);
}

void add_keybag(PKCS12_BUILDER *pb, const unsigned char *bytes, int len,
                const PKCS12_ATTR *attrs, const PKCS12_ENC *enc)
{
    PKCS12_SAFEBAG *bag = NULL;
    EVP_PKEY *pkey = NULL;

    if (!pb->success)
        return;

    TEST_info("Adding key");

    pkey = load_pkey_asn1(bytes, len);
    if (!TEST_ptr(pkey)) {
        pb->success = 0;
        return;
    }

    if (legacy)
        bag = PKCS12_add_key(&pb->bags, pkey, 0 /*keytype*/, enc->iter, enc->nid, enc->pass);
    else
        bag = PKCS12_add_key_ex(&pb->bags, pkey, 0 /*keytype*/, enc->iter, enc->nid, enc->pass,
                                test_ctx, test_propq);
    if (!TEST_ptr(bag)) {
        pb->success = 0;
        goto err;
    }
    if (!add_attributes(bag, attrs))
        pb->success = 0;
err:
    EVP_PKEY_free(pkey);
}

void add_secretbag(PKCS12_BUILDER *pb, int secret_nid, const char *secret,
                               const PKCS12_ATTR *attrs)
{
    PKCS12_SAFEBAG *bag = NULL;

    if (!pb->success)
        return;

    TEST_info("Adding secret <%s>", secret);

    bag = PKCS12_add_secret(&pb->bags, secret_nid, (const unsigned char *)secret, strlen(secret));
    if (!TEST_ptr(bag)) {
        pb->success = 0;
        return;
    }
    if (!add_attributes(bag, attrs))
        pb->success = 0;
}


/* -------------------------------------------------------------------------
 * PKCS12 structure checking
 */

static int check_asn1_string(const ASN1_TYPE *av, const char *txt)
{
    int ret = 0;
    char *value = NULL;

    if (!TEST_ptr(av))
        goto err;

    switch (av->type) {
    case V_ASN1_BMPSTRING:
        value = OPENSSL_uni2asc(av->value.bmpstring->data,
                                av->value.bmpstring->length);
        if (!TEST_str_eq(txt, (char *)value))
            goto err;
        break;

    case V_ASN1_UTF8STRING:
        if (!TEST_mem_eq(txt, strlen(txt), (char *)av->value.utf8string->data,
                         av->value.utf8string->length))
            goto err;
        break;

    case V_ASN1_OCTET_STRING:
        if (!TEST_mem_eq(txt, strlen(txt),
                         (char *)av->value.octet_string->data,
                         av->value.octet_string->length))
            goto err;
        break;

    default:
        /* Tests do not support other attribute types currently */
        goto err;
    }
    ret = 1;
err:
    OPENSSL_free(value);
    return ret;
}

static int check_attrs(const STACK_OF(X509_ATTRIBUTE) *bag_attrs, const PKCS12_ATTR *attrs)
{
    int ret = 0;
    X509_ATTRIBUTE *attr;
    ASN1_TYPE *av;
    int i, j;
    char attr_txt[100];

    for (i = 0; i < sk_X509_ATTRIBUTE_num(bag_attrs); i++) {
        const PKCS12_ATTR *p_attr = attrs;
        ASN1_OBJECT *attr_obj;

        attr = sk_X509_ATTRIBUTE_value(bag_attrs, i);
        attr_obj = X509_ATTRIBUTE_get0_object(attr);
        OBJ_obj2txt(attr_txt, 100, attr_obj, 0);

        while (p_attr->oid != NULL) {
            /* Find a matching attribute type */
            if (strcmp(p_attr->oid, attr_txt) == 0) {
                if (!TEST_int_eq(X509_ATTRIBUTE_count(attr), 1))
                    goto err;

                for (j = 0; j < X509_ATTRIBUTE_count(attr); j++) {
                    av = X509_ATTRIBUTE_get0_type(attr, j);
                    if (!TEST_true(check_asn1_string(av, p_attr->value)))
                        goto err;
                }
                break;
            }
            p_attr++;
        }
    }
    ret = 1;
err:
    return ret;
}

void check_certbag(PKCS12_BUILDER *pb, const unsigned char *bytes, int len,
                   const PKCS12_ATTR *attrs)
{
    X509 *x509 = NULL;
    X509 *ref_x509 = NULL;
    const PKCS12_SAFEBAG *bag;

    if (!pb->success)
        return;

    bag = sk_PKCS12_SAFEBAG_value(pb->bags, pb->bag_idx++);
    if (!TEST_ptr(bag)) {
        pb->success = 0;
        return;
    }
    if (!check_attrs(PKCS12_SAFEBAG_get0_attrs(bag), attrs)
        || !TEST_int_eq(PKCS12_SAFEBAG_get_nid(bag), NID_certBag)
        || !TEST_int_eq(PKCS12_SAFEBAG_get_bag_nid(bag), NID_x509Certificate)) {
        pb->success = 0;
        return;
    }
    x509 = PKCS12_SAFEBAG_get1_cert(bag);
    if (!TEST_ptr(x509)) {
        pb->success = 0;
        goto err;
    }
    ref_x509 = load_cert_asn1(bytes, len);
    if (!TEST_false(X509_cmp(x509, ref_x509)))
        pb->success = 0;
err:
    X509_free(x509);
    X509_free(ref_x509);
}

void check_keybag(PKCS12_BUILDER *pb, const unsigned char *bytes, int len,
                  const PKCS12_ATTR *attrs, const PKCS12_ENC *enc)
{
    EVP_PKEY *pkey = NULL;
    EVP_PKEY *ref_pkey = NULL;
    PKCS8_PRIV_KEY_INFO *p8;
    const PKCS8_PRIV_KEY_INFO *p8c;
    const PKCS12_SAFEBAG *bag;

    if (!pb->success)
        return;

    bag = sk_PKCS12_SAFEBAG_value(pb->bags, pb->bag_idx++);
    if (!TEST_ptr(bag)) {
        pb->success = 0;
        return;
    }

    if (!check_attrs(PKCS12_SAFEBAG_get0_attrs(bag), attrs)) {
        pb->success = 0;
        return;
    }

    switch (PKCS12_SAFEBAG_get_nid(bag)) {
    case NID_keyBag:
        p8c = PKCS12_SAFEBAG_get0_p8inf(bag);
        if (!TEST_ptr(pkey = EVP_PKCS82PKEY(p8c))) {
            pb->success = 0;
            goto err;
        }
        break;

    case NID_pkcs8ShroudedKeyBag:
        if (legacy)
            p8 = PKCS12_decrypt_skey(bag, enc->pass, strlen(enc->pass));
        else
            p8 = PKCS12_decrypt_skey_ex(bag, enc->pass, strlen(enc->pass), test_ctx, test_propq);
        if (!TEST_ptr(p8)) {
            pb->success = 0;
            goto err;
        }
        if (!TEST_ptr(pkey = EVP_PKCS82PKEY(p8))) {
            PKCS8_PRIV_KEY_INFO_free(p8);
            pb->success = 0;
            goto err;
        }
        PKCS8_PRIV_KEY_INFO_free(p8);
        break;

    default:
        pb->success = 0;
        goto err;
    }

    /* PKEY compare returns 1 for match */
    ref_pkey = load_pkey_asn1(bytes, len);
    if (!TEST_true(EVP_PKEY_eq(pkey, ref_pkey)))
        pb->success = 0;
err:
    EVP_PKEY_free(pkey);
    EVP_PKEY_free(ref_pkey);
}

void check_secretbag(PKCS12_BUILDER *pb, int secret_nid, const char *secret, const PKCS12_ATTR *attrs)
{
    const PKCS12_SAFEBAG *bag;

    if (!pb->success)
        return;

    bag = sk_PKCS12_SAFEBAG_value(pb->bags, pb->bag_idx++);
    if (!TEST_ptr(bag)) {
        pb->success = 0;
        return;
    }

    if (!check_attrs(PKCS12_SAFEBAG_get0_attrs(bag), attrs)
        || !TEST_int_eq(PKCS12_SAFEBAG_get_nid(bag), NID_secretBag)
        || !TEST_int_eq(PKCS12_SAFEBAG_get_bag_nid(bag), secret_nid)
        || !TEST_true(check_asn1_string(PKCS12_SAFEBAG_get0_bag_obj(bag), secret)))
        pb->success = 0;
}


void start_check_pkcs12(PKCS12_BUILDER *pb)
{
    PKCS12 *p12;

    if (!pb->success)
        return;

    p12 = from_bio_p12(pb->p12bio, NULL);
    if (!TEST_ptr(p12)) {
        pb->success = 0;
        return;
    }
    pb->safes = PKCS12_unpack_authsafes(p12);
    if (!TEST_ptr(pb->safes))
        pb->success = 0;

    pb->safe_idx = 0;
    PKCS12_free(p12);
}

void start_check_pkcs12_with_mac(PKCS12_BUILDER *pb, const PKCS12_ENC *mac)
{
    PKCS12 *p12;

    if (!pb->success)
        return;

    p12 = from_bio_p12(pb->p12bio, mac);
    if (!TEST_ptr(p12)) {
        pb->success = 0;
        return;
    }
    pb->safes = PKCS12_unpack_authsafes(p12);
    if (!TEST_ptr(pb->safes))
        pb->success = 0;

    pb->safe_idx = 0;
    PKCS12_free(p12);
}

void start_check_pkcs12_file(PKCS12_BUILDER *pb)
{
    PKCS12 *p12;

    if (!pb->success)
        return;

    p12 = read_p12(pb->filename, NULL);
    if (!TEST_ptr(p12)) {
        pb->success = 0;
        return;
    }
    pb->safes = PKCS12_unpack_authsafes(p12);
    if (!TEST_ptr(pb->safes))
        pb->success = 0;

    pb->safe_idx = 0;
    PKCS12_free(p12);
}

void start_check_pkcs12_file_with_mac(PKCS12_BUILDER *pb, const PKCS12_ENC *mac)
{
    PKCS12 *p12;

    if (!pb->success)
        return;

    p12 = read_p12(pb->filename, mac);
    if (!TEST_ptr(p12)) {
        pb->success = 0;
        return;
    }
    pb->safes = PKCS12_unpack_authsafes(p12);
    if (!TEST_ptr(pb->safes))
        pb->success = 0;

    pb->safe_idx = 0;
    PKCS12_free(p12);
}

void end_check_pkcs12(PKCS12_BUILDER *pb)
{
    if (!pb->success)
        return;

    sk_PKCS7_pop_free(pb->safes, PKCS7_free);
}


void start_check_contentinfo(PKCS12_BUILDER *pb)
{
    if (!pb->success)
        return;

    pb->bag_idx = 0;
    pb->bags = decode_contentinfo(pb->safes, pb->safe_idx++, NULL);
    if (!TEST_ptr(pb->bags)) {
        pb->success = 0;
        return;
    }
    TEST_info("Decoding %d bags", sk_PKCS12_SAFEBAG_num(pb->bags));
}

void start_check_contentinfo_encrypted(PKCS12_BUILDER *pb, const PKCS12_ENC *enc)
{
    if (!pb->success)
        return;

    pb->bag_idx = 0;
    pb->bags = decode_contentinfo(pb->safes, pb->safe_idx++, enc);
    if (!TEST_ptr(pb->bags)) {
        pb->success = 0;
        return;
    }
    TEST_info("Decoding %d bags", sk_PKCS12_SAFEBAG_num(pb->bags));
}


void end_check_contentinfo(PKCS12_BUILDER *pb)
{
    if (!pb->success)
        return;

    if (!TEST_int_eq(sk_PKCS12_SAFEBAG_num(pb->bags), pb->bag_idx))
        pb->success = 0;
    sk_PKCS12_SAFEBAG_pop_free(pb->bags, PKCS12_SAFEBAG_free);
    pb->bags = NULL;
}


