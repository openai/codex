/*
 * {- join("\n * ", @autowarntext) -}
 *
 * Copyright 2016-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OPENSSL_CONFIGURATION_H
# define OPENSSL_CONFIGURATION_H
# pragma once

# ifdef  __cplusplus
extern "C" {
# endif

# ifdef OPENSSL_ALGORITHM_DEFINES
#  error OPENSSL_ALGORITHM_DEFINES no longer supported
# endif

/*
 * OpenSSL was configured with the following options:
 */

{- if (@{$config{openssl_sys_defines}}) {
      foreach (@{$config{openssl_sys_defines}}) {
        $OUT .= "# ifndef $_\n";
        $OUT .= "#  define $_ 1\n";
        $OUT .= "# endif\n";
      }
    }
    foreach (@{$config{openssl_api_defines}}) {
        (my $macro, my $value) = $_ =~ /^(.*?)=(.*?)$/;
        $OUT .= "# define $macro $value\n";
    }
    if (@{$config{openssl_feature_defines}}) {
      foreach (@{$config{openssl_feature_defines}}) {
        $OUT .= "# ifndef $_\n";
        $OUT .= "#  define $_\n";
        $OUT .= "# endif\n";
      }
    }
    "";
-}

/* Generate 80386 code? */
{- $config{processor} eq "386" ? "# define" : "# undef" -} I386_ONLY

/*
 * The following are cipher-specific, but are part of the public API.
 */
# if !defined(OPENSSL_SYS_UEFI)
{- $config{bn_ll} ? "#  define" : "#  undef" -} BN_LLONG
/* Only one for the following should be defined */
{- $config{b64l} ? "#  define" : "#  undef" -} SIXTY_FOUR_BIT_LONG
{- $config{b64}  ? "#  define" : "#  undef" -} SIXTY_FOUR_BIT
{- $config{b32}  ? "#  define" : "#  undef" -} THIRTY_TWO_BIT
# endif

# define RC4_INT {- $config{rc4_int} -}

# if defined(OPENSSL_NO_COMP) || (defined(OPENSSL_NO_BROTLI) && defined(OPENSSL_NO_ZSTD) && defined(OPENSSL_NO_ZLIB))
#  define OPENSSL_NO_COMP_ALG
# else
#  undef  OPENSSL_NO_COMP_ALG
# endif

# ifdef  __cplusplus
}
# endif

#endif                          /* OPENSSL_CONFIGURATION_H */
