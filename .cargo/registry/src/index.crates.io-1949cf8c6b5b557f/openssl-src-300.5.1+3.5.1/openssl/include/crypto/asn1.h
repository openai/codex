/*
 * Copyright 2015-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_ASN1_H
# define OSSL_CRYPTO_ASN1_H
# pragma once

# include <openssl/asn1.h>
# include <openssl/core_dispatch.h> /* OSSL_FUNC_keymgmt_import() */

/* Internal ASN1 structures and functions: not for application use */

/* ASN1 public key method structure */

#include <openssl/core.h>

struct evp_pkey_asn1_method_st {
    int pkey_id;
    int pkey_base_id;
    unsigned long pkey_flags;
    char *pem_str;
    char *info;
    int (*pub_decode) (EVP_PKEY *pk, const X509_PUBKEY *pub);
    int (*pub_encode) (X509_PUBKEY *pub, const EVP_PKEY *pk);
    int (*pub_cmp) (const EVP_PKEY *a, const EVP_PKEY *b);
    int (*pub_print) (BIO *out, const EVP_PKEY *pkey, int indent,
                      ASN1_PCTX *pctx);
    int (*priv_decode) (EVP_PKEY *pk, const PKCS8_PRIV_KEY_INFO *p8inf);
    int (*priv_encode) (PKCS8_PRIV_KEY_INFO *p8, const EVP_PKEY *pk);
    int (*priv_print) (BIO *out, const EVP_PKEY *pkey, int indent,
                       ASN1_PCTX *pctx);
    int (*pkey_size) (const EVP_PKEY *pk);
    int (*pkey_bits) (const EVP_PKEY *pk);
    int (*pkey_security_bits) (const EVP_PKEY *pk);
    int (*param_decode) (EVP_PKEY *pkey,
                         const unsigned char **pder, int derlen);
    int (*param_encode) (const EVP_PKEY *pkey, unsigned char **pder);
    int (*param_missing) (const EVP_PKEY *pk);
    int (*param_copy) (EVP_PKEY *to, const EVP_PKEY *from);
    int (*param_cmp) (const EVP_PKEY *a, const EVP_PKEY *b);
    int (*param_print) (BIO *out, const EVP_PKEY *pkey, int indent,
                        ASN1_PCTX *pctx);
    int (*sig_print) (BIO *out,
                      const X509_ALGOR *sigalg, const ASN1_STRING *sig,
                      int indent, ASN1_PCTX *pctx);
    void (*pkey_free) (EVP_PKEY *pkey);
    int (*pkey_ctrl) (EVP_PKEY *pkey, int op, long arg1, void *arg2);
    /* Legacy functions for old PEM */
    int (*old_priv_decode) (EVP_PKEY *pkey,
                            const unsigned char **pder, int derlen);
    int (*old_priv_encode) (const EVP_PKEY *pkey, unsigned char **pder);
    /* Custom ASN1 signature verification */
    int (*item_verify) (EVP_MD_CTX *ctx, const ASN1_ITEM *it, const void *data,
                        const X509_ALGOR *a, const ASN1_BIT_STRING *sig,
                        EVP_PKEY *pkey);
    int (*item_sign) (EVP_MD_CTX *ctx, const ASN1_ITEM *it, const void *data,
                      X509_ALGOR *alg1, X509_ALGOR *alg2,
                      ASN1_BIT_STRING *sig);
    int (*siginf_set) (X509_SIG_INFO *siginf, const X509_ALGOR *alg,
                       const ASN1_STRING *sig);
    /* Check */
    int (*pkey_check) (const EVP_PKEY *pk);
    int (*pkey_public_check) (const EVP_PKEY *pk);
    int (*pkey_param_check) (const EVP_PKEY *pk);
    /* Get/set raw private/public key data */
    int (*set_priv_key) (EVP_PKEY *pk, const unsigned char *priv, size_t len);
    int (*set_pub_key) (EVP_PKEY *pk, const unsigned char *pub, size_t len);
    int (*get_priv_key) (const EVP_PKEY *pk, unsigned char *priv, size_t *len);
    int (*get_pub_key) (const EVP_PKEY *pk, unsigned char *pub, size_t *len);

    /* Exports and imports to / from providers */
    size_t (*dirty_cnt) (const EVP_PKEY *pk);
    int (*export_to) (const EVP_PKEY *pk, void *to_keydata,
                      OSSL_FUNC_keymgmt_import_fn *importer,
                      OSSL_LIB_CTX *libctx, const char *propq);
    OSSL_CALLBACK *import_from;
    int (*copy) (EVP_PKEY *to, EVP_PKEY *from);

    int (*priv_decode_ex) (EVP_PKEY *pk,
                                    const PKCS8_PRIV_KEY_INFO *p8inf,
                                    OSSL_LIB_CTX *libctx,
                                    const char *propq);
} /* EVP_PKEY_ASN1_METHOD */ ;

DEFINE_STACK_OF_CONST(EVP_PKEY_ASN1_METHOD)

extern const EVP_PKEY_ASN1_METHOD ossl_dh_asn1_meth;
extern const EVP_PKEY_ASN1_METHOD ossl_dhx_asn1_meth;
extern const EVP_PKEY_ASN1_METHOD ossl_dsa_asn1_meths[4];
extern const EVP_PKEY_ASN1_METHOD ossl_eckey_asn1_meth;
extern const EVP_PKEY_ASN1_METHOD ossl_ecx25519_asn1_meth;
extern const EVP_PKEY_ASN1_METHOD ossl_ecx448_asn1_meth;
extern const EVP_PKEY_ASN1_METHOD ossl_ed25519_asn1_meth;
extern const EVP_PKEY_ASN1_METHOD ossl_ed448_asn1_meth;
extern const EVP_PKEY_ASN1_METHOD ossl_sm2_asn1_meth;

extern const EVP_PKEY_ASN1_METHOD ossl_rsa_asn1_meths[2];
extern const EVP_PKEY_ASN1_METHOD ossl_rsa_pss_asn1_meth;

/*
 * These are used internally in the ASN1_OBJECT to keep track of whether the
 * names and data need to be free()ed
 */
# define ASN1_OBJECT_FLAG_DYNAMIC         0x01/* internal use */
# define ASN1_OBJECT_FLAG_CRITICAL        0x02/* critical x509v3 object id */
# define ASN1_OBJECT_FLAG_DYNAMIC_STRINGS 0x04/* internal use */
# define ASN1_OBJECT_FLAG_DYNAMIC_DATA    0x08/* internal use */
struct asn1_object_st {
    const char *sn, *ln;
    int nid;
    int length;
    const unsigned char *data;  /* data remains const after init */
    int flags;                  /* Should we free this one */
};

/* ASN1 print context structure */

struct asn1_pctx_st {
    unsigned long flags;
    unsigned long nm_flags;
    unsigned long cert_flags;
    unsigned long oid_flags;
    unsigned long str_flags;
} /* ASN1_PCTX */ ;

/* ASN1 type functions */

int ossl_asn1_type_set_octetstring_int(ASN1_TYPE *a, long num,
                                       unsigned char *data, int len);
int ossl_asn1_type_get_octetstring_int(const ASN1_TYPE *a, long *num,
                                       unsigned char *data, int max_len);

int ossl_x509_algor_new_from_md(X509_ALGOR **palg, const EVP_MD *md);
const EVP_MD *ossl_x509_algor_get_md(X509_ALGOR *alg);
X509_ALGOR *ossl_x509_algor_mgf1_decode(X509_ALGOR *alg);
int ossl_x509_algor_md_to_mgf1(X509_ALGOR **palg, const EVP_MD *mgf1md);
int ossl_asn1_time_print_ex(BIO *bp, const ASN1_TIME *tm, unsigned long flags);

EVP_PKEY *ossl_d2i_PrivateKey_legacy(int keytype, EVP_PKEY **a,
                                     const unsigned char **pp, long length,
                                     OSSL_LIB_CTX *libctx, const char *propq);
X509_ALGOR *ossl_X509_ALGOR_from_nid(int nid, int ptype, void *pval);

void ossl_asn1_string_set_bits_left(ASN1_STRING *str, unsigned int num);

#endif /* ndef OSSL_CRYPTO_ASN1_H */
