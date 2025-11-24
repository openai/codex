/*
 * Copyright 2021-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/objects.h>
#include <openssl/crypto.h>
#include <openssl/provider.h>
#include "testutil.h"

static const OSSL_ALGORITHM *obj_query(void *provctx, int operation_id,
                                       int *no_cache)
{
    *no_cache = 0;
    return NULL;
}

static const OSSL_DISPATCH obj_dispatch_table[] = {
    { OSSL_FUNC_PROVIDER_QUERY_OPERATION, (void (*)(void))obj_query },
    OSSL_DISPATCH_END
};

static OSSL_FUNC_core_obj_add_sigid_fn *c_obj_add_sigid = NULL;
static OSSL_FUNC_core_obj_create_fn *c_obj_create = NULL;

/* test signature ids requiring digest */
#define SIG_OID "1.3.6.1.4.1.16604.998877.1"
#define SIG_SN "my-sig"
#define SIG_LN "my-sig-long"
#define DIGEST_OID "1.3.6.1.4.1.16604.998877.2"
#define DIGEST_SN "my-digest"
#define DIGEST_LN "my-digest-long"
#define SIGALG_OID "1.3.6.1.4.1.16604.998877.3"
#define SIGALG_SN "my-sigalg"
#define SIGALG_LN "my-sigalg-long"

/* test signature ids requiring no digest */
#define NODIG_SIG_OID "1.3.6.1.4.1.16604.998877.4"
#define NODIG_SIG_SN "my-nodig-sig"
#define NODIG_SIG_LN "my-nodig-sig-long"
#define NODIG_SIGALG_OID "1.3.6.1.4.1.16604.998877.5"
#define NODIG_SIGALG_SN "my-nodig-sigalg"
#define NODIG_SIGALG_LN "my-nodig-sigalg-long"

static int obj_provider_init(const OSSL_CORE_HANDLE *handle,
                             const OSSL_DISPATCH *in,
                             const OSSL_DISPATCH **out,
                             void **provctx)
{
    *provctx = (void *)handle;
    *out = obj_dispatch_table;

    for (; in->function_id != 0; in++) {
        switch (in->function_id) {
        case OSSL_FUNC_CORE_OBJ_ADD_SIGID:
            c_obj_add_sigid = OSSL_FUNC_core_obj_add_sigid(in);
            break;
        case OSSL_FUNC_CORE_OBJ_CREATE:
            c_obj_create = OSSL_FUNC_core_obj_create(in);
            break;
            break;
        default:
            /* Just ignore anything we don't understand */
            break;
        }
    }

    if (!c_obj_create(handle, DIGEST_OID, DIGEST_SN, DIGEST_LN)
            || !c_obj_create(handle, SIG_OID, SIG_SN, SIG_LN)
            || !c_obj_create(handle, SIGALG_OID, SIGALG_SN, SIGALG_LN))
        return 0;

    if (!c_obj_create(handle, NODIG_SIG_OID, NODIG_SIG_SN, NODIG_SIG_LN)
            || !c_obj_create(handle, NODIG_SIGALG_OID, NODIG_SIGALG_SN, NODIG_SIGALG_LN))
        return 0;

    if (!c_obj_add_sigid(handle, SIGALG_OID, DIGEST_SN, SIG_LN))
        return 0;

    /* additional tests checking empty digest algs are accepted, too */
    if (!c_obj_add_sigid(handle, NODIG_SIGALG_OID, "", NODIG_SIG_LN))
        return 0;
    /* checking wrong digest alg name is rejected: */
    if (c_obj_add_sigid(handle, NODIG_SIGALG_OID, "NonsenseAlg", NODIG_SIG_LN))
        return 0;

    return 1;
}

static int obj_create_test(void)
{
    OSSL_LIB_CTX *libctx = OSSL_LIB_CTX_new();
    OSSL_PROVIDER *objprov = NULL;
    int sigalgnid, digestnid, signid, foundsid;
    int testresult = 0;

    if (!TEST_ptr(libctx))
        goto err;

    if (!TEST_true(OSSL_PROVIDER_add_builtin(libctx, "obj-prov",
                                             obj_provider_init))
            || !TEST_ptr(objprov = OSSL_PROVIDER_load(libctx, "obj-prov")))
        goto err;

    /* Check that the provider created the OIDs/NIDs we expected */
    sigalgnid = OBJ_txt2nid(SIGALG_OID);
    if (!TEST_int_ne(sigalgnid, NID_undef)
            || !TEST_true(OBJ_find_sigid_algs(sigalgnid, &digestnid, &signid))
            || !TEST_int_ne(digestnid, NID_undef)
            || !TEST_int_ne(signid, NID_undef)
            || !TEST_int_eq(digestnid, OBJ_sn2nid(DIGEST_SN))
            || !TEST_int_eq(signid, OBJ_ln2nid(SIG_LN)))
        goto err;

    /* Check empty digest alg storage capability */
    sigalgnid = OBJ_txt2nid(NODIG_SIGALG_OID);
    if (!TEST_int_ne(sigalgnid, NID_undef)
            || !TEST_true(OBJ_find_sigid_algs(sigalgnid, &digestnid, &signid))
            || !TEST_int_eq(digestnid, NID_undef)
            || !TEST_int_ne(signid, NID_undef))
        goto err;

    /* Testing OBJ_find_sigid_by_algs */
    /* First check exact sig/digest recall: */
    sigalgnid = OBJ_sn2nid(SIGALG_SN);
    digestnid = OBJ_sn2nid(DIGEST_SN);
    signid = OBJ_ln2nid(SIG_LN);
    if ((!OBJ_find_sigid_by_algs(&foundsid, digestnid, signid)) ||
        (foundsid != sigalgnid))
        return 0;
    /* Check wrong signature/digest combination is rejected */
    if ((OBJ_find_sigid_by_algs(&foundsid, OBJ_sn2nid("SHA512"), signid)) &&
        (foundsid == sigalgnid))
        return 0;
    /* Now also check signature not needing digest is found */
    /* a) when some digest is given */
    sigalgnid = OBJ_sn2nid(NODIG_SIGALG_SN);
    digestnid = OBJ_sn2nid("SHA512");
    signid = OBJ_ln2nid(NODIG_SIG_LN);
    if ((!OBJ_find_sigid_by_algs(&foundsid, digestnid, signid)) ||
        (foundsid != sigalgnid))
        return 0;
    /* b) when NID_undef is passed */
    digestnid = NID_undef;
    if ((!OBJ_find_sigid_by_algs(&foundsid, digestnid, signid)) ||
        (foundsid != sigalgnid))
        return 0;

    testresult = 1;
 err:
    OSSL_PROVIDER_unload(objprov);
    OSSL_LIB_CTX_free(libctx);
    return testresult;
}

int setup_tests(void)
{

    ADD_TEST(obj_create_test);

    return 1;
}
