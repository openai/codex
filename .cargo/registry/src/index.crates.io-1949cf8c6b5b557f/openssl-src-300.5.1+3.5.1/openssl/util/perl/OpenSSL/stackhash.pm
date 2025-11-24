#! /usr/bin/env perl
# Copyright 2020-2024 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

package OpenSSL::stackhash;

use strict;
use warnings;

require Exporter;
our @ISA = qw(Exporter);
our @EXPORT_OK = qw(generate_stack_macros generate_const_stack_macros
                    generate_stack_string_macros
                    generate_stack_const_string_macros
                    generate_stack_block_macros
                    generate_lhash_macros);

sub generate_stack_macros_int {
    my $nametype = shift;
    my $realtype = shift;
    my $plaintype = shift;

    my $macros = <<END_MACROS;
SKM_DEFINE_STACK_OF_INTERNAL(${nametype}, ${realtype}, ${plaintype})
#define sk_${nametype}_num(sk) OPENSSL_sk_num(ossl_check_const_${nametype}_sk_type(sk))
#define sk_${nametype}_value(sk, idx) ((${realtype} *)OPENSSL_sk_value(ossl_check_const_${nametype}_sk_type(sk), (idx)))
#define sk_${nametype}_new(cmp) ((STACK_OF(${nametype}) *)OPENSSL_sk_new(ossl_check_${nametype}_compfunc_type(cmp)))
#define sk_${nametype}_new_null() ((STACK_OF(${nametype}) *)OPENSSL_sk_new_null())
#define sk_${nametype}_new_reserve(cmp, n) ((STACK_OF(${nametype}) *)OPENSSL_sk_new_reserve(ossl_check_${nametype}_compfunc_type(cmp), (n)))
#define sk_${nametype}_reserve(sk, n) OPENSSL_sk_reserve(ossl_check_${nametype}_sk_type(sk), (n))
#define sk_${nametype}_free(sk) OPENSSL_sk_free(ossl_check_${nametype}_sk_type(sk))
#define sk_${nametype}_zero(sk) OPENSSL_sk_zero(ossl_check_${nametype}_sk_type(sk))
#define sk_${nametype}_delete(sk, i) ((${realtype} *)OPENSSL_sk_delete(ossl_check_${nametype}_sk_type(sk), (i)))
#define sk_${nametype}_delete_ptr(sk, ptr) ((${realtype} *)OPENSSL_sk_delete_ptr(ossl_check_${nametype}_sk_type(sk), ossl_check_${nametype}_type(ptr)))
#define sk_${nametype}_push(sk, ptr) OPENSSL_sk_push(ossl_check_${nametype}_sk_type(sk), ossl_check_${nametype}_type(ptr))
#define sk_${nametype}_unshift(sk, ptr) OPENSSL_sk_unshift(ossl_check_${nametype}_sk_type(sk), ossl_check_${nametype}_type(ptr))
#define sk_${nametype}_pop(sk) ((${realtype} *)OPENSSL_sk_pop(ossl_check_${nametype}_sk_type(sk)))
#define sk_${nametype}_shift(sk) ((${realtype} *)OPENSSL_sk_shift(ossl_check_${nametype}_sk_type(sk)))
#define sk_${nametype}_pop_free(sk, freefunc) OPENSSL_sk_pop_free(ossl_check_${nametype}_sk_type(sk),ossl_check_${nametype}_freefunc_type(freefunc))
#define sk_${nametype}_insert(sk, ptr, idx) OPENSSL_sk_insert(ossl_check_${nametype}_sk_type(sk), ossl_check_${nametype}_type(ptr), (idx))
#define sk_${nametype}_set(sk, idx, ptr) ((${realtype} *)OPENSSL_sk_set(ossl_check_${nametype}_sk_type(sk), (idx), ossl_check_${nametype}_type(ptr)))
#define sk_${nametype}_find(sk, ptr) OPENSSL_sk_find(ossl_check_${nametype}_sk_type(sk), ossl_check_${nametype}_type(ptr))
#define sk_${nametype}_find_ex(sk, ptr) OPENSSL_sk_find_ex(ossl_check_${nametype}_sk_type(sk), ossl_check_${nametype}_type(ptr))
#define sk_${nametype}_find_all(sk, ptr, pnum) OPENSSL_sk_find_all(ossl_check_${nametype}_sk_type(sk), ossl_check_${nametype}_type(ptr), pnum)
#define sk_${nametype}_sort(sk) OPENSSL_sk_sort(ossl_check_${nametype}_sk_type(sk))
#define sk_${nametype}_is_sorted(sk) OPENSSL_sk_is_sorted(ossl_check_const_${nametype}_sk_type(sk))
#define sk_${nametype}_dup(sk) ((STACK_OF(${nametype}) *)OPENSSL_sk_dup(ossl_check_const_${nametype}_sk_type(sk)))
#define sk_${nametype}_deep_copy(sk, copyfunc, freefunc) ((STACK_OF(${nametype}) *)OPENSSL_sk_deep_copy(ossl_check_const_${nametype}_sk_type(sk), ossl_check_${nametype}_copyfunc_type(copyfunc), ossl_check_${nametype}_freefunc_type(freefunc)))
#define sk_${nametype}_set_cmp_func(sk, cmp) ((sk_${nametype}_compfunc)OPENSSL_sk_set_cmp_func(ossl_check_${nametype}_sk_type(sk), ossl_check_${nametype}_compfunc_type(cmp)))
END_MACROS

    return $macros;
}

sub generate_stack_macros {
    my $type = shift;

    return generate_stack_macros_int($type, $type, $type);
}

sub generate_const_stack_macros {
    my $type = shift;

    return generate_stack_macros_int($type, "const $type", $type);
}

sub generate_stack_string_macros {
    return generate_stack_macros_int("OPENSSL_STRING", "char", "char");
}

sub generate_stack_const_string_macros {
    return generate_stack_macros_int("OPENSSL_CSTRING", "const char", "char");
}

sub generate_stack_block_macros {
    return generate_stack_macros_int("OPENSSL_BLOCK", "void", "void");
}

sub generate_lhash_macros {
    my $type = shift;

    my $macros = <<END_MACROS;
DEFINE_LHASH_OF_INTERNAL(${type});
#define lh_${type}_new(hfn, cmp) ((LHASH_OF(${type}) *)OPENSSL_LH_set_thunks(OPENSSL_LH_new(ossl_check_${type}_lh_hashfunc_type(hfn), ossl_check_${type}_lh_compfunc_type(cmp)), lh_${type}_hash_thunk, lh_${type}_comp_thunk, lh_${type}_doall_thunk, lh_${type}_doall_arg_thunk))
#define lh_${type}_free(lh) OPENSSL_LH_free(ossl_check_${type}_lh_type(lh))
#define lh_${type}_flush(lh) OPENSSL_LH_flush(ossl_check_${type}_lh_type(lh))
#define lh_${type}_insert(lh, ptr) ((${type} *)OPENSSL_LH_insert(ossl_check_${type}_lh_type(lh), ossl_check_${type}_lh_plain_type(ptr)))
#define lh_${type}_delete(lh, ptr) ((${type} *)OPENSSL_LH_delete(ossl_check_${type}_lh_type(lh), ossl_check_const_${type}_lh_plain_type(ptr)))
#define lh_${type}_retrieve(lh, ptr) ((${type} *)OPENSSL_LH_retrieve(ossl_check_${type}_lh_type(lh), ossl_check_const_${type}_lh_plain_type(ptr)))
#define lh_${type}_error(lh) OPENSSL_LH_error(ossl_check_${type}_lh_type(lh))
#define lh_${type}_num_items(lh) OPENSSL_LH_num_items(ossl_check_${type}_lh_type(lh))
#define lh_${type}_node_stats_bio(lh, out) OPENSSL_LH_node_stats_bio(ossl_check_const_${type}_lh_type(lh), out)
#define lh_${type}_node_usage_stats_bio(lh, out) OPENSSL_LH_node_usage_stats_bio(ossl_check_const_${type}_lh_type(lh), out)
#define lh_${type}_stats_bio(lh, out) OPENSSL_LH_stats_bio(ossl_check_const_${type}_lh_type(lh), out)
#define lh_${type}_get_down_load(lh) OPENSSL_LH_get_down_load(ossl_check_${type}_lh_type(lh))
#define lh_${type}_set_down_load(lh, dl) OPENSSL_LH_set_down_load(ossl_check_${type}_lh_type(lh), dl)
#define lh_${type}_doall(lh, dfn) OPENSSL_LH_doall(ossl_check_${type}_lh_type(lh), ossl_check_${type}_lh_doallfunc_type(dfn))
END_MACROS

    return $macros;
}
1;
