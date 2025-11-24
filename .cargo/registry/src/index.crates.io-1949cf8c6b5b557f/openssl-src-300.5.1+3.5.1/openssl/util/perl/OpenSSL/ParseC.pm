#! /usr/bin/env perl
# Copyright 2018-2024 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

package OpenSSL::ParseC;

use strict;
use warnings;

use Exporter;
use vars qw($VERSION @ISA @EXPORT @EXPORT_OK %EXPORT_TAGS);
$VERSION = "0.9";
@ISA = qw(Exporter);
@EXPORT = qw(parse);

# Global handler data
my @preprocessor_conds;         # A list of simple preprocessor conditions,
                                # each item being a list of macros defined
                                # or not defined.

# Handler helpers
sub all_conds {
    return map { ( @$_ ) } @preprocessor_conds;
}

# A list of handlers that will look at a "complete" string and try to
# figure out what to make of it.
# Each handler is a hash with the following keys:
#
# regexp                a regexp to compare the "complete" string with.
# checker               a function that does a more complex comparison.
#                       Use this instead of regexp if that isn't enough.
# massager              massages the "complete" string into an array with
#                       the following elements:
#
#                       [0]     String that needs further processing (this
#                               applies to typedefs of structs), or empty.
#                       [1]     The name of what was found.
#                       [2]     A character that denotes what type of thing
#                               this is: 'F' for function, 'S' for struct,
#                               'T' for typedef, 'M' for macro, 'V' for
#                               variable.
#                       [3]     Return type (only for type 'F' and 'V')
#                       [4]     Value (for type 'M') or signature (for type 'F',
#                               'V', 'T' or 'S')
#                       [5...]  The list of preprocessor conditions this is
#                               found in, as in checks for macro definitions
#                               (stored as the macro's name) or the absence
#                               of definition (stored as the macro's name
#                               prefixed with a '!'
#
#                       If the massager returns an empty list, it means the
#                       "complete" string has side effects but should otherwise
#                       be ignored.
#                       If the massager is undefined, the "complete" string
#                       should be ignored.
my @opensslcpphandlers = (
    ##################################################################
    # OpenSSL CPP specials
    #
    # These are used to convert certain pre-precessor expressions into
    # others that @cpphandlers have a better chance to understand.

    # This changes any OPENSSL_NO_DEPRECATED_x_y[_z] check to a check of
    # OPENSSL_NO_DEPRECATEDIN_x_y[_z].  That's due to <openssl/macros.h>
    # creating OPENSSL_NO_DEPRECATED_x_y[_z], but the ordinals files using
    # DEPRECATEDIN_x_y[_z].
    { regexp   => qr/#if(def|ndef) OPENSSL_NO_DEPRECATED_(\d+_\d+(?:_\d+)?)$/,
      massager => sub {
          return (<<"EOF");
#if$1 OPENSSL_NO_DEPRECATEDIN_$2
EOF
      }
    }
);
my @cpphandlers = (
    ##################################################################
    # CPP stuff

    { regexp   => qr/#ifdef ?(.*)/,
      massager => sub {
          my %opts;
          if (ref($_[$#_]) eq "HASH") {
              %opts = %{$_[$#_]};
              pop @_;
          }
          push @preprocessor_conds, [ $1 ];
          print STDERR "DEBUG[",$opts{debug_type},"]: preprocessor level: ", scalar(@preprocessor_conds), "\n"
              if $opts{debug};
          return ();
      },
    },
    { regexp   => qr/#ifndef ?(.*)/,
      massager => sub {
          my %opts;
          if (ref($_[$#_]) eq "HASH") {
              %opts = %{$_[$#_]};
              pop @_;
          }
          push @preprocessor_conds, [ '!'.$1 ];
          print STDERR "DEBUG[",$opts{debug_type},"]: preprocessor level: ", scalar(@preprocessor_conds), "\n"
              if $opts{debug};
          return ();
      },
    },
    { regexp   => qr/#if (0|1)/,
      massager => sub {
          my %opts;
          if (ref($_[$#_]) eq "HASH") {
              %opts = %{$_[$#_]};
              pop @_;
          }
          if ($1 eq "1") {
              push @preprocessor_conds, [ "TRUE" ];
          } else {
              push @preprocessor_conds, [ "!TRUE" ];
          }
          print STDERR "DEBUG[",$opts{debug_type},"]: preprocessor level: ", scalar(@preprocessor_conds), "\n"
              if $opts{debug};
          return ();
      },
    },
    { regexp   => qr/#if ?(.*)/,
      massager => sub {
          my %opts;
          if (ref($_[$#_]) eq "HASH") {
              %opts = %{$_[$#_]};
              pop @_;
          }
          my @results = ();
          my $conds = $1;
          if ($conds =~ m|^defined<<<\(([^\)]*)\)>>>(.*)$|) {
              push @results, $1; # Handle the simple case
              my $rest = $2;
              my $re = qr/^(?:\|\|defined<<<\([^\)]*\)>>>)*$/;
              print STDERR "DEBUG[",$opts{debug_type},"]: Matching '$rest' with '$re'\n"
                  if $opts{debug};
              if ($rest =~ m/$re/) {
                  my @rest = split /\|\|/, $rest;
                  shift @rest;
                  foreach (@rest) {
                      m|^defined<<<\(([^\)]*)\)>>>$|;
                      die "Something wrong...$opts{PLACE}" if $1 eq "";
                      push @results, $1;
                  }
              } else {
                  $conds =~ s/<<<|>>>//g;
                  warn "Warning: complicated #if expression(1): $conds$opts{PLACE}"
                      if $opts{warnings};
              }
          } elsif ($conds =~ m|^!defined<<<\(([^\)]*)\)>>>(.*)$|) {
              push @results, '!'.$1; # Handle the simple case
              my $rest = $2;
              my $re = qr/^(?:\&\&!defined<<<\([^\)]*\)>>>)*$/;
              print STDERR "DEBUG[",$opts{debug_type},"]: Matching '$rest' with '$re'\n"
                  if $opts{debug};
              if ($rest =~ m/$re/) {
                  my @rest = split /\&\&/, $rest;
                  shift @rest;
                  foreach (@rest) {
                      m|^!defined<<<\(([^\)]*)\)>>>$|;
                      die "Something wrong...$opts{PLACE}" if $1 eq "";
                      push @results, '!'.$1;
                  }
              } else {
                  $conds =~ s/<<<|>>>//g;
                  warn "Warning: complicated #if expression(2): $conds$opts{PLACE}"
                      if $opts{warnings};
              }
          } else {
              $conds =~ s/<<<|>>>//g;
              warn "Warning: complicated #if expression(3): $conds$opts{PLACE}"
                  if $opts{warnings};
          }
          print STDERR "DEBUG[",$opts{debug_type},"]: Added preprocessor conds: '", join("', '", @results), "'\n"
              if $opts{debug};
          push @preprocessor_conds, [ @results ];
          print STDERR "DEBUG[",$opts{debug_type},"]: preprocessor level: ", scalar(@preprocessor_conds), "\n"
              if $opts{debug};
          return ();
      },
    },
    { regexp   => qr/#elif (.*)/,
      massager => sub {
          my %opts;
          if (ref($_[$#_]) eq "HASH") {
              %opts = %{$_[$#_]};
              pop @_;
          }
          die "An #elif without corresponding condition$opts{PLACE}"
              if !@preprocessor_conds;
          pop @preprocessor_conds;
          print STDERR "DEBUG[",$opts{debug_type},"]: preprocessor level: ", scalar(@preprocessor_conds), "\n"
              if $opts{debug};
          return (<<"EOF");
#if $1
EOF
      },
    },
    { regexp   => qr/#else/,
      massager => sub {
          my %opts;
          if (ref($_[$#_]) eq "HASH") {
              %opts = %{$_[$#_]};
              pop @_;
          }
          die "An #else without corresponding condition$opts{PLACE}"
              if !@preprocessor_conds;
          # Invert all conditions on the last level
          my $stuff = pop @preprocessor_conds;
          push @preprocessor_conds, [
              map { m|^!(.*)$| ? $1 : '!'.$_ } @$stuff
          ];
          print STDERR "DEBUG[",$opts{debug_type},"]: preprocessor level: ", scalar(@preprocessor_conds), "\n"
              if $opts{debug};
          return ();
      },
    },
    { regexp   => qr/#endif ?/,
      massager => sub {
          my %opts;
          if (ref($_[$#_]) eq "HASH") {
              %opts = %{$_[$#_]};
              pop @_;
          }
          die "An #endif without corresponding condition$opts{PLACE}"
              if !@preprocessor_conds;
          pop @preprocessor_conds;
          print STDERR "DEBUG[",$opts{debug_type},"]: preprocessor level: ", scalar(@preprocessor_conds), "\n"
              if $opts{debug};
          return ();
      },
    },
    { regexp   => qr/#define ([[:alpha:]_]\w*)(<<<\(.*?\)>>>)?( (.*))?/,
      massager => sub {
          my $name = $1;
          my $params = $2;
          my $spaceval = $3||"";
          my $val = $4||"";
          return ("",
                  $1, 'M', "", $params ? "$name$params$spaceval" : $val,
                  all_conds()); }
    },
    { regexp   => qr/#.*/,
      massager => sub { return (); }
    },
    );

my @opensslchandlers = (
    ##################################################################
    # OpenSSL C specials
    #
    # They are really preprocessor stuff, but they look like C stuff
    # to this parser.  All of these do replacements, anything else is
    # an error.

    #####
    # Deprecated stuff, by OpenSSL release.

    # OSSL_DEPRECATEDIN_x_y[_z] is simply ignored.  Such declarations are
    # supposed to be guarded with an '#ifdef OPENSSL_NO_DEPRECATED_x_y[_z]'
    { regexp   => qr/OSSL_DEPRECATEDIN_\d+_\d+(?:_\d+)?\s+(.*)/,
      massager => sub { return $1; },
    },
    { regexp   => qr/OSSL_DEPRECATEDIN_\d+_\d+(?:_\d+)?_FOR<<<.*>>>(.*)/,
      massager => sub { return $1; },
    },
    { regexp   => qr/(.*?)\s+OSSL_DEPRECATEDIN_\d+_\d+(?:_\d+)?\s+(.*)/,
      massager => sub { return "$1 $2"; },
    },
    { regexp   => qr/(.*?)\s+OSSL_DEPRECATEDIN_\d+_\d+(?:_\d+)?_FOR<<<.*>>>(.*)/,
      massager => sub { return "$1 $2"; },
    },

    #####
    # Core stuff

    # OSSL_CORE_MAKE_FUNC is a macro to create the necessary data and inline
    # function the libcrypto<->provider interface
    { regexp   => qr/OSSL_CORE_MAKE_FUNC<<<\((.*?),(.*?),(.*?)\)>>>/,
      massager => sub {
          return (<<"EOF");
typedef $1 OSSL_FUNC_$2_fn$3;
static ossl_inline OSSL_FUNC_$2_fn *OSSL_FUNC_$2(const OSSL_DISPATCH *opf);
EOF
      },
    },

    #####
    # LHASH stuff

    # LHASH_OF(foo) is used as a type, but the chandlers won't take it
    # gracefully, so we expand it here.
    { regexp   => qr/(.*)\bLHASH_OF<<<\((.*?)\)>>>(.*)/,
      massager => sub { return ("$1struct lhash_st_$2$3"); }
    },
    { regexp   => qr/DEFINE_LHASH_OF(?:_INTERNAL|_EX)?<<<\((.*)\)>>>/,
      massager => sub {
          return (<<"EOF");
static ossl_inline LHASH_OF($1) * lh_$1_new(unsigned long (*hfn)(const $1 *),
                                            int (*cfn)(const $1 *, const $1 *));
static ossl_inline void lh_$1_free(LHASH_OF($1) *lh);
static ossl_inline $1 *lh_$1_insert(LHASH_OF($1) *lh, $1 *d);
static ossl_inline $1 *lh_$1_delete(LHASH_OF($1) *lh, const $1 *d);
static ossl_inline $1 *lh_$1_retrieve(LHASH_OF($1) *lh, const $1 *d);
static ossl_inline int lh_$1_error(LHASH_OF($1) *lh);
static ossl_inline unsigned long lh_$1_num_items(LHASH_OF($1) *lh);
static ossl_inline void lh_$1_node_stats_bio(const LHASH_OF($1) *lh, BIO *out);
static ossl_inline void lh_$1_node_usage_stats_bio(const LHASH_OF($1) *lh,
                                                   BIO *out);
static ossl_inline void lh_$1_stats_bio(const LHASH_OF($1) *lh, BIO *out);
static ossl_inline unsigned long lh_$1_get_down_load(LHASH_OF($1) *lh);
static ossl_inline void lh_$1_set_down_load(LHASH_OF($1) *lh, unsigned long dl);
static ossl_inline void lh_$1_doall(LHASH_OF($1) *lh, void (*doall)($1 *));
LHASH_OF($1)
EOF
      }
     },

    #####
    # STACK stuff

    # STACK_OF(foo) is used as a type, but the chandlers won't take it
    # gracefully, so we expand it here.
    { regexp   => qr/(.*)\bSTACK_OF<<<\((.*?)\)>>>(.*)/,
      massager => sub { return ("$1struct stack_st_$2$3"); }
    },
#    { regexp   => qr/(.*)\bSTACK_OF\((.*?)\)(.*)/,
#      massager => sub {
#          my $before = $1;
#          my $stack_of = "struct stack_st_$2";
#          my $after = $3;
#          if ($after =~ m|^\w|) { $after = " ".$after; }
#          return ("$before$stack_of$after");
#      }
#    },
    { regexp   => qr/SKM_DEFINE_STACK_OF<<<\((.*),\s*(.*),\s*(.*)\)>>>/,
      massager => sub {
          return (<<"EOF");
STACK_OF($1);
typedef int (*sk_$1_compfunc)(const $3 * const *a, const $3 *const *b);
typedef void (*sk_$1_freefunc)($3 *a);
typedef $3 * (*sk_$1_copyfunc)(const $3 *a);
static ossl_inline int sk_$1_num(const STACK_OF($1) *sk);
static ossl_inline $2 *sk_$1_value(const STACK_OF($1) *sk, int idx);
static ossl_inline STACK_OF($1) *sk_$1_new(sk_$1_compfunc compare);
static ossl_inline STACK_OF($1) *sk_$1_new_null(void);
static ossl_inline STACK_OF($1) *sk_$1_new_reserve(sk_$1_compfunc compare,
                                                   int n);
static ossl_inline int sk_$1_reserve(STACK_OF($1) *sk, int n);
static ossl_inline void sk_$1_free(STACK_OF($1) *sk);
static ossl_inline void sk_$1_zero(STACK_OF($1) *sk);
static ossl_inline $2 *sk_$1_delete(STACK_OF($1) *sk, int i);
static ossl_inline $2 *sk_$1_delete_ptr(STACK_OF($1) *sk, $2 *ptr);
static ossl_inline int sk_$1_push(STACK_OF($1) *sk, $2 *ptr);
static ossl_inline int sk_$1_unshift(STACK_OF($1) *sk, $2 *ptr);
static ossl_inline $2 *sk_$1_pop(STACK_OF($1) *sk);
static ossl_inline $2 *sk_$1_shift(STACK_OF($1) *sk);
static ossl_inline void sk_$1_pop_free(STACK_OF($1) *sk,
                                       sk_$1_freefunc freefunc);
static ossl_inline int sk_$1_insert(STACK_OF($1) *sk, $2 *ptr, int idx);
static ossl_inline $2 *sk_$1_set(STACK_OF($1) *sk, int idx, $2 *ptr);
static ossl_inline int sk_$1_find(STACK_OF($1) *sk, $2 *ptr);
static ossl_inline int sk_$1_find_ex(STACK_OF($1) *sk, $2 *ptr);
static ossl_inline void sk_$1_sort(STACK_OF($1) *sk);
static ossl_inline int sk_$1_is_sorted(const STACK_OF($1) *sk);
static ossl_inline STACK_OF($1) * sk_$1_dup(const STACK_OF($1) *sk);
static ossl_inline STACK_OF($1) *sk_$1_deep_copy(const STACK_OF($1) *sk,
                                                 sk_$1_copyfunc copyfunc,
                                                 sk_$1_freefunc freefunc);
static ossl_inline sk_$1_compfunc sk_$1_set_cmp_func(STACK_OF($1) *sk,
                                                     sk_$1_compfunc compare);
EOF
      }
    },
    { regexp   => qr/SKM_DEFINE_STACK_OF_INTERNAL<<<\((.*),\s*(.*),\s*(.*)\)>>>/,
      massager => sub {
          return (<<"EOF");
STACK_OF($1);
typedef int (*sk_$1_compfunc)(const $3 * const *a, const $3 *const *b);
typedef void (*sk_$1_freefunc)($3 *a);
typedef $3 * (*sk_$1_copyfunc)(const $3 *a);
static ossl_unused ossl_inline $2 *ossl_check_$1_type($2 *ptr);
static ossl_unused ossl_inline const OPENSSL_STACK *ossl_check_const_$1_sk_type(const STACK_OF($1) *sk);
static ossl_unused ossl_inline OPENSSL_sk_compfunc ossl_check_$1_compfunc_type(sk_$1_compfunc cmp);
static ossl_unused ossl_inline OPENSSL_sk_copyfunc ossl_check_$1_copyfunc_type(sk_$1_copyfunc cpy);
static ossl_unused ossl_inline OPENSSL_sk_freefunc ossl_check_$1_freefunc_type(sk_$1_freefunc fr);
EOF
      }
    },
    { regexp   => qr/DEFINE_SPECIAL_STACK_OF<<<\((.*),\s*(.*)\)>>>/,
      massager => sub { return ("SKM_DEFINE_STACK_OF($1,$2,$2)"); },
    },
    { regexp   => qr/DEFINE_STACK_OF<<<\((.*)\)>>>/,
      massager => sub { return ("SKM_DEFINE_STACK_OF($1,$1,$1)"); },
    },
    { regexp   => qr/DEFINE_SPECIAL_STACK_OF_CONST<<<\((.*),\s*(.*)\)>>>/,
      massager => sub { return ("SKM_DEFINE_STACK_OF($1,const $2,$2)"); },
    },
    { regexp   => qr/DEFINE_STACK_OF_CONST<<<\((.*)\)>>>/,
      massager => sub { return ("SKM_DEFINE_STACK_OF($1,const $1,$1)"); },
    },

    #####
    # ASN1 stuff
    { regexp   => qr/DECLARE_ASN1_ITEM<<<\((.*)\)>>>/,
      massager => sub {
          return (<<"EOF");
const ASN1_ITEM *$1_it(void);
EOF
      },
    },
    { regexp   => qr/DECLARE_ASN1_ENCODE_FUNCTIONS_only<<<\((.*),\s*(.*)\)>>>/,
      massager => sub {
          return (<<"EOF");
int d2i_$2(void);
int i2d_$2(void);
EOF
      },
    },
    { regexp   => qr/DECLARE_ASN1_ENCODE_FUNCTIONS<<<\((.*),\s*(.*),\s*(.*)\)>>>/,
      massager => sub {
          return (<<"EOF");
int d2i_$3(void);
int i2d_$3(void);
DECLARE_ASN1_ITEM($2)
EOF
      },
    },
    { regexp   => qr/DECLARE_ASN1_ENCODE_FUNCTIONS_name<<<\((.*),\s*(.*)\)>>>/,
      massager => sub {
          return (<<"EOF");
int d2i_$2(void);
int i2d_$2(void);
DECLARE_ASN1_ITEM($2)
EOF
      },
    },
    { regexp   => qr/DECLARE_ASN1_ALLOC_FUNCTIONS_name<<<\((.*),\s*(.*)\)>>>/,
      massager => sub {
          return (<<"EOF");
int $2_free(void);
int $2_new(void);
EOF
      },
    },
    { regexp   => qr/DECLARE_ASN1_ALLOC_FUNCTIONS<<<\((.*)\)>>>/,
      massager => sub {
          return (<<"EOF");
int $1_free(void);
int $1_new(void);
EOF
      },
    },
    { regexp   => qr/DECLARE_ASN1_FUNCTIONS_name<<<\((.*),\s*(.*)\)>>>/,
      massager => sub {
          return (<<"EOF");
int d2i_$2(void);
int i2d_$2(void);
int $2_free(void);
int $2_new(void);
DECLARE_ASN1_ITEM($2)
EOF
      },
    },
    { regexp   => qr/DECLARE_ASN1_FUNCTIONS<<<\((.*)\)>>>/,
      massager => sub { return (<<"EOF");
int d2i_$1(void);
int i2d_$1(void);
int $1_free(void);
int $1_new(void);
DECLARE_ASN1_ITEM($1)
EOF
      }
    },
    { regexp   => qr/DECLARE_ASN1_NDEF_FUNCTION<<<\((.*)\)>>>/,
      massager => sub {
          return (<<"EOF");
int i2d_$1_NDEF(void);
EOF
      }
    },
    { regexp   => qr/DECLARE_ASN1_PRINT_FUNCTION<<<\((.*)\)>>>/,
      massager => sub {
          return (<<"EOF");
int $1_print_ctx(void);
EOF
      }
    },
    { regexp   => qr/DECLARE_ASN1_PRINT_FUNCTION_name<<<\((.*),\s*(.*)\)>>>/,
      massager => sub {
          return (<<"EOF");
int $2_print_ctx(void);
EOF
      }
    },
    { regexp   => qr/DECLARE_ASN1_SET_OF<<<\((.*)\)>>>/,
      massager => sub { return (); }
    },
    { regexp   => qr/DECLARE_ASN1_DUP_FUNCTION<<<\((.*)\)>>>/,
      massager => sub {
          return (<<"EOF");
int $1_dup(void);
EOF
      }
    },
    { regexp   => qr/DECLARE_ASN1_DUP_FUNCTION_name<<<\((.*),\s*(.*)\)>>>/,
      massager => sub {
          return (<<"EOF");
int $2_dup(void);
EOF
      }
    },
    # Universal translator of attributed PEM declarators
    { regexp   => qr/
          DECLARE_ASN1
          (_ENCODE_FUNCTIONS_only|_ENCODE_FUNCTIONS|_ENCODE_FUNCTIONS_name
           |_ALLOC_FUNCTIONS_name|_ALLOC_FUNCTIONS|_FUNCTIONS_name|_FUNCTIONS
           |_NDEF_FUNCTION|_PRINT_FUNCTION|_PRINT_FUNCTION_name
           |_DUP_FUNCTION|_DUP_FUNCTION_name)
          _attr
          <<<\(\s*OSSL_DEPRECATEDIN_(.*?)\s*,(.*?)\)>>>
      /x,
      massager => sub { return (<<"EOF");
DECLARE_ASN1$1($3)
EOF
      },
    },
    { regexp   => qr/DECLARE_PKCS12_SET_OF<<<\((.*)\)>>>/,
      massager => sub { return (); }
    },

    #####
    # PEM stuff
    { regexp   => qr/DECLARE_PEM(?|_rw|_rw_cb|_rw_const)<<<\((.*?),.*\)>>>/,
      massager => sub { return (<<"EOF");
#ifndef OPENSSL_NO_STDIO
int PEM_read_$1(void);
int PEM_write_$1(void);
#endif
int PEM_read_bio_$1(void);
int PEM_write_bio_$1(void);
EOF
      },
    },
    { regexp   => qr/DECLARE_PEM(?|_rw|_rw_cb|_rw_const)_ex<<<\((.*?),.*\)>>>/,
      massager => sub { return (<<"EOF");
#ifndef OPENSSL_NO_STDIO
int PEM_read_$1(void);
int PEM_write_$1(void);
int PEM_read_$1_ex(void);
int PEM_write_$1_ex(void);
#endif
int PEM_read_bio_$1(void);
int PEM_write_bio_$1(void);
int PEM_read_bio_$1_ex(void);
int PEM_write_bio_$1_ex(void);
EOF
      },
    },
    { regexp   => qr/DECLARE_PEM(?|_write|_write_cb|_write_const)<<<\((.*?),.*\)>>>/,
      massager => sub { return (<<"EOF");
#ifndef OPENSSL_NO_STDIO
int PEM_write_$1(void);
#endif
int PEM_write_bio_$1(void);
EOF
      },
    },
    { regexp   => qr/DECLARE_PEM(?|_write|_write_cb|_write_const)_ex<<<\((.*?),.*\)>>>/,
      massager => sub { return (<<"EOF");
#ifndef OPENSSL_NO_STDIO
int PEM_write_$1(void);
int PEM_write_$1_ex(void);
#endif
int PEM_write_bio_$1(void);
int PEM_write_bio_$1_ex(void);
EOF
      },
    },
    { regexp   => qr/DECLARE_PEM(?|_read|_read_cb)<<<\((.*?),.*\)>>>/,
      massager => sub { return (<<"EOF");
#ifndef OPENSSL_NO_STDIO
int PEM_read_$1(void);
#endif
int PEM_read_bio_$1(void);
EOF
      },
    },
    { regexp   => qr/DECLARE_PEM(?|_read|_read_cb)_ex<<<\((.*?),.*\)>>>/,
      massager => sub { return (<<"EOF");
#ifndef OPENSSL_NO_STDIO
int PEM_read_$1(void);
int PEM_read_$1_ex(void);
#endif
int PEM_read_bio_$1(void);
int PEM_read_bio_$1_ex(void);
EOF
      },
    },
    # Universal translator of attributed PEM declarators
    { regexp   => qr/
          DECLARE_PEM
          ((?:_rw|_rw_cb|_rw_const|_write|_write_cb|_write_const|_read|_read_cb)
           (?:_ex)?)
          _attr
          <<<\(\s*OSSL_DEPRECATEDIN_(.*?)\s*,(.*?)\)>>>
      /x,
      massager => sub { return (<<"EOF");
DECLARE_PEM$1($3)
EOF
      },
    },

    # OpenSSL's declaration of externs with possible export linkage
    # (really only relevant on Windows)
    { regexp   => qr/OPENSSL_(?:EXPORT|EXTERN)/,
      massager => sub { return ("extern"); }
    },

    # Spurious stuff found in the OpenSSL headers
    # Usually, these are just macros that expand to, well, something
    { regexp   => qr/__NDK_FPABI__/,
      massager => sub { return (); }
    },
    );

my $anoncnt = 0;

my @chandlers = (
    ##################################################################
    # C stuff

    # extern "C" of individual items
    # Note that the main parse function has a special hack for 'extern "C" {'
    # which can't be done in handlers
    # We simply ignore it.
    { regexp   => qr/^extern "C" (.*(?:;|>>>))/,
      massager => sub { return ($1); },
    },
    # any other extern is just ignored
    { regexp   => qr/^\s*                       # Any spaces before
                     extern                     # The keyword we look for
                     \b                         # word to non-word boundary
                     .*                         # Anything after
                     ;
                    /x,
      massager => sub { return (); },
    },
    # union, struct and enum definitions
    # Because this one might appear a little everywhere within type
    # definitions, we take it out and replace it with just
    # 'union|struct|enum name' while registering it.
    # This makes use of the parser trick to surround the outer braces
    # with <<< and >>>
    { regexp   => qr/(.*)                       # Anything before       ($1)
                     \b                         # word to non-word boundary
                     (union|struct|enum)        # The word used         ($2)
                     (?:\s([[:alpha:]_]\w*))?   # Struct or enum name   ($3)
                     <<<(\{.*?\})>>>            # Struct or enum definition ($4)
                     (.*)                       # Anything after        ($5)
                     ;
                    /x,
      massager => sub {
          my $before = $1;
          my $word = $2;
          my $name = $3
              || sprintf("__anon%03d", ++$anoncnt); # Anonymous struct
          my $definition = $4;
          my $after = $5;
          my $type = $word eq "struct" ? 'S' : 'E';
          if ($before ne "" || $after ne ";") {
              if ($after =~ m|^\w|) { $after = " ".$after; }
              return ("$before$word $name$after;",
                      "$word $name", $type, "", "$word$definition", all_conds());
          }
          # If there was no before nor after, make the return much simple
          return ("", "$word $name", $type, "", "$word$definition", all_conds());
      }
    },
    # Named struct and enum forward declarations
    # We really just ignore them, but we need to parse them or the variable
    # declaration handler further down will think it's a variable declaration.
    { regexp   => qr/^(union|struct|enum) ([[:alpha:]_]\w*);/,
      massager => sub { return (); }
    },
    # Function returning function pointer declaration
    # This sort of declaration may have a body (inline functions, for example)
    { regexp   => qr/(?:(typedef)\s?)?          # Possible typedef      ($1)
                     ((?:\w|\*|\s)*?)           # Return type           ($2)
                     \s?                        # Possible space
                     <<<\(\*
                     ([[:alpha:]_]\w*)          # Function name         ($3)
                     (\(.*\))                   # Parameters            ($4)
                     \)>>>
                     <<<(\(.*\))>>>             # F.p. parameters       ($5)
                     (?:<<<\{.*\}>>>|;)         # Body or semicolon
                    /x,
      massager => sub {
          return ("", $3, 'T', "", "$2(*$4)$5", all_conds())
              if defined $1;
          return ("", $3, 'F', "$2(*)$5", "$2(*$4)$5", all_conds()); }
    },
    # Function pointer declaration, or typedef thereof
    # This sort of declaration never has a function body
    { regexp   => qr/(?:(typedef)\s?)?          # Possible typedef      ($1)
                     ((?:\w|\*|\s)*?)           # Return type           ($2)
                     <<<\(\*([[:alpha:]_]\w*)\)>>> # T.d. or var name   ($3)
                     <<<(\(.*\))>>>             # F.p. parameters       ($4)
                     ;
                    /x,
      massager => sub {
          return ("", $3, 'T', "", "$2(*)$4", all_conds())
              if defined $1;
          return ("", $3, 'V', "$2(*)$4", "$2(*)$4", all_conds());
      },
    },
    # Function declaration, or typedef thereof
    # This sort of declaration may have a body (inline functions, for example)
    { regexp   => qr/(?:(typedef)\s?)?          # Possible typedef      ($1)
                     ((?:\w|\*|\s)*?)           # Return type           ($2)
                     \s?                        # Possible space
                     ([[:alpha:]_]\w*)          # Function name         ($3)
                     <<<(\(.*\))>>>             # Parameters            ($4)
                     (?:<<<\{.*\}>>>|;)         # Body or semicolon
                    /x,
      massager => sub {
          return ("", $3, 'T', "", "$2$4", all_conds())
              if defined $1;
          return ("", $3, 'F', $2, "$2$4", all_conds());
      },
    },
    # Variable declaration, including arrays, or typedef thereof
    { regexp   => qr/(?:(typedef)\s?)?          # Possible typedef      ($1)
                     ((?:\w|\*|\s)*?)           # Type                  ($2)
                     \s?                        # Possible space
                     ([[:alpha:]_]\w*)          # Variable name         ($3)
                     ((?:<<<\[[^\]]*\]>>>)*)    # Possible array declaration ($4)
                     ;
                    /x,
      massager => sub {
          return ("", $3, 'T', "", $2.($4||""), all_conds())
              if defined $1;
          return ("", $3, 'V', $2.($4||""), $2.($4||""), all_conds());
      },
    },
);

# End handlers are almost the same as handlers, except they are run through
# ONCE when the input has been parsed through.  These are used to check for
# remaining stuff, such as an unfinished #ifdef and stuff like that that the
# main parser can't check on its own.
my @endhandlers = (
    { massager => sub {
        my %opts = %{$_[0]};

        die "Unfinished preprocessor conditions levels: ",scalar(@preprocessor_conds),($opts{filename} ? " in file ".$opts{filename}: ""),$opts{PLACE}
            if @preprocessor_conds;
      }
    }
    );

# takes a list of strings that can each contain one or several lines of code
# also takes a hash of options as last argument.
#
# returns a list of hashes with information:
#
#       name            name of the thing
#       type            type, see the massage handler function
#       returntype      return type of functions and variables
#       value           value for macros, signature for functions, variables
#                       and structs
#       conds           preprocessor conditions (array ref)

sub parse {
    my %opts;
    if (ref($_[$#_]) eq "HASH") {
        %opts = %{$_[$#_]};
        pop @_;
    }
    my %state = (
        in_extern_C => 0,       # An exception to parenthesis processing.
        cpp_parens => [],       # A list of ending parens and braces found in
                                # preprocessor directives
        c_parens => [],         # A list of ending parens and braces found in
                                # C statements
        in_string => "",        # empty string when outside a string, otherwise
                                # "'" or '"' depending on the starting quote.
        in_comment => "",       # empty string when outside a comment, otherwise
                                # "/*" or "//" depending on the type of comment
                                # found.  The latter will never be multiline
                                # NOTE: in_string and in_comment will never be
                                # true (in perl semantics) at the same time.
        current_line => 0,
        );
    my @result = ();
    my $normalized_line = "";   # $input_line, but normalized.  In essence, this
                                # means that ALL whitespace is removed unless
                                # it absolutely has to be present, and in that
                                # case, there's only one space.
                                # The cases where a space needs to stay present
                                # are:
                                # 1. between words
                                # 2. between words and number
                                # 3. after the first word of a preprocessor
                                #    directive.
                                # 4. for the #define directive, between the macro
                                #    name/args and its value, so we end up with:
                                #       #define FOO val
                                #       #define BAR(x) something(x)
    my $collected_stmt = "";    # Where we're building up a C line until it's a
                                # complete definition/declaration, as determined
                                # by any handler being capable of matching it.

    # We use $_ shamelessly when looking through @lines.
    # In case we find a \ at the end, we keep filling it up with more lines.
    $_ = undef;

    foreach my $line (@_) {
        # split tries to be smart when a string ends with the thing we split on
        $line .= "\n" unless $line =~ m|\R$|;
        $line .= "#";

        # We use ¦undef¦ as a marker for a new line from the file.
        # Since we convert one line to several and unshift that into @lines,
        # that's the only safe way we have to track the original lines
        my @lines = map { ( undef, $_ ) } split m|\R|, $line;

        # Remember that extra # we added above?  Now we remove it
        pop @lines;
        pop @lines;             # Don't forget the undef

        while (@lines) {
            if (!defined($lines[0])) {
                shift @lines;
                $state{current_line}++;
                if (!defined($_)) {
                    $opts{PLACE} = " at ".$opts{filename}." line ".$state{current_line}."\n";
                    $opts{PLACE2} = $opts{filename}.":".$state{current_line};
                }
                next;
            }

            $_ = "" unless defined $_;
            $_ .= shift @lines;

            if (m|\\$|) {
                $_ = $`;
                next;
            }

            if ($opts{debug}) {
                print STDERR "DEBUG:----------------------------\n";
                print STDERR "DEBUG: \$_      = '$_'\n";
            }

            ##########################################################
            # Now that we have a full line, let's process through it
            while(1) {
                unless ($state{in_comment}) {
                    # Begin with checking if the current $normalized_line
                    # contains a preprocessor directive
                    # This is only done if we're not inside a comment and
                    # if it's a preprocessor directive and it's finished.
                    if ($normalized_line =~ m|^#| && $_ eq "") {
                        print STDERR "DEBUG[OPENSSL CPP]: \$normalized_line = '$normalized_line'\n"
                            if $opts{debug};
                        $opts{debug_type} = "OPENSSL CPP";
                        my @r = ( _run_handlers($normalized_line,
                                                @opensslcpphandlers,
                                                \%opts) );
                        if (shift @r) {
                            # Checking if there are lines to inject.
                            if (@r) {
                                @r = split $/, (pop @r).$_;
                                print STDERR "DEBUG[OPENSSL CPP]: injecting '", join("', '", @r),"'\n"
                                    if $opts{debug} && @r;
                                @lines = ( @r, @lines );

                                $_ = "";
                            }
                        } else {
                            print STDERR "DEBUG[CPP]: \$normalized_line = '$normalized_line'\n"
                                if $opts{debug};
                            $opts{debug_type} = "CPP";
                            my @r = ( _run_handlers($normalized_line,
                                                    @cpphandlers,
                                                    \%opts) );
                            if (shift @r) {
                                if (ref($r[0]) eq "HASH") {
                                    push @result, shift @r;
                                }

                                # Now, check if there are lines to inject.
                                # Really, this should never happen, it IS a
                                # preprocessor directive after all...
                                if (@r) {
                                    @r = split $/, pop @r;
                                    print STDERR "DEBUG[CPP]: injecting '", join("', '", @r),"'\n"
                                    if $opts{debug} && @r;
                                    @lines = ( @r, @lines );
                                    $_ = "";
                                }
                            }
                        }

                        # Note: we simply ignore all directives that no
                        # handler matches
                        $normalized_line = "";
                    }

                    # If the two strings end and start with a character that
                    # shouldn't get concatenated, add a space
                    my $space =
                        ($collected_stmt =~ m/(?:"|')$/
                         || ($collected_stmt =~ m/(?:\w|\d)$/
                             && $normalized_line =~ m/^(?:\w|\d)/)) ? " " : "";

                    # Now, unless we're building up a preprocessor directive or
                    # are in the middle of a string, or the parens et al aren't
                    # balanced up yet, let's try and see if there's a OpenSSL
                    # or C handler that can make sense of what we have so far.
                    if ( $normalized_line !~ m|^#|
                         && ($collected_stmt ne "" || $normalized_line ne "")
                         && ! @{$state{c_parens}}
                         && ! $state{in_string} ) {
                        if ($opts{debug}) {
                            print STDERR "DEBUG[OPENSSL C]: \$collected_stmt  = '$collected_stmt'\n";
                            print STDERR "DEBUG[OPENSSL C]: \$normalized_line = '$normalized_line'\n";
                        }
                        $opts{debug_type} = "OPENSSL C";
                        my @r = ( _run_handlers($collected_stmt
                                                    .$space
                                                    .$normalized_line,
                                                @opensslchandlers,
                                                \%opts) );
                        if (shift @r) {
                            # Checking if there are lines to inject.
                            if (@r) {
                                @r = split $/, (pop @r).$_;
                                print STDERR "DEBUG[OPENSSL]: injecting '", join("', '", @r),"'\n"
                                    if $opts{debug} && @r;
                                @lines = ( @r, @lines );

                                $_ = "";
                            }
                            $normalized_line = "";
                            $collected_stmt = "";
                        } else {
                            if ($opts{debug}) {
                                print STDERR "DEBUG[C]: \$collected_stmt  = '$collected_stmt'\n";
                                print STDERR "DEBUG[C]: \$normalized_line = '$normalized_line'\n";
                            }
                            $opts{debug_type} = "C";
                            my @r = ( _run_handlers($collected_stmt
                                                        .$space
                                                        .$normalized_line,
                                                    @chandlers,
                                                    \%opts) );
                            if (shift @r) {
                                if (ref($r[0]) eq "HASH") {
                                    push @result, shift @r;
                                }

                                # Checking if there are lines to inject.
                                if (@r) {
                                    @r = split $/, (pop @r).$_;
                                    print STDERR "DEBUG[C]: injecting '", join("', '", @r),"'\n"
                                        if $opts{debug} && @r;
                                    @lines = ( @r, @lines );

                                    $_ = "";
                                }
                                $normalized_line = "";
                                $collected_stmt = "";
                            }
                        }
                    }
                    if ($_ eq "") {
                        $collected_stmt .= $space.$normalized_line;
                        $normalized_line = "";
                    }
                }

                if ($_ eq "") {
                    $_ = undef;
                    last;
                }

                # Take care of inside string first.
                if ($state{in_string}) {
                    if (m/ (?:^|(?<!\\))        # Make sure it's not escaped
                           $state{in_string}    # Look for matching quote
                         /x) {
                        $normalized_line .= $`.$&;
                        $state{in_string} = "";
                        $_ = $';
                        next;
                    } else {
                        die "Unfinished string without continuation found$opts{PLACE}\n";
                    }
                }
                # ... or inside comments, whichever happens to apply
                elsif ($state{in_comment}) {

                    # This should never happen
                    die "Something went seriously wrong, multiline //???$opts{PLACE}\n"
                        if ($state{in_comment} eq "//");

                    # A note: comments are simply discarded.

                    if (m/ (?:^|(?<!\\))        # Make sure it's not escaped
                           \*\/                 # Look for C comment end
                         /x) {
                        $state{in_comment} = "";
                        $_ = $';
                        print STDERR "DEBUG: Found end of comment, followed by '$_'\n"
                            if $opts{debug};
                        next;
                    } else {
                        $_ = "";
                        next;
                    }
                }

                # At this point, it's safe to remove leading whites, but
                # we need to be careful with some preprocessor lines
                if (m|^\s+|) {
                    my $rest = $';
                    my $space = "";
                    $space = " "
                        if ($normalized_line =~ m/^
                                                  \#define\s\w(?:\w|\d)*(?:<<<\([^\)]*\)>>>)?
                                                  | \#[a-z]+
                                                  $/x);
                    print STDERR "DEBUG: Processing leading spaces: \$normalized_line = '$normalized_line', \$space = '$space', \$rest = '$rest'\n"
                        if $opts{debug};
                    $_ = $space.$rest;
                }

                my $parens =
                    $normalized_line =~ m|^#| ? 'cpp_parens' : 'c_parens';
                (my $paren_singular = $parens) =~ s|s$||;

                # Now check for specific tokens, and if they are parens,
                # check them against $state{$parens}.  Note that we surround
                # the outermost parens with extra "<<<" and ">>>".  Those
                # are for the benefit of handlers who to need to detect
                # them, and they will be removed from the final output.
                if (m|^[\{\[\(]|) {
                    my $body = $&;
                    $_ = $';
                    if (!@{$state{$parens}}) {
                        if ("$normalized_line$body" =~ m|^extern "C"\{$|) {
                            $state{in_extern_C} = 1;
                            print STDERR "DEBUG: found start of 'extern \"C\"' ($normalized_line$body)\n"
                                if $opts{debug};
                            $normalized_line = "";
                        } else {
                            $normalized_line .= "<<<".$body;
                        }
                    } else {
                        $normalized_line .= $body;
                    }

                    if ($normalized_line ne "") {
                        print STDERR "DEBUG: found $paren_singular start '$body'\n"
                            if $opts{debug};
                        $body =~ tr|\{\[\(|\}\]\)|;
                        print STDERR "DEBUG: pushing $paren_singular end '$body'\n"
                            if $opts{debug};
                        push @{$state{$parens}}, $body;
                    }
                } elsif (m|^[\}\]\)]|) {
                    $_ = $';

                    if (!@{$state{$parens}}
                        && $& eq '}' && $state{in_extern_C}) {
                        print STDERR "DEBUG: found end of 'extern \"C\"'\n"
                            if $opts{debug};
                        $state{in_extern_C} = 0;
                    } else {
                        print STDERR "DEBUG: Trying to match '$&' against '"
                            ,join("', '", @{$state{$parens}})
                            ,"'\n"
                            if $opts{debug};
                        die "Unmatched parentheses$opts{PLACE}\n"
                            unless (@{$state{$parens}}
                                    && pop @{$state{$parens}} eq $&);
                        if (!@{$state{$parens}}) {
                            $normalized_line .= $&.">>>";
                        } else {
                            $normalized_line .= $&;
                        }
                    }
                } elsif (m|^["']|) { # string start
                    my $body = $&;
                    $_ = $';

                    # We want to separate strings from \w and \d with one space.
                    $normalized_line .= " " if $normalized_line =~ m/(\w|\d)$/;
                    $normalized_line .= $body;
                    $state{in_string} = $body;
                } elsif (m|^\/\*|) { # C style comment
                    print STDERR "DEBUG: found start of C style comment\n"
                        if $opts{debug};
                    $state{in_comment} = $&;
                    $_ = $';
                } elsif (m|^\/\/|) { # C++ style comment
                    print STDERR "DEBUG: found C++ style comment\n"
                        if $opts{debug};
                    $_ = "";    # (just discard it entirely)
                } elsif (m/^ (?| (?: 0[xX][[:xdigit:]]+ | 0[bB][01]+ | [0-9]+ )
                                 (?i: U | L | UL | LL | ULL )?
                               | [0-9]+\.[0-9]+(?:[eE][\-\+]\d+)? (?i: F | L)?
                               ) /x) {
                    print STDERR "DEBUG: Processing numbers: \$normalized_line = '$normalized_line', \$& = '$&', \$' = '$''\n"
                        if $opts{debug};
                    $normalized_line .= $&;
                    $_ = $';
                } elsif (m/^[[:alpha:]_]\w*/) {
                    my $body = $&;
                    my $rest = $';
                    my $space = "";

                    # Now, only add a space if it's needed to separate
                    # two \w characters, and we also surround strings with
                    # a space.  In this case, that's if $normalized_line ends
                    # with a \w, \d, " or '.
                    $space = " "
                        if ($normalized_line =~ m/("|')$/
                            || ($normalized_line =~ m/(\w|\d)$/
                                && $body =~ m/^(\w|\d)/));

                    print STDERR "DEBUG: Processing words: \$normalized_line = '$normalized_line', \$space = '$space', \$body = '$body', \$rest = '$rest'\n"
                        if $opts{debug};
                    $normalized_line .= $space.$body;
                    $_ = $rest;
                } elsif (m|^(?:\\)?.|) { # Catch-all
                    $normalized_line .= $&;
                    $_ = $';
                }
            }
        }
    }
    foreach my $handler (@endhandlers) {
        if ($handler->{massager}) {
            $handler->{massager}->(\%opts);
        }
    }
    return @result;
}

# arg1:    line to check
# arg2...: handlers to check
# return undef when no handler matched
sub _run_handlers {
    my %opts;
    if (ref($_[$#_]) eq "HASH") {
        %opts = %{$_[$#_]};
        pop @_;
    }
    my $line = shift;
    my @handlers = @_;

    foreach my $handler (@handlers) {
        if ($handler->{regexp}
            && $line =~ m|^$handler->{regexp}$|) {
            if ($handler->{massager}) {
                if ($opts{debug}) {
                    print STDERR "DEBUG[",$opts{debug_type},"]: Trying to handle '$line'\n";
                    print STDERR "DEBUG[",$opts{debug_type},"]: (matches /\^",$handler->{regexp},"\$/)\n";
                }
                my $saved_line = $line;
                my @massaged =
                    map { s/(<<<|>>>)//g; $_ }
                    $handler->{massager}->($saved_line, \%opts);
                print STDERR "DEBUG[",$opts{debug_type},"]: Got back '"
                    , join("', '", @massaged), "'\n"
                    if $opts{debug};

                # Because we may get back new lines to be
                # injected before whatever else that follows,
                # and the injected stuff might include
                # preprocessor lines, we need to inject them
                # in @lines and set $_ to the empty string to
                # break out from the inner loops
                my $injected_lines = shift @massaged || "";

                if (@massaged) {
                    return (1,
                            {
                                name    => shift @massaged,
                                type    => shift @massaged,
                                returntype => shift @massaged,
                                value   => shift @massaged,
                                conds   => [ @massaged ]
                            },
                            $injected_lines
                        );
                } else {
                    print STDERR "DEBUG[",$opts{debug_type},"]:   (ignore, possible side effects)\n"
                        if $opts{debug} && $injected_lines eq "";
                    return (1, $injected_lines);
                }
            }
            return (1);
        }
    }
    return (0);
}
