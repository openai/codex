/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/crypto.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/err.h>
#include <openssl/params.h>
#include <openssl/evp.h>
#include <openssl/proverr.h>
#include "internal/nelem.h"
#include "internal/sizes.h"
#include "prov/providercommon.h"
#include "prov/implementations.h"
#include "prov/securitycheck.h"
#include "prov/provider_ctx.h"
#include "prov/der_ecx.h"
#include "crypto/ecx.h"

#ifdef S390X_EC_ASM
# include "s390x_arch.h"

# define S390X_CAN_SIGN(edtype)                                                \
((OPENSSL_s390xcap_P.pcc[1] & S390X_CAPBIT(S390X_SCALAR_MULTIPLY_##edtype))    \
&& (OPENSSL_s390xcap_P.kdsa[0] & S390X_CAPBIT(S390X_EDDSA_SIGN_##edtype))      \
&& (OPENSSL_s390xcap_P.kdsa[0] & S390X_CAPBIT(S390X_EDDSA_VERIFY_##edtype)))

static int s390x_ed25519_digestsign(const ECX_KEY *edkey, unsigned char *sig,
                                    const unsigned char *tbs, size_t tbslen);
static int s390x_ed448_digestsign(const ECX_KEY *edkey, unsigned char *sig,
                                  const unsigned char *tbs, size_t tbslen);
static int s390x_ed25519_digestverify(const ECX_KEY *edkey,
                                      const unsigned char *sig,
                                      const unsigned char *tbs, size_t tbslen);
static int s390x_ed448_digestverify(const ECX_KEY *edkey,
                                    const unsigned char *sig,
                                    const unsigned char *tbs, size_t tbslen);

#endif /* S390X_EC_ASM */

enum ID_EdDSA_INSTANCE {
    ID_NOT_SET = 0,
    ID_Ed25519,
    ID_Ed25519ctx,
    ID_Ed25519ph,
    ID_Ed448,
    ID_Ed448ph
};

#define SN_Ed25519    "Ed25519"
#define SN_Ed25519ph  "Ed25519ph"
#define SN_Ed25519ctx "Ed25519ctx"
#define SN_Ed448      "Ed448"
#define SN_Ed448ph    "Ed448ph"

#define EDDSA_MAX_CONTEXT_STRING_LEN 255
#define EDDSA_PREHASH_OUTPUT_LEN 64

static OSSL_FUNC_signature_newctx_fn eddsa_newctx;
static OSSL_FUNC_signature_sign_message_init_fn ed25519_signverify_message_init;
static OSSL_FUNC_signature_sign_message_init_fn ed25519ph_signverify_message_init;
static OSSL_FUNC_signature_sign_message_init_fn ed25519ctx_signverify_message_init;
static OSSL_FUNC_signature_sign_message_init_fn ed448_signverify_message_init;
static OSSL_FUNC_signature_sign_message_init_fn ed448ph_signverify_message_init;
static OSSL_FUNC_signature_sign_fn ed25519_sign;
static OSSL_FUNC_signature_sign_fn ed448_sign;
static OSSL_FUNC_signature_verify_fn ed25519_verify;
static OSSL_FUNC_signature_verify_fn ed448_verify;
static OSSL_FUNC_signature_digest_sign_init_fn ed25519_digest_signverify_init;
static OSSL_FUNC_signature_digest_sign_init_fn ed448_digest_signverify_init;
static OSSL_FUNC_signature_digest_sign_fn ed25519_digest_sign;
static OSSL_FUNC_signature_digest_sign_fn ed448_digest_sign;
static OSSL_FUNC_signature_digest_verify_fn ed25519_digest_verify;
static OSSL_FUNC_signature_digest_verify_fn ed448_digest_verify;
static OSSL_FUNC_signature_freectx_fn eddsa_freectx;
static OSSL_FUNC_signature_dupctx_fn eddsa_dupctx;
static OSSL_FUNC_signature_query_key_types_fn ed25519_sigalg_query_key_types;
static OSSL_FUNC_signature_query_key_types_fn ed448_sigalg_query_key_types;
static OSSL_FUNC_signature_get_ctx_params_fn eddsa_get_ctx_params;
static OSSL_FUNC_signature_gettable_ctx_params_fn eddsa_gettable_ctx_params;
static OSSL_FUNC_signature_set_ctx_params_fn eddsa_set_ctx_params;
static OSSL_FUNC_signature_settable_ctx_params_fn eddsa_settable_ctx_params;
static OSSL_FUNC_signature_settable_ctx_params_fn eddsa_settable_variant_ctx_params;

/* there are five EdDSA instances:

         Ed25519
         Ed25519ph
         Ed25519ctx
         Ed448
         Ed448ph

   Quoting from RFC 8032, Section 5.1:

     For Ed25519, dom2(f,c) is the empty string.  The phflag value is
     irrelevant.  The context (if present at all) MUST be empty.  This
     causes the scheme to be one and the same with the Ed25519 scheme
     published earlier.

     For Ed25519ctx, phflag=0.  The context input SHOULD NOT be empty.

     For Ed25519ph, phflag=1 and PH is SHA512 instead.  That is, the input
     is hashed using SHA-512 before signing with Ed25519.

   Quoting from RFC 8032, Section 5.2:

     Ed448ph is the same but with PH being SHAKE256(x, 64) and phflag
     being 1, i.e., the input is hashed before signing with Ed448 with a
     hash constant modified.

     Value of context is set by signer and verifier (maximum of 255
     octets; the default is empty string) and has to match octet by octet
     for verification to be successful.

   Quoting from RFC 8032, Section 2:

     dom2(x, y)     The blank octet string when signing or verifying
                    Ed25519.  Otherwise, the octet string: "SigEd25519 no
                    Ed25519 collisions" || octet(x) || octet(OLEN(y)) ||
                    y, where x is in range 0-255 and y is an octet string
                    of at most 255 octets.  "SigEd25519 no Ed25519
                    collisions" is in ASCII (32 octets).

     dom4(x, y)     The octet string "SigEd448" || octet(x) ||
                    octet(OLEN(y)) || y, where x is in range 0-255 and y
                    is an octet string of at most 255 octets.  "SigEd448"
                    is in ASCII (8 octets).

   Note above that x is the pre-hash flag, and y is the context string.
*/

typedef struct {
    OSSL_LIB_CTX *libctx;
    ECX_KEY *key;

    /* The Algorithm Identifier of the signature algorithm */
    unsigned char aid_buf[OSSL_MAX_ALGORITHM_ID_SIZE];
    size_t  aid_len;

    /* id indicating the EdDSA instance */
    int instance_id;
    /* indicates that instance_id and associated flags are preset / hardcoded */
    unsigned int instance_id_preset_flag : 1;
    /* for ph instances, this indicates whether the caller is expected to prehash */
    unsigned int prehash_by_caller_flag : 1;

    unsigned int dom2_flag : 1;
    unsigned int prehash_flag : 1;

    /* indicates that a non-empty context string is required, as in Ed25519ctx */
    unsigned int context_string_flag : 1;

    unsigned char context_string[EDDSA_MAX_CONTEXT_STRING_LEN];
    size_t context_string_len;

} PROV_EDDSA_CTX;

static void *eddsa_newctx(void *provctx, const char *propq_unused)
{
    PROV_EDDSA_CTX *peddsactx;

    if (!ossl_prov_is_running())
        return NULL;

    peddsactx = OPENSSL_zalloc(sizeof(PROV_EDDSA_CTX));
    if (peddsactx == NULL)
        return NULL;

    peddsactx->libctx = PROV_LIBCTX_OF(provctx);

    return peddsactx;
}

static int eddsa_setup_instance(void *vpeddsactx, int instance_id,
                                unsigned int instance_id_preset,
                                unsigned int prehash_by_caller)
{
    PROV_EDDSA_CTX *peddsactx = (PROV_EDDSA_CTX *)vpeddsactx;

    switch (instance_id) {
    case ID_Ed25519:
        if (peddsactx->key->type != ECX_KEY_TYPE_ED25519)
            return 0;
        peddsactx->dom2_flag = 0;
        peddsactx->prehash_flag = 0;
        peddsactx->context_string_flag = 0;
        break;
    case ID_Ed25519ctx:
        if (peddsactx->key->type != ECX_KEY_TYPE_ED25519)
            return 0;
        peddsactx->dom2_flag = 1;
        peddsactx->prehash_flag = 0;
        peddsactx->context_string_flag = 1;
        break;
    case ID_Ed25519ph:
        if (peddsactx->key->type != ECX_KEY_TYPE_ED25519)
            return 0;
        peddsactx->dom2_flag = 1;
        peddsactx->prehash_flag = 1;
        peddsactx->context_string_flag = 0;
        break;
    case ID_Ed448:
        if (peddsactx->key->type != ECX_KEY_TYPE_ED448)
            return 0;
        peddsactx->prehash_flag = 0;
        peddsactx->context_string_flag = 0;
        break;
    case ID_Ed448ph:
        if (peddsactx->key->type != ECX_KEY_TYPE_ED448)
            return 0;
        peddsactx->prehash_flag = 1;
        peddsactx->context_string_flag = 0;
        break;
    default:
        /* we did not recognize the instance */
        return 0;
    }
    peddsactx->instance_id = instance_id;
    peddsactx->instance_id_preset_flag = instance_id_preset;
    peddsactx->prehash_by_caller_flag = prehash_by_caller;
    return 1;
}

static int eddsa_signverify_init(void *vpeddsactx, void *vedkey)
{
    PROV_EDDSA_CTX *peddsactx = (PROV_EDDSA_CTX *)vpeddsactx;
    ECX_KEY *edkey = (ECX_KEY *)vedkey;
    WPACKET pkt;
    int ret;
    unsigned char *aid = NULL;

    if (!ossl_prov_is_running())
        return 0;

    if (edkey == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_NO_KEY_SET);
        return 0;
    }

    if (!ossl_ecx_key_up_ref(edkey)) {
        ERR_raise(ERR_LIB_PROV, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    peddsactx->instance_id_preset_flag = 0;
    peddsactx->dom2_flag = 0;
    peddsactx->prehash_flag = 0;
    peddsactx->context_string_flag = 0;
    peddsactx->context_string_len = 0;

    peddsactx->key = edkey;

    /*
     * We do not care about DER writing errors.
     * All it really means is that for some reason, there's no
     * AlgorithmIdentifier to be had, but the operation itself is
     * still valid, just as long as it's not used to construct
     * anything that needs an AlgorithmIdentifier.
     */
    peddsactx->aid_len = 0;
    ret = WPACKET_init_der(&pkt, peddsactx->aid_buf, sizeof(peddsactx->aid_buf));
    switch (edkey->type) {
    case ECX_KEY_TYPE_ED25519:
        ret = ret && ossl_DER_w_algorithmIdentifier_ED25519(&pkt, -1, edkey);
        break;
    case ECX_KEY_TYPE_ED448:
        ret = ret && ossl_DER_w_algorithmIdentifier_ED448(&pkt, -1, edkey);
        break;
    default:
        /* Should never happen */
        ERR_raise(ERR_LIB_PROV, ERR_R_INTERNAL_ERROR);
        ossl_ecx_key_free(edkey);
        peddsactx->key = NULL;
        WPACKET_cleanup(&pkt);
        return 0;
    }
    if (ret && WPACKET_finish(&pkt)) {
        WPACKET_get_total_written(&pkt, &peddsactx->aid_len);
        aid = WPACKET_get_curr(&pkt);
    }
    WPACKET_cleanup(&pkt);
    if (aid != NULL && peddsactx->aid_len != 0)
        memmove(peddsactx->aid_buf, aid, peddsactx->aid_len);

    return 1;
}

static int ed25519_signverify_message_init(void *vpeddsactx, void *vedkey,
                                             const OSSL_PARAM params[])
{
    return eddsa_signverify_init(vpeddsactx, vedkey)
        && eddsa_setup_instance(vpeddsactx, ID_Ed25519, 1, 0)
        && eddsa_set_ctx_params(vpeddsactx, params);
}

static int ed25519ph_signverify_message_init(void *vpeddsactx, void *vedkey,
                                             const OSSL_PARAM params[])
{
    return eddsa_signverify_init(vpeddsactx, vedkey)
        && eddsa_setup_instance(vpeddsactx, ID_Ed25519ph, 1, 0)
        && eddsa_set_ctx_params(vpeddsactx, params);
}

static int ed25519ph_signverify_init(void *vpeddsactx, void *vedkey,
                                     const OSSL_PARAM params[])
{
    return eddsa_signverify_init(vpeddsactx, vedkey)
        && eddsa_setup_instance(vpeddsactx, ID_Ed25519ph, 1, 1)
        && eddsa_set_ctx_params(vpeddsactx, params);
}

/*
 * This supports using ED25519 with EVP_PKEY_{sign,verify}_init_ex() and
 * EVP_PKEY_{sign,verify}_init_ex2(), under the condition that the caller
 * explicitly sets the Ed25519ph instance (this is verified by ed25519_sign()
 * and ed25519_verify())
 */
static int ed25519_signverify_init(void *vpeddsactx, void *vedkey,
                                   const OSSL_PARAM params[])
{
    return eddsa_signverify_init(vpeddsactx, vedkey)
        && eddsa_setup_instance(vpeddsactx, ID_Ed25519, 0, 1)
        && eddsa_set_ctx_params(vpeddsactx, params);
}

static int ed25519ctx_signverify_message_init(void *vpeddsactx, void *vedkey,
                                             const OSSL_PARAM params[])
{
    return eddsa_signverify_init(vpeddsactx, vedkey)
        && eddsa_setup_instance(vpeddsactx, ID_Ed25519ctx, 1, 0)
        && eddsa_set_ctx_params(vpeddsactx, params);
}

static int ed448_signverify_message_init(void *vpeddsactx, void *vedkey,
                                         const OSSL_PARAM params[])
{
    return eddsa_signverify_init(vpeddsactx, vedkey)
        && eddsa_setup_instance(vpeddsactx, ID_Ed448, 1, 0)
        && eddsa_set_ctx_params(vpeddsactx, params);
}

static int ed448ph_signverify_message_init(void *vpeddsactx, void *vedkey,
                                           const OSSL_PARAM params[])
{
    return eddsa_signverify_init(vpeddsactx, vedkey)
        && eddsa_setup_instance(vpeddsactx, ID_Ed448ph, 1, 0)
        && eddsa_set_ctx_params(vpeddsactx, params);
}

static int ed448ph_signverify_init(void *vpeddsactx, void *vedkey,
                                   const OSSL_PARAM params[])
{
    return eddsa_signverify_init(vpeddsactx, vedkey)
        && eddsa_setup_instance(vpeddsactx, ID_Ed448ph, 1, 1)
        && eddsa_set_ctx_params(vpeddsactx, params);
}

/*
 * This supports using ED448 with EVP_PKEY_{sign,verify}_init_ex() and
 * EVP_PKEY_{sign,verify}_init_ex2(), under the condition that the caller
 * explicitly sets the Ed448ph instance (this is verified by ed448_sign()
 * and ed448_verify())
 */
static int ed448_signverify_init(void *vpeddsactx, void *vedkey,
                                   const OSSL_PARAM params[])
{
    return eddsa_signverify_init(vpeddsactx, vedkey)
        && eddsa_setup_instance(vpeddsactx, ID_Ed448, 0, 1)
        && eddsa_set_ctx_params(vpeddsactx, params);
}

/*
 * This is used directly for OSSL_FUNC_SIGNATURE_SIGN and indirectly
 * for OSSL_FUNC_SIGNATURE_DIGEST_SIGN
 */
static int ed25519_sign(void *vpeddsactx,
                        unsigned char *sigret, size_t *siglen, size_t sigsize,
                        const unsigned char *tbs, size_t tbslen)
{
    PROV_EDDSA_CTX *peddsactx = (PROV_EDDSA_CTX *)vpeddsactx;
    const ECX_KEY *edkey = peddsactx->key;
    uint8_t md[EVP_MAX_MD_SIZE];
    size_t mdlen;

    if (!ossl_prov_is_running())
        return 0;

    if (sigret == NULL) {
        *siglen = ED25519_SIGSIZE;
        return 1;
    }
    if (sigsize < ED25519_SIGSIZE) {
        ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
        return 0;
    }
    if (edkey->privkey == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_NOT_A_PRIVATE_KEY);
        return 0;
    }
#ifdef S390X_EC_ASM
    /*
     * s390x_ed25519_digestsign() does not yet support dom2 or context-strings.
     * fall back to non-accelerated sign if those options are set, or pre-hasing
     * is provided.
     */
    if (S390X_CAN_SIGN(ED25519)
            && !peddsactx->dom2_flag
            && !peddsactx->context_string_flag
            && peddsactx->context_string_len == 0
            && !peddsactx->prehash_flag
            && !peddsactx->prehash_by_caller_flag) {
        if (s390x_ed25519_digestsign(edkey, sigret, tbs, tbslen) == 0) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SIGN);
            return 0;
        }
        *siglen = ED25519_SIGSIZE;
        return 1;
    }
#endif /* S390X_EC_ASM */

    if (peddsactx->prehash_flag) {
        if (!peddsactx->prehash_by_caller_flag) {
            if (!EVP_Q_digest(peddsactx->libctx, SN_sha512, NULL,
                              tbs, tbslen, md, &mdlen)
                || mdlen != EDDSA_PREHASH_OUTPUT_LEN) {
                ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_PREHASHED_DIGEST_LENGTH);
                return 0;
            }
            tbs = md;
            tbslen = mdlen;
        } else if (tbslen != EDDSA_PREHASH_OUTPUT_LEN) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_DIGEST_LENGTH);
            return 0;
        }
    } else if (peddsactx->prehash_by_caller_flag) {
        /* The caller is supposed to set up a ph instance! */
        ERR_raise(ERR_LIB_PROV,
                  PROV_R_INVALID_EDDSA_INSTANCE_FOR_ATTEMPTED_OPERATION);
        return 0;
    }

    if (ossl_ed25519_sign(sigret, tbs, tbslen, edkey->pubkey, edkey->privkey,
            peddsactx->dom2_flag, peddsactx->prehash_flag, peddsactx->context_string_flag,
            peddsactx->context_string, peddsactx->context_string_len,
            peddsactx->libctx, NULL) == 0) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SIGN);
        return 0;
    }
    *siglen = ED25519_SIGSIZE;
    return 1;
}

/* EVP_Q_digest() does not allow variable output length for XOFs,
   so we use this function */
static int ed448_shake256(OSSL_LIB_CTX *libctx,
                          const char *propq,
                          const uint8_t *in, size_t inlen,
                          uint8_t *out, size_t outlen)
{
    int ret = 0;
    EVP_MD_CTX *hash_ctx = EVP_MD_CTX_new();
    EVP_MD *shake256 = EVP_MD_fetch(libctx, SN_shake256, propq);

    if (hash_ctx == NULL || shake256 == NULL)
        goto err;

    if (!EVP_DigestInit_ex(hash_ctx, shake256, NULL)
            || !EVP_DigestUpdate(hash_ctx, in, inlen)
            || !EVP_DigestFinalXOF(hash_ctx, out, outlen))
        goto err;

    ret = 1;

 err:
    EVP_MD_CTX_free(hash_ctx);
    EVP_MD_free(shake256);
    return ret;
}

/*
 * This is used directly for OSSL_FUNC_SIGNATURE_SIGN and indirectly
 * for OSSL_FUNC_SIGNATURE_DIGEST_SIGN
 */
static int ed448_sign(void *vpeddsactx,
                      unsigned char *sigret, size_t *siglen, size_t sigsize,
                      const unsigned char *tbs, size_t tbslen)
{
    PROV_EDDSA_CTX *peddsactx = (PROV_EDDSA_CTX *)vpeddsactx;
    const ECX_KEY *edkey = peddsactx->key;
    uint8_t md[EDDSA_PREHASH_OUTPUT_LEN];
    size_t mdlen = sizeof(md);

    if (!ossl_prov_is_running())
        return 0;

    if (sigret == NULL) {
        *siglen = ED448_SIGSIZE;
        return 1;
    }
    if (sigsize < ED448_SIGSIZE) {
        ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
        return 0;
    }
    if (edkey->privkey == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_NOT_A_PRIVATE_KEY);
        return 0;
    }
#ifdef S390X_EC_ASM
    /*
     * s390x_ed448_digestsign() does not yet support context-strings or
     * pre-hashing. Fall back to non-accelerated sign if a context-string or
     * pre-hasing is provided.
     */
    if (S390X_CAN_SIGN(ED448)
            && peddsactx->context_string_len == 0
            && !peddsactx->prehash_flag
            && !peddsactx->prehash_by_caller_flag) {
        if (s390x_ed448_digestsign(edkey, sigret, tbs, tbslen) == 0) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SIGN);
            return 0;
        }
        *siglen = ED448_SIGSIZE;
        return 1;
    }
#endif /* S390X_EC_ASM */

    if (peddsactx->prehash_flag) {
        if (!peddsactx->prehash_by_caller_flag) {
            if (!ed448_shake256(peddsactx->libctx, NULL, tbs, tbslen, md, mdlen))
                return 0;
            tbs = md;
            tbslen = mdlen;
        } else if (tbslen != EDDSA_PREHASH_OUTPUT_LEN) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_DIGEST_LENGTH);
            return 0;
        }
    } else if (peddsactx->prehash_by_caller_flag) {
        /* The caller is supposed to set up a ph instance! */
        ERR_raise(ERR_LIB_PROV,
                  PROV_R_INVALID_EDDSA_INSTANCE_FOR_ATTEMPTED_OPERATION);
        return 0;
    }

    if (ossl_ed448_sign(peddsactx->libctx, sigret, tbs, tbslen,
                        edkey->pubkey, edkey->privkey,
                        peddsactx->context_string, peddsactx->context_string_len,
                        peddsactx->prehash_flag, edkey->propq) == 0) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SIGN);
        return 0;
    }
    *siglen = ED448_SIGSIZE;
    return 1;
}

/*
 * This is used directly for OSSL_FUNC_SIGNATURE_VERIFY and indirectly
 * for OSSL_FUNC_SIGNATURE_DIGEST_VERIFY
 */
static int ed25519_verify(void *vpeddsactx,
                          const unsigned char *sig, size_t siglen,
                          const unsigned char *tbs, size_t tbslen)
{
    PROV_EDDSA_CTX *peddsactx = (PROV_EDDSA_CTX *)vpeddsactx;
    const ECX_KEY *edkey = peddsactx->key;
    uint8_t md[EVP_MAX_MD_SIZE];
    size_t mdlen;

    if (!ossl_prov_is_running() || siglen != ED25519_SIGSIZE)
        return 0;

#ifdef S390X_EC_ASM
    /*
     * s390x_ed25519_digestverify() does not yet support dom2 or context-strings.
     * fall back to non-accelerated verify if those options are set, or
     * pre-hasing is provided.
     */
    if (S390X_CAN_SIGN(ED25519)
            && !peddsactx->dom2_flag
            && !peddsactx->context_string_flag
            && peddsactx->context_string_len == 0
            && !peddsactx->prehash_flag
            && !peddsactx->prehash_by_caller_flag)
        return s390x_ed25519_digestverify(edkey, sig, tbs, tbslen);
#endif /* S390X_EC_ASM */

    if (peddsactx->prehash_flag) {
        if (!peddsactx->prehash_by_caller_flag) {
            if (!EVP_Q_digest(peddsactx->libctx, SN_sha512, NULL,
                              tbs, tbslen, md, &mdlen)
                || mdlen != EDDSA_PREHASH_OUTPUT_LEN) {
                ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_PREHASHED_DIGEST_LENGTH);
                return 0;
            }
            tbs = md;
            tbslen = mdlen;
        } else if (tbslen != EDDSA_PREHASH_OUTPUT_LEN) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_DIGEST_LENGTH);
            return 0;
        }
    } else if (peddsactx->prehash_by_caller_flag) {
        /* The caller is supposed to set up a ph instance! */
        ERR_raise(ERR_LIB_PROV,
                  PROV_R_INVALID_EDDSA_INSTANCE_FOR_ATTEMPTED_OPERATION);
        return 0;
    }

    return ossl_ed25519_verify(tbs, tbslen, sig, edkey->pubkey,
                               peddsactx->dom2_flag, peddsactx->prehash_flag, peddsactx->context_string_flag,
                               peddsactx->context_string, peddsactx->context_string_len,
                               peddsactx->libctx, edkey->propq);
}

/*
 * This is used directly for OSSL_FUNC_SIGNATURE_VERIFY and indirectly
 * for OSSL_FUNC_SIGNATURE_DIGEST_VERIFY
 */
static int ed448_verify(void *vpeddsactx,
                        const unsigned char *sig, size_t siglen,
                        const unsigned char *tbs, size_t tbslen)
{
    PROV_EDDSA_CTX *peddsactx = (PROV_EDDSA_CTX *)vpeddsactx;
    const ECX_KEY *edkey = peddsactx->key;
    uint8_t md[EDDSA_PREHASH_OUTPUT_LEN];
    size_t mdlen = sizeof(md);

    if (!ossl_prov_is_running() || siglen != ED448_SIGSIZE)
        return 0;

#ifdef S390X_EC_ASM
    /*
     * s390x_ed448_digestverify() does not yet support context-strings or
     * pre-hashing. Fall back to non-accelerated verify if a context-string or
     * pre-hasing is provided.
     */
    if (S390X_CAN_SIGN(ED448)
            && peddsactx->context_string_len == 0
            && !peddsactx->prehash_flag
            && !peddsactx->prehash_by_caller_flag)
        return s390x_ed448_digestverify(edkey, sig, tbs, tbslen);
#endif /* S390X_EC_ASM */

    if (peddsactx->prehash_flag) {
        if (!peddsactx->prehash_by_caller_flag) {
            if (!ed448_shake256(peddsactx->libctx, NULL, tbs, tbslen, md, mdlen))
                return 0;
            tbs = md;
            tbslen = mdlen;
        } else if (tbslen != EDDSA_PREHASH_OUTPUT_LEN) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_DIGEST_LENGTH);
            return 0;
        }
    } else if (peddsactx->prehash_by_caller_flag) {
        /* The caller is supposed to set up a ph instance! */
        ERR_raise(ERR_LIB_PROV,
                  PROV_R_INVALID_EDDSA_INSTANCE_FOR_ATTEMPTED_OPERATION);
        return 0;
    }

    return ossl_ed448_verify(peddsactx->libctx, tbs, tbslen, sig, edkey->pubkey,
                             peddsactx->context_string, peddsactx->context_string_len,
                             peddsactx->prehash_flag, edkey->propq);
}

/* All digest_{sign,verify} are simple wrappers around the functions above */

static int ed25519_digest_signverify_init(void *vpeddsactx, const char *mdname,
                                          void *vedkey,
                                          const OSSL_PARAM params[])
{
    PROV_EDDSA_CTX *peddsactx = (PROV_EDDSA_CTX *)vpeddsactx;

    if (mdname != NULL && mdname[0] != '\0') {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_DIGEST,
                        "Explicit digest not allowed with EdDSA operations");
        return 0;
    }

    if (vedkey == NULL && peddsactx->key != NULL)
        return eddsa_set_ctx_params(peddsactx, params);

    return eddsa_signverify_init(vpeddsactx, vedkey)
        && eddsa_setup_instance(vpeddsactx, ID_Ed25519, 0, 0)
        && eddsa_set_ctx_params(vpeddsactx, params);
}

static int ed25519_digest_sign(void *vpeddsactx,
                               unsigned char *sigret, size_t *siglen, size_t sigsize,
                               const unsigned char *tbs, size_t tbslen)
{
    return ed25519_sign(vpeddsactx, sigret, siglen, sigsize, tbs, tbslen);
}

static int ed25519_digest_verify(void *vpeddsactx,
                                 const unsigned char *sigret, size_t siglen,
                                 const unsigned char *tbs, size_t tbslen)
{
    return ed25519_verify(vpeddsactx, sigret, siglen, tbs, tbslen);
}

static int ed448_digest_signverify_init(void *vpeddsactx, const char *mdname,
                                        void *vedkey,
                                        const OSSL_PARAM params[])
{
    PROV_EDDSA_CTX *peddsactx = (PROV_EDDSA_CTX *)vpeddsactx;

    if (mdname != NULL && mdname[0] != '\0') {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_DIGEST,
                        "Explicit digest not allowed with EdDSA operations");
        return 0;
    }

    if (vedkey == NULL && peddsactx->key != NULL)
        return eddsa_set_ctx_params(peddsactx, params);

    return eddsa_signverify_init(vpeddsactx, vedkey)
        && eddsa_setup_instance(vpeddsactx, ID_Ed448, 0, 0)
        && eddsa_set_ctx_params(vpeddsactx, params);
}

static int ed448_digest_sign(void *vpeddsactx,
                             unsigned char *sigret, size_t *siglen, size_t sigsize,
                             const unsigned char *tbs, size_t tbslen)
{
    return ed448_sign(vpeddsactx, sigret, siglen, sigsize, tbs, tbslen);
}

static int ed448_digest_verify(void *vpeddsactx,
                               const unsigned char *sigret, size_t siglen,
                               const unsigned char *tbs, size_t tbslen)
{
    return ed448_verify(vpeddsactx, sigret, siglen, tbs, tbslen);
}

static void eddsa_freectx(void *vpeddsactx)
{
    PROV_EDDSA_CTX *peddsactx = (PROV_EDDSA_CTX *)vpeddsactx;

    ossl_ecx_key_free(peddsactx->key);

    OPENSSL_free(peddsactx);
}

static void *eddsa_dupctx(void *vpeddsactx)
{
    PROV_EDDSA_CTX *srcctx = (PROV_EDDSA_CTX *)vpeddsactx;
    PROV_EDDSA_CTX *dstctx;

    if (!ossl_prov_is_running())
        return NULL;

    dstctx = OPENSSL_zalloc(sizeof(*srcctx));
    if (dstctx == NULL)
        return NULL;

    *dstctx = *srcctx;
    dstctx->key = NULL;

    if (srcctx->key != NULL && !ossl_ecx_key_up_ref(srcctx->key)) {
        ERR_raise(ERR_LIB_PROV, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    dstctx->key = srcctx->key;

    return dstctx;
 err:
    eddsa_freectx(dstctx);
    return NULL;
}

static const char **ed25519_sigalg_query_key_types(void)
{
    static const char *keytypes[] = { "ED25519", NULL };

    return keytypes;
}

static const char **ed448_sigalg_query_key_types(void)
{
    static const char *keytypes[] = { "ED448", NULL };

    return keytypes;
}



static int eddsa_get_ctx_params(void *vpeddsactx, OSSL_PARAM *params)
{
    PROV_EDDSA_CTX *peddsactx = (PROV_EDDSA_CTX *)vpeddsactx;
    OSSL_PARAM *p;

    if (peddsactx == NULL)
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_SIGNATURE_PARAM_ALGORITHM_ID);
    if (p != NULL
        && !OSSL_PARAM_set_octet_string(p,
                                        peddsactx->aid_len == 0 ? NULL : peddsactx->aid_buf,
                                        peddsactx->aid_len))
        return 0;

    return 1;
}

static const OSSL_PARAM known_gettable_ctx_params[] = {
    OSSL_PARAM_octet_string(OSSL_SIGNATURE_PARAM_ALGORITHM_ID, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_SIGNATURE_PARAM_INSTANCE, NULL, 0),
    OSSL_PARAM_octet_string(OSSL_SIGNATURE_PARAM_CONTEXT_STRING, NULL, 0),
    OSSL_PARAM_END
};

static const OSSL_PARAM *eddsa_gettable_ctx_params(ossl_unused void *vpeddsactx,
                                                   ossl_unused void *provctx)
{
    return known_gettable_ctx_params;
}

static int eddsa_set_ctx_params(void *vpeddsactx, const OSSL_PARAM params[])
{
    PROV_EDDSA_CTX *peddsactx = (PROV_EDDSA_CTX *)vpeddsactx;
    const OSSL_PARAM *p;

    if (peddsactx == NULL)
        return 0;
    if (ossl_param_is_empty(params))
        return 1;

    p = OSSL_PARAM_locate_const(params, OSSL_SIGNATURE_PARAM_INSTANCE);
    if (p != NULL) {
        char instance_name[OSSL_MAX_NAME_SIZE] = "";
        char *pinstance_name = instance_name;

        if (peddsactx->instance_id_preset_flag) {
            /* When the instance is preset, the caller must no try to set it */
            ERR_raise_data(ERR_LIB_PROV, PROV_R_NO_INSTANCE_ALLOWED,
                           "the EdDSA instance is preset, you may not try to specify it",
                           NULL);
            return 0;
        }

        if (!OSSL_PARAM_get_utf8_string(p, &pinstance_name, sizeof(instance_name)))
            return 0;

        /*
         * When setting the new instance, we're careful not to change the
         * prehash_by_caller flag, as that's always preset by the init
         * functions.  The sign functions will determine if the instance
         * matches this flag.
         */
        if (OPENSSL_strcasecmp(pinstance_name, SN_Ed25519) == 0) {
            eddsa_setup_instance(peddsactx, ID_Ed25519, 0,
                                 peddsactx->prehash_by_caller_flag);
        } else if (OPENSSL_strcasecmp(pinstance_name, SN_Ed25519ctx) == 0) {
            eddsa_setup_instance(peddsactx, ID_Ed25519ctx, 0,
                                 peddsactx->prehash_by_caller_flag);
        } else if (OPENSSL_strcasecmp(pinstance_name, SN_Ed25519ph) == 0) {
            eddsa_setup_instance(peddsactx, ID_Ed25519ph, 0,
                                 peddsactx->prehash_by_caller_flag);
        } else if (OPENSSL_strcasecmp(pinstance_name, SN_Ed448) == 0) {
            eddsa_setup_instance(peddsactx, ID_Ed448, 0,
                                 peddsactx->prehash_by_caller_flag);
        } else if (OPENSSL_strcasecmp(pinstance_name, SN_Ed448ph) == 0) {
            eddsa_setup_instance(peddsactx, ID_Ed448ph, 0,
                                 peddsactx->prehash_by_caller_flag);
        } else {
            /* we did not recognize the instance */
            return 0;
        }

    }

    p = OSSL_PARAM_locate_const(params, OSSL_SIGNATURE_PARAM_CONTEXT_STRING);
    if (p != NULL) {
        void *vp_context_string = peddsactx->context_string;

        if (!OSSL_PARAM_get_octet_string(p, &vp_context_string, sizeof(peddsactx->context_string), &(peddsactx->context_string_len))) {
            peddsactx->context_string_len = 0;
            return 0;
        }
    }

    return 1;
}

static const OSSL_PARAM settable_ctx_params[] = {
    OSSL_PARAM_utf8_string(OSSL_SIGNATURE_PARAM_INSTANCE, NULL, 0),
    OSSL_PARAM_octet_string(OSSL_SIGNATURE_PARAM_CONTEXT_STRING, NULL, 0),
    OSSL_PARAM_END
};

static const OSSL_PARAM *eddsa_settable_ctx_params(ossl_unused void *vpeddsactx,
                                                   ossl_unused void *provctx)
{
    return settable_ctx_params;
}

static const OSSL_PARAM settable_variant_ctx_params[] = {
    OSSL_PARAM_octet_string(OSSL_SIGNATURE_PARAM_CONTEXT_STRING, NULL, 0),
    OSSL_PARAM_END
};

static const OSSL_PARAM *
eddsa_settable_variant_ctx_params(ossl_unused void *vpeddsactx,
                                  ossl_unused void *provctx)
{
    return settable_variant_ctx_params;
}

/*
 * Ed25519 can be used with:
 * - EVP_PKEY_sign_init_ex2()   [ instance and prehash assumed done by caller ]
 * - EVP_PKEY_verify_init_ex2() [ instance and prehash assumed done by caller ]
 * - EVP_PKEY_sign_message_init()
 * - EVP_PKEY_verify_message_init()
 * - EVP_DigestSignInit_ex()
 * - EVP_DigestVerifyInit_ex()
 * Ed25519ph can be used with:
 * - EVP_PKEY_sign_init_ex2()   [ prehash assumed done by caller ]
 * - EVP_PKEY_verify_init_ex2() [ prehash assumed done by caller ]
 * - EVP_PKEY_sign_message_init()
 * - EVP_PKEY_verify_message_init()
 * Ed25519ctx can be used with:
 * - EVP_PKEY_sign_message_init()
 * - EVP_PKEY_verify_message_init()
 * Ed448 can be used with:
 * - EVP_PKEY_sign_init_ex2()   [ instance and prehash assumed done by caller ]
 * - EVP_PKEY_verify_init_ex2() [ instance and prehash assumed done by caller ]
 * - EVP_PKEY_sign_message_init()
 * - EVP_PKEY_verify_message_init()
 * - EVP_DigestSignInit_ex()
 * - EVP_DigestVerifyInit_ex()
 * Ed448ph can be used with:
 * - EVP_PKEY_sign_init_ex2()   [ prehash assumed done by caller ]
 * - EVP_PKEY_verify_init_ex2() [ prehash assumed done by caller ]
 * - EVP_PKEY_sign_message_init()
 * - EVP_PKEY_verify_message_init()
 */

#define ed25519_DISPATCH_END                                            \
    { OSSL_FUNC_SIGNATURE_SIGN_INIT,                                    \
        (void (*)(void))ed25519_signverify_init },                      \
    { OSSL_FUNC_SIGNATURE_VERIFY_INIT,                                  \
        (void (*)(void))ed25519_signverify_init },                      \
    { OSSL_FUNC_SIGNATURE_DIGEST_SIGN_INIT,                             \
        (void (*)(void))ed25519_digest_signverify_init },               \
    { OSSL_FUNC_SIGNATURE_DIGEST_SIGN,                                  \
        (void (*)(void))ed25519_digest_sign },                          \
    { OSSL_FUNC_SIGNATURE_DIGEST_VERIFY_INIT,                           \
        (void (*)(void))ed25519_digest_signverify_init },               \
    { OSSL_FUNC_SIGNATURE_DIGEST_VERIFY,                                \
        (void (*)(void))ed25519_digest_verify },                        \
    { OSSL_FUNC_SIGNATURE_GET_CTX_PARAMS,                               \
        (void (*)(void))eddsa_get_ctx_params },                         \
    { OSSL_FUNC_SIGNATURE_GETTABLE_CTX_PARAMS,                          \
        (void (*)(void))eddsa_gettable_ctx_params },                    \
    { OSSL_FUNC_SIGNATURE_SET_CTX_PARAMS,                               \
        (void (*)(void))eddsa_set_ctx_params },                         \
    { OSSL_FUNC_SIGNATURE_SETTABLE_CTX_PARAMS,                          \
        (void (*)(void))eddsa_settable_ctx_params },                    \
    OSSL_DISPATCH_END

#define eddsa_variant_DISPATCH_END(v)                                   \
    { OSSL_FUNC_SIGNATURE_SIGN_INIT,                                    \
        (void (*)(void))v##_signverify_message_init },                  \
    { OSSL_FUNC_SIGNATURE_VERIFY_INIT,                                  \
        (void (*)(void))v##_signverify_message_init },                  \
    { OSSL_FUNC_SIGNATURE_GET_CTX_PARAMS,                               \
        (void (*)(void))eddsa_get_ctx_params },                         \
    { OSSL_FUNC_SIGNATURE_GETTABLE_CTX_PARAMS,                          \
        (void (*)(void))eddsa_gettable_ctx_params },                    \
    { OSSL_FUNC_SIGNATURE_SET_CTX_PARAMS,                               \
        (void (*)(void))eddsa_set_ctx_params },                         \
    { OSSL_FUNC_SIGNATURE_SETTABLE_CTX_PARAMS,                          \
        (void (*)(void))eddsa_settable_variant_ctx_params },            \
    OSSL_DISPATCH_END

#define ed25519ph_DISPATCH_END                                          \
    { OSSL_FUNC_SIGNATURE_SIGN_INIT,                                    \
        (void (*)(void))ed25519ph_signverify_init },                    \
    { OSSL_FUNC_SIGNATURE_VERIFY_INIT,                                  \
        (void (*)(void))ed25519ph_signverify_init },                    \
    eddsa_variant_DISPATCH_END(ed25519ph)

#define ed25519ctx_DISPATCH_END eddsa_variant_DISPATCH_END(ed25519ctx)

#define ed448_DISPATCH_END                                              \
    { OSSL_FUNC_SIGNATURE_SIGN_INIT,                                    \
        (void (*)(void))ed448_signverify_init },                        \
    { OSSL_FUNC_SIGNATURE_VERIFY_INIT,                                  \
        (void (*)(void))ed448_signverify_init },                        \
    { OSSL_FUNC_SIGNATURE_DIGEST_SIGN_INIT,                             \
        (void (*)(void))ed448_digest_signverify_init },                 \
    { OSSL_FUNC_SIGNATURE_DIGEST_SIGN,                                  \
        (void (*)(void))ed448_digest_sign },                            \
    { OSSL_FUNC_SIGNATURE_DIGEST_VERIFY_INIT,                           \
        (void (*)(void))ed448_digest_signverify_init },                 \
    { OSSL_FUNC_SIGNATURE_DIGEST_VERIFY,                                \
        (void (*)(void))ed448_digest_verify },                          \
    { OSSL_FUNC_SIGNATURE_GET_CTX_PARAMS,                               \
        (void (*)(void))eddsa_get_ctx_params },                         \
    { OSSL_FUNC_SIGNATURE_GETTABLE_CTX_PARAMS,                          \
        (void (*)(void))eddsa_gettable_ctx_params },                    \
    { OSSL_FUNC_SIGNATURE_SET_CTX_PARAMS,                               \
        (void (*)(void))eddsa_set_ctx_params },                         \
    { OSSL_FUNC_SIGNATURE_SETTABLE_CTX_PARAMS,                          \
        (void (*)(void))eddsa_settable_ctx_params },                    \
    OSSL_DISPATCH_END

#define ed448ph_DISPATCH_END                                            \
    { OSSL_FUNC_SIGNATURE_SIGN_INIT,                                    \
        (void (*)(void))ed448ph_signverify_init },                      \
    { OSSL_FUNC_SIGNATURE_VERIFY_INIT,                                  \
        (void (*)(void))ed448ph_signverify_init },                      \
    eddsa_variant_DISPATCH_END(ed448ph)

/* vn = variant name, bn = base name */
#define IMPL_EDDSA_DISPATCH(vn,bn)                                      \
    const OSSL_DISPATCH ossl_##vn##_signature_functions[] = {           \
        { OSSL_FUNC_SIGNATURE_NEWCTX, (void (*)(void))eddsa_newctx },   \
        { OSSL_FUNC_SIGNATURE_SIGN_MESSAGE_INIT,                        \
          (void (*)(void))vn##_signverify_message_init },               \
        { OSSL_FUNC_SIGNATURE_SIGN,                                     \
          (void (*)(void))bn##_sign },                                  \
        { OSSL_FUNC_SIGNATURE_VERIFY_MESSAGE_INIT,                      \
          (void (*)(void))vn##_signverify_message_init },               \
        { OSSL_FUNC_SIGNATURE_VERIFY,                                   \
          (void (*)(void))bn##_verify },                                \
        { OSSL_FUNC_SIGNATURE_FREECTX, (void (*)(void))eddsa_freectx }, \
        { OSSL_FUNC_SIGNATURE_DUPCTX, (void (*)(void))eddsa_dupctx },   \
        { OSSL_FUNC_SIGNATURE_QUERY_KEY_TYPES,                          \
          (void (*)(void))bn##_sigalg_query_key_types },                \
        vn##_DISPATCH_END                                               \
    }

IMPL_EDDSA_DISPATCH(ed25519,ed25519);
IMPL_EDDSA_DISPATCH(ed25519ph,ed25519);
IMPL_EDDSA_DISPATCH(ed25519ctx,ed25519);
IMPL_EDDSA_DISPATCH(ed448,ed448);
IMPL_EDDSA_DISPATCH(ed448ph,ed448);

#ifdef S390X_EC_ASM

static int s390x_ed25519_digestsign(const ECX_KEY *edkey, unsigned char *sig,
                                    const unsigned char *tbs, size_t tbslen)
{
    int rc;
    union {
        struct {
            unsigned char sig[64];
            unsigned char priv[32];
        } ed25519;
        unsigned long long buff[512];
    } param;

    memset(&param, 0, sizeof(param));
    memcpy(param.ed25519.priv, edkey->privkey, sizeof(param.ed25519.priv));

    rc = s390x_kdsa(S390X_EDDSA_SIGN_ED25519, &param.ed25519, tbs, tbslen);
    OPENSSL_cleanse(param.ed25519.priv, sizeof(param.ed25519.priv));
    if (rc != 0)
        return 0;

    s390x_flip_endian32(sig, param.ed25519.sig);
    s390x_flip_endian32(sig + 32, param.ed25519.sig + 32);
    return 1;
}

static int s390x_ed448_digestsign(const ECX_KEY *edkey, unsigned char *sig,
                                  const unsigned char *tbs, size_t tbslen)
{
    int rc;
    union {
        struct {
            unsigned char sig[128];
            unsigned char priv[64];
        } ed448;
        unsigned long long buff[512];
    } param;

    memset(&param, 0, sizeof(param));
    memcpy(param.ed448.priv + 64 - 57, edkey->privkey, 57);

    rc = s390x_kdsa(S390X_EDDSA_SIGN_ED448, &param.ed448, tbs, tbslen);
    OPENSSL_cleanse(param.ed448.priv, sizeof(param.ed448.priv));
    if (rc != 0)
        return 0;

    s390x_flip_endian64(param.ed448.sig, param.ed448.sig);
    s390x_flip_endian64(param.ed448.sig + 64, param.ed448.sig + 64);
    memcpy(sig, param.ed448.sig, 57);
    memcpy(sig + 57, param.ed448.sig + 64, 57);
    return 1;
}

static int s390x_ed25519_digestverify(const ECX_KEY *edkey,
                                      const unsigned char *sig,
                                      const unsigned char *tbs, size_t tbslen)
{
    union {
        struct {
            unsigned char sig[64];
            unsigned char pub[32];
        } ed25519;
        unsigned long long buff[512];
    } param;

    memset(&param, 0, sizeof(param));
    s390x_flip_endian32(param.ed25519.sig, sig);
    s390x_flip_endian32(param.ed25519.sig + 32, sig + 32);
    s390x_flip_endian32(param.ed25519.pub, edkey->pubkey);

    return s390x_kdsa(S390X_EDDSA_VERIFY_ED25519,
                      &param.ed25519, tbs, tbslen) == 0 ? 1 : 0;
}

static int s390x_ed448_digestverify(const ECX_KEY *edkey,
                                    const unsigned char *sig,
                                    const unsigned char *tbs,
                                    size_t tbslen)
{
    union {
        struct {
            unsigned char sig[128];
            unsigned char pub[64];
        } ed448;
        unsigned long long buff[512];
    } param;

    memset(&param, 0, sizeof(param));
    memcpy(param.ed448.sig, sig, 57);
    s390x_flip_endian64(param.ed448.sig, param.ed448.sig);
    memcpy(param.ed448.sig + 64, sig + 57, 57);
    s390x_flip_endian64(param.ed448.sig + 64, param.ed448.sig + 64);
    memcpy(param.ed448.pub, edkey->pubkey, 57);
    s390x_flip_endian64(param.ed448.pub, param.ed448.pub);

    return s390x_kdsa(S390X_EDDSA_VERIFY_ED448,
                      &param.ed448, tbs, tbslen) == 0 ? 1 : 0;
}

#endif /* S390X_EC_ASM */
