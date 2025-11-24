/*
 * Copyright 2000-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stddef.h>
#include <string.h>
#include "internal/cryptlib.h"
#include "internal/refcount.h"
#include <openssl/asn1.h>
#include <openssl/asn1t.h>
#include <openssl/objects.h>
#include <openssl/err.h>
#include "asn1_local.h"

/* Utility functions for manipulating fields and offsets */

/* Add 'offset' to 'addr' */
#define offset2ptr(addr, offset) (void *)(((char *) addr) + offset)

/*
 * Given an ASN1_ITEM CHOICE type return the selector value
 */

int ossl_asn1_get_choice_selector(ASN1_VALUE **pval, const ASN1_ITEM *it)
{
    int *sel = offset2ptr(*pval, it->utype);

    return *sel;
}

int ossl_asn1_get_choice_selector_const(const ASN1_VALUE **pval,
                                        const ASN1_ITEM *it)
{
    int *sel = offset2ptr(*pval, it->utype);

    return *sel;
}

/*
 * Given an ASN1_ITEM CHOICE type set the selector value, return old value.
 */

int ossl_asn1_set_choice_selector(ASN1_VALUE **pval, int value,
                                  const ASN1_ITEM *it)
{
    int *sel, ret;

    sel = offset2ptr(*pval, it->utype);
    ret = *sel;
    *sel = value;
    return ret;
}

/*
 * Do atomic reference counting. The value 'op' decides what to do.
 * If it is +1 then the count is incremented.
 * If |op| is 0, count is initialised and set to 1.
 * If |op| is -1, count is decremented and the return value is the current
 * reference count or 0 if no reference count is active.
 * It returns -1 on initialisation error.
 * Used by ASN1_SEQUENCE construct of X509, X509_REQ, X509_CRL objects
 */
int ossl_asn1_do_lock(ASN1_VALUE **pval, int op, const ASN1_ITEM *it)
{
    const ASN1_AUX *aux;
    CRYPTO_RWLOCK **lock;
    CRYPTO_REF_COUNT *refcnt;
    int ret = -1;

    if ((it->itype != ASN1_ITYPE_SEQUENCE)
        && (it->itype != ASN1_ITYPE_NDEF_SEQUENCE))
        return 0;
    aux = it->funcs;
    if (aux == NULL || (aux->flags & ASN1_AFLG_REFCOUNT) == 0)
        return 0;
    lock = offset2ptr(*pval, aux->ref_lock);
    refcnt = offset2ptr(*pval, aux->ref_offset);

    switch (op) {
    case 0:
        if (!CRYPTO_NEW_REF(refcnt, 1))
            return -1;
        *lock = CRYPTO_THREAD_lock_new();
        if (*lock == NULL) {
            CRYPTO_FREE_REF(refcnt);
            ERR_raise(ERR_LIB_ASN1, ERR_R_CRYPTO_LIB);
            return -1;
        }
        ret = 1;
        break;
    case 1:
        if (!CRYPTO_UP_REF(refcnt, &ret))
            return -1;
        break;
    case -1:
        if (!CRYPTO_DOWN_REF(refcnt, &ret))
            return -1;  /* failed */
        REF_PRINT_EX(it->sname, ret, (void *)it);
        REF_ASSERT_ISNT(ret < 0);
        if (ret == 0) {
            CRYPTO_THREAD_lock_free(*lock);
            *lock = NULL;
            CRYPTO_FREE_REF(refcnt);
        }
        break;
    }

    return ret;
}

static ASN1_ENCODING *asn1_get_enc_ptr(ASN1_VALUE **pval, const ASN1_ITEM *it)
{
    const ASN1_AUX *aux;

    if (pval == NULL || *pval == NULL)
        return NULL;
    aux = it->funcs;
    if (aux == NULL || (aux->flags & ASN1_AFLG_ENCODING) == 0)
        return NULL;
    return offset2ptr(*pval, aux->enc_offset);
}

static const ASN1_ENCODING *asn1_get_const_enc_ptr(const ASN1_VALUE **pval,
                                                   const ASN1_ITEM *it)
{
    const ASN1_AUX *aux;

    if (pval == NULL || *pval == NULL)
        return NULL;
    aux = it->funcs;
    if (aux == NULL || (aux->flags & ASN1_AFLG_ENCODING) == 0)
        return NULL;
    return offset2ptr(*pval, aux->enc_offset);
}

void ossl_asn1_enc_init(ASN1_VALUE **pval, const ASN1_ITEM *it)
{
    ASN1_ENCODING *enc = asn1_get_enc_ptr(pval, it);

    if (enc != NULL) {
        enc->enc = NULL;
        enc->len = 0;
        enc->modified = 1;
    }
}

void ossl_asn1_enc_free(ASN1_VALUE **pval, const ASN1_ITEM *it)
{
    ASN1_ENCODING *enc = asn1_get_enc_ptr(pval, it);

    if (enc != NULL) {
        OPENSSL_free(enc->enc);
        enc->enc = NULL;
        enc->len = 0;
        enc->modified = 1;
    }
}

int ossl_asn1_enc_save(ASN1_VALUE **pval, const unsigned char *in, int inlen,
                       const ASN1_ITEM *it)
{
    ASN1_ENCODING *enc = asn1_get_enc_ptr(pval, it);

    if (enc == NULL)
        return 1;

    OPENSSL_free(enc->enc);
    if (inlen <= 0)
        return 0;
    if ((enc->enc = OPENSSL_malloc(inlen)) == NULL)
        return 0;
    memcpy(enc->enc, in, inlen);
    enc->len = inlen;
    enc->modified = 0;

    return 1;
}

int ossl_asn1_enc_restore(int *len, unsigned char **out, const ASN1_VALUE **pval,
                          const ASN1_ITEM *it)
{
    const ASN1_ENCODING *enc = asn1_get_const_enc_ptr(pval, it);

    if (enc == NULL || enc->modified)
        return 0;
    if (out) {
        memcpy(*out, enc->enc, enc->len);
        *out += enc->len;
    }
    if (len != NULL)
        *len = enc->len;
    return 1;
}

/* Given an ASN1_TEMPLATE get a pointer to a field */
ASN1_VALUE **ossl_asn1_get_field_ptr(ASN1_VALUE **pval, const ASN1_TEMPLATE *tt)
{
    ASN1_VALUE **pvaltmp = offset2ptr(*pval, tt->offset);

    /*
     * NOTE for BOOLEAN types the field is just a plain int so we can't
     * return int **, so settle for (int *).
     */
    return pvaltmp;
}

/* Given an ASN1_TEMPLATE get a const pointer to a field */
const ASN1_VALUE **ossl_asn1_get_const_field_ptr(const ASN1_VALUE **pval,
                                                 const ASN1_TEMPLATE *tt)
{
    return offset2ptr(*pval, tt->offset);
}

/*
 * Handle ANY DEFINED BY template, find the selector, look up the relevant
 * ASN1_TEMPLATE in the table and return it.
 */

const ASN1_TEMPLATE *ossl_asn1_do_adb(const ASN1_VALUE *val,
                                      const ASN1_TEMPLATE *tt,
                                      int nullerr)
{
    const ASN1_ADB *adb;
    const ASN1_ADB_TABLE *atbl;
    long selector;
    const ASN1_VALUE **sfld;
    int i;

    if ((tt->flags & ASN1_TFLG_ADB_MASK) == 0)
        return tt;

    /* Else ANY DEFINED BY ... get the table */
    adb = ASN1_ADB_ptr(tt->item);

    /* Get the selector field */
    sfld = offset2ptr(val, adb->offset);

    /* Check if NULL */
    if (*sfld == NULL) {
        if (adb->null_tt == NULL)
            goto err;
        return adb->null_tt;
    }

    /*
     * Convert type to a long: NB: don't check for NID_undef here because it
     * might be a legitimate value in the table
     */
    if ((tt->flags & ASN1_TFLG_ADB_OID) != 0)
        selector = OBJ_obj2nid((ASN1_OBJECT *)*sfld);
    else
        selector = ASN1_INTEGER_get((ASN1_INTEGER *)*sfld);

    /* Let application callback translate value */
    if (adb->adb_cb != NULL && adb->adb_cb(&selector) == 0) {
        ERR_raise(ERR_LIB_ASN1, ASN1_R_UNSUPPORTED_ANY_DEFINED_BY_TYPE);
        return NULL;
    }

    /*
     * Try to find matching entry in table Maybe should check application
     * types first to allow application override? Might also be useful to
     * have a flag which indicates table is sorted and we can do a binary
     * search. For now stick to a linear search.
     */

    for (atbl = adb->tbl, i = 0; i < adb->tblcount; i++, atbl++)
        if (atbl->value == selector)
            return &atbl->tt;

    /* FIXME: need to search application table too */

    /* No match, return default type */
    if (!adb->default_tt)
        goto err;
    return adb->default_tt;

 err:
    /* FIXME: should log the value or OID of unsupported type */
    if (nullerr)
        ERR_raise(ERR_LIB_ASN1, ASN1_R_UNSUPPORTED_ANY_DEFINED_BY_TYPE);
    return NULL;
}
