/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <openssl/crypto.h>
#include <openssl/lhash.h>
#include <openssl/err.h>
#include "crypto/ctype.h"
#include "crypto/lhash.h"
#include "lhash_local.h"

/*
 * A hashing implementation that appears to be based on the linear hashing
 * algorithm:
 * https://en.wikipedia.org/wiki/Linear_hashing
 *
 * Litwin, Witold (1980), "Linear hashing: A new tool for file and table
 * addressing", Proc. 6th Conference on Very Large Databases: 212-223
 * https://hackthology.com/pdfs/Litwin-1980-Linear_Hashing.pdf
 *
 * From the Wikipedia article "Linear hashing is used in the BDB Berkeley
 * database system, which in turn is used by many software systems such as
 * OpenLDAP, using a C implementation derived from the CACM article and first
 * published on the Usenet in 1988 by Esmond Pitt."
 *
 * The CACM paper is available here:
 * https://pdfs.semanticscholar.org/ff4d/1c5deca6269cc316bfd952172284dbf610ee.pdf
 */

#undef MIN_NODES
#define MIN_NODES       16
#define UP_LOAD         (2*LH_LOAD_MULT) /* load times 256 (default 2) */
#define DOWN_LOAD       (LH_LOAD_MULT) /* load times 256 (default 1) */

static int expand(OPENSSL_LHASH *lh);
static void contract(OPENSSL_LHASH *lh);
static OPENSSL_LH_NODE **getrn(OPENSSL_LHASH *lh, const void *data, unsigned long *rhash);

OPENSSL_LHASH *OPENSSL_LH_set_thunks(OPENSSL_LHASH *lh,
                                     OPENSSL_LH_HASHFUNCTHUNK hw,
                                     OPENSSL_LH_COMPFUNCTHUNK cw,
                                     OPENSSL_LH_DOALL_FUNC_THUNK daw,
                                     OPENSSL_LH_DOALL_FUNCARG_THUNK daaw)
{

    if (lh == NULL)
        return NULL;
    lh->compw = cw;
    lh->hashw = hw;
    lh->daw = daw;
    lh->daaw = daaw;
    return lh;
}

OPENSSL_LHASH *OPENSSL_LH_new(OPENSSL_LH_HASHFUNC h, OPENSSL_LH_COMPFUNC c)
{
    OPENSSL_LHASH *ret;

    if ((ret = OPENSSL_zalloc(sizeof(*ret))) == NULL)
        return NULL;
    if ((ret->b = OPENSSL_zalloc(sizeof(*ret->b) * MIN_NODES)) == NULL)
        goto err;
    ret->comp = ((c == NULL) ? (OPENSSL_LH_COMPFUNC)strcmp : c);
    ret->hash = ((h == NULL) ? (OPENSSL_LH_HASHFUNC)OPENSSL_LH_strhash : h);
    ret->num_nodes = MIN_NODES / 2;
    ret->num_alloc_nodes = MIN_NODES;
    ret->pmax = MIN_NODES / 2;
    ret->up_load = UP_LOAD;
    ret->down_load = DOWN_LOAD;
    return ret;

err:
    OPENSSL_free(ret->b);
    OPENSSL_free(ret);
    return NULL;
}

void OPENSSL_LH_free(OPENSSL_LHASH *lh)
{
    if (lh == NULL)
        return;

    OPENSSL_LH_flush(lh);
    OPENSSL_free(lh->b);
    OPENSSL_free(lh);
}

void OPENSSL_LH_flush(OPENSSL_LHASH *lh)
{
    unsigned int i;
    OPENSSL_LH_NODE *n, *nn;

    if (lh == NULL)
        return;

    for (i = 0; i < lh->num_nodes; i++) {
        n = lh->b[i];
        while (n != NULL) {
            nn = n->next;
            OPENSSL_free(n);
            n = nn;
        }
        lh->b[i] = NULL;
    }

    lh->num_items = 0;
}

void *OPENSSL_LH_insert(OPENSSL_LHASH *lh, void *data)
{
    unsigned long hash;
    OPENSSL_LH_NODE *nn, **rn;
    void *ret;

    lh->error = 0;
    if ((lh->up_load <= (lh->num_items * LH_LOAD_MULT / lh->num_nodes)) && !expand(lh))
        return NULL;        /* 'lh->error++' already done in 'expand' */

    rn = getrn(lh, data, &hash);

    if (*rn == NULL) {
        if ((nn = OPENSSL_malloc(sizeof(*nn))) == NULL) {
            lh->error++;
            return NULL;
        }
        nn->data = data;
        nn->next = NULL;
        nn->hash = hash;
        *rn = nn;
        ret = NULL;
        lh->num_items++;
    } else {                    /* replace same key */
        ret = (*rn)->data;
        (*rn)->data = data;
    }
    return ret;
}

void *OPENSSL_LH_delete(OPENSSL_LHASH *lh, const void *data)
{
    unsigned long hash;
    OPENSSL_LH_NODE *nn, **rn;
    void *ret;

    lh->error = 0;
    rn = getrn(lh, data, &hash);

    if (*rn == NULL) {
        return NULL;
    } else {
        nn = *rn;
        *rn = nn->next;
        ret = nn->data;
        OPENSSL_free(nn);
    }

    lh->num_items--;
    if ((lh->num_nodes > MIN_NODES) &&
        (lh->down_load >= (lh->num_items * LH_LOAD_MULT / lh->num_nodes)))
        contract(lh);

    return ret;
}

void *OPENSSL_LH_retrieve(OPENSSL_LHASH *lh, const void *data)
{
    unsigned long hash;
    OPENSSL_LH_NODE **rn;

    if (lh->error != 0)
        lh->error = 0;

    rn = getrn(lh, data, &hash);

    return *rn == NULL ? NULL : (*rn)->data;
}

static void doall_util_fn(OPENSSL_LHASH *lh, int use_arg,
                          OPENSSL_LH_DOALL_FUNC_THUNK wfunc,
                          OPENSSL_LH_DOALL_FUNC func,
                          OPENSSL_LH_DOALL_FUNCARG func_arg,
                          OPENSSL_LH_DOALL_FUNCARG_THUNK wfunc_arg,
                          void *arg)
{
    int i;
    OPENSSL_LH_NODE *a, *n;

    if (lh == NULL)
        return;

    /*
     * reverse the order so we search from 'top to bottom' We were having
     * memory leaks otherwise
     */
    for (i = lh->num_nodes - 1; i >= 0; i--) {
        a = lh->b[i];
        while (a != NULL) {
            n = a->next;
            if (use_arg)
                wfunc_arg(a->data, arg, func_arg);
            else
                wfunc(a->data, func);
            a = n;
        }
    }
}

void OPENSSL_LH_doall(OPENSSL_LHASH *lh, OPENSSL_LH_DOALL_FUNC func)
{
    if (lh == NULL)
        return;

    doall_util_fn(lh, 0, lh->daw, func, (OPENSSL_LH_DOALL_FUNCARG)NULL,
                  (OPENSSL_LH_DOALL_FUNCARG_THUNK)NULL, NULL);
}

void OPENSSL_LH_doall_arg(OPENSSL_LHASH *lh,
                          OPENSSL_LH_DOALL_FUNCARG func, void *arg)
{
    if (lh == NULL)
        return;

    doall_util_fn(lh, 1, (OPENSSL_LH_DOALL_FUNC_THUNK)NULL,
                  (OPENSSL_LH_DOALL_FUNC)NULL, func, lh->daaw, arg);
}

void OPENSSL_LH_doall_arg_thunk(OPENSSL_LHASH *lh,
                                OPENSSL_LH_DOALL_FUNCARG_THUNK daaw,
                                OPENSSL_LH_DOALL_FUNCARG fn, void *arg)
{
    doall_util_fn(lh, 1, (OPENSSL_LH_DOALL_FUNC_THUNK)NULL,
                  (OPENSSL_LH_DOALL_FUNC)NULL, fn, daaw, arg);
}

static int expand(OPENSSL_LHASH *lh)
{
    OPENSSL_LH_NODE **n, **n1, **n2, *np;
    unsigned int p, pmax, nni, j;
    unsigned long hash;

    nni = lh->num_alloc_nodes;
    p = lh->p;
    pmax = lh->pmax;
    if (p + 1 >= pmax) {
        j = nni * 2;
        n = OPENSSL_realloc(lh->b, sizeof(OPENSSL_LH_NODE *) * j);
        if (n == NULL) {
            lh->error++;
            return 0;
        }
        lh->b = n;
        memset(n + nni, 0, sizeof(*n) * (j - nni));
        lh->pmax = nni;
        lh->num_alloc_nodes = j;
        lh->p = 0;
    } else {
        lh->p++;
    }

    lh->num_nodes++;
    n1 = &(lh->b[p]);
    n2 = &(lh->b[p + pmax]);
    *n2 = NULL;

    for (np = *n1; np != NULL;) {
        hash = np->hash;
        if ((hash % nni) != p) { /* move it */
            *n1 = (*n1)->next;
            np->next = *n2;
            *n2 = np;
        } else
            n1 = &((*n1)->next);
        np = *n1;
    }

    return 1;
}

static void contract(OPENSSL_LHASH *lh)
{
    OPENSSL_LH_NODE **n, *n1, *np;

    np = lh->b[lh->p + lh->pmax - 1];
    lh->b[lh->p + lh->pmax - 1] = NULL; /* 24/07-92 - eay - weird but :-( */
    if (lh->p == 0) {
        n = OPENSSL_realloc(lh->b,
                            (unsigned int)(sizeof(OPENSSL_LH_NODE *) * lh->pmax));
        if (n == NULL) {
            /* fputs("realloc error in lhash", stderr); */
            lh->error++;
        } else {
            lh->b = n;
        }
        lh->num_alloc_nodes /= 2;
        lh->pmax /= 2;
        lh->p = lh->pmax - 1;
    } else
        lh->p--;

    lh->num_nodes--;

    n1 = lh->b[(int)lh->p];
    if (n1 == NULL)
        lh->b[(int)lh->p] = np;
    else {
        while (n1->next != NULL)
            n1 = n1->next;
        n1->next = np;
    }
}

static OPENSSL_LH_NODE **getrn(OPENSSL_LHASH *lh,
                               const void *data, unsigned long *rhash)
{
    OPENSSL_LH_NODE **ret, *n1;
    unsigned long hash, nn;

    if (lh->hashw != NULL)
        hash = lh->hashw(data, lh->hash);
    else
        hash = lh->hash(data);

    *rhash = hash;

    nn = hash % lh->pmax;
    if (nn < lh->p)
        nn = hash % lh->num_alloc_nodes;

    ret = &(lh->b[(int)nn]);
    for (n1 = *ret; n1 != NULL; n1 = n1->next) {
        if (n1->hash != hash) {
            ret = &(n1->next);
            continue;
        }

        if (lh->compw != NULL) {
            if (lh->compw(n1->data, data, lh->comp) == 0)
                break;
        } else {
            if (lh->comp(n1->data, data) == 0)
                break;
        }
        ret = &(n1->next);
    }
    return ret;
}

/*
 * The following hash seems to work very well on normal text strings no
 * collisions on /usr/dict/words and it distributes on %2^n quite well, not
 * as good as MD5, but still good.
 */
unsigned long OPENSSL_LH_strhash(const char *c)
{
    unsigned long ret = 0;
    long n;
    unsigned long v;
    int r;

    if ((c == NULL) || (*c == '\0'))
        return ret;

    n = 0x100;
    while (*c) {
        v = n | (*c);
        n += 0x100;
        r = (int)((v >> 2) ^ v) & 0x0f;
        /* cast to uint64_t to avoid 32 bit shift of 32 bit value */
        ret = (ret << r) | (unsigned long)((uint64_t)ret >> (32 - r));
        ret &= 0xFFFFFFFFL;
        ret ^= v * v;
        c++;
    }
    return (ret >> 16) ^ ret;
}

/*
 * Case insensitive string hashing.
 *
 * The lower/upper case bit is masked out (forcing all letters to be capitals).
 * The major side effect on non-alpha characters is mapping the symbols and
 * digits into the control character range (which should be harmless).
 * The duplication (with respect to the hash value) of printable characters
 * are that '`', '{', '|', '}' and '~' map to '@', '[', '\', ']' and '^'
 * respectively (which seems tolerable).
 *
 * For EBCDIC, the alpha mapping is to lower case, most symbols go to control
 * characters.  The only duplication is '0' mapping to '^', which is better
 * than for ASCII.
 */
unsigned long ossl_lh_strcasehash(const char *c)
{
    unsigned long ret = 0;
    long n;
    unsigned long v;
    int r;
#if defined(CHARSET_EBCDIC) && !defined(CHARSET_EBCDIC_TEST)
    const long int case_adjust = ~0x40;
#else
    const long int case_adjust = ~0x20;
#endif

    if (c == NULL || *c == '\0')
        return ret;

    for (n = 0x100; *c != '\0'; n += 0x100) {
        v = n | (case_adjust & *c);
        r = (int)((v >> 2) ^ v) & 0x0f;
        /* cast to uint64_t to avoid 32 bit shift of 32 bit value */
        ret = (ret << r) | (unsigned long)((uint64_t)ret >> (32 - r));
        ret &= 0xFFFFFFFFL;
        ret ^= v * v;
        c++;
    }
    return (ret >> 16) ^ ret;
}

unsigned long OPENSSL_LH_num_items(const OPENSSL_LHASH *lh)
{
    return lh ? lh->num_items : 0;
}

unsigned long OPENSSL_LH_get_down_load(const OPENSSL_LHASH *lh)
{
    return lh->down_load;
}

void OPENSSL_LH_set_down_load(OPENSSL_LHASH *lh, unsigned long down_load)
{
    lh->down_load = down_load;
}

int OPENSSL_LH_error(OPENSSL_LHASH *lh)
{
    return lh->error;
}
