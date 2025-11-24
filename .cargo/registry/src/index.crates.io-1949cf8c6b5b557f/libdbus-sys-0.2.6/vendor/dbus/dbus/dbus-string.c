/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-string.c String utility class (internal to D-Bus implementation)
 *
 * Copyright 2002-2007 Red Hat, Inc.
 * Copyright 2003 CodeFactory AB
 * Copyright 2003 Mark McLoughlin
 * Copyright 2004 Michael Meeks
 * Copyright 2006-2014 Ralf Habacker <ralf.habacker@freenet.de>
 * Copyright 2006-2018 Collabora Ltd.
 * Copyright 2007 Allison Lortie
 * Copyright 2011 Roberto Guido
 * Copyright 2013 Chengwei Yang / Intel
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
#include "dbus-string.h"
/* we allow a system header here, for speed/convenience */
#include <string.h>
/* for vsnprintf */
#include <stdio.h>
#define DBUS_CAN_USE_DBUS_STRING_PRIVATE 1
#include "dbus-string-private.h"
#include "dbus-marshal-basic.h" /* probably should be removed by moving the usage of DBUS_TYPE
                                 * into the marshaling-related files
                                 */
/* for DBUS_VA_COPY */
#include "dbus-sysdeps.h"

/**
 * @defgroup DBusString DBusString class
 * @ingroup  DBusInternals
 * @brief DBusString data structure for safer string handling
 *
 * Types and functions related to DBusString. DBusString is intended
 * to be a string class that makes it hard to mess up security issues
 * (and just in general harder to write buggy code).  It should be
 * used (or extended and then used) rather than the libc stuff in
 * string.h.  The string class is a bit inconvenient at spots because
 * it handles out-of-memory failures and tries to be extra-robust.
 * 
 * A DBusString has a maximum length set at initialization time; this
 * can be used to ensure that a buffer doesn't get too big.  The
 * _dbus_string_lengthen() method checks for overflow, and for max
 * length being exceeded.
 * 
 * Try to avoid conversion to a plain C string, i.e. add methods on
 * the string object instead, only convert to C string when passing
 * things out to the public API. In particular, no sprintf, strcpy,
 * strcat, any of that should be used. The GString feature of
 * accepting negative numbers for "length of string" is also absent,
 * because it could keep us from detecting bogus huge lengths. i.e. if
 * we passed in some bogus huge length it would be taken to mean
 * "current length of string" instead of "broken crack"
 *
 * @todo #DBusString needs a lot of cleaning up; some of the
 * API is no longer used, and the API is pretty inconsistent.
 * In particular all the "append" APIs, especially those involving
 * alignment but probably lots of them, are no longer used by the
 * marshaling code which always does "inserts" now.
 */

/**
 * @addtogroup DBusString
 * @{
 */

static void
fixup_alignment (DBusRealString *real)
{
  unsigned char *aligned;
  unsigned char *real_block;
  unsigned int old_align_offset;

  /* we have to have extra space in real->allocated for the align offset and nul byte */
  _dbus_assert (real->len <= real->allocated - _DBUS_STRING_ALLOCATION_PADDING);
  
  old_align_offset = real->align_offset;
  real_block = real->str - old_align_offset;
  
  aligned = _DBUS_ALIGN_ADDRESS (real_block, 8);

  real->align_offset = aligned - real_block;
  real->str = aligned;
  
  if (old_align_offset != real->align_offset)
    {
      /* Here comes the suck */
      memmove (real_block + real->align_offset,
               real_block + old_align_offset,
               real->len + 1);
    }

  _dbus_assert (real->align_offset < 8);
  _dbus_assert (_DBUS_ALIGN_ADDRESS (real->str, 8) == real->str);
}

static void
undo_alignment (DBusRealString *real)
{
  if (real->align_offset != 0)
    {
      memmove (real->str - real->align_offset,
               real->str,
               real->len + 1);

      real->str = real->str - real->align_offset;
      real->align_offset = 0;
    }
}

/**
 * Initializes a string that can be up to the given allocation size
 * before it has to realloc. The string starts life with zero length.
 * The string must eventually be freed with _dbus_string_free().
 * 
 * @param str memory to hold the string
 * @param allocate_size amount to preallocate
 * @returns #TRUE on success, #FALSE if no memory
 */
dbus_bool_t
_dbus_string_init_preallocated (DBusString *str,
                                int         allocate_size)
{
  DBusRealString *real;

  _DBUS_STATIC_ASSERT (sizeof (DBusString) == sizeof (DBusRealString));

  _dbus_assert (str != NULL);

  real = (DBusRealString*) str;

  /* It's very important not to touch anything
   * other than real->str if we're going to fail,
   * since we also use this function to reset
   * an existing string, e.g. in _dbus_string_steal_data()
   */
  
  real->str = dbus_malloc (_DBUS_STRING_ALLOCATION_PADDING + allocate_size);
  if (real->str == NULL)
    return FALSE;  
  
  real->allocated = _DBUS_STRING_ALLOCATION_PADDING + allocate_size;
  real->len = 0;
  real->str[real->len] = '\0';
  
  real->constant = FALSE;
  real->locked = FALSE;
  real->valid = TRUE;
  real->align_offset = 0;
  
  fixup_alignment (real);
  
  return TRUE;
}

/**
 * Initializes a string. The string starts life with zero length.  The
 * string must eventually be freed with _dbus_string_free().
 * 
 * @param str memory to hold the string
 * @returns #TRUE on success, #FALSE if no memory
 */
dbus_bool_t
_dbus_string_init (DBusString *str)
{
  return _dbus_string_init_preallocated (str, 0);
}

/**
 * Initializes a constant string. The value parameter is not copied
 * (should be static), and the string may never be modified.
 * It is safe but not necessary to call _dbus_string_free()
 * on a const string. The string has a length limit of MAXINT - 8.
 * 
 * @param str memory to use for the string
 * @param value a string to be stored in str (not copied!!!)
 */
void
_dbus_string_init_const (DBusString *str,
                         const char *value)
{
  _dbus_assert (value != NULL);
  
  _dbus_string_init_const_len (str, value,
                               strlen (value));
}

/**
 * Initializes a constant string with a length. The value parameter is
 * not copied (should be static), and the string may never be
 * modified.  It is safe but not necessary to call _dbus_string_free()
 * on a const string.
 * 
 * @param str memory to use for the string
 * @param value a string to be stored in str (not copied!!!)
 * @param len the length to use
 */
void
_dbus_string_init_const_len (DBusString *str,
                             const char *value,
                             int         len)
{
  DBusRealString *real;
  
  _dbus_assert (str != NULL);
  _dbus_assert (len == 0 || value != NULL);
  _dbus_assert (len <= _DBUS_STRING_MAX_LENGTH);
  _dbus_assert (len >= 0);
  
  real = (DBusRealString*) str;
  
  real->str = (unsigned char*) value;
  real->len = len;
  real->allocated = real->len + _DBUS_STRING_ALLOCATION_PADDING; /* a lie, just to avoid special-case assertions... */
  real->constant = TRUE;
  real->locked = TRUE;
  real->valid = TRUE;
  real->align_offset = 0;

  /* We don't require const strings to be 8-byte aligned as the
   * memory is coming from elsewhere.
   */
}

/**
 * Initializes a string from another string
 *
 * The string must be freed with _dbus_string_free() in case of success.
 * In case of error the string is freed by this function itself.
 *
 * @param str memory to hold the string
 * @param from instance from which the string is initialized
 * @returns #TRUE on success, #FALSE if no memory
 */
dbus_bool_t
_dbus_string_init_from_string(DBusString       *str,
                              const DBusString *from)
{
  if (!_dbus_string_init (str))
    return FALSE;
  if (!_dbus_string_append (str, _dbus_string_get_const_data (from)))
    {
      _dbus_string_free (str);
      return FALSE;
    }
  return TRUE;
}

/**
 * Frees a string created by _dbus_string_init(), and fills it with the
 * same contents as #_DBUS_STRING_INIT_INVALID.
 *
 * Unlike all other #DBusString API, it is also valid to call this function
 * for a string that was filled with #_DBUS_STRING_INIT_INVALID.
 * This is convenient for error rollback.
 *
 * @param str memory where the string is stored.
 */
void
_dbus_string_free (DBusString *str)
{
  DBusRealString *real = (DBusRealString*) str;
  /* DBusRealString and DBusString have the same members in the same order,
   * just differently-named */
  DBusRealString invalid = _DBUS_STRING_INIT_INVALID;

  /* Allow for the _DBUS_STRING_INIT_INVALID case */
  if (real->str == NULL && real->len == 0 && real->allocated == 0 &&
      !real->constant && !real->locked && !real->valid &&
      real->align_offset == 0)
    return;

  DBUS_GENERIC_STRING_PREAMBLE (real);
  
  if (real->constant)
    goto wipe;

  /* so it's safe if @p str returned by a failed
   * _dbus_string_init call
   * Bug: https://bugs.freedesktop.org/show_bug.cgi?id=65959
   */
  if (real->str == NULL)
    goto wipe;

  dbus_free (real->str - real->align_offset);

wipe:
  *real = invalid;
  real->valid = FALSE;
}

static dbus_bool_t
compact (DBusRealString *real,
         int             max_waste)
{
  unsigned char *new_str;
  int new_allocated;
  int waste;

  waste = real->allocated - (real->len + _DBUS_STRING_ALLOCATION_PADDING);

  if (waste <= max_waste)
    return TRUE;

  new_allocated = real->len + _DBUS_STRING_ALLOCATION_PADDING;

  new_str = dbus_realloc (real->str - real->align_offset, new_allocated);
  if (_DBUS_UNLIKELY (new_str == NULL))
    return FALSE;

  real->str = new_str + real->align_offset;
  real->allocated = new_allocated;
  fixup_alignment (real);

  return TRUE;
}

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
/* Not using this feature at the moment,
 * so marked DBUS_ENABLE_EMBEDDED_TESTS-only
 */
/**
 * Locks a string such that any attempts to change the string will
 * result in aborting the program. Also, if the string is wasting a
 * lot of memory (allocation is sufficiently larger than what the
 * string is really using), _dbus_string_lock() will realloc the
 * string's data to "compact" it.
 *
 * @param str the string to lock.
 */
void
_dbus_string_lock (DBusString *str)
{  
  DBUS_LOCKED_STRING_PREAMBLE (str); /* can lock multiple times */

  real->locked = TRUE;

  /* Try to realloc to avoid excess memory usage, since
   * we know we won't change the string further
   */
#define MAX_WASTE 48
  compact (real, MAX_WASTE);
}
#endif /* DBUS_ENABLE_EMBEDDED_TESTS */

static dbus_bool_t
reallocate_for_length (DBusRealString *real,
                       int             new_length)
{
  int new_allocated;
  unsigned char *new_str;

  /* at least double our old allocation to avoid O(n), avoiding
   * overflow
   */
  if (real->allocated > (_DBUS_STRING_MAX_LENGTH + _DBUS_STRING_ALLOCATION_PADDING) / 2)
    new_allocated = _DBUS_STRING_MAX_LENGTH + _DBUS_STRING_ALLOCATION_PADDING;
  else
    new_allocated = real->allocated * 2;

  /* if you change the code just above here, run the tests without
   * the following assert-only hack before you commit
   */
  /* This is keyed off asserts in addition to tests so when you
   * disable asserts to profile, you don't get this destroyer
   * of profiles.
   */
#if defined (DBUS_ENABLE_EMBEDDED_TESTS) && !defined (DBUS_DISABLE_ASSERT)
  new_allocated = 0; /* ensure a realloc every time so that we go
                      * through all malloc failure codepaths
                      */
#endif

  /* But be sure we always alloc at least space for the new length */
  new_allocated = MAX (new_allocated,
                       new_length + _DBUS_STRING_ALLOCATION_PADDING);

  _dbus_assert (new_allocated >= real->allocated); /* code relies on this */
  new_str = dbus_realloc (real->str - real->align_offset, new_allocated);
  if (_DBUS_UNLIKELY (new_str == NULL))
    return FALSE;

  real->str = new_str + real->align_offset;
  real->allocated = new_allocated;
  fixup_alignment (real);

  return TRUE;
}

/**
 * Compacts the string to avoid wasted memory.  Wasted memory is
 * memory that is allocated but not actually required to store the
 * current length of the string.  The compact is only done if more
 * than the given amount of memory is being wasted (otherwise the
 * waste is ignored and the call does nothing).
 *
 * @param str the string
 * @param max_waste the maximum amount of waste to ignore
 * @returns #FALSE if the compact failed due to realloc failure
 */
dbus_bool_t
_dbus_string_compact (DBusString *str,
                      int         max_waste)
{
  DBUS_STRING_PREAMBLE (str);

  return compact (real, max_waste);
}

static dbus_bool_t
set_length (DBusRealString *real,
            int             new_length)
{
  /* Note, we are setting the length not including nul termination */

  /* exceeding max length is the same as failure to allocate memory */
  if (_DBUS_UNLIKELY (new_length > _DBUS_STRING_MAX_LENGTH))
    return FALSE;
  else if (new_length > (real->allocated - _DBUS_STRING_ALLOCATION_PADDING) &&
           _DBUS_UNLIKELY (!reallocate_for_length (real, new_length)))
    return FALSE;
  else
    {
      real->len = new_length;
      real->str[new_length] = '\0';
      return TRUE;
    }
}

static dbus_bool_t
open_gap (int             len,
          DBusRealString *dest,
          int             insert_at)
{
  if (len == 0)
    return TRUE;

  if (len > _DBUS_STRING_MAX_LENGTH - dest->len)
    return FALSE; /* detected overflow of dest->len + len below */
  
  if (!set_length (dest, dest->len + len))
    return FALSE;

  memmove (dest->str + insert_at + len, 
           dest->str + insert_at,
           dest->len - len - insert_at);

  return TRUE;
}

#ifndef _dbus_string_get_data
/**
 * Gets the raw character buffer from the string.  The returned buffer
 * will be nul-terminated, but note that strings may contain binary
 * data so there may be extra nul characters prior to the termination.
 * This function should be little-used, extend DBusString or add
 * stuff to dbus-sysdeps.c instead. It's an error to use this
 * function on a const string.
 *
 * @param str the string
 * @returns the data
 */
char*
_dbus_string_get_data (DBusString *str)
{
  DBUS_STRING_PREAMBLE (str);
  
  return (char*) real->str;
}
#endif /* _dbus_string_get_data */

/* only do the function if we don't have the macro */
#ifndef _dbus_string_get_const_data
/**
 * Gets the raw character buffer from a const string.
 *
 * @param str the string
 * @returns the string data
 */
const char*
_dbus_string_get_const_data (const DBusString  *str)
{
  DBUS_CONST_STRING_PREAMBLE (str);
  
  return (const char*) real->str;
}
#endif /* _dbus_string_get_const_data */

/**
 * Gets a sub-portion of the raw character buffer from the
 * string. The "len" field is required simply for error
 * checking, to be sure you don't try to use more
 * string than exists. The nul termination of the
 * returned buffer remains at the end of the entire
 * string, not at start + len.
 *
 * @param str the string
 * @param start byte offset to return
 * @param len length of segment to return
 * @returns the string data
 */
char*
_dbus_string_get_data_len (DBusString *str,
                           int         start,
                           int         len)
{
  DBUS_STRING_PREAMBLE (str);
  _dbus_assert (start >= 0);
  _dbus_assert (len >= 0);
  _dbus_assert (start <= real->len);
  _dbus_assert (len <= real->len - start);
  
  return (char*) real->str + start;
}

/* only do the function if we don't have the macro */
#ifndef _dbus_string_get_const_data_len
/**
 * const version of _dbus_string_get_data_len().
 *
 * @param str the string
 * @param start byte offset to return
 * @param len length of segment to return
 * @returns the string data
 */
const char*
_dbus_string_get_const_data_len (const DBusString  *str,
                                 int                start,
                                 int                len)
{
  DBUS_CONST_STRING_PREAMBLE (str);
  _dbus_assert (start >= 0);
  _dbus_assert (len >= 0);
  _dbus_assert (start <= real->len);
  _dbus_assert (len <= real->len - start);
  
  return (const char*) real->str + start;
}
#endif /* _dbus_string_get_const_data_len */

/* only do the function if we don't have the macro */
#ifndef _dbus_string_set_byte
/**
 * Sets the value of the byte at the given position.
 *
 * @param str the string
 * @param i the position
 * @param byte the new value
 */
void
_dbus_string_set_byte (DBusString    *str,
                       int            i,
                       unsigned char  byte)
{
  DBUS_STRING_PREAMBLE (str);
  _dbus_assert (i < real->len);
  _dbus_assert (i >= 0);
  
  real->str[i] = byte;
}
#endif /* _dbus_string_set_byte */

/* only have the function if we didn't create a macro */
#ifndef _dbus_string_get_byte
/**
 * Gets the byte at the given position. It is
 * allowed to ask for the nul byte at the end of
 * the string.
 *
 * @param str the string
 * @param start the position
 * @returns the byte at that position
 */
unsigned char
_dbus_string_get_byte (const DBusString  *str,
                       int                start)
{
  DBUS_CONST_STRING_PREAMBLE (str);
  _dbus_assert (start <= real->len);
  _dbus_assert (start >= 0);
  
  return real->str[start];
}
#endif /* _dbus_string_get_byte */

/**
 * Inserts a number of bytes of a given value at the
 * given position.
 *
 * @param str the string
 * @param i the position
 * @param n_bytes number of bytes
 * @param byte the value to insert
 * @returns #TRUE on success
 */
dbus_bool_t
_dbus_string_insert_bytes (DBusString   *str,
			   int           i,
			   int           n_bytes,
			   unsigned char byte)
{
  DBUS_STRING_PREAMBLE (str);
  _dbus_assert (i <= real->len);
  _dbus_assert (i >= 0);
  _dbus_assert (n_bytes >= 0);

  if (n_bytes == 0)
    return TRUE;
  
  if (!open_gap (n_bytes, real, i))
    return FALSE;
  
  memset (real->str + i, byte, n_bytes);

  return TRUE;
}

/**
 * Inserts a single byte at the given position.
 *
 * @param str the string
 * @param i the position
 * @param byte the value to insert
 * @returns #TRUE on success
 */
dbus_bool_t
_dbus_string_insert_byte (DBusString   *str,
			   int           i,
			   unsigned char byte)
{
  DBUS_STRING_PREAMBLE (str);
  _dbus_assert (i <= real->len);
  _dbus_assert (i >= 0);
  
  if (!open_gap (1, real, i))
    return FALSE;

  real->str[i] = byte;

  return TRUE;
}

/**
 * Like _dbus_string_get_data(), but removes the
 * gotten data from the original string. The caller
 * must free the data returned. This function may
 * fail due to lack of memory, and return #FALSE.
 *
 * @param str the string
 * @param data_return location to return the buffer
 * @returns #TRUE on success
 */
dbus_bool_t
_dbus_string_steal_data (DBusString        *str,
                         char             **data_return)
{
  DBUS_STRING_PREAMBLE (str);
  _dbus_assert (data_return != NULL);

  undo_alignment (real);
  
  *data_return = (char*) real->str;

  /* reset the string */
  if (!_dbus_string_init (str))
    {
      /* hrm, put it back then */
      real->str = (unsigned char*) *data_return;
      *data_return = NULL;
      fixup_alignment (real);
      return FALSE;
    }

  return TRUE;
}

/**
 * Copies the data from the string into a char*
 *
 * @param str the string
 * @param data_return place to return the data
 * @returns #TRUE on success, #FALSE on no memory
 */
dbus_bool_t
_dbus_string_copy_data (const DBusString  *str,
                        char             **data_return)
{
  DBUS_CONST_STRING_PREAMBLE (str);
  _dbus_assert (data_return != NULL);
  
  *data_return = dbus_malloc (real->len + 1);
  if (*data_return == NULL)
    return FALSE;

  memcpy (*data_return, real->str, real->len + 1);

  return TRUE;
}

/**
 * Copies the contents of a DBusString into a different buffer. It is
 * a bug if avail_len is too short to hold the string contents. nul
 * termination is not copied, just the supplied bytes.
 * 
 * @param str a string
 * @param buffer a C buffer to copy data to
 * @param avail_len maximum length of C buffer
 */
void
_dbus_string_copy_to_buffer (const DBusString  *str,
			     char              *buffer,
			     int                avail_len)
{
  DBUS_CONST_STRING_PREAMBLE (str);

  _dbus_assert (avail_len >= 0);
  _dbus_assert (avail_len >= real->len);
  
  memcpy (buffer, real->str, real->len);
}

/**
 * Copies the contents of a DBusString into a different buffer. It is
 * a bug if avail_len is too short to hold the string contents plus a
 * nul byte. 
 * 
 * @param str a string
 * @param buffer a C buffer to copy data to
 * @param avail_len maximum length of C buffer
 */
void
_dbus_string_copy_to_buffer_with_nul (const DBusString  *str,
                                      char              *buffer,
                                      int                avail_len)
{
  DBUS_CONST_STRING_PREAMBLE (str);

  _dbus_assert (avail_len >= 0);
  _dbus_assert (avail_len > real->len);
  
  memcpy (buffer, real->str, real->len+1);
}

/* Only have the function if we don't have the macro */
#ifndef _dbus_string_get_length
/**
 * Gets the length of a string (not including nul termination).
 *
 * @returns the length.
 */
int
_dbus_string_get_length (const DBusString  *str)
{
  DBUS_CONST_STRING_PREAMBLE (str);
  
  return real->len;
}
#endif /* !_dbus_string_get_length */

/**
 * Makes a string longer by the given number of bytes.  Checks whether
 * adding additional_length to the current length would overflow an
 * integer, and checks for exceeding a string's max length.
 * The new bytes are not initialized, other than nul-terminating
 * the end of the string. The uninitialized bytes may contain
 * nul bytes or other junk.
 *
 * @param str a string
 * @param additional_length length to add to the string.
 * @returns #TRUE on success.
 */
dbus_bool_t
_dbus_string_lengthen (DBusString *str,
                       int         additional_length)
{
  DBUS_STRING_PREAMBLE (str);  
  _dbus_assert (additional_length >= 0);

  if (_DBUS_UNLIKELY (additional_length > _DBUS_STRING_MAX_LENGTH - real->len))
    return FALSE; /* would overflow */
  
  return set_length (real,
                     real->len + additional_length);
}

/**
 * Makes a string shorter by the given number of bytes.
 *
 * @param str a string
 * @param length_to_remove length to remove from the string.
 */
void
_dbus_string_shorten (DBusString *str,
                      int         length_to_remove)
{
  DBUS_STRING_PREAMBLE (str);
  _dbus_assert (length_to_remove >= 0);
  _dbus_assert (length_to_remove <= real->len);

  set_length (real,
              real->len - length_to_remove);
}

/**
 * Sets the length of a string. Can be used to truncate or lengthen
 * the string. If the string is lengthened, the function may fail and
 * return #FALSE. Newly-added bytes are not initialized, as with
 * _dbus_string_lengthen().
 *
 * @param str a string
 * @param length new length of the string.
 * @returns #FALSE on failure.
 */
dbus_bool_t
_dbus_string_set_length (DBusString *str,
                         int         length)
{
  DBUS_STRING_PREAMBLE (str);
  _dbus_assert (length >= 0);

  return set_length (real, length);
}

static dbus_bool_t
align_insert_point_then_open_gap (DBusString *str,
                                  int        *insert_at_p,
                                  int         alignment,
                                  int         gap_size)
{
  unsigned long new_len; /* ulong to avoid _DBUS_ALIGN_VALUE overflow */
  unsigned long gap_pos;
  int insert_at;
  int delta;
  DBUS_STRING_PREAMBLE (str);
  _dbus_assert (alignment >= 1);
  _dbus_assert (alignment <= 8); /* it has to be a bug if > 8 */

  insert_at = *insert_at_p;

  _dbus_assert (insert_at <= real->len);
  
  gap_pos = _DBUS_ALIGN_VALUE (insert_at, alignment);
  new_len = real->len + (gap_pos - insert_at) + gap_size;
  
  if (_DBUS_UNLIKELY (new_len > (unsigned long) _DBUS_STRING_MAX_LENGTH))
    return FALSE;
  
  delta = new_len - real->len;
  _dbus_assert (delta >= 0);

  if (delta == 0) /* only happens if gap_size == 0 and insert_at is aligned already */
    {
      _dbus_assert (((unsigned long) *insert_at_p) == gap_pos);
      return TRUE;
    }

  if (_DBUS_UNLIKELY (!open_gap (new_len - real->len,
                                 real, insert_at)))
    return FALSE;

  /* nul the padding if we had to add any padding */
  if (gap_size < delta)
    {
      memset (&real->str[insert_at], '\0',
              gap_pos - insert_at);
    }

  *insert_at_p = gap_pos;
  
  return TRUE;
}

static dbus_bool_t
align_length_then_lengthen (DBusString *str,
                            int         alignment,
                            int         then_lengthen_by)
{
  int insert_at;

  insert_at = _dbus_string_get_length (str);
  
  return align_insert_point_then_open_gap (str,
                                           &insert_at,
                                           alignment, then_lengthen_by);
}

/**
 * Align the length of a string to a specific alignment (typically 4 or 8)
 * by appending nul bytes to the string.
 *
 * @param str a string
 * @param alignment the alignment
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_string_align_length (DBusString *str,
                           int         alignment)
{
  return align_length_then_lengthen (str, alignment, 0);
}

/**
 * Preallocate extra_bytes such that a future lengthening of the
 * string by extra_bytes is guaranteed to succeed without an out of
 * memory error.
 *
 * @param str a string
 * @param extra_bytes bytes to alloc
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_string_alloc_space (DBusString        *str,
                          int                extra_bytes)
{
  if (!_dbus_string_lengthen (str, extra_bytes))
    return FALSE;
  _dbus_string_shorten (str, extra_bytes);

  return TRUE;
}

static dbus_bool_t
append (DBusRealString *real,
        const char     *buffer,
        int             buffer_len)
{
  if (buffer_len == 0)
    return TRUE;

  if (!_dbus_string_lengthen ((DBusString*)real, buffer_len))
    return FALSE;

  memcpy (real->str + (real->len - buffer_len),
          buffer,
          buffer_len);

  return TRUE;
}

/**
 * Appends a nul-terminated C-style string to a DBusString.
 *
 * @param str the DBusString
 * @param buffer the nul-terminated characters to append
 * @returns #FALSE if not enough memory.
 */
dbus_bool_t
_dbus_string_append (DBusString *str,
                     const char *buffer)
{
  unsigned long buffer_len;
  
  DBUS_STRING_PREAMBLE (str);
  _dbus_assert (buffer != NULL);
  
  buffer_len = strlen (buffer);
  if (buffer_len > (unsigned long) _DBUS_STRING_MAX_LENGTH)
    return FALSE;
  
  return append (real, buffer, buffer_len);
}

/** assign 2 bytes from one string to another */
#define ASSIGN_2_OCTETS(p, octets) \
  *((dbus_uint16_t*)(p)) = *((dbus_uint16_t*)(octets));

/** assign 4 bytes from one string to another */
#define ASSIGN_4_OCTETS(p, octets) \
  *((dbus_uint32_t*)(p)) = *((dbus_uint32_t*)(octets));

/** assign 8 bytes from one string to another */
#define ASSIGN_8_OCTETS(p, octets) \
  *((dbus_uint64_t*)(p)) = *((dbus_uint64_t*)(octets));

/**
 * Inserts 2 bytes aligned on a 2 byte boundary
 * with any alignment padding initialized to 0.
 *
 * @param str the DBusString
 * @param insert_at where to insert
 * @param octets 2 bytes to insert
 * @returns #FALSE if not enough memory.
 */
dbus_bool_t
_dbus_string_insert_2_aligned (DBusString         *str,
                               int                 insert_at,
                               const unsigned char octets[2])
{
  DBUS_STRING_PREAMBLE (str);
  
  if (!align_insert_point_then_open_gap (str, &insert_at, 2, 2))
    return FALSE;

  ASSIGN_2_OCTETS (real->str + insert_at, octets);

  return TRUE;
}

/**
 * Inserts 4 bytes aligned on a 4 byte boundary
 * with any alignment padding initialized to 0.
 *
 * @param str the DBusString
 * @param insert_at where to insert
 * @param octets 4 bytes to insert
 * @returns #FALSE if not enough memory.
 */
dbus_bool_t
_dbus_string_insert_4_aligned (DBusString         *str,
                               int                 insert_at,
                               const unsigned char octets[4])
{
  DBUS_STRING_PREAMBLE (str);
  
  if (!align_insert_point_then_open_gap (str, &insert_at, 4, 4))
    return FALSE;

  ASSIGN_4_OCTETS (real->str + insert_at, octets);

  return TRUE;
}

/**
 * Inserts 8 bytes aligned on an 8 byte boundary
 * with any alignment padding initialized to 0.
 *
 * @param str the DBusString
 * @param insert_at where to insert
 * @param octets 8 bytes to insert
 * @returns #FALSE if not enough memory.
 */
dbus_bool_t
_dbus_string_insert_8_aligned (DBusString         *str,
                               int                 insert_at,
                               const unsigned char octets[8])
{
  DBUS_STRING_PREAMBLE (str);
  
  if (!align_insert_point_then_open_gap (str, &insert_at, 8, 8))
    return FALSE;

  _dbus_assert (_DBUS_ALIGN_VALUE (insert_at, 8) == (unsigned) insert_at);
  
  ASSIGN_8_OCTETS (real->str + insert_at, octets);

  return TRUE;
}


/**
 * Inserts padding at *insert_at such to align it to the given
 * boundary. Initializes the padding to nul bytes. Sets *insert_at
 * to the aligned position.
 *
 * @param str the DBusString
 * @param insert_at location to be aligned
 * @param alignment alignment boundary (1, 2, 4, or 8)
 * @returns #FALSE if not enough memory.
 */
dbus_bool_t
_dbus_string_insert_alignment (DBusString        *str,
                               int               *insert_at,
                               int                alignment)
{
  DBUS_STRING_PREAMBLE (str);
  
  if (!align_insert_point_then_open_gap (str, insert_at, alignment, 0))
    return FALSE;

  _dbus_assert (_DBUS_ALIGN_VALUE (*insert_at, alignment) == (unsigned) *insert_at);

  return TRUE;
}

/**
 * Appends a printf-style formatted string
 * to the #DBusString.
 *
 * @param str the string
 * @param format printf format
 * @param args variable argument list
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_string_append_printf_valist  (DBusString        *str,
                                    const char        *format,
                                    va_list            args)
{
  dbus_bool_t ret = FALSE;
  int len;
  va_list args_copy;

  DBUS_STRING_PREAMBLE (str);

  DBUS_VA_COPY (args_copy, args);

  /* Measure the message length without terminating nul */
  len = _dbus_printf_string_upper_bound (format, args);

  if (len < 0)
    goto out;

  if (!_dbus_string_lengthen (str, len))
    {
      goto out;
    }
  
  vsprintf ((char*) (real->str + (real->len - len)),
            format, args_copy);
  ret = TRUE;

out:
  va_end (args_copy);

  return ret;
}

/**
 * Appends a printf-style formatted string
 * to the #DBusString.
 *
 * @param str the string
 * @param format printf format
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_string_append_printf (DBusString        *str,
                            const char        *format,
                            ...)
{
  va_list args;
  dbus_bool_t retval;
  
  va_start (args, format);
  retval = _dbus_string_append_printf_valist (str, format, args);
  va_end (args);

  return retval;
}

/**
 * Appends block of bytes with the given length to a DBusString.
 *
 * @param str the DBusString
 * @param buffer the bytes to append
 * @param len the number of bytes to append
 * @returns #FALSE if not enough memory.
 */
dbus_bool_t
_dbus_string_append_len (DBusString *str,
                         const char *buffer,
                         int         len)
{
  DBUS_STRING_PREAMBLE (str);
  _dbus_assert (buffer != NULL);
  _dbus_assert (len >= 0);

  return append (real, buffer, len);
}

/**
 * Appends a single byte to the string, returning #FALSE
 * if not enough memory.
 *
 * @param str the string
 * @param byte the byte to append
 * @returns #TRUE on success
 */
dbus_bool_t
_dbus_string_append_byte (DBusString    *str,
                          unsigned char  byte)
{
  DBUS_STRING_PREAMBLE (str);

  if (!set_length (real, real->len + 1))
    return FALSE;

  real->str[real->len-1] = byte;

  return TRUE;
}

/**
 * Append vector with \p strings connected by \p separator
 *
 * @param str the string
 * @param strings vector with char* pointer for merging
 * @param separator separator to merge the vector
 * @return #FALSE if not enough memory
 * @return #TRUE success or empty string vector
 */
dbus_bool_t
_dbus_string_append_strings (DBusString *str, char **strings, char separator)
{
  int i;

  if (strings == NULL)
    return TRUE;

  for (i = 0; strings[i]; i++)
    {
      if (i > 0 && !_dbus_string_append_byte (str, (unsigned char) separator))
        return FALSE;

      if (!_dbus_string_append (str, strings[i]))
        return FALSE;
    }

  return TRUE;
}

static void
delete (DBusRealString *real,
        int             start,
        int             len)
{
  if (len == 0)
    return;
  
  memmove (real->str + start, real->str + start + len, real->len - (start + len));
  real->len -= len;
  real->str[real->len] = '\0';
}

/**
 * Deletes a segment of a DBusString with length len starting at
 * start. (Hint: to clear an entire string, setting length to 0
 * with _dbus_string_set_length() is easier.)
 *
 * @param str the DBusString
 * @param start where to start deleting
 * @param len the number of bytes to delete
 */
void
_dbus_string_delete (DBusString       *str,
                     int               start,
                     int               len)
{
  DBUS_STRING_PREAMBLE (str);
  _dbus_assert (start >= 0);
  _dbus_assert (len >= 0);
  _dbus_assert (start <= real->len);
  _dbus_assert (len <= real->len - start);
  
  delete (real, start, len);
}

static dbus_bool_t
copy (DBusRealString *source,
      int             start,
      int             len,
      DBusRealString *dest,
      int             insert_at)
{
  if (len == 0)
    return TRUE;

  if (!open_gap (len, dest, insert_at))
    return FALSE;
  
  memmove (dest->str + insert_at,
           source->str + start,
           len);

  return TRUE;
}

/**
 * Checks assertions for two strings we're copying a segment between,
 * and declares real_source/real_dest variables.
 *
 * @param source the source string
 * @param start the starting offset
 * @param dest the dest string
 * @param insert_at where the copied segment is inserted
 */
#define DBUS_STRING_COPY_PREAMBLE(source, start, dest, insert_at)       \
  DBusRealString *real_source = (DBusRealString*) source;               \
  DBusRealString *real_dest = (DBusRealString*) dest;                   \
  _dbus_assert ((source) != (dest));                                    \
  DBUS_GENERIC_STRING_PREAMBLE (real_source);                           \
  DBUS_GENERIC_STRING_PREAMBLE (real_dest);                             \
  _dbus_assert (!real_dest->constant);                                  \
  _dbus_assert (!real_dest->locked);                                    \
  _dbus_assert ((start) >= 0);                                          \
  _dbus_assert ((start) <= real_source->len);                           \
  _dbus_assert ((insert_at) >= 0);                                      \
  _dbus_assert ((insert_at) <= real_dest->len)

/**
 * Moves the end of one string into another string. Both strings
 * must be initialized, valid strings.
 *
 * @param source the source string
 * @param start where to chop off the source string
 * @param dest the destination string
 * @param insert_at where to move the chopped-off part of source string
 * @returns #FALSE if not enough memory
 */
dbus_bool_t
_dbus_string_move (DBusString       *source,
                   int               start,
                   DBusString       *dest,
                   int               insert_at)
{
  DBusRealString *real_source = (DBusRealString*) source;
  _dbus_assert (start <= real_source->len);
  
  return _dbus_string_move_len (source, start,
                                real_source->len - start,
                                dest, insert_at);
}

/**
 * Like _dbus_string_move(), but does not delete the section
 * of the source string that's copied to the dest string.
 *
 * @param source the source string
 * @param start where to start copying the source string
 * @param dest the destination string
 * @param insert_at where to place the copied part of source string
 * @returns #FALSE if not enough memory
 */
dbus_bool_t
_dbus_string_copy (const DBusString *source,
                   int               start,
                   DBusString       *dest,
                   int               insert_at)
{
  DBUS_STRING_COPY_PREAMBLE (source, start, dest, insert_at);

  return copy (real_source, start,
               real_source->len - start,
               real_dest,
               insert_at);
}

/**
 * Like _dbus_string_move(), but can move a segment from
 * the middle of the source string.
 *
 * @param source the source string
 * @param start first byte of source string to move
 * @param len length of segment to move
 * @param dest the destination string
 * @param insert_at where to move the bytes from the source string
 * @returns #FALSE if not enough memory
 */
dbus_bool_t
_dbus_string_move_len (DBusString       *source,
                       int               start,
                       int               len,
                       DBusString       *dest,
                       int               insert_at)

{
  DBUS_STRING_COPY_PREAMBLE (source, start, dest, insert_at);
  _dbus_assert (len >= 0);
  _dbus_assert ((start + len) <= real_source->len);


  if (len == 0)
    {
      return TRUE;
    }
  else if (start == 0 &&
           len == real_source->len &&
           real_dest->len == 0)
    {
      /* Short-circuit moving an entire existing string to an empty string
       * by just swapping the buffers.
       */
      /* we assume ->constant doesn't matter as you can't have
       * a constant string involved in a move.
       */
#define ASSIGN_DATA(a, b) do {                  \
        (a)->str = (b)->str;                    \
        (a)->len = (b)->len;                    \
        (a)->allocated = (b)->allocated;        \
        (a)->align_offset = (b)->align_offset;  \
      } while (0)
      
      DBusRealString tmp;

      ASSIGN_DATA (&tmp, real_source);
      ASSIGN_DATA (real_source, real_dest);
      ASSIGN_DATA (real_dest, &tmp);

      return TRUE;
    }
  else
    {
      if (!copy (real_source, start, len,
                 real_dest,
                 insert_at))
        return FALSE;
      
      delete (real_source, start,
              len);
      
      return TRUE;
    }
}

/**
 * Like _dbus_string_copy(), but can copy a segment from the middle of
 * the source string.
 *
 * @param source the source string
 * @param start where to start copying the source string
 * @param len length of segment to copy
 * @param dest the destination string
 * @param insert_at where to place the copied segment of source string
 * @returns #FALSE if not enough memory
 */
dbus_bool_t
_dbus_string_copy_len (const DBusString *source,
                       int               start,
                       int               len,
                       DBusString       *dest,
                       int               insert_at)
{
  DBUS_STRING_COPY_PREAMBLE (source, start, dest, insert_at);
  _dbus_assert (len >= 0);
  _dbus_assert (start <= real_source->len);
  _dbus_assert (len <= real_source->len - start);
  
  return copy (real_source, start, len,
               real_dest,
               insert_at);
}

/**
 * Replaces a segment of dest string with a segment of source string.
 *
 * @param source the source string
 * @param start where to start copying the source string
 * @param len length of segment to copy
 * @param dest the destination string
 * @param replace_at start of segment of dest string to replace
 * @param replace_len length of segment of dest string to replace
 * @returns #FALSE if not enough memory
 *
 */
dbus_bool_t
_dbus_string_replace_len (const DBusString *source,
                          int               start,
                          int               len,
                          DBusString       *dest,
                          int               replace_at,
                          int               replace_len)
{
  DBUS_STRING_COPY_PREAMBLE (source, start, dest, replace_at);
  _dbus_assert (len >= 0);
  _dbus_assert (start <= real_source->len);
  _dbus_assert (len <= real_source->len - start);
  _dbus_assert (replace_at >= 0);
  _dbus_assert (replace_at <= real_dest->len);
  _dbus_assert (replace_len <= real_dest->len - replace_at);

  if (len == replace_len)
    {
      memmove (real_dest->str + replace_at,
               real_source->str + start, len);
    }
  else if (len < replace_len)
    {
      memmove (real_dest->str + replace_at,
               real_source->str + start, len);
      delete (real_dest, replace_at + len,
              replace_len - len);
    }
  else
    {
      int diff;

      _dbus_assert (len > replace_len);

      diff = len - replace_len;

      /* First of all we check if destination string can be enlarged as
       * required, then we overwrite previous bytes
       */

      if (!copy (real_source, start + replace_len, diff,
                 real_dest, replace_at + replace_len))
        return FALSE;

      memmove (real_dest->str + replace_at,
               real_source->str + start, replace_len);
    }

  return TRUE;
}

/**
 * Looks for the first occurance of a byte, deletes that byte,
 * and moves everything after the byte to the beginning of a
 * separate string.  Both strings must be initialized, valid
 * strings.
 *
 * @param source the source string
 * @param byte the byte to remove and split the string at
 * @param tail the split off string
 * @returns #FALSE if not enough memory or if byte could not be found
 *
 */
dbus_bool_t
_dbus_string_split_on_byte (DBusString        *source,
                            unsigned char      byte,
                            DBusString        *tail)
{
  int byte_position;
  char byte_string[2] = "";
  int head_length;
  int tail_length;

  byte_string[0] = (char) byte;

  if (!_dbus_string_find (source, 0, byte_string, &byte_position))
    return FALSE;

  head_length = byte_position;
  tail_length = _dbus_string_get_length (source) - head_length - 1;

  if (!_dbus_string_move_len (source, byte_position + 1, tail_length,
                              tail, 0))
    return FALSE;

  /* remove the trailing delimiter byte from the head now.
   */
  if (!_dbus_string_set_length (source, head_length))
    return FALSE;

  return TRUE;
}

/* Unicode macros and utf8_validate() from GLib Owen Taylor, Havoc
 * Pennington, and Tom Tromey are the authors and authorized relicense.
 */

/** computes length and mask of a unicode character
 * @param Char the char
 * @param Mask the mask variable to assign to
 * @param Len the length variable to assign to
 */
#define UTF8_COMPUTE(Char, Mask, Len)					      \
  if (Char < 128)							      \
    {									      \
      Len = 1;								      \
      Mask = 0x7f;							      \
    }									      \
  else if ((Char & 0xe0) == 0xc0)					      \
    {									      \
      Len = 2;								      \
      Mask = 0x1f;							      \
    }									      \
  else if ((Char & 0xf0) == 0xe0)					      \
    {									      \
      Len = 3;								      \
      Mask = 0x0f;							      \
    }									      \
  else if ((Char & 0xf8) == 0xf0)					      \
    {									      \
      Len = 4;								      \
      Mask = 0x07;							      \
    }									      \
  else if ((Char & 0xfc) == 0xf8)					      \
    {									      \
      Len = 5;								      \
      Mask = 0x03;							      \
    }									      \
  else if ((Char & 0xfe) == 0xfc)					      \
    {									      \
      Len = 6;								      \
      Mask = 0x01;							      \
    }									      \
  else                                                                        \
    {                                                                         \
      Len = 0;                                                               \
      Mask = 0;                                                               \
    }

/**
 * computes length of a unicode character in UTF-8
 * @param Char the char
 */
#define UTF8_LENGTH(Char)              \
  ((Char) < 0x80 ? 1 :                 \
   ((Char) < 0x800 ? 2 :               \
    ((Char) < 0x10000 ? 3 :            \
     ((Char) < 0x200000 ? 4 :          \
      ((Char) < 0x4000000 ? 5 : 6)))))
   
/**
 * Gets a UTF-8 value.
 *
 * @param Result variable for extracted unicode char.
 * @param Chars the bytes to decode
 * @param Count counter variable
 * @param Mask mask for this char
 * @param Len length for this char in bytes
 */
#define UTF8_GET(Result, Chars, Count, Mask, Len)			      \
  (Result) = (Chars)[0] & (Mask);					      \
  for ((Count) = 1; (Count) < (Len); ++(Count))				      \
    {									      \
      if (((Chars)[(Count)] & 0xc0) != 0x80)				      \
	{								      \
	  (Result) = -1;						      \
	  break;							      \
	}								      \
      (Result) <<= 6;							      \
      (Result) |= ((Chars)[(Count)] & 0x3f);				      \
    }

/**
 * Check whether a Unicode (5.2) char is in a valid range.
 *
 * The first check comes from the Unicode guarantee to never encode
 * a point above 0x0010ffff, since UTF-16 couldn't represent it.
 *
 * The second check covers surrogate pairs (category Cs).
 *
 * @param Char the character
 */
#define UNICODE_VALID(Char)                   \
    ((Char) < 0x110000 &&                     \
     (((Char) & 0xFFFFF800) != 0xD800))

/**
 * Finds the given substring in the string,
 * returning #TRUE and filling in the byte index
 * where the substring was found, if it was found.
 * Returns #FALSE if the substring wasn't found.
 * Sets *start to the length of the string if the substring
 * is not found.
 *
 * @param str the string
 * @param start where to start looking
 * @param substr the substring
 * @param found return location for where it was found, or #NULL
 * @returns #TRUE if found
 */
dbus_bool_t
_dbus_string_find (const DBusString *str,
                   int               start,
                   const char       *substr,
                   int              *found)
{
  return _dbus_string_find_to (str, start,
                               ((const DBusRealString*)str)->len,
                               substr, found);
}

/**
 * Finds end of line ("\r\n" or "\n") in the string,
 * returning #TRUE and filling in the byte index
 * where the eol string was found, if it was found.
 * Returns #FALSE if eol wasn't found.
 *
 * @param str the string
 * @param start where to start looking
 * @param found return location for where eol was found or string length otherwise
 * @param found_len return length of found eol string or zero otherwise
 * @returns #TRUE if found
 */
dbus_bool_t
_dbus_string_find_eol (const DBusString *str,
                       int               start,
                       int              *found,
                       int              *found_len)
{
  int i;

  DBUS_CONST_STRING_PREAMBLE (str);
  _dbus_assert (start <= real->len);
  _dbus_assert (start >= 0);
  
  i = start;
  while (i < real->len)
    {
      if (real->str[i] == '\r') 
        {
          if ((i+1) < real->len && real->str[i+1] == '\n') /* "\r\n" */
            {
              if (found) 
                *found = i;
              if (found_len)
                *found_len = 2;
              return TRUE;
            } 
          else /* only "\r" */
            {
              if (found) 
                *found = i;
              if (found_len)
                *found_len = 1;
              return TRUE;
            }
        } 
      else if (real->str[i] == '\n')  /* only "\n" */
        {
          if (found) 
            *found = i;
          if (found_len)
            *found_len = 1;
          return TRUE;
        }
      ++i;
    }

  if (found)
    *found = real->len;

  if (found_len)
    *found_len = 0;
  
  return FALSE;
}

/**
 * Finds the given substring in the string,
 * up to a certain position,
 * returning #TRUE and filling in the byte index
 * where the substring was found, if it was found.
 * Returns #FALSE if the substring wasn't found.
 * Sets *start to the length of the string if the substring
 * is not found.
 *
 * @param str the string
 * @param start where to start looking
 * @param end where to stop looking
 * @param substr the substring
 * @param found return location for where it was found, or #NULL
 * @returns #TRUE if found
 */
dbus_bool_t
_dbus_string_find_to (const DBusString *str,
		      int               start,
		      int               end,
		      const char       *substr,
		      int              *found)
{
  int i;
  DBUS_CONST_STRING_PREAMBLE (str);
  _dbus_assert (substr != NULL);
  _dbus_assert (start <= real->len);
  _dbus_assert (start >= 0);
  _dbus_assert (substr != NULL);
  _dbus_assert (end <= real->len);
  _dbus_assert (start <= end);

  /* we always "find" an empty string */
  if (*substr == '\0')
    {
      if (found)
        *found = start;
      return TRUE;
    }

  i = start;
  while (i < end)
    {
      if (real->str[i] == substr[0])
        {
          int j = i + 1;
          
          while (j < end)
            {
              if (substr[j - i] == '\0')
                break;
              else if (real->str[j] != substr[j - i])
                break;
              
              ++j;
            }

          if (substr[j - i] == '\0')
            {
              if (found)
                *found = i;
              return TRUE;
            }
        }
      
      ++i;
    }

  if (found)
    *found = end;
  
  return FALSE;  
}

/**
 * Finds a blank (space or tab) in the string. Returns #TRUE
 * if found, #FALSE otherwise. If a blank is not found sets
 * *found to the length of the string.
 *
 * @param str the string
 * @param start byte index to start looking
 * @param found place to store the location of the first blank
 * @returns #TRUE if a blank was found
 */
dbus_bool_t
_dbus_string_find_blank (const DBusString *str,
                         int               start,
                         int              *found)
{
  int i;
  DBUS_CONST_STRING_PREAMBLE (str);
  _dbus_assert (start <= real->len);
  _dbus_assert (start >= 0);
  
  i = start;
  while (i < real->len)
    {
      if (real->str[i] == ' ' ||
          real->str[i] == '\t')
        {
          if (found)
            *found = i;
          return TRUE;
        }
      
      ++i;
    }

  if (found)
    *found = real->len;
  
  return FALSE;
}

/**
 * Skips blanks from start, storing the first non-blank in *end
 * (blank is space or tab).
 *
 * @param str the string
 * @param start where to start
 * @param end where to store the first non-blank byte index
 */
void
_dbus_string_skip_blank (const DBusString *str,
                         int               start,
                         int              *end)
{
  int i;
  DBUS_CONST_STRING_PREAMBLE (str);
  _dbus_assert (start <= real->len);
  _dbus_assert (start >= 0);
  
  i = start;
  while (i < real->len)
    {
      if (!DBUS_IS_ASCII_BLANK (real->str[i]))
        break;
      
      ++i;
    }

  _dbus_assert (i == real->len || !DBUS_IS_ASCII_WHITE (real->str[i]));
  
  if (end)
    *end = i;
}


/**
 * Skips whitespace from start, storing the first non-whitespace in *end.
 * (whitespace is space, tab, newline, CR).
 *
 * @param str the string
 * @param start where to start
 * @param end where to store the first non-whitespace byte index
 */
void
_dbus_string_skip_white (const DBusString *str,
                         int               start,
                         int              *end)
{
  int i;
  DBUS_CONST_STRING_PREAMBLE (str);
  _dbus_assert (start <= real->len);
  _dbus_assert (start >= 0);
  
  i = start;
  while (i < real->len)
    {
      if (!DBUS_IS_ASCII_WHITE (real->str[i]))
        break;
      
      ++i;
    }

  _dbus_assert (i == real->len || !(DBUS_IS_ASCII_WHITE (real->str[i])));
  
  if (end)
    *end = i;
}

/**
 * Skips whitespace from end, storing the start index of the trailing
 * whitespace in *start. (whitespace is space, tab, newline, CR).
 *
 * @param str the string
 * @param end where to start scanning backward
 * @param start where to store the start of whitespace chars
 */
void
_dbus_string_skip_white_reverse (const DBusString *str,
                                 int               end,
                                 int              *start)
{
  int i;
  DBUS_CONST_STRING_PREAMBLE (str);
  _dbus_assert (end <= real->len);
  _dbus_assert (end >= 0);
  
  i = end;
  while (i > 0)
    {
      if (!DBUS_IS_ASCII_WHITE (real->str[i-1]))
        break;
      --i;
    }

  _dbus_assert (i >= 0 && (i == 0 || !(DBUS_IS_ASCII_WHITE (real->str[i-1]))));
  
  if (start)
    *start = i;
}

/**
 * Assigns a newline-terminated or \\r\\n-terminated line from the front
 * of the string to the given dest string. The dest string's previous
 * contents are deleted. If the source string contains no newline,
 * moves the entire source string to the dest string.
 *
 * @todo owen correctly notes that this is a stupid function (it was
 * written purely for test code,
 * e.g. dbus-message-builder.c). Probably should be enforced as test
 * code only with ifdef DBUS_ENABLE_EMBEDDED_TESTS
 * 
 * @param source the source string
 * @param dest the destination string (contents are replaced)
 * @returns #FALSE if no memory, or source has length 0
 */
dbus_bool_t
_dbus_string_pop_line (DBusString *source,
                       DBusString *dest)
{
  int eol, eol_len;
  
  _dbus_string_set_length (dest, 0);
  
  eol = 0;
  eol_len = 0;
  if (!_dbus_string_find_eol (source, 0, &eol, &eol_len))
    {
      _dbus_assert (eol == _dbus_string_get_length (source));
      if (eol == 0)
        {
          /* If there's no newline and source has zero length, we're done */
          return FALSE;
        }
      /* otherwise, the last line of the file has no eol characters */
    }

  /* remember eol can be 0 if it's an empty line, but eol_len should not be zero also
   * since find_eol returned TRUE
   */
  
  if (!_dbus_string_move_len (source, 0, eol + eol_len, dest, 0))
    return FALSE;
  
  /* remove line ending */
  if (!_dbus_string_set_length (dest, eol))
    {
      _dbus_assert_not_reached ("out of memory when shortening a string");
      return FALSE;
    }

  return TRUE;
}

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
/**
 * Deletes up to and including the first blank space
 * in the string.
 *
 * @param str the string
 */
void
_dbus_string_delete_first_word (DBusString *str)
{
  int i;
  
  if (_dbus_string_find_blank (str, 0, &i))
    _dbus_string_skip_blank (str, i, &i);

  _dbus_string_delete (str, 0, i);
}
#endif

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
/**
 * Deletes any leading blanks in the string
 *
 * @param str the string
 */
void
_dbus_string_delete_leading_blanks (DBusString *str)
{
  int i;
  
  _dbus_string_skip_blank (str, 0, &i);

  if (i > 0)
    _dbus_string_delete (str, 0, i);
}
#endif

/**
 * Deletes leading and trailing whitespace
 * 
 * @param str the string
 */
void
_dbus_string_chop_white(DBusString *str)
{
  int i;
  
  _dbus_string_skip_white (str, 0, &i);

  if (i > 0)
    _dbus_string_delete (str, 0, i);
  
  _dbus_string_skip_white_reverse (str, _dbus_string_get_length (str), &i);

  _dbus_string_set_length (str, i);
}

/**
 * Tests two DBusString for equality.
 *
 * @todo memcmp is probably faster
 *
 * @param a first string
 * @param b second string
 * @returns #TRUE if equal
 */
dbus_bool_t
_dbus_string_equal (const DBusString *a,
                    const DBusString *b)
{
  const unsigned char *ap;
  const unsigned char *bp;
  const unsigned char *a_end;
  const DBusRealString *real_a = (const DBusRealString*) a;
  const DBusRealString *real_b = (const DBusRealString*) b;
  DBUS_GENERIC_STRING_PREAMBLE (real_a);
  DBUS_GENERIC_STRING_PREAMBLE (real_b);

  if (real_a->len != real_b->len)
    return FALSE;

  ap = real_a->str;
  bp = real_b->str;
  a_end = real_a->str + real_a->len;
  while (ap != a_end)
    {
      if (*ap != *bp)
        return FALSE;
      
      ++ap;
      ++bp;
    }

  return TRUE;
}

/**
 * Tests two DBusString for equality up to the given length.
 * The strings may be shorter than the given length.
 *
 * @todo write a unit test
 *
 * @todo memcmp is probably faster
 *
 * @param a first string
 * @param b second string
 * @param len the maximum length to look at
 * @returns #TRUE if equal for the given number of bytes
 */
dbus_bool_t
_dbus_string_equal_len (const DBusString *a,
                        const DBusString *b,
                        int               len)
{
  const unsigned char *ap;
  const unsigned char *bp;
  const unsigned char *a_end;
  const DBusRealString *real_a = (const DBusRealString*) a;
  const DBusRealString *real_b = (const DBusRealString*) b;
  DBUS_GENERIC_STRING_PREAMBLE (real_a);
  DBUS_GENERIC_STRING_PREAMBLE (real_b);

  if (real_a->len != real_b->len &&
      (real_a->len < len || real_b->len < len))
    return FALSE;

  ap = real_a->str;
  bp = real_b->str;
  a_end = real_a->str + MIN (real_a->len, len);
  while (ap != a_end)
    {
      if (*ap != *bp)
        return FALSE;
      
      ++ap;
      ++bp;
    }

  return TRUE;
}

/**
 * Tests two sub-parts of two DBusString for equality.  The specified
 * range of the first string must exist; the specified start position
 * of the second string must exist.
 *
 * @todo write a unit test
 *
 * @todo memcmp is probably faster
 *
 * @param a first string
 * @param a_start where to start substring in first string
 * @param a_len length of substring in first string
 * @param b second string
 * @param b_start where to start substring in second string
 * @returns #TRUE if the two substrings are equal
 */
dbus_bool_t
_dbus_string_equal_substring (const DBusString  *a,
                              int                a_start,
                              int                a_len,
                              const DBusString  *b,
                              int                b_start)
{
  const unsigned char *ap;
  const unsigned char *bp;
  const unsigned char *a_end;
  const DBusRealString *real_a = (const DBusRealString*) a;
  const DBusRealString *real_b = (const DBusRealString*) b;
  DBUS_GENERIC_STRING_PREAMBLE (real_a);
  DBUS_GENERIC_STRING_PREAMBLE (real_b);
  _dbus_assert (a_start >= 0);
  _dbus_assert (a_len >= 0);
  _dbus_assert (a_start <= real_a->len);
  _dbus_assert (a_len <= real_a->len - a_start);
  _dbus_assert (b_start >= 0);
  _dbus_assert (b_start <= real_b->len);
  
  if (a_len > real_b->len - b_start)
    return FALSE;

  ap = real_a->str + a_start;
  bp = real_b->str + b_start;
  a_end = ap + a_len;
  while (ap != a_end)
    {
      if (*ap != *bp)
        return FALSE;
      
      ++ap;
      ++bp;
    }

  _dbus_assert (bp <= (real_b->str + real_b->len));
  
  return TRUE;
}

/**
 * Checks whether a string is equal to a C string.
 *
 * @param a the string
 * @param c_str the C string
 * @returns #TRUE if equal
 */
dbus_bool_t
_dbus_string_equal_c_str (const DBusString *a,
                          const char       *c_str)
{
  const unsigned char *ap;
  const unsigned char *bp;
  const unsigned char *a_end;
  const DBusRealString *real_a = (const DBusRealString*) a;
  DBUS_GENERIC_STRING_PREAMBLE (real_a);
  _dbus_assert (c_str != NULL);
  
  ap = real_a->str;
  bp = (const unsigned char*) c_str;
  a_end = real_a->str + real_a->len;
  while (ap != a_end && *bp)
    {
      if (*ap != *bp)
        return FALSE;
      
      ++ap;
      ++bp;
    }

  if (ap != a_end || *bp)
    return FALSE;
  
  return TRUE;
}

/**
 * Checks whether a string starts with the given C string.
 *
 * @param a the string
 * @param c_str the C string
 * @returns #TRUE if string starts with it
 */
dbus_bool_t
_dbus_string_starts_with_c_str (const DBusString *a,
                                const char       *c_str)
{
  const unsigned char *ap;
  const unsigned char *bp;
  const unsigned char *a_end;
  const DBusRealString *real_a = (const DBusRealString*) a;
  DBUS_GENERIC_STRING_PREAMBLE (real_a);
  _dbus_assert (c_str != NULL);
  
  ap = real_a->str;
  bp = (const unsigned char*) c_str;
  a_end = real_a->str + real_a->len;
  while (ap != a_end && *bp)
    {
      if (*ap != *bp)
        return FALSE;
      
      ++ap;
      ++bp;
    }

  if (*bp == '\0')
    return TRUE;
  else
    return FALSE;
}

/**
 * Checks whether a string starts with the given C string, after which it ends or is separated from
 * the rest by a given separator character.
 *
 * @param a the string
 * @param c_str the C string
 * @param word_separator the separator
 * @returns #TRUE if string starts with it
 */
dbus_bool_t
_dbus_string_starts_with_words_c_str (const DBusString  *a,
                                      const char        *c_str,
                                      char               word_separator)
{
  char next_char;
  const char *data;
  _dbus_assert (c_str != NULL);

  if (!_dbus_string_starts_with_c_str (a, c_str))
    return FALSE;

  data = _dbus_string_get_const_data (a);
  next_char = data[strlen (c_str)];
  return next_char == '\0' || next_char == word_separator;
}

/**
 * Appends a two-character hex digit to a string, where the hex digit
 * has the value of the given byte.
 *
 * @param str the string
 * @param byte the byte
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_string_append_byte_as_hex (DBusString *str,
                                 unsigned char byte)
{
  const char hexdigits[16] = {
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
    'a', 'b', 'c', 'd', 'e', 'f'
  };

  if (!_dbus_string_append_byte (str,
                                 hexdigits[(byte >> 4)]))
    return FALSE;
  
  if (!_dbus_string_append_byte (str,
                                 hexdigits[(byte & 0x0f)]))
    {
      _dbus_string_set_length (str,
                               _dbus_string_get_length (str) - 1);
      return FALSE;
    }

  return TRUE;
}

/**
 * Encodes a string in hex, the way MD5 and SHA-1 are usually
 * encoded. (Each byte is two hex digits.)
 *
 * @param source the string to encode
 * @param start byte index to start encoding
 * @param dest string where encoded data should be placed
 * @param insert_at where to place encoded data
 * @returns #TRUE if encoding was successful, #FALSE if no memory etc.
 */
dbus_bool_t
_dbus_string_hex_encode (const DBusString *source,
                         int               start,
                         DBusString       *dest,
                         int               insert_at)
{
  DBusString result;
  const unsigned char *p;
  const unsigned char *end;
  dbus_bool_t retval;
  
  _dbus_assert (start <= _dbus_string_get_length (source));

  if (!_dbus_string_init (&result))
    return FALSE;

  retval = FALSE;
  
  p = (const unsigned char*) _dbus_string_get_const_data (source);
  end = p + _dbus_string_get_length (source);
  p += start;
  
  while (p != end)
    {
      if (!_dbus_string_append_byte_as_hex (&result, *p))
        goto out;
      
      ++p;
    }

  if (!_dbus_string_move (&result, 0, dest, insert_at))
    goto out;

  retval = TRUE;

 out:
  _dbus_string_free (&result);
  return retval;
}

/**
 * Decodes a string from hex encoding.
 *
 * @param source the string to decode
 * @param start byte index to start decode
 * @param end_return return location of the end of the hex data, or #NULL
 * @param dest string where decoded data should be placed
 * @param insert_at where to place decoded data
 * @returns #TRUE if decoding was successful, #FALSE if no memory.
 */
dbus_bool_t
_dbus_string_hex_decode (const DBusString *source,
                         int               start,
			 int              *end_return,
                         DBusString       *dest,
                         int               insert_at)
{
  DBusString result;
  const unsigned char *p;
  const unsigned char *end;
  dbus_bool_t retval;
  dbus_bool_t high_bits;
  
  _dbus_assert (start <= _dbus_string_get_length (source));

  if (!_dbus_string_init (&result))
    return FALSE;

  retval = FALSE;

  high_bits = TRUE;
  p = (const unsigned char*) _dbus_string_get_const_data (source);
  end = p + _dbus_string_get_length (source);
  p += start;
  
  while (p != end)
    {
      unsigned int val;

      switch (*p)
        {
        case '0':
          val = 0;
          break;
        case '1':
          val = 1;
          break;
        case '2':
          val = 2;
          break;
        case '3':
          val = 3;
          break;
        case '4':
          val = 4;
          break;
        case '5':
          val = 5;
          break;
        case '6':
          val = 6;
          break;
        case '7':
          val = 7;
          break;
        case '8':
          val = 8;
          break;
        case '9':
          val = 9;
          break;
        case 'a':
        case 'A':
          val = 10;
          break;
        case 'b':
        case 'B':
          val = 11;
          break;
        case 'c':
        case 'C':
          val = 12;
          break;
        case 'd':
        case 'D':
          val = 13;
          break;
        case 'e':
        case 'E':
          val = 14;
          break;
        case 'f':
        case 'F':
          val = 15;
          break;
        default:
          goto done;
        }

      if (high_bits)
        {
          if (!_dbus_string_append_byte (&result,
                                         val << 4))
	    goto out;
        }
      else
        {
          int len;
          unsigned char b;

          len = _dbus_string_get_length (&result);
          
          b = _dbus_string_get_byte (&result, len - 1);

          b |= val;

          _dbus_string_set_byte (&result, len - 1, b);
        }

      high_bits = !high_bits;

      ++p;
    }

 done:
  if (!_dbus_string_move (&result, 0, dest, insert_at))
    goto out;

  if (end_return)
    *end_return = p - (const unsigned char*) _dbus_string_get_const_data (source);

  retval = TRUE;
  
 out:
  _dbus_string_free (&result);  
  return retval;
}

/**
 * Checks that the given range of the string is valid ASCII with no
 * nul bytes. If the given range is not entirely contained in the
 * string, returns #FALSE.
 *
 * @todo this is inconsistent with most of DBusString in that
 * it allows a start,len range that extends past the string end.
 * 
 * @param str the string
 * @param start first byte index to check
 * @param len number of bytes to check
 * @returns #TRUE if the byte range exists and is all valid ASCII
 */
dbus_bool_t
_dbus_string_validate_ascii (const DBusString *str,
                             int               start,
                             int               len)
{
  const unsigned char *s;
  const unsigned char *end;
  DBUS_CONST_STRING_PREAMBLE (str);
  _dbus_assert (start >= 0);
  _dbus_assert (start <= real->len);
  _dbus_assert (len >= 0);
  
  if (len > real->len - start)
    return FALSE;
  
  s = real->str + start;
  end = s + len;
  while (s != end)
    {
      if (_DBUS_UNLIKELY (!_DBUS_ISASCII (*s)))
        return FALSE;
        
      ++s;
    }
  
  return TRUE;
}

/**
 * Converts the given range of the string to lower case.
 *
 * @param str the string
 * @param start first byte index to convert
 * @param len number of bytes to convert
 */
void
_dbus_string_tolower_ascii (const DBusString *str,
                            int               start,
                            int               len)
{
  unsigned char *s;
  unsigned char *end;
  DBUS_STRING_PREAMBLE (str);
  _dbus_assert (start >= 0);
  _dbus_assert (start <= real->len);
  _dbus_assert (len >= 0);
  _dbus_assert (len <= real->len - start);

  s = real->str + start;
  end = s + len;

  while (s != end)
    {
      if (*s >= 'A' && *s <= 'Z')
          *s += 'a' - 'A';
      ++s;
    }
}

/**
 * Converts the given range of the string to upper case.
 *
 * @param str the string
 * @param start first byte index to convert
 * @param len number of bytes to convert
 */
void
_dbus_string_toupper_ascii (const DBusString *str,
                            int               start,
                            int               len)
{
  unsigned char *s;
  unsigned char *end;
  DBUS_STRING_PREAMBLE (str);
  _dbus_assert (start >= 0);
  _dbus_assert (start <= real->len);
  _dbus_assert (len >= 0);
  _dbus_assert (len <= real->len - start);

  s = real->str + start;
  end = s + len;

  while (s != end)
    {
      if (*s >= 'a' && *s <= 'z')
          *s += 'A' - 'a';
      ++s;
    }
}

/**
 * Checks that the given range of the string is valid UTF-8. If the
 * given range is not entirely contained in the string, returns
 * #FALSE. If the string contains any nul bytes in the given range,
 * returns #FALSE. If the start and start+len are not on character
 * boundaries, returns #FALSE.
 *
 * @todo this is inconsistent with most of DBusString in that
 * it allows a start,len range that extends past the string end.
 * 
 * @param str the string
 * @param start first byte index to check
 * @param len number of bytes to check
 * @returns #TRUE if the byte range exists and is all valid UTF-8
 */
dbus_bool_t
_dbus_string_validate_utf8  (const DBusString *str,
                             int               start,
                             int               len)
{
  const unsigned char *p;
  const unsigned char *end;
  DBUS_CONST_STRING_PREAMBLE (str);
  _dbus_assert (start >= 0);
  _dbus_assert (start <= real->len);
  _dbus_assert (len >= 0);

  /* we are doing _DBUS_UNLIKELY() here which might be
   * dubious in a generic library like GLib, but in D-Bus
   * we know we're validating messages and that it would
   * only be evil/broken apps that would have invalid
   * UTF-8. Also, this function seems to be a performance
   * bottleneck in profiles.
   */
  
  if (_DBUS_UNLIKELY (len > real->len - start))
    return FALSE;
  
  p = real->str + start;
  end = p + len;
  
  while (p < end)
    {
      int i, mask, char_len;
      dbus_unichar_t result;

      /* nul bytes considered invalid */
      if (*p == '\0')
        break;
      
      /* Special-case ASCII; this makes us go a lot faster in
       * D-Bus profiles where we are typically validating
       * function names and such. We have to know that
       * all following checks will pass for ASCII though,
       * comments follow ...
       */      
      if (*p < 128)
        {
          ++p;
          continue;
        }
      
      UTF8_COMPUTE (*p, mask, char_len);

      if (_DBUS_UNLIKELY (char_len == 0))  /* ASCII: char_len == 1 */
        break;

      /* check that the expected number of bytes exists in the remaining length */
      if (_DBUS_UNLIKELY ((end - p) < char_len)) /* ASCII: p < end and char_len == 1 */
        break;
        
      UTF8_GET (result, p, i, mask, char_len);

      /* Check for overlong UTF-8 */
      if (_DBUS_UNLIKELY (UTF8_LENGTH (result) != char_len)) /* ASCII: UTF8_LENGTH == 1 */
        break;
#if 0
      /* The UNICODE_VALID check below will catch this */
      if (_DBUS_UNLIKELY (result == (dbus_unichar_t)-1)) /* ASCII: result = ascii value */
        break;
#endif

      if (_DBUS_UNLIKELY (!UNICODE_VALID (result))) /* ASCII: always valid */
        break;

      /* UNICODE_VALID should have caught it */
      _dbus_assert (result != (dbus_unichar_t)-1);
      
      p += char_len;
    }

  /* See that we covered the entire length if a length was
   * passed in
   */
  if (_DBUS_UNLIKELY (p != end))
    return FALSE;
  else
    return TRUE;
}

/**
 * Checks that the given range of the string is all nul bytes. If the
 * given range is not entirely contained in the string, returns
 * #FALSE.
 *
 * @todo this is inconsistent with most of DBusString in that
 * it allows a start,len range that extends past the string end.
 * 
 * @param str the string
 * @param start first byte index to check
 * @param len number of bytes to check
 * @returns #TRUE if the byte range exists and is all nul bytes
 */
dbus_bool_t
_dbus_string_validate_nul (const DBusString *str,
                           int               start,
                           int               len)
{
  const unsigned char *s;
  const unsigned char *end;
  DBUS_CONST_STRING_PREAMBLE (str);
  _dbus_assert (start >= 0);
  _dbus_assert (len >= 0);
  _dbus_assert (start <= real->len);
  
  if (len > real->len - start)
    return FALSE;
  
  s = real->str + start;
  end = s + len;
  while (s != end)
    {
      if (_DBUS_UNLIKELY (*s != '\0'))
        return FALSE;
      ++s;
    }
  
  return TRUE;
}

/**
 * Clears all allocated bytes in the string to zero.
 *
 * @param str the string
 */
void
_dbus_string_zero (DBusString *str)
{
  DBUS_STRING_PREAMBLE (str);

  memset (real->str - real->align_offset, '\0', real->allocated);
}
/** @} */

/* tests are in dbus-string-util.c */
