#ifndef BESSEL_SOLLYA_LIBRARY_H
#define BESSEL_SOLLYA_LIBRARY_H

#include <mpfi.h>

int bessel_y1(mpfi_t result, mpfi_t x, int n);
int bessel_i0_approximant(mpfi_t result, mpfi_t x, int n);
int bessel_i0(mpfi_t result, mpfi_t x, int n);
int bessel_i1_approximant_small(mpfi_t result, mpfi_t x, int n);
int bessel_i1_approximant_big(mpfi_t result, mpfi_t x, int n);
int bessel_j1(mpfi_t result, mpfi_t x, int n);
int bessel_j0(mpfi_t result, mpfi_t x, int n);
int bessel_y0(mpfi_t result, mpfi_t x, int n);
int bessel_k0(mpfi_t result, mpfi_t x, int n);
int bessel_k0_approximant(mpfi_t result, mpfi_t x, int n);
int bessel_k0_asympt(mpfi_t result, mpfi_t x, int n);
int pxfm_gamma(mpfi_t result, mpfi_t x, int n);

#endif //BESSEL_SOLLYA_LIBRARY_H