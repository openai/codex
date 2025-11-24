/*
 * Copyright 2021-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Some ctrls depend on deprecated functionality.  We trust that this is
 * functionality that remains internally even when 'no-deprecated' is
 * configured.  When we drop #legacy EVP_PKEYs, this source should be
 * possible to drop as well.
 */
#include "internal/deprecated.h"

#include <string.h>

/* The following includes get us all the EVP_PKEY_CTRL macros */
#include <openssl/dh.h>
#include <openssl/dsa.h>
#include <openssl/ec.h>
#include <openssl/rsa.h>
#include <openssl/kdf.h>

/* This include gets us all the OSSL_PARAM key string macros */
#include <openssl/core_names.h>

#include <openssl/err.h>
#include <openssl/evperr.h>
#include <openssl/params.h>
#include "internal/nelem.h"
#include "internal/cryptlib.h"
#include "internal/ffc.h"
#include "crypto/evp.h"
#include "crypto/dh.h"
#include "crypto/ec.h"

struct translation_ctx_st;       /* Forwarding */
struct translation_st;           /* Forwarding */

/*
 * The fixup_args functions are called with the following parameters:
 *
 * |state|              The state we're called in, explained further at the
 *                      end of this comment.
 * |translation|        The translation item, to be pilfered for data as
 *                      necessary.
 * |ctx|                The translation context, which contains copies of
 *                      the following arguments, applicable according to
 *                      the caller.  All of the attributes in this context
 *                      may be freely modified by the fixup_args function.
 *                      For cleanup, call cleanup_translation_ctx().
 *
 * The |state| tells the fixup_args function something about the caller and
 * what they may expect:
 *
 * PKEY                         The fixup_args function has been called
 *                              from an EVP_PKEY payload getter / setter,
 *                              and is fully responsible for getting or
 *                              setting the requested data.  With this
 *                              state, the fixup_args function is expected
 *                              to use or modify |*params|, depending on
 *                              |action_type|.
 *
 * PRE_CTRL_TO_PARAMS           The fixup_args function has been called
 * POST_CTRL_TO_PARAMS          from EVP_PKEY_CTX_ctrl(), to help with
 *                              translating the ctrl data to an OSSL_PARAM
 *                              element or back.  The calling sequence is
 *                              as follows:
 *
 *                              1. fixup_args(PRE_CTRL_TO_PARAMS, ...)
 *                              2. EVP_PKEY_CTX_set_params() or
 *                                 EVP_PKEY_CTX_get_params()
 *                              3. fixup_args(POST_CTRL_TO_PARAMS, ...)
 *
 *                              With the PRE_CTRL_TO_PARAMS state, the
 *                              fixup_args function is expected to modify
 *                              the passed |*params| in whatever way
 *                              necessary, when |action_type == OSSL_ACTION_SET|.
 *                              With the POST_CTRL_TO_PARAMS state, the
 *                              fixup_args function is expected to modify
 *                              the passed |p2| in whatever way necessary,
 *                              when |action_type == OSSL_ACTION_GET|.
 *
 *                              The return value from the fixup_args call
 *                              with the POST_CTRL_TO_PARAMS state becomes
 *                              the return value back to EVP_PKEY_CTX_ctrl().
 *
 * CLEANUP_CTRL_TO_PARAMS       The cleanup_args functions has been called
 *                              from EVP_PKEY_CTX_ctrl(), to clean up what
 *                              the fixup_args function has done, if needed.
 *
 *
 * PRE_CTRL_STR_TO_PARAMS       The fixup_args function has been called
 * POST_CTRL_STR_TO_PARAMS      from EVP_PKEY_CTX_ctrl_str(), to help with
 *                              translating the ctrl_str data to an
 *                              OSSL_PARAM element or back.  The calling
 *                              sequence is as follows:
 *
 *                              1. fixup_args(PRE_CTRL_STR_TO_PARAMS, ...)
 *                              2. EVP_PKEY_CTX_set_params() or
 *                                 EVP_PKEY_CTX_get_params()
 *                              3. fixup_args(POST_CTRL_STR_TO_PARAMS, ...)
 *
 *                              With the PRE_CTRL_STR_TO_PARAMS state,
 *                              the fixup_args function is expected to
 *                              modify the passed |*params| in whatever
 *                              way necessary, when |action_type == OSSL_ACTION_SET|.
 *                              With the POST_CTRL_STR_TO_PARAMS state,
 *                              the fixup_args function is only expected
 *                              to return a value.
 *
 * CLEANUP_CTRL_STR_TO_PARAMS   The cleanup_args functions has been called
 *                              from EVP_PKEY_CTX_ctrl_str(), to clean up
 *                              what the fixup_args function has done, if
 *                              needed.
 *
 * PRE_PARAMS_TO_CTRL           The fixup_args function has been called
 * POST_PARAMS_TO_CTRL          from EVP_PKEY_CTX_get_params() or
 *                              EVP_PKEY_CTX_set_params(), to help with
 *                              translating the OSSL_PARAM data to the
 *                              corresponding EVP_PKEY_CTX_ctrl() arguments
 *                              or the other way around.  The calling
 *                              sequence is as follows:
 *
 *                              1. fixup_args(PRE_PARAMS_TO_CTRL, ...)
 *                              2. EVP_PKEY_CTX_ctrl()
 *                              3. fixup_args(POST_PARAMS_TO_CTRL, ...)
 *
 *                              With the PRE_PARAMS_TO_CTRL state, the
 *                              fixup_args function is expected to modify
 *                              the passed |p1| and |p2| in whatever way
 *                              necessary, when |action_type == OSSL_ACTION_SET|.
 *                              With the POST_PARAMS_TO_CTRL state, the
 *                              fixup_args function is expected to
 *                              modify the passed |*params| in whatever
 *                              way necessary, when |action_type == OSSL_ACTION_GET|.
 *
 * CLEANUP_PARAMS_TO_CTRL       The cleanup_args functions has been called
 *                              from EVP_PKEY_CTX_get_params() or
 *                              EVP_PKEY_CTX_set_params(), to clean up what
 *                              the fixup_args function has done, if needed.
 */
enum state {
    PKEY,
    PRE_CTRL_TO_PARAMS, POST_CTRL_TO_PARAMS, CLEANUP_CTRL_TO_PARAMS,
    PRE_CTRL_STR_TO_PARAMS, POST_CTRL_STR_TO_PARAMS, CLEANUP_CTRL_STR_TO_PARAMS,
    PRE_PARAMS_TO_CTRL, POST_PARAMS_TO_CTRL, CLEANUP_PARAMS_TO_CTRL
};
enum action {
    OSSL_ACTION_NONE = 0, OSSL_ACTION_GET = 1, OSSL_ACTION_SET = 2
};
typedef int fixup_args_fn(enum state state,
                          const struct translation_st *translation,
                          struct translation_ctx_st *ctx);
typedef int cleanup_args_fn(enum state state,
                            const struct translation_st *translation,
                            struct translation_ctx_st *ctx);

struct translation_ctx_st {
    /*
     * The EVP_PKEY_CTX, for calls on that structure, to be pilfered for data
     * as necessary.
     */
    EVP_PKEY_CTX *pctx;
    /*
     * The action type (OSSL_ACTION_GET or OSSL_ACTION_SET). This may be 0 in some cases, and should
     * be modified by the fixup_args function in the PRE states.  It should
     * otherwise remain untouched once set.
     */
    enum action action_type;
    /*
     * For ctrl to params translation, the actual ctrl command number used.
     * For params to ctrl translation, 0.
     */
    int ctrl_cmd;
    /*
     * For ctrl_str to params translation, the actual ctrl command string
     * used.  In this case, the (string) value is always passed as |p2|.
     * For params to ctrl translation, this is NULL.  Along with it is also
     * and indicator whether it matched |ctrl_str| or |ctrl_hexstr| in the
     * translation item.
     */
    const char *ctrl_str;
    int ishex;
    /* the ctrl-style int argument. */
    int p1;
    /* the ctrl-style void* argument. */
    void *p2;
    /* a size, for passing back the |p2| size where applicable */
    size_t sz;
    /* pointer to the OSSL_PARAM-style params array. */
    OSSL_PARAM *params;

    /*-
     * The following are used entirely internally by the fixup_args functions
     * and should not be touched by the callers, at all.
     */

    /*
     * Copy of the ctrl-style void* argument, if the fixup_args function
     * needs to manipulate |p2| but wants to remember original.
     */
    void *orig_p2;
    /* Diverse types of storage for the needy. */
    char name_buf[OSSL_MAX_NAME_SIZE];
    void *allocated_buf;
    void *bufp;
    size_t buflen;
};

struct translation_st {
    /*-
     * What this table item does.
     *
     * If the item has this set to 0, it means that both OSSL_ACTION_GET and OSSL_ACTION_SET are
     * supported, and |fixup_args| will determine which it is.  This is to
     * support translations of ctrls where the action type depends on the
     * value of |p1| or |p2| (ctrls are really bi-directional, but are
     * seldom used that way).
     *
     * This can be also used in the lookup template when it looks up by
     * OSSL_PARAM key, to indicate if a setter or a getter called.
     */
    enum action action_type;

    /*-
     * Conditions, for params->ctrl translations.
     *
     * In table item, |keytype1| and |keytype2| can be set to -1 to indicate
     * that this item supports all key types (or rather, that |fixup_args|
     * will check and return an error if it's not supported).
     * Any of these may be set to 0 to indicate that they are unset.
     */
    int keytype1;    /* The EVP_PKEY_XXX type, i.e. NIDs. #legacy */
    int keytype2;    /* Another EVP_PKEY_XXX type, used for aliases */
    int optype;      /* The operation type */

    /*
     * Lookup and translation attributes
     *
     * |ctrl_num|, |ctrl_str|, |ctrl_hexstr| and |param_key| are lookup
     * attributes.
     *
     * |ctrl_num| may be 0 or that |param_key| may be NULL in the table item,
     * but not at the same time.  If they are, they are simply not used for
     * lookup.
     * When |ctrl_num| == 0, no ctrl will be called.  Likewise, when
     * |param_key| == NULL, no OSSL_PARAM setter/getter will be called.
     * In that case the treatment of the translation item relies entirely on
     * |fixup_args|, which is then assumed to have side effects.
     *
     * As a special case, it's possible to set |ctrl_hexstr| and assign NULL
     * to |ctrl_str|.  That will signal to default_fixup_args() that the
     * value must always be interpreted as hex.
     */
    int ctrl_num;            /* EVP_PKEY_CTRL_xxx */
    const char *ctrl_str;    /* The corresponding ctrl string */
    const char *ctrl_hexstr; /* The alternative "hex{str}" ctrl string */
    const char *param_key;   /* The corresponding OSSL_PARAM key */
    /*
     * The appropriate OSSL_PARAM data type.  This may be 0 to indicate that
     * this OSSL_PARAM may have more than one data type, depending on input
     * material.  In this case, |fixup_args| is expected to check and handle
     * it.
     */
    unsigned int param_data_type;

    /*
     * Fixer functions
     *
     * |fixup_args| is always called before (for OSSL_ACTION_SET) or after (for OSSL_ACTION_GET)
     * the actual ctrl / OSSL_PARAM function.
     */
    fixup_args_fn *fixup_args;
};

/*-
 * Fixer function implementations
 * ==============================
 */

/*
 * default_check isn't a fixer per se, but rather a helper function to
 * perform certain standard checks.
 */
static int default_check(enum state state,
                         const struct translation_st *translation,
                         const struct translation_ctx_st *ctx)
{
    switch (state) {
    default:
        break;
    case PRE_CTRL_TO_PARAMS:
        if (!ossl_assert(translation != NULL)) {
            ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
            return -2;
        }
        if (!ossl_assert(translation->param_key != 0)
            || !ossl_assert(translation->param_data_type != 0)) {
            ERR_raise(ERR_LIB_EVP, ERR_R_INTERNAL_ERROR);
            return -1;
        }
        break;
    case PRE_CTRL_STR_TO_PARAMS:
        /*
         * For ctrl_str to params translation, we allow direct use of
         * OSSL_PARAM keys as ctrl_str keys.  Therefore, it's possible that
         * we end up with |translation == NULL|, which is fine.  The fixup
         * function will have to deal with it carefully.
         */
        if (translation != NULL) {
            if (!ossl_assert(translation->action_type != OSSL_ACTION_GET)) {
                ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
                return -2;
            }
            if (!ossl_assert(translation->param_key != NULL)
                || !ossl_assert(translation->param_data_type != 0)) {
                ERR_raise(ERR_LIB_EVP, ERR_R_INTERNAL_ERROR);
                return 0;
            }
        }
        break;
    case PRE_PARAMS_TO_CTRL:
    case POST_PARAMS_TO_CTRL:
        if (!ossl_assert(translation != NULL)) {
            ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
            return -2;
        }
        if (!ossl_assert(translation->ctrl_num != 0)
            || !ossl_assert(translation->param_data_type != 0)) {
            ERR_raise(ERR_LIB_EVP, ERR_R_INTERNAL_ERROR);
            return -1;
        }
    }

    /* Nothing else to check */
    return 1;
}

/*-
 * default_fixup_args fixes up all sorts of arguments, governed by the
 * diverse attributes in the translation item.  It covers all "standard"
 * base ctrl functionality, meaning it can handle basic conversion of
 * data between p1+p2 (OSSL_ACTION_SET) or return value+p2 (OSSL_ACTION_GET) as long as the values
 * don't have extra semantics (such as NIDs, OIDs, that sort of stuff).
 * Extra semantics must be handled via specific fixup_args functions.
 *
 * The following states and action type combinations have standard handling
 * done in this function:
 *
 * PRE_CTRL_TO_PARAMS, 0                - ERROR.  action type must be
 *                                        determined by a fixup function.
 * PRE_CTRL_TO_PARAMS, OSSL_ACTION_SET
 *                   | OSSL_ACTION_GET  - |p1| and |p2| are converted to an
 *                                        OSSL_PARAM according to the data
 *                                        type given in |translattion|.
 *                                        For OSSL_PARAM_UNSIGNED_INTEGER,
 *                                        a BIGNUM passed as |p2| is accepted.
 * POST_CTRL_TO_PARAMS, OSSL_ACTION_GET - If the OSSL_PARAM data type is a
 *                                        STRING or PTR type, |p1| is set
 *                                        to the OSSL_PARAM return size, and
 *                                        |p2| is set to the string.
 * PRE_CTRL_STR_TO_PARAMS,
 *                     !OSSL_ACTION_SET - ERROR.  That combination is not
 *                                        supported.
 * PRE_CTRL_STR_TO_PARAMS,
 *                      OSSL_ACTION_SET - |p2| is taken as a string, and is
 *                                        converted to an OSSL_PARAM in a
 *                                        standard manner, guided by the
 *                                        param key and data type from
 *                                        |translation|.
 * PRE_PARAMS_TO_CTRL, OSSL_ACTION_SET  - the OSSL_PARAM is converted to
 *                                        |p1| and |p2| according to the
 *                                        data type given in |translation|
 *                                        For OSSL_PARAM_UNSIGNED_INTEGER,
 *                                        if |p2| is non-NULL, then |*p2|
 *                                        is assigned a BIGNUM, otherwise
 *                                        |p1| is assigned an unsigned int.
 * POST_PARAMS_TO_CTRL, OSSL_ACTION_GET - |p1| and |p2| are converted to
 *                                        an OSSL_PARAM, in the same manner
 *                                        as for the combination of
 *                                        PRE_CTRL_TO_PARAMS, OSSL_ACTION_SET.
 */
static int default_fixup_args(enum state state,
                              const struct translation_st *translation,
                              struct translation_ctx_st *ctx)
{
    int ret;

    if ((ret = default_check(state, translation, ctx)) <= 0)
        return ret;

    switch (state) {
    default:
        /* For states this function should never have been called with */
        ERR_raise_data(ERR_LIB_EVP, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED,
                       "[action:%d, state:%d]", ctx->action_type, state);
        return 0;

    /*
     * PRE_CTRL_TO_PARAMS and POST_CTRL_TO_PARAMS handle ctrl to params
     * translations.  PRE_CTRL_TO_PARAMS is responsible for preparing
     * |*params|, and POST_CTRL_TO_PARAMS is responsible for bringing the
     * result back to |*p2| and the return value.
     */
    case PRE_CTRL_TO_PARAMS:
        /* This is ctrl to params translation, so we need an OSSL_PARAM key */
        if (ctx->action_type == OSSL_ACTION_NONE) {
            /*
             * No action type is an error here.  That's a case for a
             * special fixup function.
             */
            ERR_raise_data(ERR_LIB_EVP, ERR_R_UNSUPPORTED,
                           "[action:%d, state:%d]", ctx->action_type, state);
            return 0;
        }

        if (translation->optype != 0) {
            if ((EVP_PKEY_CTX_IS_SIGNATURE_OP(ctx->pctx)
                 && ctx->pctx->op.sig.algctx == NULL)
                || (EVP_PKEY_CTX_IS_DERIVE_OP(ctx->pctx)
                    && ctx->pctx->op.kex.algctx == NULL)
                || (EVP_PKEY_CTX_IS_ASYM_CIPHER_OP(ctx->pctx)
                    && ctx->pctx->op.ciph.algctx == NULL)
                || (EVP_PKEY_CTX_IS_KEM_OP(ctx->pctx)
                    && ctx->pctx->op.encap.algctx == NULL)
                /*
                 * The following may be unnecessary, but we have them
                 * for good measure...
                 */
                || (EVP_PKEY_CTX_IS_GEN_OP(ctx->pctx)
                    && ctx->pctx->op.keymgmt.genctx == NULL)
                || (EVP_PKEY_CTX_IS_FROMDATA_OP(ctx->pctx)
                    && ctx->pctx->op.keymgmt.genctx == NULL)) {
                ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
                /* Uses the same return values as EVP_PKEY_CTX_ctrl */
                return -2;
            }
        }

        /*
         * OSSL_PARAM_construct_TYPE() works equally well for OSSL_ACTION_SET and OSSL_ACTION_GET.
         */
        switch (translation->param_data_type) {
        case OSSL_PARAM_INTEGER:
            *ctx->params = OSSL_PARAM_construct_int(translation->param_key,
                                                    &ctx->p1);
            break;
        case OSSL_PARAM_UNSIGNED_INTEGER:
            /*
             * BIGNUMs are passed via |p2|.  For all ctrl's that just want
             * to pass a simple integer via |p1|, |p2| is expected to be
             * NULL.
             *
             * Note that this allocates a buffer, which the cleanup function
             * must deallocate.
             */
            if (ctx->p2 != NULL) {
                if (ctx->action_type == OSSL_ACTION_SET) {
                    ctx->buflen = BN_num_bytes(ctx->p2);
                    if ((ctx->allocated_buf
                         = OPENSSL_malloc(ctx->buflen)) == NULL)
                        return 0;
                    if (BN_bn2nativepad(ctx->p2,
                                         ctx->allocated_buf, ctx->buflen) < 0) {
                        OPENSSL_free(ctx->allocated_buf);
                        ctx->allocated_buf = NULL;
                        return 0;
                    }
                    *ctx->params =
                        OSSL_PARAM_construct_BN(translation->param_key,
                                                ctx->allocated_buf,
                                                ctx->buflen);
                } else {
                    /*
                     * No support for getting a BIGNUM by ctrl, this needs
                     * fixup_args function support.
                     */
                    ERR_raise_data(ERR_LIB_EVP, ERR_R_UNSUPPORTED,
                                   "[action:%d, state:%d] trying to get a "
                                   "BIGNUM via ctrl call",
                                   ctx->action_type, state);
                    return 0;
                }
            } else {
                *ctx->params =
                    OSSL_PARAM_construct_uint(translation->param_key,
                                              (unsigned int *)&ctx->p1);
            }
            break;
        case OSSL_PARAM_UTF8_STRING:
            *ctx->params =
                OSSL_PARAM_construct_utf8_string(translation->param_key,
                                                 ctx->p2, (size_t)ctx->p1);
            break;
        case OSSL_PARAM_UTF8_PTR:
            *ctx->params =
                OSSL_PARAM_construct_utf8_ptr(translation->param_key,
                                              ctx->p2, (size_t)ctx->p1);
            break;
        case OSSL_PARAM_OCTET_STRING:
            *ctx->params =
                OSSL_PARAM_construct_octet_string(translation->param_key,
                                                  ctx->p2, (size_t)ctx->p1);
            break;
        case OSSL_PARAM_OCTET_PTR:
            *ctx->params =
                OSSL_PARAM_construct_octet_ptr(translation->param_key,
                                               ctx->p2, (size_t)ctx->p1);
            break;
        }
        break;
    case POST_CTRL_TO_PARAMS:
        /*
         * Because EVP_PKEY_CTX_ctrl() returns the length of certain objects
         * as its return value, we need to ensure that we do it here as well,
         * for the OSSL_PARAM data types where this makes sense.
         */
        if (ctx->action_type == OSSL_ACTION_GET) {
            switch (translation->param_data_type) {
            case OSSL_PARAM_UTF8_STRING:
            case OSSL_PARAM_UTF8_PTR:
            case OSSL_PARAM_OCTET_STRING:
            case OSSL_PARAM_OCTET_PTR:
                ctx->p1 = (int)ctx->params[0].return_size;
                break;
            }
        }
        break;

    /*
     * PRE_CTRL_STR_TO_PARAMS and POST_CTRL_STR_TO_PARAMS handle ctrl_str to
     * params translations.  PRE_CTRL_TO_PARAMS is responsible for preparing
     * |*params|, and POST_CTRL_TO_PARAMS currently has nothing to do, since
     * there's no support for getting data via ctrl_str calls.
     */
    case PRE_CTRL_STR_TO_PARAMS:
        {
            /* This is ctrl_str to params translation */
            const char *tmp_ctrl_str = ctx->ctrl_str;
            const char *orig_ctrl_str = ctx->ctrl_str;
            const char *orig_value = ctx->p2;
            const OSSL_PARAM *settable = NULL;
            int exists = 0;

            /* Only setting is supported here */
            if (ctx->action_type != OSSL_ACTION_SET) {
                ERR_raise_data(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED,
                                   "[action:%d, state:%d] only setting allowed",
                                   ctx->action_type, state);
                return 0;
            }

            /*
             * If no translation exists, we simply pass the control string
             * unmodified.
             */
            if (translation != NULL) {
                tmp_ctrl_str = ctx->ctrl_str = translation->param_key;

                if (ctx->ishex) {
                    strcpy(ctx->name_buf, "hex");
                    if (OPENSSL_strlcat(ctx->name_buf, tmp_ctrl_str,
                                        sizeof(ctx->name_buf)) <= 3) {
                        ERR_raise(ERR_LIB_EVP, ERR_R_INTERNAL_ERROR);
                        return -1;
                    }
                    tmp_ctrl_str = ctx->name_buf;
                }
            }

            settable = EVP_PKEY_CTX_settable_params(ctx->pctx);
            if (!OSSL_PARAM_allocate_from_text(ctx->params, settable,
                                               tmp_ctrl_str,
                                               ctx->p2, strlen(ctx->p2),
                                               &exists)) {
                if (!exists) {
                    ERR_raise_data(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED,
                                   "[action:%d, state:%d] name=%s, value=%s",
                                   ctx->action_type, state,
                                   orig_ctrl_str, orig_value);
                    return -2;
                }
                return 0;
            }
            ctx->allocated_buf = ctx->params->data;
            ctx->buflen = ctx->params->data_size;
        }
        break;
    case POST_CTRL_STR_TO_PARAMS:
        /* Nothing to be done */
        break;

    /*
     * PRE_PARAMS_TO_CTRL and POST_PARAMS_TO_CTRL handle params to ctrl
     * translations.  PRE_PARAMS_TO_CTRL is responsible for preparing
     * |p1| and |p2|, and POST_PARAMS_TO_CTRL is responsible for bringing
     * the EVP_PKEY_CTX_ctrl() return value (passed as |p1|) and |p2| back
     * to |*params|.
     *
     * PKEY is treated just like POST_PARAMS_TO_CTRL, making it easy
     * for the related fixup_args functions to just set |p1| and |p2|
     * appropriately and leave it to this section of code to fix up
     * |ctx->params| accordingly.
     */
    case PKEY:
    case POST_PARAMS_TO_CTRL:
        ret = ctx->p1;
        /* FALLTHRU */
    case PRE_PARAMS_TO_CTRL:
        {
            /* This is params to ctrl translation */
            if (state == PRE_PARAMS_TO_CTRL && ctx->action_type == OSSL_ACTION_SET) {
                /* For the PRE state, only setting needs some work to be done */

                /* When setting, we populate |p1| and |p2| from |*params| */
                switch (translation->param_data_type) {
                case OSSL_PARAM_INTEGER:
                    return OSSL_PARAM_get_int(ctx->params, &ctx->p1);
                case OSSL_PARAM_UNSIGNED_INTEGER:
                    if (ctx->p2 != NULL) {
                        /* BIGNUM passed down with p2 */
                        if (!OSSL_PARAM_get_BN(ctx->params, ctx->p2))
                            return 0;
                    } else {
                        /* Normal C unsigned int passed down */
                        if (!OSSL_PARAM_get_uint(ctx->params,
                                                 (unsigned int *)&ctx->p1))
                            return 0;
                    }
                    return 1;
                case OSSL_PARAM_UTF8_STRING:
                    return OSSL_PARAM_get_utf8_string(ctx->params,
                                                      ctx->p2, ctx->sz);
                case OSSL_PARAM_OCTET_STRING:
                    return OSSL_PARAM_get_octet_string(ctx->params,
                                                       &ctx->p2, ctx->sz,
                                                       (size_t *)&ctx->p1);
                case OSSL_PARAM_OCTET_PTR:
                    return OSSL_PARAM_get_octet_ptr(ctx->params,
                                                    ctx->p2, &ctx->sz);
                default:
                    ERR_raise_data(ERR_LIB_EVP, ERR_R_UNSUPPORTED,
                                   "[action:%d, state:%d] "
                                   "unknown OSSL_PARAM data type %d",
                                   ctx->action_type, state,
                                   translation->param_data_type);
                    return 0;
                }
            } else if ((state == POST_PARAMS_TO_CTRL || state == PKEY)
                       && ctx->action_type == OSSL_ACTION_GET) {
                /* For the POST state, only getting needs some work to be done */
                unsigned int param_data_type = translation->param_data_type;
                size_t size = (size_t)ctx->p1;

                if (state == PKEY)
                    size = ctx->sz;
                if (param_data_type == 0) {
                    /* we must have a fixup_args function to work */
                    if (!ossl_assert(translation->fixup_args != NULL)) {
                        ERR_raise(ERR_LIB_EVP, ERR_R_INTERNAL_ERROR);
                        return 0;
                    }
                    param_data_type = ctx->params->data_type;
                }
                /* When getting, we populate |*params| from |p1| and |p2| */
                switch (param_data_type) {
                case OSSL_PARAM_INTEGER:
                    return OSSL_PARAM_set_int(ctx->params, ctx->p1);
                case OSSL_PARAM_UNSIGNED_INTEGER:
                    if (ctx->p2 != NULL) {
                        /* BIGNUM passed back */
                        return OSSL_PARAM_set_BN(ctx->params, ctx->p2);
                    } else {
                        /* Normal C unsigned int passed back */
                        return OSSL_PARAM_set_uint(ctx->params,
                                                   (unsigned int)ctx->p1);
                    }
                    return 0;
                case OSSL_PARAM_UTF8_STRING:
                    return OSSL_PARAM_set_utf8_string(ctx->params, ctx->p2);
                case OSSL_PARAM_OCTET_STRING:
                    return OSSL_PARAM_set_octet_string(ctx->params, ctx->p2,
                                                       size);
                case OSSL_PARAM_OCTET_PTR:
                    return OSSL_PARAM_set_octet_ptr(ctx->params, *(void **)ctx->p2,
                                                    size);
                default:
                    ERR_raise_data(ERR_LIB_EVP, ERR_R_UNSUPPORTED,
                                   "[action:%d, state:%d] "
                                   "unsupported OSSL_PARAM data type %d",
                                   ctx->action_type, state,
                                   translation->param_data_type);
                    return 0;
                }
            } else if (state == PRE_PARAMS_TO_CTRL && ctx->action_type == OSSL_ACTION_GET) {
                if (translation->param_data_type == OSSL_PARAM_OCTET_PTR)
                    ctx->p2 = &ctx->bufp;
            }
        }
        /* Any other combination is simply pass-through */
        break;
    }
    return ret;
}

static int
cleanup_translation_ctx(enum state state,
                        const struct translation_st *translation,
                        struct translation_ctx_st *ctx)
{
    if (ctx->allocated_buf != NULL)
        OPENSSL_free(ctx->allocated_buf);
    ctx->allocated_buf = NULL;
    return 1;
}

/*
 * fix_cipher_md fixes up an EVP_CIPHER / EVP_MD to its name on OSSL_ACTION_SET,
 * and cipher / md name to EVP_MD on OSSL_ACTION_GET.
 */
static const char *get_cipher_name(void *cipher)
{
    return EVP_CIPHER_get0_name(cipher);
}

static const char *get_md_name(void *md)
{
    return EVP_MD_get0_name(md);
}

static const void *get_cipher_by_name(OSSL_LIB_CTX *libctx, const char *name)
{
    return evp_get_cipherbyname_ex(libctx, name);
}

static const void *get_md_by_name(OSSL_LIB_CTX *libctx, const char *name)
{
    return evp_get_digestbyname_ex(libctx, name);
}

static int fix_cipher_md(enum state state,
                         const struct translation_st *translation,
                         struct translation_ctx_st *ctx,
                         const char *(*get_name)(void *algo),
                         const void *(*get_algo_by_name)(OSSL_LIB_CTX *libctx,
                                                         const char *name))
{
    int ret = 1;

    if ((ret = default_check(state, translation, ctx)) <= 0)
        return ret;

    if (state == PRE_CTRL_TO_PARAMS && ctx->action_type == OSSL_ACTION_GET) {
        /*
         * |ctx->p2| contains the address to an EVP_CIPHER or EVP_MD pointer
         * to be filled in.  We need to remember it, then make |ctx->p2|
         * point at a buffer to be filled in with the name, and |ctx->p1|
         * with its size.  default_fixup_args() will take care of the rest
         * for us.
         */
        ctx->orig_p2 = ctx->p2;
        ctx->p2 = ctx->name_buf;
        ctx->p1 = sizeof(ctx->name_buf);
    } else if (state == PRE_CTRL_TO_PARAMS && ctx->action_type == OSSL_ACTION_SET) {
        /*
         * In different parts of OpenSSL, this ctrl command is used
         * differently.  Some calls pass a NID as p1, others pass an
         * EVP_CIPHER pointer as p2...
         */
        ctx->p2 = (char *)(ctx->p2 == NULL
                           ? OBJ_nid2sn(ctx->p1)
                           : get_name(ctx->p2));
        ctx->p1 = strlen(ctx->p2);
    } else if (state == POST_PARAMS_TO_CTRL && ctx->action_type == OSSL_ACTION_GET) {
        ctx->p2 = (ctx->p2 == NULL ? "" : (char *)get_name(ctx->p2));
        ctx->p1 = strlen(ctx->p2);
    }

    if ((ret = default_fixup_args(state, translation, ctx)) <= 0)
        return ret;

    if (state == POST_CTRL_TO_PARAMS && ctx->action_type == OSSL_ACTION_GET) {
        /*
         * Here's how we reuse |ctx->orig_p2| that was set in the
         * PRE_CTRL_TO_PARAMS state above.
         */
        *(void **)ctx->orig_p2 =
            (void *)get_algo_by_name(ctx->pctx->libctx, ctx->p2);
        ctx->p1 = 1;
    } else if (state == PRE_PARAMS_TO_CTRL && ctx->action_type == OSSL_ACTION_SET) {
        ctx->p2 = (void *)get_algo_by_name(ctx->pctx->libctx, ctx->p2);
        ctx->p1 = 0;
    }

    return ret;
}

static int fix_cipher(enum state state,
                      const struct translation_st *translation,
                      struct translation_ctx_st *ctx)
{
    return fix_cipher_md(state, translation, ctx,
                         get_cipher_name, get_cipher_by_name);
}

static int fix_md(enum state state,
                  const struct translation_st *translation,
                  struct translation_ctx_st *ctx)
{
    return fix_cipher_md(state, translation, ctx,
                         get_md_name, get_md_by_name);
}

static int fix_distid_len(enum state state,
                          const struct translation_st *translation,
                          struct translation_ctx_st *ctx)
{
    int ret = default_fixup_args(state, translation, ctx);

    if (ret > 0) {
        ret = 0;
        if ((state == POST_CTRL_TO_PARAMS
             || state == POST_CTRL_STR_TO_PARAMS) && ctx->action_type == OSSL_ACTION_GET) {
            *(size_t *)ctx->p2 = ctx->sz;
            ret = 1;
        }
    }
    return ret;
}

struct kdf_type_map_st {
    int kdf_type_num;
    const char *kdf_type_str;
};

static int fix_kdf_type(enum state state,
                        const struct translation_st *translation,
                        struct translation_ctx_st *ctx,
                        const struct kdf_type_map_st *kdf_type_map)
{
    /*
     * The EVP_PKEY_CTRL_DH_KDF_TYPE ctrl command is a bit special, in
     * that it's used both for setting a value, and for getting it, all
     * depending on the value if |p1|; if |p1| is -2, the backend is
     * supposed to place the current kdf type in |p2|, and if not, |p1|
     * is interpreted as the new kdf type.
     */
    int ret = 0;

    if ((ret = default_check(state, translation, ctx)) <= 0)
        return ret;

    if (state == PRE_CTRL_TO_PARAMS) {
        /*
         * In |translations|, the initial value for |ctx->action_type| must
         * be OSSL_ACTION_NONE.
         */
        if (!ossl_assert(ctx->action_type == OSSL_ACTION_NONE))
            return 0;

        /* The action type depends on the value of *p1 */
        if (ctx->p1 == -2) {
            /*
             * The OSSL_PARAMS getter needs space to store a copy of the kdf
             * type string.  We use |ctx->name_buf|, which has enough space
             * allocated.
             *
             * (this wouldn't be needed if the OSSL_xxx_PARAM_KDF_TYPE
             * had the data type OSSL_PARAM_UTF8_PTR)
             */
            ctx->p2 = ctx->name_buf;
            ctx->p1 = sizeof(ctx->name_buf);
            ctx->action_type = OSSL_ACTION_GET;
        } else {
            ctx->action_type = OSSL_ACTION_SET;
        }
    }

    if ((ret = default_check(state, translation, ctx)) <= 0)
        return ret;

    if ((state == PRE_CTRL_TO_PARAMS && ctx->action_type == OSSL_ACTION_SET)
        || (state == POST_PARAMS_TO_CTRL && ctx->action_type == OSSL_ACTION_GET)) {
        ret = -2;
        /* Convert KDF type numbers to strings */
        for (; kdf_type_map->kdf_type_str != NULL; kdf_type_map++)
            if (ctx->p1 == kdf_type_map->kdf_type_num) {
                ctx->p2 = (char *)kdf_type_map->kdf_type_str;
                ret = 1;
                break;
            }
        if (ret <= 0)
            goto end;
        ctx->p1 = strlen(ctx->p2);
    }

    if ((ret = default_fixup_args(state, translation, ctx)) <= 0)
        return ret;

    if ((state == POST_CTRL_TO_PARAMS && ctx->action_type == OSSL_ACTION_GET)
        || (state == PRE_PARAMS_TO_CTRL && ctx->action_type == OSSL_ACTION_SET)) {
        ctx->p1 = ret = -1;

        /* Convert KDF type strings to numbers */
        for (; kdf_type_map->kdf_type_str != NULL; kdf_type_map++)
            if (OPENSSL_strcasecmp(ctx->p2, kdf_type_map->kdf_type_str) == 0) {
                ctx->p1 = kdf_type_map->kdf_type_num;
                ret = 1;
                break;
            }
        ctx->p2 = NULL;
    } else if (state == PRE_PARAMS_TO_CTRL && ctx->action_type == OSSL_ACTION_GET) {
        ctx->p1 = -2;
    }
 end:
    return ret;
}

/* EVP_PKEY_CTRL_DH_KDF_TYPE */
static int fix_dh_kdf_type(enum state state,
                           const struct translation_st *translation,
                           struct translation_ctx_st *ctx)
{
    static const struct kdf_type_map_st kdf_type_map[] = {
        { EVP_PKEY_DH_KDF_NONE, "" },
        { EVP_PKEY_DH_KDF_X9_42, OSSL_KDF_NAME_X942KDF_ASN1 },
        { 0, NULL }
    };

    return fix_kdf_type(state, translation, ctx, kdf_type_map);
}

/* EVP_PKEY_CTRL_EC_KDF_TYPE */
static int fix_ec_kdf_type(enum state state,
                           const struct translation_st *translation,
                           struct translation_ctx_st *ctx)
{
    static const struct kdf_type_map_st kdf_type_map[] = {
        { EVP_PKEY_ECDH_KDF_NONE, "" },
        { EVP_PKEY_ECDH_KDF_X9_63, OSSL_KDF_NAME_X963KDF },
        { 0, NULL }
    };

    return fix_kdf_type(state, translation, ctx, kdf_type_map);
}

/* EVP_PKEY_CTRL_DH_KDF_OID, EVP_PKEY_CTRL_GET_DH_KDF_OID, ...??? */
static int fix_oid(enum state state,
                   const struct translation_st *translation,
                   struct translation_ctx_st *ctx)
{
    int ret;

    if ((ret = default_check(state, translation, ctx)) <= 0)
        return ret;

    if ((state == PRE_CTRL_TO_PARAMS && ctx->action_type == OSSL_ACTION_SET)
        || (state == POST_PARAMS_TO_CTRL && ctx->action_type == OSSL_ACTION_GET)) {
        /*
         * We're translating from ctrl to params and setting the OID, or
         * we're translating from params to ctrl and getting the OID.
         * Either way, |ctx->p2| points at an ASN1_OBJECT, and needs to have
         * that replaced with the corresponding name.
         * default_fixup_args() will then be able to convert that to the
         * corresponding OSSL_PARAM.
         */
        OBJ_obj2txt(ctx->name_buf, sizeof(ctx->name_buf), ctx->p2, 0);
        ctx->p2 = (char *)ctx->name_buf;
        ctx->p1 = 0; /* let default_fixup_args() figure out the length */
    }

    if ((ret = default_fixup_args(state, translation, ctx)) <= 0)
        return ret;

    if ((state == PRE_PARAMS_TO_CTRL && ctx->action_type == OSSL_ACTION_SET)
        || (state == POST_CTRL_TO_PARAMS && ctx->action_type == OSSL_ACTION_GET)) {
        /*
         * We're translating from ctrl to params and setting the OID name,
         * or we're translating from params to ctrl and getting the OID
         * name.  Either way, default_fixup_args() has placed the OID name
         * in |ctx->p2|, all we need to do now is to replace that with the
         * corresponding ASN1_OBJECT.
         */
        ctx->p2 = (ASN1_OBJECT *)OBJ_txt2obj(ctx->p2, 0);
    }

    return ret;
}

/* EVP_PKEY_CTRL_DH_NID */
static int fix_dh_nid(enum state state,
                      const struct translation_st *translation,
                      struct translation_ctx_st *ctx)
{
    int ret;

    if ((ret = default_check(state, translation, ctx)) <= 0)
        return ret;

    /* This is only settable */
    if (ctx->action_type != OSSL_ACTION_SET)
        return 0;

    if (state == PRE_CTRL_TO_PARAMS) {
        if ((ctx->p2 = (char *)ossl_ffc_named_group_get_name
             (ossl_ffc_uid_to_dh_named_group(ctx->p1))) == NULL) {
            ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_VALUE);
            return 0;
        }
        ctx->p1 = 0;
    }

    return default_fixup_args(state, translation, ctx);
}

/* EVP_PKEY_CTRL_DH_RFC5114 */
static int fix_dh_nid5114(enum state state,
                          const struct translation_st *translation,
                          struct translation_ctx_st *ctx)
{
    int ret;

    if ((ret = default_check(state, translation, ctx)) <= 0)
        return ret;

    /* This is only settable */
    if (ctx->action_type != OSSL_ACTION_SET)
        return 0;

    switch (state) {
    case PRE_CTRL_TO_PARAMS:
        if ((ctx->p2 = (char *)ossl_ffc_named_group_get_name
             (ossl_ffc_uid_to_dh_named_group(ctx->p1))) == NULL) {
            ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_VALUE);
            return 0;
        }

        ctx->p1 = 0;
        break;

    case PRE_CTRL_STR_TO_PARAMS:
        if (ctx->p2 == NULL)
            return 0;
        if ((ctx->p2 = (char *)ossl_ffc_named_group_get_name
             (ossl_ffc_uid_to_dh_named_group(atoi(ctx->p2)))) == NULL) {
            ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_VALUE);
            return 0;
        }

        ctx->p1 = 0;
        break;

    default:
        break;
    }

    return default_fixup_args(state, translation, ctx);
}

/* EVP_PKEY_CTRL_DH_PARAMGEN_TYPE */
static int fix_dh_paramgen_type(enum state state,
                                const struct translation_st *translation,
                                struct translation_ctx_st *ctx)
{
    int ret;

    if ((ret = default_check(state, translation, ctx)) <= 0)
        return ret;

    /* This is only settable */
    if (ctx->action_type != OSSL_ACTION_SET)
        return 0;

    if (state == PRE_CTRL_STR_TO_PARAMS) {
        if ((ctx->p2 = (char *)ossl_dh_gen_type_id2name(atoi(ctx->p2)))
             == NULL) {
            ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_VALUE);
            return 0;
        }
        ctx->p1 = strlen(ctx->p2);
    }

    return default_fixup_args(state, translation, ctx);
}

/* EVP_PKEY_CTRL_EC_PARAM_ENC */
static int fix_ec_param_enc(enum state state,
                            const struct translation_st *translation,
                            struct translation_ctx_st *ctx)
{
    int ret;

    if ((ret = default_check(state, translation, ctx)) <= 0)
        return ret;

    /* This is currently only settable */
    if (ctx->action_type != OSSL_ACTION_SET)
        return 0;

    if (state == PRE_CTRL_TO_PARAMS) {
        switch (ctx->p1) {
        case OPENSSL_EC_EXPLICIT_CURVE:
            ctx->p2 = OSSL_PKEY_EC_ENCODING_EXPLICIT;
            break;
        case OPENSSL_EC_NAMED_CURVE:
            ctx->p2 = OSSL_PKEY_EC_ENCODING_GROUP;
            break;
        default:
            ret = -2;
            goto end;
        }
        ctx->p1 = 0;
    }

    if ((ret = default_fixup_args(state, translation, ctx)) <= 0)
        return ret;

    if (state == PRE_PARAMS_TO_CTRL) {
        if (strcmp(ctx->p2, OSSL_PKEY_EC_ENCODING_EXPLICIT) == 0)
            ctx->p1 = OPENSSL_EC_EXPLICIT_CURVE;
        else if (strcmp(ctx->p2, OSSL_PKEY_EC_ENCODING_GROUP) == 0)
            ctx->p1 = OPENSSL_EC_NAMED_CURVE;
        else
            ctx->p1 = ret = -2;
        ctx->p2 = NULL;
    }

 end:
    if (ret == -2)
        ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
    return ret;
}

/* EVP_PKEY_CTRL_EC_PARAMGEN_CURVE_NID */
static int fix_ec_paramgen_curve_nid(enum state state,
                                     const struct translation_st *translation,
                                     struct translation_ctx_st *ctx)
{
    char *p2 = NULL;
    int ret;

    if ((ret = default_check(state, translation, ctx)) <= 0)
        return ret;

    /* This is currently only settable */
    if (ctx->action_type != OSSL_ACTION_SET)
        return 0;

    if (state == PRE_CTRL_TO_PARAMS) {
        ctx->p2 = (char *)OBJ_nid2sn(ctx->p1);
        ctx->p1 = 0;
    } else if (state == PRE_PARAMS_TO_CTRL) {
        /*
         * We're translating from params to ctrl and setting the curve name.
         * The ctrl function needs it to be a NID, but meanwhile, we need
         * space to get the curve name from the param.  |ctx->name_buf| is
         * sufficient for that.
         * The double indirection is necessary for default_fixup_args()'s
         * call of OSSL_PARAM_get_utf8_string() to be done correctly.
         */
        p2 = ctx->name_buf;
        ctx->p2 = &p2;
        ctx->sz = sizeof(ctx->name_buf);
    }

    if ((ret = default_fixup_args(state, translation, ctx)) <= 0)
        return ret;

    if (state == PRE_PARAMS_TO_CTRL) {
        ctx->p1 = OBJ_sn2nid(p2);
        ctx->p2 = NULL;
    }

    return ret;
}

/* EVP_PKEY_CTRL_EC_ECDH_COFACTOR */
static int fix_ecdh_cofactor(enum state state,
                             const struct translation_st *translation,
                             struct translation_ctx_st *ctx)
{
    /*
     * The EVP_PKEY_CTRL_EC_ECDH_COFACTOR ctrl command is a bit special, in
     * that it's used both for setting a value, and for getting it, all
     * depending on the value if |ctx->p1|; if |ctx->p1| is -2, the backend is
     * supposed to place the current cofactor mode in |ctx->p2|, and if not,
     * |ctx->p1| is interpreted as the new cofactor mode.
     */
    int ret = 0;

    if (state == PRE_CTRL_TO_PARAMS) {
        /*
         * The initial value for |ctx->action_type| must be zero.
         * evp_pkey_ctrl_to_params() takes it from the translation item.
         */
        if (!ossl_assert(ctx->action_type == OSSL_ACTION_NONE))
            return 0;

        /* The action type depends on the value of ctx->p1 */
        if (ctx->p1 == -2)
            ctx->action_type = OSSL_ACTION_GET;
        else
            ctx->action_type = OSSL_ACTION_SET;
    } else if (state == PRE_CTRL_STR_TO_PARAMS) {
        ctx->action_type = OSSL_ACTION_SET;
    } else if (state == PRE_PARAMS_TO_CTRL) {
        /* The initial value for |ctx->action_type| must not be zero. */
        if (!ossl_assert(ctx->action_type != OSSL_ACTION_NONE))
            return 0;
    } else if (state == POST_PARAMS_TO_CTRL && ctx->action_type == OSSL_ACTION_NONE) {
        ctx->action_type = OSSL_ACTION_GET;
    }

    if ((ret = default_check(state, translation, ctx)) <= 0)
        return ret;

    if (state == PRE_CTRL_TO_PARAMS && ctx->action_type == OSSL_ACTION_SET) {
        if (ctx->p1 < -1 || ctx->p1 > 1) {
            /* Uses the same return value of pkey_ec_ctrl() */
            return -2;
        }
    }

    if ((ret = default_fixup_args(state, translation, ctx)) <= 0)
        return ret;

    if (state == POST_CTRL_TO_PARAMS && ctx->action_type == OSSL_ACTION_GET) {
        if (ctx->p1 < 0 || ctx->p1 > 1) {
            /*
             * The provider should return either 0 or 1, any other value is a
             * provider error.
             */
            ctx->p1 = ret = -1;
        }
    } else if (state == PRE_PARAMS_TO_CTRL && ctx->action_type == OSSL_ACTION_GET) {
        ctx->p1 = -2;
    } else if (state == POST_PARAMS_TO_CTRL && ctx->action_type == OSSL_ACTION_GET) {
        ctx->p1 = ret;
    }

    return ret;
}

/* EVP_PKEY_CTRL_RSA_PADDING, EVP_PKEY_CTRL_GET_RSA_PADDING */
static int fix_rsa_padding_mode(enum state state,
                                const struct translation_st *translation,
                                struct translation_ctx_st *ctx)
{
    static const OSSL_ITEM str_value_map[] = {
        { RSA_PKCS1_PADDING,            "pkcs1"  },
        { RSA_NO_PADDING,               "none"   },
        { RSA_PKCS1_OAEP_PADDING,       "oaep"   },
        { RSA_PKCS1_OAEP_PADDING,       "oeap"   },
        { RSA_X931_PADDING,             "x931"   },
        { RSA_PKCS1_PSS_PADDING,        "pss"    },
        /* Special case, will pass directly as an integer */
        { RSA_PKCS1_WITH_TLS_PADDING,   NULL     }
    };
    int ret;

    if ((ret = default_check(state, translation, ctx)) <= 0)
        return ret;

    if (state == PRE_CTRL_TO_PARAMS && ctx->action_type == OSSL_ACTION_GET) {
        /*
         * EVP_PKEY_CTRL_GET_RSA_PADDING returns the padding mode in the
         * weirdest way for a ctrl.  Instead of doing like all other ctrls
         * that return a simple, i.e. just have that as a return value,
         * this particular ctrl treats p2 as the address for the int to be
         * returned.  We must therefore remember |ctx->p2|, then make
         * |ctx->p2| point at a buffer to be filled in with the name, and
         * |ctx->p1| with its size.  default_fixup_args() will take care
         * of the rest for us, along with the POST_CTRL_TO_PARAMS && OSSL_ACTION_GET
         * code section further down.
         */
        ctx->orig_p2 = ctx->p2;
        ctx->p2 = ctx->name_buf;
        ctx->p1 = sizeof(ctx->name_buf);
    } else if (state == PRE_CTRL_TO_PARAMS && ctx->action_type == OSSL_ACTION_SET) {
        /*
         * Ideally, we should use utf8 strings for the diverse padding modes.
         * We only came here because someone called EVP_PKEY_CTX_ctrl(),
         * though, and since that can reasonably be seen as legacy code
         * that uses the diverse RSA macros for the padding mode, and we
         * know that at least our providers can handle the numeric modes,
         * we take the cheap route for now.
         *
         * The other solution would be to match |ctx->p1| against entries
         * in str_value_map and pass the corresponding string.  However,
         * since we don't have a string for RSA_PKCS1_WITH_TLS_PADDING,
         * we have to do this same hack at least for that one.
         *
         * Since the "official" data type for the RSA padding mode is utf8
         * string, we cannot count on default_fixup_args().  Instead, we
         * build the OSSL_PARAM item ourselves and return immediately.
         */
        ctx->params[0] = OSSL_PARAM_construct_int(translation->param_key,
                                                  &ctx->p1);
        return 1;
    } else if (state == POST_PARAMS_TO_CTRL && ctx->action_type == OSSL_ACTION_GET) {
        size_t i;

        /*
         * The EVP_PKEY_CTX_get_params() caller may have asked for a utf8
         * string, or may have asked for an integer of some sort.  If they
         * ask for an integer, we respond directly.  If not, we translate
         * the response from the ctrl function into a string.
         */
        switch (ctx->params->data_type) {
        case OSSL_PARAM_INTEGER:
            return OSSL_PARAM_get_int(ctx->params, &ctx->p1);
        case OSSL_PARAM_UNSIGNED_INTEGER:
            return OSSL_PARAM_get_uint(ctx->params, (unsigned int *)&ctx->p1);
        default:
            break;
        }

        for (i = 0; i < OSSL_NELEM(str_value_map); i++) {
            if (ctx->p1 == (int)str_value_map[i].id)
                break;
        }
        if (i == OSSL_NELEM(str_value_map)) {
            ERR_raise_data(ERR_LIB_RSA, RSA_R_UNKNOWN_PADDING_TYPE,
                           "[action:%d, state:%d] padding number %d",
                           ctx->action_type, state, ctx->p1);
            return -2;
        }
        /*
         * If we don't have a string, we can't do anything.  The caller
         * should have asked for a number...
         */
        if (str_value_map[i].ptr == NULL) {
            ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
            return -2;
        }
        ctx->p2 = str_value_map[i].ptr;
        ctx->p1 = strlen(ctx->p2);
    }

    if ((ret = default_fixup_args(state, translation, ctx)) <= 0)
        return ret;

    if ((ctx->action_type == OSSL_ACTION_SET && state == PRE_PARAMS_TO_CTRL)
        || (ctx->action_type == OSSL_ACTION_GET && state == POST_CTRL_TO_PARAMS)) {
        size_t i;

        for (i = 0; i < OSSL_NELEM(str_value_map); i++) {
            if (strcmp(ctx->p2, str_value_map[i].ptr) == 0)
                break;
        }

        if (i == OSSL_NELEM(str_value_map)) {
            ERR_raise_data(ERR_LIB_RSA, RSA_R_UNKNOWN_PADDING_TYPE,
                           "[action:%d, state:%d] padding name %s",
                           ctx->action_type, state, ctx->p1);
            ctx->p1 = ret = -2;
        } else if (state == POST_CTRL_TO_PARAMS) {
            /* EVP_PKEY_CTRL_GET_RSA_PADDING weirdness explained further up */
            *(int *)ctx->orig_p2 = str_value_map[i].id;
        } else {
            ctx->p1 = str_value_map[i].id;
        }
        ctx->p2 = NULL;
    }

    return ret;
}

/* EVP_PKEY_CTRL_RSA_PSS_SALTLEN, EVP_PKEY_CTRL_GET_RSA_PSS_SALTLEN */
static int fix_rsa_pss_saltlen(enum state state,
                               const struct translation_st *translation,
                               struct translation_ctx_st *ctx)
{
    static const OSSL_ITEM str_value_map[] = {
        { (unsigned int)RSA_PSS_SALTLEN_DIGEST, "digest" },
        { (unsigned int)RSA_PSS_SALTLEN_MAX,    "max"    },
        { (unsigned int)RSA_PSS_SALTLEN_AUTO,   "auto"   }
    };
    int ret;

    if ((ret = default_check(state, translation, ctx)) <= 0)
        return ret;

    if (state == PRE_CTRL_TO_PARAMS && ctx->action_type == OSSL_ACTION_GET) {
        /*
         * EVP_PKEY_CTRL_GET_RSA_PSS_SALTLEN returns the saltlen by filling
         * in the int pointed at by p2.  This is potentially as weird as
         * the way EVP_PKEY_CTRL_GET_RSA_PADDING works, except that saltlen
         * might be a negative value, so it wouldn't work as a legitimate
         * return value.
         * In any case, we must therefore remember |ctx->p2|, then make
         * |ctx->p2| point at a buffer to be filled in with the name, and
         * |ctx->p1| with its size.  default_fixup_args() will take care
         * of the rest for us, along with the POST_CTRL_TO_PARAMS && OSSL_ACTION_GET
         * code section further down.
         */
        ctx->orig_p2 = ctx->p2;
        ctx->p2 = ctx->name_buf;
        ctx->p1 = sizeof(ctx->name_buf);
    } else if ((ctx->action_type == OSSL_ACTION_SET && state == PRE_CTRL_TO_PARAMS)
               || (ctx->action_type == OSSL_ACTION_GET && state == POST_PARAMS_TO_CTRL)) {
        size_t i;

        for (i = 0; i < OSSL_NELEM(str_value_map); i++) {
            if (ctx->p1 == (int)str_value_map[i].id)
                break;
        }
        if (i == OSSL_NELEM(str_value_map)) {
            BIO_snprintf(ctx->name_buf, sizeof(ctx->name_buf), "%d", ctx->p1);
        } else {
            /* This won't truncate but it will quiet static analysers */
            strncpy(ctx->name_buf, str_value_map[i].ptr, sizeof(ctx->name_buf) - 1);
            ctx->name_buf[sizeof(ctx->name_buf) - 1] = '\0';
        }
        ctx->p2 = ctx->name_buf;
        ctx->p1 = strlen(ctx->p2);
    }

    if ((ret = default_fixup_args(state, translation, ctx)) <= 0)
        return ret;

    if ((ctx->action_type == OSSL_ACTION_SET && state == PRE_PARAMS_TO_CTRL)
        || (ctx->action_type == OSSL_ACTION_GET && state == POST_CTRL_TO_PARAMS)) {
        size_t i;
        int val;

        for (i = 0; i < OSSL_NELEM(str_value_map); i++) {
            if (strcmp(ctx->p2, str_value_map[i].ptr) == 0)
                break;
        }

        val = i == OSSL_NELEM(str_value_map) ? atoi(ctx->p2)
                                             : (int)str_value_map[i].id;
        if (state == POST_CTRL_TO_PARAMS) {
            /*
             * EVP_PKEY_CTRL_GET_RSA_PSS_SALTLEN weirdness explained further
             * up
             */
            *(int *)ctx->orig_p2 = val;
        } else {
            ctx->p1 = val;
        }
        ctx->p2 = NULL;
    }

    return ret;
}

/* EVP_PKEY_CTRL_HKDF_MODE */
static int fix_hkdf_mode(enum state state,
                         const struct translation_st *translation,
                         struct translation_ctx_st *ctx)
{
    static const OSSL_ITEM str_value_map[] = {
        { EVP_KDF_HKDF_MODE_EXTRACT_AND_EXPAND, "EXTRACT_AND_EXPAND" },
        { EVP_KDF_HKDF_MODE_EXTRACT_ONLY,       "EXTRACT_ONLY"       },
        { EVP_KDF_HKDF_MODE_EXPAND_ONLY,        "EXPAND_ONLY"        }
    };
    int ret;

    if ((ret = default_check(state, translation, ctx)) <= 0)
        return ret;

    if ((ctx->action_type == OSSL_ACTION_SET && state == PRE_CTRL_TO_PARAMS)
        || (ctx->action_type == OSSL_ACTION_GET && state == POST_PARAMS_TO_CTRL)) {
        size_t i;

        for (i = 0; i < OSSL_NELEM(str_value_map); i++) {
            if (ctx->p1 == (int)str_value_map[i].id)
                break;
        }
        if (i == OSSL_NELEM(str_value_map))
            return 0;
        ctx->p2 = str_value_map[i].ptr;
        ctx->p1 = strlen(ctx->p2);
    }

    if ((ret = default_fixup_args(state, translation, ctx)) <= 0)
        return ret;

    if ((ctx->action_type == OSSL_ACTION_SET && state == PRE_PARAMS_TO_CTRL)
        || (ctx->action_type == OSSL_ACTION_GET && state == POST_CTRL_TO_PARAMS)) {
        size_t i;

        for (i = 0; i < OSSL_NELEM(str_value_map); i++) {
            if (strcmp(ctx->p2, str_value_map[i].ptr) == 0)
                break;
        }
        if (i == OSSL_NELEM(str_value_map))
            return 0;
        if (state == POST_CTRL_TO_PARAMS)
            ret = str_value_map[i].id;
        else
            ctx->p1 = str_value_map[i].id;
        ctx->p2 = NULL;
    }

    return 1;
}

/*-
 * Payload getters
 * ===============
 *
 * These all get the data they want, then call default_fixup_args() as
 * a post-ctrl OSSL_ACTION_GET fixup.  They all get NULL ctx, ctrl_cmd, ctrl_str,
 * p1, sz
 */

/* Pilfering DH, DSA and EC_KEY */
static int get_payload_group_name(enum state state,
                                  const struct translation_st *translation,
                                  struct translation_ctx_st *ctx)
{
    EVP_PKEY *pkey = ctx->p2;

    ctx->p2 = NULL;
    switch (EVP_PKEY_get_base_id(pkey)) {
#ifndef OPENSSL_NO_DH
    case EVP_PKEY_DH:
        {
            const DH *dh = EVP_PKEY_get0_DH(pkey);
            int uid = DH_get_nid(dh);

            if (uid != NID_undef) {
                const DH_NAMED_GROUP *dh_group =
                    ossl_ffc_uid_to_dh_named_group(uid);

                ctx->p2 = (char *)ossl_ffc_named_group_get_name(dh_group);
            }
        }
        break;
#endif
#ifndef OPENSSL_NO_EC
    case EVP_PKEY_EC:
        {
            const EC_GROUP *grp =
                EC_KEY_get0_group(EVP_PKEY_get0_EC_KEY(pkey));
            int nid = NID_undef;

            if (grp != NULL)
                nid = EC_GROUP_get_curve_name(grp);
            if (nid != NID_undef)
                ctx->p2 = (char *)OSSL_EC_curve_nid2name(nid);
        }
        break;
#endif
    default:
        ERR_raise(ERR_LIB_EVP, EVP_R_UNSUPPORTED_KEY_TYPE);
        return 0;
    }

    /*
     * Quietly ignoring unknown groups matches the behaviour on the provider
     * side.
     */
    if (ctx->p2 == NULL)
        return 1;

    ctx->p1 = strlen(ctx->p2);
    return default_fixup_args(state, translation, ctx);
}

static int get_payload_private_key(enum state state,
                                   const struct translation_st *translation,
                                   struct translation_ctx_st *ctx)
{
    EVP_PKEY *pkey = ctx->p2;

    ctx->p2 = NULL;
    if (ctx->params->data_type != OSSL_PARAM_UNSIGNED_INTEGER)
        return 0;

    switch (EVP_PKEY_get_base_id(pkey)) {
#ifndef OPENSSL_NO_DH
    case EVP_PKEY_DH:
        {
            const DH *dh = EVP_PKEY_get0_DH(pkey);

            ctx->p2 = (BIGNUM *)DH_get0_priv_key(dh);
        }
        break;
#endif
#ifndef OPENSSL_NO_EC
    case EVP_PKEY_EC:
        {
            const EC_KEY *ec = EVP_PKEY_get0_EC_KEY(pkey);

            ctx->p2 = (BIGNUM *)EC_KEY_get0_private_key(ec);
        }
        break;
#endif
    default:
        ERR_raise(ERR_LIB_EVP, EVP_R_UNSUPPORTED_KEY_TYPE);
        return 0;
    }

    return default_fixup_args(state, translation, ctx);
}

static int get_payload_public_key(enum state state,
                                  const struct translation_st *translation,
                                  struct translation_ctx_st *ctx)
{
    EVP_PKEY *pkey = ctx->p2;
    unsigned char *buf = NULL;
    int ret;

    ctx->p2 = NULL;
    switch (EVP_PKEY_get_base_id(pkey)) {
#ifndef OPENSSL_NO_DH
    case EVP_PKEY_DHX:
    case EVP_PKEY_DH:
        switch (ctx->params->data_type) {
        case OSSL_PARAM_OCTET_STRING:
            ctx->sz = ossl_dh_key2buf(EVP_PKEY_get0_DH(pkey), &buf, 0, 1);
            ctx->p2 = buf;
            break;
        case OSSL_PARAM_UNSIGNED_INTEGER:
            ctx->p2 = (void *)DH_get0_pub_key(EVP_PKEY_get0_DH(pkey));
            break;
        default:
            return 0;
        }
        break;
#endif
#ifndef OPENSSL_NO_DSA
    case EVP_PKEY_DSA:
        if (ctx->params->data_type == OSSL_PARAM_UNSIGNED_INTEGER) {
            ctx->p2 = (void *)DSA_get0_pub_key(EVP_PKEY_get0_DSA(pkey));
            break;
        }
        return 0;
#endif
#ifndef OPENSSL_NO_EC
    case EVP_PKEY_EC:
        if (ctx->params->data_type == OSSL_PARAM_OCTET_STRING) {
            const EC_KEY *eckey = EVP_PKEY_get0_EC_KEY(pkey);
            BN_CTX *bnctx = BN_CTX_new_ex(ossl_ec_key_get_libctx(eckey));
            const EC_GROUP *ecg = EC_KEY_get0_group(eckey);
            const EC_POINT *point = EC_KEY_get0_public_key(eckey);

            if (bnctx == NULL)
                return 0;
            ctx->sz = EC_POINT_point2buf(ecg, point,
                                         POINT_CONVERSION_COMPRESSED,
                                         &buf, bnctx);
            ctx->p2 = buf;
            BN_CTX_free(bnctx);
            break;
        }
        return 0;
#endif
    default:
        ERR_raise(ERR_LIB_EVP, EVP_R_UNSUPPORTED_KEY_TYPE);
        return 0;
    }

    ret = default_fixup_args(state, translation, ctx);
    OPENSSL_free(buf);
    return ret;
}

static int get_payload_public_key_ec(enum state state,
                                     const struct translation_st *translation,
                                     struct translation_ctx_st *ctx)
{
#ifndef OPENSSL_NO_EC
    EVP_PKEY *pkey = ctx->p2;
    const EC_KEY *eckey = EVP_PKEY_get0_EC_KEY(pkey);
    BN_CTX *bnctx;
    const EC_POINT *point;
    const EC_GROUP *ecg;
    BIGNUM *x = NULL;
    BIGNUM *y = NULL;
    int ret = 0;

    ctx->p2 = NULL;

    if (eckey == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_UNSUPPORTED_KEY_TYPE);
        return 0;
    }

    bnctx = BN_CTX_new_ex(ossl_ec_key_get_libctx(eckey));
    if (bnctx == NULL)
        return 0;

    point = EC_KEY_get0_public_key(eckey);
    ecg = EC_KEY_get0_group(eckey);

    /* Caller should have requested a BN, fail if not */
    if (ctx->params->data_type != OSSL_PARAM_UNSIGNED_INTEGER)
        goto out;

    x = BN_CTX_get(bnctx);
    y = BN_CTX_get(bnctx);
    if (y == NULL)
        goto out;

    if (!EC_POINT_get_affine_coordinates(ecg, point, x, y, bnctx))
        goto out;

    if (strncmp(ctx->params->key, OSSL_PKEY_PARAM_EC_PUB_X, 2) == 0)
        ctx->p2 = x;
    else if (strncmp(ctx->params->key, OSSL_PKEY_PARAM_EC_PUB_Y, 2) == 0)
        ctx->p2 = y;
    else
        goto out;

    /* Return the payload */
    ret = default_fixup_args(state, translation, ctx);
out:
    BN_CTX_free(bnctx);
    return ret;
#else
    ERR_raise(ERR_LIB_EVP, EVP_R_UNSUPPORTED_KEY_TYPE);
    return 0;
#endif
}

static int get_payload_bn(enum state state,
                          const struct translation_st *translation,
                          struct translation_ctx_st *ctx, const BIGNUM *bn)
{
    if (bn == NULL)
        return 0;
    if (ctx->params->data_type != OSSL_PARAM_UNSIGNED_INTEGER)
        return 0;
    ctx->p2 = (BIGNUM *)bn;

    return default_fixup_args(state, translation, ctx);
}

static int get_dh_dsa_payload_p(enum state state,
                                const struct translation_st *translation,
                                struct translation_ctx_st *ctx)
{
    const BIGNUM *bn = NULL;
    EVP_PKEY *pkey = ctx->p2;

    switch (EVP_PKEY_get_base_id(pkey)) {
#ifndef OPENSSL_NO_DH
    case EVP_PKEY_DH:
        bn = DH_get0_p(EVP_PKEY_get0_DH(pkey));
        break;
#endif
#ifndef OPENSSL_NO_DSA
    case EVP_PKEY_DSA:
        bn = DSA_get0_p(EVP_PKEY_get0_DSA(pkey));
        break;
#endif
    default:
        ERR_raise(ERR_LIB_EVP, EVP_R_UNSUPPORTED_KEY_TYPE);
    }

    return get_payload_bn(state, translation, ctx, bn);
}

static int get_dh_dsa_payload_q(enum state state,
                                const struct translation_st *translation,
                                struct translation_ctx_st *ctx)
{
    const BIGNUM *bn = NULL;

    switch (EVP_PKEY_get_base_id(ctx->p2)) {
#ifndef OPENSSL_NO_DH
    case EVP_PKEY_DH:
        bn = DH_get0_q(EVP_PKEY_get0_DH(ctx->p2));
        break;
#endif
#ifndef OPENSSL_NO_DSA
    case EVP_PKEY_DSA:
        bn = DSA_get0_q(EVP_PKEY_get0_DSA(ctx->p2));
        break;
#endif
    }

    return get_payload_bn(state, translation, ctx, bn);
}

static int get_dh_dsa_payload_g(enum state state,
                                const struct translation_st *translation,
                                struct translation_ctx_st *ctx)
{
    const BIGNUM *bn = NULL;

    switch (EVP_PKEY_get_base_id(ctx->p2)) {
#ifndef OPENSSL_NO_DH
    case EVP_PKEY_DH:
        bn = DH_get0_g(EVP_PKEY_get0_DH(ctx->p2));
        break;
#endif
#ifndef OPENSSL_NO_DSA
    case EVP_PKEY_DSA:
        bn = DSA_get0_g(EVP_PKEY_get0_DSA(ctx->p2));
        break;
#endif
    }

    return get_payload_bn(state, translation, ctx, bn);
}

static int get_payload_int(enum state state,
                           const struct translation_st *translation,
                           struct translation_ctx_st *ctx,
                           const int val)
{
    if (ctx->params->data_type != OSSL_PARAM_INTEGER)
        return 0;
    ctx->p1 = val;
    ctx->p2 = NULL;

    return default_fixup_args(state, translation, ctx);
}

static int get_ec_decoded_from_explicit_params(enum state state,
                                               const struct translation_st *translation,
                                               struct translation_ctx_st *ctx)
{
    int val = 0;
    EVP_PKEY *pkey = ctx->p2;

    switch (EVP_PKEY_base_id(pkey)) {
#ifndef OPENSSL_NO_EC
    case EVP_PKEY_EC:
        val = EC_KEY_decoded_from_explicit_params(EVP_PKEY_get0_EC_KEY(pkey));
        if (val < 0) {
            ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_KEY);
            return 0;
        }
        break;
#endif
    default:
        ERR_raise(ERR_LIB_EVP, EVP_R_UNSUPPORTED_KEY_TYPE);
        return 0;
    }

    return get_payload_int(state, translation, ctx, val);
}

static int get_rsa_payload_n(enum state state,
                             const struct translation_st *translation,
                             struct translation_ctx_st *ctx)
{
    const BIGNUM *bn = NULL;

    if (EVP_PKEY_get_base_id(ctx->p2) != EVP_PKEY_RSA
        && EVP_PKEY_get_base_id(ctx->p2) != EVP_PKEY_RSA_PSS)
        return 0;
    bn = RSA_get0_n(EVP_PKEY_get0_RSA(ctx->p2));

    return get_payload_bn(state, translation, ctx, bn);
}

static int get_rsa_payload_e(enum state state,
                             const struct translation_st *translation,
                             struct translation_ctx_st *ctx)
{
    const BIGNUM *bn = NULL;

    if (EVP_PKEY_get_base_id(ctx->p2) != EVP_PKEY_RSA
        && EVP_PKEY_get_base_id(ctx->p2) != EVP_PKEY_RSA_PSS)
        return 0;
    bn = RSA_get0_e(EVP_PKEY_get0_RSA(ctx->p2));

    return get_payload_bn(state, translation, ctx, bn);
}

static int get_rsa_payload_d(enum state state,
                             const struct translation_st *translation,
                             struct translation_ctx_st *ctx)
{
    const BIGNUM *bn = NULL;

    if (EVP_PKEY_get_base_id(ctx->p2) != EVP_PKEY_RSA
        && EVP_PKEY_get_base_id(ctx->p2) != EVP_PKEY_RSA_PSS)
        return 0;
    bn = RSA_get0_d(EVP_PKEY_get0_RSA(ctx->p2));

    return get_payload_bn(state, translation, ctx, bn);
}

static int get_rsa_payload_factor(enum state state,
                                  const struct translation_st *translation,
                                  struct translation_ctx_st *ctx,
                                  size_t factornum)
{
    const RSA *r = EVP_PKEY_get0_RSA(ctx->p2);
    const BIGNUM *bn = NULL;

    switch (factornum) {
    case 0:
        bn = RSA_get0_p(r);
        break;
    case 1:
        bn = RSA_get0_q(r);
        break;
    default:
        {
            size_t pnum = RSA_get_multi_prime_extra_count(r);
            const BIGNUM *factors[10];

            if (factornum - 2 < pnum
                && RSA_get0_multi_prime_factors(r, factors))
                bn = factors[factornum - 2];
        }
        break;
    }

    return get_payload_bn(state, translation, ctx, bn);
}

static int get_rsa_payload_exponent(enum state state,
                                    const struct translation_st *translation,
                                    struct translation_ctx_st *ctx,
                                    size_t exponentnum)
{
    const RSA *r = EVP_PKEY_get0_RSA(ctx->p2);
    const BIGNUM *bn = NULL;

    switch (exponentnum) {
    case 0:
        bn = RSA_get0_dmp1(r);
        break;
    case 1:
        bn = RSA_get0_dmq1(r);
        break;
    default:
        {
            size_t pnum = RSA_get_multi_prime_extra_count(r);
            const BIGNUM *exps[10], *coeffs[10];

            if (exponentnum - 2 < pnum
                && RSA_get0_multi_prime_crt_params(r, exps, coeffs))
                bn = exps[exponentnum - 2];
        }
        break;
    }

    return get_payload_bn(state, translation, ctx, bn);
}

static int get_rsa_payload_coefficient(enum state state,
                                       const struct translation_st *translation,
                                       struct translation_ctx_st *ctx,
                                       size_t coefficientnum)
{
    const RSA *r = EVP_PKEY_get0_RSA(ctx->p2);
    const BIGNUM *bn = NULL;

    switch (coefficientnum) {
    case 0:
        bn = RSA_get0_iqmp(r);
        break;
    default:
        {
            size_t pnum = RSA_get_multi_prime_extra_count(r);
            const BIGNUM *exps[10], *coeffs[10];

            if (coefficientnum - 1 < pnum
                && RSA_get0_multi_prime_crt_params(r, exps, coeffs))
                bn = coeffs[coefficientnum - 1];
        }
        break;
    }

    return get_payload_bn(state, translation, ctx, bn);
}

#define IMPL_GET_RSA_PAYLOAD_FACTOR(n)                                  \
    static int                                                          \
    get_rsa_payload_f##n(enum state state,                              \
                         const struct translation_st *translation,      \
                         struct translation_ctx_st *ctx)                \
    {                                                                   \
        if (EVP_PKEY_get_base_id(ctx->p2) != EVP_PKEY_RSA               \
            && EVP_PKEY_get_base_id(ctx->p2) != EVP_PKEY_RSA_PSS)       \
            return 0;                                                   \
        return get_rsa_payload_factor(state, translation, ctx, n - 1);  \
    }

#define IMPL_GET_RSA_PAYLOAD_EXPONENT(n)                                \
    static int                                                          \
    get_rsa_payload_e##n(enum state state,                              \
                         const struct translation_st *translation,      \
                         struct translation_ctx_st *ctx)                \
    {                                                                   \
        if (EVP_PKEY_get_base_id(ctx->p2) != EVP_PKEY_RSA               \
            && EVP_PKEY_get_base_id(ctx->p2) != EVP_PKEY_RSA_PSS)       \
            return 0;                                                   \
        return get_rsa_payload_exponent(state, translation, ctx,        \
                                        n - 1);                         \
    }

#define IMPL_GET_RSA_PAYLOAD_COEFFICIENT(n)                             \
    static int                                                          \
    get_rsa_payload_c##n(enum state state,                              \
                         const struct translation_st *translation,      \
                         struct translation_ctx_st *ctx)                \
    {                                                                   \
        if (EVP_PKEY_get_base_id(ctx->p2) != EVP_PKEY_RSA               \
            && EVP_PKEY_get_base_id(ctx->p2) != EVP_PKEY_RSA_PSS)       \
            return 0;                                                   \
        return get_rsa_payload_coefficient(state, translation, ctx,     \
                                           n - 1);                      \
    }

IMPL_GET_RSA_PAYLOAD_FACTOR(1)
IMPL_GET_RSA_PAYLOAD_FACTOR(2)
IMPL_GET_RSA_PAYLOAD_FACTOR(3)
IMPL_GET_RSA_PAYLOAD_FACTOR(4)
IMPL_GET_RSA_PAYLOAD_FACTOR(5)
IMPL_GET_RSA_PAYLOAD_FACTOR(6)
IMPL_GET_RSA_PAYLOAD_FACTOR(7)
IMPL_GET_RSA_PAYLOAD_FACTOR(8)
IMPL_GET_RSA_PAYLOAD_FACTOR(9)
IMPL_GET_RSA_PAYLOAD_FACTOR(10)
IMPL_GET_RSA_PAYLOAD_EXPONENT(1)
IMPL_GET_RSA_PAYLOAD_EXPONENT(2)
IMPL_GET_RSA_PAYLOAD_EXPONENT(3)
IMPL_GET_RSA_PAYLOAD_EXPONENT(4)
IMPL_GET_RSA_PAYLOAD_EXPONENT(5)
IMPL_GET_RSA_PAYLOAD_EXPONENT(6)
IMPL_GET_RSA_PAYLOAD_EXPONENT(7)
IMPL_GET_RSA_PAYLOAD_EXPONENT(8)
IMPL_GET_RSA_PAYLOAD_EXPONENT(9)
IMPL_GET_RSA_PAYLOAD_EXPONENT(10)
IMPL_GET_RSA_PAYLOAD_COEFFICIENT(1)
IMPL_GET_RSA_PAYLOAD_COEFFICIENT(2)
IMPL_GET_RSA_PAYLOAD_COEFFICIENT(3)
IMPL_GET_RSA_PAYLOAD_COEFFICIENT(4)
IMPL_GET_RSA_PAYLOAD_COEFFICIENT(5)
IMPL_GET_RSA_PAYLOAD_COEFFICIENT(6)
IMPL_GET_RSA_PAYLOAD_COEFFICIENT(7)
IMPL_GET_RSA_PAYLOAD_COEFFICIENT(8)
IMPL_GET_RSA_PAYLOAD_COEFFICIENT(9)

static int fix_group_ecx(enum state state,
                         const struct translation_st *translation,
                         struct translation_ctx_st *ctx)
{
    const char *value = NULL;

    switch (state) {
    case PRE_PARAMS_TO_CTRL:
        if (!EVP_PKEY_CTX_IS_GEN_OP(ctx->pctx))
            return 0;
        ctx->action_type = OSSL_ACTION_NONE;
        return 1;
    case POST_PARAMS_TO_CTRL:
        if (OSSL_PARAM_get_utf8_string_ptr(ctx->params, &value) == 0 ||
            OPENSSL_strcasecmp(ctx->pctx->keytype, value) != 0) {
            ERR_raise(ERR_LIB_EVP, ERR_R_PASSED_INVALID_ARGUMENT);
            ctx->p1 = 0;
            return 0;
        }
        ctx->p1 = 1;
        return 1;
    default:
        return 0;
    }
}

/*-
 * The translation table itself
 * ============================
 */

static const struct translation_st evp_pkey_ctx_translations[] = {
    /*
     * DistID: we pass it to the backend as an octet string,
     * but get it back as a pointer to an octet string.
     *
     * Note that the EVP_PKEY_CTRL_GET1_ID_LEN is purely for legacy purposes
     * that has no separate counterpart in OSSL_PARAM terms, since we get
     * the length of the DistID automatically when getting the DistID itself.
     */
    { OSSL_ACTION_SET, -1, -1, EVP_PKEY_OP_TYPE_SIG,
      EVP_PKEY_CTRL_SET1_ID, "distid", "hexdistid",
      OSSL_PKEY_PARAM_DIST_ID, OSSL_PARAM_OCTET_STRING, NULL },
    { OSSL_ACTION_GET, -1, -1, -1,
      EVP_PKEY_CTRL_GET1_ID, "distid", "hexdistid",
      OSSL_PKEY_PARAM_DIST_ID, OSSL_PARAM_OCTET_PTR, NULL },
    { OSSL_ACTION_GET, -1, -1, -1,
      EVP_PKEY_CTRL_GET1_ID_LEN, NULL, NULL,
      OSSL_PKEY_PARAM_DIST_ID, OSSL_PARAM_OCTET_PTR, fix_distid_len },

    /*-
     * DH & DHX
     * ========
     */

    /*
     * EVP_PKEY_CTRL_DH_KDF_TYPE is used both for setting and getting.  The
     * fixup function has to handle this...
     */
    { OSSL_ACTION_NONE, EVP_PKEY_DHX, 0, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_DH_KDF_TYPE, NULL, NULL,
      OSSL_EXCHANGE_PARAM_KDF_TYPE, OSSL_PARAM_UTF8_STRING,
      fix_dh_kdf_type },
    { OSSL_ACTION_SET, EVP_PKEY_DHX, 0, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_DH_KDF_MD, NULL, NULL,
      OSSL_EXCHANGE_PARAM_KDF_DIGEST, OSSL_PARAM_UTF8_STRING, fix_md },
    { OSSL_ACTION_GET, EVP_PKEY_DHX, 0, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_GET_DH_KDF_MD, NULL, NULL,
      OSSL_EXCHANGE_PARAM_KDF_DIGEST, OSSL_PARAM_UTF8_STRING, fix_md },
    { OSSL_ACTION_SET, EVP_PKEY_DHX, 0, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_DH_KDF_OUTLEN, NULL, NULL,
      OSSL_EXCHANGE_PARAM_KDF_OUTLEN, OSSL_PARAM_UNSIGNED_INTEGER, NULL },
    { OSSL_ACTION_GET, EVP_PKEY_DHX, 0, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_GET_DH_KDF_OUTLEN, NULL, NULL,
      OSSL_EXCHANGE_PARAM_KDF_OUTLEN, OSSL_PARAM_UNSIGNED_INTEGER, NULL },
    { OSSL_ACTION_SET, EVP_PKEY_DHX, 0, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_DH_KDF_UKM, NULL, NULL,
      OSSL_EXCHANGE_PARAM_KDF_UKM, OSSL_PARAM_OCTET_STRING, NULL },
    { OSSL_ACTION_GET, EVP_PKEY_DHX, 0, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_GET_DH_KDF_UKM, NULL, NULL,
      OSSL_EXCHANGE_PARAM_KDF_UKM, OSSL_PARAM_OCTET_PTR, NULL },
    { OSSL_ACTION_SET, EVP_PKEY_DHX, 0, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_DH_KDF_OID, NULL, NULL,
      OSSL_KDF_PARAM_CEK_ALG, OSSL_PARAM_UTF8_STRING, fix_oid },
    { OSSL_ACTION_GET, EVP_PKEY_DHX, 0, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_GET_DH_KDF_OID, NULL, NULL,
      OSSL_KDF_PARAM_CEK_ALG, OSSL_PARAM_UTF8_STRING, fix_oid },

    /* DHX Keygen Parameters that are shared with DH */
    { OSSL_ACTION_SET, EVP_PKEY_DHX, 0, EVP_PKEY_OP_PARAMGEN,
      EVP_PKEY_CTRL_DH_PARAMGEN_TYPE, "dh_paramgen_type", NULL,
      OSSL_PKEY_PARAM_FFC_TYPE, OSSL_PARAM_UTF8_STRING, fix_dh_paramgen_type },
    { OSSL_ACTION_SET, EVP_PKEY_DHX, 0, EVP_PKEY_OP_PARAMGEN,
      EVP_PKEY_CTRL_DH_PARAMGEN_PRIME_LEN, "dh_paramgen_prime_len", NULL,
      OSSL_PKEY_PARAM_FFC_PBITS, OSSL_PARAM_UNSIGNED_INTEGER, NULL },
    { OSSL_ACTION_SET, EVP_PKEY_DHX, 0, EVP_PKEY_OP_PARAMGEN | EVP_PKEY_OP_KEYGEN,
      EVP_PKEY_CTRL_DH_NID, "dh_param", NULL,
      OSSL_PKEY_PARAM_GROUP_NAME, OSSL_PARAM_UTF8_STRING, NULL },
    { OSSL_ACTION_SET, EVP_PKEY_DHX, 0, EVP_PKEY_OP_PARAMGEN | EVP_PKEY_OP_KEYGEN,
      EVP_PKEY_CTRL_DH_RFC5114, "dh_rfc5114", NULL,
      OSSL_PKEY_PARAM_GROUP_NAME, OSSL_PARAM_UTF8_STRING, fix_dh_nid5114 },

    /* DH Keygen Parameters that are shared with DHX */
    { OSSL_ACTION_SET, EVP_PKEY_DH, 0, EVP_PKEY_OP_PARAMGEN,
      EVP_PKEY_CTRL_DH_PARAMGEN_TYPE, "dh_paramgen_type", NULL,
      OSSL_PKEY_PARAM_FFC_TYPE, OSSL_PARAM_UTF8_STRING, fix_dh_paramgen_type },
    { OSSL_ACTION_SET, EVP_PKEY_DH, 0, EVP_PKEY_OP_PARAMGEN,
      EVP_PKEY_CTRL_DH_PARAMGEN_PRIME_LEN, "dh_paramgen_prime_len", NULL,
      OSSL_PKEY_PARAM_FFC_PBITS, OSSL_PARAM_UNSIGNED_INTEGER, NULL },
    { OSSL_ACTION_SET, EVP_PKEY_DH, 0, EVP_PKEY_OP_PARAMGEN | EVP_PKEY_OP_KEYGEN,
      EVP_PKEY_CTRL_DH_NID, "dh_param", NULL,
      OSSL_PKEY_PARAM_GROUP_NAME, OSSL_PARAM_UTF8_STRING, fix_dh_nid },
    { OSSL_ACTION_SET, EVP_PKEY_DH, 0, EVP_PKEY_OP_PARAMGEN | EVP_PKEY_OP_KEYGEN,
      EVP_PKEY_CTRL_DH_RFC5114, "dh_rfc5114", NULL,
      OSSL_PKEY_PARAM_GROUP_NAME, OSSL_PARAM_UTF8_STRING, fix_dh_nid5114 },

    /* DH specific Keygen Parameters */
    { OSSL_ACTION_SET, EVP_PKEY_DH, 0, EVP_PKEY_OP_PARAMGEN,
      EVP_PKEY_CTRL_DH_PARAMGEN_GENERATOR, "dh_paramgen_generator", NULL,
      OSSL_PKEY_PARAM_DH_GENERATOR, OSSL_PARAM_INTEGER, NULL },

    /* DHX specific Keygen Parameters */
    { OSSL_ACTION_SET, EVP_PKEY_DHX, 0, EVP_PKEY_OP_PARAMGEN,
      EVP_PKEY_CTRL_DH_PARAMGEN_SUBPRIME_LEN, "dh_paramgen_subprime_len", NULL,
      OSSL_PKEY_PARAM_FFC_QBITS, OSSL_PARAM_UNSIGNED_INTEGER, NULL },

    { OSSL_ACTION_SET, EVP_PKEY_DH, 0, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_DH_PAD, "dh_pad", NULL,
      OSSL_EXCHANGE_PARAM_PAD, OSSL_PARAM_UNSIGNED_INTEGER, NULL },

    /*-
     * DSA
     * ===
     */
    { OSSL_ACTION_SET, EVP_PKEY_DSA, 0, EVP_PKEY_OP_PARAMGEN,
      EVP_PKEY_CTRL_DSA_PARAMGEN_BITS, "dsa_paramgen_bits", NULL,
      OSSL_PKEY_PARAM_FFC_PBITS, OSSL_PARAM_UNSIGNED_INTEGER, NULL },
    { OSSL_ACTION_SET, EVP_PKEY_DSA, 0, EVP_PKEY_OP_PARAMGEN,
      EVP_PKEY_CTRL_DSA_PARAMGEN_Q_BITS, "dsa_paramgen_q_bits", NULL,
      OSSL_PKEY_PARAM_FFC_QBITS, OSSL_PARAM_UNSIGNED_INTEGER, NULL },
    { OSSL_ACTION_SET, EVP_PKEY_DSA, 0, EVP_PKEY_OP_PARAMGEN,
      EVP_PKEY_CTRL_DSA_PARAMGEN_MD, "dsa_paramgen_md", NULL,
      OSSL_PKEY_PARAM_FFC_DIGEST, OSSL_PARAM_UTF8_STRING, fix_md },

    /*-
     * EC
     * ==
     */
    { OSSL_ACTION_SET, EVP_PKEY_EC, 0, EVP_PKEY_OP_PARAMGEN | EVP_PKEY_OP_KEYGEN,
      EVP_PKEY_CTRL_EC_PARAM_ENC, "ec_param_enc", NULL,
      OSSL_PKEY_PARAM_EC_ENCODING, OSSL_PARAM_UTF8_STRING, fix_ec_param_enc },
    { OSSL_ACTION_SET, EVP_PKEY_EC, 0, EVP_PKEY_OP_PARAMGEN | EVP_PKEY_OP_KEYGEN,
      EVP_PKEY_CTRL_EC_PARAMGEN_CURVE_NID, "ec_paramgen_curve", NULL,
      OSSL_PKEY_PARAM_GROUP_NAME, OSSL_PARAM_UTF8_STRING,
      fix_ec_paramgen_curve_nid },
    /*
     * EVP_PKEY_CTRL_EC_ECDH_COFACTOR and EVP_PKEY_CTRL_EC_KDF_TYPE are used
     * both for setting and getting.  The fixup function has to handle this...
     */
    { OSSL_ACTION_NONE, EVP_PKEY_EC, 0, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_EC_ECDH_COFACTOR, "ecdh_cofactor_mode", NULL,
      OSSL_EXCHANGE_PARAM_EC_ECDH_COFACTOR_MODE, OSSL_PARAM_INTEGER,
      fix_ecdh_cofactor },
    { OSSL_ACTION_NONE, EVP_PKEY_EC, 0, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_EC_KDF_TYPE, NULL, NULL,
      OSSL_EXCHANGE_PARAM_KDF_TYPE, OSSL_PARAM_UTF8_STRING, fix_ec_kdf_type },
    { OSSL_ACTION_SET, EVP_PKEY_EC, 0, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_EC_KDF_MD, "ecdh_kdf_md", NULL,
      OSSL_EXCHANGE_PARAM_KDF_DIGEST, OSSL_PARAM_UTF8_STRING, fix_md },
    { OSSL_ACTION_GET, EVP_PKEY_EC, 0, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_GET_EC_KDF_MD, NULL, NULL,
      OSSL_EXCHANGE_PARAM_KDF_DIGEST, OSSL_PARAM_UTF8_STRING, fix_md },
    { OSSL_ACTION_SET, EVP_PKEY_EC, 0, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_EC_KDF_OUTLEN, NULL, NULL,
      OSSL_EXCHANGE_PARAM_KDF_OUTLEN, OSSL_PARAM_UNSIGNED_INTEGER, NULL },
    { OSSL_ACTION_GET, EVP_PKEY_EC, 0, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_GET_EC_KDF_OUTLEN, NULL, NULL,
      OSSL_EXCHANGE_PARAM_KDF_OUTLEN, OSSL_PARAM_UNSIGNED_INTEGER, NULL },
    { OSSL_ACTION_SET, EVP_PKEY_EC, 0, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_EC_KDF_UKM, NULL, NULL,
      OSSL_EXCHANGE_PARAM_KDF_UKM, OSSL_PARAM_OCTET_STRING, NULL },
    { OSSL_ACTION_GET, EVP_PKEY_EC, 0, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_GET_EC_KDF_UKM, NULL, NULL,
      OSSL_EXCHANGE_PARAM_KDF_UKM, OSSL_PARAM_OCTET_PTR, NULL },

    /*-
     * SM2
     * ==
     */
    { OSSL_ACTION_SET, EVP_PKEY_SM2, 0, EVP_PKEY_OP_PARAMGEN | EVP_PKEY_OP_KEYGEN,
      EVP_PKEY_CTRL_EC_PARAM_ENC, "ec_param_enc", NULL,
      OSSL_PKEY_PARAM_EC_ENCODING, OSSL_PARAM_UTF8_STRING, fix_ec_param_enc },
    { OSSL_ACTION_SET, EVP_PKEY_SM2, 0, EVP_PKEY_OP_PARAMGEN | EVP_PKEY_OP_KEYGEN,
      EVP_PKEY_CTRL_EC_PARAMGEN_CURVE_NID, "ec_paramgen_curve", NULL,
      OSSL_PKEY_PARAM_GROUP_NAME, OSSL_PARAM_UTF8_STRING,
      fix_ec_paramgen_curve_nid },
    /*
     * EVP_PKEY_CTRL_EC_ECDH_COFACTOR and EVP_PKEY_CTRL_EC_KDF_TYPE are used
     * both for setting and getting.  The fixup function has to handle this...
     */
    { OSSL_ACTION_NONE, EVP_PKEY_SM2, 0, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_EC_ECDH_COFACTOR, "ecdh_cofactor_mode", NULL,
      OSSL_EXCHANGE_PARAM_EC_ECDH_COFACTOR_MODE, OSSL_PARAM_INTEGER,
      fix_ecdh_cofactor },
    { OSSL_ACTION_NONE, EVP_PKEY_SM2, 0, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_EC_KDF_TYPE, NULL, NULL,
      OSSL_EXCHANGE_PARAM_KDF_TYPE, OSSL_PARAM_UTF8_STRING, fix_ec_kdf_type },
    { OSSL_ACTION_SET, EVP_PKEY_SM2, 0, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_EC_KDF_MD, "ecdh_kdf_md", NULL,
      OSSL_EXCHANGE_PARAM_KDF_DIGEST, OSSL_PARAM_UTF8_STRING, fix_md },
    { OSSL_ACTION_GET, EVP_PKEY_SM2, 0, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_GET_EC_KDF_MD, NULL, NULL,
      OSSL_EXCHANGE_PARAM_KDF_DIGEST, OSSL_PARAM_UTF8_STRING, fix_md },
    { OSSL_ACTION_SET, EVP_PKEY_SM2, 0, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_EC_KDF_OUTLEN, NULL, NULL,
      OSSL_EXCHANGE_PARAM_KDF_OUTLEN, OSSL_PARAM_UNSIGNED_INTEGER, NULL },
    { OSSL_ACTION_GET, EVP_PKEY_SM2, 0, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_GET_EC_KDF_OUTLEN, NULL, NULL,
      OSSL_EXCHANGE_PARAM_KDF_OUTLEN, OSSL_PARAM_UNSIGNED_INTEGER, NULL },
    { OSSL_ACTION_SET, EVP_PKEY_SM2, 0, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_EC_KDF_UKM, NULL, NULL,
      OSSL_EXCHANGE_PARAM_KDF_UKM, OSSL_PARAM_OCTET_STRING, NULL },
    { OSSL_ACTION_GET, EVP_PKEY_SM2, 0, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_GET_EC_KDF_UKM, NULL, NULL,
      OSSL_EXCHANGE_PARAM_KDF_UKM, OSSL_PARAM_OCTET_PTR, NULL },
    /*-
     * RSA
     * ===
     */

    /*
     * RSA padding modes are numeric with ctrls, strings with ctrl_strs,
     * and can be both with OSSL_PARAM.  We standardise on strings here,
     * fix_rsa_padding_mode() does the work when the caller has a different
     * idea.
     */
    { OSSL_ACTION_SET, EVP_PKEY_RSA, EVP_PKEY_RSA_PSS,
      EVP_PKEY_OP_TYPE_CRYPT | EVP_PKEY_OP_TYPE_SIG,
      EVP_PKEY_CTRL_RSA_PADDING, "rsa_padding_mode", NULL,
      OSSL_PKEY_PARAM_PAD_MODE, OSSL_PARAM_UTF8_STRING, fix_rsa_padding_mode },
    { OSSL_ACTION_GET, EVP_PKEY_RSA, EVP_PKEY_RSA_PSS,
      EVP_PKEY_OP_TYPE_CRYPT | EVP_PKEY_OP_TYPE_SIG,
      EVP_PKEY_CTRL_GET_RSA_PADDING, NULL, NULL,
      OSSL_PKEY_PARAM_PAD_MODE, OSSL_PARAM_UTF8_STRING, fix_rsa_padding_mode },

    { OSSL_ACTION_SET, EVP_PKEY_RSA, EVP_PKEY_RSA_PSS,
      EVP_PKEY_OP_TYPE_CRYPT | EVP_PKEY_OP_TYPE_SIG,
      EVP_PKEY_CTRL_RSA_MGF1_MD, "rsa_mgf1_md", NULL,
      OSSL_PKEY_PARAM_MGF1_DIGEST, OSSL_PARAM_UTF8_STRING, fix_md },
    { OSSL_ACTION_GET, EVP_PKEY_RSA, EVP_PKEY_RSA_PSS,
      EVP_PKEY_OP_TYPE_CRYPT | EVP_PKEY_OP_TYPE_SIG,
      EVP_PKEY_CTRL_GET_RSA_MGF1_MD, NULL, NULL,
      OSSL_PKEY_PARAM_MGF1_DIGEST, OSSL_PARAM_UTF8_STRING, fix_md },

    /*
     * RSA-PSS saltlen is essentially numeric, but certain values can be
     * expressed as keywords (strings) with ctrl_str.  The corresponding
     * OSSL_PARAM allows both forms.
     * fix_rsa_pss_saltlen() takes care of the distinction.
     */
    { OSSL_ACTION_SET, EVP_PKEY_RSA, EVP_PKEY_RSA_PSS, EVP_PKEY_OP_TYPE_SIG,
      EVP_PKEY_CTRL_RSA_PSS_SALTLEN, "rsa_pss_saltlen", NULL,
      OSSL_PKEY_PARAM_RSA_PSS_SALTLEN, OSSL_PARAM_UTF8_STRING,
      fix_rsa_pss_saltlen },
    { OSSL_ACTION_GET, EVP_PKEY_RSA, EVP_PKEY_RSA_PSS, EVP_PKEY_OP_TYPE_SIG,
      EVP_PKEY_CTRL_GET_RSA_PSS_SALTLEN, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_PSS_SALTLEN, OSSL_PARAM_UTF8_STRING,
      fix_rsa_pss_saltlen },

    { OSSL_ACTION_SET, EVP_PKEY_RSA, 0, EVP_PKEY_OP_TYPE_CRYPT,
      EVP_PKEY_CTRL_RSA_OAEP_MD, "rsa_oaep_md", NULL,
      OSSL_ASYM_CIPHER_PARAM_OAEP_DIGEST, OSSL_PARAM_UTF8_STRING, fix_md },
    { OSSL_ACTION_GET, EVP_PKEY_RSA, 0, EVP_PKEY_OP_TYPE_CRYPT,
      EVP_PKEY_CTRL_GET_RSA_OAEP_MD, NULL, NULL,
      OSSL_ASYM_CIPHER_PARAM_OAEP_DIGEST, OSSL_PARAM_UTF8_STRING, fix_md },
    /*
     * The "rsa_oaep_label" ctrl_str expects the value to always be hex.
     * This is accommodated by default_fixup_args() above, which mimics that
     * expectation for any translation item where |ctrl_str| is NULL and
     * |ctrl_hexstr| is non-NULL.
     */
    { OSSL_ACTION_SET, EVP_PKEY_RSA, 0, EVP_PKEY_OP_TYPE_CRYPT,
      EVP_PKEY_CTRL_RSA_OAEP_LABEL, NULL, "rsa_oaep_label",
      OSSL_ASYM_CIPHER_PARAM_OAEP_LABEL, OSSL_PARAM_OCTET_STRING, NULL },
    { OSSL_ACTION_GET, EVP_PKEY_RSA, 0, EVP_PKEY_OP_TYPE_CRYPT,
      EVP_PKEY_CTRL_GET_RSA_OAEP_LABEL, NULL, NULL,
      OSSL_ASYM_CIPHER_PARAM_OAEP_LABEL, OSSL_PARAM_OCTET_PTR, NULL },

    { OSSL_ACTION_SET, EVP_PKEY_RSA, 0, EVP_PKEY_OP_TYPE_CRYPT,
      EVP_PKEY_CTRL_RSA_IMPLICIT_REJECTION, NULL,
      "rsa_pkcs1_implicit_rejection",
      OSSL_ASYM_CIPHER_PARAM_IMPLICIT_REJECTION, OSSL_PARAM_UNSIGNED_INTEGER,
      NULL },

    { OSSL_ACTION_SET, EVP_PKEY_RSA_PSS, 0, EVP_PKEY_OP_TYPE_GEN,
      EVP_PKEY_CTRL_MD, "rsa_pss_keygen_md", NULL,
      OSSL_ALG_PARAM_DIGEST, OSSL_PARAM_UTF8_STRING, fix_md },
    { OSSL_ACTION_SET, EVP_PKEY_RSA_PSS, 0, EVP_PKEY_OP_TYPE_GEN,
      EVP_PKEY_CTRL_RSA_MGF1_MD, "rsa_pss_keygen_mgf1_md", NULL,
      OSSL_PKEY_PARAM_MGF1_DIGEST, OSSL_PARAM_UTF8_STRING, fix_md },
    { OSSL_ACTION_SET, EVP_PKEY_RSA_PSS, 0, EVP_PKEY_OP_TYPE_GEN,
      EVP_PKEY_CTRL_RSA_PSS_SALTLEN, "rsa_pss_keygen_saltlen", NULL,
      OSSL_SIGNATURE_PARAM_PSS_SALTLEN, OSSL_PARAM_INTEGER, NULL },
    { OSSL_ACTION_SET, EVP_PKEY_RSA, EVP_PKEY_RSA_PSS, EVP_PKEY_OP_KEYGEN,
      EVP_PKEY_CTRL_RSA_KEYGEN_BITS, "rsa_keygen_bits", NULL,
      OSSL_PKEY_PARAM_RSA_BITS, OSSL_PARAM_UNSIGNED_INTEGER, NULL },
    { OSSL_ACTION_SET, EVP_PKEY_RSA, EVP_PKEY_RSA_PSS, EVP_PKEY_OP_KEYGEN,
      EVP_PKEY_CTRL_RSA_KEYGEN_PUBEXP, "rsa_keygen_pubexp", NULL,
      OSSL_PKEY_PARAM_RSA_E, OSSL_PARAM_UNSIGNED_INTEGER, NULL },
    { OSSL_ACTION_SET, EVP_PKEY_RSA, EVP_PKEY_RSA_PSS, EVP_PKEY_OP_KEYGEN,
      EVP_PKEY_CTRL_RSA_KEYGEN_PRIMES, "rsa_keygen_primes", NULL,
      OSSL_PKEY_PARAM_RSA_PRIMES, OSSL_PARAM_UNSIGNED_INTEGER, NULL },

    /*-
     * SipHash
     * ======
     */
    { OSSL_ACTION_SET, -1, -1, EVP_PKEY_OP_TYPE_SIG,
      EVP_PKEY_CTRL_SET_DIGEST_SIZE, "digestsize", NULL,
      OSSL_MAC_PARAM_SIZE, OSSL_PARAM_UNSIGNED_INTEGER, NULL },

    /*-
     * TLS1-PRF
     * ========
     */
    { OSSL_ACTION_SET, -1, -1, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_TLS_MD, "md", NULL,
      OSSL_KDF_PARAM_DIGEST, OSSL_PARAM_UTF8_STRING, fix_md },
    { OSSL_ACTION_SET, -1, -1, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_TLS_SECRET, "secret", "hexsecret",
      OSSL_KDF_PARAM_SECRET, OSSL_PARAM_OCTET_STRING, NULL },
    { OSSL_ACTION_SET, -1, -1, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_TLS_SEED, "seed", "hexseed",
      OSSL_KDF_PARAM_SEED, OSSL_PARAM_OCTET_STRING, NULL },

    /*-
     * HKDF
     * ====
     */
    { OSSL_ACTION_SET, -1, -1, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_HKDF_MD, "md", NULL,
      OSSL_KDF_PARAM_DIGEST, OSSL_PARAM_UTF8_STRING, fix_md },
    { OSSL_ACTION_SET, -1, -1, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_HKDF_SALT, "salt", "hexsalt",
      OSSL_KDF_PARAM_SALT, OSSL_PARAM_OCTET_STRING, NULL },
    { OSSL_ACTION_SET, -1, -1, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_HKDF_KEY, "key", "hexkey",
      OSSL_KDF_PARAM_KEY, OSSL_PARAM_OCTET_STRING, NULL },
    { OSSL_ACTION_SET, -1, -1, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_HKDF_INFO, "info", "hexinfo",
      OSSL_KDF_PARAM_INFO, OSSL_PARAM_OCTET_STRING, NULL },
    { OSSL_ACTION_SET, -1, -1, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_HKDF_MODE, "mode", NULL,
      OSSL_KDF_PARAM_MODE, OSSL_PARAM_INTEGER, fix_hkdf_mode },

    /*-
     * Scrypt
     * ======
     */
    { OSSL_ACTION_SET, -1, -1, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_PASS, "pass", "hexpass",
      OSSL_KDF_PARAM_PASSWORD, OSSL_PARAM_OCTET_STRING, NULL },
    { OSSL_ACTION_SET, -1, -1, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_SCRYPT_SALT, "salt", "hexsalt",
      OSSL_KDF_PARAM_SALT, OSSL_PARAM_OCTET_STRING, NULL },
    { OSSL_ACTION_SET, -1, -1, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_SCRYPT_N, "N", NULL,
      OSSL_KDF_PARAM_SCRYPT_N, OSSL_PARAM_UNSIGNED_INTEGER, NULL },
    { OSSL_ACTION_SET, -1, -1, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_SCRYPT_R, "r", NULL,
      OSSL_KDF_PARAM_SCRYPT_R, OSSL_PARAM_UNSIGNED_INTEGER, NULL },
    { OSSL_ACTION_SET, -1, -1, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_SCRYPT_P, "p", NULL,
      OSSL_KDF_PARAM_SCRYPT_P, OSSL_PARAM_UNSIGNED_INTEGER, NULL },
    { OSSL_ACTION_SET, -1, -1, EVP_PKEY_OP_DERIVE,
      EVP_PKEY_CTRL_SCRYPT_MAXMEM_BYTES, "maxmem_bytes", NULL,
      OSSL_KDF_PARAM_SCRYPT_MAXMEM, OSSL_PARAM_UNSIGNED_INTEGER, NULL },

    { OSSL_ACTION_SET, -1, -1, EVP_PKEY_OP_KEYGEN | EVP_PKEY_OP_TYPE_CRYPT,
      EVP_PKEY_CTRL_CIPHER, NULL, NULL,
      OSSL_PKEY_PARAM_CIPHER, OSSL_PARAM_UTF8_STRING, fix_cipher },
    { OSSL_ACTION_SET, -1, -1, EVP_PKEY_OP_KEYGEN,
      EVP_PKEY_CTRL_SET_MAC_KEY, "key", "hexkey",
      OSSL_PKEY_PARAM_PRIV_KEY, OSSL_PARAM_OCTET_STRING, NULL },

    { OSSL_ACTION_SET, -1, -1, EVP_PKEY_OP_TYPE_SIG,
      EVP_PKEY_CTRL_MD, NULL, NULL,
      OSSL_SIGNATURE_PARAM_DIGEST, OSSL_PARAM_UTF8_STRING, fix_md },
    { OSSL_ACTION_GET, -1, -1, EVP_PKEY_OP_TYPE_SIG,
      EVP_PKEY_CTRL_GET_MD, NULL, NULL,
      OSSL_SIGNATURE_PARAM_DIGEST, OSSL_PARAM_UTF8_STRING, fix_md },

    /*-
     * ECX
     * ===
     */
    { OSSL_ACTION_SET, EVP_PKEY_X25519, EVP_PKEY_X25519, EVP_PKEY_OP_KEYGEN, -1, NULL, NULL,
      OSSL_PKEY_PARAM_GROUP_NAME, OSSL_PARAM_UTF8_STRING, fix_group_ecx },
    { OSSL_ACTION_SET, EVP_PKEY_X25519, EVP_PKEY_X25519, EVP_PKEY_OP_PARAMGEN, -1, NULL, NULL,
      OSSL_PKEY_PARAM_GROUP_NAME, OSSL_PARAM_UTF8_STRING, fix_group_ecx },
    { OSSL_ACTION_SET, EVP_PKEY_X448, EVP_PKEY_X448, EVP_PKEY_OP_KEYGEN, -1, NULL, NULL,
      OSSL_PKEY_PARAM_GROUP_NAME, OSSL_PARAM_UTF8_STRING, fix_group_ecx },
    { OSSL_ACTION_SET, EVP_PKEY_X448, EVP_PKEY_X448, EVP_PKEY_OP_PARAMGEN, -1, NULL, NULL,
      OSSL_PKEY_PARAM_GROUP_NAME, OSSL_PARAM_UTF8_STRING, fix_group_ecx },
};

static const struct translation_st evp_pkey_translations[] = {
    /*
     * The following contain no ctrls, they are exclusively here to extract
     * key payloads from legacy keys, using OSSL_PARAMs, and rely entirely
     * on |fixup_args| to pass the actual data.  The |fixup_args| should
     * expect to get the EVP_PKEY pointer through |ctx->p2|.
     */

    /* DH, DSA & EC */
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_GROUP_NAME, OSSL_PARAM_UTF8_STRING,
      get_payload_group_name },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_PRIV_KEY, OSSL_PARAM_UNSIGNED_INTEGER,
      get_payload_private_key },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_PUB_KEY,
      0 /* no data type, let get_payload_public_key() handle that */,
      get_payload_public_key },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_EC_PUB_X, OSSL_PARAM_UNSIGNED_INTEGER,
      get_payload_public_key_ec },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_EC_PUB_Y, OSSL_PARAM_UNSIGNED_INTEGER,
      get_payload_public_key_ec },

    /* DH and DSA */
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_FFC_P, OSSL_PARAM_UNSIGNED_INTEGER,
      get_dh_dsa_payload_p },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_FFC_G, OSSL_PARAM_UNSIGNED_INTEGER,
      get_dh_dsa_payload_g },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_FFC_Q, OSSL_PARAM_UNSIGNED_INTEGER,
      get_dh_dsa_payload_q },

    /* RSA */
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_N, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_n },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_E, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_e },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_D, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_d },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_FACTOR1, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_f1 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_FACTOR2, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_f2 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_FACTOR3, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_f3 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_FACTOR4, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_f4 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_FACTOR5, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_f5 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_FACTOR6, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_f6 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_FACTOR7, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_f7 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_FACTOR8, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_f8 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_FACTOR9, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_f9 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_FACTOR10, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_f10 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_EXPONENT1, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_e1 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_EXPONENT2, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_e2 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_EXPONENT3, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_e3 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_EXPONENT4, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_e4 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_EXPONENT5, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_e5 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_EXPONENT6, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_e6 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_EXPONENT7, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_e7 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_EXPONENT8, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_e8 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_EXPONENT9, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_e9 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_EXPONENT10, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_e10 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_COEFFICIENT1, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_c1 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_COEFFICIENT2, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_c2 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_COEFFICIENT3, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_c3 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_COEFFICIENT4, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_c4 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_COEFFICIENT5, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_c5 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_COEFFICIENT6, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_c6 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_COEFFICIENT7, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_c7 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_COEFFICIENT8, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_c8 },
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_RSA_COEFFICIENT9, OSSL_PARAM_UNSIGNED_INTEGER,
      get_rsa_payload_c9 },

    /* EC */
    { OSSL_ACTION_GET, -1, -1, -1, 0, NULL, NULL,
      OSSL_PKEY_PARAM_EC_DECODED_FROM_EXPLICIT_PARAMS, OSSL_PARAM_INTEGER,
      get_ec_decoded_from_explicit_params },
};

static const struct translation_st *
lookup_translation(struct translation_st *tmpl,
                   const struct translation_st *translations,
                   size_t translations_num)
{
    size_t i;

    for (i = 0; i < translations_num; i++) {
        const struct translation_st *item = &translations[i];

        /*
         * Sanity check the translation table item.
         *
         * 1.  Either both keytypes are -1, or neither of them are.
         * 2.  TBA...
         */
        if (!ossl_assert((item->keytype1 == -1) == (item->keytype2 == -1)))
            continue;


        /*
         * Base search criteria: check that the optype and keytypes match,
         * if relevant.  All callers must synthesise these bits somehow.
         */
        if (item->optype != -1 && (tmpl->optype & item->optype) == 0)
            continue;
        /*
         * This expression is stunningly simple thanks to the sanity check
         * above.
         */
        if (item->keytype1 != -1
            && tmpl->keytype1 != item->keytype1
            && tmpl->keytype2 != item->keytype2)
            continue;

        /*
         * Done with the base search criteria, now we check the criteria for
         * the individual types of translations:
         * ctrl->params, ctrl_str->params, and params->ctrl
         */
        if (tmpl->ctrl_num != 0) {
            if (tmpl->ctrl_num != item->ctrl_num)
                continue;
        } else if (tmpl->ctrl_str != NULL) {
            const char *ctrl_str = NULL;
            const char *ctrl_hexstr = NULL;

            /*
             * Search criteria that originates from a ctrl_str is only used
             * for setting, never for getting.  Therefore, we only look at
             * the setter items.
             */
            if (item->action_type != OSSL_ACTION_NONE
                && item->action_type != OSSL_ACTION_SET)
                continue;
            /*
             * At least one of the ctrl cmd names must be match the ctrl
             * cmd name in the template.
             */
            if (item->ctrl_str != NULL
                && OPENSSL_strcasecmp(tmpl->ctrl_str, item->ctrl_str) == 0)
                ctrl_str = tmpl->ctrl_str;
            else if (item->ctrl_hexstr != NULL
                     && OPENSSL_strcasecmp(tmpl->ctrl_hexstr,
                                           item->ctrl_hexstr) == 0)
                ctrl_hexstr = tmpl->ctrl_hexstr;
            else
                continue;

            /* Modify the template to signal which string matched */
            tmpl->ctrl_str = ctrl_str;
            tmpl->ctrl_hexstr = ctrl_hexstr;
        } else if (tmpl->param_key != NULL) {
            /*
             * Search criteria that originates from an OSSL_PARAM setter or
             * getter.
             *
             * Ctrls were fundamentally bidirectional, with only the ctrl
             * command macro name implying direction (if you're lucky).
             * A few ctrl commands were even taking advantage of the
             * bidirectional nature, making the direction depend in the
             * value of the numeric argument.
             *
             * OSSL_PARAM functions are fundamentally different, in that
             * setters and getters are separated, so the data direction is
             * implied by the function that's used.  The same OSSL_PARAM
             * key name can therefore be used in both directions.  We must
             * therefore take the action type into account in this case.
             */
            if ((item->action_type != OSSL_ACTION_NONE
                 && tmpl->action_type != item->action_type)
                || (item->param_key != NULL
                    && OPENSSL_strcasecmp(tmpl->param_key,
                                          item->param_key) != 0))
                continue;
        } else {
            return NULL;
        }

        return item;
    }

    return NULL;
}

static const struct translation_st *
lookup_evp_pkey_ctx_translation(struct translation_st *tmpl)
{
    return lookup_translation(tmpl, evp_pkey_ctx_translations,
                              OSSL_NELEM(evp_pkey_ctx_translations));
}

static const struct translation_st *
lookup_evp_pkey_translation(struct translation_st *tmpl)
{
    return lookup_translation(tmpl, evp_pkey_translations,
                              OSSL_NELEM(evp_pkey_translations));
}

/* This must ONLY be called for provider side operations */
int evp_pkey_ctx_ctrl_to_param(EVP_PKEY_CTX *pctx,
                               int keytype, int optype,
                               int cmd, int p1, void *p2)
{
    struct translation_ctx_st ctx = { 0, };
    struct translation_st tmpl = { 0, };
    const struct translation_st *translation = NULL;
    OSSL_PARAM params[2] = { OSSL_PARAM_END, OSSL_PARAM_END };
    int ret;
    fixup_args_fn *fixup = default_fixup_args;

    if (keytype == -1)
        keytype = pctx->legacy_keytype;
    tmpl.ctrl_num = cmd;
    tmpl.keytype1 = tmpl.keytype2 = keytype;
    tmpl.optype = optype;
    translation = lookup_evp_pkey_ctx_translation(&tmpl);

    if (translation == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
        return -2;
    }

    if (pctx->pmeth != NULL
        && pctx->pmeth->pkey_id != translation->keytype1
        && pctx->pmeth->pkey_id != translation->keytype2)
        return -1;

    if (translation->fixup_args != NULL)
        fixup = translation->fixup_args;
    ctx.action_type = translation->action_type;
    ctx.ctrl_cmd = cmd;
    ctx.p1 = p1;
    ctx.p2 = p2;
    ctx.pctx = pctx;
    ctx.params = params;

    ret = fixup(PRE_CTRL_TO_PARAMS, translation, &ctx);

    if (ret > 0) {
        switch (ctx.action_type) {
        default:
            /* fixup_args is expected to make sure this is dead code */
            break;
        case OSSL_ACTION_GET:
            ret = evp_pkey_ctx_get_params_strict(pctx, ctx.params);
            break;
        case OSSL_ACTION_SET:
            ret = evp_pkey_ctx_set_params_strict(pctx, ctx.params);
            break;
        }
    }

    /*
     * In POST, we pass the return value as p1, allowing the fixup_args
     * function to affect it by changing its value.
     */
    if (ret > 0) {
        ctx.p1 = ret;
        fixup(POST_CTRL_TO_PARAMS, translation, &ctx);
        ret = ctx.p1;
    }

    cleanup_translation_ctx(POST_CTRL_TO_PARAMS, translation, &ctx);

    return ret;
}

/* This must ONLY be called for provider side operations */
int evp_pkey_ctx_ctrl_str_to_param(EVP_PKEY_CTX *pctx,
                                   const char *name, const char *value)
{
    struct translation_ctx_st ctx = { 0, };
    struct translation_st tmpl = { 0, };
    const struct translation_st *translation = NULL;
    OSSL_PARAM params[2] = { OSSL_PARAM_END, OSSL_PARAM_END };
    int keytype = pctx->legacy_keytype;
    int optype = pctx->operation == 0 ? -1 : pctx->operation;
    int ret;
    fixup_args_fn *fixup = default_fixup_args;

    tmpl.action_type = OSSL_ACTION_SET;
    tmpl.keytype1 = tmpl.keytype2 = keytype;
    tmpl.optype = optype;
    tmpl.ctrl_str = name;
    tmpl.ctrl_hexstr = name;
    translation = lookup_evp_pkey_ctx_translation(&tmpl);

    if (translation != NULL) {
        if (translation->fixup_args != NULL)
            fixup = translation->fixup_args;
        ctx.action_type = translation->action_type;
        ctx.ishex = (tmpl.ctrl_hexstr != NULL);
    } else {
        /* String controls really only support setting */
        ctx.action_type = OSSL_ACTION_SET;
    }
    ctx.ctrl_str = name;
    ctx.p1 = (int)strlen(value);
    ctx.p2 = (char *)value;
    ctx.pctx = pctx;
    ctx.params = params;

    ret = fixup(PRE_CTRL_STR_TO_PARAMS, translation, &ctx);

    if (ret > 0) {
        switch (ctx.action_type) {
        default:
            /* fixup_args is expected to make sure this is dead code */
            break;
        case OSSL_ACTION_GET:
            /*
             * this is dead code, but must be present, or some compilers
             * will complain
             */
            break;
        case OSSL_ACTION_SET:
            ret = evp_pkey_ctx_set_params_strict(pctx, ctx.params);
            break;
        }
    }

    if (ret > 0)
        ret = fixup(POST_CTRL_STR_TO_PARAMS, translation, &ctx);

    cleanup_translation_ctx(CLEANUP_CTRL_STR_TO_PARAMS, translation, &ctx);

    return ret;
}

/* This must ONLY be called for legacy operations */
static int evp_pkey_ctx_setget_params_to_ctrl(EVP_PKEY_CTX *pctx,
                                              enum action action_type,
                                              OSSL_PARAM *params)
{
    int keytype = pctx->legacy_keytype;
    int optype = pctx->operation == 0 ? -1 : pctx->operation;

    for (; params != NULL && params->key != NULL; params++) {
        struct translation_ctx_st ctx = { 0, };
        struct translation_st tmpl = { 0, };
        const struct translation_st *translation = NULL;
        fixup_args_fn *fixup = default_fixup_args;
        int ret;

        ctx.action_type = tmpl.action_type = action_type;
        tmpl.keytype1 = tmpl.keytype2 = keytype;
        tmpl.optype = optype;
        tmpl.param_key = params->key;
        translation = lookup_evp_pkey_ctx_translation(&tmpl);

        if (translation != NULL) {
            if (translation->fixup_args != NULL)
                fixup = translation->fixup_args;
            ctx.ctrl_cmd = translation->ctrl_num;
        }
        ctx.pctx = pctx;
        ctx.params = params;

        ret = fixup(PRE_PARAMS_TO_CTRL, translation, &ctx);

        if (ret > 0 && ctx.action_type != OSSL_ACTION_NONE)
            ret = EVP_PKEY_CTX_ctrl(pctx, keytype, optype,
                                    ctx.ctrl_cmd, ctx.p1, ctx.p2);

        /*
         * In POST, we pass the return value as p1, allowing the fixup_args
         * function to put it to good use, or maybe affect it.
         *
         * NOTE: even though EVP_PKEY_CTX_ctrl return value is documented
         * as return positive on Success and 0 or negative on falure. There
         * maybe parameters (e.g. ecdh_cofactor), which actually return 0
         * as success value. That is why we do POST_PARAMS_TO_CTRL for 0
         * value as well
         */
        if (ret >= 0) {
            ctx.p1 = ret;
            fixup(POST_PARAMS_TO_CTRL, translation, &ctx);
            ret = ctx.p1;
        }

        cleanup_translation_ctx(CLEANUP_PARAMS_TO_CTRL, translation, &ctx);

        if (ret <= 0)
            return 0;
    }
    return 1;
}

int evp_pkey_ctx_set_params_to_ctrl(EVP_PKEY_CTX *ctx, const OSSL_PARAM *params)
{
    if (ctx->keymgmt != NULL)
        return 0;
    return evp_pkey_ctx_setget_params_to_ctrl(ctx, OSSL_ACTION_SET, (OSSL_PARAM *)params);
}

int evp_pkey_ctx_get_params_to_ctrl(EVP_PKEY_CTX *ctx, OSSL_PARAM *params)
{
    if (ctx->keymgmt != NULL)
        return 0;
    return evp_pkey_ctx_setget_params_to_ctrl(ctx, OSSL_ACTION_GET, params);
}

/* This must ONLY be called for legacy EVP_PKEYs */
static int evp_pkey_setget_params_to_ctrl(const EVP_PKEY *pkey,
                                          enum action action_type,
                                          OSSL_PARAM *params)
{
    int ret = 1;

    for (; params != NULL && params->key != NULL; params++) {
        struct translation_ctx_st ctx = { 0, };
        struct translation_st tmpl = { 0, };
        const struct translation_st *translation = NULL;
        fixup_args_fn *fixup = default_fixup_args;

        tmpl.action_type = action_type;
        tmpl.param_key = params->key;
        translation = lookup_evp_pkey_translation(&tmpl);

        if (translation != NULL) {
            if (translation->fixup_args != NULL)
                fixup = translation->fixup_args;
            ctx.action_type = translation->action_type;
        }
        ctx.p2 = (void *)pkey;
        ctx.params = params;

        /*
         * EVP_PKEY doesn't have any ctrl function, so we rely completely
         * on fixup_args to do the whole work.  Also, we currently only
         * support getting.
         */
        if (!ossl_assert(translation != NULL)
            || !ossl_assert(translation->action_type == OSSL_ACTION_GET)
            || !ossl_assert(translation->fixup_args != NULL)) {
            return -2;
        }

        ret = fixup(PKEY, translation, &ctx);

        cleanup_translation_ctx(PKEY, translation, &ctx);
    }
    return ret;
}

int evp_pkey_get_params_to_ctrl(const EVP_PKEY *pkey, OSSL_PARAM *params)
{
    return evp_pkey_setget_params_to_ctrl(pkey, OSSL_ACTION_GET, params);
}
