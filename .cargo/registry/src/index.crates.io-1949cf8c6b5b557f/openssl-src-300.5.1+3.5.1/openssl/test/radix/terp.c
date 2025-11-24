/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include <openssl/ssl.h>
#include <openssl/quic.h>
#include <openssl/bio.h>
#include <openssl/lhash.h>
#include <openssl/rand.h>
#include "../testutil.h"
#include "internal/numbers.h"   /* UINT64_C */
#include "internal/time.h"      /* OSSL_TIME */

static const char *cert_file, *key_file;

/*
 * TERP - Test Executive Script Interpreter
 * ========================================
 */
typedef struct gen_ctx_st GEN_CTX;

typedef void (*script_gen_t)(GEN_CTX *ctx);

typedef struct script_info_st {
    /* name: A symbolic name, like simple_conn. */
    const char      *name;
    /* desc: A short, one-line description. */
    const char      *desc;
    const char      *file;
    int             line;
    /* gen_func: The script generation function. */
    script_gen_t    gen_func;
} SCRIPT_INFO;

struct gen_ctx_st {
    SCRIPT_INFO *script_info;
    const char  *cur_file;
    int         error, cur_line;
    const char  *first_error_msg, *first_error_file;
    int         first_error_line;

    uint8_t     *build_buf_beg, *build_buf_cur, *build_buf_end;
};

static int GEN_CTX_init(GEN_CTX *ctx, SCRIPT_INFO *script_info)
{
    ctx->script_info        = script_info;
    ctx->error              = 0;
    ctx->cur_file           = NULL;
    ctx->cur_line           = 0;
    ctx->first_error_msg    = NULL;
    ctx->first_error_line   = 0;
    ctx->build_buf_beg      = NULL;
    ctx->build_buf_cur      = NULL;
    ctx->build_buf_end      = NULL;
    return 1;
}

static void GEN_CTX_cleanup(GEN_CTX *ctx)
{
    OPENSSL_free(ctx->build_buf_beg);
    ctx->build_buf_beg = ctx->build_buf_cur = ctx->build_buf_end = NULL;
}

typedef struct terp_st TERP;

#define F_RET_SPIN_AGAIN        2
#define F_RET_SKIP_REST         3

#define F_SPIN_AGAIN()                          \
    do {                                        \
        ok                  = F_RET_SPIN_AGAIN; \
        fctx->spin_again    = 1;                \
        goto err;                               \
    } while (0)

#define F_SKIP_REST()                           \
    do {                                        \
        ok                  = F_RET_SKIP_REST;  \
        fctx->skip_rest     = 1;                \
        goto err;                               \
    } while (0)

typedef struct func_ctx_st {
    TERP    *terp;

    /*
     * Set to 1 inside a user function if the function should spin again.
     * Cleared automatically after the user function returns.
     */
    int     spin_again;

    /*
     * Immediately exit script successfully. Useful for skipping.
     */
    int     skip_rest;
} FUNC_CTX;

static ossl_inline int TERP_stk_pop(TERP *terp,
                                    void *buf, size_t buf_len);

#define TERP_STK_PUSH(terp, v)                                  \
    do {                                                        \
        if (!TEST_true(TERP_stk_push((terp), &(v), sizeof(v)))) \
            goto err;                                           \
    } while (0)

#define TERP_STK_POP(terp, v)                                   \
    do {                                                        \
        memset(&(v), 0, sizeof(v)); /* quiet warnings */        \
        if (!TEST_true(TERP_stk_pop((terp), &(v), sizeof(v))))  \
            goto err;                                           \
    } while (0)

#define TERP_STK_POP2(terp, a, b)                               \
    do {                                                        \
        TERP_STK_POP((terp), (b));                              \
        TERP_STK_POP((terp), (a));                              \
    } while (0)

#define F_PUSH(v)       TERP_STK_PUSH(fctx->terp, (v))
#define F_POP(v)        TERP_STK_POP (fctx->terp, (v))
#define F_POP2(a, b)    TERP_STK_POP2(fctx->terp, (a), (b))

typedef int (*helper_func_t)(FUNC_CTX *fctx);

#define DEF_FUNC(name) ossl_unused static int name(FUNC_CTX *fctx)

#define DEF_SCRIPT(name, desc)                          \
    static void script_gen_##name(GEN_CTX *ctx);        \
    static SCRIPT_INFO script_info_##name = {           \
        #name, desc, __FILE__, __LINE__,                \
        script_gen_##name                               \
    };                                                  \
    static void script_gen_##name(GEN_CTX *ctx)

enum {
    OPK_INVALID,
    OPK_END,
    OPK_PUSH_P,
    /*
     * This is exactly like PUSH_P, but the script dumper knows the pointer
     * points to a static NUL-terminated string and can therefore print it.
     */
    OPK_PUSH_PZ,
    OPK_PUSH_U64,
    /*
     * Could use OPK_PUSH_U64 for this but it's annoying to have to avoid using
     * size_t in case it is a different size.
     */
    OPK_PUSH_SIZE,
    OPK_FUNC,
    OPK_LABEL
};

static void *openc_alloc_space(GEN_CTX *ctx, size_t num_bytes);

#define DEF_ENCODER(name, type)                         \
    static void name(GEN_CTX *ctx, type v)              \
    {                                                   \
        void *dst = openc_alloc_space(ctx, sizeof(v));  \
        if (dst == NULL)                                \
            return;                                     \
                                                        \
        memcpy(dst, &v, sizeof(v));                     \
    }

DEF_ENCODER(openc_u64, uint64_t)
DEF_ENCODER(openc_size, size_t)
DEF_ENCODER(openc_p, void *)
DEF_ENCODER(openc_fp, helper_func_t)
#define openc_opcode    openc_u64

static void opgen_END(GEN_CTX *ctx)
{
    openc_opcode(ctx, OPK_END);
}

static ossl_unused void opgen_PUSH_P(GEN_CTX *ctx, void *p)
{
    openc_opcode(ctx, OPK_PUSH_P);
    openc_p(ctx, p);
}

static void opgen_PUSH_PZ(GEN_CTX *ctx, void *p)
{
    openc_opcode(ctx, OPK_PUSH_PZ);
    openc_p(ctx, p);
}

static void opgen_PUSH_U64(GEN_CTX *ctx, uint64_t v)
{
    openc_opcode(ctx, OPK_PUSH_U64);
    openc_u64(ctx, v);
}

ossl_unused static void opgen_PUSH_SIZE(GEN_CTX *ctx, size_t v)
{
    openc_opcode(ctx, OPK_PUSH_SIZE);
    openc_size(ctx, v);
}

ossl_unused static void opgen_FUNC(GEN_CTX *ctx, helper_func_t f,
                                   const char *f_name)
{
    openc_opcode(ctx, OPK_FUNC);
    openc_fp(ctx, f);
    openc_p(ctx, (void *)f_name);
}

ossl_unused static void opgen_LABEL(GEN_CTX *ctx, const char *name)
{
    openc_opcode(ctx, OPK_LABEL);
    openc_p(ctx, (void *)name);
}

static void opgen_set_line(GEN_CTX *ctx, const char *file, int line)
{
    ctx->cur_file = file;
    ctx->cur_line = line;
}

static ossl_unused void opgen_fail(GEN_CTX *ctx, const char *msg)
{
    if (!ctx->error) {
        ctx->first_error_file = ctx->cur_file;
        ctx->first_error_line = ctx->cur_line;
        ctx->first_error_msg  = msg;
    }

    ctx->error = 1;
}

#define OPGEN(n)        (opgen_set_line(ctx, __FILE__, __LINE__), \
                         opgen_##n)
#define OP_END()        OPGEN(END)      (ctx)
#define OP_PUSH_P(v)    OPGEN(PUSH_P)   (ctx, (v))
#define OP_PUSH_PZ(v)   OPGEN(PUSH_PZ)  (ctx, (v))
#define OP_PUSH_U64(v)  OPGEN(PUSH_U64) (ctx, (v))
#define OP_PUSH_SIZE(v) OPGEN(PUSH_SIZE) (ctx, (v))
#define OP_PUSH_BUFP(p, l)  (OP_PUSH_P(p), OP_PUSH_SIZE(l))
#define OP_PUSH_BUF(v)      OP_PUSH_BUFP(&(v), sizeof(v))
#define OP_PUSH_LREF(v) OPGEN(PUSH_LREF)(ctx, (lref))
#define OP_FUNC(f)      OPGEN(FUNC)     (ctx, (f), #f)
#define OP_LABEL(name)  OPGEN(LABEL)    (ctx, (name))
#define GEN_FAIL(msg)   OPGEN(fail)     (ctx, (msg))

static void *openc_alloc_space(GEN_CTX *ctx, size_t num_bytes)
{
    void *p;
    size_t cur_spare, old_size, new_size, off;

    cur_spare = ctx->build_buf_end - ctx->build_buf_cur;
    if (cur_spare < num_bytes) {
        off         = ctx->build_buf_cur - ctx->build_buf_beg;
        old_size    = ctx->build_buf_end - ctx->build_buf_beg;
        new_size    = (old_size == 0) ? 1024 : old_size * 2;
        p = OPENSSL_realloc(ctx->build_buf_beg, new_size);
        if (!TEST_ptr(p))
            return NULL;

        ctx->build_buf_beg = p;
        ctx->build_buf_cur = ctx->build_buf_beg + off;
        ctx->build_buf_end = ctx->build_buf_beg + new_size;
    }

    p = ctx->build_buf_cur;
    ctx->build_buf_cur += num_bytes;
    return p;
}

/*
 * Script Interpreter
 * ============================================================================
 */
typedef struct gen_script_st {
    const uint8_t *buf;
    size_t buf_len;
} GEN_SCRIPT;

static int GEN_CTX_finish(GEN_CTX *ctx, GEN_SCRIPT *script)
{
    script->buf         = ctx->build_buf_beg;
    script->buf_len     = ctx->build_buf_cur - ctx->build_buf_beg;
    ctx->build_buf_beg = ctx->build_buf_cur = ctx->build_buf_end = NULL;
    return 1;
}

static void GEN_SCRIPT_cleanup(GEN_SCRIPT *script)
{
    OPENSSL_free((char *)script->buf);

    script->buf     = NULL;
    script->buf_len = 0;
}

static int GEN_SCRIPT_init(GEN_SCRIPT *gen_script, SCRIPT_INFO *script_info)
{
    int ok = 0;
    GEN_CTX gctx;

    if (!TEST_true(GEN_CTX_init(&gctx, script_info)))
        return 0;

    script_info->gen_func(&gctx);
    opgen_END(&gctx);

    if (!TEST_false(gctx.error))
        goto err;

    if (!TEST_true(GEN_CTX_finish(&gctx, gen_script)))
        goto err;

    ok = 1;
err:
    if (!ok) {
        if (gctx.error)
            TEST_error("script generation failed: %s (at %s:%d)",
                       gctx.first_error_msg,
                       gctx.first_error_file,
                       gctx.first_error_line);

        GEN_CTX_cleanup(&gctx);
    }
    return ok;
}

typedef struct srdr_st {
    const uint8_t   *beg, *cur, *end, *save_cur;
} SRDR;

static void SRDR_init(SRDR *rdr, const uint8_t *buf, size_t buf_len)
{
    rdr->beg = rdr->cur = buf;
    rdr->end = rdr->beg + buf_len;
    rdr->save_cur = NULL;
}

static ossl_inline int SRDR_get_operand(SRDR *srdr, void *buf, size_t buf_len)
{
    if (!TEST_size_t_ge(srdr->end - srdr->cur, buf_len))
        return 0; /* malformed script */

    memcpy(buf, srdr->cur, buf_len);
    srdr->cur += buf_len;
    return 1;
}

static ossl_inline void SRDR_save(SRDR *srdr)
{
    srdr->save_cur = srdr->cur;
}

static ossl_inline void SRDR_restore(SRDR *srdr)
{
    srdr->cur = srdr->save_cur;
}

#define GET_OPERAND(srdr, v)                                        \
    do {                                                            \
        memset(&(v), 0, sizeof(v)); /* quiet uninitialized warn */  \
        if (!TEST_true(SRDR_get_operand(srdr, &(v), sizeof(v))))    \
            goto err;                                               \
    } while (0)


static void print_opc(BIO *bio, size_t op_num, size_t offset, const char *name)
{
    if (op_num != SIZE_MAX)
        BIO_printf(bio, "%3zu-  %4zx>\t%-8s \t", op_num,
                   offset, name);
    else
        BIO_printf(bio, "      %4zx>\t%-8s \t",
                   offset, name);
}

static int SRDR_print_one(SRDR *srdr, BIO *bio, size_t i, int *was_end)
{
    int ok = 0;
    const uint8_t *opc_start;
    uint64_t opc;

    if (was_end != NULL)
        *was_end = 0;

    opc_start = srdr->cur;
    GET_OPERAND(srdr, opc);

#define PRINT_OPC(name) print_opc(bio, i, (size_t)(opc_start - srdr->beg), #name)

    switch (opc) {
    case OPK_END:
        PRINT_OPC(END);
        opc_start = srdr->cur;
        if (was_end != NULL)
            *was_end = 1;
        break;
    case OPK_PUSH_P:
        {
            void *v;

            GET_OPERAND(srdr, v);
            PRINT_OPC(PUSH_P);
            BIO_printf(bio, "%20p", v);
        }
        break;
    case OPK_PUSH_PZ:
        {
            void *v;

            GET_OPERAND(srdr, v);
            PRINT_OPC(PUSH_PZ);
            if (v != NULL && strlen((const char *)v) == 1)
                BIO_printf(bio, "%20p (%s)", v, (const char *)v);
            else
                BIO_printf(bio, "%20p (\"%s\")", v, (const char *)v);
        }
        break;
    case OPK_PUSH_U64:
        {
            uint64_t v;

            GET_OPERAND(srdr, v);
            PRINT_OPC(PUSH_U64);
            BIO_printf(bio, "%#20llx (%llu)",
                       (unsigned long long)v, (unsigned long long)v);
        }
        break;
    case OPK_PUSH_SIZE:
        {
            size_t v;

            GET_OPERAND(srdr, v);
            PRINT_OPC(PUSH_SIZE);
            BIO_printf(bio, "%#20llx (%llu)",
                       (unsigned long long)v, (unsigned long long)v);
        }
        break;
    case OPK_FUNC:
        {
            helper_func_t v;
            void *f_name = NULL, *x = NULL;

            GET_OPERAND(srdr, v);
            GET_OPERAND(srdr, f_name);

            PRINT_OPC(FUNC);
            memcpy(&x, &v, sizeof(x) < sizeof(v) ? sizeof(x) : sizeof(v));
            BIO_printf(bio, "%s", (const char *)f_name);
        }
        break;
    case OPK_LABEL:
        {
            void *l_name;

            GET_OPERAND(srdr, l_name);

            BIO_printf(bio, "\n%s:\n", (const char *)l_name);
            PRINT_OPC(LABEL);
        }
        break;
    default:
        TEST_error("unsupported opcode while printing: %llu",
                   (unsigned long long)opc);
        goto err;
    }

    ok = 1;
err:
    return ok;
}

static int GEN_SCRIPT_print(GEN_SCRIPT *gen_script, BIO *bio,
                            const SCRIPT_INFO *script_info)
{
    int ok = 0;
    size_t i;
    SRDR srdr_v, *srdr = &srdr_v;
    int was_end = 0;

    SRDR_init(srdr, gen_script->buf, gen_script->buf_len);

    if (script_info != NULL) {
        BIO_printf(bio, "\nGenerated script for '%s':\n",
                               script_info->name);
        BIO_printf(bio, "\n--GENERATED-------------------------------------"
                  "----------------------\n");
        BIO_printf(bio, "  # NAME:\n  #   %s\n",
                   script_info->name);
        BIO_printf(bio, "  # SOURCE:\n  #   %s:%d\n",
                   script_info->file, script_info->line);
        BIO_printf(bio, "  # DESCRIPTION:\n  #   %s\n", script_info->desc);
    }

    for (i = 0; !was_end; ++i) {
        BIO_printf(bio, "\n");

        if (!TEST_true(SRDR_print_one(srdr, bio, i, &was_end)))
            goto err;
    }

    if (script_info != NULL) {
        const unsigned char *opc_start = srdr->cur;

        BIO_printf(bio, "\n");
        PRINT_OPC(+++);
        BIO_printf(bio, "\n------------------------------------------------"
                  "----------------------\n\n");
    }

    ok = 1;
err:
    return ok;
}

static void SCRIPT_INFO_print(SCRIPT_INFO *script_info, BIO *bio, int error,
                              const char *msg)
{
    if (error)
        TEST_error("%s: script '%s' (%s)",
                   msg, script_info->name, script_info->desc);
    else
        TEST_info("%s: script '%s' (%s)",
                  msg, script_info->name, script_info->desc);
}

typedef struct terp_config_st {
    BIO         *debug_bio;

    OSSL_TIME   (*now_cb)(void *arg);
    void        *now_cb_arg;

    int         (*per_op_cb)(TERP *terp, void *arg);
    void        *per_op_cb_arg;

    OSSL_TIME   max_execution_time; /* duration */
} TERP_CONFIG;

#define TERP_DEFAULT_MAX_EXECUTION_TIME     (ossl_ms2time(3000))

struct terp_st {
    TERP_CONFIG         cfg;
    const SCRIPT_INFO   *script_info;
    const GEN_SCRIPT    *gen_script;
    SRDR                srdr;
    uint8_t             *stk_beg, *stk_cur, *stk_end, *stk_save_cur;
    FUNC_CTX            fctx;
    uint64_t            ops_executed;
    int                 log_execute;
    OSSL_TIME           start_time, deadline_time;
};

static int TERP_init(TERP *terp,
                     const TERP_CONFIG *cfg,
                     const SCRIPT_INFO *script_info,
                     const GEN_SCRIPT *gen_script)
{
    if (!TEST_true(cfg->now_cb != NULL))
        return 0;

    terp->cfg               = *cfg;
    terp->script_info       = script_info;
    terp->gen_script        = gen_script;
    terp->fctx.terp         = terp;
    terp->fctx.spin_again   = 0;
    terp->fctx.skip_rest    = 0;
    terp->stk_beg           = NULL;
    terp->stk_cur           = NULL;
    terp->stk_end           = NULL;
    terp->stk_save_cur      = NULL;
    terp->ops_executed      = 0;
    terp->log_execute       = 1;

    if (ossl_time_is_zero(terp->cfg.max_execution_time))
        terp->cfg.max_execution_time = TERP_DEFAULT_MAX_EXECUTION_TIME;

    return 1;
}

static void TERP_cleanup(TERP *terp)
{
    if (terp->script_info == NULL)
        return;

    OPENSSL_free(terp->stk_beg);
    terp->stk_beg = terp->stk_cur = terp->stk_end = NULL;
    terp->script_info = NULL;
}

static int TERP_stk_ensure_capacity(TERP *terp, size_t spare)
{
    uint8_t *p;
    size_t old_size, new_size, off;

    old_size = terp->stk_end - terp->stk_beg;
    if (old_size >= spare)
        return 1;

    off         = terp->stk_end - terp->stk_cur;
    new_size    = old_size != 0 ? old_size * 2 : 256;
    p = OPENSSL_realloc(terp->stk_beg, new_size);
    if (!TEST_ptr(p))
        return 0;

    terp->stk_beg = p;
    terp->stk_end = terp->stk_beg + new_size;
    terp->stk_cur = terp->stk_end - off;
    return 1;
}

static ossl_inline int TERP_stk_push(TERP *terp,
                                     const void *buf, size_t buf_len)
{
    if (!TEST_true(TERP_stk_ensure_capacity(terp, buf_len)))
        return 0;

    terp->stk_cur -= buf_len;
    memcpy(terp->stk_cur, buf, buf_len);
    return 1;
}

static ossl_inline int TERP_stk_pop(TERP *terp,
                                    void *buf, size_t buf_len)
{
    if (!TEST_size_t_ge(terp->stk_end - terp->stk_cur, buf_len))
        return 0;

    memcpy(buf, terp->stk_cur, buf_len);
    terp->stk_cur += buf_len;
    return 1;
}

static void TERP_print_stack(TERP *terp, BIO *bio, const char *header)
{
    test_output_memory(header, terp->stk_cur, terp->stk_end - terp->stk_cur);
    BIO_printf(bio, "  (%zu bytes)\n", (size_t)(terp->stk_end - terp->stk_cur));
    BIO_printf(bio, "\n");
}

#define TERP_GET_OPERAND(v) GET_OPERAND(&terp->srdr, (v))

#define TERP_SPIN_AGAIN()                       \
    do {                                        \
        SRDR_restore(&terp->srdr);              \
        terp->stk_cur = terp->stk_save_cur;     \
        ++spin_count;                           \
        goto spin_again;                        \
    } while (0)

static OSSL_TIME TERP_now(TERP *terp)
{
    return terp->cfg.now_cb(terp->cfg.now_cb_arg);
}

static void TERP_log_spin(TERP *terp, size_t spin_count)
{
    if (spin_count > 0)
        BIO_printf(terp->cfg.debug_bio, "           \t\t(span %zu times)\n",
                   spin_count);
}

static int TERP_execute(TERP *terp)
{
    int ok = 0;
    uint64_t opc;
    size_t op_num = 0;
    int in_debug_output = 0;
    size_t spin_count = 0;
    BIO *debug_bio = terp->cfg.debug_bio;

    SRDR_init(&terp->srdr, terp->gen_script->buf, terp->gen_script->buf_len);

    terp->start_time    = TERP_now(terp);
    terp->deadline_time = ossl_time_add(terp->start_time,
                                        terp->cfg.max_execution_time);

    for (;;) {
        if (terp->log_execute) {
            SRDR srdr_copy = terp->srdr;

            if (!in_debug_output) {
                BIO_printf(debug_bio, "\n--EXECUTION-----------------------------"
                          "------------------------------\n");
                in_debug_output = 1;
            }

            TERP_log_spin(terp, spin_count);
            if (!TEST_true(SRDR_print_one(&srdr_copy, debug_bio, SIZE_MAX, NULL)))
                goto err;

            BIO_printf(debug_bio, "\n");
        }

        TERP_GET_OPERAND(opc);
        ++op_num;
        SRDR_save(&terp->srdr);
        terp->stk_save_cur = terp->stk_cur;
        spin_count = 0;

        ++terp->ops_executed;

spin_again:
        if (ossl_time_compare(TERP_now(terp), terp->deadline_time) >= 0) {
            TEST_error("timed out while executing op %zu", op_num);
            if (terp->log_execute)
                TERP_log_spin(terp, spin_count);
            goto err;
        }

        if (terp->cfg.per_op_cb != NULL)
            if (!TEST_true(terp->cfg.per_op_cb(terp, terp->cfg.per_op_cb_arg))) {
                TEST_error("pre-operation processing failed at op %zu", op_num);
                if (terp->log_execute)
                    TERP_log_spin(terp, spin_count);
                goto err;
            }

        switch (opc) {
        case OPK_END:
            goto stop;
        case OPK_PUSH_P:
        case OPK_PUSH_PZ:
            {
                void *v;

                TERP_GET_OPERAND(v);
                TERP_STK_PUSH(terp, v);
            }
            break;
        case OPK_PUSH_U64:
            {
                uint64_t v;

                TERP_GET_OPERAND(v);
                TERP_STK_PUSH(terp, v);
            }
            break;
        case OPK_PUSH_SIZE:
            {
                size_t v;

                TERP_GET_OPERAND(v);
                TERP_STK_PUSH(terp, v);
            }
            break;
        case OPK_LABEL:
            {
                const char *l_name;

                TERP_GET_OPERAND(l_name);
                /* no-op */
            }
            break;
        case OPK_FUNC:
            {
                helper_func_t v;
                const void *f_name;
                int ret;

                TERP_GET_OPERAND(v);
                TERP_GET_OPERAND(f_name);

                if (!TEST_true(v != NULL))
                    goto err;

                ret = v(&terp->fctx);

                if (terp->fctx.skip_rest) {
                    if (!TEST_int_eq(ret, F_RET_SKIP_REST))
                        goto err;

                    if (terp->log_execute)
                        BIO_printf(terp->cfg.debug_bio, "           \t\t(skipping)\n");

                    terp->fctx.skip_rest = 0;
                    goto stop;
                } else if (terp->fctx.spin_again) {
                    if (!TEST_int_eq(ret, F_RET_SPIN_AGAIN))
                        goto err;

                    terp->fctx.spin_again = 0;
                    TERP_SPIN_AGAIN();
                } else {
                    if (!TEST_false(terp->fctx.spin_again))
                        goto err;

                    if (ret != 1) {
                        TEST_error("op %zu (FUNC %s) failed with return value %d",
                                   op_num, (const char *)f_name, ret);
                        goto err;
                    }
                }
            }
            break;
        default:
            TEST_error("unknown opcode: %llu", (unsigned long long)opc);
            goto err;
        }
    }

stop:
    ok = 1;
err:
    if (in_debug_output)
        BIO_printf(debug_bio, "----------------------------------------"
                   "------------------------------\n");

    if (!ok) {
        TEST_error("FAILED while executing script: %s at op %zu, error stack:",
                   terp->script_info->name, op_num);
        ERR_print_errors(terp->cfg.debug_bio);
        BIO_printf(debug_bio, "\n");
    } else if (ERR_peek_last_error() != 0) {
        TEST_info("WARNING: errors on error stack despite success:");
        ERR_print_errors(terp->cfg.debug_bio);
        BIO_printf(debug_bio, "\n");
    }

    return ok;
}

static int TERP_run(SCRIPT_INFO *script_info, TERP_CONFIG *cfg)
{
    int ok = 0, have_terp = 0;
    TERP terp;
    GEN_SCRIPT gen_script = {0};
    BIO *debug_bio = cfg->debug_bio;

    SCRIPT_INFO_print(script_info, debug_bio, /*error=*/0, "generating script");

    /* Generate the script by calling the generator function. */
    if (!TEST_true(GEN_SCRIPT_init(&gen_script, script_info))) {
        SCRIPT_INFO_print(script_info, debug_bio, /*error=*/1,
                          "error while generating script");
        goto err;
    }

    /* Output the script for debugging purposes. */
    if (!TEST_true(GEN_SCRIPT_print(&gen_script, debug_bio, script_info))) {
        SCRIPT_INFO_print(script_info, debug_bio, /*error=*/1,
                          "error while printing script");
        goto err;
    }

    /* Execute the script. */
    if (!TEST_true(TERP_init(&terp, cfg, script_info, &gen_script)))
        goto err;

    have_terp = 1;

    SCRIPT_INFO_print(script_info, debug_bio, /*error=*/0, "executing script");

    if (!TERP_execute(&terp))
        goto err;

    if (terp.stk_end - terp.stk_cur != 0) {
        TEST_error("stack not empty: %zu bytes left",
                   (size_t)(terp.stk_end - terp.stk_cur));
        goto err;
    }

    ok = 1;
err:
    if (have_terp) {
        TERP_print_stack(&terp, debug_bio, "Final state of stack");
        TERP_cleanup(&terp);
    }

    GEN_SCRIPT_cleanup(&gen_script);
    BIO_printf(debug_bio, "Stats:\n  Ops executed: %16llu\n\n",
               (unsigned long long)terp.ops_executed);
    SCRIPT_INFO_print(script_info, debug_bio, /*error=*/!ok,
                      ok ? "completed" : "failed, exiting");
    return ok;
}

#define SCRIPT(name)    (&script_info_##name)
#define USE(name)       SCRIPT(name),
