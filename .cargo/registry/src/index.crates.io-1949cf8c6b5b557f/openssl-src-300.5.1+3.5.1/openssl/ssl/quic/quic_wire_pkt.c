/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/err.h>
#include "internal/common.h"
#include "internal/quic_wire_pkt.h"

int ossl_quic_hdr_protector_init(QUIC_HDR_PROTECTOR *hpr,
                                 OSSL_LIB_CTX *libctx,
                                 const char *propq,
                                 uint32_t cipher_id,
                                 const unsigned char *quic_hp_key,
                                 size_t quic_hp_key_len)
{
    const char *cipher_name = NULL;

    switch (cipher_id) {
        case QUIC_HDR_PROT_CIPHER_AES_128:
            cipher_name = "AES-128-ECB";
            break;
        case QUIC_HDR_PROT_CIPHER_AES_256:
            cipher_name = "AES-256-ECB";
            break;
        case QUIC_HDR_PROT_CIPHER_CHACHA:
            cipher_name = "ChaCha20";
            break;
        default:
            ERR_raise(ERR_LIB_SSL, ERR_R_UNSUPPORTED);
            return 0;
    }

    hpr->cipher_ctx = EVP_CIPHER_CTX_new();
    if (hpr->cipher_ctx == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_EVP_LIB);
        return 0;
    }

    hpr->cipher = EVP_CIPHER_fetch(libctx, cipher_name, propq);
    if (hpr->cipher == NULL
        || quic_hp_key_len != (size_t)EVP_CIPHER_get_key_length(hpr->cipher)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_EVP_LIB);
        goto err;
    }

    if (!EVP_CipherInit_ex(hpr->cipher_ctx, hpr->cipher, NULL,
                           quic_hp_key, NULL, 1)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_EVP_LIB);
        goto err;
    }

    hpr->libctx     = libctx;
    hpr->propq      = propq;
    hpr->cipher_id  = cipher_id;
    return 1;

err:
    ossl_quic_hdr_protector_cleanup(hpr);
    return 0;
}

void ossl_quic_hdr_protector_cleanup(QUIC_HDR_PROTECTOR *hpr)
{
    EVP_CIPHER_CTX_free(hpr->cipher_ctx);
    hpr->cipher_ctx = NULL;

    EVP_CIPHER_free(hpr->cipher);
    hpr->cipher = NULL;
}

static int hdr_generate_mask(QUIC_HDR_PROTECTOR *hpr,
                             const unsigned char *sample, size_t sample_len,
                             unsigned char *mask)
{
    int l = 0;
    unsigned char dst[16];
    static const unsigned char zeroes[5] = {0};
    size_t i;

    if (hpr->cipher_id == QUIC_HDR_PROT_CIPHER_AES_128
        || hpr->cipher_id == QUIC_HDR_PROT_CIPHER_AES_256) {
        if (sample_len < 16) {
            ERR_raise(ERR_LIB_SSL, ERR_R_PASSED_INVALID_ARGUMENT);
            return 0;
        }

        if (!EVP_CipherInit_ex(hpr->cipher_ctx, NULL, NULL, NULL, NULL, 1)
            || !EVP_CipherUpdate(hpr->cipher_ctx, dst, &l, sample, 16)) {
            ERR_raise(ERR_LIB_SSL, ERR_R_EVP_LIB);
            return 0;
        }

        for (i = 0; i < 5; ++i)
            mask[i] = dst[i];
    } else if (hpr->cipher_id == QUIC_HDR_PROT_CIPHER_CHACHA) {
        if (sample_len < 16) {
            ERR_raise(ERR_LIB_SSL, ERR_R_PASSED_INVALID_ARGUMENT);
            return 0;
        }

        if (!EVP_CipherInit_ex(hpr->cipher_ctx, NULL, NULL, NULL, sample, 1)
            || !EVP_CipherUpdate(hpr->cipher_ctx, mask, &l,
                                 zeroes, sizeof(zeroes))) {
            ERR_raise(ERR_LIB_SSL, ERR_R_EVP_LIB);
            return 0;
        }
    } else {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        assert(0);
        return 0;
    }

#ifdef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
    /* No matter what we did above we use the same mask in fuzzing mode */
    memset(mask, 0, 5);
#endif

    return 1;
}

int ossl_quic_hdr_protector_decrypt(QUIC_HDR_PROTECTOR *hpr,
                                    QUIC_PKT_HDR_PTRS *ptrs)
{
    return ossl_quic_hdr_protector_decrypt_fields(hpr,
                                                  ptrs->raw_sample,
                                                  ptrs->raw_sample_len,
                                                  ptrs->raw_start,
                                                  ptrs->raw_pn);
}

int ossl_quic_hdr_protector_decrypt_fields(QUIC_HDR_PROTECTOR *hpr,
                                           const unsigned char *sample,
                                           size_t sample_len,
                                           unsigned char *first_byte,
                                           unsigned char *pn_bytes)
{
    unsigned char mask[5], pn_len, i;

    if (!hdr_generate_mask(hpr, sample, sample_len, mask))
        return 0;

    *first_byte ^= mask[0] & ((*first_byte & 0x80) != 0 ? 0xf : 0x1f);
    pn_len = (*first_byte & 0x3) + 1;

    for (i = 0; i < pn_len; ++i)
        pn_bytes[i] ^= mask[i + 1];

    return 1;
}

int ossl_quic_hdr_protector_encrypt(QUIC_HDR_PROTECTOR *hpr,
                                    QUIC_PKT_HDR_PTRS *ptrs)
{
    return ossl_quic_hdr_protector_encrypt_fields(hpr,
                                                  ptrs->raw_sample,
                                                  ptrs->raw_sample_len,
                                                  ptrs->raw_start,
                                                  ptrs->raw_pn);
}

int ossl_quic_hdr_protector_encrypt_fields(QUIC_HDR_PROTECTOR *hpr,
                                           const unsigned char *sample,
                                           size_t sample_len,
                                           unsigned char *first_byte,
                                           unsigned char *pn_bytes)
{
    unsigned char mask[5], pn_len, i;

    if (!hdr_generate_mask(hpr, sample, sample_len, mask))
        return 0;

    pn_len = (*first_byte & 0x3) + 1;
    for (i = 0; i < pn_len; ++i)
        pn_bytes[i] ^= mask[i + 1];

    *first_byte ^= mask[0] & ((*first_byte & 0x80) != 0 ? 0xf : 0x1f);
    return 1;
}

int ossl_quic_wire_decode_pkt_hdr(PACKET *pkt,
                                  size_t short_conn_id_len,
                                  int partial,
                                  int nodata,
                                  QUIC_PKT_HDR *hdr,
                                  QUIC_PKT_HDR_PTRS *ptrs,
                                  uint64_t *fail_cause)
{
    unsigned int b0;
    unsigned char *pn = NULL;
    size_t l = PACKET_remaining(pkt);

    if (fail_cause != NULL)
        *fail_cause = QUIC_PKT_HDR_DECODE_DECODE_ERR;

    if (ptrs != NULL) {
        ptrs->raw_start         = (unsigned char *)PACKET_data(pkt);
        ptrs->raw_sample        = NULL;
        ptrs->raw_sample_len    = 0;
        ptrs->raw_pn            = NULL;
    }

    if (l < QUIC_MIN_VALID_PKT_LEN
        || !PACKET_get_1(pkt, &b0))
        return 0;

    hdr->partial    = partial;
    hdr->unused     = 0;
    hdr->reserved   = 0;

    if ((b0 & 0x80) == 0) {
        /* Short header. */
        if (short_conn_id_len > QUIC_MAX_CONN_ID_LEN)
            return 0;

        if ((b0 & 0x40) == 0 /* fixed bit not set? */
            || l < QUIC_MIN_VALID_PKT_LEN_CRYPTO)
            return 0;

        hdr->type       = QUIC_PKT_TYPE_1RTT;
        hdr->fixed      = 1;
        hdr->spin_bit   = (b0 & 0x20) != 0;
        if (partial) {
            hdr->key_phase  = 0; /* protected, zero for now */
            hdr->pn_len     = 0; /* protected, zero for now */
            hdr->reserved   = 0; /* protected, zero for now */
        } else {
            hdr->key_phase  = (b0 & 0x04) != 0;
            hdr->pn_len     = (b0 & 0x03) + 1;
            hdr->reserved   = (b0 & 0x18) >> 3;
        }

        /* Copy destination connection ID field to header structure. */
        if (!PACKET_copy_bytes(pkt, hdr->dst_conn_id.id, short_conn_id_len))
            return 0;

        hdr->dst_conn_id.id_len = (unsigned char)short_conn_id_len;

        /*
         * Skip over the PN. If this is a partial decode, the PN length field
         * currently has header protection applied. Thus we do not know the
         * length of the PN but we are allowed to assume it is 4 bytes long at
         * this stage.
         */
        memset(hdr->pn, 0, sizeof(hdr->pn));
        pn = (unsigned char *)PACKET_data(pkt);
        if (partial) {
            if (!PACKET_forward(pkt, sizeof(hdr->pn)))
                return 0;
        } else {
            if (!PACKET_copy_bytes(pkt, hdr->pn, hdr->pn_len))
                return 0;
        }

        /* Fields not used in short-header packets. */
        hdr->version            = 0;
        hdr->src_conn_id.id_len = 0;
        hdr->token              = NULL;
        hdr->token_len          = 0;

        /*
         * Short-header packets always come last in a datagram, the length
         * is the remainder of the buffer.
         */
        hdr->len                = PACKET_remaining(pkt);
        hdr->data               = PACKET_data(pkt);

        /*
         * Skip over payload. Since this is a short header packet, which cannot
         * be followed by any other kind of packet, this advances us to the end
         * of the datagram.
         */
        if (!PACKET_forward(pkt, hdr->len))
            return 0;
    } else {
        /* Long header. */
        unsigned long version;
        unsigned int dst_conn_id_len, src_conn_id_len, raw_type;

        if (!PACKET_get_net_4(pkt, &version))
            return 0;

        /*
         * All QUIC packets must have the fixed bit set, except exceptionally
         * for Version Negotiation packets.
         */
        if (version != 0 && (b0 & 0x40) == 0)
            return 0;

        if (!PACKET_get_1(pkt, &dst_conn_id_len)
            || dst_conn_id_len > QUIC_MAX_CONN_ID_LEN
            || !PACKET_copy_bytes(pkt, hdr->dst_conn_id.id, dst_conn_id_len)
            || !PACKET_get_1(pkt, &src_conn_id_len)
            || src_conn_id_len > QUIC_MAX_CONN_ID_LEN
            || !PACKET_copy_bytes(pkt, hdr->src_conn_id.id, src_conn_id_len))
            return 0;

        hdr->version            = (uint32_t)version;
        hdr->dst_conn_id.id_len = (unsigned char)dst_conn_id_len;
        hdr->src_conn_id.id_len = (unsigned char)src_conn_id_len;

        if (version == 0) {
            /*
             * Version negotiation packet. Version negotiation packets are
             * identified by a version field of 0 and the type bits in the first
             * byte are ignored (they may take any value, and we ignore them).
             */
            hdr->type       = QUIC_PKT_TYPE_VERSION_NEG;
            hdr->fixed      = (b0 & 0x40) != 0;

            hdr->data       = PACKET_data(pkt);
            hdr->len        = PACKET_remaining(pkt);

            /*
             * Version negotiation packets must contain an array of u32s, so it
             * is invalid for their payload length to not be divisible by 4.
             */
            if ((hdr->len % 4) != 0)
                return 0;

            /* Version negotiation packets are always fully decoded. */
            hdr->partial    = 0;

            /* Fields not used in version negotiation packets. */
            hdr->pn_len             = 0;
            hdr->spin_bit           = 0;
            hdr->key_phase          = 0;
            hdr->token              = NULL;
            hdr->token_len          = 0;
            memset(hdr->pn, 0, sizeof(hdr->pn));

            if (!PACKET_forward(pkt, hdr->len))
                return 0;
        } else if (version != QUIC_VERSION_1) {
            if (fail_cause != NULL)
                *fail_cause |= QUIC_PKT_HDR_DECODE_BAD_VERSION;
            /* Unknown version, do not decode. */
            return 0;
        } else {
            if (l < QUIC_MIN_VALID_PKT_LEN_CRYPTO)
                return 0;

            /* Get long packet type and decode to QUIC_PKT_TYPE_*. */
            raw_type = ((b0 >> 4) & 0x3);

            switch (raw_type) {
            case 0:
                hdr->type = QUIC_PKT_TYPE_INITIAL;
                break;
            case 1:
                hdr->type = QUIC_PKT_TYPE_0RTT;
                break;
            case 2:
                hdr->type = QUIC_PKT_TYPE_HANDSHAKE;
                break;
            case 3:
                hdr->type = QUIC_PKT_TYPE_RETRY;
                break;
            }

            hdr->pn_len     = 0;
            hdr->fixed      = 1;

            /* Fields not used in long-header packets. */
            hdr->spin_bit   = 0;
            hdr->key_phase  = 0;

            if (hdr->type == QUIC_PKT_TYPE_INITIAL) {
                /* Initial packet. */
                uint64_t token_len;

                if (!PACKET_get_quic_vlint(pkt, &token_len)
                    || token_len > SIZE_MAX
                    || !PACKET_get_bytes(pkt, &hdr->token, (size_t)token_len))
                    return 0;

                hdr->token_len  = (size_t)token_len;
                if (token_len == 0)
                    hdr->token = NULL;
            } else {
                hdr->token      = NULL;
                hdr->token_len  = 0;
            }

            if (hdr->type == QUIC_PKT_TYPE_RETRY) {
                /* Retry packet. */
                hdr->data       = PACKET_data(pkt);
                hdr->len        = PACKET_remaining(pkt);

                /* Retry packets are always fully decoded. */
                hdr->partial    = 0;

                /* Unused bits in Retry header. */
                hdr->unused     = b0 & 0x0f;

                /* Fields not used in Retry packets. */
                memset(hdr->pn, 0, sizeof(hdr->pn));

                if (!PACKET_forward(pkt, hdr->len))
                    return 0;
            } else {
                /* Initial, 0-RTT or Handshake packet. */
                uint64_t len;

                hdr->pn_len     = partial ? 0 : ((b0 & 0x03) + 1);
                hdr->reserved   = partial ? 0 : ((b0 & 0x0C) >> 2);

                if (!PACKET_get_quic_vlint(pkt, &len)
                        || len < sizeof(hdr->pn))
                    return 0;

                if (!nodata && len > PACKET_remaining(pkt))
                    return 0;

                /*
                 * Skip over the PN. If this is a partial decode, the PN length
                 * field currently has header protection applied. Thus we do not
                 * know the length of the PN but we are allowed to assume it is
                 * 4 bytes long at this stage.
                 */
                pn = (unsigned char *)PACKET_data(pkt);
                memset(hdr->pn, 0, sizeof(hdr->pn));
                if (partial) {
                    if (!PACKET_forward(pkt, sizeof(hdr->pn)))
                        return 0;

                    hdr->len = (size_t)(len - sizeof(hdr->pn));
                } else {
                    if (!PACKET_copy_bytes(pkt, hdr->pn, hdr->pn_len))
                        return 0;

                    hdr->len = (size_t)(len - hdr->pn_len);
                }

                if (nodata) {
                    hdr->data = NULL;
                } else {
                    hdr->data = PACKET_data(pkt);

                    /* Skip over packet body. */
                    if (!PACKET_forward(pkt, hdr->len))
                        return 0;
                }
            }
        }
    }

    if (ptrs != NULL) {
        ptrs->raw_pn = pn;
        if (pn != NULL) {
            ptrs->raw_sample        = pn + 4;
            ptrs->raw_sample_len    = PACKET_end(pkt) - ptrs->raw_sample;
        }
    }

    /*
     * Good decode, clear the generic DECODE_ERR flag
     */
    if (fail_cause != NULL)
        *fail_cause &= ~QUIC_PKT_HDR_DECODE_DECODE_ERR;

    return 1;
}

int ossl_quic_wire_encode_pkt_hdr(WPACKET *pkt,
                                  size_t short_conn_id_len,
                                  const QUIC_PKT_HDR *hdr,
                                  QUIC_PKT_HDR_PTRS *ptrs)
{
    unsigned char b0;
    size_t off_start, off_sample, off_pn;
    unsigned char *start = WPACKET_get_curr(pkt);

    if (!WPACKET_get_total_written(pkt, &off_start))
        return 0;

    if (ptrs != NULL) {
        /* ptrs would not be stable on non-static WPACKET */
        if (!ossl_assert(pkt->staticbuf != NULL))
            return 0;
        ptrs->raw_start         = NULL;
        ptrs->raw_sample        = NULL;
        ptrs->raw_sample_len    = 0;
        ptrs->raw_pn            = 0;
    }

    /* Cannot serialize a partial header, or one whose DCID length is wrong. */
    if (hdr->partial
        || (hdr->type == QUIC_PKT_TYPE_1RTT
            && hdr->dst_conn_id.id_len != short_conn_id_len))
        return 0;

    if (hdr->type == QUIC_PKT_TYPE_1RTT) {
        /* Short header. */

        /*
         * Cannot serialize a header whose DCID length is wrong, or with an
         * invalid PN length.
         */
        if (hdr->dst_conn_id.id_len != short_conn_id_len
            || short_conn_id_len > QUIC_MAX_CONN_ID_LEN
            || hdr->pn_len < 1 || hdr->pn_len > 4)
            return 0;

        b0 = (hdr->spin_bit << 5)
             | (hdr->key_phase << 2)
             | (hdr->pn_len - 1)
             | (hdr->reserved << 3)
             | 0x40; /* fixed bit */

        if (!WPACKET_put_bytes_u8(pkt, b0)
            || !WPACKET_memcpy(pkt, hdr->dst_conn_id.id, short_conn_id_len)
            || !WPACKET_get_total_written(pkt, &off_pn)
            || !WPACKET_memcpy(pkt, hdr->pn, hdr->pn_len))
            return 0;
    } else {
        /* Long header. */
        unsigned int raw_type;

        if (hdr->dst_conn_id.id_len > QUIC_MAX_CONN_ID_LEN
            || hdr->src_conn_id.id_len > QUIC_MAX_CONN_ID_LEN)
            return 0;

        if (ossl_quic_pkt_type_has_pn(hdr->type)
            && (hdr->pn_len < 1 || hdr->pn_len > 4))
            return 0;

        switch (hdr->type) {
            case QUIC_PKT_TYPE_VERSION_NEG:
                if (hdr->version != 0)
                    return 0;

                /* Version negotiation packets use zero for the type bits */
                raw_type = 0;
                break;

            case QUIC_PKT_TYPE_INITIAL:     raw_type = 0; break;
            case QUIC_PKT_TYPE_0RTT:        raw_type = 1; break;
            case QUIC_PKT_TYPE_HANDSHAKE:   raw_type = 2; break;
            case QUIC_PKT_TYPE_RETRY:       raw_type = 3; break;
            default:
                return 0;
        }

        b0 = (raw_type << 4) | 0x80; /* long */
        if (hdr->type != QUIC_PKT_TYPE_VERSION_NEG || hdr->fixed)
            b0 |= 0x40; /* fixed */
        if (ossl_quic_pkt_type_has_pn(hdr->type)) {
            b0 |= hdr->pn_len - 1;
            b0 |= (hdr->reserved << 2);
        }
        if (hdr->type == QUIC_PKT_TYPE_RETRY)
            b0 |= hdr->unused;

        if (!WPACKET_put_bytes_u8(pkt, b0)
            || !WPACKET_put_bytes_u32(pkt, hdr->version)
            || !WPACKET_put_bytes_u8(pkt, hdr->dst_conn_id.id_len)
            || !WPACKET_memcpy(pkt, hdr->dst_conn_id.id,
                               hdr->dst_conn_id.id_len)
            || !WPACKET_put_bytes_u8(pkt, hdr->src_conn_id.id_len)
            || !WPACKET_memcpy(pkt, hdr->src_conn_id.id,
                               hdr->src_conn_id.id_len))
            return 0;

        if (hdr->type == QUIC_PKT_TYPE_VERSION_NEG) {
            if (hdr->len > 0 && !WPACKET_reserve_bytes(pkt, hdr->len, NULL))
                return 0;

            return 1;
        }

        if (hdr->type == QUIC_PKT_TYPE_INITIAL) {
            if (!WPACKET_quic_write_vlint(pkt, hdr->token_len)
                || !WPACKET_memcpy(pkt, hdr->token, hdr->token_len))
                return 0;
        }

        if (hdr->type == QUIC_PKT_TYPE_RETRY) {
            if (!WPACKET_memcpy(pkt, hdr->token, hdr->token_len))
                return 0;
            return 1;
        }

        if (!WPACKET_quic_write_vlint(pkt, hdr->len + hdr->pn_len)
            || !WPACKET_get_total_written(pkt, &off_pn)
            || !WPACKET_memcpy(pkt, hdr->pn, hdr->pn_len))
            return 0;
    }

    if (hdr->len > 0 && !WPACKET_reserve_bytes(pkt, hdr->len, NULL))
        return 0;

    off_sample = off_pn + 4;

    if (ptrs != NULL) {
        ptrs->raw_start         = start;
        ptrs->raw_sample        = start + (off_sample - off_start);
        ptrs->raw_sample_len
            = WPACKET_get_curr(pkt) + hdr->len - ptrs->raw_sample;
        ptrs->raw_pn            = start + (off_pn - off_start);
    }

    return 1;
}

int ossl_quic_wire_get_encoded_pkt_hdr_len(size_t short_conn_id_len,
                                           const QUIC_PKT_HDR *hdr)
{
    size_t len = 0, enclen;

    /* Cannot serialize a partial header, or one whose DCID length is wrong. */
    if (hdr->partial
        || (hdr->type == QUIC_PKT_TYPE_1RTT
            && hdr->dst_conn_id.id_len != short_conn_id_len))
        return 0;

    if (hdr->type == QUIC_PKT_TYPE_1RTT) {
        /* Short header. */

        /*
         * Cannot serialize a header whose DCID length is wrong, or with an
         * invalid PN length.
         */
        if (hdr->dst_conn_id.id_len != short_conn_id_len
            || short_conn_id_len > QUIC_MAX_CONN_ID_LEN
            || hdr->pn_len < 1 || hdr->pn_len > 4)
            return 0;

        return 1 + short_conn_id_len + hdr->pn_len;
    } else {
        /* Long header. */
        if (hdr->dst_conn_id.id_len > QUIC_MAX_CONN_ID_LEN
            || hdr->src_conn_id.id_len > QUIC_MAX_CONN_ID_LEN)
            return 0;

        len += 1 /* Initial byte */ + 4 /* Version */
            + 1 + hdr->dst_conn_id.id_len /* DCID Len, DCID */
            + 1 + hdr->src_conn_id.id_len /* SCID Len, SCID */
            ;

        if (ossl_quic_pkt_type_has_pn(hdr->type)) {
            if (hdr->pn_len < 1 || hdr->pn_len > 4)
                return 0;

            len += hdr->pn_len;
        }

        if (hdr->type == QUIC_PKT_TYPE_INITIAL) {
            enclen = ossl_quic_vlint_encode_len(hdr->token_len);
            if (!enclen)
                return 0;

            len += enclen + hdr->token_len;
        }

        if (!ossl_quic_pkt_type_must_be_last(hdr->type)) {
            enclen = ossl_quic_vlint_encode_len(hdr->len + hdr->pn_len);
            if (!enclen)
                return 0;

            len += enclen;
        }

        return len;
    }
}

int ossl_quic_wire_get_pkt_hdr_dst_conn_id(const unsigned char *buf,
                                           size_t buf_len,
                                           size_t short_conn_id_len,
                                           QUIC_CONN_ID *dst_conn_id)
{
    unsigned char b0;
    size_t blen;

    if (buf_len < QUIC_MIN_VALID_PKT_LEN
        || short_conn_id_len > QUIC_MAX_CONN_ID_LEN)
        return 0;

    b0 = buf[0];
    if ((b0 & 0x80) != 0) {
        /*
         * Long header. We need 6 bytes (initial byte, 4 version bytes, DCID
         * length byte to begin with). This is covered by the buf_len test
         * above.
         */

        /*
         * If the version field is non-zero (meaning that this is not a Version
         * Negotiation packet), the fixed bit must be set.
         */
        if ((buf[1] || buf[2] || buf[3] || buf[4]) && (b0 & 0x40) == 0)
            return 0;

        blen = (size_t)buf[5]; /* DCID Length */
        if (blen > QUIC_MAX_CONN_ID_LEN
            || buf_len < QUIC_MIN_VALID_PKT_LEN + blen)
            return 0;

        dst_conn_id->id_len = (unsigned char)blen;
        memcpy(dst_conn_id->id, buf + 6, blen);
        return 1;
    } else {
        /* Short header. */
        if ((b0 & 0x40) == 0)
            /* Fixed bit not set, not a valid QUIC packet header. */
            return 0;

        if (buf_len < QUIC_MIN_VALID_PKT_LEN_CRYPTO + short_conn_id_len)
            return 0;

        dst_conn_id->id_len = (unsigned char)short_conn_id_len;
        memcpy(dst_conn_id->id, buf + 1, short_conn_id_len);
        return 1;
    }
}

int ossl_quic_wire_decode_pkt_hdr_pn(const unsigned char *enc_pn,
                                     size_t enc_pn_len,
                                     QUIC_PN largest_pn,
                                     QUIC_PN *res_pn)
{
    int64_t expected_pn, truncated_pn, candidate_pn, pn_win, pn_hwin, pn_mask;

    switch (enc_pn_len) {
        case 1:
            truncated_pn = enc_pn[0];
            break;
        case 2:
            truncated_pn = ((QUIC_PN)enc_pn[0] << 8)
                         |  (QUIC_PN)enc_pn[1];
            break;
        case 3:
            truncated_pn = ((QUIC_PN)enc_pn[0] << 16)
                         | ((QUIC_PN)enc_pn[1] << 8)
                         |  (QUIC_PN)enc_pn[2];
            break;
        case 4:
            truncated_pn = ((QUIC_PN)enc_pn[0] << 24)
                         | ((QUIC_PN)enc_pn[1] << 16)
                         | ((QUIC_PN)enc_pn[2] << 8)
                         |  (QUIC_PN)enc_pn[3];
            break;
        default:
            return 0;
    }

    /* Implemented as per RFC 9000 Section A.3. */
    expected_pn     = largest_pn + 1;
    pn_win          = ((int64_t)1) << (enc_pn_len * 8);
    pn_hwin         = pn_win / 2;
    pn_mask         = pn_win - 1;
    candidate_pn    = (expected_pn & ~pn_mask) | truncated_pn;
    if (candidate_pn <= expected_pn - pn_hwin
        && candidate_pn < (((int64_t)1) << 62) - pn_win)
        *res_pn = candidate_pn + pn_win;
    else if (candidate_pn > expected_pn + pn_hwin
             && candidate_pn >= pn_win)
        *res_pn = candidate_pn - pn_win;
    else
        *res_pn = candidate_pn;
    return 1;
}

/* From RFC 9000 Section A.2. Simplified implementation. */
int ossl_quic_wire_determine_pn_len(QUIC_PN pn,
                                    QUIC_PN largest_acked)
{
    uint64_t num_unacked
        = (largest_acked == QUIC_PN_INVALID) ? pn + 1 : pn - largest_acked;

    /*
     * num_unacked \in [    0, 2** 7] -> 1 byte
     * num_unacked \in (2** 7, 2**15] -> 2 bytes
     * num_unacked \in (2**15, 2**23] -> 3 bytes
     * num_unacked \in (2**23,      ] -> 4 bytes
     */

    if (num_unacked <= (1U<<7))  return 1;
    if (num_unacked <= (1U<<15)) return 2;
    if (num_unacked <= (1U<<23)) return 3;
    return 4;
}

int ossl_quic_wire_encode_pkt_hdr_pn(QUIC_PN pn,
                                     unsigned char *enc_pn,
                                     size_t enc_pn_len)
{
    switch (enc_pn_len) {
        case 1:
            enc_pn[0] = (unsigned char)pn;
            break;
        case 2:
            enc_pn[1] = (unsigned char)pn;
            enc_pn[0] = (unsigned char)(pn >> 8);
            break;
        case 3:
            enc_pn[2] = (unsigned char)pn;
            enc_pn[1] = (unsigned char)(pn >> 8);
            enc_pn[0] = (unsigned char)(pn >> 16);
            break;
        case 4:
            enc_pn[3] = (unsigned char)pn;
            enc_pn[2] = (unsigned char)(pn >> 8);
            enc_pn[1] = (unsigned char)(pn >> 16);
            enc_pn[0] = (unsigned char)(pn >> 24);
            break;
        default:
            return 0;
    }

    return 1;
}

int ossl_quic_validate_retry_integrity_tag(OSSL_LIB_CTX *libctx,
                                           const char *propq,
                                           const QUIC_PKT_HDR *hdr,
                                           const QUIC_CONN_ID *client_initial_dcid)
{
    unsigned char expected_tag[QUIC_RETRY_INTEGRITY_TAG_LEN];
    const unsigned char *actual_tag;

    if (hdr == NULL || hdr->len < QUIC_RETRY_INTEGRITY_TAG_LEN)
        return 0;

    if (!ossl_quic_calculate_retry_integrity_tag(libctx, propq,
                                                 hdr, client_initial_dcid,
                                                 expected_tag))
        return 0;

    actual_tag = hdr->data + hdr->len - QUIC_RETRY_INTEGRITY_TAG_LEN;

    return !CRYPTO_memcmp(expected_tag, actual_tag,
                          QUIC_RETRY_INTEGRITY_TAG_LEN);
}

/* RFC 9001 s. 5.8 */
static const unsigned char retry_integrity_key[] = {
    0xbe, 0x0c, 0x69, 0x0b, 0x9f, 0x66, 0x57, 0x5a,
    0x1d, 0x76, 0x6b, 0x54, 0xe3, 0x68, 0xc8, 0x4e
};

static const unsigned char retry_integrity_nonce[] = {
    0x46, 0x15, 0x99, 0xd3, 0x5d, 0x63, 0x2b, 0xf2,
    0x23, 0x98, 0x25, 0xbb
};

int ossl_quic_calculate_retry_integrity_tag(OSSL_LIB_CTX *libctx,
                                            const char *propq,
                                            const QUIC_PKT_HDR *hdr,
                                            const QUIC_CONN_ID *client_initial_dcid,
                                            unsigned char *tag)
{
    EVP_CIPHER *cipher = NULL;
    EVP_CIPHER_CTX *cctx = NULL;
    int ok = 0, l = 0, l2 = 0, wpkt_valid = 0;
    WPACKET wpkt;
    /* Worst case length of the Retry Psuedo-Packet header is 68 bytes. */
    unsigned char buf[128];
    QUIC_PKT_HDR hdr2;
    size_t hdr_enc_len = 0;

    if (hdr->type != QUIC_PKT_TYPE_RETRY || hdr->version == 0
        || hdr->len < QUIC_RETRY_INTEGRITY_TAG_LEN
        || hdr->data == NULL
        || client_initial_dcid == NULL || tag == NULL
        || client_initial_dcid->id_len > QUIC_MAX_CONN_ID_LEN) {
        ERR_raise(ERR_LIB_SSL, ERR_R_PASSED_INVALID_ARGUMENT);
        goto err;
    }

    /*
     * Do not reserve packet body in WPACKET. Retry packet header
     * does not contain a Length field so this does not affect
     * the serialized packet header.
     */
    hdr2 = *hdr;
    hdr2.len = 0;

    /* Assemble retry psuedo-packet. */
    if (!WPACKET_init_static_len(&wpkt, buf, sizeof(buf), 0)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_CRYPTO_LIB);
        goto err;
    }

    wpkt_valid = 1;

    /* Prepend original DCID to the packet. */
    if (!WPACKET_put_bytes_u8(&wpkt, client_initial_dcid->id_len)
        || !WPACKET_memcpy(&wpkt, client_initial_dcid->id,
                           client_initial_dcid->id_len)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_CRYPTO_LIB);
        goto err;
    }

    /* Encode main retry header. */
    if (!ossl_quic_wire_encode_pkt_hdr(&wpkt, hdr2.dst_conn_id.id_len,
                                       &hdr2, NULL))
        goto err;

    if (!WPACKET_get_total_written(&wpkt, &hdr_enc_len)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_CRYPTO_LIB);
        goto err;
    }

    /* Create and initialise cipher context. */
    /* TODO(QUIC FUTURE): Cipher fetch caching. */
    if ((cipher = EVP_CIPHER_fetch(libctx, "AES-128-GCM", propq)) == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_EVP_LIB);
        goto err;
    }

    if ((cctx = EVP_CIPHER_CTX_new()) == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_EVP_LIB);
        goto err;
    }

    if (!EVP_CipherInit_ex(cctx, cipher, NULL,
                           retry_integrity_key, retry_integrity_nonce, /*enc=*/1)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_EVP_LIB);
        goto err;
    }

    /* Feed packet header as AAD data. */
    if (EVP_CipherUpdate(cctx, NULL, &l, buf, hdr_enc_len) != 1) {
        ERR_raise(ERR_LIB_SSL, ERR_R_EVP_LIB);
        goto err;
    }

    /* Feed packet body as AAD data. */
    if (EVP_CipherUpdate(cctx, NULL, &l, hdr->data,
                         hdr->len - QUIC_RETRY_INTEGRITY_TAG_LEN) != 1) {
        ERR_raise(ERR_LIB_SSL, ERR_R_EVP_LIB);
        goto err;
    }

    /* Finalise and get tag. */
    if (EVP_CipherFinal_ex(cctx, NULL, &l2) != 1) {
        ERR_raise(ERR_LIB_SSL, ERR_R_EVP_LIB);
        goto err;
    }

    if (EVP_CIPHER_CTX_ctrl(cctx, EVP_CTRL_AEAD_GET_TAG,
                            QUIC_RETRY_INTEGRITY_TAG_LEN,
                            tag) != 1) {
        ERR_raise(ERR_LIB_SSL, ERR_R_EVP_LIB);
        goto err;
    }

    ok = 1;
err:
    EVP_CIPHER_free(cipher);
    EVP_CIPHER_CTX_free(cctx);
    if (wpkt_valid)
        WPACKET_finish(&wpkt);

    return ok;
}
