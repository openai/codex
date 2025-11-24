/*
 * Copyright 2005-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Internal ASN1 structures and functions: not for application use */

#include "crypto/asn1.h"

typedef const ASN1_VALUE const_ASN1_VALUE;
SKM_DEFINE_STACK_OF(const_ASN1_VALUE, const ASN1_VALUE, ASN1_VALUE)

int ossl_asn1_time_to_tm(struct tm *tm, const ASN1_TIME *d);
int ossl_asn1_utctime_to_tm(struct tm *tm, const ASN1_UTCTIME *d);

/* ASN1 scan context structure */

struct asn1_sctx_st {
    /* The ASN1_ITEM associated with this field */
    const ASN1_ITEM *it;
    /* If ASN1_TEMPLATE associated with this field */
    const ASN1_TEMPLATE *tt;
    /* Various flags associated with field and context */
    unsigned long flags;
    /* If SEQUENCE OF or SET OF, field index */
    int skidx;
    /* ASN1 depth of field */
    int depth;
    /* Structure and field name */
    const char *sname, *fname;
    /* If a primitive type the type of underlying field */
    int prim_type;
    /* The field value itself */
    ASN1_VALUE **field;
    /* Callback to pass information to */
    int (*scan_cb) (ASN1_SCTX *ctx);
    /* Context specific application data */
    void *app_data;
} /* ASN1_SCTX */ ;

typedef struct mime_param_st MIME_PARAM;
DEFINE_STACK_OF(MIME_PARAM)
typedef struct mime_header_st MIME_HEADER;
DEFINE_STACK_OF(MIME_HEADER)

void ossl_asn1_string_embed_free(ASN1_STRING *a, int embed);

int ossl_asn1_get_choice_selector(ASN1_VALUE **pval, const ASN1_ITEM *it);
int ossl_asn1_get_choice_selector_const(const ASN1_VALUE **pval,
                                        const ASN1_ITEM *it);
int ossl_asn1_set_choice_selector(ASN1_VALUE **pval, int value,
                                  const ASN1_ITEM *it);

ASN1_VALUE **ossl_asn1_get_field_ptr(ASN1_VALUE **pval, const ASN1_TEMPLATE *tt);
const ASN1_VALUE **ossl_asn1_get_const_field_ptr(const ASN1_VALUE **pval,
                                                 const ASN1_TEMPLATE *tt);

const ASN1_TEMPLATE *ossl_asn1_do_adb(const ASN1_VALUE *val,
                                      const ASN1_TEMPLATE *tt,
                                      int nullerr);

int ossl_asn1_do_lock(ASN1_VALUE **pval, int op, const ASN1_ITEM *it);

void ossl_asn1_enc_init(ASN1_VALUE **pval, const ASN1_ITEM *it);
void ossl_asn1_enc_free(ASN1_VALUE **pval, const ASN1_ITEM *it);
int ossl_asn1_enc_restore(int *len, unsigned char **out, const ASN1_VALUE **pval,
                          const ASN1_ITEM *it);
int ossl_asn1_enc_save(ASN1_VALUE **pval, const unsigned char *in, int inlen,
                       const ASN1_ITEM *it);

void ossl_asn1_item_embed_free(ASN1_VALUE **pval, const ASN1_ITEM *it, int embed);
void ossl_asn1_primitive_free(ASN1_VALUE **pval, const ASN1_ITEM *it, int embed);
void ossl_asn1_template_free(ASN1_VALUE **pval, const ASN1_TEMPLATE *tt);

ASN1_OBJECT *ossl_c2i_ASN1_OBJECT(ASN1_OBJECT **a, const unsigned char **pp,
                                  long length);
int ossl_i2c_ASN1_BIT_STRING(ASN1_BIT_STRING *a, unsigned char **pp);
ASN1_BIT_STRING *ossl_c2i_ASN1_BIT_STRING(ASN1_BIT_STRING **a,
                                          const unsigned char **pp, long length);
int ossl_i2c_ASN1_INTEGER(ASN1_INTEGER *a, unsigned char **pp);
ASN1_INTEGER *ossl_c2i_ASN1_INTEGER(ASN1_INTEGER **a, const unsigned char **pp,
                                    long length);

/* Internal functions used by x_int64.c */
int ossl_c2i_uint64_int(uint64_t *ret, int *neg, const unsigned char **pp,
                        long len);
int ossl_i2c_uint64_int(unsigned char *p, uint64_t r, int neg);

ASN1_TIME *ossl_asn1_time_from_tm(ASN1_TIME *s, struct tm *ts, int type);

int ossl_asn1_item_ex_new_intern(ASN1_VALUE **pval, const ASN1_ITEM *it,
                                 OSSL_LIB_CTX *libctx, const char *propq);
