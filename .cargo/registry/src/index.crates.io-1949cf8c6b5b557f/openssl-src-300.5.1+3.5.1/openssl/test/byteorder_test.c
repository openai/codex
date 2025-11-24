#include <string.h>
#include <openssl/e_os2.h>
#include <openssl/byteorder.h>
#include "testutil.h"
#include "testutil/output.h"

static int test_byteorder(void)
{
    const unsigned char in[] = { 0, 1, 2, 3, 4, 5, 6, 7 };
    unsigned char out[8];
    const unsigned char *restin;
    unsigned char *restout;
    uint16_t u16;
    uint32_t u32;
    uint64_t u64;

    memset(out, 0xff, sizeof(out));
    restin = OPENSSL_load_u16_le(&u16, in);
    restout = OPENSSL_store_u16_le(out, u16);
    if (!TEST_true(u16 == 0x0100U
                   && memcmp(in, out, (size_t) 2) == 0
                   && restin == in + 2
                   && restout == out + 2)) {
        TEST_info("Failed byteorder.h u16 LE load/store");
        return 0;
    }

    memset(out, 0xff, sizeof(out));
    restin = OPENSSL_load_u16_be(&u16, in);
    restout = OPENSSL_store_u16_be(out, u16);
    if (!TEST_true(u16 == 0x0001U
                   && memcmp(in, out, (size_t) 2) == 0
                   && restin == in + 2
                   && restout == out + 2)) {
        TEST_info("Failed byteorder.h u16 BE load/store");
        return 0;
    }

    memset(out, 0xff, sizeof(out));
    restin = OPENSSL_load_u32_le(&u32, in);
    restout = OPENSSL_store_u32_le(out, u32);
    if (!TEST_true(u32 == 0x03020100UL
                   && memcmp(in, out, (size_t) 4) == 0
                   && restin == in + 4
                   && restout == out + 4)) {
        TEST_info("Failed byteorder.h u32 LE load/store");
        return 0;
    }

    memset(out, 0xff, sizeof(out));
    restin = OPENSSL_load_u32_be(&u32, in);
    restout = OPENSSL_store_u32_be(out, u32);
    if (!TEST_true(u32 == 0x00010203UL
                  && memcmp(in, out, (size_t) 4) == 0
                  && restin == in + 4
                  && restout == out + 4)) {
        TEST_info("Failed byteorder.h u32 BE load/store");
        return 0;
    }

    memset(out, 0xff, sizeof(out));
    restin = OPENSSL_load_u64_le(&u64, in);
    restout = OPENSSL_store_u64_le(out, u64);
    if (!TEST_true(u64 == 0x0706050403020100ULL
                   && memcmp(in, out, (size_t) 8) == 0
                   && restin == in + 8
                   && restout == out + 8)) {
        TEST_info("Failed byteorder.h u64 LE load/store");
        return 0;
    }

    memset(out, 0xff, sizeof(out));
    restin = OPENSSL_load_u64_be(&u64, in);
    restout = OPENSSL_store_u64_be(out, u64);
    if (!TEST_true(u64 == 0x0001020304050607ULL
                   && memcmp(in, out, (size_t) 8) == 0
                   && restin == in + 8
                   && restout == out + 8)) {
        TEST_info("Failed byteorder.h u64 BE load/store");
        return 0;
    }
    return 1;
}

int setup_tests(void)
{
    ADD_TEST(test_byteorder);
    return 1;
}
