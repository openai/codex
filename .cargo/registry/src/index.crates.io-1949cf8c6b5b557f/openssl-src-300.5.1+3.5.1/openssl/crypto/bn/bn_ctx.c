/*
 * Copyright 2000-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/trace.h>
#include "internal/cryptlib.h"
#include "bn_local.h"

/* How many bignums are in each "pool item"; */
#define BN_CTX_POOL_SIZE        16
/* The stack frame info is resizing, set a first-time expansion size; */
#define BN_CTX_START_FRAMES     32

/***********/
/* BN_POOL */
/***********/

/* A bundle of bignums that can be linked with other bundles */
typedef struct bignum_pool_item {
    /* The bignum values */
    BIGNUM vals[BN_CTX_POOL_SIZE];
    /* Linked-list admin */
    struct bignum_pool_item *prev, *next;
} BN_POOL_ITEM;
/* A linked-list of bignums grouped in bundles */
typedef struct bignum_pool {
    /* Linked-list admin */
    BN_POOL_ITEM *head, *current, *tail;
    /* Stack depth and allocation size */
    unsigned used, size;
} BN_POOL;
static void BN_POOL_init(BN_POOL *);
static void BN_POOL_finish(BN_POOL *);
static BIGNUM *BN_POOL_get(BN_POOL *, int);
static void BN_POOL_release(BN_POOL *, unsigned int);

/************/
/* BN_STACK */
/************/

/* A wrapper to manage the "stack frames" */
typedef struct bignum_ctx_stack {
    /* Array of indexes into the bignum stack */
    unsigned int *indexes;
    /* Number of stack frames, and the size of the allocated array */
    unsigned int depth, size;
} BN_STACK;
static void BN_STACK_init(BN_STACK *);
static void BN_STACK_finish(BN_STACK *);
static int BN_STACK_push(BN_STACK *, unsigned int);
static unsigned int BN_STACK_pop(BN_STACK *);

/**********/
/* BN_CTX */
/**********/

/* The opaque BN_CTX type */
struct bignum_ctx {
    /* The bignum bundles */
    BN_POOL pool;
    /* The "stack frames", if you will */
    BN_STACK stack;
    /* The number of bignums currently assigned */
    unsigned int used;
    /* Depth of stack overflow */
    int err_stack;
    /* Block "gets" until an "end" (compatibility behaviour) */
    int too_many;
    /* Flags. */
    int flags;
    /* The library context */
    OSSL_LIB_CTX *libctx;
};

#ifndef FIPS_MODULE
/* Debugging functionality */
static void ctxdbg(BIO *channel, const char *text, BN_CTX *ctx)
{
    unsigned int bnidx = 0, fpidx = 0;
    BN_POOL_ITEM *item = ctx->pool.head;
    BN_STACK *stack = &ctx->stack;

    BIO_printf(channel, "%s\n", text);
    BIO_printf(channel, "  (%16p): ", (void*)ctx);
    while (bnidx < ctx->used) {
        BIO_printf(channel, "%03x ",
                   item->vals[bnidx++ % BN_CTX_POOL_SIZE].dmax);
        if (!(bnidx % BN_CTX_POOL_SIZE))
            item = item->next;
    }
    BIO_printf(channel, "\n");
    bnidx = 0;
    BIO_printf(channel, "   %16s : ", "");
    while (fpidx < stack->depth) {
        while (bnidx++ < stack->indexes[fpidx])
            BIO_printf(channel, "    ");
        BIO_printf(channel, "^^^ ");
        bnidx++;
        fpidx++;
    }
    BIO_printf(channel, "\n");
}

# define CTXDBG(str, ctx)           \
    OSSL_TRACE_BEGIN(BN_CTX) {      \
        ctxdbg(trc_out, str, ctx);  \
    } OSSL_TRACE_END(BN_CTX)
#else
/* We do not want tracing in FIPS module */
# define CTXDBG(str, ctx) do {} while(0)
#endif /* FIPS_MODULE */

BN_CTX *BN_CTX_new_ex(OSSL_LIB_CTX *ctx)
{
    BN_CTX *ret;

    if ((ret = OPENSSL_zalloc(sizeof(*ret))) == NULL)
        return NULL;
    /* Initialise the structure */
    BN_POOL_init(&ret->pool);
    BN_STACK_init(&ret->stack);
    ret->libctx = ctx;
    return ret;
}

#ifndef FIPS_MODULE
BN_CTX *BN_CTX_new(void)
{
    return BN_CTX_new_ex(NULL);
}
#endif

BN_CTX *BN_CTX_secure_new_ex(OSSL_LIB_CTX *ctx)
{
    BN_CTX *ret = BN_CTX_new_ex(ctx);

    if (ret != NULL)
        ret->flags = BN_FLG_SECURE;
    return ret;
}

#ifndef FIPS_MODULE
BN_CTX *BN_CTX_secure_new(void)
{
    return BN_CTX_secure_new_ex(NULL);
}
#endif

void BN_CTX_free(BN_CTX *ctx)
{
    if (ctx == NULL)
        return;
#ifndef FIPS_MODULE
    OSSL_TRACE_BEGIN(BN_CTX) {
        BN_POOL_ITEM *pool = ctx->pool.head;
        BIO_printf(trc_out,
                   "BN_CTX_free(): stack-size=%d, pool-bignums=%d\n",
                   ctx->stack.size, ctx->pool.size);
        BIO_printf(trc_out, "  dmaxs: ");
        while (pool) {
            unsigned loop = 0;
            while (loop < BN_CTX_POOL_SIZE)
                BIO_printf(trc_out, "%02x ", pool->vals[loop++].dmax);
            pool = pool->next;
        }
        BIO_printf(trc_out, "\n");
    } OSSL_TRACE_END(BN_CTX);
#endif
    BN_STACK_finish(&ctx->stack);
    BN_POOL_finish(&ctx->pool);
    OPENSSL_free(ctx);
}

void BN_CTX_start(BN_CTX *ctx)
{
    CTXDBG("ENTER BN_CTX_start()", ctx);
    /* If we're already overflowing ... */
    if (ctx->err_stack || ctx->too_many)
        ctx->err_stack++;
    /* (Try to) get a new frame pointer */
    else if (!BN_STACK_push(&ctx->stack, ctx->used)) {
        ERR_raise(ERR_LIB_BN, BN_R_TOO_MANY_TEMPORARY_VARIABLES);
        ctx->err_stack++;
    }
    CTXDBG("LEAVE BN_CTX_start()", ctx);
}

void BN_CTX_end(BN_CTX *ctx)
{
    if (ctx == NULL)
        return;
    CTXDBG("ENTER BN_CTX_end()", ctx);
    if (ctx->err_stack)
        ctx->err_stack--;
    else {
        unsigned int fp = BN_STACK_pop(&ctx->stack);
        /* Does this stack frame have anything to release? */
        if (fp < ctx->used)
            BN_POOL_release(&ctx->pool, ctx->used - fp);
        ctx->used = fp;
        /* Unjam "too_many" in case "get" had failed */
        ctx->too_many = 0;
    }
    CTXDBG("LEAVE BN_CTX_end()", ctx);
}

BIGNUM *BN_CTX_get(BN_CTX *ctx)
{
    BIGNUM *ret;

    CTXDBG("ENTER BN_CTX_get()", ctx);
    if (ctx->err_stack || ctx->too_many)
        return NULL;
    if ((ret = BN_POOL_get(&ctx->pool, ctx->flags)) == NULL) {
        /*
         * Setting too_many prevents repeated "get" attempts from cluttering
         * the error stack.
         */
        ctx->too_many = 1;
        ERR_raise(ERR_LIB_BN, BN_R_TOO_MANY_TEMPORARY_VARIABLES);
        return NULL;
    }
    /* OK, make sure the returned bignum is "zero" */
    BN_zero(ret);
    /* clear BN_FLG_CONSTTIME if leaked from previous frames */
    ret->flags &= (~BN_FLG_CONSTTIME);
    ctx->used++;
    CTXDBG("LEAVE BN_CTX_get()", ctx);
    return ret;
}

OSSL_LIB_CTX *ossl_bn_get_libctx(BN_CTX *ctx)
{
    if (ctx == NULL)
        return NULL;
    return ctx->libctx;
}

/************/
/* BN_STACK */
/************/

static void BN_STACK_init(BN_STACK *st)
{
    st->indexes = NULL;
    st->depth = st->size = 0;
}

static void BN_STACK_finish(BN_STACK *st)
{
    OPENSSL_free(st->indexes);
    st->indexes = NULL;
}


static int BN_STACK_push(BN_STACK *st, unsigned int idx)
{
    if (st->depth == st->size) {
        /* Need to expand */
        unsigned int newsize =
            st->size ? (st->size * 3 / 2) : BN_CTX_START_FRAMES;
        unsigned int *newitems;

        if ((newitems = OPENSSL_malloc(sizeof(*newitems) * newsize)) == NULL)
            return 0;
        if (st->depth)
            memcpy(newitems, st->indexes, sizeof(*newitems) * st->depth);
        OPENSSL_free(st->indexes);
        st->indexes = newitems;
        st->size = newsize;
    }
    st->indexes[(st->depth)++] = idx;
    return 1;
}

static unsigned int BN_STACK_pop(BN_STACK *st)
{
    return st->indexes[--(st->depth)];
}

/***********/
/* BN_POOL */
/***********/

static void BN_POOL_init(BN_POOL *p)
{
    p->head = p->current = p->tail = NULL;
    p->used = p->size = 0;
}

static void BN_POOL_finish(BN_POOL *p)
{
    unsigned int loop;
    BIGNUM *bn;

    while (p->head) {
        for (loop = 0, bn = p->head->vals; loop++ < BN_CTX_POOL_SIZE; bn++)
            if (bn->d)
                BN_clear_free(bn);
        p->current = p->head->next;
        OPENSSL_free(p->head);
        p->head = p->current;
    }
}


static BIGNUM *BN_POOL_get(BN_POOL *p, int flag)
{
    BIGNUM *bn;
    unsigned int loop;

    /* Full; allocate a new pool item and link it in. */
    if (p->used == p->size) {
        BN_POOL_ITEM *item;

        if ((item = OPENSSL_malloc(sizeof(*item))) == NULL)
            return NULL;
        for (loop = 0, bn = item->vals; loop++ < BN_CTX_POOL_SIZE; bn++) {
            bn_init(bn);
            if ((flag & BN_FLG_SECURE) != 0)
                BN_set_flags(bn, BN_FLG_SECURE);
        }
        item->prev = p->tail;
        item->next = NULL;

        if (p->head == NULL)
            p->head = p->current = p->tail = item;
        else {
            p->tail->next = item;
            p->tail = item;
            p->current = item;
        }
        p->size += BN_CTX_POOL_SIZE;
        p->used++;
        /* Return the first bignum from the new pool */
        return item->vals;
    }

    if (!p->used)
        p->current = p->head;
    else if ((p->used % BN_CTX_POOL_SIZE) == 0)
        p->current = p->current->next;
    return p->current->vals + ((p->used++) % BN_CTX_POOL_SIZE);
}

static void BN_POOL_release(BN_POOL *p, unsigned int num)
{
    unsigned int offset = (p->used - 1) % BN_CTX_POOL_SIZE;

    p->used -= num;
    while (num--) {
        bn_check_top(p->current->vals + offset);
        if (offset == 0) {
            offset = BN_CTX_POOL_SIZE - 1;
            p->current = p->current->prev;
        } else
            offset--;
    }
}
