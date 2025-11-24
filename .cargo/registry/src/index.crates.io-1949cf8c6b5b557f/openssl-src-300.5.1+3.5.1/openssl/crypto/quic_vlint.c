#include "internal/quic_vlint.h"
#include "internal/e_os.h"

#ifndef OPENSSL_NO_QUIC

void ossl_quic_vlint_encode_n(uint8_t *buf, uint64_t v, int n)
{
    if (n == 1) {
        buf[0] = (uint8_t)v;
    } else if (n == 2) {
        buf[0] = (uint8_t)(0x40 | ((v >> 8) & 0x3F));
        buf[1] = (uint8_t)v;
    } else if (n == 4) {
        buf[0] = (uint8_t)(0x80 | ((v >> 24) & 0x3F));
        buf[1] = (uint8_t)(v >> 16);
        buf[2] = (uint8_t)(v >>  8);
        buf[3] = (uint8_t)v;
    } else {
        buf[0] = (uint8_t)(0xC0 | ((v >> 56) & 0x3F));
        buf[1] = (uint8_t)(v >> 48);
        buf[2] = (uint8_t)(v >> 40);
        buf[3] = (uint8_t)(v >> 32);
        buf[4] = (uint8_t)(v >> 24);
        buf[5] = (uint8_t)(v >> 16);
        buf[6] = (uint8_t)(v >>  8);
        buf[7] = (uint8_t)v;
    }
}

void ossl_quic_vlint_encode(uint8_t *buf, uint64_t v)
{
    ossl_quic_vlint_encode_n(buf, v, ossl_quic_vlint_encode_len(v));
}

uint64_t ossl_quic_vlint_decode_unchecked(const unsigned char *buf)
{
    uint8_t first_byte = buf[0];
    size_t sz = ossl_quic_vlint_decode_len(first_byte);

    if (sz == 1)
        return first_byte & 0x3F;

    if (sz == 2)
        return ((uint64_t)(first_byte & 0x3F) << 8)
             | buf[1];

    if (sz == 4)
        return ((uint64_t)(first_byte & 0x3F) << 24)
             | ((uint64_t)buf[1] << 16)
             | ((uint64_t)buf[2] <<  8)
             |  buf[3];

    return ((uint64_t)(first_byte & 0x3F) << 56)
         | ((uint64_t)buf[1] << 48)
         | ((uint64_t)buf[2] << 40)
         | ((uint64_t)buf[3] << 32)
         | ((uint64_t)buf[4] << 24)
         | ((uint64_t)buf[5] << 16)
         | ((uint64_t)buf[6] <<  8)
         |  buf[7];
}

int ossl_quic_vlint_decode(const unsigned char *buf, size_t buf_len, uint64_t *v)
{
    size_t dec_len;
    uint64_t x;

    if (buf_len < 1)
        return 0;

    dec_len = ossl_quic_vlint_decode_len(buf[0]);
    if (buf_len < dec_len)
        return 0;

    x = ossl_quic_vlint_decode_unchecked(buf);

    *v = x;
    return dec_len;
}

#endif
