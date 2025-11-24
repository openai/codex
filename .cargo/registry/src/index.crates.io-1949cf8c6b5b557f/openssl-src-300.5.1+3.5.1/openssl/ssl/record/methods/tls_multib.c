/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "../../ssl_local.h"
#include "../record_local.h"
#include "recmethod_local.h"

#if defined(OPENSSL_SMALL_FOOTPRINT) \
    || !(defined(AES_ASM) && (defined(__x86_64) \
                              || defined(__x86_64__) \
                              || defined(_M_AMD64) \
                              || defined(_M_X64)))
# undef EVP_CIPH_FLAG_TLS1_1_MULTIBLOCK
# define EVP_CIPH_FLAG_TLS1_1_MULTIBLOCK 0
#endif

static int tls_is_multiblock_capable(OSSL_RECORD_LAYER *rl, uint8_t type,
                                     size_t len, size_t fraglen)
{
#if !defined(OPENSSL_NO_MULTIBLOCK) && EVP_CIPH_FLAG_TLS1_1_MULTIBLOCK
    if (type == SSL3_RT_APPLICATION_DATA
            && len >= 4 * fraglen
            && rl->compctx == NULL
            && rl->msg_callback == NULL
            && !rl->use_etm
            && RLAYER_USE_EXPLICIT_IV(rl)
            && !BIO_get_ktls_send(rl->bio)
            && (EVP_CIPHER_get_flags(EVP_CIPHER_CTX_get0_cipher(rl->enc_ctx))
                & EVP_CIPH_FLAG_TLS1_1_MULTIBLOCK) != 0)
        return 1;
#endif
    return 0;
}

size_t tls_get_max_records_multiblock(OSSL_RECORD_LAYER *rl, uint8_t type,
                                      size_t len, size_t maxfrag,
                                      size_t *preffrag)
{
    if (tls_is_multiblock_capable(rl, type, len, *preffrag)) {
        /* minimize address aliasing conflicts */
        if ((*preffrag & 0xfff) == 0)
            *preffrag -= 512;

        if (len >= 8 * (*preffrag))
            return 8;

        return 4;
    }

    return tls_get_max_records_default(rl, type, len, maxfrag, preffrag);
}

/*
 * Write records using the multiblock method.
 *
 * Returns 1 on success, 0 if multiblock isn't suitable (non-fatal error), or
 * -1 on fatal error.
 */
static int tls_write_records_multiblock_int(OSSL_RECORD_LAYER *rl,
                                            OSSL_RECORD_TEMPLATE *templates,
                                            size_t numtempl)
{
#if !defined(OPENSSL_NO_MULTIBLOCK) && EVP_CIPH_FLAG_TLS1_1_MULTIBLOCK
    size_t i;
    size_t totlen;
    TLS_BUFFER *wb;
    unsigned char aad[13];
    EVP_CTRL_TLS1_1_MULTIBLOCK_PARAM mb_param;
    size_t packlen;
    int packleni;

    if (numtempl != 4 && numtempl != 8)
        return 0;

    /*
     * Check templates have contiguous buffers and are all the same type and
     * length
     */
    for (i = 1; i < numtempl; i++) {
        if (templates[i - 1].type != templates[i].type
                || templates[i - 1].buflen != templates[i].buflen
                || templates[i - 1].buf + templates[i - 1].buflen
                   != templates[i].buf)
            return 0;
    }

    totlen = templates[0].buflen * numtempl;
    if (!tls_is_multiblock_capable(rl, templates[0].type, totlen,
                                   templates[0].buflen))
        return 0;

    /*
     * If we get this far, then multiblock is suitable
     * Depending on platform multi-block can deliver several *times*
     * better performance. Downside is that it has to allocate
     * jumbo buffer to accommodate up to 8 records, but the
     * compromise is considered worthy.
     */

    /*
     * Allocate jumbo buffer. This will get freed next time we do a non
     * multiblock write in the call to tls_setup_write_buffer() - the different
     * buffer sizes will be spotted and the buffer reallocated.
     */
    packlen = EVP_CIPHER_CTX_ctrl(rl->enc_ctx,
                                  EVP_CTRL_TLS1_1_MULTIBLOCK_MAX_BUFSIZE,
                                  (int)templates[0].buflen, NULL);
    packlen *= numtempl;
    if (!tls_setup_write_buffer(rl, 1, packlen, packlen)) {
        /* RLAYERfatal() already called */
        return -1;
    }
    wb = &rl->wbuf[0];

    mb_param.interleave = numtempl;
    memcpy(aad, rl->sequence, 8);
    aad[8] = templates[0].type;
    aad[9] = (unsigned char)(templates[0].version >> 8);
    aad[10] = (unsigned char)(templates[0].version);
    aad[11] = 0;
    aad[12] = 0;
    mb_param.out = NULL;
    mb_param.inp = aad;
    mb_param.len = totlen;

    packleni = EVP_CIPHER_CTX_ctrl(rl->enc_ctx,
                                   EVP_CTRL_TLS1_1_MULTIBLOCK_AAD,
                                   sizeof(mb_param), &mb_param);
    packlen = (size_t)packleni;
    if (packleni <= 0 || packlen > wb->len) { /* never happens */
        RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return -1;
    }

    mb_param.out = wb->buf;
    mb_param.inp = templates[0].buf;
    mb_param.len = totlen;

    if (EVP_CIPHER_CTX_ctrl(rl->enc_ctx,
                            EVP_CTRL_TLS1_1_MULTIBLOCK_ENCRYPT,
                            sizeof(mb_param), &mb_param) <= 0) {
        RLAYERfatal(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return -1;
    }

    rl->sequence[7] += mb_param.interleave;
    if (rl->sequence[7] < mb_param.interleave) {
        int j = 6;
        while (j >= 0 && (++rl->sequence[j--]) == 0) ;
    }

    wb->offset = 0;
    wb->left = packlen;

    return 1;
#else  /* !defined(OPENSSL_NO_MULTIBLOCK) && EVP_CIPH_FLAG_TLS1_1_MULTIBLOCK */
    return 0;
#endif
}

int tls_write_records_multiblock(OSSL_RECORD_LAYER *rl,
                                 OSSL_RECORD_TEMPLATE *templates,
                                 size_t numtempl)
{
    int ret;

    ret = tls_write_records_multiblock_int(rl, templates, numtempl);
    if (ret < 0) {
        /* RLAYERfatal already called */
        return 0;
    }
    if (ret == 0) {
        /* Multiblock wasn't suitable so just do a standard write */
        if (!tls_write_records_default(rl, templates, numtempl)) {
            /* RLAYERfatal already called */
            return 0;
        }
    }

    return 1;
}
