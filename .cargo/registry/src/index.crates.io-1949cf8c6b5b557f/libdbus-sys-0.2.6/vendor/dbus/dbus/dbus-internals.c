/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-internals.c  random utility stuff (internal to D-Bus implementation)
 *
 * Copyright (C) 2002, 2003  Red Hat, Inc.
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
#include "dbus-protocol.h"
#include "dbus-marshal-basic.h"
#include "dbus-test.h"
#include "dbus-test-tap.h"
#include "dbus-valgrind-internal.h"
#include <stdio.h>
#include <stdarg.h>
#include <string.h>
#include <stdlib.h>
#ifdef DBUS_USE_OUTPUT_DEBUG_STRING
#include <windows.h>
#include <mbstring.h>
#endif

/**
 * @defgroup DBusInternals D-Bus secret internal implementation details
 * @brief Documentation useful when developing or debugging D-Bus itself.
 * 
 */

/**
 * @defgroup DBusInternalsUtils Utilities and portability
 * @ingroup DBusInternals
 * @brief Utility functions (_dbus_assert(), _dbus_warn(), etc.)
 * @{
 */

/**
 * @def _dbus_assert
 *
 * Aborts with an error message if the condition is false.
 * 
 * @param condition condition which must be true.
 */

/**
 * @def _dbus_assert_not_reached
 *
 * Aborts with an error message if called.
 * The given explanation will be printed.
 * 
 * @param explanation explanation of what happened if the code was reached.
 */

/**
 * @def _DBUS_N_ELEMENTS
 *
 * Computes the number of elements in a fixed-size array using
 * sizeof().
 *
 * @param array the array to count elements in.
 */

/**
 * @def _DBUS_POINTER_TO_INT
 *
 * Safely casts a void* to an integer; should only be used on void*
 * that actually contain integers, for example one created with
 * _DBUS_INT_TO_POINTER.  Only guaranteed to preserve 32 bits.
 * (i.e. it's used to store 32-bit ints in pointers, but
 * can't be used to store 64-bit pointers in ints.)
 *
 * @param pointer pointer to extract an integer from.
 */
/**
 * @def _DBUS_INT_TO_POINTER
 *
 * Safely stuffs an integer into a pointer, to be extracted later with
 * _DBUS_POINTER_TO_INT. Only guaranteed to preserve 32 bits.
 *
 * @param integer the integer to stuff into a pointer.
 */
/**
 * @def _DBUS_ZERO
 *
 * Sets all bits in an object to zero.
 *
 * @param object the object to be zeroed.
 */
/**
 * @def _DBUS_INT16_MIN
 *
 * Minimum value of type "int16"
 */
/**
 * @def _DBUS_INT16_MAX
 *
 * Maximum value of type "int16"
 */
/**
 * @def _DBUS_UINT16_MAX
 *
 * Maximum value of type "uint16"
 */

/**
 * @def _DBUS_INT32_MIN
 *
 * Minimum value of type "int32"
 */
/**
 * @def _DBUS_INT32_MAX
 *
 * Maximum value of type "int32"
 */
/**
 * @def _DBUS_UINT32_MAX
 *
 * Maximum value of type "uint32"
 */

/**
 * @def _DBUS_INT_MIN
 *
 * Minimum value of type "int"
 */
/**
 * @def _DBUS_INT_MAX
 *
 * Maximum value of type "int"
 */
/**
 * @def _DBUS_UINT_MAX
 *
 * Maximum value of type "uint"
 */

/**
 * @typedef DBusForeachFunction
 * 
 * Used to iterate over each item in a collection, such as
 * a DBusList.
 */

/**
 * @def _DBUS_LOCK_NAME
 *
 * Expands to name of a global lock variable.
 */

/**
 * @def _DBUS_LOCK
 *
 * Locks a global lock, initializing it first if necessary.
 *
 * @returns #FALSE if not enough memory
 */

/**
 * @def _DBUS_UNLOCK
 *
 * Unlocks a global lock
 */

/**
 * Fixed "out of memory" error message, just to avoid
 * making up a different string every time and wasting
 * space.
 */
const char *_dbus_no_memory_message = "Not enough memory";

/* Not necessarily thread-safe, but if writes don't propagate between
 * threads, the worst that will happen is that we duplicate work in more than
 * one thread. */
static dbus_bool_t warn_initted = FALSE;

/* Not necessarily thread-safe, but if writes don't propagate between
 * threads, the worst that will happen is that warnings get their default
 * fatal/non-fatal nature. */
static dbus_bool_t fatal_warnings = FALSE;
static dbus_bool_t fatal_warnings_on_check_failed = TRUE;

static void
init_warnings(void)
{
  if (!warn_initted)
    {
      const char *s;
      s = _dbus_getenv ("DBUS_FATAL_WARNINGS");
      if (s && *s)
        {
          if (*s == '0')
            {
              fatal_warnings = FALSE;
              fatal_warnings_on_check_failed = FALSE;
            }
          else if (*s == '1')
            {
              fatal_warnings = TRUE;
              fatal_warnings_on_check_failed = TRUE;
            }
          else
            {
              fprintf(stderr, "DBUS_FATAL_WARNINGS should be set to 0 or 1 if set, not '%s'",
                      s);
            }
        }

      warn_initted = TRUE;
    }
}

/**
 * Prints a warning message to stderr. Can optionally be made to exit
 * fatally by setting DBUS_FATAL_WARNINGS, but this is rarely
 * used. This function should be considered pretty much equivalent to
 * fprintf(stderr). _dbus_warn_check_failed() on the other hand is
 * suitable for use when a programming mistake has been made.
 *
 * @param format printf-style format string.
 */
void
_dbus_warn (const char *format,
            ...)
{
  DBusSystemLogSeverity severity = DBUS_SYSTEM_LOG_WARNING;
  va_list args;

  if (!warn_initted)
    init_warnings ();

  if (fatal_warnings)
    severity = DBUS_SYSTEM_LOG_ERROR;

  va_start (args, format);
  _dbus_logv (severity, format, args);
  va_end (args);

  if (fatal_warnings)
    {
      fflush (stderr);
      _dbus_abort ();
    }
}

/**
 * Prints a "critical" warning to stderr when an assertion fails;
 * differs from _dbus_warn primarily in that it
 * defaults to fatal. This should be used only when a programming
 * error has been detected. (NOT for unavoidable errors that an app
 * might handle - those should be returned as DBusError.) Calling this
 * means "there is a bug"
 */
void
_dbus_warn_check_failed(const char *format,
                        ...)
{
  DBusSystemLogSeverity severity = DBUS_SYSTEM_LOG_WARNING;
  va_list args;
  
  if (!warn_initted)
    init_warnings ();

  if (fatal_warnings_on_check_failed)
    severity = DBUS_SYSTEM_LOG_ERROR;

  va_start (args, format);
  _dbus_logv (severity, format, args);
  va_end (args);

  if (fatal_warnings_on_check_failed)
    {
      fflush (stderr);
      _dbus_abort ();
    }
}

#ifdef DBUS_ENABLE_VERBOSE_MODE

static dbus_bool_t verbose_initted = FALSE;
static dbus_bool_t verbose = TRUE;

#ifdef DBUS_USE_OUTPUT_DEBUG_STRING
static char module_name[1024];
#endif

static inline void
_dbus_verbose_init (void)
{
  if (!verbose_initted)
    {
      const char *p = _dbus_getenv ("DBUS_VERBOSE");
      verbose = p != NULL && *p == '1';
      verbose_initted = TRUE;
#ifdef DBUS_USE_OUTPUT_DEBUG_STRING
      {
        char *last_period, *last_slash;
        GetModuleFileName(0,module_name,sizeof(module_name)-1);
        last_period = _mbsrchr(module_name,'.');
        if (last_period)
          *last_period ='\0';
        last_slash = _mbsrchr(module_name,'\\');
        if (last_slash)
          strcpy(module_name,last_slash+1);
        strcat(module_name,": ");
      }
#endif
    }
}

/** @def DBUS_IS_DIR_SEPARATOR(c)
 * macro for checking if character c is a patch separator
 * 
 * @todo move to a header file so that others can use this too
 */
#ifdef DBUS_WIN 
#define DBUS_IS_DIR_SEPARATOR(c) (c == '\\' || c == '/')
#else
#define DBUS_IS_DIR_SEPARATOR(c) (c == '/')
#endif

/** 
 remove source root from file path 
 the source root is determined by 
*/ 
static char *_dbus_file_path_extract_elements_from_tail(const char *file,int level)
{
  int prefix = 0;
  char *p = (char *)file + strlen(file);
  int i = 0;

  for (;p >= file;p--)
    {
      if (DBUS_IS_DIR_SEPARATOR(*p))
        {
          if (++i >= level)
            {
              prefix = p-file+1;
              break;
            }
       }
    }

  return (char *)file+prefix;
}

/**
 * Implementation of dbus_is_verbose() macro if built with verbose logging
 * enabled.
 * @returns whether verbose logging is active.
 */
dbus_bool_t
_dbus_is_verbose_real (void)
{
  _dbus_verbose_init ();
  return verbose;
}

void _dbus_set_verbose (dbus_bool_t state)
{
    verbose = state;
}

dbus_bool_t _dbus_get_verbose (void)
{
    return verbose;
}

/**
 * Low-level function for displaying a string
 * for the predefined output channel, which
 * can be the Windows debug output port or stderr.
 *
 * This function must be used instead of
 * dbus_verbose(), if a dynamic memory request
 * cannot be used to avoid recursive call loops.
 *
 * @param s string to display
 */
void
_dbus_verbose_raw (const char *s)
{
  if (!_dbus_is_verbose_real ())
    return;
#ifdef DBUS_USE_OUTPUT_DEBUG_STRING
  OutputDebugStringA (s);
#else
  fputs (s, stderr);
  fflush (stderr);
#endif
}

/**
 * Prints a warning message to stderr
 * if the user has enabled verbose mode.
 * This is the real function implementation,
 * use _dbus_verbose() macro in code.
 *
 * @param format printf-style format string.
 */
void
_dbus_verbose_real (
#ifdef DBUS_CPP_SUPPORTS_VARIABLE_MACRO_ARGUMENTS
                    const char *file,
                    const int line, 
                    const char *function, 
#endif
                    const char *format,
                    ...)
{
  va_list args;
  static dbus_bool_t need_pid = TRUE;
  int len;
  long sec, usec;
  
  /* things are written a bit oddly here so that
   * in the non-verbose case we just have the one
   * conditional and return immediately.
   */
  if (!_dbus_is_verbose_real())
    return;

#ifndef DBUS_USE_OUTPUT_DEBUG_STRING
  /* Print out pid before the line */
  if (need_pid)
    {
      _dbus_print_thread ();
    }
  _dbus_get_real_time (&sec, &usec);
  fprintf (stderr, "%ld.%06ld ", sec, usec);
#endif

  /* Only print pid again if the next line is a new line */
  len = strlen (format);
  if (format[len-1] == '\n')
    need_pid = TRUE;
  else
    need_pid = FALSE;

  va_start (args, format);

#ifdef DBUS_USE_OUTPUT_DEBUG_STRING
  {
    DBusString out = _DBUS_STRING_INIT_INVALID;
    const char *message = NULL;

    if (!_dbus_string_init (&out))
      goto out;

    if (!_dbus_string_append (&out, module_name))
      goto out;

#ifdef DBUS_CPP_SUPPORTS_VARIABLE_MACRO_ARGUMENTS
    if (!_dbus_string_append_printf (&out, "[%s(%d):%s] ", _dbus_file_path_extract_elements_from_tail (file, 2), line, function))
      goto out;
#endif
    if (!_dbus_string_append_printf_valist (&out, format, args))
      goto out;
    message = _dbus_string_get_const_data (&out);
out:
    if (message == NULL)
      {
        OutputDebugStringA ("Out of memory while formatting verbose message: '''");
        OutputDebugStringA (format);
        OutputDebugStringA ("'''");
      }
    else
      {
        OutputDebugStringA (message);
      }
    _dbus_string_free (&out);
  }
#else
#ifdef DBUS_CPP_SUPPORTS_VARIABLE_MACRO_ARGUMENTS
  fprintf (stderr, "[%s(%d):%s] ",_dbus_file_path_extract_elements_from_tail(file,2),line,function);
#endif

  vfprintf (stderr, format, args);
  fflush (stderr);
#endif

  va_end (args);
}

/**
 * Reinitializes the verbose logging code, used
 * as a hack in dbus-spawn-unix.c so that a child
 * process re-reads its pid
 *
 */
void
_dbus_verbose_reset_real (void)
{
  verbose_initted = FALSE;
}

void
_dbus_trace_ref (const char *obj_name,
                 void       *obj,
                 int         old_refcount,
                 int         new_refcount,
                 const char *why,
                 const char *env_var,
                 int        *enabled)
{
  _dbus_assert (obj_name != NULL);
  _dbus_assert (obj != NULL);
  _dbus_assert (old_refcount >= -1);
  _dbus_assert (new_refcount >= -1);

  if (old_refcount == -1)
    {
      _dbus_assert (new_refcount == -1);
    }
  else
    {
      _dbus_assert (new_refcount >= 0);
      _dbus_assert (old_refcount >= 0);
      _dbus_assert (old_refcount > 0 || new_refcount > 0);
    }

  _dbus_assert (why != NULL);
  _dbus_assert (env_var != NULL);
  _dbus_assert (enabled != NULL);

  if (*enabled < 0)
    {
      const char *s = _dbus_getenv (env_var);

      *enabled = FALSE;

      if (s && *s)
        {
          if (*s == '0')
            *enabled = FALSE;
          else if (*s == '1')
            *enabled = TRUE;
          else
            _dbus_warn ("%s should be 0 or 1 if set, not '%s'", env_var, s);
        }
    }

  if (*enabled)
    {
      if (old_refcount == -1)
        {
          VALGRIND_PRINTF_BACKTRACE ("%s %p ref stolen (%s)",
                                     obj_name, obj, why);
          _dbus_verbose ("%s %p ref stolen (%s)\n",
                         obj_name, obj, why);
        }
      else
        {
          VALGRIND_PRINTF_BACKTRACE ("%s %p %d -> %d refs (%s)",
                                     obj_name, obj,
                                     old_refcount, new_refcount, why);
          _dbus_verbose ("%s %p %d -> %d refs (%s)\n",
                         obj_name, obj, old_refcount, new_refcount, why);
        }
    }
}

#endif /* DBUS_ENABLE_VERBOSE_MODE */

/**
 * Duplicates a string. Result must be freed with
 * dbus_free(). Returns #NULL if memory allocation fails.
 * If the string to be duplicated is #NULL, returns #NULL.
 * 
 * @param str string to duplicate.
 * @returns newly-allocated copy.
 */
char*
_dbus_strdup (const char *str)
{
  size_t len;
  char *copy;
  
  if (str == NULL)
    return NULL;
  
  len = strlen (str);

  copy = dbus_malloc (len + 1);
  if (copy == NULL)
    return NULL;

  memcpy (copy, str, len + 1);
  
  return copy;
}

/**
 * Duplicates a block of memory. Returns
 * #NULL on failure.
 *
 * @param mem memory to copy
 * @param n_bytes number of bytes to copy
 * @returns the copy
 */
void*
_dbus_memdup (const void  *mem,
              size_t       n_bytes)
{
  void *copy;

  copy = dbus_malloc (n_bytes);
  if (copy == NULL)
    return NULL;

  memcpy (copy, mem, n_bytes);
  
  return copy;
}

/**
 * Duplicates a string array. Result may be freed with
 * dbus_free_string_array(). Returns #NULL if memory allocation fails.
 * If the array to be duplicated is #NULL, returns #NULL.
 * 
 * @param array array to duplicate.
 * @returns newly-allocated copy.
 */
char**
_dbus_dup_string_array (const char **array)
{
  int len;
  int i;
  char **copy;
  
  if (array == NULL)
    return NULL;

  for (len = 0; array[len] != NULL; ++len)
    ;

  copy = dbus_new0 (char*, len + 1);
  if (copy == NULL)
    return NULL;

  i = 0;
  while (i < len)
    {
      copy[i] = _dbus_strdup (array[i]);
      if (copy[i] == NULL)
        {
          dbus_free_string_array (copy);
          return NULL;
        }

      ++i;
    }

  return copy;
}

/**
 * Checks whether a string array contains the given string.
 * 
 * @param array array to search.
 * @param str string to look for
 * @returns #TRUE if array contains string
 */
dbus_bool_t
_dbus_string_array_contains (const char **array,
                             const char  *str)
{
  int i;

  i = 0;
  while (array[i] != NULL)
    {
      if (strcmp (array[i], str) == 0)
        return TRUE;
      ++i;
    }

  return FALSE;
}

/**
 * Returns the size of a string array
 *
 * @param array array to search.
 * @returns size of array
 */
size_t
_dbus_string_array_length (const char **array)
{
  size_t i;
  for (i = 0; array[i]; i++) {}
  return i;
}


/**
 * Generates a new UUID. If you change how this is done,
 * there's some text about it in the spec that should also change.
 *
 * @param uuid the uuid to initialize
 * @param error location to store reason for failure
 * @returns #TRUE on success
 */
dbus_bool_t
_dbus_generate_uuid (DBusGUID  *uuid,
                     DBusError *error)
{
  DBusError rand_error;
  long now;

  dbus_error_init (&rand_error);

  /* don't use monotonic time because the UUID may be saved to disk, e.g.
   * it may persist across reboots
   */
  _dbus_get_real_time (&now, NULL);

  uuid->as_uint32s[DBUS_UUID_LENGTH_WORDS - 1] = DBUS_UINT32_TO_BE (now);
  
  if (!_dbus_generate_random_bytes_buffer (uuid->as_bytes,
                                           DBUS_UUID_LENGTH_BYTES - 4,
                                           &rand_error))
    {
      dbus_set_error (error, rand_error.name,
                      "Failed to generate UUID: %s", rand_error.message);
      dbus_error_free (&rand_error);
      return FALSE;
    }

  return TRUE;
}

/**
 * Hex-encode a UUID.
 *
 * @param uuid the uuid
 * @param encoded string to append hex uuid to
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_uuid_encode (const DBusGUID *uuid,
                   DBusString     *encoded)
{
  DBusString binary;
  _dbus_string_init_const_len (&binary, uuid->as_bytes, DBUS_UUID_LENGTH_BYTES);
  return _dbus_string_hex_encode (&binary, 0, encoded, _dbus_string_get_length (encoded));
}

static dbus_bool_t
_dbus_read_uuid_file_without_creating (const DBusString *filename,
                                       DBusGUID         *uuid,
                                       DBusError        *error)
{
  DBusString contents;
  DBusString decoded;
  int end;
  
  if (!_dbus_string_init (&contents))
    {
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  if (!_dbus_string_init (&decoded))
    {
      _dbus_string_free (&contents);
      _DBUS_SET_OOM (error);
      return FALSE;
    }
  
  if (!_dbus_file_get_contents (&contents, filename, error))
    goto error;

  _dbus_string_chop_white (&contents);

  if (_dbus_string_get_length (&contents) != DBUS_UUID_LENGTH_HEX)
    {
      dbus_set_error (error, DBUS_ERROR_INVALID_FILE_CONTENT,
                      "UUID file '%s' should contain a hex string of length %d, not length %d, with no other text",
                      _dbus_string_get_const_data (filename),
                      DBUS_UUID_LENGTH_HEX,
                      _dbus_string_get_length (&contents));
      goto error;
    }

  if (!_dbus_string_hex_decode (&contents, 0, &end, &decoded, 0))
    {
      _DBUS_SET_OOM (error);
      goto error;
    }

  if (end == 0)
    {
      dbus_set_error (error, DBUS_ERROR_INVALID_FILE_CONTENT,
                      "UUID file '%s' contains invalid hex data",
                      _dbus_string_get_const_data (filename));
      goto error;
    }

  if (_dbus_string_get_length (&decoded) != DBUS_UUID_LENGTH_BYTES)
    {
      dbus_set_error (error, DBUS_ERROR_INVALID_FILE_CONTENT,
                      "UUID file '%s' contains %d bytes of hex-encoded data instead of %d",
                      _dbus_string_get_const_data (filename),
                      _dbus_string_get_length (&decoded),
                      DBUS_UUID_LENGTH_BYTES);
      goto error;
    }

  _dbus_string_copy_to_buffer (&decoded, uuid->as_bytes, DBUS_UUID_LENGTH_BYTES);

  _dbus_string_free (&decoded);
  _dbus_string_free (&contents);

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  return TRUE;
  
 error:
  _DBUS_ASSERT_ERROR_IS_SET (error);
  _dbus_string_free (&contents);
  _dbus_string_free (&decoded);
  return FALSE;
}

/**
 * Write the give UUID to a file.
 *
 * @param filename the file to write
 * @param uuid the UUID to save
 * @param error used to raise an error
 * @returns #FALSE on error
 */
dbus_bool_t
_dbus_write_uuid_file (const DBusString *filename,
                       const DBusGUID   *uuid,
                       DBusError        *error)
{
  DBusString encoded;

  if (!_dbus_string_init (&encoded))
    {
      _DBUS_SET_OOM (error);
      return FALSE;
    }
  
  if (!_dbus_uuid_encode (uuid, &encoded))
    {
      _DBUS_SET_OOM (error);
      goto error;
    }
  
  if (!_dbus_string_append_byte (&encoded, '\n'))
    {
      _DBUS_SET_OOM (error);
      goto error;
    }
  
  if (!_dbus_string_save_to_file (&encoded, filename, TRUE, error))
    goto error;

  _dbus_string_free (&encoded);

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  return TRUE;
  
 error:
  _DBUS_ASSERT_ERROR_IS_SET (error);
  _dbus_string_free (&encoded);
  return FALSE;        
}

/**
 * Reads (and optionally writes) a uuid to a file. Initializes the uuid
 * unless an error is returned.
 *
 * @param filename the name of the file
 * @param uuid uuid to be initialized with the loaded uuid
 * @param create_if_not_found #TRUE to create a new uuid and save it if the file doesn't exist
 * @param error the error return
 * @returns #FALSE if the error is set
 */
dbus_bool_t
_dbus_read_uuid_file (const DBusString *filename,
                      DBusGUID         *uuid,
                      dbus_bool_t       create_if_not_found,
                      DBusError        *error)
{
  DBusError read_error = DBUS_ERROR_INIT;

  if (_dbus_read_uuid_file_without_creating (filename, uuid, &read_error))
    return TRUE;

  if (!create_if_not_found)
    {
      dbus_move_error (&read_error, error);
      return FALSE;
    }

  /* If the file exists and contains junk, we want to keep that error
   * message instead of overwriting it with a "file exists" error
   * message when we try to write
   */
  if (dbus_error_has_name (&read_error, DBUS_ERROR_INVALID_FILE_CONTENT))
    {
      dbus_move_error (&read_error, error);
      return FALSE;
    }
  else
    {
      dbus_error_free (&read_error);

      if (!_dbus_generate_uuid (uuid, error))
        return FALSE;

      return _dbus_write_uuid_file (filename, uuid, error);
    }
}

/* Protected by _DBUS_LOCK (machine_uuid) */
static int machine_uuid_initialized_generation = 0;
static DBusGUID machine_uuid;

/**
 * Gets the hex-encoded UUID of the machine this function is
 * executed on. This UUID is guaranteed to be the same for a given
 * machine at least until it next reboots, though it also
 * makes some effort to be the same forever, it may change if the
 * machine is reconfigured or its hardware is modified.
 * 
 * @param uuid_str string to append hex-encoded machine uuid to
 * @param error location to store reason for failure
 * @returns #TRUE if successful
 */
dbus_bool_t
_dbus_get_local_machine_uuid_encoded (DBusString *uuid_str,
                                      DBusError  *error)
{
  dbus_bool_t ok = TRUE;
  
  if (!_DBUS_LOCK (machine_uuid))
    {
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  if (machine_uuid_initialized_generation != _dbus_current_generation)
    {
      if (!_dbus_read_local_machine_uuid (&machine_uuid, FALSE, error))
        ok = FALSE;
    }

  if (ok)
    {
      if (!_dbus_uuid_encode (&machine_uuid, uuid_str))
        {
          ok = FALSE;
          _DBUS_SET_OOM (error);
        }
    }

  _DBUS_UNLOCK (machine_uuid);

  return ok;
}

#ifndef DBUS_DISABLE_CHECKS
void
_dbus_warn_return_if_fail (const char *function,
                           const char *assertion,
                           const char *file,
                           int line)
{
  _dbus_warn_check_failed (
      "arguments to %s() were incorrect, assertion \"%s\" failed in file %s line %d.\n"
      "This is normally a bug in some application using the D-Bus library.\n",
      function, assertion, file, line);
}
#endif

#ifndef DBUS_DISABLE_ASSERT
/**
 * Internals of _dbus_assert(); it's a function
 * rather than a macro with the inline code so
 * that the assertion failure blocks don't show up
 * in test suite coverage, and to shrink code size.
 *
 * @param condition TRUE if assertion succeeded
 * @param condition_text condition as a string
 * @param file file the assertion is in
 * @param line line the assertion is in
 * @param func function the assertion is in
 */
void
_dbus_real_assert (dbus_bool_t  condition,
                   const char  *condition_text,
                   const char  *file,
                   int          line,
                   const char  *func)
{
  if (_DBUS_UNLIKELY (!condition))
    {
      _dbus_warn ("assertion failed \"%s\" file \"%s\" line %d function %s",
                  condition_text, file, line, func);
      _dbus_abort ();
    }
}

/**
 * Internals of _dbus_assert_not_reached(); it's a function
 * rather than a macro with the inline code so
 * that the assertion failure blocks don't show up
 * in test suite coverage, and to shrink code size.
 *
 * @param explanation what was reached that shouldn't have been
 * @param file file the assertion is in
 * @param line line the assertion is in
 */
void
_dbus_real_assert_not_reached (const char *explanation,
                               const char *file,
                               int         line)
{
  _dbus_warn ("File \"%s\" line %d should not have been reached: %s",
              file, line, explanation);
  _dbus_abort ();
}
#endif /* DBUS_DISABLE_ASSERT */
  
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
static dbus_bool_t
run_failing_each_malloc (int                    n_mallocs,
                         const char            *description,
                         DBusTestMemoryFunction func,
                         void                  *data)
{
  n_mallocs += 10; /* fudge factor to ensure reallocs etc. are covered */
  
  while (n_mallocs >= 0)
    {      
      _dbus_set_fail_alloc_counter (n_mallocs);

      _dbus_test_diag ("%s: will fail malloc %d and %d that follow",
                       description, n_mallocs,
                       _dbus_get_fail_alloc_failures () - 1);

      if (!(* func) (data, FALSE))
        return FALSE;
      
      n_mallocs -= 1;
    }

  _dbus_set_fail_alloc_counter (_DBUS_INT_MAX);

  return TRUE;
}                        

/**
 * Tests how well the given function responds to out-of-memory
 * situations. Calls the function repeatedly, failing a different
 * call to malloc() each time. If the function ever returns #FALSE,
 * the test fails. The function should return #TRUE whenever something
 * valid (such as returning an error, or succeeding) occurs, and #FALSE
 * if it gets confused in some way.
 *
 * @param description description of the test used in verbose output
 * @param func function to call
 * @param data data to pass to function
 * @returns #TRUE if the function never returns FALSE
 */
dbus_bool_t
_dbus_test_oom_handling (const char             *description,
                         DBusTestMemoryFunction  func,
                         void                   *data)
{
  int approx_mallocs;
  const char *setting;
  int max_failures_to_try;
  int i;

  /* Run once to see about how many mallocs are involved */
  
  _dbus_set_fail_alloc_counter (_DBUS_INT_MAX);

  _dbus_test_diag ("Running \"%s\" once to count mallocs", description);

  if (!(* func) (data, TRUE))
    return FALSE;

  approx_mallocs = _DBUS_INT_MAX - _dbus_get_fail_alloc_counter ();

  _dbus_test_diag ("\"%s\" has about %d mallocs in total",
                   description, approx_mallocs);

  setting = _dbus_getenv ("DBUS_TEST_MALLOC_FAILURES");

  if (setting != NULL)
    {
      DBusString str;
      long v;
      _dbus_string_init_const (&str, setting);
      v = 4;
      if (!_dbus_string_parse_int (&str, 0, &v, NULL))
        _dbus_warn ("couldn't parse '%s' as integer\n", setting);
      max_failures_to_try = v;
    }
  else
    {
      max_failures_to_try = 4;
    }

  if (RUNNING_ON_VALGRIND && _dbus_getenv ("DBUS_TEST_SLOW") == NULL)
    {
      /* The full OOM testing is slow, valgrind is slow, so the
       * combination is just horrible. Only do this if the user
       * asked for extra-slow tests. */
      max_failures_to_try = 0;
    }

  if (max_failures_to_try < 1)
    {
      _dbus_test_diag ("not testing OOM handling");
      return TRUE;
    }

  _dbus_test_diag ("testing \"%s\" with up to %d consecutive malloc failures",
                   description, max_failures_to_try);

  i = setting ? max_failures_to_try - 1 : 1;
  while (i < max_failures_to_try)
    {
      _dbus_test_diag ("testing \"%s\" with %d consecutive malloc failures",
                       description, i + 1);

      _dbus_set_fail_alloc_failures (i);
      if (!run_failing_each_malloc (approx_mallocs, description, func, data))
        return FALSE;
      ++i;
    }
  
  _dbus_verbose ("\"%s\" coped OK with malloc failures\n", description);

  return TRUE;
}
#endif /* DBUS_ENABLE_EMBEDDED_TESTS */

/** @} */
