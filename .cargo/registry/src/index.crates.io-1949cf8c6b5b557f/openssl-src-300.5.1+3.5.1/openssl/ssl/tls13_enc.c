/*
 * Copyright 2016-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdlib.h>
#include "ssl_local.h"
#include "internal/ktls.h"
#include "record/record_local.h"
#include "internal/cryptlib.h"
#include "internal/ssl_unwrap.h"
#include <openssl/evp.h>
#include <openssl/kdf.h>
#include <openssl/core_names.h>

#define TLS13_MAX_LABEL_LEN     249

/* ASCII: "tls13 ", in hex for EBCDIC compatibility */
static const unsigned char label_prefix[] = "\x74\x6C\x73\x31\x33\x20";

/*
 * Given a |secret|; a |label| of length |labellen|; and |data| of length
 * |datalen| (e.g. typically a hash of the handshake messages), derive a new
 * secret |outlen| bytes long and store it in the location pointed to be |out|.
 * The |data| value may be zero length. Any errors will be treated as fatal if
 * |fatal| is set. Returns 1 on success  0 on failure.
 * If |raise_error| is set, ERR_raise is called on failure.
 */
int tls13_hkdf_expand_ex(OSSL_LIB_CTX *libctx, const char *propq,
                         const EVP_MD *md,
                         const unsigned char *secret,
                         const unsigned char *label, size_t labellen,
                         const unsigned char *data, size_t datalen,
                         unsigned char *out, size_t outlen, int raise_error)
{
    EVP_KDF *kdf = EVP_KDF_fetch(libctx, OSSL_KDF_NAME_TLS1_3_KDF, propq);
    EVP_KDF_CTX *kctx;
    OSSL_PARAM params[7], *p = params;
    int mode = EVP_PKEY_HKDEF_MODE_EXPAND_ONLY;
    const char *mdname = EVP_MD_get0_name(md);
    int ret;
    size_t hashlen;

    kctx = EVP_KDF_CTX_new(kdf);
    EVP_KDF_free(kdf);
    if (kctx == NULL)
        return 0;

    if (labellen > TLS13_MAX_LABEL_LEN) {
        if (raise_error)
            /*
             * Probably we have been called from SSL_export_keying_material(),
             * or SSL_export_keying_material_early().
             */
            ERR_raise(ERR_LIB_SSL, SSL_R_TLS_ILLEGAL_EXPORTER_LABEL);

        EVP_KDF_CTX_free(kctx);
        return 0;
    }

    if ((ret = EVP_MD_get_size(md)) <= 0) {
        EVP_KDF_CTX_free(kctx);
        if (raise_error)
            ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        return 0;
    }
    hashlen = (size_t)ret;

    *p++ = OSSL_PARAM_construct_int(OSSL_KDF_PARAM_MODE, &mode);
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_DIGEST,
                                            (char *)mdname, 0);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_KEY,
                                             (unsigned char *)secret, hashlen);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_PREFIX,
                                             (unsigned char *)label_prefix,
                                             sizeof(label_prefix) - 1);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_LABEL,
                                             (unsigned char *)label, labellen);
    if (data != NULL)
        *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_DATA,
                                                 (unsigned char *)data,
                                                 datalen);
    *p++ = OSSL_PARAM_construct_end();

    ret = EVP_KDF_derive(kctx, out, outlen, params) <= 0;
    EVP_KDF_CTX_free(kctx);

    if (ret != 0) {
        if (raise_error)
            ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
    }

    return ret == 0;
}

int tls13_hkdf_expand(SSL_CONNECTION *s, const EVP_MD *md,
                      const unsigned char *secret,
                      const unsigned char *label, size_t labellen,
                      const unsigned char *data, size_t datalen,
                      unsigned char *out, size_t outlen, int fatal)
{
    int ret;
    SSL_CTX *sctx = SSL_CONNECTION_GET_CTX(s);

    ret = tls13_hkdf_expand_ex(sctx->libctx, sctx->propq, md,
                               secret, label, labellen, data, datalen,
                               out, outlen, !fatal);
    if (ret == 0 && fatal)
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);

    return ret;
}

/*
 * Given a |secret| generate a |key| of length |keylen| bytes. Returns 1 on
 * success  0 on failure.
 */
int tls13_derive_key(SSL_CONNECTION *s, const EVP_MD *md,
                     const unsigned char *secret,
                     unsigned char *key, size_t keylen)
{
    /* ASCII: "key", in hex for EBCDIC compatibility */
    static const unsigned char keylabel[] = "\x6B\x65\x79";

    return tls13_hkdf_expand(s, md, secret, keylabel, sizeof(keylabel) - 1,
                             NULL, 0, key, keylen, 1);
}

/*
 * Given a |secret| generate an |iv| of length |ivlen| bytes. Returns 1 on
 * success  0 on failure.
 */
int tls13_derive_iv(SSL_CONNECTION *s, const EVP_MD *md,
                    const unsigned char *secret,
                    unsigned char *iv, size_t ivlen)
{
    /* ASCII: "iv", in hex for EBCDIC compatibility */
    static const unsigned char ivlabel[] = "\x69\x76";

    return tls13_hkdf_expand(s, md, secret, ivlabel, sizeof(ivlabel) - 1,
                             NULL, 0, iv, ivlen, 1);
}

int tls13_derive_finishedkey(SSL_CONNECTION *s, const EVP_MD *md,
                             const unsigned char *secret,
                             unsigned char *fin, size_t finlen)
{
    /* ASCII: "finished", in hex for EBCDIC compatibility */
    static const unsigned char finishedlabel[] = "\x66\x69\x6E\x69\x73\x68\x65\x64";

    return tls13_hkdf_expand(s, md, secret, finishedlabel,
                             sizeof(finishedlabel) - 1, NULL, 0, fin, finlen, 1);
}

/*
 * Given the previous secret |prevsecret| and a new input secret |insecret| of
 * length |insecretlen|, generate a new secret and store it in the location
 * pointed to by |outsecret|. Returns 1 on success  0 on failure.
 */
int tls13_generate_secret(SSL_CONNECTION *s, const EVP_MD *md,
                          const unsigned char *prevsecret,
                          const unsigned char *insecret,
                          size_t insecretlen,
                          unsigned char *outsecret)
{
    size_t mdlen;
    int mdleni;
    int ret;
    EVP_KDF *kdf;
    EVP_KDF_CTX *kctx;
    OSSL_PARAM params[7], *p = params;
    int mode = EVP_PKEY_HKDEF_MODE_EXTRACT_ONLY;
    const char *mdname = EVP_MD_get0_name(md);
    /* ASCII: "derived", in hex for EBCDIC compatibility */
    static const char derived_secret_label[] = "\x64\x65\x72\x69\x76\x65\x64";
    SSL_CTX *sctx = SSL_CONNECTION_GET_CTX(s);

    kdf = EVP_KDF_fetch(sctx->libctx, OSSL_KDF_NAME_TLS1_3_KDF, sctx->propq);
    kctx = EVP_KDF_CTX_new(kdf);
    EVP_KDF_free(kdf);
    if (kctx == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    mdleni = EVP_MD_get_size(md);
    /* Ensure cast to size_t is safe */
    if (!ossl_assert(mdleni > 0)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        EVP_KDF_CTX_free(kctx);
        return 0;
    }
    mdlen = (size_t)mdleni;

    *p++ = OSSL_PARAM_construct_int(OSSL_KDF_PARAM_MODE, &mode);
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_DIGEST,
                                            (char *)mdname, 0);
    if (insecret != NULL)
        *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_KEY,
                                                 (unsigned char *)insecret,
                                                 insecretlen);
    if (prevsecret != NULL)
        *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_SALT,
                                                 (unsigned char *)prevsecret, mdlen);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_PREFIX,
                                             (unsigned char *)label_prefix,
                                             sizeof(label_prefix) - 1);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_LABEL,
                                             (unsigned char *)derived_secret_label,
                                             sizeof(derived_secret_label) - 1);
    *p++ = OSSL_PARAM_construct_end();

    ret = EVP_KDF_derive(kctx, outsecret, mdlen, params) <= 0;

    if (ret != 0)
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);

    EVP_KDF_CTX_free(kctx);
    return ret == 0;
}

/*
 * Given an input secret |insecret| of length |insecretlen| generate the
 * handshake secret. This requires the early secret to already have been
 * generated. Returns 1 on success  0 on failure.
 */
int tls13_generate_handshake_secret(SSL_CONNECTION *s,
                                    const unsigned char *insecret,
                                    size_t insecretlen)
{
    /* Calls SSLfatal() if required */
    return tls13_generate_secret(s, ssl_handshake_md(s), s->early_secret,
                                 insecret, insecretlen,
                                 (unsigned char *)&s->handshake_secret);
}

/*
 * Given the handshake secret |prev| of length |prevlen| generate the master
 * secret and store its length in |*secret_size|. Returns 1 on success  0 on
 * failure.
 */
int tls13_generate_master_secret(SSL_CONNECTION *s, unsigned char *out,
                                 unsigned char *prev, size_t prevlen,
                                 size_t *secret_size)
{
    const EVP_MD *md = ssl_handshake_md(s);
    int md_size;

    md_size = EVP_MD_get_size(md);
    if (md_size <= 0) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return 0;
    }
    *secret_size = (size_t)md_size;
    /* Calls SSLfatal() if required */
    return tls13_generate_secret(s, md, prev, NULL, 0, out);
}

/*
 * Generates the mac for the Finished message. Returns the length of the MAC or
 * 0 on error.
 */
size_t tls13_final_finish_mac(SSL_CONNECTION *s, const char *str, size_t slen,
                             unsigned char *out)
{
    const EVP_MD *md = ssl_handshake_md(s);
    const char *mdname = EVP_MD_get0_name(md);
    unsigned char hash[EVP_MAX_MD_SIZE];
    unsigned char finsecret[EVP_MAX_MD_SIZE];
    unsigned char *key = NULL;
    size_t len = 0, hashlen;
    OSSL_PARAM params[2], *p = params;
    SSL_CTX *sctx = SSL_CONNECTION_GET_CTX(s);

    if (md == NULL)
        return 0;

    /* Safe to cast away const here since we're not "getting" any data */
    if (sctx->propq != NULL)
        *p++ = OSSL_PARAM_construct_utf8_string(OSSL_ALG_PARAM_PROPERTIES,
                                                (char *)sctx->propq,
                                                0);
    *p = OSSL_PARAM_construct_end();

    if (!ssl_handshake_hash(s, hash, sizeof(hash), &hashlen)) {
        /* SSLfatal() already called */
        goto err;
    }

    if (str == SSL_CONNECTION_GET_SSL(s)->method->ssl3_enc->server_finished_label) {
        key = s->server_finished_secret;
    } else if (SSL_IS_FIRST_HANDSHAKE(s)) {
        key = s->client_finished_secret;
    } else {
        if (!tls13_derive_finishedkey(s, md,
                                      s->client_app_traffic_secret,
                                      finsecret, hashlen))
            goto err;
        key = finsecret;
    }

    if (!EVP_Q_mac(sctx->libctx, "HMAC", sctx->propq, mdname,
                   params, key, hashlen, hash, hashlen,
                   /* outsize as per sizeof(peer_finish_md) */
                   out, EVP_MAX_MD_SIZE * 2, &len)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }

 err:
    OPENSSL_cleanse(finsecret, sizeof(finsecret));
    return len;
}

/*
 * There isn't really a key block in TLSv1.3, but we still need this function
 * for initialising the cipher and hash. Returns 1 on success or 0 on failure.
 */
int tls13_setup_key_block(SSL_CONNECTION *s)
{
    const EVP_CIPHER *c;
    const EVP_MD *hash;
    int mac_type = NID_undef;
    size_t mac_secret_size = 0;

    s->session->cipher = s->s3.tmp.new_cipher;
    if (!ssl_cipher_get_evp(SSL_CONNECTION_GET_CTX(s), s->session, &c, &hash,
                            &mac_type, &mac_secret_size, NULL, 0)) {
        /* Error is already recorded */
        SSLfatal_alert(s, SSL_AD_INTERNAL_ERROR);
        return 0;
    }

    ssl_evp_cipher_free(s->s3.tmp.new_sym_enc);
    s->s3.tmp.new_sym_enc = c;
    ssl_evp_md_free(s->s3.tmp.new_hash);
    s->s3.tmp.new_hash = hash;
    s->s3.tmp.new_mac_pkey_type = mac_type;
    s->s3.tmp.new_mac_secret_size = mac_secret_size;

    return 1;
}

static int derive_secret_key_and_iv(SSL_CONNECTION *s, const EVP_MD *md,
                                    const EVP_CIPHER *ciph,
                                    int mac_type,
                                    const EVP_MD *mac_md,
                                    const unsigned char *insecret,
                                    const unsigned char *hash,
                                    const unsigned char *label,
                                    size_t labellen, unsigned char *secret,
                                    unsigned char *key, size_t *keylen,
                                    unsigned char **iv, size_t *ivlen,
                                    size_t *taglen)
{
    int hashleni = EVP_MD_get_size(md);
    size_t hashlen;
    int mode, mac_mdleni;

    /* Ensure cast to size_t is safe */
    if (!ossl_assert(hashleni > 0)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_EVP_LIB);
        return 0;
    }
    hashlen = (size_t)hashleni;

    if (!tls13_hkdf_expand(s, md, insecret, label, labellen, hash, hashlen,
                           secret, hashlen, 1)) {
        /* SSLfatal() already called */
        return 0;
    }

    /* if ciph is NULL cipher, then use new_hash to calculate keylen */
    if (EVP_CIPHER_is_a(ciph, "NULL")
        && mac_md != NULL
        && mac_type == NID_hmac) {
        mac_mdleni = EVP_MD_get_size(mac_md);

        if (mac_mdleni <= 0) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            return 0;
        }
        *ivlen = *taglen = (size_t)mac_mdleni;
        *keylen = s->s3.tmp.new_mac_secret_size;
    } else {

        *keylen = EVP_CIPHER_get_key_length(ciph);

        mode = EVP_CIPHER_get_mode(ciph);
        if (mode == EVP_CIPH_CCM_MODE) {
            uint32_t algenc;

            *ivlen = EVP_CCM_TLS_IV_LEN;
            if (s->s3.tmp.new_cipher != NULL) {
                algenc = s->s3.tmp.new_cipher->algorithm_enc;
            } else if (s->session->cipher != NULL) {
                /* We've not selected a cipher yet - we must be doing early data */
                algenc = s->session->cipher->algorithm_enc;
            } else if (s->psksession != NULL && s->psksession->cipher != NULL) {
                /* We must be doing early data with out-of-band PSK */
                algenc = s->psksession->cipher->algorithm_enc;
            } else {
                SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_EVP_LIB);
                return 0;
            }
            if (algenc & (SSL_AES128CCM8 | SSL_AES256CCM8))
                *taglen = EVP_CCM8_TLS_TAG_LEN;
            else
                *taglen = EVP_CCM_TLS_TAG_LEN;
        } else {
            int iivlen;

            if (mode == EVP_CIPH_GCM_MODE) {
                *taglen = EVP_GCM_TLS_TAG_LEN;
            } else {
                /* CHACHA20P-POLY1305 */
                *taglen = EVP_CHACHAPOLY_TLS_TAG_LEN;
            }
            iivlen = EVP_CIPHER_get_iv_length(ciph);
            if (iivlen < 0) {
                SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_EVP_LIB);
                return 0;
            }
            *ivlen = iivlen;
        }
    }

    if (*ivlen > EVP_MAX_IV_LENGTH) {
        *iv = OPENSSL_malloc(*ivlen);
        if (*iv == NULL) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_MALLOC_FAILURE);
            return 0;
        }
    }

    if (!tls13_derive_key(s, md, secret, key, *keylen)
            || !tls13_derive_iv(s, md, secret, *iv, *ivlen)) {
        /* SSLfatal() already called */
        return 0;
    }

    return 1;
}

static int tls13_store_hash(SSL_CONNECTION *s, unsigned char *hash, size_t len)
{
    size_t hashlen;

    if (!ssl3_digest_cached_records(s, 1)
            || !ssl_handshake_hash(s, hash, len, &hashlen)) {
        /* SSLfatal() already called */;
        return 0;
    }

    return 1;
}

int tls13_store_handshake_traffic_hash(SSL_CONNECTION *s)
{
    return tls13_store_hash(s, s->handshake_traffic_hash,
                            sizeof(s->handshake_traffic_hash));
}

int tls13_store_server_finished_hash(SSL_CONNECTION *s)
{
    return tls13_store_hash(s, s->server_finished_hash,
                            sizeof(s->server_finished_hash));
}

int tls13_change_cipher_state(SSL_CONNECTION *s, int which)
{
    /* ASCII: "c e traffic", in hex for EBCDIC compatibility */
    static const unsigned char client_early_traffic[] = "\x63\x20\x65\x20\x74\x72\x61\x66\x66\x69\x63";
    /* ASCII: "c hs traffic", in hex for EBCDIC compatibility */
    static const unsigned char client_handshake_traffic[] = "\x63\x20\x68\x73\x20\x74\x72\x61\x66\x66\x69\x63";
    /* ASCII: "c ap traffic", in hex for EBCDIC compatibility */
    static const unsigned char client_application_traffic[] = "\x63\x20\x61\x70\x20\x74\x72\x61\x66\x66\x69\x63";
    /* ASCII: "s hs traffic", in hex for EBCDIC compatibility */
    static const unsigned char server_handshake_traffic[] = "\x73\x20\x68\x73\x20\x74\x72\x61\x66\x66\x69\x63";
    /* ASCII: "s ap traffic", in hex for EBCDIC compatibility */
    static const unsigned char server_application_traffic[] = "\x73\x20\x61\x70\x20\x74\x72\x61\x66\x66\x69\x63";
    /* ASCII: "exp master", in hex for EBCDIC compatibility */
    static const unsigned char exporter_master_secret[] = "\x65\x78\x70\x20\x6D\x61\x73\x74\x65\x72";
    /* ASCII: "res master", in hex for EBCDIC compatibility */
    static const unsigned char resumption_master_secret[] = "\x72\x65\x73\x20\x6D\x61\x73\x74\x65\x72";
    /* ASCII: "e exp master", in hex for EBCDIC compatibility */
    static const unsigned char early_exporter_master_secret[] = "\x65\x20\x65\x78\x70\x20\x6D\x61\x73\x74\x65\x72";
    unsigned char iv_intern[EVP_MAX_IV_LENGTH];
    unsigned char *iv = iv_intern;
    unsigned char key[EVP_MAX_KEY_LENGTH];
    unsigned char secret[EVP_MAX_MD_SIZE];
    unsigned char hashval[EVP_MAX_MD_SIZE];
    unsigned char *hash = hashval;
    unsigned char *insecret;
    unsigned char *finsecret = NULL;
    const char *log_label = NULL;
    int finsecretlen = 0;
    const unsigned char *label;
    size_t labellen, hashlen = 0;
    int ret = 0;
    const EVP_MD *md = NULL, *mac_md = NULL;
    const EVP_CIPHER *cipher = NULL;
    int mac_pkey_type = NID_undef;
    SSL_CTX *sctx = SSL_CONNECTION_GET_CTX(s);
    size_t keylen, ivlen = EVP_MAX_IV_LENGTH, taglen;
    int level;
    int direction = (which & SSL3_CC_READ) != 0 ? OSSL_RECORD_DIRECTION_READ
                                                : OSSL_RECORD_DIRECTION_WRITE;

    if (((which & SSL3_CC_CLIENT) && (which & SSL3_CC_WRITE))
            || ((which & SSL3_CC_SERVER) && (which & SSL3_CC_READ))) {
        if ((which & SSL3_CC_EARLY) != 0) {
            EVP_MD_CTX *mdctx = NULL;
            long handlen;
            void *hdata;
            unsigned int hashlenui;
            const SSL_CIPHER *sslcipher = SSL_SESSION_get0_cipher(s->session);

            insecret = s->early_secret;
            label = client_early_traffic;
            labellen = sizeof(client_early_traffic) - 1;
            log_label = CLIENT_EARLY_LABEL;

            handlen = BIO_get_mem_data(s->s3.handshake_buffer, &hdata);
            if (handlen <= 0) {
                SSLfatal(s, SSL_AD_INTERNAL_ERROR, SSL_R_BAD_HANDSHAKE_LENGTH);
                goto err;
            }

            if (s->early_data_state == SSL_EARLY_DATA_CONNECTING
                    && s->max_early_data > 0
                    && s->session->ext.max_early_data == 0) {
                /*
                 * If we are attempting to send early data, and we've decided to
                 * actually do it but max_early_data in s->session is 0 then we
                 * must be using an external PSK.
                 */
                if (!ossl_assert(s->psksession != NULL
                        && s->max_early_data ==
                           s->psksession->ext.max_early_data)) {
                    SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
                    goto err;
                }
                sslcipher = SSL_SESSION_get0_cipher(s->psksession);
            }
            if (sslcipher == NULL) {
                SSLfatal(s, SSL_AD_INTERNAL_ERROR, SSL_R_BAD_PSK);
                goto err;
            }

            /*
             * This ups the ref count on cipher so we better make sure we free
             * it again
             */
            if (!ssl_cipher_get_evp_cipher(sctx, sslcipher, &cipher)) {
                /* Error is already recorded */
                SSLfatal_alert(s, SSL_AD_INTERNAL_ERROR);
                goto err;
            }

            if (((EVP_CIPHER_flags(cipher) & EVP_CIPH_FLAG_AEAD_CIPHER) == 0)
                && (!ssl_cipher_get_evp_md_mac(sctx, sslcipher, &mac_md,
                                               &mac_pkey_type, NULL))) {
                SSLfatal_alert(s, SSL_AD_INTERNAL_ERROR);
                goto err;
            }

            /*
             * We need to calculate the handshake digest using the digest from
             * the session. We haven't yet selected our ciphersuite so we can't
             * use ssl_handshake_md().
             */
            mdctx = EVP_MD_CTX_new();
            if (mdctx == NULL) {
                SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_EVP_LIB);
                goto err;
            }

            md = ssl_md(sctx, sslcipher->algorithm2);
            if (md == NULL || !EVP_DigestInit_ex(mdctx, md, NULL)
                    || !EVP_DigestUpdate(mdctx, hdata, handlen)
                    || !EVP_DigestFinal_ex(mdctx, hashval, &hashlenui)) {
                SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
                EVP_MD_CTX_free(mdctx);
                goto err;
            }
            hashlen = hashlenui;
            EVP_MD_CTX_free(mdctx);

            if (!tls13_hkdf_expand(s, md, insecret,
                                   early_exporter_master_secret,
                                   sizeof(early_exporter_master_secret) - 1,
                                   hashval, hashlen,
                                   s->early_exporter_master_secret, hashlen,
                                   1)) {
                SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
                goto err;
            }

            if (!ssl_log_secret(s, EARLY_EXPORTER_SECRET_LABEL,
                                s->early_exporter_master_secret, hashlen)) {
                /* SSLfatal() already called */
                goto err;
            }
        } else if (which & SSL3_CC_HANDSHAKE) {
            insecret = s->handshake_secret;
            finsecret = s->client_finished_secret;
            finsecretlen = EVP_MD_get_size(ssl_handshake_md(s));
            if (finsecretlen <= 0) {
                SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
                goto err;
            }
            label = client_handshake_traffic;
            labellen = sizeof(client_handshake_traffic) - 1;
            log_label = CLIENT_HANDSHAKE_LABEL;
            /*
             * The handshake hash used for the server read/client write handshake
             * traffic secret is the same as the hash for the server
             * write/client read handshake traffic secret. However, if we
             * processed early data then we delay changing the server
             * read/client write cipher state until later, and the handshake
             * hashes have moved on. Therefore we use the value saved earlier
             * when we did the server write/client read change cipher state.
             */
            hash = s->handshake_traffic_hash;
        } else {
            insecret = s->master_secret;
            label = client_application_traffic;
            labellen = sizeof(client_application_traffic) - 1;
            log_label = CLIENT_APPLICATION_LABEL;
            /*
             * For this we only use the handshake hashes up until the server
             * Finished hash. We do not include the client's Finished, which is
             * what ssl_handshake_hash() would give us. Instead we use the
             * previously saved value.
             */
            hash = s->server_finished_hash;
        }
    } else {
        /* Early data never applies to client-read/server-write */
        if (which & SSL3_CC_HANDSHAKE) {
            insecret = s->handshake_secret;
            finsecret = s->server_finished_secret;
            finsecretlen = EVP_MD_get_size(ssl_handshake_md(s));
            if (finsecretlen <= 0) {
                SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
                goto err;
            }
            label = server_handshake_traffic;
            labellen = sizeof(server_handshake_traffic) - 1;
            log_label = SERVER_HANDSHAKE_LABEL;
        } else {
            insecret = s->master_secret;
            label = server_application_traffic;
            labellen = sizeof(server_application_traffic) - 1;
            log_label = SERVER_APPLICATION_LABEL;
            hash = s->server_finished_hash;
        }
    }

    if ((which & SSL3_CC_EARLY) == 0) {
        md = ssl_handshake_md(s);
        cipher = s->s3.tmp.new_sym_enc;
        mac_md = s->s3.tmp.new_hash;
        mac_pkey_type = s->s3.tmp.new_mac_pkey_type;
        if (!ssl3_digest_cached_records(s, 1)
                || !ssl_handshake_hash(s, hashval, sizeof(hashval), &hashlen)) {
            /* SSLfatal() already called */;
            goto err;
        }
    }

    if (label == client_application_traffic) {
        /*
         * We also create the resumption master secret, but this time use the
         * hash for the whole handshake including the Client Finished
         */
        if (!tls13_hkdf_expand(s, ssl_handshake_md(s), insecret,
                               resumption_master_secret,
                               sizeof(resumption_master_secret) - 1,
                               hashval, hashlen, s->resumption_master_secret,
                               hashlen, 1)) {
            /* SSLfatal() already called */
            goto err;
        }
    }

    /* check whether cipher is known */
    if (!ossl_assert(cipher != NULL))
        goto err;

    if (!derive_secret_key_and_iv(s, md, cipher, mac_pkey_type, mac_md,
                                  insecret, hash, label, labellen, secret, key,
                                  &keylen, &iv, &ivlen, &taglen)) {
        /* SSLfatal() already called */
        goto err;
    }

    if (label == server_application_traffic) {
        memcpy(s->server_app_traffic_secret, secret, hashlen);
        /* Now we create the exporter master secret */
        if (!tls13_hkdf_expand(s, ssl_handshake_md(s), insecret,
                               exporter_master_secret,
                               sizeof(exporter_master_secret) - 1,
                               hash, hashlen, s->exporter_master_secret,
                               hashlen, 1)) {
            /* SSLfatal() already called */
            goto err;
        }

        if (!ssl_log_secret(s, EXPORTER_SECRET_LABEL, s->exporter_master_secret,
                            hashlen)) {
            /* SSLfatal() already called */
            goto err;
        }
    } else if (label == client_application_traffic)
        memcpy(s->client_app_traffic_secret, secret, hashlen);

    if (!ssl_log_secret(s, log_label, secret, hashlen)) {
        /* SSLfatal() already called */
        goto err;
    }

    if (finsecret != NULL
            && !tls13_derive_finishedkey(s, ssl_handshake_md(s), secret,
                                         finsecret, (size_t)finsecretlen)) {
        /* SSLfatal() already called */
        goto err;
    }

    if ((which & SSL3_CC_WRITE) != 0) {
        if (!s->server && label == client_early_traffic)
            s->rlayer.wrlmethod->set_plain_alerts(s->rlayer.wrl, 1);
        else
            s->rlayer.wrlmethod->set_plain_alerts(s->rlayer.wrl, 0);
    }

    level = (which & SSL3_CC_EARLY) != 0
            ? OSSL_RECORD_PROTECTION_LEVEL_EARLY
            : ((which &SSL3_CC_HANDSHAKE) != 0
               ? OSSL_RECORD_PROTECTION_LEVEL_HANDSHAKE
               : OSSL_RECORD_PROTECTION_LEVEL_APPLICATION);

    if (!ssl_set_new_record_layer(s, s->version,
                                  direction,
                                  level, secret, hashlen, key, keylen, iv,
                                  ivlen, NULL, 0, cipher, taglen,
                                  mac_pkey_type, mac_md, NULL, md)) {
        /* SSLfatal already called */
        goto err;
    }

    ret = 1;
 err:
    if ((which & SSL3_CC_EARLY) != 0) {
        /* We up-refed this so now we need to down ref */
        if ((EVP_CIPHER_flags(cipher) & EVP_CIPH_FLAG_AEAD_CIPHER) == 0)
            ssl_evp_md_free(mac_md);
        ssl_evp_cipher_free(cipher);
    }
    OPENSSL_cleanse(key, sizeof(key));
    OPENSSL_cleanse(secret, sizeof(secret));
    if (iv != iv_intern)
        OPENSSL_free(iv);
    return ret;
}

int tls13_update_key(SSL_CONNECTION *s, int sending)
{
    /* ASCII: "traffic upd", in hex for EBCDIC compatibility */
    static const unsigned char application_traffic[] = "\x74\x72\x61\x66\x66\x69\x63\x20\x75\x70\x64";
    const EVP_MD *md = ssl_handshake_md(s);
    size_t hashlen;
    unsigned char key[EVP_MAX_KEY_LENGTH];
    unsigned char *insecret;
    unsigned char secret[EVP_MAX_MD_SIZE];
    char *log_label;
    size_t keylen, ivlen, taglen;
    int ret = 0, l;
    int direction = sending ? OSSL_RECORD_DIRECTION_WRITE
                            : OSSL_RECORD_DIRECTION_READ;
    unsigned char iv_intern[EVP_MAX_IV_LENGTH];
    unsigned char *iv = iv_intern;

    if ((l = EVP_MD_get_size(md)) <= 0) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return 0;
    }
    hashlen = (size_t)l;

    if (s->server == sending)
        insecret = s->server_app_traffic_secret;
    else
        insecret = s->client_app_traffic_secret;

    if (!derive_secret_key_and_iv(s, md,
                                  s->s3.tmp.new_sym_enc,
                                  s->s3.tmp.new_mac_pkey_type, s->s3.tmp.new_hash,
                                  insecret, NULL,
                                  application_traffic,
                                  sizeof(application_traffic) - 1, secret, key,
                                  &keylen, &iv, &ivlen, &taglen)) {
        /* SSLfatal() already called */
        goto err;
    }

    memcpy(insecret, secret, hashlen);

    if (!ssl_set_new_record_layer(s, s->version,
                            direction,
                            OSSL_RECORD_PROTECTION_LEVEL_APPLICATION,
                            insecret, hashlen, key, keylen, iv, ivlen, NULL, 0,
                            s->s3.tmp.new_sym_enc, taglen, NID_undef, NULL,
                            NULL, md)) {
        /* SSLfatal already called */
        goto err;
    }

    /* Call Key log on successful traffic secret update */
    log_label = s->server == sending ? SERVER_APPLICATION_N_LABEL : CLIENT_APPLICATION_N_LABEL;
    if (!ssl_log_secret(s, log_label, secret, hashlen)) {
        /* SSLfatal() already called */
        goto err;
    }
    ret = 1;
 err:
    OPENSSL_cleanse(key, sizeof(key));
    OPENSSL_cleanse(secret, sizeof(secret));
    if (iv != iv_intern)
        OPENSSL_free(iv);
    return ret;
}

int tls13_alert_code(int code)
{
    /* There are 2 additional alerts in TLSv1.3 compared to TLSv1.2 */
    if (code == SSL_AD_MISSING_EXTENSION || code == SSL_AD_CERTIFICATE_REQUIRED)
        return code;

    return tls1_alert_code(code);
}

int tls13_export_keying_material(SSL_CONNECTION *s,
                                 unsigned char *out, size_t olen,
                                 const char *label, size_t llen,
                                 const unsigned char *context,
                                 size_t contextlen, int use_context)
{
    unsigned char exportsecret[EVP_MAX_MD_SIZE];
    /* ASCII: "exporter", in hex for EBCDIC compatibility */
    static const unsigned char exporterlabel[] = "\x65\x78\x70\x6F\x72\x74\x65\x72";
    unsigned char hash[EVP_MAX_MD_SIZE], data[EVP_MAX_MD_SIZE];
    const EVP_MD *md = ssl_handshake_md(s);
    EVP_MD_CTX *ctx = EVP_MD_CTX_new();
    unsigned int hashsize, datalen;
    int ret = 0;

    if (ctx == NULL || md == NULL || !ossl_statem_export_allowed(s))
        goto err;

    if (!use_context)
        contextlen = 0;

    if (EVP_DigestInit_ex(ctx, md, NULL) <= 0
            || EVP_DigestUpdate(ctx, context, contextlen) <= 0
            || EVP_DigestFinal_ex(ctx, hash, &hashsize) <= 0
            || EVP_DigestInit_ex(ctx, md, NULL) <= 0
            || EVP_DigestFinal_ex(ctx, data, &datalen) <= 0
            || !tls13_hkdf_expand(s, md, s->exporter_master_secret,
                                  (const unsigned char *)label, llen,
                                  data, datalen, exportsecret, hashsize, 0)
            || !tls13_hkdf_expand(s, md, exportsecret, exporterlabel,
                                  sizeof(exporterlabel) - 1, hash, hashsize,
                                  out, olen, 0))
        goto err;

    ret = 1;
 err:
    EVP_MD_CTX_free(ctx);
    return ret;
}

int tls13_export_keying_material_early(SSL_CONNECTION *s,
                                       unsigned char *out, size_t olen,
                                       const char *label, size_t llen,
                                       const unsigned char *context,
                                       size_t contextlen)
{
    /* ASCII: "exporter", in hex for EBCDIC compatibility */
    static const unsigned char exporterlabel[] = "\x65\x78\x70\x6F\x72\x74\x65\x72";
    unsigned char exportsecret[EVP_MAX_MD_SIZE];
    unsigned char hash[EVP_MAX_MD_SIZE], data[EVP_MAX_MD_SIZE];
    const EVP_MD *md;
    EVP_MD_CTX *ctx = EVP_MD_CTX_new();
    unsigned int hashsize, datalen;
    int ret = 0;
    const SSL_CIPHER *sslcipher;

    if (ctx == NULL || !ossl_statem_export_early_allowed(s))
        goto err;

    if (!s->server && s->max_early_data > 0
            && s->session->ext.max_early_data == 0)
        sslcipher = SSL_SESSION_get0_cipher(s->psksession);
    else
        sslcipher = SSL_SESSION_get0_cipher(s->session);

    md = ssl_md(SSL_CONNECTION_GET_CTX(s), sslcipher->algorithm2);

    /*
     * Calculate the hash value and store it in |data|. The reason why
     * the empty string is used is that the definition of TLS-Exporter
     * is like so:
     *
     * TLS-Exporter(label, context_value, key_length) =
     *     HKDF-Expand-Label(Derive-Secret(Secret, label, ""),
     *                       "exporter", Hash(context_value), key_length)
     *
     * Derive-Secret(Secret, Label, Messages) =
     *       HKDF-Expand-Label(Secret, Label,
     *                         Transcript-Hash(Messages), Hash.length)
     *
     * Here Transcript-Hash is the cipher suite hash algorithm.
     */
    if (md == NULL
            || EVP_DigestInit_ex(ctx, md, NULL) <= 0
            || EVP_DigestUpdate(ctx, context, contextlen) <= 0
            || EVP_DigestFinal_ex(ctx, hash, &hashsize) <= 0
            || EVP_DigestInit_ex(ctx, md, NULL) <= 0
            || EVP_DigestFinal_ex(ctx, data, &datalen) <= 0
            || !tls13_hkdf_expand(s, md, s->early_exporter_master_secret,
                                  (const unsigned char *)label, llen,
                                  data, datalen, exportsecret, hashsize, 0)
            || !tls13_hkdf_expand(s, md, exportsecret, exporterlabel,
                                  sizeof(exporterlabel) - 1, hash, hashsize,
                                  out, olen, 0))
        goto err;

    ret = 1;
 err:
    EVP_MD_CTX_free(ctx);
    return ret;
}
