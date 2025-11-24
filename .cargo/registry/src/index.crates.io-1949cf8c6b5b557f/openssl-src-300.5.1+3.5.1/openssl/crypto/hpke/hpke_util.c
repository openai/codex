/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/core_names.h>
#include <openssl/kdf.h>
#include <openssl/params.h>
#include <openssl/err.h>
#include <openssl/proverr.h>
#include <openssl/hpke.h>
#include <openssl/sha.h>
#include <openssl/rand.h>
#include "crypto/ecx.h"
#include "crypto/rand.h"
#include "internal/hpke_util.h"
#include "internal/packet.h"
#include "internal/nelem.h"
#include "internal/common.h"

/*
 * Delimiter used in OSSL_HPKE_str2suite
 */
#define OSSL_HPKE_STR_DELIMCHAR ','

/*
 * table with identifier and synonym strings
 * right now, there are 4 synonyms for each - a name, a hex string
 * a hex string with a leading zero and a decimal string - more
 * could be added but that seems like enough
 */
typedef struct {
    uint16_t id;
    char *synonyms[4];
} synonymttab_t;

/* max length of string we'll try map to a suite */
#define OSSL_HPKE_MAX_SUITESTR 38

/* Define HPKE labels from RFC9180 in hex for EBCDIC compatibility */
/* ASCII: "HPKE-v1", in hex for EBCDIC compatibility */
static const char LABEL_HPKEV1[] = "\x48\x50\x4B\x45\x2D\x76\x31";

/*
 * Note that if additions are made to the set of IANA codepoints
 * and the tables below, corresponding additions should also be
 * made to the synonymtab tables a little further down so that
 * OSSL_HPKE_str2suite() continues to function correctly.
 *
 * The canonical place to check for IANA registered codepoints
 * is: https://www.iana.org/assignments/hpke/hpke.xhtml
 */

/*
 * @brief table of KEMs
 * See RFC9180 Section 7.1 "Table 2 KEM IDs"
 */
static const OSSL_HPKE_KEM_INFO hpke_kem_tab[] = {
#ifndef OPENSSL_NO_EC
    { OSSL_HPKE_KEM_ID_P256, "EC", OSSL_HPKE_KEMSTR_P256,
      LN_sha256, SHA256_DIGEST_LENGTH, 65, 65, 32, 0xFF },
    { OSSL_HPKE_KEM_ID_P384, "EC", OSSL_HPKE_KEMSTR_P384,
      LN_sha384, SHA384_DIGEST_LENGTH, 97, 97, 48, 0xFF },
    { OSSL_HPKE_KEM_ID_P521, "EC", OSSL_HPKE_KEMSTR_P521,
      LN_sha512, SHA512_DIGEST_LENGTH, 133, 133, 66, 0x01 },
# ifndef OPENSSL_NO_ECX
    { OSSL_HPKE_KEM_ID_X25519, OSSL_HPKE_KEMSTR_X25519, NULL,
      LN_sha256, SHA256_DIGEST_LENGTH,
      X25519_KEYLEN, X25519_KEYLEN, X25519_KEYLEN, 0x00 },
    { OSSL_HPKE_KEM_ID_X448, OSSL_HPKE_KEMSTR_X448, NULL,
      LN_sha512, SHA512_DIGEST_LENGTH,
      X448_KEYLEN, X448_KEYLEN, X448_KEYLEN, 0x00 }
# endif
#else
    { OSSL_HPKE_KEM_ID_RESERVED, NULL, NULL, NULL, 0, 0, 0, 0, 0x00 }
#endif
};

/*
 * @brief table of AEADs
 * See RFC9180 Section 7.2 "Table 3 KDF IDs"
 */
static const OSSL_HPKE_AEAD_INFO hpke_aead_tab[] = {
    { OSSL_HPKE_AEAD_ID_AES_GCM_128, LN_aes_128_gcm, 16, 16,
      OSSL_HPKE_MAX_NONCELEN },
    { OSSL_HPKE_AEAD_ID_AES_GCM_256, LN_aes_256_gcm, 16, 32,
      OSSL_HPKE_MAX_NONCELEN },
#if !defined(OPENSSL_NO_CHACHA) && !defined(OPENSSL_NO_POLY1305)
    { OSSL_HPKE_AEAD_ID_CHACHA_POLY1305, LN_chacha20_poly1305, 16, 32,
      OSSL_HPKE_MAX_NONCELEN },
#endif
    { OSSL_HPKE_AEAD_ID_EXPORTONLY, NULL, 0, 0, 0 }
};

/*
 * @brief table of KDFs
 * See RFC9180 Section 7.3 "Table 5 AEAD IDs"
 */
static const OSSL_HPKE_KDF_INFO hpke_kdf_tab[] = {
    { OSSL_HPKE_KDF_ID_HKDF_SHA256, LN_sha256, SHA256_DIGEST_LENGTH },
    { OSSL_HPKE_KDF_ID_HKDF_SHA384, LN_sha384, SHA384_DIGEST_LENGTH },
    { OSSL_HPKE_KDF_ID_HKDF_SHA512, LN_sha512, SHA512_DIGEST_LENGTH }
};

/**
 * Synonym tables for KEMs, KDFs and AEADs: idea is to allow
 * mapping strings to suites with a little flexibility in terms
 * of allowing a name or a couple of forms of number (for
 * the IANA codepoint). If new IANA codepoints are allocated
 * then these tables should be updated at the same time as the
 * others above.
 *
 * The function to use these is ossl_hpke_str2suite() further down
 * this file and shouldn't need modification so long as the table
 * sizes (i.e. allow exactly 4 synonyms) don't change.
 */
static const synonymttab_t kemstrtab[] = {
    {OSSL_HPKE_KEM_ID_P256,
     {OSSL_HPKE_KEMSTR_P256, "0x10", "0x10", "16" }},
    {OSSL_HPKE_KEM_ID_P384,
     {OSSL_HPKE_KEMSTR_P384, "0x11", "0x11", "17" }},
    {OSSL_HPKE_KEM_ID_P521,
     {OSSL_HPKE_KEMSTR_P521, "0x12", "0x12", "18" }},
# ifndef OPENSSL_NO_ECX
    {OSSL_HPKE_KEM_ID_X25519,
     {OSSL_HPKE_KEMSTR_X25519, "0x20", "0x20", "32" }},
    {OSSL_HPKE_KEM_ID_X448,
     {OSSL_HPKE_KEMSTR_X448, "0x21", "0x21", "33" }}
# endif
};
static const synonymttab_t kdfstrtab[] = {
    {OSSL_HPKE_KDF_ID_HKDF_SHA256,
     {OSSL_HPKE_KDFSTR_256, "0x1", "0x01", "1"}},
    {OSSL_HPKE_KDF_ID_HKDF_SHA384,
     {OSSL_HPKE_KDFSTR_384, "0x2", "0x02", "2"}},
    {OSSL_HPKE_KDF_ID_HKDF_SHA512,
     {OSSL_HPKE_KDFSTR_512, "0x3", "0x03", "3"}}
};
static const synonymttab_t aeadstrtab[] = {
    {OSSL_HPKE_AEAD_ID_AES_GCM_128,
     {OSSL_HPKE_AEADSTR_AES128GCM, "0x1", "0x01", "1"}},
    {OSSL_HPKE_AEAD_ID_AES_GCM_256,
     {OSSL_HPKE_AEADSTR_AES256GCM, "0x2", "0x02", "2"}},
    {OSSL_HPKE_AEAD_ID_CHACHA_POLY1305,
     {OSSL_HPKE_AEADSTR_CP, "0x3", "0x03", "3"}},
    {OSSL_HPKE_AEAD_ID_EXPORTONLY,
     {OSSL_HPKE_AEADSTR_EXP, "ff", "0xff", "255"}}
};

/* Return an object containing KEM constants associated with a EC curve name */
const OSSL_HPKE_KEM_INFO *ossl_HPKE_KEM_INFO_find_curve(const char *curve)
{
    int i, sz = OSSL_NELEM(hpke_kem_tab);

    for (i = 0; i < sz; ++i) {
        const char *group = hpke_kem_tab[i].groupname;

        if (group == NULL)
            group = hpke_kem_tab[i].keytype;
        if (OPENSSL_strcasecmp(curve, group) == 0)
            return &hpke_kem_tab[i];
    }
    ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_CURVE);
    return NULL;
}

const OSSL_HPKE_KEM_INFO *ossl_HPKE_KEM_INFO_find_id(uint16_t kemid)
{
    int i, sz = OSSL_NELEM(hpke_kem_tab);

    /*
     * this check can happen if we're in a no-ec build and there are no
     * KEMS available
     */
    if (kemid == OSSL_HPKE_KEM_ID_RESERVED) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_CURVE);
        return NULL;
    }
    for (i = 0; i != sz; ++i) {
        if (hpke_kem_tab[i].kem_id == kemid)
            return &hpke_kem_tab[i];
    }
    ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_CURVE);
    return NULL;
}

const OSSL_HPKE_KEM_INFO *ossl_HPKE_KEM_INFO_find_random(OSSL_LIB_CTX *ctx)
{
    uint32_t rval = 0;
    int err = 0;
    size_t sz = OSSL_NELEM(hpke_kem_tab);

    rval = ossl_rand_uniform_uint32(ctx, sz, &err);
    return (err == 1 ? NULL : &hpke_kem_tab[rval]);
}

const OSSL_HPKE_KDF_INFO *ossl_HPKE_KDF_INFO_find_id(uint16_t kdfid)
{
    int i, sz = OSSL_NELEM(hpke_kdf_tab);

    for (i = 0; i != sz; ++i) {
        if (hpke_kdf_tab[i].kdf_id == kdfid)
            return &hpke_kdf_tab[i];
    }
    ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KDF);
    return NULL;
}

const OSSL_HPKE_KDF_INFO *ossl_HPKE_KDF_INFO_find_random(OSSL_LIB_CTX *ctx)
{
    uint32_t rval = 0;
    int err = 0;
    size_t sz = OSSL_NELEM(hpke_kdf_tab);

    rval = ossl_rand_uniform_uint32(ctx, sz, &err);
    return (err == 1 ? NULL : &hpke_kdf_tab[rval]);
}

const OSSL_HPKE_AEAD_INFO *ossl_HPKE_AEAD_INFO_find_id(uint16_t aeadid)
{
    int i, sz = OSSL_NELEM(hpke_aead_tab);

    for (i = 0; i != sz; ++i) {
        if (hpke_aead_tab[i].aead_id == aeadid)
            return &hpke_aead_tab[i];
    }
    ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_AEAD);
    return NULL;
}

const OSSL_HPKE_AEAD_INFO *ossl_HPKE_AEAD_INFO_find_random(OSSL_LIB_CTX *ctx)
{
    uint32_t rval = 0;
    int err = 0;
    /* the minus 1 below is so we don't pick the EXPORTONLY codepoint */
    size_t sz = OSSL_NELEM(hpke_aead_tab) - 1;

    rval = ossl_rand_uniform_uint32(ctx, sz, &err);
    return (err == 1 ? NULL : &hpke_aead_tab[rval]);
}

static int kdf_derive(EVP_KDF_CTX *kctx,
                      unsigned char *out, size_t outlen, int mode,
                      const unsigned char *salt, size_t saltlen,
                      const unsigned char *ikm, size_t ikmlen,
                      const unsigned char *info, size_t infolen)
{
    int ret;
    OSSL_PARAM params[5], *p = params;

    *p++ = OSSL_PARAM_construct_int(OSSL_KDF_PARAM_MODE, &mode);
    if (salt != NULL)
        *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_SALT,
                                                 (char *)salt, saltlen);
    if (ikm != NULL)
        *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_KEY,
                                                 (char *)ikm, ikmlen);
    if (info != NULL)
        *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_INFO,
                                                 (char *)info, infolen);
    *p = OSSL_PARAM_construct_end();
    ret = EVP_KDF_derive(kctx, out, outlen, params) > 0;
    if (!ret)
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_DURING_DERIVATION);
    return ret;
}

int ossl_hpke_kdf_extract(EVP_KDF_CTX *kctx,
                          unsigned char *prk, size_t prklen,
                          const unsigned char *salt, size_t saltlen,
                          const unsigned char *ikm, size_t ikmlen)
{
    return kdf_derive(kctx, prk, prklen, EVP_KDF_HKDF_MODE_EXTRACT_ONLY,
                      salt, saltlen, ikm, ikmlen, NULL, 0);
}

/* Common code to perform a HKDF expand */
int ossl_hpke_kdf_expand(EVP_KDF_CTX *kctx,
                         unsigned char *okm, size_t okmlen,
                         const unsigned char *prk, size_t prklen,
                         const unsigned char *info, size_t infolen)
{
    return kdf_derive(kctx, okm, okmlen, EVP_KDF_HKDF_MODE_EXPAND_ONLY,
                      NULL, 0, prk, prklen, info, infolen);
}

/*
 * See RFC 9180 Section 4 LabelExtract()
 */
int ossl_hpke_labeled_extract(EVP_KDF_CTX *kctx,
                              unsigned char *prk, size_t prklen,
                              const unsigned char *salt, size_t saltlen,
                              const char *protocol_label,
                              const unsigned char *suiteid, size_t suiteidlen,
                              const char *label,
                              const unsigned char *ikm, size_t ikmlen)
{
    int ret = 0;
    size_t label_hpkev1len = 0;
    size_t protocol_labellen = 0;
    size_t labellen = 0;
    size_t labeled_ikmlen = 0;
    unsigned char *labeled_ikm = NULL;
    WPACKET pkt;

    label_hpkev1len = strlen(LABEL_HPKEV1);
    protocol_labellen = strlen(protocol_label);
    labellen = strlen(label);
    labeled_ikmlen = label_hpkev1len + protocol_labellen
        + suiteidlen + labellen + ikmlen;
    labeled_ikm = OPENSSL_malloc(labeled_ikmlen);
    if (labeled_ikm == NULL)
        return 0;

    /* labeled_ikm = concat("HPKE-v1", suiteid, label, ikm) */
    if (!WPACKET_init_static_len(&pkt, labeled_ikm, labeled_ikmlen, 0)
            || !WPACKET_memcpy(&pkt, LABEL_HPKEV1, label_hpkev1len)
            || !WPACKET_memcpy(&pkt, protocol_label, protocol_labellen)
            || !WPACKET_memcpy(&pkt, suiteid, suiteidlen)
            || !WPACKET_memcpy(&pkt, label, labellen)
            || !WPACKET_memcpy(&pkt, ikm, ikmlen)
            || !WPACKET_get_total_written(&pkt, &labeled_ikmlen)
            || !WPACKET_finish(&pkt)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
        goto end;
    }

    ret = ossl_hpke_kdf_extract(kctx, prk, prklen, salt, saltlen,
                                labeled_ikm, labeled_ikmlen);
end:
    WPACKET_cleanup(&pkt);
    OPENSSL_cleanse(labeled_ikm, labeled_ikmlen);
    OPENSSL_free(labeled_ikm);
    return ret;
}

/*
 * See RFC 9180 Section 4 LabelExpand()
 */
int ossl_hpke_labeled_expand(EVP_KDF_CTX *kctx,
                             unsigned char *okm, size_t okmlen,
                             const unsigned char *prk, size_t prklen,
                             const char *protocol_label,
                             const unsigned char *suiteid, size_t suiteidlen,
                             const char *label,
                             const unsigned char *info, size_t infolen)
{
    int ret = 0;
    size_t label_hpkev1len = 0;
    size_t protocol_labellen = 0;
    size_t labellen = 0;
    size_t labeled_infolen = 0;
    unsigned char *labeled_info = NULL;
    WPACKET pkt;

    label_hpkev1len = strlen(LABEL_HPKEV1);
    protocol_labellen = strlen(protocol_label);
    labellen = strlen(label);
    labeled_infolen = 2 + okmlen + prklen + label_hpkev1len
        + protocol_labellen + suiteidlen + labellen + infolen;
    labeled_info = OPENSSL_malloc(labeled_infolen);
    if (labeled_info == NULL)
        return 0;

    /* labeled_info = concat(okmlen, "HPKE-v1", suiteid, label, info) */
    if (!WPACKET_init_static_len(&pkt, labeled_info, labeled_infolen, 0)
            || !WPACKET_put_bytes_u16(&pkt, okmlen)
            || !WPACKET_memcpy(&pkt, LABEL_HPKEV1, label_hpkev1len)
            || !WPACKET_memcpy(&pkt, protocol_label, protocol_labellen)
            || !WPACKET_memcpy(&pkt, suiteid, suiteidlen)
            || !WPACKET_memcpy(&pkt, label, labellen)
            || !WPACKET_memcpy(&pkt, info, infolen)
            || !WPACKET_get_total_written(&pkt, &labeled_infolen)
            || !WPACKET_finish(&pkt)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
        goto end;
    }

    ret = ossl_hpke_kdf_expand(kctx, okm, okmlen,
                               prk, prklen, labeled_info, labeled_infolen);
end:
    WPACKET_cleanup(&pkt);
    OPENSSL_free(labeled_info);
    return ret;
}

/* Common code to create a HKDF ctx */
EVP_KDF_CTX *ossl_kdf_ctx_create(const char *kdfname, const char *mdname,
                                 OSSL_LIB_CTX *libctx, const char *propq)
{
    EVP_KDF *kdf;
    EVP_KDF_CTX *kctx = NULL;

    kdf = EVP_KDF_fetch(libctx, kdfname, propq);
    if (kdf == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_FETCH_FAILED);
        return NULL;
    }
    kctx = EVP_KDF_CTX_new(kdf);
    EVP_KDF_free(kdf);
    if (kctx != NULL && mdname != NULL) {
        OSSL_PARAM params[3], *p = params;

        if (mdname != NULL)
            *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_DIGEST,
                                                    (char *)mdname, 0);
        if (propq != NULL)
            *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_PROPERTIES,
                                                    (char *)propq, 0);
        *p = OSSL_PARAM_construct_end();
        if (EVP_KDF_CTX_set_params(kctx, params) <= 0) {
            EVP_KDF_CTX_free(kctx);
            return NULL;
        }
    }
    return kctx;
}

/*
 * @brief look for a label into the synonym tables, and return its id
 * @param st is the string value
 * @param synp is the synonyms labels array
 * @param arrsize is the previous array size
 * @return 0 when not found, else the matching item id.
 */
static uint16_t synonyms_name2id(const char *st, const synonymttab_t *synp,
                                 size_t arrsize)
{
    size_t i, j;

    for (i = 0; i < arrsize; ++i) {
        for (j = 0; j < OSSL_NELEM(synp[i].synonyms); ++j) {
            if (OPENSSL_strcasecmp(st, synp[i].synonyms[j]) == 0)
                return synp[i].id;
        }
    }
    return 0;
}

/*
 * @brief map a string to a HPKE suite based on synonym tables
 * @param str is the string value
 * @param suite is the resulting suite
 * @return 1 for success, otherwise failure
 */
int ossl_hpke_str2suite(const char *suitestr, OSSL_HPKE_SUITE *suite)
{
    uint16_t kem = 0, kdf = 0, aead = 0;
    char *st = NULL, *instrcp = NULL;
    size_t inplen;
    int labels = 0, result = 0;
    int delim_count = 0;

    if (suitestr == NULL || suitestr[0] == 0x00 || suite == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    inplen = OPENSSL_strnlen(suitestr, OSSL_HPKE_MAX_SUITESTR);
    if (inplen >= OSSL_HPKE_MAX_SUITESTR) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }

    /*
     * we don't want a delimiter at the end of the string;
     * strtok_r/s() doesn't care about that, so we should
     */
    if (suitestr[inplen - 1] == OSSL_HPKE_STR_DELIMCHAR)
        return 0;
    /* We want exactly two delimiters in the input string */
    for (st = (char *)suitestr; *st != '\0'; st++) {
        if (*st == OSSL_HPKE_STR_DELIMCHAR)
            delim_count++;
    }
    if (delim_count != 2)
        return 0;

    /* Duplicate `suitestr` to allow its parsing  */
    instrcp = OPENSSL_memdup(suitestr, inplen + 1);
    if (instrcp == NULL)
        goto fail;

    /* See if it contains a mix of our strings and numbers */
    st = instrcp;

    while (st != NULL && labels < 3) {
        char *cp = strchr(st, OSSL_HPKE_STR_DELIMCHAR);

        /* add a NUL like strtok would if we're not at the end */
        if (cp != NULL)
            *cp = '\0';

        /* check if string is known or number and if so handle appropriately */
        if (labels == 0
            && (kem = synonyms_name2id(st, kemstrtab,
                                       OSSL_NELEM(kemstrtab))) == 0)
            goto fail;
        else if (labels == 1
                 && (kdf = synonyms_name2id(st, kdfstrtab,
                                            OSSL_NELEM(kdfstrtab))) == 0)
            goto fail;
        else if (labels == 2
                 && (aead = synonyms_name2id(st, aeadstrtab,
                                             OSSL_NELEM(aeadstrtab))) == 0)
            goto fail;

        if (cp == NULL)
            st = NULL;
        else
            st = cp + 1;
        ++labels;
    }
    if (st != NULL || labels != 3)
        goto fail;
    suite->kem_id = kem;
    suite->kdf_id = kdf;
    suite->aead_id = aead;
    result = 1;

fail:
    OPENSSL_free(instrcp);
    return result;
}
