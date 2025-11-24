/*
 * Copyright 2001-2023 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2002, Oracle and/or its affiliates. All rights reserved
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_ENGINE_ENG_LOCAL_H
# define OSSL_CRYPTO_ENGINE_ENG_LOCAL_H

# include <openssl/trace.h>
# include "internal/cryptlib.h"
# include "crypto/engine.h"
# include "internal/thread_once.h"
# include "internal/refcount.h"

extern CRYPTO_RWLOCK *global_engine_lock;

/*
 * This prints the engine's pointer address, "struct" or "funct" to
 * indicate the reference type, the before and after reference count, and
 * the file:line-number pair. The "ENGINE_REF_PRINT" statements must come
 * *after* the change.
 */
# define ENGINE_REF_PRINT(e, isfunct, diff)                             \
    OSSL_TRACE6(ENGINE_REF_COUNT,                                       \
               "engine: %p %s from %d to %d (%s:%d)\n",                 \
               (void *)(e), (isfunct ? "funct" : "struct"),             \
               ((isfunct)                                               \
                ? ((e)->funct_ref - (diff))                             \
                : (eng_struct_ref(e) - (diff))),                        \
               ((isfunct) ? (e)->funct_ref : eng_struct_ref(e)),        \
               (OPENSSL_FILE), (OPENSSL_LINE))

/*
 * Any code that will need cleanup operations should use these functions to
 * register callbacks. engine_cleanup_int() will call all registered
 * callbacks in order. NB: both the "add" functions assume the engine lock to
 * already be held (in "write" mode).
 */
typedef void (ENGINE_CLEANUP_CB) (void);
typedef struct st_engine_cleanup_item {
    ENGINE_CLEANUP_CB *cb;
} ENGINE_CLEANUP_ITEM;
DEFINE_STACK_OF(ENGINE_CLEANUP_ITEM)
int engine_cleanup_add_first(ENGINE_CLEANUP_CB *cb);
int engine_cleanup_add_last(ENGINE_CLEANUP_CB *cb);

/* We need stacks of ENGINEs for use in eng_table.c */
DEFINE_STACK_OF(ENGINE)

/*
 * This represents an implementation table. Dependent code should instantiate
 * it as a (ENGINE_TABLE *) pointer value set initially to NULL.
 */
typedef struct st_engine_table ENGINE_TABLE;
int engine_table_register(ENGINE_TABLE **table, ENGINE_CLEANUP_CB *cleanup,
                          ENGINE *e, const int *nids, int num_nids,
                          int setdefault);
void engine_table_unregister(ENGINE_TABLE **table, ENGINE *e);
void engine_table_cleanup(ENGINE_TABLE **table);
ENGINE *ossl_engine_table_select(ENGINE_TABLE **table, int nid,
                                 const char *f, int l);
typedef void (engine_table_doall_cb) (int nid, STACK_OF(ENGINE) *sk,
                                      ENGINE *def, void *arg);
void engine_table_doall(ENGINE_TABLE *table, engine_table_doall_cb *cb,
                        void *arg);

/*
 * Internal versions of API functions that have control over locking. These
 * are used between C files when functionality needs to be shared but the
 * caller may already be controlling of the engine lock.
 */
int engine_unlocked_init(ENGINE *e);
int engine_unlocked_finish(ENGINE *e, int unlock_for_handlers);
int engine_free_util(ENGINE *e, int not_locked);

/*
 * This function will reset all "set"able values in an ENGINE to NULL. This
 * won't touch reference counts or ex_data, but is equivalent to calling all
 * the ENGINE_set_***() functions with a NULL value.
 */
void engine_set_all_null(ENGINE *e);

/*
 * NB: Bitwise OR-able values for the "flags" variable in ENGINE are now
 * exposed in engine.h.
 */

/* Free up dynamically allocated public key methods associated with ENGINE */

void engine_pkey_meths_free(ENGINE *e);
void engine_pkey_asn1_meths_free(ENGINE *e);

/* Once initialisation function */
extern CRYPTO_ONCE engine_lock_init;
DECLARE_RUN_ONCE(do_engine_lock_init)

typedef void (*ENGINE_DYNAMIC_ID)(void);
int engine_add_dynamic_id(ENGINE *e, ENGINE_DYNAMIC_ID dynamic_id,
                          int not_locked);
void engine_remove_dynamic_id(ENGINE *e, int not_locked);

/*
 * This is a structure for storing implementations of various crypto
 * algorithms and functions.
 */
struct engine_st {
    const char *id;
    const char *name;
    const RSA_METHOD *rsa_meth;
    const DSA_METHOD *dsa_meth;
    const DH_METHOD *dh_meth;
    const EC_KEY_METHOD *ec_meth;
    const RAND_METHOD *rand_meth;
    /* Cipher handling is via this callback */
    ENGINE_CIPHERS_PTR ciphers;
    /* Digest handling is via this callback */
    ENGINE_DIGESTS_PTR digests;
    /* Public key handling via this callback */
    ENGINE_PKEY_METHS_PTR pkey_meths;
    /* ASN1 public key handling via this callback */
    ENGINE_PKEY_ASN1_METHS_PTR pkey_asn1_meths;
    ENGINE_GEN_INT_FUNC_PTR destroy;
    ENGINE_GEN_INT_FUNC_PTR init;
    ENGINE_GEN_INT_FUNC_PTR finish;
    ENGINE_CTRL_FUNC_PTR ctrl;
    ENGINE_LOAD_KEY_PTR load_privkey;
    ENGINE_LOAD_KEY_PTR load_pubkey;
    ENGINE_SSL_CLIENT_CERT_PTR load_ssl_client_cert;
    const ENGINE_CMD_DEFN *cmd_defns;
    int flags;
    /* reference count on the structure itself */
    CRYPTO_REF_COUNT struct_ref;
    /*
     * reference count on usability of the engine type. NB: This controls the
     * loading and initialisation of any functionality required by this
     * engine, whereas the previous count is simply to cope with
     * (de)allocation of this structure. Hence, running_ref <= struct_ref at
     * all times.
     */
    int funct_ref;
    /* A place to store per-ENGINE data */
    CRYPTO_EX_DATA ex_data;
    /* Used to maintain the linked-list of engines. */
    struct engine_st *prev;
    struct engine_st *next;
    /* Used to maintain the linked-list of dynamic engines. */
    struct engine_st *prev_dyn;
    struct engine_st *next_dyn;
    ENGINE_DYNAMIC_ID dynamic_id;
};

typedef struct st_engine_pile ENGINE_PILE;

DEFINE_LHASH_OF_EX(ENGINE_PILE);

static ossl_unused ossl_inline int eng_struct_ref(ENGINE *e)
{
    int res;

    CRYPTO_GET_REF(&e->struct_ref, &res);
    return res;
}

#endif                          /* OSSL_CRYPTO_ENGINE_ENG_LOCAL_H */
