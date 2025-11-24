/*
 * Copyright 2015-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_X509_H
# define OSSL_CRYPTO_X509_H
# pragma once

# include "internal/refcount.h"
# include <openssl/asn1.h>
# include <openssl/x509.h>
# include <openssl/conf.h>
# include "crypto/types.h"

/* Internal X509 structures and functions: not for application use */

/* Note: unless otherwise stated a field pointer is mandatory and should
 * never be set to NULL: the ASN.1 code and accessors rely on mandatory
 * fields never being NULL.
 */

/*
 * name entry structure, equivalent to AttributeTypeAndValue defined
 * in RFC5280 et al.
 */
struct X509_name_entry_st {
    ASN1_OBJECT *object;        /* AttributeType */
    ASN1_STRING *value;         /* AttributeValue */
    int set;                    /* index of RDNSequence for this entry */
    int size;                   /* temp variable */
};

/* Name from RFC 5280. */
struct X509_name_st {
    STACK_OF(X509_NAME_ENTRY) *entries; /* DN components */
    int modified;               /* true if 'bytes' needs to be built */
    BUF_MEM *bytes;             /* cached encoding: cannot be NULL */
    /* canonical encoding used for rapid Name comparison */
    unsigned char *canon_enc;
    int canon_enclen;
} /* X509_NAME */ ;

/* Signature info structure */

struct x509_sig_info_st {
    /* NID of message digest */
    int mdnid;
    /* NID of public key algorithm */
    int pknid;
    /* Security bits */
    int secbits;
    /* Various flags */
    uint32_t flags;
};

/* PKCS#10 certificate request */

struct X509_req_info_st {
    ASN1_ENCODING enc;          /* cached encoding of signed part */
    ASN1_INTEGER *version;      /* version, defaults to v1(0) so can be NULL */
    X509_NAME *subject;         /* certificate request DN */
    X509_PUBKEY *pubkey;        /* public key of request */
    /*
     * Zero or more attributes.
     * NB: although attributes is a mandatory field some broken
     * encodings omit it so this may be NULL in that case.
     */
    STACK_OF(X509_ATTRIBUTE) *attributes;
};

struct X509_req_st {
    X509_REQ_INFO req_info;     /* signed certificate request data */
    X509_ALGOR sig_alg;         /* signature algorithm */
    ASN1_BIT_STRING *signature; /* signature */
    CRYPTO_REF_COUNT references;
    CRYPTO_RWLOCK *lock;

    /* Set on live certificates for authentication purposes */
    ASN1_OCTET_STRING *distinguishing_id;
    OSSL_LIB_CTX *libctx;
    char *propq;
};

struct X509_crl_info_st {
    ASN1_INTEGER *version;      /* version: defaults to v1(0) so may be NULL */
    X509_ALGOR sig_alg;         /* signature algorithm */
    X509_NAME *issuer;          /* CRL issuer name */
    ASN1_TIME *lastUpdate;      /* lastUpdate field */
    ASN1_TIME *nextUpdate;      /* nextUpdate field: optional */
    STACK_OF(X509_REVOKED) *revoked;        /* revoked entries: optional */
    STACK_OF(X509_EXTENSION) *extensions;   /* extensions: optional */
    ASN1_ENCODING enc;                      /* encoding of signed portion of CRL */
};

struct X509_crl_st {
    X509_CRL_INFO crl;          /* signed CRL data */
    X509_ALGOR sig_alg;         /* CRL signature algorithm */
    ASN1_BIT_STRING signature;  /* CRL signature */
    CRYPTO_REF_COUNT references;
    int flags;
    /*
     * Cached copies of decoded extension values, since extensions
     * are optional any of these can be NULL.
     */
    AUTHORITY_KEYID *akid;
    ISSUING_DIST_POINT *idp;
    /* Convenient breakdown of IDP */
    int idp_flags;
    int idp_reasons;
    /* CRL and base CRL numbers for delta processing */
    ASN1_INTEGER *crl_number;
    ASN1_INTEGER *base_crl_number;
    STACK_OF(GENERAL_NAMES) *issuers;
    /* hash of CRL */
    unsigned char sha1_hash[SHA_DIGEST_LENGTH];
    /* alternative method to handle this CRL */
    const X509_CRL_METHOD *meth;
    void *meth_data;
    CRYPTO_RWLOCK *lock;

    OSSL_LIB_CTX *libctx;
    char *propq;
};

struct x509_revoked_st {
    ASN1_INTEGER serialNumber; /* revoked entry serial number */
    ASN1_TIME *revocationDate;  /* revocation date */
    STACK_OF(X509_EXTENSION) *extensions;   /* CRL entry extensions: optional */
    /* decoded value of CRLissuer extension: set if indirect CRL */
    STACK_OF(GENERAL_NAME) *issuer;
    /* revocation reason: set to CRL_REASON_NONE if reason extension absent */
    int reason;
    /*
     * CRL entries are reordered for faster lookup of serial numbers. This
     * field contains the original load sequence for this entry.
     */
    int sequence;
};

/*
 * This stuff is certificate "auxiliary info": it contains details which are
 * useful in certificate stores and databases. When used this is tagged onto
 * the end of the certificate itself. OpenSSL specific structure not defined
 * in any RFC.
 */

struct x509_cert_aux_st {
    STACK_OF(ASN1_OBJECT) *trust; /* trusted uses */
    STACK_OF(ASN1_OBJECT) *reject; /* rejected uses */
    ASN1_UTF8STRING *alias;     /* "friendly name" */
    ASN1_OCTET_STRING *keyid;   /* key id of private key */
    STACK_OF(X509_ALGOR) *other; /* other unspecified info */
};

struct x509_cinf_st {
    ASN1_INTEGER *version;      /* [ 0 ] default of v1 */
    ASN1_INTEGER serialNumber;
    X509_ALGOR signature;
    X509_NAME *issuer;
    X509_VAL validity;
    X509_NAME *subject;
    X509_PUBKEY *key;
    ASN1_BIT_STRING *issuerUID; /* [ 1 ] optional in v2 */
    ASN1_BIT_STRING *subjectUID; /* [ 2 ] optional in v2 */
    STACK_OF(X509_EXTENSION) *extensions; /* [ 3 ] optional in v3 */
    ASN1_ENCODING enc;
};

struct x509_st {
    X509_CINF cert_info;
    X509_ALGOR sig_alg;
    ASN1_BIT_STRING signature;
    X509_SIG_INFO siginf;
    CRYPTO_REF_COUNT references;
    CRYPTO_EX_DATA ex_data;
    /* These contain copies of various extension values */
    long ex_pathlen;
    long ex_pcpathlen;
    uint32_t ex_flags;
    uint32_t ex_kusage;
    uint32_t ex_xkusage;
    uint32_t ex_nscert;
    ASN1_OCTET_STRING *skid;
    AUTHORITY_KEYID *akid;
    X509_POLICY_CACHE *policy_cache;
    STACK_OF(DIST_POINT) *crldp;
    STACK_OF(GENERAL_NAME) *altname;
    NAME_CONSTRAINTS *nc;
# ifndef OPENSSL_NO_RFC3779
    STACK_OF(IPAddressFamily) *rfc3779_addr;
    struct ASIdentifiers_st *rfc3779_asid;
# endif
    unsigned char sha1_hash[SHA_DIGEST_LENGTH];
    X509_CERT_AUX *aux;
    CRYPTO_RWLOCK *lock;
    volatile int ex_cached;

    /* Set on live certificates for authentication purposes */
    ASN1_OCTET_STRING *distinguishing_id;

    OSSL_LIB_CTX *libctx;
    char *propq;
} /* X509 */ ;

/*
 * This is a used when verifying cert chains.  Since the gathering of the
 * cert chain can take some time (and have to be 'retried', this needs to be
 * kept and passed around.
 */
struct x509_store_ctx_st {      /* X509_STORE_CTX */
    X509_STORE *store;
    /* The following are set by the caller */
    /* The cert to check */
    X509 *cert;
    /* chain of X509s - untrusted - passed in */
    STACK_OF(X509) *untrusted;
    /* set of CRLs passed in */
    STACK_OF(X509_CRL) *crls;
    X509_VERIFY_PARAM *param;
    /* Other info for use with get_issuer() */
    void *other_ctx;
    /* Callbacks for various operations */
    /* called to verify a certificate */
    int (*verify) (X509_STORE_CTX *ctx);
    /* error callback */
    int (*verify_cb) (int ok, X509_STORE_CTX *ctx);
    /* get issuers cert from ctx */
    int (*get_issuer) (X509 **issuer, X509_STORE_CTX *ctx, X509 *x);
    /* check issued */
    int (*check_issued) (X509_STORE_CTX *ctx, X509 *x, X509 *issuer);
    /* Check revocation status of chain */
    int (*check_revocation) (X509_STORE_CTX *ctx);
    /* retrieve CRL */
    int (*get_crl) (X509_STORE_CTX *ctx, X509_CRL **crl, X509 *x);
    /* Check CRL validity */
    int (*check_crl) (X509_STORE_CTX *ctx, X509_CRL *crl);
    /* Check certificate against CRL */
    int (*cert_crl) (X509_STORE_CTX *ctx, X509_CRL *crl, X509 *x);
    /* Check policy status of the chain */
    int (*check_policy) (X509_STORE_CTX *ctx);
    STACK_OF(X509) *(*lookup_certs) (X509_STORE_CTX *ctx,
                                     const X509_NAME *nm);
    /* cannot constify 'ctx' param due to lookup_certs_sk() in x509_vfy.c */
    STACK_OF(X509_CRL) *(*lookup_crls) (const X509_STORE_CTX *ctx,
                                        const X509_NAME *nm);
    int (*cleanup) (X509_STORE_CTX *ctx);
    /* The following is built up */
    /* if 0, rebuild chain */
    int valid;
    /* number of untrusted certs */
    int num_untrusted;
    /* chain of X509s - built up and trusted */
    STACK_OF(X509) *chain;
    /* Valid policy tree */
    X509_POLICY_TREE *tree;
    /* Require explicit policy value */
    int explicit_policy;
    /* When something goes wrong, this is why */
    int error_depth;
    int error;
    X509 *current_cert;
    /* cert currently being tested as valid issuer */
    X509 *current_issuer;
    /* current CRL */
    X509_CRL *current_crl;
    /* score of current CRL */
    int current_crl_score;
    /* Reason mask */
    unsigned int current_reasons;
    /* For CRL path validation: parent context */
    X509_STORE_CTX *parent;
    CRYPTO_EX_DATA ex_data;
    SSL_DANE *dane;
    /* signed via bare TA public key, rather than CA certificate */
    int bare_ta_signed;
    /* Raw Public Key */
    EVP_PKEY *rpk;

    OSSL_LIB_CTX *libctx;
    char *propq;
};

/* PKCS#8 private key info structure */

struct pkcs8_priv_key_info_st {
    ASN1_INTEGER *version;
    X509_ALGOR *pkeyalg;
    ASN1_OCTET_STRING *pkey;
    STACK_OF(X509_ATTRIBUTE) *attributes;
    ASN1_OCTET_STRING *kpub;
};

struct X509_sig_st {
    X509_ALGOR *algor;
    ASN1_OCTET_STRING *digest;
};

struct x509_object_st {
    /* one of the above types */
    X509_LOOKUP_TYPE type;
    union {
        X509 *x509;
        X509_CRL *crl;
    } data;
};

int ossl_a2i_ipadd(unsigned char *ipout, const char *ipasc);
int ossl_x509_set1_time(int *modified, ASN1_TIME **ptm, const ASN1_TIME *tm);
int ossl_x509_print_ex_brief(BIO *bio, X509 *cert, unsigned long neg_cflags);
int ossl_x509v3_cache_extensions(X509 *x);
int ossl_x509_init_sig_info(X509 *x);

int ossl_x509_set0_libctx(X509 *x, OSSL_LIB_CTX *libctx, const char *propq);
int ossl_x509_crl_set0_libctx(X509_CRL *x, OSSL_LIB_CTX *libctx,
                              const char *propq);
int ossl_x509_req_set0_libctx(X509_REQ *x, OSSL_LIB_CTX *libctx,
                              const char *propq);
int ossl_asn1_item_digest_ex(const ASN1_ITEM *it, const EVP_MD *type,
                             void *data, unsigned char *md, unsigned int *len,
                             OSSL_LIB_CTX *libctx, const char *propq);
int ossl_x509_add_cert_new(STACK_OF(X509) **sk, X509 *cert, int flags);
int ossl_x509_add_certs_new(STACK_OF(X509) **p_sk, STACK_OF(X509) *certs,
                            int flags);

STACK_OF(X509_ATTRIBUTE) *ossl_x509at_dup(const STACK_OF(X509_ATTRIBUTE) *x);

int ossl_x509_PUBKEY_get0_libctx(OSSL_LIB_CTX **plibctx, const char **ppropq,
                                 const X509_PUBKEY *key);
/* Calculate default key identifier according to RFC 5280 section 4.2.1.2 (1) */
ASN1_OCTET_STRING *ossl_x509_pubkey_hash(X509_PUBKEY *pubkey);

X509_PUBKEY *ossl_d2i_X509_PUBKEY_INTERNAL(const unsigned char **pp,
                                           long len, OSSL_LIB_CTX *libctx,
                                           const char *propq);
void ossl_X509_PUBKEY_INTERNAL_free(X509_PUBKEY *xpub);

RSA *ossl_d2i_RSA_PSS_PUBKEY(RSA **a, const unsigned char **pp, long length);
int ossl_i2d_RSA_PSS_PUBKEY(const RSA *a, unsigned char **pp);
# ifndef OPENSSL_NO_DSA
DSA *ossl_d2i_DSA_PUBKEY(DSA **a, const unsigned char **pp, long length);
# endif /* OPENSSL_NO_DSA */
# ifndef OPENSSL_NO_DH
DH *ossl_d2i_DH_PUBKEY(DH **a, const unsigned char **pp, long length);
int ossl_i2d_DH_PUBKEY(const DH *a, unsigned char **pp);
DH *ossl_d2i_DHx_PUBKEY(DH **a, const unsigned char **pp, long length);
int ossl_i2d_DHx_PUBKEY(const DH *a, unsigned char **pp);
# endif /* OPENSSL_NO_DH */
# ifndef OPENSSL_NO_EC
ECX_KEY *ossl_d2i_ED25519_PUBKEY(ECX_KEY **a,
                                 const unsigned char **pp, long length);
int ossl_i2d_ED25519_PUBKEY(const ECX_KEY *a, unsigned char **pp);
ECX_KEY *ossl_d2i_ED448_PUBKEY(ECX_KEY **a,
                               const unsigned char **pp, long length);
int ossl_i2d_ED448_PUBKEY(const ECX_KEY *a, unsigned char **pp);
ECX_KEY *ossl_d2i_X25519_PUBKEY(ECX_KEY **a,
                                const unsigned char **pp, long length);
int ossl_i2d_X25519_PUBKEY(const ECX_KEY *a, unsigned char **pp);
ECX_KEY *ossl_d2i_X448_PUBKEY(ECX_KEY **a,
                              const unsigned char **pp, long length);
int ossl_i2d_X448_PUBKEY(const ECX_KEY *a, unsigned char **pp);
# endif /* OPENSSL_NO_EC */

EVP_PKEY *ossl_d2i_PUBKEY_legacy(EVP_PKEY **a, const unsigned char **pp,
                                 long length);
int ossl_x509_check_private_key(const EVP_PKEY *k, const EVP_PKEY *pkey);

int x509v3_add_len_value_uchar(const char *name, const unsigned char *value,
                               size_t vallen, STACK_OF(CONF_VALUE) **extlist);
/* Attribute addition functions not checking for duplicate attributes */
STACK_OF(X509_ATTRIBUTE) *ossl_x509at_add1_attr(STACK_OF(X509_ATTRIBUTE) **x,
                                                const X509_ATTRIBUTE *attr);
STACK_OF(X509_ATTRIBUTE) *ossl_x509at_add1_attr_by_OBJ(STACK_OF(X509_ATTRIBUTE) **x,
                                                       const ASN1_OBJECT *obj,
                                                       int type,
                                                       const unsigned char *bytes,
                                                       int len);
STACK_OF(X509_ATTRIBUTE) *ossl_x509at_add1_attr_by_NID(STACK_OF(X509_ATTRIBUTE) **x,
                                                       int nid, int type,
                                                       const unsigned char *bytes,
                                                       int len);
STACK_OF(X509_ATTRIBUTE) *ossl_x509at_add1_attr_by_txt(STACK_OF(X509_ATTRIBUTE) **x,
                                                       const char *attrname,
                                                       int type,
                                                       const unsigned char *bytes,
                                                       int len);
                                            
int ossl_print_attribute_value(BIO *out,
                               int obj_nid,
                               const ASN1_TYPE *av,
                               int indent);

int ossl_serial_number_print(BIO *out, const ASN1_INTEGER *bs, int indent);
int ossl_bio_print_hex(BIO *out, unsigned char *buf, int len);

#endif  /* OSSL_CRYPTO_X509_H */
