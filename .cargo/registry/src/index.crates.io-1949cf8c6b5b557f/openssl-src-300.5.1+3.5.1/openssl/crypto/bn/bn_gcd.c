/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/cryptlib.h"
#include "bn_local.h"
#include "internal/constant_time.h"

/*
 * bn_mod_inverse_no_branch is a special version of BN_mod_inverse. It does
 * not contain branches that may leak sensitive information.
 *
 * This is a static function, we ensure all callers in this file pass valid
 * arguments: all passed pointers here are non-NULL.
 */
static ossl_inline
BIGNUM *bn_mod_inverse_no_branch(BIGNUM *in,
                                 const BIGNUM *a, const BIGNUM *n,
                                 BN_CTX *ctx, int *pnoinv)
{
    BIGNUM *A, *B, *X, *Y, *M, *D, *T, *R = NULL;
    BIGNUM *ret = NULL;
    int sign;

    bn_check_top(a);
    bn_check_top(n);

    BN_CTX_start(ctx);
    A = BN_CTX_get(ctx);
    B = BN_CTX_get(ctx);
    X = BN_CTX_get(ctx);
    D = BN_CTX_get(ctx);
    M = BN_CTX_get(ctx);
    Y = BN_CTX_get(ctx);
    T = BN_CTX_get(ctx);
    if (T == NULL)
        goto err;

    if (in == NULL)
        R = BN_new();
    else
        R = in;
    if (R == NULL)
        goto err;

    if (!BN_one(X))
        goto err;
    BN_zero(Y);
    if (BN_copy(B, a) == NULL)
        goto err;
    if (BN_copy(A, n) == NULL)
        goto err;
    A->neg = 0;

    if (B->neg || (BN_ucmp(B, A) >= 0)) {
        /*
         * Turn BN_FLG_CONSTTIME flag on, so that when BN_div is invoked,
         * BN_div_no_branch will be called eventually.
         */
         {
            BIGNUM local_B;
            bn_init(&local_B);
            BN_with_flags(&local_B, B, BN_FLG_CONSTTIME);
            if (!BN_nnmod(B, &local_B, A, ctx))
                goto err;
            /* Ensure local_B goes out of scope before any further use of B */
        }
    }
    sign = -1;
    /*-
     * From  B = a mod |n|,  A = |n|  it follows that
     *
     *      0 <= B < A,
     *     -sign*X*a  ==  B   (mod |n|),
     *      sign*Y*a  ==  A   (mod |n|).
     */

    while (!BN_is_zero(B)) {
        BIGNUM *tmp;

        /*-
         *      0 < B < A,
         * (*) -sign*X*a  ==  B   (mod |n|),
         *      sign*Y*a  ==  A   (mod |n|)
         */

        /*
         * Turn BN_FLG_CONSTTIME flag on, so that when BN_div is invoked,
         * BN_div_no_branch will be called eventually.
         */
        {
            BIGNUM local_A;
            bn_init(&local_A);
            BN_with_flags(&local_A, A, BN_FLG_CONSTTIME);

            /* (D, M) := (A/B, A%B) ... */
            if (!BN_div(D, M, &local_A, B, ctx))
                goto err;
            /* Ensure local_A goes out of scope before any further use of A */
        }

        /*-
         * Now
         *      A = D*B + M;
         * thus we have
         * (**)  sign*Y*a  ==  D*B + M   (mod |n|).
         */

        tmp = A;                /* keep the BIGNUM object, the value does not
                                 * matter */

        /* (A, B) := (B, A mod B) ... */
        A = B;
        B = M;
        /* ... so we have  0 <= B < A  again */

        /*-
         * Since the former  M  is now  B  and the former  B  is now  A,
         * (**) translates into
         *       sign*Y*a  ==  D*A + B    (mod |n|),
         * i.e.
         *       sign*Y*a - D*A  ==  B    (mod |n|).
         * Similarly, (*) translates into
         *      -sign*X*a  ==  A          (mod |n|).
         *
         * Thus,
         *   sign*Y*a + D*sign*X*a  ==  B  (mod |n|),
         * i.e.
         *        sign*(Y + D*X)*a  ==  B  (mod |n|).
         *
         * So if we set  (X, Y, sign) := (Y + D*X, X, -sign), we arrive back at
         *      -sign*X*a  ==  B   (mod |n|),
         *       sign*Y*a  ==  A   (mod |n|).
         * Note that  X  and  Y  stay non-negative all the time.
         */

        if (!BN_mul(tmp, D, X, ctx))
            goto err;
        if (!BN_add(tmp, tmp, Y))
            goto err;

        M = Y;                  /* keep the BIGNUM object, the value does not
                                 * matter */
        Y = X;
        X = tmp;
        sign = -sign;
    }

    /*-
     * The while loop (Euclid's algorithm) ends when
     *      A == gcd(a,n);
     * we have
     *       sign*Y*a  ==  A  (mod |n|),
     * where  Y  is non-negative.
     */

    if (sign < 0) {
        if (!BN_sub(Y, n, Y))
            goto err;
    }
    /* Now  Y*a  ==  A  (mod |n|).  */

    if (BN_is_one(A)) {
        /* Y*a == 1  (mod |n|) */
        if (!Y->neg && BN_ucmp(Y, n) < 0) {
            if (!BN_copy(R, Y))
                goto err;
        } else {
            if (!BN_nnmod(R, Y, n, ctx))
                goto err;
        }
    } else {
        *pnoinv = 1;
        /* caller sets the BN_R_NO_INVERSE error */
        goto err;
    }

    ret = R;
    *pnoinv = 0;

 err:
    if ((ret == NULL) && (in == NULL))
        BN_free(R);
    BN_CTX_end(ctx);
    bn_check_top(ret);
    return ret;
}

/*
 * This is an internal function, we assume all callers pass valid arguments:
 * all pointers passed here are assumed non-NULL.
 */
BIGNUM *int_bn_mod_inverse(BIGNUM *in,
                           const BIGNUM *a, const BIGNUM *n, BN_CTX *ctx,
                           int *pnoinv)
{
    BIGNUM *A, *B, *X, *Y, *M, *D, *T, *R = NULL;
    BIGNUM *ret = NULL;
    int sign;

    /* This is invalid input so we don't worry about constant time here */
    if (BN_abs_is_word(n, 1) || BN_is_zero(n)) {
        *pnoinv = 1;
        return NULL;
    }

    *pnoinv = 0;

    if ((BN_get_flags(a, BN_FLG_CONSTTIME) != 0)
        || (BN_get_flags(n, BN_FLG_CONSTTIME) != 0)) {
        return bn_mod_inverse_no_branch(in, a, n, ctx, pnoinv);
    }

    bn_check_top(a);
    bn_check_top(n);

    BN_CTX_start(ctx);
    A = BN_CTX_get(ctx);
    B = BN_CTX_get(ctx);
    X = BN_CTX_get(ctx);
    D = BN_CTX_get(ctx);
    M = BN_CTX_get(ctx);
    Y = BN_CTX_get(ctx);
    T = BN_CTX_get(ctx);
    if (T == NULL)
        goto err;

    if (in == NULL)
        R = BN_new();
    else
        R = in;
    if (R == NULL)
        goto err;

    if (!BN_one(X))
        goto err;
    BN_zero(Y);
    if (BN_copy(B, a) == NULL)
        goto err;
    if (BN_copy(A, n) == NULL)
        goto err;
    A->neg = 0;
    if (B->neg || (BN_ucmp(B, A) >= 0)) {
        if (!BN_nnmod(B, B, A, ctx))
            goto err;
    }
    sign = -1;
    /*-
     * From  B = a mod |n|,  A = |n|  it follows that
     *
     *      0 <= B < A,
     *     -sign*X*a  ==  B   (mod |n|),
     *      sign*Y*a  ==  A   (mod |n|).
     */

    if (BN_is_odd(n) && (BN_num_bits(n) <= 2048)) {
        /*
         * Binary inversion algorithm; requires odd modulus. This is faster
         * than the general algorithm if the modulus is sufficiently small
         * (about 400 .. 500 bits on 32-bit systems, but much more on 64-bit
         * systems)
         */
        int shift;

        while (!BN_is_zero(B)) {
            /*-
             *      0 < B < |n|,
             *      0 < A <= |n|,
             * (1) -sign*X*a  ==  B   (mod |n|),
             * (2)  sign*Y*a  ==  A   (mod |n|)
             */

            /*
             * Now divide B by the maximum possible power of two in the
             * integers, and divide X by the same value mod |n|. When we're
             * done, (1) still holds.
             */
            shift = 0;
            while (!BN_is_bit_set(B, shift)) { /* note that 0 < B */
                shift++;

                if (BN_is_odd(X)) {
                    if (!BN_uadd(X, X, n))
                        goto err;
                }
                /*
                 * now X is even, so we can easily divide it by two
                 */
                if (!BN_rshift1(X, X))
                    goto err;
            }
            if (shift > 0) {
                if (!BN_rshift(B, B, shift))
                    goto err;
            }

            /*
             * Same for A and Y.  Afterwards, (2) still holds.
             */
            shift = 0;
            while (!BN_is_bit_set(A, shift)) { /* note that 0 < A */
                shift++;

                if (BN_is_odd(Y)) {
                    if (!BN_uadd(Y, Y, n))
                        goto err;
                }
                /* now Y is even */
                if (!BN_rshift1(Y, Y))
                    goto err;
            }
            if (shift > 0) {
                if (!BN_rshift(A, A, shift))
                    goto err;
            }

            /*-
             * We still have (1) and (2).
             * Both  A  and  B  are odd.
             * The following computations ensure that
             *
             *     0 <= B < |n|,
             *      0 < A < |n|,
             * (1) -sign*X*a  ==  B   (mod |n|),
             * (2)  sign*Y*a  ==  A   (mod |n|),
             *
             * and that either  A  or  B  is even in the next iteration.
             */
            if (BN_ucmp(B, A) >= 0) {
                /* -sign*(X + Y)*a == B - A  (mod |n|) */
                if (!BN_uadd(X, X, Y))
                    goto err;
                /*
                 * NB: we could use BN_mod_add_quick(X, X, Y, n), but that
                 * actually makes the algorithm slower
                 */
                if (!BN_usub(B, B, A))
                    goto err;
            } else {
                /*  sign*(X + Y)*a == A - B  (mod |n|) */
                if (!BN_uadd(Y, Y, X))
                    goto err;
                /*
                 * as above, BN_mod_add_quick(Y, Y, X, n) would slow things down
                 */
                if (!BN_usub(A, A, B))
                    goto err;
            }
        }
    } else {
        /* general inversion algorithm */

        while (!BN_is_zero(B)) {
            BIGNUM *tmp;

            /*-
             *      0 < B < A,
             * (*) -sign*X*a  ==  B   (mod |n|),
             *      sign*Y*a  ==  A   (mod |n|)
             */

            /* (D, M) := (A/B, A%B) ... */
            if (BN_num_bits(A) == BN_num_bits(B)) {
                if (!BN_one(D))
                    goto err;
                if (!BN_sub(M, A, B))
                    goto err;
            } else if (BN_num_bits(A) == BN_num_bits(B) + 1) {
                /* A/B is 1, 2, or 3 */
                if (!BN_lshift1(T, B))
                    goto err;
                if (BN_ucmp(A, T) < 0) {
                    /* A < 2*B, so D=1 */
                    if (!BN_one(D))
                        goto err;
                    if (!BN_sub(M, A, B))
                        goto err;
                } else {
                    /* A >= 2*B, so D=2 or D=3 */
                    if (!BN_sub(M, A, T))
                        goto err;
                    if (!BN_add(D, T, B))
                        goto err; /* use D (:= 3*B) as temp */
                    if (BN_ucmp(A, D) < 0) {
                        /* A < 3*B, so D=2 */
                        if (!BN_set_word(D, 2))
                            goto err;
                        /*
                         * M (= A - 2*B) already has the correct value
                         */
                    } else {
                        /* only D=3 remains */
                        if (!BN_set_word(D, 3))
                            goto err;
                        /*
                         * currently M = A - 2*B, but we need M = A - 3*B
                         */
                        if (!BN_sub(M, M, B))
                            goto err;
                    }
                }
            } else {
                if (!BN_div(D, M, A, B, ctx))
                    goto err;
            }

            /*-
             * Now
             *      A = D*B + M;
             * thus we have
             * (**)  sign*Y*a  ==  D*B + M   (mod |n|).
             */

            tmp = A;    /* keep the BIGNUM object, the value does not matter */

            /* (A, B) := (B, A mod B) ... */
            A = B;
            B = M;
            /* ... so we have  0 <= B < A  again */

            /*-
             * Since the former  M  is now  B  and the former  B  is now  A,
             * (**) translates into
             *       sign*Y*a  ==  D*A + B    (mod |n|),
             * i.e.
             *       sign*Y*a - D*A  ==  B    (mod |n|).
             * Similarly, (*) translates into
             *      -sign*X*a  ==  A          (mod |n|).
             *
             * Thus,
             *   sign*Y*a + D*sign*X*a  ==  B  (mod |n|),
             * i.e.
             *        sign*(Y + D*X)*a  ==  B  (mod |n|).
             *
             * So if we set  (X, Y, sign) := (Y + D*X, X, -sign), we arrive back at
             *      -sign*X*a  ==  B   (mod |n|),
             *       sign*Y*a  ==  A   (mod |n|).
             * Note that  X  and  Y  stay non-negative all the time.
             */

            /*
             * most of the time D is very small, so we can optimize tmp := D*X+Y
             */
            if (BN_is_one(D)) {
                if (!BN_add(tmp, X, Y))
                    goto err;
            } else {
                if (BN_is_word(D, 2)) {
                    if (!BN_lshift1(tmp, X))
                        goto err;
                } else if (BN_is_word(D, 4)) {
                    if (!BN_lshift(tmp, X, 2))
                        goto err;
                } else if (D->top == 1) {
                    if (!BN_copy(tmp, X))
                        goto err;
                    if (!BN_mul_word(tmp, D->d[0]))
                        goto err;
                } else {
                    if (!BN_mul(tmp, D, X, ctx))
                        goto err;
                }
                if (!BN_add(tmp, tmp, Y))
                    goto err;
            }

            M = Y;      /* keep the BIGNUM object, the value does not matter */
            Y = X;
            X = tmp;
            sign = -sign;
        }
    }

    /*-
     * The while loop (Euclid's algorithm) ends when
     *      A == gcd(a,n);
     * we have
     *       sign*Y*a  ==  A  (mod |n|),
     * where  Y  is non-negative.
     */

    if (sign < 0) {
        if (!BN_sub(Y, n, Y))
            goto err;
    }
    /* Now  Y*a  ==  A  (mod |n|).  */

    if (BN_is_one(A)) {
        /* Y*a == 1  (mod |n|) */
        if (!Y->neg && BN_ucmp(Y, n) < 0) {
            if (!BN_copy(R, Y))
                goto err;
        } else {
            if (!BN_nnmod(R, Y, n, ctx))
                goto err;
        }
    } else {
        *pnoinv = 1;
        goto err;
    }
    ret = R;
 err:
    if ((ret == NULL) && (in == NULL))
        BN_free(R);
    BN_CTX_end(ctx);
    bn_check_top(ret);
    return ret;
}

/* solves ax == 1 (mod n) */
BIGNUM *BN_mod_inverse(BIGNUM *in,
                       const BIGNUM *a, const BIGNUM *n, BN_CTX *ctx)
{
    BN_CTX *new_ctx = NULL;
    BIGNUM *rv;
    int noinv = 0;

    if (ctx == NULL) {
        ctx = new_ctx = BN_CTX_new_ex(NULL);
        if (ctx == NULL) {
            ERR_raise(ERR_LIB_BN, ERR_R_BN_LIB);
            return NULL;
        }
    }

    rv = int_bn_mod_inverse(in, a, n, ctx, &noinv);
    if (noinv)
        ERR_raise(ERR_LIB_BN, BN_R_NO_INVERSE);
    BN_CTX_free(new_ctx);
    return rv;
}

/*
 * The numbers a and b are coprime if the only positive integer that is a
 * divisor of both of them is 1.
 * i.e. gcd(a,b) = 1.
 *
 * Coprimes have the property: b has a multiplicative inverse modulo a
 * i.e there is some value x such that bx = 1 (mod a).
 *
 * Testing the modulo inverse is currently much faster than the constant
 * time version of BN_gcd().
 */
int BN_are_coprime(BIGNUM *a, const BIGNUM *b, BN_CTX *ctx)
{
    int ret = 0;
    BIGNUM *tmp;

    BN_CTX_start(ctx);
    tmp = BN_CTX_get(ctx);
    if (tmp == NULL)
        goto end;

    ERR_set_mark();
    BN_set_flags(a, BN_FLG_CONSTTIME);
    ret = (BN_mod_inverse(tmp, a, b, ctx) != NULL);
    /* Clear any errors (an error is returned if there is no inverse) */
    ERR_pop_to_mark();
end:
    BN_CTX_end(ctx);
    return ret;
}

/*-
 * This function is based on the constant-time GCD work by Bernstein and Yang:
 * https://eprint.iacr.org/2019/266
 * Generalized fast GCD function to allow even inputs.
 * The algorithm first finds the shared powers of 2 between
 * the inputs, and removes them, reducing at least one of the
 * inputs to an odd value. Then it proceeds to calculate the GCD.
 * Before returning the resulting GCD, we take care of adding
 * back the powers of two removed at the beginning.
 * Note 1: we assume the bit length of both inputs is public information,
 * since access to top potentially leaks this information.
 */
int BN_gcd(BIGNUM *r, const BIGNUM *in_a, const BIGNUM *in_b, BN_CTX *ctx)
{
    BIGNUM *g, *temp = NULL;
    BN_ULONG pow2_numbits, pow2_numbits_temp, pow2_condition_mask, pow2_flag;
    int i, j, top, rlen, glen, m, delta = 1, cond = 0, pow2_shifts, ret = 0;

    /* Note 2: zero input corner cases are not constant-time since they are
     * handled immediately. An attacker can run an attack under this
     * assumption without the need of side-channel information. */
    if (BN_is_zero(in_b)) {
        ret = BN_copy(r, in_a) != NULL;
        r->neg = 0;
        return ret;
    }
    if (BN_is_zero(in_a)) {
        ret = BN_copy(r, in_b) != NULL;
        r->neg = 0;
        return ret;
    }

    bn_check_top(in_a);
    bn_check_top(in_b);

    BN_CTX_start(ctx);
    temp = BN_CTX_get(ctx);
    g = BN_CTX_get(ctx);

    /* make r != 0, g != 0 even, so BN_rshift is not a potential nop */
    if (g == NULL
        || !BN_lshift1(g, in_b)
        || !BN_lshift1(r, in_a))
        goto err;

    /* find shared powers of two, i.e. "shifts" >= 1 */
    pow2_flag = 1;
    pow2_shifts = 0;
    pow2_numbits = 0;
    for (i = 0; i < r->dmax && i < g->dmax; i++) {
        pow2_numbits_temp = r->d[i] | g->d[i];
        pow2_condition_mask = constant_time_is_zero_bn(pow2_flag);
        pow2_flag &= constant_time_is_zero_bn(pow2_numbits_temp);
        pow2_shifts += pow2_flag;
        pow2_numbits = constant_time_select_bn(pow2_condition_mask,
                                               pow2_numbits, pow2_numbits_temp);
    }
    pow2_numbits = ~pow2_numbits;
    pow2_shifts *= BN_BITS2;
    pow2_flag = 1;
    for (j = 0; j < BN_BITS2; j++) {
        pow2_flag &= pow2_numbits;
        pow2_shifts += pow2_flag;
        pow2_numbits >>= 1;
    }

    /* subtract shared powers of two; shifts >= 1 */
    if (!BN_rshift(r, r, pow2_shifts)
        || !BN_rshift(g, g, pow2_shifts))
        goto err;

    /* expand to biggest nword, with room for a possible extra word */
    top = 1 + ((r->top >= g->top) ? r->top : g->top);
    if (bn_wexpand(r, top) == NULL
        || bn_wexpand(g, top) == NULL
        || bn_wexpand(temp, top) == NULL)
        goto err;

    /* re arrange inputs s.t. r is odd */
    BN_consttime_swap((~r->d[0]) & 1, r, g, top);

    /* compute the number of iterations */
    rlen = BN_num_bits(r);
    glen = BN_num_bits(g);
    m = 4 + 3 * ((rlen >= glen) ? rlen : glen);

    for (i = 0; i < m; i++) {
        /* conditionally flip signs if delta is positive and g is odd */
        cond = ((unsigned int)-delta >> (8 * sizeof(delta) - 1)) & g->d[0] & 1
            /* make sure g->top > 0 (i.e. if top == 0 then g == 0 always) */
            & (~((unsigned int)(g->top - 1) >> (sizeof(g->top) * 8 - 1)));
        delta = (-cond & -delta) | ((cond - 1) & delta);
        r->neg ^= cond;
        /* swap */
        BN_consttime_swap(cond, r, g, top);

        /* elimination step */
        delta++;
        if (!BN_add(temp, g, r))
            goto err;
        BN_consttime_swap(g->d[0] & 1 /* g is odd */
                /* make sure g->top > 0 (i.e. if top == 0 then g == 0 always) */
                & (~((unsigned int)(g->top - 1) >> (sizeof(g->top) * 8 - 1))),
                g, temp, top);
        if (!BN_rshift1(g, g))
            goto err;
    }

    /* remove possible negative sign */
    r->neg = 0;
    /* add powers of 2 removed, then correct the artificial shift */
    if (!BN_lshift(r, r, pow2_shifts)
        || !BN_rshift1(r, r))
        goto err;

    ret = 1;

 err:
    BN_CTX_end(ctx);
    bn_check_top(r);
    return ret;
}
