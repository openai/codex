/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * RSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"
#include "internal/nelem.h"
#include <openssl/crypto.h>
#include <openssl/evp.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/rsa.h>
#include <openssl/params.h>
#include <openssl/err.h>
#include <openssl/proverr.h>
#include "crypto/rsa.h"
#include "prov/provider_ctx.h"
#include "prov/providercommon.h"
#include "prov/implementations.h"
#include "prov/securitycheck.h"

static OSSL_FUNC_kem_newctx_fn rsakem_newctx;
static OSSL_FUNC_kem_encapsulate_init_fn rsakem_encapsulate_init;
static OSSL_FUNC_kem_encapsulate_fn rsakem_generate;
static OSSL_FUNC_kem_decapsulate_init_fn rsakem_decapsulate_init;
static OSSL_FUNC_kem_decapsulate_fn rsakem_recover;
static OSSL_FUNC_kem_freectx_fn rsakem_freectx;
static OSSL_FUNC_kem_dupctx_fn rsakem_dupctx;
static OSSL_FUNC_kem_get_ctx_params_fn rsakem_get_ctx_params;
static OSSL_FUNC_kem_gettable_ctx_params_fn rsakem_gettable_ctx_params;
static OSSL_FUNC_kem_set_ctx_params_fn rsakem_set_ctx_params;
static OSSL_FUNC_kem_settable_ctx_params_fn rsakem_settable_ctx_params;

/*
 * Only the KEM for RSASVE as defined in SP800-56b r2 is implemented
 * currently.
 */
#define KEM_OP_UNDEFINED   -1
#define KEM_OP_RSASVE       0

/*
 * What's passed as an actual key is defined by the KEYMGMT interface.
 * We happen to know that our KEYMGMT simply passes RSA structures, so
 * we use that here too.
 */
typedef struct {
    OSSL_LIB_CTX *libctx;
    RSA *rsa;
    int op;
    OSSL_FIPS_IND_DECLARE
} PROV_RSA_CTX;

static const OSSL_ITEM rsakem_opname_id_map[] = {
    { KEM_OP_RSASVE, OSSL_KEM_PARAM_OPERATION_RSASVE },
};

static int name2id(const char *name, const OSSL_ITEM *map, size_t sz)
{
    size_t i;

    if (name == NULL)
        return -1;

    for (i = 0; i < sz; ++i) {
        if (OPENSSL_strcasecmp(map[i].ptr, name) == 0)
            return map[i].id;
    }
    return -1;
}

static int rsakem_opname2id(const char *name)
{
    return name2id(name, rsakem_opname_id_map, OSSL_NELEM(rsakem_opname_id_map));
}

static void *rsakem_newctx(void *provctx)
{
    PROV_RSA_CTX *prsactx;

    if (!ossl_prov_is_running())
        return NULL;

    prsactx =  OPENSSL_zalloc(sizeof(PROV_RSA_CTX));
    if (prsactx == NULL)
        return NULL;
    prsactx->libctx = PROV_LIBCTX_OF(provctx);
    prsactx->op = KEM_OP_RSASVE;
    OSSL_FIPS_IND_INIT(prsactx)

    return prsactx;
}

static void rsakem_freectx(void *vprsactx)
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;

    RSA_free(prsactx->rsa);
    OPENSSL_free(prsactx);
}

static void *rsakem_dupctx(void *vprsactx)
{
    PROV_RSA_CTX *srcctx = (PROV_RSA_CTX *)vprsactx;
    PROV_RSA_CTX *dstctx;

    if (!ossl_prov_is_running())
        return NULL;

    dstctx = OPENSSL_zalloc(sizeof(*srcctx));
    if (dstctx == NULL)
        return NULL;

    *dstctx = *srcctx;
    if (dstctx->rsa != NULL && !RSA_up_ref(dstctx->rsa)) {
        OPENSSL_free(dstctx);
        return NULL;
    }
    return dstctx;
}

static int rsakem_init(void *vprsactx, void *vrsa,
                       const OSSL_PARAM params[], int operation,
                       const char *desc)
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;
    int protect = 0;

    if (!ossl_prov_is_running())
        return 0;

    if (prsactx == NULL || vrsa == NULL)
        return 0;

    if (!ossl_rsa_key_op_get_protect(vrsa, operation, &protect))
        return 0;
    if (!RSA_up_ref(vrsa))
        return 0;
    RSA_free(prsactx->rsa);
    prsactx->rsa = vrsa;

    OSSL_FIPS_IND_SET_APPROVED(prsactx)
    if (!rsakem_set_ctx_params(prsactx, params))
        return 0;
#ifdef FIPS_MODULE
    if (!ossl_fips_ind_rsa_key_check(OSSL_FIPS_IND_GET(prsactx),
                                     OSSL_FIPS_IND_SETTABLE0, prsactx->libctx,
                                     prsactx->rsa, desc, protect))
        return 0;
#endif
    return 1;
}

static int rsakem_encapsulate_init(void *vprsactx, void *vrsa,
                                   const OSSL_PARAM params[])
{
    return rsakem_init(vprsactx, vrsa, params, EVP_PKEY_OP_ENCAPSULATE,
                       "RSA Encapsulate Init");
}

static int rsakem_decapsulate_init(void *vprsactx, void *vrsa,
                                   const OSSL_PARAM params[])
{
    return rsakem_init(vprsactx, vrsa, params, EVP_PKEY_OP_DECAPSULATE,
                       "RSA Decapsulate Init");
}

static int rsakem_get_ctx_params(void *vprsactx, OSSL_PARAM *params)
{
    PROV_RSA_CTX *ctx = (PROV_RSA_CTX *)vprsactx;

    if (ctx == NULL)
        return 0;

    if (!OSSL_FIPS_IND_GET_CTX_PARAM(ctx, params))
        return 0;
    return 1;
}

static const OSSL_PARAM known_gettable_rsakem_ctx_params[] = {
    OSSL_FIPS_IND_GETTABLE_CTX_PARAM()
    OSSL_PARAM_END
};

static const OSSL_PARAM *rsakem_gettable_ctx_params(ossl_unused void *vprsactx,
                                                    ossl_unused void *provctx)
{
    return known_gettable_rsakem_ctx_params;
}

static int rsakem_set_ctx_params(void *vprsactx, const OSSL_PARAM params[])
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;
    const OSSL_PARAM *p;
    int op;

    if (prsactx == NULL)
        return 0;
    if (ossl_param_is_empty(params))
        return 1;

    if (!OSSL_FIPS_IND_SET_CTX_PARAM(prsactx, OSSL_FIPS_IND_SETTABLE0, params,
                                     OSSL_KEM_PARAM_FIPS_KEY_CHECK))
        return  0;
    p = OSSL_PARAM_locate_const(params, OSSL_KEM_PARAM_OPERATION);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_UTF8_STRING)
            return 0;
        op = rsakem_opname2id(p->data);
        if (op < 0)
            return 0;
        prsactx->op = op;
    }
    return 1;
}

static const OSSL_PARAM known_settable_rsakem_ctx_params[] = {
    OSSL_PARAM_utf8_string(OSSL_KEM_PARAM_OPERATION, NULL, 0),
    OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_KEM_PARAM_FIPS_KEY_CHECK)
    OSSL_PARAM_END
};

static const OSSL_PARAM *rsakem_settable_ctx_params(ossl_unused void *vprsactx,
                                                    ossl_unused void *provctx)
{
    return known_settable_rsakem_ctx_params;
}

/*
 * NIST.SP.800-56Br2
 * 7.2.1.2 RSASVE Generate Operation (RSASVE.GENERATE).
 *
 * Generate a random in the range 1 < z < (n – 1)
 */
static int rsasve_gen_rand_bytes(RSA *rsa_pub,
                                 unsigned char *out, int outlen)
{
    int ret = 0;
    BN_CTX *bnctx;
    BIGNUM *z, *nminus3;

    bnctx = BN_CTX_secure_new_ex(ossl_rsa_get0_libctx(rsa_pub));
    if (bnctx == NULL)
        return 0;

    /*
     * Generate a random in the range 1 < z < (n – 1).
     * Since BN_priv_rand_range_ex() returns a value in range 0 <= r < max
     * We can achieve this by adding 2.. but then we need to subtract 3 from
     * the upper bound i.e: 2 + (0 <= r < (n - 3))
     */
    BN_CTX_start(bnctx);
    nminus3 = BN_CTX_get(bnctx);
    z = BN_CTX_get(bnctx);
    ret = (z != NULL
           && (BN_copy(nminus3, RSA_get0_n(rsa_pub)) != NULL)
           && BN_sub_word(nminus3, 3)
           && BN_priv_rand_range_ex(z, nminus3, 0, bnctx)
           && BN_add_word(z, 2)
           && (BN_bn2binpad(z, out, outlen) == outlen));
    BN_CTX_end(bnctx);
    BN_CTX_free(bnctx);
    return ret;
}

/*
 * NIST.SP.800-56Br2
 * 7.2.1.2 RSASVE Generate Operation (RSASVE.GENERATE).
 */
static int rsasve_generate(PROV_RSA_CTX *prsactx,
                           unsigned char *out, size_t *outlen,
                           unsigned char *secret, size_t *secretlen)
{
    int ret;
    size_t nlen;

    /* Step (1): nlen = Ceil(len(n)/8) */
    nlen = RSA_size(prsactx->rsa);

    if (out == NULL) {
        if (nlen == 0) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY);
            return 0;
        }
        if (outlen == NULL && secretlen == NULL)
            return 0;
        if (outlen != NULL)
            *outlen = nlen;
        if (secretlen != NULL)
            *secretlen = nlen;
        return 1;
    }

    /*
     * If outlen is specified, then it must report the length
     * of the out buffer on input so that we can confirm
     * its size is sufficent for encapsulation
     */
    if (outlen != NULL && *outlen < nlen) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_OUTPUT_LENGTH);
        return 0;
    }

    /*
     * Step (2): Generate a random byte string z of nlen bytes where
     *            1 < z < n - 1
     */
    if (!rsasve_gen_rand_bytes(prsactx->rsa, secret, nlen))
        return 0;

    /* Step(3): out = RSAEP((n,e), z) */
    ret = RSA_public_encrypt(nlen, secret, out, prsactx->rsa, RSA_NO_PADDING);
    if (ret) {
        ret = 1;
        if (outlen != NULL)
            *outlen = nlen;
        if (secretlen != NULL)
            *secretlen = nlen;
    } else {
        OPENSSL_cleanse(secret, nlen);
    }
    return ret;
}

/**
 * rsasve_recover - Recovers a secret value from ciphertext using an RSA
 * private key.  Once, recovered, the secret value is considered to be a
 * shared secret.  Algorithm is preformed as per
 * NIST SP 800-56B Rev 2
 * 7.2.1.3 RSASVE Recovery Operation (RSASVE.RECOVER).
 *
 * This function performs RSA decryption using the private key from the
 * provided RSA context (`prsactx`). It takes the input ciphertext, decrypts
 * it, and writes the decrypted message to the output buffer.
 *
 * @prsactx:      The RSA context containing the private key.
 * @out:          The output buffer to store the decrypted message.
 * @outlen:       On input, the size of the output buffer. On successful
 *                completion, the actual length of the decrypted message.
 * @in:           The input buffer containing the ciphertext to be decrypted.
 * @inlen:        The length of the input ciphertext in bytes.
 *
 * Returns 1 on success, or 0 on error. In case of error, appropriate
 * error messages are raised using the ERR_raise function.
 */
static int rsasve_recover(PROV_RSA_CTX *prsactx,
                          unsigned char *out, size_t *outlen,
                          const unsigned char *in, size_t inlen)
{
    size_t nlen;
    int ret;

    /* Step (1): get the byte length of n */
    nlen = RSA_size(prsactx->rsa);

    if (out == NULL) {
        if (nlen == 0) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY);
            return 0;
        }
        *outlen = nlen;
        return 1;
    }

    /*
     * Step (2): check the input ciphertext 'inlen' matches the nlen
     * and that outlen is at least nlen bytes
     */
    if (inlen != nlen) {
        ERR_raise(ERR_LIB_PROV, PROV_R_BAD_LENGTH);
        return 0;
    }

    /*
     * If outlen is specified, then it must report the length
     * of the out buffer, so that we can confirm that it is of
     * sufficient size to hold the output of decapsulation
     */
    if (outlen != NULL && *outlen < nlen) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_OUTPUT_LENGTH);
        return 0;
    }

    /* Step (3): out = RSADP((n,d), in) */
    ret = RSA_private_decrypt(inlen, in, out, prsactx->rsa, RSA_NO_PADDING);
    if (ret > 0 && outlen != NULL)
        *outlen = ret;
    return ret > 0;
}

static int rsakem_generate(void *vprsactx, unsigned char *out, size_t *outlen,
                           unsigned char *secret, size_t *secretlen)
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;

    if (!ossl_prov_is_running())
        return 0;

    switch (prsactx->op) {
        case KEM_OP_RSASVE:
            return rsasve_generate(prsactx, out, outlen, secret, secretlen);
        default:
            return -2;
    }
}

static int rsakem_recover(void *vprsactx, unsigned char *out, size_t *outlen,
                          const unsigned char *in, size_t inlen)
{
    PROV_RSA_CTX *prsactx = (PROV_RSA_CTX *)vprsactx;

    if (!ossl_prov_is_running())
        return 0;

    switch (prsactx->op) {
        case KEM_OP_RSASVE:
            return rsasve_recover(prsactx, out, outlen, in, inlen);
        default:
            return -2;
    }
}

const OSSL_DISPATCH ossl_rsa_asym_kem_functions[] = {
    { OSSL_FUNC_KEM_NEWCTX, (void (*)(void))rsakem_newctx },
    { OSSL_FUNC_KEM_ENCAPSULATE_INIT,
      (void (*)(void))rsakem_encapsulate_init },
    { OSSL_FUNC_KEM_ENCAPSULATE, (void (*)(void))rsakem_generate },
    { OSSL_FUNC_KEM_DECAPSULATE_INIT,
      (void (*)(void))rsakem_decapsulate_init },
    { OSSL_FUNC_KEM_DECAPSULATE, (void (*)(void))rsakem_recover },
    { OSSL_FUNC_KEM_FREECTX, (void (*)(void))rsakem_freectx },
    { OSSL_FUNC_KEM_DUPCTX, (void (*)(void))rsakem_dupctx },
    { OSSL_FUNC_KEM_GET_CTX_PARAMS,
      (void (*)(void))rsakem_get_ctx_params },
    { OSSL_FUNC_KEM_GETTABLE_CTX_PARAMS,
      (void (*)(void))rsakem_gettable_ctx_params },
    { OSSL_FUNC_KEM_SET_CTX_PARAMS,
      (void (*)(void))rsakem_set_ctx_params },
    { OSSL_FUNC_KEM_SETTABLE_CTX_PARAMS,
      (void (*)(void))rsakem_settable_ctx_params },
    OSSL_DISPATCH_END
};
