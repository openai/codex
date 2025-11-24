/*
 * Copyright 2022-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/evp.h>
#include <openssl/core_names.h>
#include "internal/ssl3_cbc.h"
#include "../../ssl_local.h"
#include "../record_local.h"
#include "recmethod_local.h"

static int ssl3_set_crypto_state(OSSL_RECORD_LAYER *rl, int level,
                                 unsigned char *key, size_t keylen,
                                 unsigned char *iv, size_t ivlen,
                                 unsigned char *mackey, size_t mackeylen,
                                 const EVP_CIPHER *ciph,
                                 size_t taglen,
                                 int mactype,
                                 const EVP_MD *md,
                                 COMP_METHOD *comp)
{
    EVP_CIPHER_CTX *ciph_ctx;
    int enc = (rl->direction == OSSL_RECORD_DIRECTION_WRITE) ? 1 : 0;

    if (md == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        return OSSL_RECORD_RETURN_FATAL;
    }

    if ((rl->enc_ctx = EVP_CIPHER_CTX_new()) == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        return OSSL_RECORD_RETURN_FATAL;
    }
    ciph_ctx = rl->enc_ctx;

    rl->md_ctx = EVP_MD_CTX_new();
    if (rl->md_ctx == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        return OSSL_RECORD_RETURN_FATAL;
    }

    if ((md != NULL && EVP_DigestInit_ex(rl->md_ctx, md, NULL) <= 0)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        return OSSL_RECORD_RETURN_FATAL;
    }

#ifndef OPENSSL_NO_COMP
    if (comp != NULL) {
        rl->compctx = COMP_CTX_new(comp);
        if (rl->compctx == NULL) {
            ERR_raise(ERR_LIB_SSL, SSL_R_COMPRESSION_LIBRARY_ERROR);
            return OSSL_RECORD_RETURN_FATAL;
        }
    }
#endif

    if (!EVP_CipherInit_ex(ciph_ctx, ciph, NULL, key, iv, enc)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        return OSSL_RECORD_RETURN_FATAL;
    }

    /*
     * The cipher we actually ended up using in the EVP_CIPHER_CTX may be
     * different to that in ciph if we have an ENGINE in use
     */
    if (EVP_CIPHER_get0_provider(EVP_CIPHER_CTX_get0_cipher(ciph_ctx)) != NULL
            && !ossl_set_tls_provider_parameters(rl, ciph_ctx, ciph, md)) {
        /* ERR_raise already called */
        return OSSL_RECORD_RETURN_FATAL;
    }

    if (mackeylen > sizeof(rl->mac_secret)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        return OSSL_RECORD_RETURN_FATAL;
    }
    memcpy(rl->mac_secret, mackey, mackeylen);

    return OSSL_RECORD_RETURN_SUCCESS;
}

/*
 * ssl3_cipher encrypts/decrypts |n_recs| records in |inrecs|. Calls RLAYERfatal
 * on internal error, but not otherwise. It is the responsibility of the caller
 * to report a bad_record_mac
 *
 * Returns:
 *    0: if the record is publicly invalid, or an internal error
 *    1: Success or Mac-then-encrypt decryption failed (MAC will be randomised)
 */
static int ssl3_cipher(OSSL_RECORD_LAYER *rl, TLS_RL_RECORD *inrecs,
                       size_t n_recs, int sending, SSL_MAC_BUF *mac,
                       size_t macsize)
{
    TLS_RL_RECORD *rec;
    EVP_CIPHER_CTX *ds;
    size_t l, i;
    size_t bs;
    const EVP_CIPHER *enc;
    int provided;

    rec = inrecs;
    /*
     * We shouldn't ever be called with more than one record in the SSLv3 case
     */
    if (n_recs != 1)
        return 0;

    ds = rl->enc_ctx;
    if (ds == NULL || (enc = EVP_CIPHER_CTX_get0_cipher(ds)) == NULL)
        return 0;

    provided = (EVP_CIPHER_get0_provider(enc) != NULL);

    l = rec->length;
    bs = EVP_CIPHER_CTX_get_block_size(ds);

    if (bs == 0)
        return 0;

    /* COMPRESS */

    if ((bs != 1) && sending && !provided) {
        /*
         * We only do this for legacy ciphers. Provided ciphers add the
         * padding on the provider side.
         */
        i = bs - (l % bs);

        /* we need to add 'i-1' padding bytes */
        l += i;
        /*
         * the last of these zero bytes will be overwritten with the
         * padding length.
         */
        memset(&rec->input[rec->length], 0, i);
        rec->length += i;
        rec->input[l - 1] = (unsigned char)(i - 1);
    }

    if (!sending) {
        if (l == 0 || l % bs != 0) {
            /* Publicly invalid */
            return 0;
        }
        /* otherwise, rec->length >= bs */
    }

    if (provided) {
        int outlen;

        if (!EVP_CipherUpdate(ds, rec->data, &outlen, rec->input,
                              (unsigned int)l))
            return 0;
        rec->length = outlen;

        if (!sending && mac != NULL) {
            /* Now get a pointer to the MAC */
            OSSL_PARAM params[2], *p = params;

            /* Get the MAC */
            mac->alloced = 0;

            *p++ = OSSL_PARAM_construct_octet_ptr(OSSL_CIPHER_PARAM_TLS_MAC,
                                                  (void **)&mac->mac,
                                                  macsize);
            *p = OSSL_PARAM_construct_end();

            if (!EVP_CIPHER_CTX_get_params(ds, params)) {
                /* Shouldn't normally happen */
                RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
                return 0;
            }
        }
    } else {
        if (EVP_Cipher(ds, rec->data, rec->input, (unsigned int)l) < 1) {
            /* Shouldn't happen */
            RLAYERfatal(rl, SSL_AD_BAD_RECORD_MAC, ERR_R_INTERNAL_ERROR);
            return 0;
        }

        if (!sending)
            return ssl3_cbc_remove_padding_and_mac(&rec->length,
                                        rec->orig_len,
                                        rec->data,
                                        (mac != NULL) ? &mac->mac : NULL,
                                        (mac != NULL) ? &mac->alloced : NULL,
                                        bs,
                                        macsize,
                                        rl->libctx);
    }

    return 1;
}

static const unsigned char ssl3_pad_1[48] = {
    0x36, 0x36, 0x36, 0x36, 0x36, 0x36, 0x36, 0x36,
    0x36, 0x36, 0x36, 0x36, 0x36, 0x36, 0x36, 0x36,
    0x36, 0x36, 0x36, 0x36, 0x36, 0x36, 0x36, 0x36,
    0x36, 0x36, 0x36, 0x36, 0x36, 0x36, 0x36, 0x36,
    0x36, 0x36, 0x36, 0x36, 0x36, 0x36, 0x36, 0x36,
    0x36, 0x36, 0x36, 0x36, 0x36, 0x36, 0x36, 0x36
};

static const unsigned char ssl3_pad_2[48] = {
    0x5c, 0x5c, 0x5c, 0x5c, 0x5c, 0x5c, 0x5c, 0x5c,
    0x5c, 0x5c, 0x5c, 0x5c, 0x5c, 0x5c, 0x5c, 0x5c,
    0x5c, 0x5c, 0x5c, 0x5c, 0x5c, 0x5c, 0x5c, 0x5c,
    0x5c, 0x5c, 0x5c, 0x5c, 0x5c, 0x5c, 0x5c, 0x5c,
    0x5c, 0x5c, 0x5c, 0x5c, 0x5c, 0x5c, 0x5c, 0x5c,
    0x5c, 0x5c, 0x5c, 0x5c, 0x5c, 0x5c, 0x5c, 0x5c
};

static int ssl3_mac(OSSL_RECORD_LAYER *rl, TLS_RL_RECORD *rec, unsigned char *md,
                    int sending)
{
    unsigned char *mac_sec, *seq = rl->sequence;
    const EVP_MD_CTX *hash;
    unsigned char *p, rec_char;
    size_t md_size;
    size_t npad;
    int t;

    mac_sec = &(rl->mac_secret[0]);
    hash = rl->md_ctx;

    t = EVP_MD_CTX_get_size(hash);
    if (t <= 0)
        return 0;
    md_size = t;
    npad = (48 / md_size) * md_size;

    if (!sending
        && EVP_CIPHER_CTX_get_mode(rl->enc_ctx) == EVP_CIPH_CBC_MODE
        && ssl3_cbc_record_digest_supported(hash)) {
#ifdef OPENSSL_NO_DEPRECATED_3_0
        return 0;
#else
        /*
         * This is a CBC-encrypted record. We must avoid leaking any
         * timing-side channel information about how many blocks of data we
         * are hashing because that gives an attacker a timing-oracle.
         */

        /*-
         * npad is, at most, 48 bytes and that's with MD5:
         *   16 + 48 + 8 (sequence bytes) + 1 + 2 = 75.
         *
         * With SHA-1 (the largest hash speced for SSLv3) the hash size
         * goes up 4, but npad goes down by 8, resulting in a smaller
         * total size.
         */
        unsigned char header[75];
        size_t j = 0;
        memcpy(header + j, mac_sec, md_size);
        j += md_size;
        memcpy(header + j, ssl3_pad_1, npad);
        j += npad;
        memcpy(header + j, seq, 8);
        j += 8;
        header[j++] = rec->type;
        header[j++] = (unsigned char)(rec->length >> 8);
        header[j++] = (unsigned char)(rec->length & 0xff);

        /* Final param == is SSLv3 */
        if (ssl3_cbc_digest_record(EVP_MD_CTX_get0_md(hash),
                                   md, &md_size,
                                   header, rec->input,
                                   rec->length, rec->orig_len,
                                   mac_sec, md_size, 1) <= 0)
            return 0;
#endif
    } else {
        unsigned int md_size_u;
        /* Chop the digest off the end :-) */
        EVP_MD_CTX *md_ctx = EVP_MD_CTX_new();

        if (md_ctx == NULL)
            return 0;

        rec_char = rec->type;
        p = md;
        s2n(rec->length, p);
        if (EVP_MD_CTX_copy_ex(md_ctx, hash) <= 0
            || EVP_DigestUpdate(md_ctx, mac_sec, md_size) <= 0
            || EVP_DigestUpdate(md_ctx, ssl3_pad_1, npad) <= 0
            || EVP_DigestUpdate(md_ctx, seq, 8) <= 0
            || EVP_DigestUpdate(md_ctx, &rec_char, 1) <= 0
            || EVP_DigestUpdate(md_ctx, md, 2) <= 0
            || EVP_DigestUpdate(md_ctx, rec->input, rec->length) <= 0
            || EVP_DigestFinal_ex(md_ctx, md, NULL) <= 0
            || EVP_MD_CTX_copy_ex(md_ctx, hash) <= 0
            || EVP_DigestUpdate(md_ctx, mac_sec, md_size) <= 0
            || EVP_DigestUpdate(md_ctx, ssl3_pad_2, npad) <= 0
            || EVP_DigestUpdate(md_ctx, md, md_size) <= 0
            || EVP_DigestFinal_ex(md_ctx, md, &md_size_u) <= 0) {
            EVP_MD_CTX_free(md_ctx);
            return 0;
        }

        EVP_MD_CTX_free(md_ctx);
    }

    if (!tls_increment_sequence_ctr(rl))
        return 0;

    return 1;
}

const struct record_functions_st ssl_3_0_funcs = {
    ssl3_set_crypto_state,
    ssl3_cipher,
    ssl3_mac,
    tls_default_set_protocol_version,
    tls_default_read_n,
    tls_get_more_records,
    tls_default_validate_record_header,
    tls_default_post_process_record,
    tls_get_max_records_default,
    tls_write_records_default,
    /* These 2 functions are defined in tls1_meth.c */
    tls1_allocate_write_buffers,
    tls1_initialise_write_packets,
    NULL,
    tls_prepare_record_header_default,
    NULL,
    tls_prepare_for_encryption_default,
    tls_post_encryption_processing_default,
    NULL
};
