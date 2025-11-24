/*
 * Copyright 2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/byteorder.h>
#include <openssl/proverr.h>
#include <openssl/x509.h>
#include <openssl/core_names.h>
#include "internal/encoder.h"
#include "prov/ml_kem.h"
#include "ml_kem_codecs.h"

/* Tables describing supported ASN.1 input/output formats. */

/*-
 * ML-KEM-512:
 * Public key bytes:   800 (0x0320)
 * Private key bytes: 1632 (0x0660)
 */
static const ML_COMMON_SPKI_FMT ml_kem_512_spkifmt = {
    { 0x30, 0x82, 0x03, 0x32, 0x30, 0x0b, 0x06, 0x09, 0x60, 0x86, 0x48,
      0x01, 0x65, 0x03, 0x04, 0x04, 0x01, 0x03, 0x82, 0x03, 0x21, 0x00, }
};
static const ML_COMMON_PKCS8_FMT ml_kem_512_p8fmt[NUM_PKCS8_FORMATS] = {
    { "seed-priv",  0x06aa, 0, 0x308206a6, 0x0440, 6, 0x40, 0x04820660, 0x4a, 0x0660, 0,      0      },
    { "priv-only",  0x0664, 0, 0x04820660, 0,      0, 0,    0,          0x04, 0x0660, 0,      0      },
    { "oqskeypair", 0x0984, 0, 0x04820980, 0,      0, 0,    0,          0x04, 0x0660, 0x0664, 0x0320 },
    { "seed-only",  0x0042, 2, 0x8040,     0,      2, 0x40, 0,          0,    0,      0,      0      },
    { "bare-priv",  0x0660, 4, 0,          0,      0, 0,    0,          0,    0x0660, 0,      0      },
    { "bare-seed",  0x0040, 4, 0,          0,      0, 0x40, 0,          0,    0,      0,      0      },
};

/*-
 * ML-KEM-768:
 * Public key bytes:  1184 (0x04a0)
 * Private key bytes: 2400 (0x0960)
 */
static const ML_COMMON_SPKI_FMT ml_kem_768_spkifmt = {
    { 0x30, 0x82, 0x04, 0xb2, 0x30, 0x0b, 0x06, 0x09, 0x60, 0x86, 0x48,
      0x01, 0x65, 0x03, 0x04, 0x04, 0x02, 0x03, 0x82, 0x04, 0xa1, 0x00, }
};
static const ML_COMMON_PKCS8_FMT ml_kem_768_p8fmt[NUM_PKCS8_FORMATS] = {
    { "seed-priv",  0x09aa, 0, 0x308209a6, 0x0440, 6, 0x40, 0x04820960, 0x4a, 0x0960, 0,      0,     },
    { "priv-only",  0x0964, 0, 0x04820960, 0,      0, 0,    0,          0x04, 0x0960, 0,      0,     },
    { "oqskeypair", 0x0e04, 0, 0x04820e00, 0,      0, 0,    0,          0x04, 0x0960, 0x0964, 0x04a0 },
    { "seed-only",  0x0042, 2, 0x8040,     0,      2, 0x40, 0,          0,    0,      0,      0,     },
    { "bare-priv",  0x0960, 4, 0,          0,      0, 0,    0,          0,    0x0960, 0,      0,     },
    { "bare-seed",  0x0040, 4, 0,          0,      0, 0x40, 0,          0,    0,      0,      0,     },
};

/*-
 * ML-KEM-1024:
 * Private key bytes: 3168 (0x0c60)
 * Public key bytes:  1568 (0x0620)
 */
static const ML_COMMON_SPKI_FMT ml_kem_1024_spkifmt = {
    { 0x30, 0x82, 0x06, 0x32, 0x30, 0x0b, 0x06, 0x09, 0x60, 0x86, 0x48,
      0x01, 0x65, 0x03, 0x04, 0x04, 0x03, 0x03, 0x82, 0x06, 0x21, 0x00, }
};
static const ML_COMMON_PKCS8_FMT ml_kem_1024_p8fmt[NUM_PKCS8_FORMATS] = {
    { "seed-priv",  0x0caa, 0, 0x30820ca6, 0x0440, 6, 0x40, 0x04820c60, 0x4a, 0x0c60, 0,      0      },
    { "priv-only",  0x0c64, 0, 0x04820c60, 0,      0, 0,    0,          0x04, 0x0c60, 0,      0      },
    { "oqskeypair", 0x1284, 0, 0x04821280, 0,      0, 0,    0,          0x04, 0x0c60, 0x0c64, 0x0620 },
    { "seed-only",  0x0042, 2, 0x8040,     0,      2, 0x40, 0,          0,    0,      0,      0      },
    { "bare-priv",  0x0c60, 4, 0,          0,      0, 0,    0,          0,    0x0c60, 0,      0      },
    { "bare-seed",  0x0040, 4, 0,          0,      0, 0x40, 0,          0,    0,      0,      0      },
};

/* Indices of slots in the `codecs` table below */
#define ML_KEM_512_CODEC    0
#define ML_KEM_768_CODEC    1
#define ML_KEM_1024_CODEC   2

/*
 * Per-variant fixed parameters
 */
static const ML_COMMON_CODEC codecs[3] = {
    { &ml_kem_512_spkifmt,  ml_kem_512_p8fmt },
    { &ml_kem_768_spkifmt,  ml_kem_768_p8fmt },
    { &ml_kem_1024_spkifmt, ml_kem_1024_p8fmt }
};

/* Retrieve the parameters of one of the ML-KEM variants */
static const ML_COMMON_CODEC *ml_kem_get_codec(int evp_type)
{
    switch (evp_type) {
    case EVP_PKEY_ML_KEM_512:
        return &codecs[ML_KEM_512_CODEC];
    case EVP_PKEY_ML_KEM_768:
        return &codecs[ML_KEM_768_CODEC];
    case EVP_PKEY_ML_KEM_1024:
        return &codecs[ML_KEM_1024_CODEC];
    }
    return NULL;
}

ML_KEM_KEY *
ossl_ml_kem_d2i_PUBKEY(const uint8_t *pubenc, int publen, int evp_type,
                       PROV_CTX *provctx, const char *propq)
{
    OSSL_LIB_CTX *libctx = PROV_LIBCTX_OF(provctx);
    const ML_KEM_VINFO *v;
    const ML_COMMON_CODEC *codec;
    const ML_COMMON_SPKI_FMT *vspki;
    ML_KEM_KEY *ret;

    if ((v = ossl_ml_kem_get_vinfo(evp_type)) == NULL
        || (codec = ml_kem_get_codec(evp_type)) == NULL)
        return NULL;
    vspki = codec->spkifmt;
    if (publen != ML_COMMON_SPKI_OVERHEAD + (ossl_ssize_t) v->pubkey_bytes
        || memcmp(pubenc, vspki->asn1_prefix, ML_COMMON_SPKI_OVERHEAD) != 0)
        return NULL;
    publen -= ML_COMMON_SPKI_OVERHEAD;
    pubenc += ML_COMMON_SPKI_OVERHEAD;

    if ((ret = ossl_ml_kem_key_new(libctx, propq, evp_type)) == NULL)
        return NULL;

    if (!ossl_ml_kem_parse_public_key(pubenc, (size_t) publen, ret)) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_BAD_ENCODING,
                       "errror parsing %s public key from input SPKI",
                       v->algorithm_name);
        ossl_ml_kem_key_free(ret);
        return NULL;
    }

    return ret;
}

ML_KEM_KEY *
ossl_ml_kem_d2i_PKCS8(const uint8_t *prvenc, int prvlen,
                      int evp_type, PROV_CTX *provctx,
                      const char *propq)
{
    const ML_KEM_VINFO *v;
    const ML_COMMON_CODEC *codec;
    ML_COMMON_PKCS8_FMT_PREF *fmt_slots = NULL, *slot;
    const ML_COMMON_PKCS8_FMT *p8fmt;
    ML_KEM_KEY *key = NULL, *ret = NULL;
    PKCS8_PRIV_KEY_INFO *p8inf = NULL;
    const uint8_t *buf, *pos;
    const X509_ALGOR *alg = NULL;
    const char *formats;
    int len, ptype;
    uint32_t magic;
    uint16_t seed_magic;

    /* Which ML-KEM variant? */
    if ((v = ossl_ml_kem_get_vinfo(evp_type)) == NULL
        || (codec = ml_kem_get_codec(evp_type)) == NULL)
        return 0;

    /* Extract the key OID and any parameters. */
    if ((p8inf = d2i_PKCS8_PRIV_KEY_INFO(NULL, &prvenc, prvlen)) == NULL)
        return 0;
    /* Shortest prefix is 4 bytes: seq tag/len  + octet string tag/len */
    if (!PKCS8_pkey_get0(NULL, &buf, &len, &alg, p8inf))
        goto end;
    /* Bail out early if this is some other key type. */
    if (OBJ_obj2nid(alg->algorithm) != evp_type)
        goto end;

    /* Get the list of enabled decoders. Their order is not important here. */
    formats = ossl_prov_ctx_get_param(
        provctx, OSSL_PKEY_PARAM_ML_KEM_INPUT_FORMATS, NULL);
    fmt_slots = ossl_ml_common_pkcs8_fmt_order(v->algorithm_name, codec->p8fmt,
                                               "input", formats);
    if (fmt_slots == NULL)
        goto end;

    /* Parameters must be absent. */
    X509_ALGOR_get0(NULL, &ptype, NULL, alg);
    if (ptype != V_ASN1_UNDEF) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_UNEXPECTED_KEY_PARAMETERS,
                       "unexpected parameters with a PKCS#8 %s private key",
                       v->algorithm_name);
        goto end;
    }
    if ((ossl_ssize_t)len < (ossl_ssize_t)sizeof(magic))
        goto end;

    /* Find the matching p8 info slot, that also has the expected length. */
    pos = OPENSSL_load_u32_be(&magic, buf);
    for (slot = fmt_slots; (p8fmt = slot->fmt) != NULL; ++slot) {
        if (len != (ossl_ssize_t)p8fmt->p8_bytes)
            continue;
        if (p8fmt->p8_shift == sizeof(magic)
            || (magic >> (p8fmt->p8_shift * 8)) == p8fmt->p8_magic) {
            pos -= p8fmt->p8_shift;
            break;
        }
    }
    if (p8fmt == NULL
        || (p8fmt->seed_length > 0 && p8fmt->seed_length != ML_KEM_SEED_BYTES)
        || (p8fmt->priv_length > 0 && p8fmt->priv_length != v->prvkey_bytes)
        || (p8fmt->pub_length > 0 && p8fmt->pub_length != v->pubkey_bytes)) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_ML_KEM_NO_FORMAT,
                       "no matching enabled %s private key input formats",
                       v->algorithm_name);
        goto end;
    }

    if (p8fmt->seed_length > 0) {
        /* Check |seed| tag/len, if not subsumed by |magic|. */
        if (pos + sizeof(uint16_t) == buf + p8fmt->seed_offset) {
            pos = OPENSSL_load_u16_be(&seed_magic, pos);
            if (seed_magic != p8fmt->seed_magic)
                goto end;
        } else if (pos != buf + p8fmt->seed_offset) {
            goto end;
        }
        pos += ML_KEM_SEED_BYTES;
    }
    if (p8fmt->priv_length > 0) {
        /* Check |priv| tag/len */
        if (pos + sizeof(uint32_t) == buf + p8fmt->priv_offset) {
            pos = OPENSSL_load_u32_be(&magic, pos);
            if (magic != p8fmt->priv_magic)
                goto end;
        } else if (pos != buf + p8fmt->priv_offset) {
            goto end;
        }
        pos += v->prvkey_bytes;
    }
    if (p8fmt->pub_length > 0) {
        if (pos != buf + p8fmt->pub_offset)
            goto end;
        pos += v->pubkey_bytes;
    }
    if (pos != buf + len)
        goto end;

    /*
     * Collect the seed and/or key into a "decoded" private key object,
     * to be turned into a real key on provider "load" or "import".
     */
    if ((key = ossl_prov_ml_kem_new(provctx, propq, evp_type)) == NULL)
        goto end;

    if (p8fmt->seed_length > 0) {
        if (!ossl_ml_kem_set_seed(buf + p8fmt->seed_offset,
                                  ML_KEM_SEED_BYTES, key)) {
            ERR_raise_data(ERR_LIB_OSSL_DECODER, ERR_R_INTERNAL_ERROR,
                           "error storing %s private key seed",
                           v->algorithm_name);
            goto end;
        }
    }
    if (p8fmt->priv_length > 0) {
        if ((key->encoded_dk = OPENSSL_malloc(p8fmt->priv_length)) == NULL) {
            ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_KEY,
                           "error parsing %s private key",
                           v->algorithm_name);
            goto end;
        }
        memcpy(key->encoded_dk, buf + p8fmt->priv_offset, p8fmt->priv_length);
    }
    /* Any OQS public key content is ignored */
    ret = key;

 end:
    OPENSSL_free(fmt_slots);
    PKCS8_PRIV_KEY_INFO_free(p8inf);
    if (ret == NULL)
        ossl_ml_kem_key_free(key);
    return ret;
}

/* Same as ossl_ml_kem_encode_pubkey, but allocates the output buffer. */
int ossl_ml_kem_i2d_pubkey(const ML_KEM_KEY *key, unsigned char **out)
{
    size_t publen;

    if (!ossl_ml_kem_have_pubkey(key)) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_NOT_A_PUBLIC_KEY,
                       "no %s public key data available",
                       key->vinfo->algorithm_name);
        return 0;
    }
    publen = key->vinfo->pubkey_bytes;

    if (out != NULL
        && (*out = OPENSSL_malloc(publen)) == NULL)
        return 0;
    if (!ossl_ml_kem_encode_public_key(*out, publen, key)) {
        ERR_raise_data(ERR_LIB_OSSL_ENCODER, ERR_R_INTERNAL_ERROR,
                       "error encoding %s public key",
                       key->vinfo->algorithm_name);
        OPENSSL_free(*out);
        return 0;
    }

    return (int)publen;
}

/* Allocate and encode PKCS#8 private key payload. */
int ossl_ml_kem_i2d_prvkey(const ML_KEM_KEY *key, uint8_t **out,
                           PROV_CTX *provctx)
{
    const ML_KEM_VINFO *v = key->vinfo;
    const ML_COMMON_CODEC *codec;
    ML_COMMON_PKCS8_FMT_PREF *fmt_slots, *slot;
    const ML_COMMON_PKCS8_FMT *p8fmt;
    uint8_t *buf = NULL, *pos;
    const char *formats;
    int len = ML_KEM_SEED_BYTES;
    int ret = 0;

    /* Not ours to handle */
    if ((codec = ml_kem_get_codec(v->evp_type)) == NULL)
        return 0;

    if (!ossl_ml_kem_have_prvkey(key)) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_NOT_A_PRIVATE_KEY,
                       "no %s private key data available",
                       key->vinfo->algorithm_name);
        return 0;
    }

    formats = ossl_prov_ctx_get_param(
        provctx, OSSL_PKEY_PARAM_ML_KEM_OUTPUT_FORMATS, NULL);
    fmt_slots = ossl_ml_common_pkcs8_fmt_order(v->algorithm_name, codec->p8fmt,
                                               "output", formats);
    if (fmt_slots == NULL)
        return 0;

    /* If we don't have a seed, skip seedful entries */
    for (slot = fmt_slots; (p8fmt = slot->fmt) != NULL; ++slot)
        if (ossl_ml_kem_have_seed(key) || p8fmt->seed_length == 0)
            break;
    /* No matching table entries, give up */
    if (p8fmt == NULL
        || (p8fmt->seed_length > 0 && p8fmt->seed_length != ML_KEM_SEED_BYTES)
        || (p8fmt->priv_length > 0 && p8fmt->priv_length != v->prvkey_bytes)
        || (p8fmt->pub_length > 0 && p8fmt->pub_length != v->pubkey_bytes)) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_ML_KEM_NO_FORMAT,
                       "no matching enabled %s private key output formats",
                       v->algorithm_name);
        goto end;
    }
    len = p8fmt->p8_bytes;

    if (out == NULL) {
        ret = len;
        goto end;
    }

    if ((pos = buf = OPENSSL_malloc((size_t) len)) == NULL)
        goto end;

    switch (p8fmt->p8_shift) {
    case 0:
        pos = OPENSSL_store_u32_be(pos, p8fmt->p8_magic);
        break;
    case 2:
        pos = OPENSSL_store_u16_be(pos, (uint16_t)p8fmt->p8_magic);
        break;
    case 4:
        break;
    default:
        ERR_raise_data(ERR_LIB_PROV, ERR_R_INTERNAL_ERROR,
                       "error encoding %s private key",
                       v->algorithm_name);
        goto end;
    }

    if (p8fmt->seed_length != 0) {
        /*
         * Either the tag/len were already included in |magic| or they require
         * us to write two bytes now.
         */
        if (pos + sizeof(uint16_t) == buf + p8fmt->seed_offset)
            pos = OPENSSL_store_u16_be(pos, p8fmt->seed_magic);
        if (pos != buf + p8fmt->seed_offset
            || !ossl_ml_kem_encode_seed(pos, ML_KEM_SEED_BYTES, key)) {
            ERR_raise_data(ERR_LIB_PROV, ERR_R_INTERNAL_ERROR,
                           "error encoding %s private key",
                           v->algorithm_name);
            goto end;
        }
        pos += ML_KEM_SEED_BYTES;
    }
    if (p8fmt->priv_length != 0) {
        if (pos + sizeof(uint32_t) == buf + p8fmt->priv_offset)
            pos = OPENSSL_store_u32_be(pos, p8fmt->priv_magic);
        if (pos != buf + p8fmt->priv_offset
            || !ossl_ml_kem_encode_private_key(pos, v->prvkey_bytes, key)) {
            ERR_raise_data(ERR_LIB_PROV, ERR_R_INTERNAL_ERROR,
                           "error encoding %s private key",
                           v->algorithm_name);
            goto end;
        }
        pos += v->prvkey_bytes;
    }
    /* OQS form output with tacked-on public key */
    if (p8fmt->pub_length != 0) {
        /* The OQS pubkey is never separately DER-wrapped */
        if (pos != buf + p8fmt->pub_offset
            || !ossl_ml_kem_encode_public_key(pos, v->pubkey_bytes, key)) {
            ERR_raise_data(ERR_LIB_PROV, ERR_R_INTERNAL_ERROR,
                           "error encoding %s private key",
                           v->algorithm_name);
            goto end;
        }
        pos += v->pubkey_bytes;
    }

    if (pos == buf + len) {
        *out = buf;
        ret = len;
    }

 end:
    OPENSSL_free(fmt_slots);
    if (ret == 0)
        OPENSSL_free(buf);
    return ret;
}

int ossl_ml_kem_key_to_text(BIO *out, const ML_KEM_KEY *key, int selection)
{
    uint8_t seed[ML_KEM_SEED_BYTES], *prvenc = NULL, *pubenc = NULL;
    size_t publen, prvlen;
    const char *type_label = NULL;
    int ret = 0;

    if (out == NULL || key == NULL) {
        ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    type_label = key->vinfo->algorithm_name;
    publen = key->vinfo->pubkey_bytes;
    prvlen = key->vinfo->prvkey_bytes;

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0
        && (ossl_ml_kem_have_prvkey(key)
            || ossl_ml_kem_have_seed(key))) {
        if (BIO_printf(out, "%s Private-Key:\n", type_label) <= 0)
            return 0;

        if (ossl_ml_kem_have_seed(key)) {
            if (!ossl_ml_kem_encode_seed(seed, sizeof(seed), key))
                goto end;
            if (!ossl_bio_print_labeled_buf(out, "seed:", seed, sizeof(seed)))
                goto end;
        }
        if (ossl_ml_kem_have_prvkey(key)) {
            if ((prvenc = OPENSSL_malloc(prvlen)) == NULL)
                return 0;
            if (!ossl_ml_kem_encode_private_key(prvenc, prvlen, key))
                goto end;
            if (!ossl_bio_print_labeled_buf(out, "dk:", prvenc, prvlen))
                goto end;
        }
        ret = 1;
    }

    /* The public key is output regardless of the selection */
    if (ossl_ml_kem_have_pubkey(key)) {
        /* If we did not output private key bits, this is a public key */
        if (ret == 0 && BIO_printf(out, "%s Public-Key:\n", type_label) <= 0)
            goto end;

        if ((pubenc = OPENSSL_malloc(key->vinfo->pubkey_bytes)) == NULL
            || !ossl_ml_kem_encode_public_key(pubenc, publen, key)
            || !ossl_bio_print_labeled_buf(out, "ek:", pubenc, publen))
            goto end;
        ret = 1;
    }

    /* If we got here, and ret == 0, there was no key material */
    if (ret == 0)
        ERR_raise_data(ERR_LIB_PROV, PROV_R_MISSING_KEY,
                       "no %s key material available",
                       type_label);

 end:
    OPENSSL_free(pubenc);
    OPENSSL_free(prvenc);
    return ret;
}
