/*
 * Copyright 2000-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core_dispatch.h>
#include "internal/refcount.h"

#define EVP_CTRL_RET_UNSUPPORTED -1


struct evp_md_ctx_st {
    const EVP_MD *reqdigest;    /* The original requested digest */
    const EVP_MD *digest;
    ENGINE *engine;             /* functional reference if 'digest' is
                                 * ENGINE-provided */
    unsigned long flags;
    void *md_data;
    /* Public key context for sign/verify */
    EVP_PKEY_CTX *pctx;
    /* Update function: usually copied from EVP_MD */
    int (*update) (EVP_MD_CTX *ctx, const void *data, size_t count);

    /*
     * Opaque ctx returned from a providers digest algorithm implementation
     * OSSL_FUNC_digest_newctx()
     */
    void *algctx;
    EVP_MD *fetched_digest;
} /* EVP_MD_CTX */ ;

struct evp_cipher_ctx_st {
    const EVP_CIPHER *cipher;
    ENGINE *engine;             /* functional reference if 'cipher' is
                                 * ENGINE-provided */
    int encrypt;                /* encrypt or decrypt */
    int buf_len;                /* number we have left */
    unsigned char oiv[EVP_MAX_IV_LENGTH]; /* original iv */
    unsigned char iv[EVP_MAX_IV_LENGTH]; /* working iv */
    unsigned char buf[EVP_MAX_BLOCK_LENGTH]; /* saved partial block */
    int num;                    /* used by cfb/ofb/ctr mode */
    /* FIXME: Should this even exist? It appears unused */
    void *app_data;             /* application stuff */
    int key_len;                /* May change for variable length cipher */
    int iv_len;                 /* IV length */
    unsigned long flags;        /* Various flags */
    void *cipher_data;          /* per EVP data */
    int final_used;
    int block_mask;
    unsigned char final[EVP_MAX_BLOCK_LENGTH]; /* possible final block */
    size_t numpipes;

    /*
     * Opaque ctx returned from a providers cipher algorithm implementation
     * OSSL_FUNC_cipher_newctx()
     */
    void *algctx;
    EVP_CIPHER *fetched_cipher;
} /* EVP_CIPHER_CTX */ ;

struct evp_mac_ctx_st {
    EVP_MAC *meth;               /* Method structure */
    /*
     * Opaque ctx returned from a providers MAC algorithm implementation
     * OSSL_FUNC_mac_newctx()
     */
    void *algctx;
} /* EVP_MAC_CTX */;

struct evp_kdf_ctx_st {
    EVP_KDF *meth;              /* Method structure */
    /*
     * Opaque ctx returned from a providers KDF algorithm implementation
     * OSSL_FUNC_kdf_newctx()
     */
    void *algctx;
} /* EVP_KDF_CTX */ ;

struct evp_rand_ctx_st {
    EVP_RAND *meth;             /* Method structure */
    /*
     * Opaque ctx returned from a providers rand algorithm implementation
     * OSSL_FUNC_rand_newctx()
     */
    void *algctx;
    EVP_RAND_CTX *parent;       /* Parent EVP_RAND or NULL if none */
    CRYPTO_REF_COUNT refcnt;    /* Context reference count */
    CRYPTO_RWLOCK *refcnt_lock;
} /* EVP_RAND_CTX */ ;

struct evp_keymgmt_st {
    int id;                      /* libcrypto internal */

    int name_id;
    /* NID for the legacy alg if there is one */
    int legacy_alg;
    char *type_name;
    const char *description;
    OSSL_PROVIDER *prov;
    CRYPTO_REF_COUNT refcnt;

    /* Constructor(s), destructor, information */
    OSSL_FUNC_keymgmt_new_fn *new;
    OSSL_FUNC_keymgmt_free_fn *free;
    OSSL_FUNC_keymgmt_get_params_fn *get_params;
    OSSL_FUNC_keymgmt_gettable_params_fn *gettable_params;
    OSSL_FUNC_keymgmt_set_params_fn *set_params;
    OSSL_FUNC_keymgmt_settable_params_fn *settable_params;

    /* Generation, a complex constructor */
    OSSL_FUNC_keymgmt_gen_init_fn *gen_init;
    OSSL_FUNC_keymgmt_gen_set_template_fn *gen_set_template;
    OSSL_FUNC_keymgmt_gen_get_params_fn *gen_get_params;
    OSSL_FUNC_keymgmt_gen_gettable_params_fn *gen_gettable_params;
    OSSL_FUNC_keymgmt_gen_set_params_fn *gen_set_params;
    OSSL_FUNC_keymgmt_gen_settable_params_fn *gen_settable_params;
    OSSL_FUNC_keymgmt_gen_fn *gen;
    OSSL_FUNC_keymgmt_gen_cleanup_fn *gen_cleanup;

    OSSL_FUNC_keymgmt_load_fn *load;

    /* Key object checking */
    OSSL_FUNC_keymgmt_query_operation_name_fn *query_operation_name;
    OSSL_FUNC_keymgmt_has_fn *has;
    OSSL_FUNC_keymgmt_validate_fn *validate;
    OSSL_FUNC_keymgmt_match_fn *match;

    /* Import and export routines */
    OSSL_FUNC_keymgmt_import_fn *import;
    OSSL_FUNC_keymgmt_import_types_fn *import_types;
    OSSL_FUNC_keymgmt_import_types_ex_fn *import_types_ex;
    OSSL_FUNC_keymgmt_export_fn *export;
    OSSL_FUNC_keymgmt_export_types_fn *export_types;
    OSSL_FUNC_keymgmt_export_types_ex_fn *export_types_ex;
    OSSL_FUNC_keymgmt_dup_fn *dup;
} /* EVP_KEYMGMT */ ;

struct evp_keyexch_st {
    int name_id;
    char *type_name;
    const char *description;
    OSSL_PROVIDER *prov;
    CRYPTO_REF_COUNT refcnt;

    OSSL_FUNC_keyexch_newctx_fn *newctx;
    OSSL_FUNC_keyexch_init_fn *init;
    OSSL_FUNC_keyexch_set_peer_fn *set_peer;
    OSSL_FUNC_keyexch_derive_fn *derive;
    OSSL_FUNC_keyexch_freectx_fn *freectx;
    OSSL_FUNC_keyexch_dupctx_fn *dupctx;
    OSSL_FUNC_keyexch_set_ctx_params_fn *set_ctx_params;
    OSSL_FUNC_keyexch_settable_ctx_params_fn *settable_ctx_params;
    OSSL_FUNC_keyexch_get_ctx_params_fn *get_ctx_params;
    OSSL_FUNC_keyexch_gettable_ctx_params_fn *gettable_ctx_params;
} /* EVP_KEYEXCH */;

struct evp_signature_st {
    int name_id;
    char *type_name;
    const char *description;
    OSSL_PROVIDER *prov;
    CRYPTO_REF_COUNT refcnt;

    OSSL_FUNC_signature_newctx_fn *newctx;
    OSSL_FUNC_signature_sign_init_fn *sign_init;
    OSSL_FUNC_signature_sign_fn *sign;
    OSSL_FUNC_signature_sign_message_init_fn *sign_message_init;
    OSSL_FUNC_signature_sign_message_update_fn *sign_message_update;
    OSSL_FUNC_signature_sign_message_final_fn *sign_message_final;
    OSSL_FUNC_signature_verify_init_fn *verify_init;
    OSSL_FUNC_signature_verify_fn *verify;
    OSSL_FUNC_signature_verify_message_init_fn *verify_message_init;
    OSSL_FUNC_signature_verify_message_update_fn *verify_message_update;
    OSSL_FUNC_signature_verify_message_final_fn *verify_message_final;
    OSSL_FUNC_signature_verify_recover_init_fn *verify_recover_init;
    OSSL_FUNC_signature_verify_recover_fn *verify_recover;
    OSSL_FUNC_signature_digest_sign_init_fn *digest_sign_init;
    OSSL_FUNC_signature_digest_sign_update_fn *digest_sign_update;
    OSSL_FUNC_signature_digest_sign_final_fn *digest_sign_final;
    OSSL_FUNC_signature_digest_sign_fn *digest_sign;
    OSSL_FUNC_signature_digest_verify_init_fn *digest_verify_init;
    OSSL_FUNC_signature_digest_verify_update_fn *digest_verify_update;
    OSSL_FUNC_signature_digest_verify_final_fn *digest_verify_final;
    OSSL_FUNC_signature_digest_verify_fn *digest_verify;
    OSSL_FUNC_signature_freectx_fn *freectx;
    OSSL_FUNC_signature_dupctx_fn *dupctx;
    OSSL_FUNC_signature_get_ctx_params_fn *get_ctx_params;
    OSSL_FUNC_signature_gettable_ctx_params_fn *gettable_ctx_params;
    OSSL_FUNC_signature_set_ctx_params_fn *set_ctx_params;
    OSSL_FUNC_signature_settable_ctx_params_fn *settable_ctx_params;
    OSSL_FUNC_signature_get_ctx_md_params_fn *get_ctx_md_params;
    OSSL_FUNC_signature_gettable_ctx_md_params_fn *gettable_ctx_md_params;
    OSSL_FUNC_signature_set_ctx_md_params_fn *set_ctx_md_params;
    OSSL_FUNC_signature_settable_ctx_md_params_fn *settable_ctx_md_params;

    /* Signature object checking */
    OSSL_FUNC_signature_query_key_types_fn *query_key_types;
} /* EVP_SIGNATURE */;

struct evp_skeymgmt_st {
    int name_id;
    char *type_name;
    const char *description;
    OSSL_PROVIDER *prov;
    CRYPTO_REF_COUNT refcnt;

    /* Import and export routines */
    OSSL_FUNC_skeymgmt_imp_settable_params_fn *imp_params;
    OSSL_FUNC_skeymgmt_import_fn *import;
    OSSL_FUNC_skeymgmt_export_fn *export;

    /* Key generation */
    OSSL_FUNC_skeymgmt_gen_settable_params_fn *gen_params;
    OSSL_FUNC_skeymgmt_generate_fn *generate;

    /* Key identifier */
    OSSL_FUNC_skeymgmt_get_key_id_fn *get_key_id;

    /* destructor */
    OSSL_FUNC_skeymgmt_free_fn *free;
} /* EVP_SKEYMGMT */;

struct evp_asym_cipher_st {
    int name_id;
    char *type_name;
    const char *description;
    OSSL_PROVIDER *prov;
    CRYPTO_REF_COUNT refcnt;

    OSSL_FUNC_asym_cipher_newctx_fn *newctx;
    OSSL_FUNC_asym_cipher_encrypt_init_fn *encrypt_init;
    OSSL_FUNC_asym_cipher_encrypt_fn *encrypt;
    OSSL_FUNC_asym_cipher_decrypt_init_fn *decrypt_init;
    OSSL_FUNC_asym_cipher_decrypt_fn *decrypt;
    OSSL_FUNC_asym_cipher_freectx_fn *freectx;
    OSSL_FUNC_asym_cipher_dupctx_fn *dupctx;
    OSSL_FUNC_asym_cipher_get_ctx_params_fn *get_ctx_params;
    OSSL_FUNC_asym_cipher_gettable_ctx_params_fn *gettable_ctx_params;
    OSSL_FUNC_asym_cipher_set_ctx_params_fn *set_ctx_params;
    OSSL_FUNC_asym_cipher_settable_ctx_params_fn *settable_ctx_params;
} /* EVP_ASYM_CIPHER */;

struct evp_kem_st {
    int name_id;
    char *type_name;
    const char *description;
    OSSL_PROVIDER *prov;
    CRYPTO_REF_COUNT refcnt;

    OSSL_FUNC_kem_newctx_fn *newctx;
    OSSL_FUNC_kem_encapsulate_init_fn *encapsulate_init;
    OSSL_FUNC_kem_encapsulate_fn *encapsulate;
    OSSL_FUNC_kem_decapsulate_init_fn *decapsulate_init;
    OSSL_FUNC_kem_decapsulate_fn *decapsulate;
    OSSL_FUNC_kem_freectx_fn *freectx;
    OSSL_FUNC_kem_dupctx_fn *dupctx;
    OSSL_FUNC_kem_get_ctx_params_fn *get_ctx_params;
    OSSL_FUNC_kem_gettable_ctx_params_fn *gettable_ctx_params;
    OSSL_FUNC_kem_set_ctx_params_fn *set_ctx_params;
    OSSL_FUNC_kem_settable_ctx_params_fn *settable_ctx_params;
    OSSL_FUNC_kem_auth_encapsulate_init_fn *auth_encapsulate_init;
    OSSL_FUNC_kem_auth_decapsulate_init_fn *auth_decapsulate_init;
} /* EVP_KEM */;

int PKCS5_v2_PBKDF2_keyivgen(EVP_CIPHER_CTX *ctx, const char *pass,
                             int passlen, ASN1_TYPE *param,
                             const EVP_CIPHER *c, const EVP_MD *md,
                             int en_de);
int PKCS5_v2_PBKDF2_keyivgen_ex(EVP_CIPHER_CTX *ctx, const char *pass,
                                int passlen, ASN1_TYPE *param,
                                const EVP_CIPHER *c, const EVP_MD *md,
                                int en_de, OSSL_LIB_CTX *libctx, const char *propq);

struct evp_Encode_Ctx_st {
    /* number saved in a partial encode/decode */
    int num;
    /*
     * The length is either the output line length (in input bytes) or the
     * shortest input line length that is ok.  Once decoding begins, the
     * length is adjusted up each time a longer line is decoded
     */
    int length;
    /* data to encode */
    unsigned char enc_data[80];
    /* number read on current line */
    int line_num;
    unsigned int flags;
};

typedef struct evp_pbe_st EVP_PBE_CTL;
DEFINE_STACK_OF(EVP_PBE_CTL)

int ossl_is_partially_overlapping(const void *ptr1, const void *ptr2, int len);

#include <openssl/types.h>
#include <openssl/core.h>

void *evp_generic_fetch(OSSL_LIB_CTX *ctx, int operation_id,
                        const char *name, const char *properties,
                        void *(*new_method)(int name_id,
                                            const OSSL_ALGORITHM *algodef,
                                            OSSL_PROVIDER *prov),
                        int (*up_ref_method)(void *),
                        void (*free_method)(void *));
void *evp_generic_fetch_from_prov(OSSL_PROVIDER *prov, int operation_id,
                                  const char *name, const char *properties,
                                  void *(*new_method)(int name_id,
                                                      const OSSL_ALGORITHM *algodef,
                                                      OSSL_PROVIDER *prov),
                                  int (*up_ref_method)(void *),
                                  void (*free_method)(void *));
void evp_generic_do_all_prefetched(OSSL_LIB_CTX *libctx, int operation_id,
                                   void (*user_fn)(void *method, void *arg),
                                   void *user_arg);
void evp_generic_do_all(OSSL_LIB_CTX *libctx, int operation_id,
                        void (*user_fn)(void *method, void *arg),
                        void *user_arg,
                        void *(*new_method)(int name_id,
                                            const OSSL_ALGORITHM *algodef,
                                            OSSL_PROVIDER *prov),
                        int (*up_ref_method)(void *),
                        void (*free_method)(void *));

/* Internal fetchers for method types that are to be combined with others */
EVP_KEYMGMT *evp_keymgmt_fetch_by_number(OSSL_LIB_CTX *ctx, int name_id,
                                         const char *properties);
EVP_SIGNATURE *evp_signature_fetch_from_prov(OSSL_PROVIDER *prov,
                                             const char *name,
                                             const char *properties);
EVP_ASYM_CIPHER *evp_asym_cipher_fetch_from_prov(OSSL_PROVIDER *prov,
                                                 const char *name,
                                                 const char *properties);
EVP_KEYEXCH *evp_keyexch_fetch_from_prov(OSSL_PROVIDER *prov,
                                         const char *name,
                                         const char *properties);
EVP_KEM *evp_kem_fetch_from_prov(OSSL_PROVIDER *prov,
                                 const char *name,
                                 const char *properties);
EVP_CIPHER *evp_cipher_fetch_from_prov(OSSL_PROVIDER *prov,
                                       const char *algorithm,
                                       const char *properties);
EVP_MD *evp_digest_fetch_from_prov(OSSL_PROVIDER *prov,
                                   const char *algorithm,
                                   const char *properties);
EVP_MAC *evp_mac_fetch_from_prov(OSSL_PROVIDER *prov,
                                 const char *algorithm,
                                 const char *properties);

/* Internal structure constructors for fetched methods */
EVP_MD *evp_md_new(void);
EVP_CIPHER *evp_cipher_new(void);

int evp_cipher_get_asn1_aead_params(EVP_CIPHER_CTX *c, ASN1_TYPE *type,
                                    evp_cipher_aead_asn1_params *asn1_params);
int evp_cipher_set_asn1_aead_params(EVP_CIPHER_CTX *c, ASN1_TYPE *type,
                                    evp_cipher_aead_asn1_params *asn1_params);

/* Helper functions to avoid duplicating code */

/*
 * These methods implement different ways to pass a params array to the
 * provider.  They will return one of these values:
 *
 * -2 if the method doesn't come from a provider
 *    (evp_do_param will return this to the called)
 * -1 if the provider doesn't offer the desired function
 *    (evp_do_param will raise an error and return 0)
 * or the return value from the desired function
 *    (evp_do_param will return it to the caller)
 */
int evp_do_ciph_getparams(const EVP_CIPHER *ciph, OSSL_PARAM params[]);
int evp_do_ciph_ctx_getparams(const EVP_CIPHER *ciph, void *provctx,
                              OSSL_PARAM params[]);
int evp_do_ciph_ctx_setparams(const EVP_CIPHER *ciph, void *provctx,
                              OSSL_PARAM params[]);
int evp_do_md_getparams(const EVP_MD *md, OSSL_PARAM params[]);
int evp_do_md_ctx_getparams(const EVP_MD *md, void *provctx,
                            OSSL_PARAM params[]);
int evp_do_md_ctx_setparams(const EVP_MD *md, void *provctx,
                            OSSL_PARAM params[]);

OSSL_PARAM *evp_pkey_to_param(EVP_PKEY *pkey, size_t *sz);

#define M_check_autoarg(ctx, arg, arglen, err) \
    if (ctx->pmeth->flags & EVP_PKEY_FLAG_AUTOARGLEN) {           \
        size_t pksize = (size_t)EVP_PKEY_get_size(ctx->pkey);         \
                                                                  \
        if (pksize == 0) {                                        \
            ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_KEY); /*ckerr_ignore*/ \
            return 0;                                             \
        }                                                         \
        if (arg == NULL) {                                        \
            *arglen = pksize;                                     \
            return 1;                                             \
        }                                                         \
        if (*arglen < pksize) {                                   \
            ERR_raise(ERR_LIB_EVP, EVP_R_BUFFER_TOO_SMALL); /*ckerr_ignore*/ \
            return 0;                                             \
        }                                                         \
    }

void evp_pkey_ctx_free_old_ops(EVP_PKEY_CTX *ctx);
void evp_cipher_free_int(EVP_CIPHER *md);
void evp_md_free_int(EVP_MD *md);

/* OSSL_PROVIDER * is only used to get the library context */
int evp_is_a(OSSL_PROVIDER *prov, int number,
             const char *legacy_name, const char *name);
int evp_names_do_all(OSSL_PROVIDER *prov, int number,
                     void (*fn)(const char *name, void *data),
                     void *data);
int evp_cipher_cache_constants(EVP_CIPHER *cipher);
