/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_INTERNAL_CRYPTLIB_H
# define OSSL_INTERNAL_CRYPTLIB_H
# pragma once

# ifdef OPENSSL_USE_APPLINK
#  define BIO_FLAGS_UPLINK_INTERNAL 0x8000
#  include "ms/uplink.h"
# else
#  define BIO_FLAGS_UPLINK_INTERNAL 0
# endif

# include "internal/common.h"

# include <openssl/crypto.h>
# include <openssl/buffer.h>
# include <openssl/bio.h>
# include <openssl/asn1.h>
# include <openssl/err.h>

typedef struct ex_callback_st EX_CALLBACK;
DEFINE_STACK_OF(EX_CALLBACK)

typedef struct mem_st MEM;
DEFINE_LHASH_OF_EX(MEM);

void OPENSSL_cpuid_setup(void);
#if defined(__i386)   || defined(__i386__)   || defined(_M_IX86) || \
    defined(__x86_64) || defined(__x86_64__) || \
    defined(_M_AMD64) || defined(_M_X64)
#  define OPENSSL_IA32CAP_P_MAX_INDEXES 10
extern unsigned int OPENSSL_ia32cap_P[];
#endif

void OPENSSL_showfatal(const char *fmta, ...);
int ossl_do_ex_data_init(OSSL_LIB_CTX *ctx);
void ossl_crypto_cleanup_all_ex_data_int(OSSL_LIB_CTX *ctx);
int openssl_init_fork_handlers(void);
int openssl_get_fork_id(void);

char *ossl_safe_getenv(const char *name);

extern CRYPTO_RWLOCK *memdbg_lock;
int openssl_strerror_r(int errnum, char *buf, size_t buflen);
# if !defined(OPENSSL_NO_STDIO)
FILE *openssl_fopen(const char *filename, const char *mode);
# else
void *openssl_fopen(const char *filename, const char *mode);
# endif

uint32_t OPENSSL_rdtsc(void);
size_t OPENSSL_instrument_bus(unsigned int *, size_t);
size_t OPENSSL_instrument_bus2(unsigned int *, size_t, size_t);

/* ex_data structures */

/*
 * Each structure type (sometimes called a class), that supports
 * exdata has a stack of callbacks for each instance.
 */
struct ex_callback_st {
    long argl;                  /* Arbitrary long */
    void *argp;                 /* Arbitrary void * */
    int priority;               /* Priority ordering for freeing */
    CRYPTO_EX_new *new_func;
    CRYPTO_EX_free *free_func;
    CRYPTO_EX_dup *dup_func;
};

/*
 * The state for each class.  This could just be a typedef, but
 * a structure allows future changes.
 */
typedef struct ex_callbacks_st {
    STACK_OF(EX_CALLBACK) *meth;
} EX_CALLBACKS;

typedef struct ossl_ex_data_global_st {
    CRYPTO_RWLOCK *ex_data_lock;
    EX_CALLBACKS ex_data[CRYPTO_EX_INDEX__COUNT];
} OSSL_EX_DATA_GLOBAL;

/* OSSL_LIB_CTX */

# define OSSL_LIB_CTX_PROVIDER_STORE_RUN_ONCE_INDEX          0
# define OSSL_LIB_CTX_DEFAULT_METHOD_STORE_RUN_ONCE_INDEX    1
# define OSSL_LIB_CTX_METHOD_STORE_RUN_ONCE_INDEX            2
# define OSSL_LIB_CTX_MAX_RUN_ONCE                           3

# define OSSL_LIB_CTX_EVP_METHOD_STORE_INDEX         0
# define OSSL_LIB_CTX_PROVIDER_STORE_INDEX           1
# define OSSL_LIB_CTX_PROPERTY_DEFN_INDEX            2
# define OSSL_LIB_CTX_PROPERTY_STRING_INDEX          3
# define OSSL_LIB_CTX_NAMEMAP_INDEX                  4
# define OSSL_LIB_CTX_DRBG_INDEX                     5
# define OSSL_LIB_CTX_DRBG_NONCE_INDEX               6
/* slot 7 unused, was CRNG test data and can be reused */
# ifdef FIPS_MODULE
#  define OSSL_LIB_CTX_THREAD_EVENT_HANDLER_INDEX    8
# endif
# define OSSL_LIB_CTX_FIPS_PROV_INDEX                9
# define OSSL_LIB_CTX_ENCODER_STORE_INDEX           10
# define OSSL_LIB_CTX_DECODER_STORE_INDEX           11
# define OSSL_LIB_CTX_SELF_TEST_CB_INDEX            12
# define OSSL_LIB_CTX_BIO_PROV_INDEX                13
# define OSSL_LIB_CTX_GLOBAL_PROPERTIES             14
# define OSSL_LIB_CTX_STORE_LOADER_STORE_INDEX      15
# define OSSL_LIB_CTX_PROVIDER_CONF_INDEX           16
# define OSSL_LIB_CTX_BIO_CORE_INDEX                17
# define OSSL_LIB_CTX_CHILD_PROVIDER_INDEX          18
# define OSSL_LIB_CTX_THREAD_INDEX                  19
# define OSSL_LIB_CTX_DECODER_CACHE_INDEX           20
# define OSSL_LIB_CTX_COMP_METHODS                  21
# define OSSL_LIB_CTX_INDICATOR_CB_INDEX            22
# define OSSL_LIB_CTX_MAX_INDEXES                   22

OSSL_LIB_CTX *ossl_lib_ctx_get_concrete(OSSL_LIB_CTX *ctx);
int ossl_lib_ctx_is_default(OSSL_LIB_CTX *ctx);
int ossl_lib_ctx_is_global_default(OSSL_LIB_CTX *ctx);

/* Functions to retrieve pointers to data by index */
void *ossl_lib_ctx_get_data(OSSL_LIB_CTX *, int /* index */);

void ossl_lib_ctx_default_deinit(void);
OSSL_EX_DATA_GLOBAL *ossl_lib_ctx_get_ex_data_global(OSSL_LIB_CTX *ctx);

const char *ossl_lib_ctx_get_descriptor(OSSL_LIB_CTX *libctx);
CRYPTO_THREAD_LOCAL *ossl_lib_ctx_get_rcukey(OSSL_LIB_CTX *libctx);

OSSL_LIB_CTX *ossl_crypto_ex_data_get_ossl_lib_ctx(const CRYPTO_EX_DATA *ad);
int ossl_crypto_new_ex_data_ex(OSSL_LIB_CTX *ctx, int class_index, void *obj,
                               CRYPTO_EX_DATA *ad);
int ossl_crypto_get_ex_new_index_ex(OSSL_LIB_CTX *ctx, int class_index,
                                    long argl, void *argp,
                                    CRYPTO_EX_new *new_func,
                                    CRYPTO_EX_dup *dup_func,
                                    CRYPTO_EX_free *free_func,
                                    int priority);
int ossl_crypto_free_ex_index_ex(OSSL_LIB_CTX *ctx, int class_index, int idx);

/* Function for simple binary search */

/* Flags */
# define OSSL_BSEARCH_VALUE_ON_NOMATCH            0x01
# define OSSL_BSEARCH_FIRST_VALUE_ON_MATCH        0x02

const void *ossl_bsearch(const void *key, const void *base, int num,
                         int size, int (*cmp) (const void *, const void *),
                         int flags);

char *ossl_sk_ASN1_UTF8STRING2text(STACK_OF(ASN1_UTF8STRING) *text,
                                   const char *sep, size_t max_len);
char *ossl_ipaddr_to_asc(unsigned char *p, int len);

char *ossl_buf2hexstr_sep(const unsigned char *buf, long buflen, char sep);
unsigned char *ossl_hexstr2buf_sep(const char *str, long *buflen,
                                   const char sep);

/**
 *  Writes |n| value in hex format into |buf|,
 *  and returns the number of bytes written
 */
size_t ossl_to_hex(char *buf, uint8_t n);

STACK_OF(SSL_COMP) *ossl_load_builtin_compressions(void);
void ossl_free_compression_methods_int(STACK_OF(SSL_COMP) *methods);

#endif
