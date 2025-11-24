/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include <string.h>
#include <openssl/byteorder.h>
#include "slh_adrs.h"

/* See FIPS 205 - Section 4.3 Table 1  Uncompressed Addresses */
#define SLH_ADRS_OFF_LAYER_ADR      0
#define SLH_ADRS_OFF_TREE_ADR       4
#define SLH_ADRS_OFF_TYPE           16
#define SLH_ADRS_OFF_KEYPAIR_ADDR   20
#define SLH_ADRS_OFF_CHAIN_ADDR     24
#define SLH_ADRS_OFF_HASH_ADDR      28
#define SLH_ADRS_OFF_TREE_INDEX     SLH_ADRS_OFF_HASH_ADDR
#define SLH_ADRS_SIZE_TYPE          4
/* Number of bytes after type to clear */
#define SLH_ADRS_SIZE_TYPECLEAR     SLH_ADRS_SIZE - (SLH_ADRS_OFF_TYPE + SLH_ADRS_SIZE_TYPE)
#define SLH_ADRS_SIZE_KEYPAIR_ADDR  4

/* See FIPS 205 - Section 11.2 Table 3 Compressed Addresses */
#define SLH_ADRSC_OFF_LAYER_ADR     0
#define SLH_ADRSC_OFF_TREE_ADR      1
#define SLH_ADRSC_OFF_TYPE          9
#define SLH_ADRSC_OFF_KEYPAIR_ADDR  10
#define SLH_ADRSC_OFF_CHAIN_ADDR    14
#define SLH_ADRSC_OFF_HASH_ADDR     18
#define SLH_ADRSC_OFF_TREE_INDEX    SLH_ADRSC_OFF_HASH_ADDR
#define SLH_ADRSC_SIZE_TYPE         1
#define SLH_ADRSC_SIZE_TYPECLEAR    SLH_ADRS_SIZE_TYPECLEAR
#define SLH_ADRSC_SIZE_KEYPAIR_ADDR SLH_ADRS_SIZE_KEYPAIR_ADDR

#define slh_adrs_set_tree_height slh_adrs_set_chain_address
#define slh_adrs_set_tree_index slh_adrs_set_hash_address

#define slh_adrsc_set_tree_height slh_adrsc_set_chain_address
#define slh_adrsc_set_tree_index slh_adrsc_set_hash_address

static OSSL_SLH_ADRS_FUNC_set_layer_address slh_adrs_set_layer_address;
static OSSL_SLH_ADRS_FUNC_set_tree_address slh_adrs_set_tree_address;
static OSSL_SLH_ADRS_FUNC_set_type_and_clear slh_adrs_set_type_and_clear;
static OSSL_SLH_ADRS_FUNC_set_keypair_address slh_adrs_set_keypair_address;
static OSSL_SLH_ADRS_FUNC_copy_keypair_address slh_adrs_copy_keypair_address;
static OSSL_SLH_ADRS_FUNC_set_chain_address slh_adrs_set_chain_address;
static OSSL_SLH_ADRS_FUNC_set_hash_address slh_adrs_set_hash_address;
static OSSL_SLH_ADRS_FUNC_zero slh_adrs_zero;
static OSSL_SLH_ADRS_FUNC_copy slh_adrs_copy;

static OSSL_SLH_ADRS_FUNC_set_layer_address slh_adrsc_set_layer_address;
static OSSL_SLH_ADRS_FUNC_set_tree_address slh_adrsc_set_tree_address;
static OSSL_SLH_ADRS_FUNC_set_type_and_clear slh_adrsc_set_type_and_clear;
static OSSL_SLH_ADRS_FUNC_set_keypair_address slh_adrsc_set_keypair_address;
static OSSL_SLH_ADRS_FUNC_copy_keypair_address slh_adrsc_copy_keypair_address;
static OSSL_SLH_ADRS_FUNC_set_chain_address slh_adrsc_set_chain_address;
static OSSL_SLH_ADRS_FUNC_set_hash_address slh_adrsc_set_hash_address;
static OSSL_SLH_ADRS_FUNC_zero slh_adrsc_zero;
static OSSL_SLH_ADRS_FUNC_copy slh_adrsc_copy;

/*
 * The non compressed versions of the ADRS functions use 32 bytes
 * This is only used by SHAKE.
 */
static void slh_adrs_set_layer_address(uint8_t *adrs, uint32_t layer)
{
    OPENSSL_store_u32_be(adrs + SLH_ADRS_OFF_LAYER_ADR, layer);
}
static void slh_adrs_set_tree_address(uint8_t *adrs, uint64_t address)
{
    /*
     * There are 12 bytes reserved for this - but the largest number
     * used by the parameter sets is only 64 bits. Because this is BE the
     * first 4 of the 12 bytes will be zeros. This assumes that the 4 bytes
     * are zero initially.
     */
    OPENSSL_store_u64_be(adrs + SLH_ADRS_OFF_TREE_ADR + 4, address);
}
static void slh_adrs_set_type_and_clear(uint8_t *adrs, uint32_t type)
{
    OPENSSL_store_u32_be(adrs + SLH_ADRS_OFF_TYPE, type);
    memset(adrs + SLH_ADRS_OFF_TYPE + SLH_ADRS_SIZE_TYPE, 0, SLH_ADRS_SIZE_TYPECLEAR);
}
static void slh_adrs_set_keypair_address(uint8_t *adrs, uint32_t in)
{
    OPENSSL_store_u32_be(adrs + SLH_ADRS_OFF_KEYPAIR_ADDR, in);
}
static void slh_adrs_copy_keypair_address(uint8_t *dst, const uint8_t *src)
{
    memcpy(dst + SLH_ADRS_OFF_KEYPAIR_ADDR, src + SLH_ADRS_OFF_KEYPAIR_ADDR,
           SLH_ADRS_SIZE_KEYPAIR_ADDR);
}
static void slh_adrs_set_chain_address(uint8_t *adrs, uint32_t in)
{
    OPENSSL_store_u32_be(adrs + SLH_ADRS_OFF_CHAIN_ADDR, in);
}
static void slh_adrs_set_hash_address(uint8_t *adrs, uint32_t in)
{
    OPENSSL_store_u32_be(adrs + SLH_ADRS_OFF_HASH_ADDR, in);
}
static void slh_adrs_zero(uint8_t *adrs)
{
    memset(adrs, 0, SLH_ADRS_SIZE);
}
static void slh_adrs_copy(uint8_t *dst, const uint8_t *src)
{
    memcpy(dst, src, SLH_ADRS_SIZE);
}

/* Compressed versions of ADRS functions See Table 3 */
static void slh_adrsc_set_layer_address(uint8_t *adrsc, uint32_t layer)
{
    adrsc[SLH_ADRSC_OFF_LAYER_ADR] = (uint8_t)layer;
}
static void slh_adrsc_set_tree_address(uint8_t *adrsc, uint64_t in)
{
    OPENSSL_store_u64_be(adrsc + SLH_ADRSC_OFF_TREE_ADR, in);
}
static void slh_adrsc_set_type_and_clear(uint8_t *adrsc, uint32_t type)
{
    adrsc[SLH_ADRSC_OFF_TYPE] = (uint8_t)type;
    memset(adrsc + SLH_ADRSC_OFF_TYPE + SLH_ADRSC_SIZE_TYPE, 0, SLH_ADRSC_SIZE_TYPECLEAR);
}
static void slh_adrsc_set_keypair_address(uint8_t *adrsc, uint32_t in)
{
    OPENSSL_store_u32_be(adrsc + SLH_ADRSC_OFF_KEYPAIR_ADDR, in);
}
static void slh_adrsc_copy_keypair_address(uint8_t *dst, const uint8_t *src)
{
    memcpy(dst + SLH_ADRSC_OFF_KEYPAIR_ADDR, src + SLH_ADRSC_OFF_KEYPAIR_ADDR,
           SLH_ADRSC_SIZE_KEYPAIR_ADDR);
}
static void slh_adrsc_set_chain_address(uint8_t *adrsc, uint32_t in)
{
    OPENSSL_store_u32_be(adrsc + SLH_ADRSC_OFF_CHAIN_ADDR, in);
}
static void slh_adrsc_set_hash_address(uint8_t *adrsc, uint32_t in)
{
    OPENSSL_store_u32_be(adrsc + SLH_ADRSC_OFF_HASH_ADDR, in);
}
static void slh_adrsc_zero(uint8_t *adrsc)
{
    memset(adrsc, 0, SLH_ADRSC_SIZE);
}
static void slh_adrsc_copy(uint8_t *dst, const uint8_t *src)
{
    memcpy(dst, src, SLH_ADRSC_SIZE);
}

const SLH_ADRS_FUNC *ossl_slh_get_adrs_fn(int is_compressed)
{
    static const SLH_ADRS_FUNC methods[] = {
        {
            slh_adrs_set_layer_address,
            slh_adrs_set_tree_address,
            slh_adrs_set_type_and_clear,
            slh_adrs_set_keypair_address,
            slh_adrs_copy_keypair_address,
            slh_adrs_set_chain_address,
            slh_adrs_set_tree_height,
            slh_adrs_set_hash_address,
            slh_adrs_set_tree_index,
            slh_adrs_zero,
            slh_adrs_copy,
        },
        {
            slh_adrsc_set_layer_address,
            slh_adrsc_set_tree_address,
            slh_adrsc_set_type_and_clear,
            slh_adrsc_set_keypair_address,
            slh_adrsc_copy_keypair_address,
            slh_adrsc_set_chain_address,
            slh_adrsc_set_tree_height,
            slh_adrsc_set_hash_address,
            slh_adrsc_set_tree_index,
            slh_adrsc_zero,
            slh_adrsc_copy,
        }
    };
    return &methods[is_compressed == 0 ? 0 : 1];
}
