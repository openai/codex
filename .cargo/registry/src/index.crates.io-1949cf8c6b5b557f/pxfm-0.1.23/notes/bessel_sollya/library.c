#include "library.h"
#include <mpfi.h>

__attribute__((visibility("default")))
int bessel_y1(mpfi_t result, mpfi_t x, int n)
{
    mpfr_t a, b;
    mpfr_init2(a, mpfi_get_prec(result));
    mpfr_init2(b, mpfi_get_prec(result));

    mpfi_get_left(a, x);
    mpfr_y1(a, a, GMP_RNDD);
    mpfi_get_right(b, x);
    mpfr_y1(b, b,GMP_RNDU);
    mpfi_interv_fr(result, a, b);

    mpfr_clear(a);
    mpfr_clear(b);
    return 0;
}

#include <stdio.h>
#include <stdlib.h>
#include <mpfi.h>
#include <mpfr.h>

#include <mpfr.h>

void bessel_i1(mpfr_t result, const mpfr_t x, mpfr_prec_t prec, int max_terms, const mpfr_t epsilon)
{
    if (mpfr_sgn(x) == 0)
    {
        mpfr_set_d(result, 0, MPFR_RNDN);
        return;
    }
    mpfr_t sum, term, x_half, x_pow, k_fact, kp1_fact, denom;

    mpfr_inits2(prec, sum, term, x_half, x_pow, k_fact, kp1_fact, denom, (mpfr_ptr)0);
    mpfr_set_ui(sum, 0, MPFR_RNDN);

    // x_half = x / 2
    mpfr_div_ui(x_half, x, 2, MPFR_RNDN);

    // x_pow = x_half^1 = x/2
    mpfr_set(x_pow, x_half, MPFR_RNDN);

    // k_fact = 1, kp1_fact = 1
    mpfr_set_ui(k_fact, 1, MPFR_RNDN);
    mpfr_set_ui(kp1_fact, 1, MPFR_RNDN);

    for (int k = 0; k < max_terms; ++k)
    {
        if (k > 0)
        {
            mpfr_mul_ui(k_fact, k_fact, k, MPFR_RNDN); // k!
            mpfr_mul_ui(kp1_fact, kp1_fact, k + 1, MPFR_RNDN); // (k+1)!
            mpfr_mul(x_pow, x_pow, x_half, MPFR_RNDN);
            mpfr_mul(x_pow, x_pow, x_half, MPFR_RNDN); // (x/2)^{2k+1}
        }

        mpfr_mul(denom, k_fact, kp1_fact, MPFR_RNDN); // denom = k!(k+1)!
        mpfr_div(term, x_pow, denom, MPFR_RNDN); // term = x_pow / denom

        if (mpfr_cmpabs(term, epsilon) < 0)
            break;

        mpfr_add(sum, sum, term, MPFR_RNDN);
    }

    mpfr_set(result, sum, MPFR_RNDN);

    mpfr_clears(sum, term, x_half, x_pow, k_fact, kp1_fact, denom, (mpfr_ptr)0);
}

void bessel_i0_impl(mpfr_t result, const mpfr_t x, mpfr_prec_t prec)
{
    mpfr_t epsilon;
    mpfr_init2(epsilon, prec);
    mpfr_set_ui(epsilon, 1, MPFR_RNDN);
    mpfr_div_2si(epsilon, epsilon, prec + 2, MPFR_RNDN); // ε = 2^(-prec - 2)
    mpfr_t term, sum, k_fact, x_half_pow, x_half;
    mpfr_inits2(prec, term, sum, k_fact, x_half_pow, x_half, (mpfr_ptr)0);

    mpfr_set_ui(sum, 1, MPFR_RNDN); // sum = 1
    mpfr_set_ui(k_fact, 1, MPFR_RNDN); // k! = 1
    mpfr_set_ui(x_half_pow, 1, MPFR_RNDN); // (x/2)^0 = 1

    mpfr_div_ui(x_half, x, 2, MPFR_RNDN); // x/2

    for (int k = 1; k < 1500; ++k)
    {
        // x_half_pow *= (x/2)^2
        mpfr_mul(x_half_pow, x_half_pow, x_half, MPFR_RNDN);
        mpfr_mul(x_half_pow, x_half_pow, x_half, MPFR_RNDN);

        // k_fact *= k
        mpfr_mul_ui(k_fact, k_fact, k, MPFR_RNDN);

        // term = x_half_pow / (k_fact)^2
        mpfr_mul(term, k_fact, k_fact, MPFR_RNDN);
        mpfr_div(term, x_half_pow, term, MPFR_RNDN);

        // sum += term
        mpfr_add(sum, sum, term, MPFR_RNDN);

        if (mpfr_cmp(term, epsilon) < 0) break;
    }

    mpfr_set(result, sum, MPFR_RNDN);
    mpfr_clears(term, k_fact, sum, x_half_pow, x_half, (mpfr_ptr)0);
    mpfr_clear(epsilon);
}

void compute_i0_approximant_asympt(mpfr_t result, const mpfr_t x, mpfr_prec_t prec)
{
    mpfr_t sqrt_x, exp_mx, bessi, recip, ones;
    mpfr_inits2(prec, sqrt_x, exp_mx, bessi, recip, ones, (mpfr_ptr)0);

    mpfr_set_d(ones, 1, MPFR_RNDN);

    // mpfr_div(recip, ones, x, MPFR_RNDN);
    mpfr_sqrt(sqrt_x, x, MPFR_RNDN);
    mpfr_neg(exp_mx, x, MPFR_RNDN);
    mpfr_exp(exp_mx, exp_mx, MPFR_RNDN);

    bessel_i0_impl(bessi, x, prec);

    mpfr_mul(result, sqrt_x, exp_mx, MPFR_RNDN);
    mpfr_mul(result, result, bessi, MPFR_RNDN);

    mpfr_clears(sqrt_x, exp_mx, bessi, recip, ones, (mpfr_ptr)0);
}

void compute_i1_approximant_asympt_big(mpfr_t result, const mpfr_t x, mpfr_prec_t prec)
{
    mpfr_t sqrt_x, exp_mx, bessi, recip, ones, eps;
    mpfr_inits2(prec, sqrt_x, exp_mx, bessi, recip, ones, eps, (mpfr_ptr)0);

    mpfr_set_d(ones, 1, MPFR_RNDN);
    mpfr_set_d(eps, 1e-41, MPFR_RNDN);

    // mpfr_div(recip, ones, x, MPFR_RNDN);
    mpfr_sqrt(sqrt_x, x, MPFR_RNDN);
    mpfr_neg(exp_mx, x, MPFR_RNDN);
    mpfr_exp(exp_mx, exp_mx, MPFR_RNDN);

    bessel_i1(bessi, x, prec, 1500, eps);

    mpfr_mul(result, sqrt_x, exp_mx, MPFR_RNDN);
    mpfr_mul(result, result, bessi, MPFR_RNDN);

    mpfr_clears(sqrt_x, exp_mx, bessi, recip, ones, eps, (mpfr_ptr)0);
}

__attribute__((visibility("default")))
int bessel_i0_approximant(mpfi_t result, mpfi_t x, int n)
{
    mpfr_t a, b;
    mpfr_init2(a, mpfi_get_prec(result));
    mpfr_init2(b, mpfi_get_prec(result));

    mpfi_get_left(a, x);
    compute_i0_approximant_asympt(a, a, mpfi_get_prec(result));

    mpfi_get_right(b, x);
    compute_i0_approximant_asympt(b, b, mpfi_get_prec(result));
    mpfi_interv_fr(result, a, b);

    mpfr_clear(a);
    mpfr_clear(b);
    return 0;
}

__attribute__((visibility("default")))
int bessel_i0(mpfi_t result, mpfi_t x, int n)
{
    mpfr_t a, b;
    mpfr_init2(a, mpfi_get_prec(result));
    mpfr_init2(b, mpfi_get_prec(result));

    mpfi_get_left(a, x);
    bessel_i0_impl(a, a, mpfi_get_prec(result));

    mpfi_get_right(b, x);
    bessel_i0_impl(b, b, mpfi_get_prec(result));
    mpfi_interv_fr(result, a, b);

    mpfr_clear(a);
    mpfr_clear(b);
    return 0;
}

// void compute_i1_approximant_asympt_small(mpfr_t result, const mpfr_t x, mpfr_prec_t prec)
// {
//     if (mpfr_sgn(x) == 0)
//     {
//         mpfr_set_d(result, 0, MPFR_RNDN);
//         return;
//     }
//     mpfr_t p1, eps, p2, p3, two_over_2;
//     mpfr_inits2(prec, p1, eps, p2, p3, two_over_2, (mpfr_ptr)0);
//
//     mpfr_set_d(eps, 1e-41, MPFR_RNDN);
//
//     bessel_i1(p1, x, prec, 1500, eps);
//
//     mpfr_div(p1, p1, x, MPFR_RNDN);
//     mpfr_mul_ui(p1, p1, 2, MPFR_RNDN);
//
//     mpfr_mul_d(two_over_2, x, 0.5, MPFR_RNDN);
//
//     mpfr_mul(p2, two_over_2, two_over_2, MPFR_RNDN);
//
//     mpfr_mul_d(p2, p2, 0.5, MPFR_RNDN);
//
//     mpfr_mul(p3, two_over_2, two_over_2, MPFR_RNDN);
//     mpfr_mul(p3, p3, p3, MPFR_RNDN);
//
//     mpfr_add_d(result, p1, -1, MPFR_RNDN);
//     mpfr_sub(result, result, p2, MPFR_RNDN);
//
//     mpfr_clears(p1, eps, p2, p3,two_over_2, (mpfr_ptr)0);
// }

void compute_i1_approximant_asympt_small(mpfr_t result, const mpfr_t x, mpfr_prec_t prec)
{
    if (mpfr_sgn(x) == 0)
    {
        mpfr_set_d(result, 0.0, MPFR_RNDN);
        return;
    }

    mpfr_t i1x, two_i1x_over_x, y, num, eps, denom, tmp;
    mpfr_inits2(prec, i1x, two_i1x_over_x, y, num, denom, tmp, eps, (mpfr_ptr)0);
    mpfr_set_d(eps, 1e-41, MPFR_RNDN);
    // Compute I1(x)
    bessel_i1(i1x, x, prec, 1500, eps);

    // Compute 2 * I1(x) / x
    mpfr_mul_ui(two_i1x_over_x, i1x, 2, MPFR_RNDN);
    mpfr_div(two_i1x_over_x, two_i1x_over_x, x, MPFR_RNDN);

    // y = (x/2)^2 = (x^2) / 4
    mpfr_sqr(y, x, MPFR_RNDN);
    mpfr_div_ui(y, y, 4, MPFR_RNDN);

    // Numerator = 2*I1(x)/x - 1 - 0.5 * y
    mpfr_set(num, two_i1x_over_x, MPFR_RNDN);
    mpfr_sub_ui(num, num, 1, MPFR_RNDN); // num -= 1
    mpfr_mul_d(tmp, y, 0.5, MPFR_RNDN); // tmp = 0.5 * y
    mpfr_sub(num, num, tmp, MPFR_RNDN); // num -= tmp

    // Denominator = y^2
    mpfr_sqr(denom, y, MPFR_RNDN);

    // Final result: result = num / denom
    mpfr_div(result, num, denom, MPFR_RNDN);

    mpfr_clears(i1x, two_i1x_over_x, y, num, eps, denom, tmp, (mpfr_ptr)0);
}

__attribute__((visibility("default")))
int bessel_i1_approximant_small(mpfi_t result, mpfi_t x, int n)
{
    mpfr_t a, b;
    mpfr_init2(a, mpfi_get_prec(result));
    mpfr_init2(b, mpfi_get_prec(result));

    mpfi_get_left(a, x);
    compute_i1_approximant_asympt_small(a, a, mpfi_get_prec(result));

    mpfi_get_right(b, x);
    compute_i1_approximant_asympt_small(b, b, mpfi_get_prec(result));
    mpfi_interv_fr(result, a, b);

    mpfr_clear(a);
    mpfr_clear(b);
    return 0;
}

__attribute__((visibility("default")))
int bessel_i1_approximant_big(mpfi_t result, mpfi_t x, int n)
{
    mpfr_t a, b;
    mpfr_init2(a, mpfi_get_prec(result));
    mpfr_init2(b, mpfi_get_prec(result));

    mpfr_t eps;
    mpfr_init2(eps, mpfi_get_prec(result));

    mpfr_set_d(eps, 1e-41, MPFR_RNDN);

    mpfi_get_left(a, x);
    compute_i1_approximant_asympt_big(a, a, mpfi_get_prec(result));

    mpfi_get_right(b, x);
    compute_i1_approximant_asympt_big(b, b, mpfi_get_prec(result));
    mpfi_interv_fr(result, a, b);

    mpfr_clear(a);
    mpfr_clear(b);
    mpfr_clear(eps);
    return 0;
}

__attribute__((visibility("default")))
int bessel_j1(mpfi_t result, mpfi_t x, int n)
{
    mpfr_t a, b;
    mpfr_init2(a, mpfi_get_prec(result));
    mpfr_init2(b, mpfi_get_prec(result));

    mpfi_get_left(a, x);
    mpfr_j1(a, a, mpfi_get_prec(result));

    mpfi_get_right(b, x);
    mpfr_j1(b, b, mpfi_get_prec(result));
    mpfi_interv_fr(result, a, b);

    mpfr_clear(a);
    mpfr_clear(b);
    return 0;
}

__attribute__((visibility("default")))
int bessel_j0(mpfi_t result, mpfi_t x, int n)
{
    mpfr_t a, b;
    mpfr_init2(a, mpfi_get_prec(result));
    mpfr_init2(b, mpfi_get_prec(result));

    mpfi_get_left(a, x);
    mpfr_j0(a, a, mpfi_get_prec(result));

    mpfi_get_right(b, x);
    mpfr_j0(b, b, mpfi_get_prec(result));
    mpfi_interv_fr(result, a, b);

    mpfr_clear(a);
    mpfr_clear(b);
    return 0;
}

__attribute__((visibility("default")))
int bessel_y0(mpfi_t result, mpfi_t x, int n)
{
    mpfr_t a, b;
    mpfr_init2(a, mpfi_get_prec(result));
    mpfr_init2(b, mpfi_get_prec(result));

    mpfi_get_left(a, x);
    mpfr_y0(a, a, mpfi_get_prec(result));

    mpfi_get_right(b, x);
    mpfr_y0(b, b, mpfi_get_prec(result));
    mpfi_interv_fr(result, a, b);

    mpfr_clear(a);
    mpfr_clear(b);
    return 0;
}

void harmonic(mpfr_t res, int k, mpfr_prec_t prec)
{
    mpfr_set_ui(res, 0, MPFR_RNDN);
    mpfr_t term;
    mpfr_init2(term, prec);

    for (int i = 1; i <= k; ++i)
    {
        mpfr_set_ui(term, i, MPFR_RNDN);
        mpfr_ui_div(term, 1, term, MPFR_RNDN);
        mpfr_add(res, res, term, MPFR_RNDN);
    }

    mpfr_clear(term);
}

// Compute Euler–Mascheroni constant
void euler_gamma(mpfr_t res, mpfr_prec_t prec)
{
    // Approximate γ ≈ 0.5772...
    mpfr_const_euler(res, MPFR_RNDN); // available in MPFR ≥ 3.0
}

// Compute K₀(x) using series
void bessel_k0_impl(mpfr_t result, const mpfr_t x, mpfr_prec_t prec)
{
    mpfr_t gamma, i0, x2, term, sum, log_term, logx2, psi_k1, k_fact, temp, num, denom;
    mpfr_t epsilon;
    mpfr_inits2(prec, gamma, i0, x2, term, sum, log_term, logx2, psi_k1, k_fact, temp, num, denom, epsilon,
                (mpfr_ptr)0);

    mpfr_set_ui(epsilon, 1, MPFR_RNDN);
    mpfr_div_2si(epsilon, epsilon, prec + 2, MPFR_RNDN); // ε = 2^(-prec - 2)

    euler_gamma(gamma, prec);
    bessel_i0_impl(i0, x, prec);
    mpfr_mul(x2, x, x, MPFR_RNDN);
    mpfr_div_ui(x2, x2, 4, MPFR_RNDN); // x² / 4

    // Compute log(x / 2)
    mpfr_div_ui(logx2, x, 2, MPFR_RNDN);
    mpfr_log(log_term, logx2, MPFR_RNDN);
    mpfr_mul(log_term, log_term, i0, MPFR_RNDN); // -ln(x/2) * I₀(x)

    mpfr_set_ui(sum, 0, MPFR_RNDN);
    mpfr_set_ui(k_fact, 1, MPFR_RNDN); // k!

    for (int k = 0; k < 1500; ++k)
    {
        if (k > 0)
            mpfr_mul_ui(k_fact, k_fact, k, MPFR_RNDN);

        harmonic(psi_k1, k, prec);
        mpfr_sub(psi_k1, psi_k1, gamma, MPFR_RNDN); // ψ(k+1) = H_k - γ

        mpfr_pow_ui(num, x2, k, MPFR_RNDN); // (x²/4)^k
        mpfr_mul(denom, k_fact, k_fact, MPFR_RNDN); // (k!)²
        mpfr_div(term, num, denom, MPFR_RNDN);
        mpfr_mul(term, term, psi_k1, MPFR_RNDN);

        mpfr_add(sum, sum, term, MPFR_RNDN);

        if (mpfr_cmpabs(term, epsilon) < 0) break;
    }

    mpfr_sub(result, sum, log_term, MPFR_RNDN);

    mpfr_clears(gamma, i0, x2, term, sum, log_term, logx2, psi_k1, k_fact, temp, num, denom, epsilon, (mpfr_ptr)0);
}

void bessel_k0_approximant_impl(mpfr_t result, const mpfr_t x, mpfr_prec_t prec)
{
    mpfr_t i0, log0;
    mpfr_inits2(prec, i0, log0, (mpfr_ptr)0);

    mpfr_log(log0, x, MPFR_RNDN);
    bessel_i0_impl(i0, x, prec);
    bessel_k0_impl(result, x, prec);

    mpfr_mul(i0, i0, log0, MPFR_RNDN);

    mpfr_add(result, result, i0, MPFR_RNDN);

    mpfr_clears(i0, log0, (mpfr_ptr)0);
}

void bessel_k0_asympt_impl(mpfr_t result, const mpfr_t x, mpfr_prec_t prec)
{
    mpfr_t tmp;
    mpfr_inits2(prec, tmp, (mpfr_ptr)0);
    bessel_k0_impl(result, x, MPFR_RNDN);
    mpfr_sqrt(tmp, x, MPFR_RNDN);
    mpfr_mul(result, result, tmp, MPFR_RNDN);
    mpfr_exp(tmp, x, MPFR_RNDN);
    mpfr_mul(result, result, tmp, MPFR_RNDN);
    mpfr_clears(tmp, (mpfr_ptr)0);
}

__attribute__((visibility("default")))
int bessel_k0_approximant(mpfi_t result, mpfi_t x, int n)
{
    mpfr_t a, b;
    mpfr_init2(a, mpfi_get_prec(result));
    mpfr_init2(b, mpfi_get_prec(result));

    mpfi_get_left(a, x);
    bessel_k0_approximant_impl(a, a, mpfi_get_prec(result));

    mpfi_get_right(b, x);
    bessel_k0_approximant_impl(b, b, mpfi_get_prec(result));
    mpfi_interv_fr(result, a, b);

    mpfr_clear(a);
    mpfr_clear(b);
    return 0;
}

__attribute__((visibility("default")))
int bessel_k0(mpfi_t result, mpfi_t x, int n)
{
    mpfr_t a, b;
    mpfr_init2(a, mpfi_get_prec(result));
    mpfr_init2(b, mpfi_get_prec(result));

    mpfi_get_left(a, x);
    bessel_k0_impl(a, a, mpfi_get_prec(result));

    mpfi_get_right(b, x);
    bessel_k0_impl(b, b, mpfi_get_prec(result));
    mpfi_interv_fr(result, a, b);

    mpfr_clear(a);
    mpfr_clear(b);
    return 0;
}

__attribute__((visibility("default")))
int bessel_k0_asympt(mpfi_t result, mpfi_t x, int n)
{
    mpfr_t a, b;
    mpfr_init2(a, mpfi_get_prec(result));
    mpfr_init2(b, mpfi_get_prec(result));

    mpfi_get_left(a, x);
    bessel_k0_asympt_impl(a, a, mpfi_get_prec(result));

    mpfi_get_right(b, x);
    bessel_k0_asympt_impl(b, b, mpfi_get_prec(result));
    mpfi_interv_fr(result, a, b);

    mpfr_clear(a);
    mpfr_clear(b);
    return 0;
}

__attribute__((visibility("default")))
int pxfm_gamma(mpfi_t result, mpfi_t x, int n)
{
    mpfr_t a, b;
    mpfr_init2(a, mpfi_get_prec(result));
    mpfr_init2(b, mpfi_get_prec(result));

    mpfi_get_left(a, x);
    mpfr_gamma(a, a, MPFR_RNDN);

    mpfi_get_right(b, x);
    mpfr_gamma(b, b, MPFR_RNDN);
    mpfi_interv_fr(result, a, b);

    mpfr_clear(a);
    mpfr_clear(b);
    return 0;
}
