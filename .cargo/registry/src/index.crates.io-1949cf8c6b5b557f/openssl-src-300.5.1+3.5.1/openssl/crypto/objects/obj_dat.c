/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "crypto/ctype.h"
#include <limits.h>
#include "internal/cryptlib.h"
#include "internal/thread_once.h"
#include "internal/tsan_assist.h"
#include <openssl/lhash.h>
#include <openssl/asn1.h>
#include "crypto/objects.h"
#include <openssl/bn.h>
#include "crypto/asn1.h"
#include "obj_local.h"

/* obj_dat.h is generated from objects.txt and obj_mac.{num,h} by obj_dat.pl */
#include "obj_dat.h"

DECLARE_OBJ_BSEARCH_CMP_FN(const ASN1_OBJECT *, unsigned int, sn);
DECLARE_OBJ_BSEARCH_CMP_FN(const ASN1_OBJECT *, unsigned int, ln);
DECLARE_OBJ_BSEARCH_CMP_FN(const ASN1_OBJECT *, unsigned int, obj);

#define ADDED_DATA      0
#define ADDED_SNAME     1
#define ADDED_LNAME     2
#define ADDED_NID       3

struct added_obj_st {
    int type;
    ASN1_OBJECT *obj;
};

static LHASH_OF(ADDED_OBJ) *added = NULL;
static CRYPTO_RWLOCK *ossl_obj_lock = NULL;
#ifdef TSAN_REQUIRES_LOCKING
static CRYPTO_RWLOCK *ossl_obj_nid_lock = NULL;
#endif

static CRYPTO_ONCE ossl_obj_lock_init = CRYPTO_ONCE_STATIC_INIT;

static ossl_inline void objs_free_locks(void)
{
    CRYPTO_THREAD_lock_free(ossl_obj_lock);
    ossl_obj_lock = NULL;
#ifdef TSAN_REQUIRES_LOCKING
    CRYPTO_THREAD_lock_free(ossl_obj_nid_lock);
    ossl_obj_nid_lock = NULL;
#endif
}

DEFINE_RUN_ONCE_STATIC(obj_lock_initialise)
{
    ossl_obj_lock = CRYPTO_THREAD_lock_new();
    if (ossl_obj_lock == NULL)
        return 0;

#ifdef TSAN_REQUIRES_LOCKING
    ossl_obj_nid_lock = CRYPTO_THREAD_lock_new();
    if (ossl_obj_nid_lock == NULL) {
        objs_free_locks();
        return 0;
    }
#endif
    return 1;
}

static ossl_inline int ossl_init_added_lock(void)
{
#ifndef OPENSSL_NO_AUTOLOAD_CONFIG
    /* Make sure we've loaded config before checking for any "added" objects */
    OPENSSL_init_crypto(OPENSSL_INIT_LOAD_CONFIG, NULL);
#endif
    return RUN_ONCE(&ossl_obj_lock_init, obj_lock_initialise);
}

static ossl_inline int ossl_obj_write_lock(int lock)
{
    if (!lock)
        return 1;
    if (!ossl_init_added_lock())
        return 0;
    return CRYPTO_THREAD_write_lock(ossl_obj_lock);
}

static ossl_inline int ossl_obj_read_lock(int lock)
{
    if (!lock)
        return 1;
    if (!ossl_init_added_lock())
        return 0;
    return CRYPTO_THREAD_read_lock(ossl_obj_lock);
}

static ossl_inline void ossl_obj_unlock(int lock)
{
    if (lock)
        CRYPTO_THREAD_unlock(ossl_obj_lock);
}

static int sn_cmp(const ASN1_OBJECT *const *a, const unsigned int *b)
{
    return strcmp((*a)->sn, nid_objs[*b].sn);
}

IMPLEMENT_OBJ_BSEARCH_CMP_FN(const ASN1_OBJECT *, unsigned int, sn);

static int ln_cmp(const ASN1_OBJECT *const *a, const unsigned int *b)
{
    return strcmp((*a)->ln, nid_objs[*b].ln);
}

IMPLEMENT_OBJ_BSEARCH_CMP_FN(const ASN1_OBJECT *, unsigned int, ln);

static unsigned long added_obj_hash(const ADDED_OBJ *ca)
{
    const ASN1_OBJECT *a;
    int i;
    unsigned long ret = 0;
    unsigned char *p;

    a = ca->obj;
    switch (ca->type) {
    case ADDED_DATA:
        ret = (unsigned long)a->length << 20UL;
        p = (unsigned char *)a->data;
        for (i = 0; i < a->length; i++)
            ret ^= p[i] << ((i * 3) % 24);
        break;
    case ADDED_SNAME:
        ret = OPENSSL_LH_strhash(a->sn);
        break;
    case ADDED_LNAME:
        ret = OPENSSL_LH_strhash(a->ln);
        break;
    case ADDED_NID:
        ret = a->nid;
        break;
    default:
        /* abort(); */
        return 0;
    }
    ret &= 0x3fffffffL;
    ret |= ((unsigned long)ca->type) << 30L;
    return ret;
}

static int added_obj_cmp(const ADDED_OBJ *ca, const ADDED_OBJ *cb)
{
    ASN1_OBJECT *a, *b;
    int i;

    i = ca->type - cb->type;
    if (i)
        return i;
    a = ca->obj;
    b = cb->obj;
    switch (ca->type) {
    case ADDED_DATA:
        i = (a->length - b->length);
        if (i)
            return i;
        return memcmp(a->data, b->data, (size_t)a->length);
    case ADDED_SNAME:
        if (a->sn == NULL)
            return -1;
        else if (b->sn == NULL)
            return 1;
        else
            return strcmp(a->sn, b->sn);
    case ADDED_LNAME:
        if (a->ln == NULL)
            return -1;
        else if (b->ln == NULL)
            return 1;
        else
            return strcmp(a->ln, b->ln);
    case ADDED_NID:
        return a->nid - b->nid;
    default:
        /* abort(); */
        return 0;
    }
}

static void cleanup1_doall(ADDED_OBJ *a)
{
    a->obj->nid = 0;
    a->obj->flags |= ASN1_OBJECT_FLAG_DYNAMIC |
        ASN1_OBJECT_FLAG_DYNAMIC_STRINGS | ASN1_OBJECT_FLAG_DYNAMIC_DATA;
}

static void cleanup2_doall(ADDED_OBJ *a)
{
    a->obj->nid++;
}

static void cleanup3_doall(ADDED_OBJ *a)
{
    if (--a->obj->nid == 0)
        ASN1_OBJECT_free(a->obj);
    OPENSSL_free(a);
}

void ossl_obj_cleanup_int(void)
{
    if (added != NULL) {
        lh_ADDED_OBJ_set_down_load(added, 0);
        lh_ADDED_OBJ_doall(added, cleanup1_doall); /* zero counters */
        lh_ADDED_OBJ_doall(added, cleanup2_doall); /* set counters */
        lh_ADDED_OBJ_doall(added, cleanup3_doall); /* free objects */
        lh_ADDED_OBJ_free(added);
        added = NULL;
    }
    objs_free_locks();
}

/*
 * Requires that the ossl_obj_lock be held
 * if TSAN_REQUIRES_LOCKING defined
 */
static int obj_new_nid_unlocked(int num)
{
    static TSAN_QUALIFIER int new_nid = NUM_NID;
#ifdef TSAN_REQUIRES_LOCKING
    int i;

    i = new_nid;
    new_nid += num;

    return i;
#else
    return tsan_add(&new_nid, num);
#endif
}

int OBJ_new_nid(int num)
{
#ifdef TSAN_REQUIRES_LOCKING
    int i;

    if (!ossl_obj_write_lock(1)) {
        ERR_raise(ERR_LIB_OBJ, ERR_R_UNABLE_TO_GET_WRITE_LOCK);
        return NID_undef;
    }

    i = obj_new_nid_unlocked(num);

    ossl_obj_unlock(1);

    return i;
#else
    return obj_new_nid_unlocked(num);
#endif
}

static int ossl_obj_add_object(const ASN1_OBJECT *obj, int lock)
{
    ASN1_OBJECT *o = NULL;
    ADDED_OBJ *ao[4] = { NULL, NULL, NULL, NULL }, *aop[4];
    int i;

    if ((o = OBJ_dup(obj)) == NULL)
        return NID_undef;
    if ((ao[ADDED_NID] = OPENSSL_malloc(sizeof(*ao[0]))) == NULL
            || (o->length != 0
                && obj->data != NULL
                && (ao[ADDED_DATA] = OPENSSL_malloc(sizeof(*ao[0]))) == NULL)
            || (o->sn != NULL
                && (ao[ADDED_SNAME] = OPENSSL_malloc(sizeof(*ao[0]))) == NULL)
            || (o->ln != NULL
                && (ao[ADDED_LNAME] = OPENSSL_malloc(sizeof(*ao[0]))) == NULL))
        goto err2;

    if (!ossl_obj_write_lock(lock)) {
        ERR_raise(ERR_LIB_OBJ, ERR_R_UNABLE_TO_GET_WRITE_LOCK);
        goto err2;
    }
    if (added == NULL) {
        added = lh_ADDED_OBJ_new(added_obj_hash, added_obj_cmp);
        if (added == NULL) {
            ERR_raise(ERR_LIB_OBJ, ERR_R_CRYPTO_LIB);
            goto err;
        }
    }

    for (i = ADDED_DATA; i <= ADDED_NID; i++) {
        if (ao[i] != NULL) {
            ao[i]->type = i;
            ao[i]->obj = o;
            aop[i] = lh_ADDED_OBJ_retrieve(added, ao[i]);
            if (aop[i] != NULL)
                aop[i]->type = -1;
            (void)lh_ADDED_OBJ_insert(added, ao[i]);
            if (lh_ADDED_OBJ_error(added)) {
                if (aop[i] != NULL)
                    aop[i]->type = i;
                while (i-- > ADDED_DATA) {
                    lh_ADDED_OBJ_delete(added, ao[i]);
                    if (aop[i] != NULL)
                        aop[i]->type = i;
                }
                ERR_raise(ERR_LIB_OBJ, ERR_R_CRYPTO_LIB);
                goto err;
            }
        }
    }
    o->flags &=
        ~(ASN1_OBJECT_FLAG_DYNAMIC | ASN1_OBJECT_FLAG_DYNAMIC_STRINGS |
          ASN1_OBJECT_FLAG_DYNAMIC_DATA);

    ossl_obj_unlock(lock);
    return o->nid;

 err:
    ossl_obj_unlock(lock);
 err2:
    for (i = ADDED_DATA; i <= ADDED_NID; i++)
        OPENSSL_free(ao[i]);
    ASN1_OBJECT_free(o);
    return NID_undef;
}

ASN1_OBJECT *OBJ_nid2obj(int n)
{
    ADDED_OBJ ad, *adp = NULL;
    ASN1_OBJECT ob;

    if (n == NID_undef
        || (n > 0 && n < NUM_NID && nid_objs[n].nid != NID_undef))
        return (ASN1_OBJECT *)&(nid_objs[n]);

    ad.type = ADDED_NID;
    ad.obj = &ob;
    ob.nid = n;
    if (!ossl_obj_read_lock(1)) {
        ERR_raise(ERR_LIB_OBJ, ERR_R_UNABLE_TO_GET_READ_LOCK);
        return NULL;
    }
    if (added != NULL)
        adp = lh_ADDED_OBJ_retrieve(added, &ad);
    ossl_obj_unlock(1);
    if (adp != NULL)
        return adp->obj;

    ERR_raise(ERR_LIB_OBJ, OBJ_R_UNKNOWN_NID);
    return NULL;
}

const char *OBJ_nid2sn(int n)
{
    ASN1_OBJECT *ob = OBJ_nid2obj(n);

    return ob == NULL ? NULL : ob->sn;
}

const char *OBJ_nid2ln(int n)
{
    ASN1_OBJECT *ob = OBJ_nid2obj(n);

    return ob == NULL ? NULL : ob->ln;
}

static int obj_cmp(const ASN1_OBJECT *const *ap, const unsigned int *bp)
{
    int j;
    const ASN1_OBJECT *a = *ap;
    const ASN1_OBJECT *b = &nid_objs[*bp];

    j = (a->length - b->length);
    if (j)
        return j;
    if (a->length == 0)
        return 0;
    return memcmp(a->data, b->data, a->length);
}

IMPLEMENT_OBJ_BSEARCH_CMP_FN(const ASN1_OBJECT *, unsigned int, obj);

static int ossl_obj_obj2nid(const ASN1_OBJECT *a, const int lock)
{
    int nid = NID_undef;
    const unsigned int *op;
    ADDED_OBJ ad, *adp;

    if (a == NULL)
        return NID_undef;
    if (a->nid != NID_undef)
        return a->nid;
    if (a->length == 0)
        return NID_undef;

    op = OBJ_bsearch_obj(&a, obj_objs, NUM_OBJ);
    if (op != NULL)
        return nid_objs[*op].nid;
    if (!ossl_obj_read_lock(lock)) {
        ERR_raise(ERR_LIB_OBJ, ERR_R_UNABLE_TO_GET_READ_LOCK);
        return NID_undef;
    }
    if (added != NULL) {
        ad.type = ADDED_DATA;
        ad.obj = (ASN1_OBJECT *)a; /* casting away const is harmless here */
        adp = lh_ADDED_OBJ_retrieve(added, &ad);
        if (adp != NULL)
            nid = adp->obj->nid;
    }
    ossl_obj_unlock(lock);
    return nid;
}

/*
 * Convert an object name into an ASN1_OBJECT if "noname" is not set then
 * search for short and long names first. This will convert the "dotted" form
 * into an object: unlike OBJ_txt2nid it can be used with any objects, not
 * just registered ones.
 */
ASN1_OBJECT *OBJ_txt2obj(const char *s, int no_name)
{
    int nid = NID_undef;
    ASN1_OBJECT *op = NULL;
    unsigned char *buf;
    unsigned char *p;
    const unsigned char *cp;
    int i, j;

    if (!no_name) {
        if ((nid = OBJ_sn2nid(s)) != NID_undef
            || (nid = OBJ_ln2nid(s)) != NID_undef) {
            return OBJ_nid2obj(nid);
        }
        if (!ossl_isdigit(*s)) {
            ERR_raise(ERR_LIB_OBJ, OBJ_R_UNKNOWN_OBJECT_NAME);
            return NULL;
        }
    }

    /* Work out size of content octets */
    i = a2d_ASN1_OBJECT(NULL, 0, s, -1);
    if (i <= 0)
        return NULL;

    /* Work out total size */
    j = ASN1_object_size(0, i, V_ASN1_OBJECT);
    if (j < 0)
        return NULL;

    if ((buf = OPENSSL_malloc(j)) == NULL)
        return NULL;

    p = buf;
    /* Write out tag+length */
    ASN1_put_object(&p, 0, i, V_ASN1_OBJECT, V_ASN1_UNIVERSAL);
    /* Write out contents */
    a2d_ASN1_OBJECT(p, i, s, -1);

    cp = buf;
    op = d2i_ASN1_OBJECT(NULL, &cp, j);
    OPENSSL_free(buf);
    return op;
}

int OBJ_obj2txt(char *buf, int buf_len, const ASN1_OBJECT *a, int no_name)
{
    int i, n = 0, len, nid, first, use_bn;
    BIGNUM *bl;
    unsigned long l;
    const unsigned char *p;
    char tbuf[DECIMAL_SIZE(i) + DECIMAL_SIZE(l) + 2];
    const char *s;

    /* Ensure that, at every state, |buf| is NUL-terminated. */
    if (buf != NULL && buf_len > 0)
        buf[0] = '\0';

    if (a == NULL || a->data == NULL)
        return 0;

    if (!no_name && (nid = OBJ_obj2nid(a)) != NID_undef) {
        s = OBJ_nid2ln(nid);
        if (s == NULL)
            s = OBJ_nid2sn(nid);
        if (s != NULL) {
            if (buf != NULL)
                OPENSSL_strlcpy(buf, s, buf_len);
            return (int)strlen(s);
        }
    }

    len = a->length;
    p = a->data;

    first = 1;
    bl = NULL;

    /*
     * RFC 2578 (STD 58) says this about OBJECT IDENTIFIERs:
     *
     * > 3.5. OBJECT IDENTIFIER values
     * >
     * > An OBJECT IDENTIFIER value is an ordered list of non-negative
     * > numbers. For the SMIv2, each number in the list is referred to as a
     * > sub-identifier, there are at most 128 sub-identifiers in a value,
     * > and each sub-identifier has a maximum value of 2^32-1 (4294967295
     * > decimal).
     *
     * So a legitimate OID according to this RFC is at most (32 * 128 / 7),
     * i.e. 586 bytes long.
     *
     * Ref: https://datatracker.ietf.org/doc/html/rfc2578#section-3.5
     */
    if (len > 586)
        goto err;

    while (len > 0) {
        l = 0;
        use_bn = 0;
        for (;;) {
            unsigned char c = *p++;

            len--;
            if (len == 0 && (c & 0x80) != 0)
                goto err;
            if (use_bn) {
                if (!BN_add_word(bl, c & 0x7f))
                    goto err;
            } else {
                l |= c & 0x7f;
            }
            if ((c & 0x80) == 0)
                break;
            if (!use_bn && l > (ULONG_MAX >> 7L)) {
                if (bl == NULL && (bl = BN_new()) == NULL)
                    goto err;
                if (!BN_set_word(bl, l))
                    goto err;
                use_bn = 1;
            }
            if (use_bn) {
                if (!BN_lshift(bl, bl, 7))
                    goto err;
            } else {
                l <<= 7L;
            }
        }

        if (first) {
            first = 0;
            if (l >= 80) {
                i = 2;
                if (use_bn) {
                    if (!BN_sub_word(bl, 80))
                        goto err;
                } else {
                    l -= 80;
                }
            } else {
                i = (int)(l / 40);
                l -= (long)(i * 40);
            }
            if (buf != NULL && buf_len > 1) {
                *buf++ = i + '0';
                *buf = '\0';
                buf_len--;
            }
            n++;
        }

        if (use_bn) {
            char *bndec;
            bndec = BN_bn2dec(bl);
            if (!bndec)
                goto err;
            i = strlen(bndec);
            if (buf != NULL) {
                if (buf_len > 1) {
                    *buf++ = '.';
                    *buf = '\0';
                    buf_len--;
                }
                OPENSSL_strlcpy(buf, bndec, buf_len);
                if (i > buf_len) {
                    buf += buf_len;
                    buf_len = 0;
                } else {
                    buf += i;
                    buf_len -= i;
                }
            }
            n++;
            n += i;
            OPENSSL_free(bndec);
        } else {
            BIO_snprintf(tbuf, sizeof(tbuf), ".%lu", l);
            i = strlen(tbuf);
            if (buf && buf_len > 0) {
                OPENSSL_strlcpy(buf, tbuf, buf_len);
                if (i > buf_len) {
                    buf += buf_len;
                    buf_len = 0;
                } else {
                    buf += i;
                    buf_len -= i;
                }
            }
            n += i;
            l = 0;
        }
    }

    BN_free(bl);
    return n;

 err:
    BN_free(bl);
    return -1;
}

int OBJ_txt2nid(const char *s)
{
    ASN1_OBJECT *obj = OBJ_txt2obj(s, 0);
    int nid = NID_undef;

    if (obj != NULL) {
        nid = OBJ_obj2nid(obj);
        ASN1_OBJECT_free(obj);
    }
    return nid;
}

int OBJ_ln2nid(const char *s)
{
    ASN1_OBJECT o;
    const ASN1_OBJECT *oo = &o;
    ADDED_OBJ ad, *adp;
    const unsigned int *op;
    int nid = NID_undef;

    o.ln = s;
    op = OBJ_bsearch_ln(&oo, ln_objs, NUM_LN);
    if (op != NULL)
        return nid_objs[*op].nid;
    if (!ossl_obj_read_lock(1)) {
        ERR_raise(ERR_LIB_OBJ, ERR_R_UNABLE_TO_GET_READ_LOCK);
        return NID_undef;
    }
    if (added != NULL) {
        ad.type = ADDED_LNAME;
        ad.obj = &o;
        adp = lh_ADDED_OBJ_retrieve(added, &ad);
        if (adp != NULL)
            nid = adp->obj->nid;
    }
    ossl_obj_unlock(1);
    return nid;
}

int OBJ_sn2nid(const char *s)
{
    ASN1_OBJECT o;
    const ASN1_OBJECT *oo = &o;
    ADDED_OBJ ad, *adp;
    const unsigned int *op;
    int nid = NID_undef;

    o.sn = s;
    op = OBJ_bsearch_sn(&oo, sn_objs, NUM_SN);
    if (op != NULL)
        return nid_objs[*op].nid;
    if (!ossl_obj_read_lock(1)) {
        ERR_raise(ERR_LIB_OBJ, ERR_R_UNABLE_TO_GET_READ_LOCK);
        return NID_undef;
    }
    if (added != NULL) {
        ad.type = ADDED_SNAME;
        ad.obj = &o;
        adp = lh_ADDED_OBJ_retrieve(added, &ad);
        if (adp != NULL)
            nid = adp->obj->nid;
    }
    ossl_obj_unlock(1);
    return nid;
}

const void *OBJ_bsearch_(const void *key, const void *base, int num, int size,
                         int (*cmp) (const void *, const void *))
{
    return OBJ_bsearch_ex_(key, base, num, size, cmp, 0);
}

const void *OBJ_bsearch_ex_(const void *key, const void *base, int num,
                            int size,
                            int (*cmp) (const void *, const void *),
                            int flags)
{
    const char *p = ossl_bsearch(key, base, num, size, cmp, flags);

#ifdef CHARSET_EBCDIC
    /*
     * THIS IS A KLUDGE - Because the *_obj is sorted in ASCII order, and I
     * don't have perl (yet), we revert to a *LINEAR* search when the object
     * wasn't found in the binary search.
     */
    if (p == NULL) {
        const char *base_ = base;
        int l, h, i = 0, c = 0;
        char *p1;

        for (i = 0; i < num; ++i) {
            p1 = &(base_[i * size]);
            c = (*cmp) (key, p1);
            if (c == 0
                || (c < 0 && (flags & OBJ_BSEARCH_VALUE_ON_NOMATCH)))
                return p1;
        }
    }
#endif
    return p;
}

/*
 * Parse a BIO sink to create some extra oid's objects.
 * Line format:<OID:isdigit or '.']><isspace><SN><isspace><LN>
 */
int OBJ_create_objects(BIO *in)
{
    char buf[512];
    int i, num = 0;
    char *o, *s, *l = NULL;

    for (;;) {
        s = o = NULL;
        i = BIO_gets(in, buf, 512);
        if (i <= 0)
            return num;
        buf[i - 1] = '\0';
        if (!ossl_isalnum(buf[0]))
            return num;
        o = s = buf;
        while (ossl_isdigit(*s) || *s == '.')
            s++;
        if (*s != '\0') {
            *(s++) = '\0';
            while (ossl_isspace(*s))
                s++;
            if (*s == '\0') {
                s = NULL;
            } else {
                l = s;
                while (*l != '\0' && !ossl_isspace(*l))
                    l++;
                if (*l != '\0') {
                    *(l++) = '\0';
                    while (ossl_isspace(*l))
                        l++;
                    if (*l == '\0') {
                        l = NULL;
                    }
                } else {
                    l = NULL;
                }
            }
        } else {
            s = NULL;
        }
        if (*o == '\0')
            return num;
        if (!OBJ_create(o, s, l))
            return num;
        num++;
    }
}

int OBJ_create(const char *oid, const char *sn, const char *ln)
{
    ASN1_OBJECT *tmpoid = NULL;
    int ok = 0;

    /* With no arguments at all, nothing can be done */
    if (oid == NULL && sn == NULL && ln == NULL) {
        ERR_raise(ERR_LIB_OBJ, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }

    /* Check to see if short or long name already present */
    if ((sn != NULL && OBJ_sn2nid(sn) != NID_undef)
            || (ln != NULL && OBJ_ln2nid(ln) != NID_undef)) {
        ERR_raise(ERR_LIB_OBJ, OBJ_R_OID_EXISTS);
        return 0;
    }

    if (oid != NULL) {
        /* Convert numerical OID string to an ASN1_OBJECT structure */
        tmpoid = OBJ_txt2obj(oid, 1);
        if (tmpoid == NULL)
            return 0;
    } else {
        /* Create a no-OID ASN1_OBJECT */
        tmpoid = ASN1_OBJECT_new();
        if (tmpoid == NULL) {
            ERR_raise(ERR_LIB_OBJ, ERR_R_ASN1_LIB);
            return 0;
        }
    }

    if (!ossl_obj_write_lock(1)) {
        ERR_raise(ERR_LIB_OBJ, ERR_R_UNABLE_TO_GET_WRITE_LOCK);
        ASN1_OBJECT_free(tmpoid);
        return 0;
    }

    /* If NID is not NID_undef then object already exists */
    if (oid != NULL
        && ossl_obj_obj2nid(tmpoid, 0) != NID_undef) {
        ERR_raise(ERR_LIB_OBJ, OBJ_R_OID_EXISTS);
        goto err;
    }

    tmpoid->nid = obj_new_nid_unlocked(1);

    if (tmpoid->nid == NID_undef)
        goto err;

    tmpoid->sn = (char *)sn;
    tmpoid->ln = (char *)ln;

    ok = ossl_obj_add_object(tmpoid, 0);

    tmpoid->sn = NULL;
    tmpoid->ln = NULL;

 err:
    ossl_obj_unlock(1);
    ASN1_OBJECT_free(tmpoid);
    return ok;
}

size_t OBJ_length(const ASN1_OBJECT *obj)
{
    if (obj == NULL)
        return 0;
    return obj->length;
}

const unsigned char *OBJ_get0_data(const ASN1_OBJECT *obj)
{
    if (obj == NULL)
        return NULL;
    return obj->data;
}

int OBJ_add_object(const ASN1_OBJECT *obj)
{
    return ossl_obj_add_object(obj, 1);
}

int OBJ_obj2nid(const ASN1_OBJECT *a)
{
    return ossl_obj_obj2nid(a, 1);
}
