/*
 * Copyright 2005-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Support for PVK format keys and related structures (such a PUBLICKEYBLOB
 * and PRIVATEKEYBLOB).
 */

/*
 * RSA and DSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <openssl/pem.h>
#include <openssl/rand.h>
#include <openssl/bn.h>
#include <openssl/dsa.h>
#include <openssl/rsa.h>
#include <openssl/kdf.h>
#include <openssl/core_names.h>
#include "internal/cryptlib.h"
#include "crypto/pem.h"
#include "crypto/evp.h"

/*
 * Utility function: read a DWORD (4 byte unsigned integer) in little endian
 * format
 */

static unsigned int read_ledword(const unsigned char **in)
{
    const unsigned char *p = *in;
    unsigned int ret;

    ret = (unsigned int)*p++;
    ret |= (unsigned int)*p++ << 8;
    ret |= (unsigned int)*p++ << 16;
    ret |= (unsigned int)*p++ << 24;
    *in = p;
    return ret;
}

/*
 * Read a BIGNUM in little endian format. The docs say that this should take
 * up bitlen/8 bytes.
 */

static int read_lebn(const unsigned char **in, unsigned int nbyte, BIGNUM **r)
{
    *r = BN_lebin2bn(*in, nbyte, NULL);
    if (*r == NULL)
        return 0;
    *in += nbyte;
    return 1;
}

/*
 * Create an EVP_PKEY from a type specific key.
 * This takes ownership of |key|, as long as the |evp_type| is acceptable
 * (EVP_PKEY_RSA or EVP_PKEY_DSA), even if the resulting EVP_PKEY wasn't
 * created.
 */
#define isdss_to_evp_type(isdss)                                \
    (isdss == 0 ? EVP_PKEY_RSA : isdss == 1 ? EVP_PKEY_DSA : EVP_PKEY_NONE)
static EVP_PKEY *evp_pkey_new0_key(void *key, int evp_type)
{
    EVP_PKEY *pkey = NULL;

    /*
     * It's assumed that if |key| is NULL, something went wrong elsewhere
     * and suitable errors are already reported.
     */
    if (key == NULL)
        return NULL;

    if (!ossl_assert(evp_type == EVP_PKEY_RSA || evp_type == EVP_PKEY_DSA)) {
        ERR_raise(ERR_LIB_PEM, ERR_R_INTERNAL_ERROR);
        return NULL;
    }

    if ((pkey = EVP_PKEY_new()) != NULL) {
        switch (evp_type) {
        case EVP_PKEY_RSA:
            if (EVP_PKEY_set1_RSA(pkey, key))
                break;
            ERR_raise(ERR_LIB_PEM, ERR_R_EVP_LIB);
            EVP_PKEY_free(pkey);
            pkey = NULL;
            break;
#ifndef OPENSSL_NO_DSA
        case EVP_PKEY_DSA:
            if (EVP_PKEY_set1_DSA(pkey, key))
                break;
            ERR_raise(ERR_LIB_PEM, ERR_R_EVP_LIB);
            EVP_PKEY_free(pkey);
            pkey = NULL;
            break;
#endif
        }
    } else {
        ERR_raise(ERR_LIB_PEM, ERR_R_EVP_LIB);
    }

    switch (evp_type) {
    case EVP_PKEY_RSA:
        RSA_free(key);
        break;
#ifndef OPENSSL_NO_DSA
    case EVP_PKEY_DSA:
        DSA_free(key);
        break;
#endif
    }

    return pkey;
}

/* Convert private key blob to EVP_PKEY: RSA and DSA keys supported */

# define MS_PUBLICKEYBLOB        0x6
# define MS_PRIVATEKEYBLOB       0x7
# define MS_RSA1MAGIC            0x31415352L
# define MS_RSA2MAGIC            0x32415352L
# define MS_DSS1MAGIC            0x31535344L
# define MS_DSS2MAGIC            0x32535344L

# define MS_KEYALG_RSA_KEYX      0xa400
# define MS_KEYALG_DSS_SIGN      0x2200

# define MS_KEYTYPE_KEYX         0x1
# define MS_KEYTYPE_SIGN         0x2

/* The PVK file magic number: seems to spell out "bobsfile", who is Bob? */
# define MS_PVKMAGIC             0xb0b5f11eL
/* Salt length for PVK files */
# define PVK_SALTLEN             0x10
/* Maximum length in PVK header */
# define PVK_MAX_KEYLEN          102400
/* Maximum salt length */
# define PVK_MAX_SALTLEN         10240

/*
 * Read the MSBLOB header and get relevant data from it.
 *
 * |pisdss| and |pispub| have a double role, as they can be used for
 * discovery as well as to check the blob meets expectations.
 * |*pisdss| is the indicator for whether the key is a DSA key or not.
 * |*pispub| is the indicator for whether the key is public or not.
 * In both cases, the following input values apply:
 *
 * 0    Expected to not be what the variable indicates.
 * 1    Expected to be what the variable indicates.
 * -1   No expectations, this function will assign 0 or 1 depending on
 *      header data.
 */
int ossl_do_blob_header(const unsigned char **in, unsigned int length,
                        unsigned int *pmagic, unsigned int *pbitlen,
                        int *pisdss, int *pispub)
{
    const unsigned char *p = *in;

    if (length < 16)
        return 0;
    /* bType */
    switch (*p) {
    case MS_PUBLICKEYBLOB:
        if (*pispub == 0) {
            ERR_raise(ERR_LIB_PEM, PEM_R_EXPECTING_PRIVATE_KEY_BLOB);
            return 0;
        }
        *pispub = 1;
        break;

    case MS_PRIVATEKEYBLOB:
        if (*pispub == 1) {
            ERR_raise(ERR_LIB_PEM, PEM_R_EXPECTING_PUBLIC_KEY_BLOB);
            return 0;
        }
        *pispub = 0;
        break;

    default:
        return 0;
    }
    p++;
    /* Version */
    if (*p++ != 0x2) {
        ERR_raise(ERR_LIB_PEM, PEM_R_BAD_VERSION_NUMBER);
        return 0;
    }
    /* Ignore reserved, aiKeyAlg */
    p += 6;
    *pmagic = read_ledword(&p);
    *pbitlen = read_ledword(&p);

    /* Consistency check for private vs public */
    switch (*pmagic) {
    case MS_DSS1MAGIC:
    case MS_RSA1MAGIC:
        if (*pispub == 0) {
            ERR_raise(ERR_LIB_PEM, PEM_R_EXPECTING_PRIVATE_KEY_BLOB);
            return 0;
        }
        break;

    case MS_DSS2MAGIC:
    case MS_RSA2MAGIC:
        if (*pispub == 1) {
            ERR_raise(ERR_LIB_PEM, PEM_R_EXPECTING_PUBLIC_KEY_BLOB);
            return 0;
        }
        break;

    default:
        ERR_raise(ERR_LIB_PEM, PEM_R_BAD_MAGIC_NUMBER);
        return -1;
    }

    /* Check that we got the expected type */
    switch (*pmagic) {
    case MS_DSS1MAGIC:
    case MS_DSS2MAGIC:
        if (*pisdss == 0) {
            ERR_raise(ERR_LIB_PEM, PEM_R_EXPECTING_DSS_KEY_BLOB);
            return 0;
        }
        *pisdss = 1;
        break;
    case MS_RSA1MAGIC:
    case MS_RSA2MAGIC:
        if (*pisdss == 1) {
            ERR_raise(ERR_LIB_PEM, PEM_R_EXPECTING_RSA_KEY_BLOB);
            return 0;
        }
        *pisdss = 0;
        break;

    default:
        ERR_raise(ERR_LIB_PEM, PEM_R_BAD_MAGIC_NUMBER);
        return -1;
    }
    *in = p;
    return 1;
}

unsigned int ossl_blob_length(unsigned bitlen, int isdss, int ispub)
{
    unsigned int nbyte = (bitlen + 7) >> 3;
    unsigned int hnbyte = (bitlen + 15) >> 4;

    if (isdss) {

        /*
         * Expected length: 20 for q + 3 components bitlen each + 24 for seed
         * structure.
         */
        if (ispub)
            return 44 + 3 * nbyte;
        /*
         * Expected length: 20 for q, priv, 2 bitlen components + 24 for seed
         * structure.
         */
        else
            return 64 + 2 * nbyte;
    } else {
        /* Expected length: 4 for 'e' + 'n' */
        if (ispub)
            return 4 + nbyte;
        else
            /*
             * Expected length: 4 for 'e' and 7 other components. 2
             * components are bitlen size, 5 are bitlen/2
             */
            return 4 + 2 * nbyte + 5 * hnbyte;
    }

}

static void *do_b2i_key(const unsigned char **in, unsigned int length,
                        int *isdss, int *ispub)
{
    const unsigned char *p = *in;
    unsigned int bitlen, magic;
    void *key = NULL;

    if (ossl_do_blob_header(&p, length, &magic, &bitlen, isdss, ispub) <= 0) {
        ERR_raise(ERR_LIB_PEM, PEM_R_KEYBLOB_HEADER_PARSE_ERROR);
        return NULL;
    }
    length -= 16;
    if (length < ossl_blob_length(bitlen, *isdss, *ispub)) {
        ERR_raise(ERR_LIB_PEM, PEM_R_KEYBLOB_TOO_SHORT);
        return NULL;
    }
    if (!*isdss)
        key = ossl_b2i_RSA_after_header(&p, bitlen, *ispub);
#ifndef OPENSSL_NO_DSA
    else
        key = ossl_b2i_DSA_after_header(&p, bitlen, *ispub);
#endif

    if (key == NULL) {
        ERR_raise(ERR_LIB_PEM, PEM_R_UNSUPPORTED_PUBLIC_KEY_TYPE);
        return NULL;
    }

    return key;
}

EVP_PKEY *ossl_b2i(const unsigned char **in, unsigned int length, int *ispub)
{
    int isdss = -1;
    void *key = do_b2i_key(in, length, &isdss, ispub);

    return evp_pkey_new0_key(key, isdss_to_evp_type(isdss));
}

EVP_PKEY *ossl_b2i_bio(BIO *in, int *ispub)
{
    const unsigned char *p;
    unsigned char hdr_buf[16], *buf = NULL;
    unsigned int bitlen, magic, length;
    int isdss = -1;
    void *key = NULL;
    EVP_PKEY *pkey = NULL;

    if (BIO_read(in, hdr_buf, 16) != 16) {
        ERR_raise(ERR_LIB_PEM, PEM_R_KEYBLOB_TOO_SHORT);
        return NULL;
    }
    p = hdr_buf;
    if (ossl_do_blob_header(&p, 16, &magic, &bitlen, &isdss, ispub) <= 0)
        return NULL;

    length = ossl_blob_length(bitlen, isdss, *ispub);
    if (length > BLOB_MAX_LENGTH) {
        ERR_raise(ERR_LIB_PEM, PEM_R_HEADER_TOO_LONG);
        return NULL;
    }
    buf = OPENSSL_malloc(length);
    if (buf == NULL)
        goto err;
    p = buf;
    if (BIO_read(in, buf, length) != (int)length) {
        ERR_raise(ERR_LIB_PEM, PEM_R_KEYBLOB_TOO_SHORT);
        goto err;
    }

    if (!isdss)
        key = ossl_b2i_RSA_after_header(&p, bitlen, *ispub);
#ifndef OPENSSL_NO_DSA
    else
        key = ossl_b2i_DSA_after_header(&p, bitlen, *ispub);
#endif

    if (key == NULL) {
        ERR_raise(ERR_LIB_PEM, PEM_R_UNSUPPORTED_PUBLIC_KEY_TYPE);
        goto err;
    }

    pkey = evp_pkey_new0_key(key, isdss_to_evp_type(isdss));
 err:
    OPENSSL_free(buf);
    return pkey;
}

#ifndef OPENSSL_NO_DSA
DSA *ossl_b2i_DSA_after_header(const unsigned char **in, unsigned int bitlen,
                               int ispub)
{
    const unsigned char *p = *in;
    DSA *dsa = NULL;
    BN_CTX *ctx = NULL;
    BIGNUM *pbn = NULL, *qbn = NULL, *gbn = NULL, *priv_key = NULL;
    BIGNUM *pub_key = NULL;
    unsigned int nbyte = (bitlen + 7) >> 3;

    dsa = DSA_new();
    if (dsa == NULL)
        goto dsaerr;
    if (!read_lebn(&p, nbyte, &pbn))
        goto bnerr;

    if (!read_lebn(&p, 20, &qbn))
        goto bnerr;

    if (!read_lebn(&p, nbyte, &gbn))
        goto bnerr;

    if (ispub) {
        if (!read_lebn(&p, nbyte, &pub_key))
            goto bnerr;
    } else {
        if (!read_lebn(&p, 20, &priv_key))
            goto bnerr;

        /* Set constant time flag before public key calculation */
        BN_set_flags(priv_key, BN_FLG_CONSTTIME);

        /* Calculate public key */
        pub_key = BN_new();
        if (pub_key == NULL)
            goto bnerr;
        if ((ctx = BN_CTX_new()) == NULL)
            goto bnerr;

        if (!BN_mod_exp(pub_key, gbn, priv_key, pbn, ctx))
            goto bnerr;

        BN_CTX_free(ctx);
        ctx = NULL;
    }
    if (!DSA_set0_pqg(dsa, pbn, qbn, gbn))
        goto dsaerr;
    pbn = qbn = gbn = NULL;
    if (!DSA_set0_key(dsa, pub_key, priv_key))
        goto dsaerr;
    pub_key = priv_key = NULL;

    *in = p;
    return dsa;

 dsaerr:
    ERR_raise(ERR_LIB_PEM, ERR_R_DSA_LIB);
    goto err;
 bnerr:
    ERR_raise(ERR_LIB_PEM, ERR_R_BN_LIB);

 err:
    DSA_free(dsa);
    BN_free(pbn);
    BN_free(qbn);
    BN_free(gbn);
    BN_free(pub_key);
    BN_free(priv_key);
    BN_CTX_free(ctx);
    return NULL;
}
#endif

RSA *ossl_b2i_RSA_after_header(const unsigned char **in, unsigned int bitlen,
                               int ispub)
{
    const unsigned char *pin = *in;
    BIGNUM *e = NULL, *n = NULL, *d = NULL;
    BIGNUM *p = NULL, *q = NULL, *dmp1 = NULL, *dmq1 = NULL, *iqmp = NULL;
    RSA *rsa = NULL;
    unsigned int nbyte = (bitlen + 7) >> 3;
    unsigned int hnbyte = (bitlen + 15) >> 4;

    rsa = RSA_new();
    if (rsa == NULL)
        goto rsaerr;
    e = BN_new();
    if (e == NULL)
        goto bnerr;
    if (!BN_set_word(e, read_ledword(&pin)))
        goto bnerr;
    if (!read_lebn(&pin, nbyte, &n))
        goto bnerr;
    if (!ispub) {
        if (!read_lebn(&pin, hnbyte, &p))
            goto bnerr;
        if (!read_lebn(&pin, hnbyte, &q))
            goto bnerr;
        if (!read_lebn(&pin, hnbyte, &dmp1))
            goto bnerr;
        if (!read_lebn(&pin, hnbyte, &dmq1))
            goto bnerr;
        if (!read_lebn(&pin, hnbyte, &iqmp))
            goto bnerr;
        if (!read_lebn(&pin, nbyte, &d))
            goto bnerr;
        if (!RSA_set0_factors(rsa, p, q))
            goto rsaerr;
        p = q = NULL;
        if (!RSA_set0_crt_params(rsa, dmp1, dmq1, iqmp))
            goto rsaerr;
        dmp1 = dmq1 = iqmp = NULL;
    }
    if (!RSA_set0_key(rsa, n, e, d))
        goto rsaerr;
    n = e = d = NULL;

    *in = pin;
    return rsa;

 rsaerr:
    ERR_raise(ERR_LIB_PEM, ERR_R_RSA_LIB);
    goto err;
 bnerr:
    ERR_raise(ERR_LIB_PEM, ERR_R_BN_LIB);

 err:
    BN_free(e);
    BN_free(n);
    BN_free(p);
    BN_free(q);
    BN_free(dmp1);
    BN_free(dmq1);
    BN_free(iqmp);
    BN_free(d);
    RSA_free(rsa);
    return NULL;
}

EVP_PKEY *b2i_PrivateKey(const unsigned char **in, long length)
{
    int ispub = 0;

    return ossl_b2i(in, length, &ispub);
}

EVP_PKEY *b2i_PublicKey(const unsigned char **in, long length)
{
    int ispub = 1;

    return ossl_b2i(in, length, &ispub);
}

EVP_PKEY *b2i_PrivateKey_bio(BIO *in)
{
    int ispub = 0;

    return ossl_b2i_bio(in, &ispub);
}

EVP_PKEY *b2i_PublicKey_bio(BIO *in)
{
    int ispub = 1;

    return ossl_b2i_bio(in, &ispub);
}

static void write_ledword(unsigned char **out, unsigned int dw)
{
    unsigned char *p = *out;

    *p++ = dw & 0xff;
    *p++ = (dw >> 8) & 0xff;
    *p++ = (dw >> 16) & 0xff;
    *p++ = (dw >> 24) & 0xff;
    *out = p;
}

static void write_lebn(unsigned char **out, const BIGNUM *bn, int len)
{
    BN_bn2lebinpad(bn, *out, len);
    *out += len;
}

static int check_bitlen_rsa(const RSA *rsa, int ispub, unsigned int *magic);
static void write_rsa(unsigned char **out, const RSA *rsa, int ispub);

#ifndef OPENSSL_NO_DSA
static int check_bitlen_dsa(const DSA *dsa, int ispub, unsigned int *magic);
static void write_dsa(unsigned char **out, const DSA *dsa, int ispub);
#endif

static int do_i2b(unsigned char **out, const EVP_PKEY *pk, int ispub)
{
    unsigned char *p;
    unsigned int bitlen = 0, magic = 0, keyalg = 0;
    int outlen = -1, noinc = 0;

    if (EVP_PKEY_is_a(pk, "RSA")) {
        bitlen = check_bitlen_rsa(EVP_PKEY_get0_RSA(pk), ispub, &magic);
        keyalg = MS_KEYALG_RSA_KEYX;
#ifndef OPENSSL_NO_DSA
    } else if (EVP_PKEY_is_a(pk, "DSA")) {
        bitlen = check_bitlen_dsa(EVP_PKEY_get0_DSA(pk), ispub, &magic);
        keyalg = MS_KEYALG_DSS_SIGN;
#endif
    }
    if (bitlen == 0) {
        goto end;
    }
    outlen = 16
        + ossl_blob_length(bitlen, keyalg == MS_KEYALG_DSS_SIGN ? 1 : 0, ispub);
    if (out == NULL)
        goto end;
    if (*out)
        p = *out;
    else {
        if ((p = OPENSSL_malloc(outlen)) == NULL) {
            outlen = -1;
            goto end;
        }
        *out = p;
        noinc = 1;
    }
    if (ispub)
        *p++ = MS_PUBLICKEYBLOB;
    else
        *p++ = MS_PRIVATEKEYBLOB;
    *p++ = 0x2;
    *p++ = 0;
    *p++ = 0;
    write_ledword(&p, keyalg);
    write_ledword(&p, magic);
    write_ledword(&p, bitlen);
    if (keyalg == MS_KEYALG_RSA_KEYX)
        write_rsa(&p, EVP_PKEY_get0_RSA(pk), ispub);
#ifndef OPENSSL_NO_DSA
    else
        write_dsa(&p, EVP_PKEY_get0_DSA(pk), ispub);
#endif
    if (!noinc)
        *out += outlen;
 end:
    return outlen;
}

static int do_i2b_bio(BIO *out, const EVP_PKEY *pk, int ispub)
{
    unsigned char *tmp = NULL;
    int outlen, wrlen;

    outlen = do_i2b(&tmp, pk, ispub);
    if (outlen < 0)
        return -1;
    wrlen = BIO_write(out, tmp, outlen);
    OPENSSL_free(tmp);
    if (wrlen == outlen)
        return outlen;
    return -1;
}

static int check_bitlen_rsa(const RSA *rsa, int ispub, unsigned int *pmagic)
{
    int nbyte, hnbyte, bitlen;
    const BIGNUM *e;

    RSA_get0_key(rsa, NULL, &e, NULL);
    if (BN_num_bits(e) > 32)
        goto badkey;
    bitlen = RSA_bits(rsa);
    nbyte = RSA_size(rsa);
    hnbyte = (bitlen + 15) >> 4;
    if (ispub) {
        *pmagic = MS_RSA1MAGIC;
        return bitlen;
    } else {
        const BIGNUM *d, *p, *q, *iqmp, *dmp1, *dmq1;

        *pmagic = MS_RSA2MAGIC;

        /*
         * For private key each component must fit within nbyte or hnbyte.
         */
        RSA_get0_key(rsa, NULL, NULL, &d);
        if (BN_num_bytes(d) > nbyte)
            goto badkey;
        RSA_get0_factors(rsa, &p, &q);
        RSA_get0_crt_params(rsa, &dmp1, &dmq1, &iqmp);
        if ((BN_num_bytes(iqmp) > hnbyte)
            || (BN_num_bytes(p) > hnbyte)
            || (BN_num_bytes(q) > hnbyte)
            || (BN_num_bytes(dmp1) > hnbyte)
            || (BN_num_bytes(dmq1) > hnbyte))
            goto badkey;
    }
    return bitlen;
 badkey:
    ERR_raise(ERR_LIB_PEM, PEM_R_UNSUPPORTED_KEY_COMPONENTS);
    return 0;
}

static void write_rsa(unsigned char **out, const RSA *rsa, int ispub)
{
    int nbyte, hnbyte;
    const BIGNUM *n, *d, *e, *p, *q, *iqmp, *dmp1, *dmq1;

    nbyte = RSA_size(rsa);
    hnbyte = (RSA_bits(rsa) + 15) >> 4;
    RSA_get0_key(rsa, &n, &e, &d);
    write_lebn(out, e, 4);
    write_lebn(out, n, nbyte);
    if (ispub)
        return;
    RSA_get0_factors(rsa, &p, &q);
    RSA_get0_crt_params(rsa, &dmp1, &dmq1, &iqmp);
    write_lebn(out, p, hnbyte);
    write_lebn(out, q, hnbyte);
    write_lebn(out, dmp1, hnbyte);
    write_lebn(out, dmq1, hnbyte);
    write_lebn(out, iqmp, hnbyte);
    write_lebn(out, d, nbyte);
}

#ifndef OPENSSL_NO_DSA
static int check_bitlen_dsa(const DSA *dsa, int ispub, unsigned int *pmagic)
{
    int bitlen;
    const BIGNUM *p = NULL, *q = NULL, *g = NULL;
    const BIGNUM *pub_key = NULL, *priv_key = NULL;

    DSA_get0_pqg(dsa, &p, &q, &g);
    DSA_get0_key(dsa, &pub_key, &priv_key);
    bitlen = BN_num_bits(p);
    if ((bitlen & 7) || (BN_num_bits(q) != 160)
        || (BN_num_bits(g) > bitlen))
        goto badkey;
    if (ispub) {
        if (BN_num_bits(pub_key) > bitlen)
            goto badkey;
        *pmagic = MS_DSS1MAGIC;
    } else {
        if (BN_num_bits(priv_key) > 160)
            goto badkey;
        *pmagic = MS_DSS2MAGIC;
    }

    return bitlen;
 badkey:
    ERR_raise(ERR_LIB_PEM, PEM_R_UNSUPPORTED_KEY_COMPONENTS);
    return 0;
}

static void write_dsa(unsigned char **out, const DSA *dsa, int ispub)
{
    int nbyte;
    const BIGNUM *p = NULL, *q = NULL, *g = NULL;
    const BIGNUM *pub_key = NULL, *priv_key = NULL;

    DSA_get0_pqg(dsa, &p, &q, &g);
    DSA_get0_key(dsa, &pub_key, &priv_key);
    nbyte = BN_num_bytes(p);
    write_lebn(out, p, nbyte);
    write_lebn(out, q, 20);
    write_lebn(out, g, nbyte);
    if (ispub)
        write_lebn(out, pub_key, nbyte);
    else
        write_lebn(out, priv_key, 20);
    /* Set "invalid" for seed structure values */
    memset(*out, 0xff, 24);
    *out += 24;
    return;
}
#endif

int i2b_PrivateKey_bio(BIO *out, const EVP_PKEY *pk)
{
    return do_i2b_bio(out, pk, 0);
}

int i2b_PublicKey_bio(BIO *out, const EVP_PKEY *pk)
{
    return do_i2b_bio(out, pk, 1);
}

int ossl_do_PVK_header(const unsigned char **in, unsigned int length,
                       int skip_magic, int *isdss,
                       unsigned int *psaltlen, unsigned int *pkeylen)
{
    const unsigned char *p = *in;
    unsigned int pvk_magic, is_encrypted;

    if (skip_magic) {
        if (length < 20) {
            ERR_raise(ERR_LIB_PEM, PEM_R_PVK_TOO_SHORT);
            return 0;
        }
    } else {
        if (length < 24) {
            ERR_raise(ERR_LIB_PEM, PEM_R_PVK_TOO_SHORT);
            return 0;
        }
        pvk_magic = read_ledword(&p);
        if (pvk_magic != MS_PVKMAGIC) {
            ERR_raise(ERR_LIB_PEM, PEM_R_BAD_MAGIC_NUMBER);
            return 0;
        }
    }
    /* Skip reserved */
    p += 4;
    /* Check the key type */
    switch (read_ledword(&p)) {
    case MS_KEYTYPE_KEYX:
        if (*isdss == 1) {
            ERR_raise(ERR_LIB_PEM, PEM_R_EXPECTING_RSA_KEY_BLOB);
            return 0;
        }
        *isdss = 0;
        break;
    case MS_KEYTYPE_SIGN:
        if (*isdss == 0) {
            ERR_raise(ERR_LIB_PEM, PEM_R_EXPECTING_DSS_KEY_BLOB);
            return 0;
        }
        *isdss = 1;
        break;
    default:
        ERR_raise(ERR_LIB_PEM, PEM_R_UNSUPPORTED_PVK_KEY_TYPE);
        return 0;
    }
    is_encrypted = read_ledword(&p);
    *psaltlen = read_ledword(&p);
    *pkeylen = read_ledword(&p);

    if (*pkeylen > PVK_MAX_KEYLEN || *psaltlen > PVK_MAX_SALTLEN)
        return 0;

    if (is_encrypted && *psaltlen == 0) {
        ERR_raise(ERR_LIB_PEM, PEM_R_INCONSISTENT_HEADER);
        return 0;
    }

    *in = p;
    return 1;
}

#ifndef OPENSSL_NO_RC4
static int derive_pvk_key(unsigned char *key, size_t keylen,
                          const unsigned char *salt, unsigned int saltlen,
                          const unsigned char *pass, int passlen,
                          OSSL_LIB_CTX *libctx, const char *propq)
{
    EVP_KDF *kdf;
    EVP_KDF_CTX *ctx;
    OSSL_PARAM params[5], *p = params;
    int rv;

    if ((kdf = EVP_KDF_fetch(libctx, "PVKKDF", propq)) == NULL)
        return 0;
    ctx = EVP_KDF_CTX_new(kdf);
    EVP_KDF_free(kdf);
    if (ctx == NULL)
        return 0;

    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_SALT,
                                             (void *)salt, saltlen);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_PASSWORD,
                                             (void *)pass, passlen);
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_DIGEST, SN_sha1, 0);
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_PROPERTIES,
                                            (char *)propq, 0);
    *p = OSSL_PARAM_construct_end();

    rv = EVP_KDF_derive(ctx, key, keylen, params);
    EVP_KDF_CTX_free(ctx);
    return rv;
}
#endif

static void *do_PVK_body_key(const unsigned char **in,
                             unsigned int saltlen, unsigned int keylen,
                             pem_password_cb *cb, void *u,
                             int *isdss, int *ispub,
                             OSSL_LIB_CTX *libctx, const char *propq)
{
    const unsigned char *p = *in;
    unsigned char *enctmp = NULL;
    unsigned char keybuf[20];
    void *key = NULL;
#ifndef OPENSSL_NO_RC4
    EVP_CIPHER *rc4 = NULL;
#endif
    EVP_CIPHER_CTX *cctx = EVP_CIPHER_CTX_new();

    if (cctx == NULL) {
        ERR_raise(ERR_LIB_PEM, ERR_R_EVP_LIB);
        goto err;
    }

    if (saltlen) {
#ifndef OPENSSL_NO_RC4
        unsigned int magic;
        char psbuf[PEM_BUFSIZE];
        int enctmplen, inlen;
        unsigned char *q;

        if (cb)
            inlen = cb(psbuf, PEM_BUFSIZE, 0, u);
        else
            inlen = PEM_def_callback(psbuf, PEM_BUFSIZE, 0, u);
        if (inlen < 0) {
            ERR_raise(ERR_LIB_PEM, PEM_R_BAD_PASSWORD_READ);
            goto err;
        }
        enctmp = OPENSSL_malloc(keylen + 8);
        if (enctmp == NULL)
            goto err;
        if (!derive_pvk_key(keybuf, sizeof(keybuf), p, saltlen,
                            (unsigned char *)psbuf, inlen, libctx, propq))
            goto err;
        p += saltlen;
        /* Copy BLOBHEADER across, decrypt rest */
        memcpy(enctmp, p, 8);
        p += 8;
        if (keylen < 8) {
            ERR_raise(ERR_LIB_PEM, PEM_R_PVK_TOO_SHORT);
            goto err;
        }
        inlen = keylen - 8;
        q = enctmp + 8;
        if ((rc4 = EVP_CIPHER_fetch(libctx, "RC4", propq)) == NULL)
            goto err;
        if (!EVP_DecryptInit_ex(cctx, rc4, NULL, keybuf, NULL))
            goto err;
        if (!EVP_DecryptUpdate(cctx, q, &enctmplen, p, inlen))
            goto err;
        if (!EVP_DecryptFinal_ex(cctx, q + enctmplen, &enctmplen))
            goto err;
        magic = read_ledword((const unsigned char **)&q);
        if (magic != MS_RSA2MAGIC && magic != MS_DSS2MAGIC) {
            q = enctmp + 8;
            memset(keybuf + 5, 0, 11);
            if (!EVP_DecryptInit_ex(cctx, rc4, NULL, keybuf, NULL))
                goto err;
            if (!EVP_DecryptUpdate(cctx, q, &enctmplen, p, inlen))
                goto err;
            if (!EVP_DecryptFinal_ex(cctx, q + enctmplen, &enctmplen))
                goto err;
            magic = read_ledword((const unsigned char **)&q);
            if (magic != MS_RSA2MAGIC && magic != MS_DSS2MAGIC) {
                ERR_raise(ERR_LIB_PEM, PEM_R_BAD_DECRYPT);
                goto err;
            }
        }
        p = enctmp;
#else
        ERR_raise(ERR_LIB_PEM, PEM_R_UNSUPPORTED_CIPHER);
        goto err;
#endif
    }

    key = do_b2i_key(&p, keylen, isdss, ispub);
 err:
    EVP_CIPHER_CTX_free(cctx);
#ifndef OPENSSL_NO_RC4
    EVP_CIPHER_free(rc4);
#endif
    if (enctmp != NULL) {
        OPENSSL_cleanse(keybuf, sizeof(keybuf));
        OPENSSL_free(enctmp);
    }
    return key;
}

static void *do_PVK_key_bio(BIO *in, pem_password_cb *cb, void *u,
                            int *isdss, int *ispub,
                            OSSL_LIB_CTX *libctx, const char *propq)
{
    unsigned char pvk_hdr[24], *buf = NULL;
    const unsigned char *p;
    int buflen;
    void *key = NULL;
    unsigned int saltlen, keylen;

    if (BIO_read(in, pvk_hdr, 24) != 24) {
        ERR_raise(ERR_LIB_PEM, PEM_R_PVK_DATA_TOO_SHORT);
        return NULL;
    }
    p = pvk_hdr;

    if (!ossl_do_PVK_header(&p, 24, 0, isdss, &saltlen, &keylen))
        return 0;
    buflen = (int)keylen + saltlen;
    buf = OPENSSL_malloc(buflen);
    if (buf == NULL)
        return 0;
    p = buf;
    if (BIO_read(in, buf, buflen) != buflen) {
        ERR_raise(ERR_LIB_PEM, PEM_R_PVK_DATA_TOO_SHORT);
        goto err;
    }
    key = do_PVK_body_key(&p, saltlen, keylen, cb, u, isdss, ispub, libctx, propq);

 err:
    OPENSSL_clear_free(buf, buflen);
    return key;
}

#ifndef OPENSSL_NO_DSA
DSA *b2i_DSA_PVK_bio_ex(BIO *in, pem_password_cb *cb, void *u,
                        OSSL_LIB_CTX *libctx, const char *propq)
{
    int isdss = 1;
    int ispub = 0;               /* PVK keys are always private */

    return do_PVK_key_bio(in, cb, u, &isdss, &ispub, libctx, propq);
}

DSA *b2i_DSA_PVK_bio(BIO *in, pem_password_cb *cb, void *u)
{
    return b2i_DSA_PVK_bio_ex(in, cb, u, NULL, NULL);
}
#endif

RSA *b2i_RSA_PVK_bio_ex(BIO *in, pem_password_cb *cb, void *u,
                        OSSL_LIB_CTX *libctx, const char *propq)
{
    int isdss = 0;
    int ispub = 0;               /* PVK keys are always private */

    return do_PVK_key_bio(in, cb, u, &isdss, &ispub, libctx, propq);
}

RSA *b2i_RSA_PVK_bio(BIO *in, pem_password_cb *cb, void *u)
{
    return b2i_RSA_PVK_bio_ex(in, cb, u, NULL, NULL);
}

EVP_PKEY *b2i_PVK_bio_ex(BIO *in, pem_password_cb *cb, void *u,
                         OSSL_LIB_CTX *libctx, const char *propq)
{
    int isdss = -1;
    int ispub = -1;
    void *key = do_PVK_key_bio(in, cb, u, &isdss, &ispub, NULL, NULL);

    return evp_pkey_new0_key(key, isdss_to_evp_type(isdss));
}

EVP_PKEY *b2i_PVK_bio(BIO *in, pem_password_cb *cb, void *u)
{
    return b2i_PVK_bio_ex(in, cb, u, NULL, NULL);
}

static int i2b_PVK(unsigned char **out, const EVP_PKEY *pk, int enclevel,
                   pem_password_cb *cb, void *u, OSSL_LIB_CTX *libctx,
                   const char *propq)
{
    int ret = -1;
    int outlen = 24, pklen;
    unsigned char *p = NULL, *start = NULL;
    EVP_CIPHER_CTX *cctx = NULL;
#ifndef OPENSSL_NO_RC4
    unsigned char *salt = NULL;
    EVP_CIPHER *rc4 = NULL;
#endif

    if (enclevel)
        outlen += PVK_SALTLEN;
    pklen = do_i2b(NULL, pk, 0);
    if (pklen < 0)
        return -1;
    outlen += pklen;
    if (out == NULL)
        return outlen;
    if (*out != NULL) {
        p = *out;
    } else {
        start = p = OPENSSL_malloc(outlen);
        if (p == NULL)
            return -1;
    }

    cctx = EVP_CIPHER_CTX_new();
    if (cctx == NULL)
        goto error;

    write_ledword(&p, MS_PVKMAGIC);
    write_ledword(&p, 0);
    if (EVP_PKEY_get_id(pk) == EVP_PKEY_RSA)
        write_ledword(&p, MS_KEYTYPE_KEYX);
#ifndef OPENSSL_NO_DSA
    else
        write_ledword(&p, MS_KEYTYPE_SIGN);
#endif
    write_ledword(&p, enclevel ? 1 : 0);
    write_ledword(&p, enclevel ? PVK_SALTLEN : 0);
    write_ledword(&p, pklen);
    if (enclevel) {
#ifndef OPENSSL_NO_RC4
        if (RAND_bytes_ex(libctx, p, PVK_SALTLEN, 0) <= 0)
            goto error;
        salt = p;
        p += PVK_SALTLEN;
#endif
    }
    do_i2b(&p, pk, 0);
    if (enclevel != 0) {
#ifndef OPENSSL_NO_RC4
        char psbuf[PEM_BUFSIZE];
        unsigned char keybuf[20];
        int enctmplen, inlen;
        if (cb)
            inlen = cb(psbuf, PEM_BUFSIZE, 1, u);
        else
            inlen = PEM_def_callback(psbuf, PEM_BUFSIZE, 1, u);
        if (inlen <= 0) {
            ERR_raise(ERR_LIB_PEM, PEM_R_BAD_PASSWORD_READ);
            goto error;
        }
        if (!derive_pvk_key(keybuf, sizeof(keybuf), salt, PVK_SALTLEN,
                            (unsigned char *)psbuf, inlen, libctx, propq))
            goto error;
        if ((rc4 = EVP_CIPHER_fetch(libctx, "RC4", propq)) == NULL)
            goto error;
        if (enclevel == 1)
            memset(keybuf + 5, 0, 11);
        p = salt + PVK_SALTLEN + 8;
        if (!EVP_EncryptInit_ex(cctx, rc4, NULL, keybuf, NULL))
            goto error;
        OPENSSL_cleanse(keybuf, 20);
        if (!EVP_EncryptUpdate(cctx, p, &enctmplen, p, pklen - 8))
            goto error;
        if (!EVP_EncryptFinal_ex(cctx, p + enctmplen, &enctmplen))
            goto error;
#else
        ERR_raise(ERR_LIB_PEM, PEM_R_UNSUPPORTED_CIPHER);
        goto error;
#endif
    }

    if (*out == NULL)
        *out = start;
    ret = outlen;
 error:
    EVP_CIPHER_CTX_free(cctx);
#ifndef OPENSSL_NO_RC4
    EVP_CIPHER_free(rc4);
#endif
    if (*out == NULL)
        OPENSSL_free(start);

    return ret;
}

int i2b_PVK_bio_ex(BIO *out, const EVP_PKEY *pk, int enclevel,
                   pem_password_cb *cb, void *u, OSSL_LIB_CTX *libctx,
                   const char *propq)
{
    unsigned char *tmp = NULL;
    int outlen, wrlen;

    outlen = i2b_PVK(&tmp, pk, enclevel, cb, u, libctx, propq);
    if (outlen < 0)
        return -1;
    wrlen = BIO_write(out, tmp, outlen);
    OPENSSL_free(tmp);
    if (wrlen == outlen) {
        return outlen;
    }
    ERR_raise(ERR_LIB_PEM, PEM_R_BIO_WRITE_FAILURE);
    return -1;
}

int i2b_PVK_bio(BIO *out, const EVP_PKEY *pk, int enclevel,
                pem_password_cb *cb, void *u)
{
    return i2b_PVK_bio_ex(out, pk, enclevel, cb, u, NULL, NULL);
}

