// Copyright 2020 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
// SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
// OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
// CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

#ifndef RING_CHECK_H
#define RING_CHECK_H

// |debug_assert_nonsecret| is like |assert| and should be used (only) when the
// assertion does not have any potential to leak a secret. |NDEBUG| controls this
// exactly like |assert|. It is emulated when there is no assert.h to make
// cross-building easier.
//
// When reviewing uses of |debug_assert_nonsecret|, verify that the check
// really does not have potential to leak a secret.

#if !defined(RING_CORE_NOSTDLIBINC)
# include <assert.h>
# define debug_assert_nonsecret(x) assert(x)
#else
# if !defined(NDEBUG)
#  define debug_assert_nonsecret(x) ((x) ? ((void)0) : __builtin_trap())
# else
#  define debug_assert_nonsecret(x) ((void)0)
# endif
#endif

// |dev_assert_secret| is like |assert| and should be used (only) when the
// assertion operates on secret data in a way that has the potential to leak
// the secret. |dev_assert_secret| can only be enabled by changing the |#if 0|
// here to |#if 1| (or equivalent) when |NDEBUG| is not defined. This is not
// controlled only through |NDEBUG| so that such checks do not leak into debug
// builds that may make it into production use.
//
// When reviewing uses of |dev_assert_secret|, verify that the check really
// does have the potential to leak a secret.
#if 0 // DO NOT COMMIT CHANGES TO THIS LINE.
# define dev_assert_secret debug_assert_nonsecret
#else
# define dev_assert_secret(x) ((void)0)
#endif

#endif // RING_CHECK_H
