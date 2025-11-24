/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-sha.c SHA-1 implementation
 *
 * Copyright (C) 2003 Red Hat Inc.
 * Copyright (C) 1995 A. M. Kuchling
 *
 * Licensed under the Academic Free License version 2.1
 *
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 2 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program; if not, write to the Free Software
 * Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301  USA
 *
 */

#include <config.h>
#include "dbus-internals.h"
#include "dbus-sha.h"
#include "dbus-marshal-basic.h" /* for byteswap routines */
#include <string.h>

/* The following comments have the history of where this code
 * comes from. I actually copied it from GNet in GNOME CVS.
 * - hp@redhat.com
 */

/*
 *  sha.h : Implementation of the Secure Hash Algorithm
 *
 * Part of the Python Cryptography Toolkit, version 1.0.0
 *
 * Copyright (C) 1995, A.M. Kuchling
 *
 * Distribute and use freely; there are no restrictions on further
 * dissemination and usage except those imposed by the laws of your
 * country of residence.
 *
 */

/* SHA: NIST's Secure Hash Algorithm */

/* Based on SHA code originally posted to sci.crypt by Peter Gutmann
   in message <30ajo5$oe8@ccu2.auckland.ac.nz>.
   Modified to test for endianness on creation of SHA objects by AMK.
   Also, the original specification of SHA was found to have a weakness
   by NSA/NIST.  This code implements the fixed version of SHA.
*/

/* Here's the first paragraph of Peter Gutmann's posting:

The following is my SHA (FIPS 180) code updated to allow use of the "fixed"
SHA, thanks to Jim Gillogly and an anonymous contributor for the information on
what's changed in the new version.  The fix is a simple change which involves
adding a single rotate in the initial expansion function.  It is unknown
whether this is an optimal solution to the problem which was discovered in the
SHA or whether it's simply a bandaid which fixes the problem with a minimum of
effort (for example the reengineering of a great many Capstone chips).
*/

/**
 * @defgroup DBusSHA SHA implementation
 * @ingroup  DBusInternals
 * @brief SHA-1 hash
 *
 * Types and functions related to computing SHA-1 hash.
 */

/**
 * @defgroup DBusSHAInternals SHA implementation details
 * @ingroup  DBusInternals
 * @brief Internals of SHA implementation.
 *
 * The implementation of SHA-1 (see http://www.itl.nist.gov/fipspubs/fip180-1.htm).
 * This SHA implementation was written by A.M. Kuchling
 *
 * @{
 */

#ifndef DOXYGEN_SHOULD_SKIP_THIS

/* The SHA block size and message digest sizes, in bytes */

#define SHA_DATASIZE    64
#define SHA_DIGESTSIZE  20

/* The SHA f()-functions.  The f1 and f3 functions can be optimized to
   save one boolean operation each - thanks to Rich Schroeppel,
   rcs@cs.arizona.edu for discovering this */

/*#define f1(x,y,z) ( ( x & y ) | ( ~x & z ) )          // Rounds  0-19 */
#define f1(x,y,z)  ( z ^ ( x & ( y ^ z ) ) )           /* Rounds  0-19 */
#define f2(x,y,z)  ( x ^ y ^ z )                       /* Rounds 20-39 */
/*#define f3(x,y,z) ( ( x & y ) | ( x & z ) | ( y & z ) )   // Rounds 40-59 */
#define f3(x,y,z)  ( ( x & y ) | ( z & ( x | y ) ) )   /* Rounds 40-59 */
#define f4(x,y,z)  ( x ^ y ^ z )                       /* Rounds 60-79 */

/* The SHA Mysterious Constants */

#define K1  0x5A827999L                                 /* Rounds  0-19 */
#define K2  0x6ED9EBA1L                                 /* Rounds 20-39 */
#define K3  0x8F1BBCDCL                                 /* Rounds 40-59 */
#define K4  0xCA62C1D6L                                 /* Rounds 60-79 */

/* SHA initial values */

#define h0init  0x67452301L
#define h1init  0xEFCDAB89L
#define h2init  0x98BADCFEL
#define h3init  0x10325476L
#define h4init  0xC3D2E1F0L

/* Note that it may be necessary to add parentheses to these macros if they
   are to be called with expressions as arguments */
/* 32-bit rotate left - kludged with shifts */

#define ROTL(n,X) ( ( ( X ) << n ) | ( ( X ) >> ( 32 - n ) ) )

/* The initial expanding function.  The hash function is defined over an
   80-word expanded input array W, where the first 16 are copies of the input
   data, and the remaining 64 are defined by

        W[ i ] = W[ i - 16 ] ^ W[ i - 14 ] ^ W[ i - 8 ] ^ W[ i - 3 ]

   This implementation generates these values on the fly in a circular
   buffer - thanks to Colin Plumb, colin@nyx10.cs.du.edu for this
   optimization.

   The updated SHA changes the expanding function by adding a rotate of 1
   bit.  Thanks to Jim Gillogly, jim@rand.org, and an anonymous contributor
   for this information */

#define expand(W,i) ( W[ i & 15 ] = ROTL( 1, ( W[ i & 15 ] ^ W[ (i - 14) & 15 ] ^ \
                                                 W[ (i - 8) & 15 ] ^ W[ (i - 3) & 15 ] ) ) )


/* The prototype SHA sub-round.  The fundamental sub-round is:

        a' = e + ROTL( 5, a ) + f( b, c, d ) + k + data;
        b' = a;
        c' = ROTL( 30, b );
        d' = c;
        e' = d;

   but this is implemented by unrolling the loop 5 times and renaming the
   variables ( e, a, b, c, d ) = ( a', b', c', d', e' ) each iteration.
   This code is then replicated 20 times for each of the 4 functions, using
   the next 20 values from the W[] array each time */

#define subRound(a, b, c, d, e, f, k, data) \
   ( e += ROTL( 5, a ) + f( b, c, d ) + k + data, b = ROTL( 30, b ) )

#endif /* !DOXYGEN_SHOULD_SKIP_THIS */

/* Perform the SHA transformation.  Note that this code, like MD5, seems to
   break some optimizing compilers due to the complexity of the expressions
   and the size of the basic block.  It may be necessary to split it into
   sections, e.g. based on the four subrounds

   Note that this corrupts the context->data area */

static void
SHATransform(dbus_uint32_t *digest, dbus_uint32_t *data)
{
  dbus_uint32_t A, B, C, D, E;     /* Local vars */
  dbus_uint32_t eData[16];       /* Expanded data */

  /* Set up first buffer and local data buffer */
  A = digest[0];
  B = digest[1];
  C = digest[2];
  D = digest[3];
  E = digest[4];
  memmove (eData, data, SHA_DATASIZE);

  /* Heavy mangling, in 4 sub-rounds of 20 interations each. */
  subRound (A, B, C, D, E, f1, K1, eData[0]);
  subRound (E, A, B, C, D, f1, K1, eData[1]);
  subRound (D, E, A, B, C, f1, K1, eData[2]);
  subRound (C, D, E, A, B, f1, K1, eData[3]);
  subRound (B, C, D, E, A, f1, K1, eData[4]);
  subRound (A, B, C, D, E, f1, K1, eData[5]);
  subRound (E, A, B, C, D, f1, K1, eData[6]);
  subRound (D, E, A, B, C, f1, K1, eData[7]);
  subRound (C, D, E, A, B, f1, K1, eData[8]);
  subRound (B, C, D, E, A, f1, K1, eData[9]);
  subRound (A, B, C, D, E, f1, K1, eData[10]);
  subRound (E, A, B, C, D, f1, K1, eData[11]);
  subRound (D, E, A, B, C, f1, K1, eData[12]);
  subRound (C, D, E, A, B, f1, K1, eData[13]);
  subRound (B, C, D, E, A, f1, K1, eData[14]);
  subRound (A, B, C, D, E, f1, K1, eData[15]);
  subRound (E, A, B, C, D, f1, K1, expand ( eData, 16) );
  subRound (D, E, A, B, C, f1, K1, expand ( eData, 17) );
  subRound (C, D, E, A, B, f1, K1, expand ( eData, 18) );
  subRound (B, C, D, E, A, f1, K1, expand ( eData, 19) );

  subRound (A, B, C, D, E, f2, K2, expand ( eData, 20) );
  subRound (E, A, B, C, D, f2, K2, expand ( eData, 21) );
  subRound (D, E, A, B, C, f2, K2, expand ( eData, 22) );
  subRound (C, D, E, A, B, f2, K2, expand ( eData, 23) );
  subRound (B, C, D, E, A, f2, K2, expand ( eData, 24) );
  subRound (A, B, C, D, E, f2, K2, expand ( eData, 25) );
  subRound (E, A, B, C, D, f2, K2, expand ( eData, 26) );
  subRound (D, E, A, B, C, f2, K2, expand ( eData, 27) );
  subRound (C, D, E, A, B, f2, K2, expand ( eData, 28) );
  subRound (B, C, D, E, A, f2, K2, expand ( eData, 29) );
  subRound (A, B, C, D, E, f2, K2, expand ( eData, 30) );
  subRound (E, A, B, C, D, f2, K2, expand ( eData, 31) );
  subRound (D, E, A, B, C, f2, K2, expand ( eData, 32) );
  subRound (C, D, E, A, B, f2, K2, expand ( eData, 33) );
  subRound (B, C, D, E, A, f2, K2, expand ( eData, 34) );
  subRound (A, B, C, D, E, f2, K2, expand ( eData, 35) );
  subRound (E, A, B, C, D, f2, K2, expand ( eData, 36) );
  subRound (D, E, A, B, C, f2, K2, expand ( eData, 37) );
  subRound (C, D, E, A, B, f2, K2, expand ( eData, 38) );
  subRound (B, C, D, E, A, f2, K2, expand ( eData, 39) );

  subRound (A, B, C, D, E, f3, K3, expand ( eData, 40) );
  subRound (E, A, B, C, D, f3, K3, expand ( eData, 41) );
  subRound (D, E, A, B, C, f3, K3, expand ( eData, 42) );
  subRound (C, D, E, A, B, f3, K3, expand ( eData, 43) );
  subRound (B, C, D, E, A, f3, K3, expand ( eData, 44) );
  subRound (A, B, C, D, E, f3, K3, expand ( eData, 45) );
  subRound (E, A, B, C, D, f3, K3, expand ( eData, 46) );
  subRound (D, E, A, B, C, f3, K3, expand ( eData, 47) );
  subRound (C, D, E, A, B, f3, K3, expand ( eData, 48) );
  subRound (B, C, D, E, A, f3, K3, expand ( eData, 49) );
  subRound (A, B, C, D, E, f3, K3, expand ( eData, 50) );
  subRound (E, A, B, C, D, f3, K3, expand ( eData, 51) );
  subRound (D, E, A, B, C, f3, K3, expand ( eData, 52) );
  subRound (C, D, E, A, B, f3, K3, expand ( eData, 53) );
  subRound (B, C, D, E, A, f3, K3, expand ( eData, 54) );
  subRound (A, B, C, D, E, f3, K3, expand ( eData, 55) );
  subRound (E, A, B, C, D, f3, K3, expand ( eData, 56) );
  subRound (D, E, A, B, C, f3, K3, expand ( eData, 57) );
  subRound (C, D, E, A, B, f3, K3, expand ( eData, 58) );
  subRound (B, C, D, E, A, f3, K3, expand ( eData, 59) );

  subRound (A, B, C, D, E, f4, K4, expand ( eData, 60) );
  subRound (E, A, B, C, D, f4, K4, expand ( eData, 61) );
  subRound (D, E, A, B, C, f4, K4, expand ( eData, 62) );
  subRound (C, D, E, A, B, f4, K4, expand ( eData, 63) );
  subRound (B, C, D, E, A, f4, K4, expand ( eData, 64) );
  subRound (A, B, C, D, E, f4, K4, expand ( eData, 65) );
  subRound (E, A, B, C, D, f4, K4, expand ( eData, 66) );
  subRound (D, E, A, B, C, f4, K4, expand ( eData, 67) );
  subRound (C, D, E, A, B, f4, K4, expand ( eData, 68) );
  subRound (B, C, D, E, A, f4, K4, expand ( eData, 69) );
  subRound (A, B, C, D, E, f4, K4, expand ( eData, 70) );
  subRound (E, A, B, C, D, f4, K4, expand ( eData, 71) );
  subRound (D, E, A, B, C, f4, K4, expand ( eData, 72) );
  subRound (C, D, E, A, B, f4, K4, expand ( eData, 73) );
  subRound (B, C, D, E, A, f4, K4, expand ( eData, 74) );
  subRound (A, B, C, D, E, f4, K4, expand ( eData, 75) );
  subRound (E, A, B, C, D, f4, K4, expand ( eData, 76) );
  subRound (D, E, A, B, C, f4, K4, expand ( eData, 77) );
  subRound (C, D, E, A, B, f4, K4, expand ( eData, 78) );
  subRound (B, C, D, E, A, f4, K4, expand ( eData, 79) );

  /* Build message digest */
  digest[0] += A;
  digest[1] += B;
  digest[2] += C;
  digest[3] += D;
  digest[4] += E;
}

/* When run on a little-endian CPU we need to perform byte reversal on an
   array of longwords. */

#ifdef WORDS_BIGENDIAN
#define swap_words(buffer, byte_count)
#else
static void
swap_words (dbus_uint32_t *buffer,
            int            byte_count)
{
  byte_count /= sizeof (dbus_uint32_t);
  while (byte_count--)
    {
      *buffer = DBUS_UINT32_SWAP_LE_BE (*buffer);
      ++buffer;
    }
}
#endif

static void
sha_init (DBusSHAContext *context)
{
  /* Set the h-vars to their initial values */
  context->digest[0] = h0init;
  context->digest[1] = h1init;
  context->digest[2] = h2init;
  context->digest[3] = h3init;
  context->digest[4] = h4init;

  /* Initialise bit count */
  context->count_lo = context->count_hi = 0;
}

static void
sha_append (DBusSHAContext      *context,
            const unsigned char *buffer,
            unsigned int         count)
{
  dbus_uint32_t tmp;
  unsigned int dataCount;

  /* Update bitcount */
  tmp = context->count_lo;
  if (( context->count_lo = tmp + ( ( dbus_uint32_t) count << 3) ) < tmp)
    context->count_hi++;             /* Carry from low to high */
  context->count_hi += count >> 29;

  /* Get count of bytes already in data */
  dataCount = (int) (tmp >> 3) & 0x3F;

  /* Handle any leading odd-sized chunks */
  if (dataCount)
    {
      unsigned char *p = (unsigned char *) context->data + dataCount;

      dataCount = SHA_DATASIZE - dataCount;
      if (count < dataCount)
        {
          memmove (p, buffer, count);
          return;
        }
      memmove (p, buffer, dataCount);
      swap_words (context->data, SHA_DATASIZE);
      SHATransform (context->digest, context->data);
      buffer += dataCount;
      count -= dataCount;
    }

  /* Process data in SHA_DATASIZE chunks */
  while (count >= SHA_DATASIZE)
    {
      memmove (context->data, buffer, SHA_DATASIZE);
      swap_words (context->data, SHA_DATASIZE);
      SHATransform (context->digest, context->data);
      buffer += SHA_DATASIZE;
      count -= SHA_DATASIZE;
    }

  /* Handle any remaining bytes of data. */
  memmove (context->data, buffer, count);
}


/* Final wrapup - pad to SHA_DATASIZE-byte boundary with the bit pattern
   1 0* (64-bit count of bits processed, MSB-first) */

static void
sha_finish (DBusSHAContext *context, unsigned char digest[20])
{
  int count;
  unsigned char *data_p;

  /* Compute number of bytes mod 64 */
  count = (int) context->count_lo;
  count = (count >> 3) & 0x3F;

  /* Set the first char of padding to 0x80.  This is safe since there is
     always at least one byte free */
  data_p = (unsigned char *) context->data + count;
  *data_p++ = 0x80;

  /* Bytes of padding needed to make 64 bytes */
  count = SHA_DATASIZE - 1 - count;

  /* Pad out to 56 mod 64 */
  if (count < 8)
    {
      /* Two lots of padding:  Pad the first block to 64 bytes */
      memset (data_p, 0, count);
      swap_words (context->data, SHA_DATASIZE);
      SHATransform (context->digest, context->data);

      /* Now fill the next block with 56 bytes */
      memset (context->data, 0, SHA_DATASIZE - 8);
    }
  else
    /* Pad block to 56 bytes */
    memset (data_p, 0, count - 8);

  /* Append length in bits and transform */
  context->data[14] = context->count_hi;
  context->data[15] = context->count_lo;

  swap_words (context->data, SHA_DATASIZE - 8);
  SHATransform (context->digest, context->data);
  swap_words (context->digest, SHA_DIGESTSIZE);
  memmove (digest, context->digest, SHA_DIGESTSIZE);
}

/** @} */ /* End of internals */

/**
 * @addtogroup DBusSHA
 *
 * @{
 */

/**
 * Initializes the SHA context.
 *
 * @param context an uninitialized context, typically on the stack.
 */
void
_dbus_sha_init (DBusSHAContext *context)
{
  sha_init (context);
}

/**
 * Feeds more data into an existing shasum computation.
 *
 * @param context the SHA context
 * @param data the additional data to hash
 */
void
_dbus_sha_update (DBusSHAContext   *context,
                  const DBusString *data)
{
  unsigned int inputLen;
  const unsigned char *input;

  input = (const unsigned char*) _dbus_string_get_const_data (data);
  inputLen = _dbus_string_get_length (data);

  sha_append (context, input, inputLen);
}

/**
 * SHA finalization. Ends an SHA message-digest operation, writing the
 * the message digest and zeroing the context.  The results are
 * returned as a raw 20-byte digest, not as the ascii-hex-digits
 * string form of the digest.
 *
 * @param context the SHA context
 * @param results string to append the 20-byte SHA digest to
 * @returns #FALSE if not enough memory to append the digest
 *
 */
dbus_bool_t
_dbus_sha_final (DBusSHAContext   *context,
                 DBusString       *results)
{
  unsigned char digest[20];

  sha_finish (context, digest);

  if (!_dbus_string_append_len (results, (const char *) digest, 20))
    return FALSE;

  /* some kind of security paranoia, though it seems pointless
   * to me given the nonzeroed stuff flying around
   */
  _DBUS_ZERO(*context);

  return TRUE;
}

/**
 * Computes the ASCII hex-encoded shasum of the given data and
 * appends it to the output string.
 *
 * @param data input data to be hashed
 * @param ascii_output string to append ASCII shasum to
 * @returns #FALSE if not enough memory
 */
dbus_bool_t
_dbus_sha_compute (const DBusString *data,
                   DBusString       *ascii_output)
{
  DBusSHAContext context;
  DBusString digest;

  _dbus_sha_init (&context);

  _dbus_sha_update (&context, data);

  if (!_dbus_string_init (&digest))
    return FALSE;

  if (!_dbus_sha_final (&context, &digest))
    goto error;

  if (!_dbus_string_hex_encode (&digest, 0, ascii_output,
                                _dbus_string_get_length (ascii_output)))
    goto error;

  _dbus_string_free (&digest);
  
  return TRUE;

 error:
  _dbus_string_free (&digest);
  return FALSE;
}

/** @} */ /* end of exported functions */
