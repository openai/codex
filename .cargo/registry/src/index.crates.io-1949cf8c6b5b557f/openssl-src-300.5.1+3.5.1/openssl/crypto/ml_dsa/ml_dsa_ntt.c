/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "ml_dsa_local.h"
#include "ml_dsa_poly.h"

/*
 * This file has multiple parts required for fast matrix multiplication,
 * 1) NTT (See https://eprint.iacr.org/2024/585.pdf)
 * NTT and NTT inverse transformations are Discrete Fourier Transforms in a
 * polynomial ring. Fast-Fourier Transformations can then be applied to make
 * multiplications n log(n). This uses the symmetry of the transformation to
 * reduce computations.
 *
 * 2) Montgomery multiplication
 * The multiplication of a.b mod q requires division by q which is a slow operation.
 *
 * When many multiplications mod q are required montgomery multiplication
 * can be used. This requires a number R > q such that R & q are coprime
 * (i.e. GCD(R, q) = 1), so that division happens using R instead of q.
 * If r is a power of 2 then this division can be done as a bit shift.
 *
 * Given that q = 2^23 - 2^13 + 1
 * We can chose a Montgomery multiplier of R = 2^32.
 *
 * To transform |a| into Montgomery form |m| we use
 *   m = a mod q * ((2^32)*(2^32) mod q)
 * which is then Montgomery reduced, removing the excess factor of R = 2^32.
 */

/*
 * The table in FIPS 204 Appendix B uses the following formula
 * zeta[k]= 1753^bitrev(k) mod q for (k = 1..255) (The first value is not used).
 *
 * As this implementation uses montgomery form with a multiplier of 2^32.
 * The values need to be transformed i.e.
 *
 * zetasMontgomery[k] = reduce_montgomery(zeta[k] * (2^32 * 2^32 mod(q)))
 * reduce_montgomery() is defined below..
 */
static const uint32_t zetas_montgomery[256] = {
    4193792, 25847,   5771523, 7861508, 237124,  7602457, 7504169, 466468,
    1826347, 2353451, 8021166, 6288512, 3119733, 5495562, 3111497, 2680103,
    2725464, 1024112, 7300517, 3585928, 7830929, 7260833, 2619752, 6271868,
    6262231, 4520680, 6980856, 5102745, 1757237, 8360995, 4010497, 280005,
    2706023, 95776,   3077325, 3530437, 6718724, 4788269, 5842901, 3915439,
    4519302, 5336701, 3574422, 5512770, 3539968, 8079950, 2348700, 7841118,
    6681150, 6736599, 3505694, 4558682, 3507263, 6239768, 6779997, 3699596,
    811944,  531354,  954230,  3881043, 3900724, 5823537, 2071892, 5582638,
    4450022, 6851714, 4702672, 5339162, 6927966, 3475950, 2176455, 6795196,
    7122806, 1939314, 4296819, 7380215, 5190273, 5223087, 4747489, 126922,
    3412210, 7396998, 2147896, 2715295, 5412772, 4686924, 7969390, 5903370,
    7709315, 7151892, 8357436, 7072248, 7998430, 1349076, 1852771, 6949987,
    5037034, 264944,  508951,  3097992, 44288,   7280319, 904516,  3958618,
    4656075, 8371839, 1653064, 5130689, 2389356, 8169440, 759969,  7063561,
    189548,  4827145, 3159746, 6529015, 5971092, 8202977, 1315589, 1341330,
    1285669, 6795489, 7567685, 6940675, 5361315, 4499357, 4751448, 3839961,
    2091667, 3407706, 2316500, 3817976, 5037939, 2244091, 5933984, 4817955,
    266997,  2434439, 7144689, 3513181, 4860065, 4621053, 7183191, 5187039,
    900702,  1859098, 909542,  819034,  495491,  6767243, 8337157, 7857917,
    7725090, 5257975, 2031748, 3207046, 4823422, 7855319, 7611795, 4784579,
    342297,  286988,  5942594, 4108315, 3437287, 5038140, 1735879, 203044,
    2842341, 2691481, 5790267, 1265009, 4055324, 1247620, 2486353, 1595974,
    4613401, 1250494, 2635921, 4832145, 5386378, 1869119, 1903435, 7329447,
    7047359, 1237275, 5062207, 6950192, 7929317, 1312455, 3306115, 6417775,
    7100756, 1917081, 5834105, 7005614, 1500165, 777191,  2235880, 3406031,
    7838005, 5548557, 6709241, 6533464, 5796124, 4656147, 594136,  4603424,
    6366809, 2432395, 2454455, 8215696, 1957272, 3369112, 185531,  7173032,
    5196991, 162844,  1616392, 3014001, 810149,  1652634, 4686184, 6581310,
    5341501, 3523897, 3866901, 269760,  2213111, 7404533, 1717735, 472078,
    7953734, 1723600, 6577327, 1910376, 6712985, 7276084, 8119771, 4546524,
    5441381, 6144432, 7959518, 6094090, 183443,  7403526, 1612842, 4834730,
    7826001, 3919660, 8332111, 7018208, 3937738, 1400424, 7534263, 1976782
};

/*
 * @brief When multiplying 2 numbers mod q that are in montgomery form, the
 * product mod q needs to be multiplied by 2^-32 to be in montgomery form.
 * See FIPS 204, Algorithm 49, MontgomeryReduce()
 * Note it is slightly different due to the input range being positive
 *
 * @param a is the result of a multiply of 2 numbers in montgomery form,
 *          in the range 0...(2^32)*q
 * @returns The Montgomery form of 'a' with multiplier 2^32 in the range 0..q-1
 *          The result is congruent to x * 2^-32 mod q
 */
static uint32_t reduce_montgomery(uint64_t a)
{
    uint64_t t = (uint32_t)a * (uint32_t)ML_DSA_Q_NEG_INV; /* a * -qinv */
    uint64_t b = a + t * ML_DSA_Q; /* a - t * q */
    uint32_t c = b >> 32; /* /2^32  = 0..2q */

    return reduce_once(c); /* 0..q */
}

/*
 * @brief Multiply two polynomials in the number theoretically transformed state.
 * See FIPS 204, Algorithm 45, MultiplyNTT()
 * This function has been modified to use montgomery multiplication
 *
 * @param lhs A polynomial multiplicand
 * @param rhs A polynomial multiplier
 * @param out The returned result of the polynomial multiply
 */
void ossl_ml_dsa_poly_ntt_mult(const POLY *lhs, const POLY *rhs, POLY *out)
{
    int i;

    for (i = 0; i < ML_DSA_NUM_POLY_COEFFICIENTS; i++)
        out->coeff[i] =
            reduce_montgomery((uint64_t)lhs->coeff[i] * (uint64_t)rhs->coeff[i]);
}

/*
 * In place number theoretic transform of a given polynomial.
 *
 * See FIPS 204, Algorithm 41, NTT()
 * This function uses montgomery multiplication.
 *
 * @param p a polynomial that is used as the input, that is replaced with
 *        the NTT of the polynomial
 */
void ossl_ml_dsa_poly_ntt(POLY *p)
{
    int i, j, k;
    int step;
    int offset = ML_DSA_NUM_POLY_COEFFICIENTS;

    /* Step: 1, 2, 4, 8, ..., 128 */
    for (step = 1; step < ML_DSA_NUM_POLY_COEFFICIENTS; step <<= 1) {
        k = 0;
        offset >>= 1; /* Offset: 128, 64, 32, 16, ..., 1 */
        for (i = 0; i < step; i++) {
            const uint32_t z_step_root = zetas_montgomery[step + i];

            for (j = k; j < k + offset; j++) {
                uint32_t w_even = p->coeff[j];
                uint32_t t_odd =
                    reduce_montgomery((uint64_t)z_step_root
                                      * (uint64_t)p->coeff[j + offset]);

                p->coeff[j] = reduce_once(w_even + t_odd);
                p->coeff[j + offset] = mod_sub(w_even, t_odd);
            }
            k += 2 * offset;
        }
    }
}

/*
 * @brief In place inverse number theoretic transform of a given polynomial.
 * See FIPS 204, Algorithm 42,  NTT^-1()
 *
 * @param p a polynomial that is used as the input, that is overwritten with
 *          the inverse of the NTT.
 */
void ossl_ml_dsa_poly_ntt_inverse(POLY *p)
{
    /*
     * Step: 128, 64, 32, 16, ..., 1
     * Offset: 1, 2, 4, 8, ..., 128
     */
    int i, j, k, offset, step = ML_DSA_NUM_POLY_COEFFICIENTS;
    /*
     * The multiplicative inverse of 256 mod q, in Montgomery form is
     * ((256^-1 mod q) * ((2^32 * 2^32) mod q)) mod q = (8347681 * 2365951) mod 8380417
     */
    static const uint32_t inverse_degree_montgomery = 41978;

    for (offset = 1; offset < ML_DSA_NUM_POLY_COEFFICIENTS; offset <<= 1) {
        step >>= 1;
        k = 0;
        for (i = 0; i < step; i++) {
            const uint32_t step_root =
                ML_DSA_Q - zetas_montgomery[step + (step - 1 - i)];

            for (j = k; j < k + offset; j++) {
                uint32_t even = p->coeff[j];
                uint32_t odd = p->coeff[j + offset];

                p->coeff[j] = reduce_once(odd + even);
                p->coeff[j + offset] =
                    reduce_montgomery((uint64_t)step_root
                                      * (uint64_t)(ML_DSA_Q + even - odd));
            }
            k += 2 * offset;
        }
    }
    for (i = 0; i < ML_DSA_NUM_POLY_COEFFICIENTS; i++)
        p->coeff[i] = reduce_montgomery((uint64_t)p->coeff[i] *
                                        (uint64_t)inverse_degree_montgomery);
}
