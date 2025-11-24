/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-errors.c Error reporting
 *
 * Copyright (C) 2002, 2004  Red Hat Inc.
 * Copyright (C) 2003  CodeFactory AB
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
#include "dbus-errors.h"
#include "dbus-internals.h"
#include "dbus-string.h"
#include "dbus-protocol.h"
#include <stdarg.h>
#include <string.h>

/**
 * @defgroup DBusErrorInternals Error reporting internals
 * @ingroup  DBusInternals
 * @brief Error reporting internals
 * @{
 */

/**
 * @def DBUS_ERROR_INIT
 *
 * Expands to a suitable initializer for a DBusError on the stack.
 * Declaring a DBusError with:
 *
 * @code
 * DBusError error = DBUS_ERROR_INIT;
 *
 * do_things_with (&error);
 * @endcode
 *
 * is a more concise form of:
 *
 * @code
 * DBusError error;
 *
 * dbus_error_init (&error);
 * do_things_with (&error);
 * @endcode
 */

/**
 * Internals of DBusError
 */
typedef struct
{
  char *name; /**< error name */
  char *message; /**< error message */

  unsigned int const_message : 1; /**< Message is not owned by DBusError */

  unsigned int dummy2 : 1; /**< placeholder */
  unsigned int dummy3 : 1; /**< placeholder */
  unsigned int dummy4 : 1; /**< placeholder */
  unsigned int dummy5 : 1; /**< placeholder */

  void *padding1; /**< placeholder */
  
} DBusRealError;

_DBUS_STATIC_ASSERT (sizeof (DBusRealError) == sizeof (DBusError));

/**
 * Returns a longer message describing an error name.
 * If the error name is unknown, returns the name
 * itself.
 *
 * @param error the error to describe
 * @returns a constant string describing the error.
 */
static const char*
message_from_error (const char *error)
{
  if (strcmp (error, DBUS_ERROR_FAILED) == 0)
    return "Unknown error";
  else if (strcmp (error, DBUS_ERROR_NO_MEMORY) == 0)
    return "Not enough memory available";
  else if (strcmp (error, DBUS_ERROR_IO_ERROR) == 0)
    return "Error reading or writing data";
  else if (strcmp (error, DBUS_ERROR_BAD_ADDRESS) == 0)
    return "Could not parse address";
  else if (strcmp (error, DBUS_ERROR_NOT_SUPPORTED) == 0)
    return "Feature not supported";
  else if (strcmp (error, DBUS_ERROR_LIMITS_EXCEEDED) == 0)
    return "Resource limits exceeded";
  else if (strcmp (error, DBUS_ERROR_ACCESS_DENIED) == 0)
    return "Permission denied";
  else if (strcmp (error, DBUS_ERROR_AUTH_FAILED) == 0)
    return "Could not authenticate to server";
  else if (strcmp (error, DBUS_ERROR_NO_SERVER) == 0)
    return "No server available at address";
  else if (strcmp (error, DBUS_ERROR_TIMEOUT) == 0)
    return "Connection timed out";
  else if (strcmp (error, DBUS_ERROR_NO_NETWORK) == 0)
    return "Network unavailable";
  else if (strcmp (error, DBUS_ERROR_ADDRESS_IN_USE) == 0)
    return "Address already in use";
  else if (strcmp (error, DBUS_ERROR_DISCONNECTED) == 0)
    return "Disconnected.";
  else if (strcmp (error, DBUS_ERROR_INVALID_ARGS) == 0)
    return "Invalid arguments.";
  else if (strcmp (error, DBUS_ERROR_NO_REPLY) == 0)
    return "Did not get a reply message.";
  else if (strcmp (error, DBUS_ERROR_FILE_NOT_FOUND) == 0)
    return "File doesn't exist.";
  else if (strcmp (error, DBUS_ERROR_OBJECT_PATH_IN_USE) == 0)
    return "Object path already in use";
  else
    return error;
}

/** @} */ /* End of internals */

/**
 * @defgroup DBusErrors Error reporting
 * @ingroup  DBus
 * @brief Error reporting
 *
 * Types and functions related to reporting errors.
 *
 *
 * In essence D-Bus error reporting works as follows:
 *
 * @code
 * DBusError error;
 * dbus_error_init (&error);
 * dbus_some_function (arg1, arg2, &error);
 * if (dbus_error_is_set (&error))
 *   {
 *     fprintf (stderr, "an error occurred: %s\n", error.message);
 *     dbus_error_free (&error);
 *   }
 * @endcode
 *
 * By convention, all functions allow #NULL instead of a DBusError*,
 * so callers who don't care about the error can ignore it.
 * 
 * There are some rules. An error passed to a D-Bus function must
 * always be unset; you can't pass in an error that's already set.  If
 * a function has a return code indicating whether an error occurred,
 * and also a #DBusError parameter, then the error will always be set
 * if and only if the return code indicates an error occurred. i.e.
 * the return code and the error are never going to disagree.
 *
 * An error only needs to be freed if it's been set, not if
 * it's merely been initialized.
 *
 * You can check the specific error that occurred using
 * dbus_error_has_name().
 * 
 * Errors will not be set for programming errors, such as passing
 * invalid arguments to the libdbus API. Instead, libdbus will print
 * warnings, exit on a failed assertion, or even crash in those cases
 * (in other words, incorrect use of the API results in undefined
 * behavior, possibly accompanied by helpful debugging output if
 * you're lucky).
 * 
 * @{
 */

/**
 * Initializes a DBusError structure. Does not allocate any memory;
 * the error only needs to be freed if it is set at some point.
 *
 * @param error the DBusError.
 */
void
dbus_error_init (DBusError *error)
{
  DBusRealError *real;

  _DBUS_STATIC_ASSERT (sizeof (DBusError) == sizeof (DBusRealError));

  _dbus_return_if_fail (error != NULL);

  real = (DBusRealError *)error;
  
  real->name = NULL;  
  real->message = NULL;

  real->const_message = TRUE;
}

/**
 * Frees an error that's been set (or just initialized),
 * then reinitializes the error as in dbus_error_init().
 *
 * @param error memory where the error is stored.
 */
void
dbus_error_free (DBusError *error)
{
  DBusRealError *real;

  _dbus_return_if_fail (error != NULL);
  
  real = (DBusRealError *)error;

  if (!real->const_message)
    {
      dbus_free (real->name);
      dbus_free (real->message);
    }

  dbus_error_init (error);
}

/**
 * Assigns an error name and message to a DBusError.  Does nothing if
 * error is #NULL. The message may be #NULL, which means a default
 * message will be deduced from the name. The default message will be
 * totally useless, though, so using a #NULL message is not recommended.
 *
 * Because this function does not copy the error name or message, you
 * must ensure the name and message are global data that won't be
 * freed. You probably want dbus_set_error() instead, in most cases.
 * 
 * @param error the error or #NULL
 * @param name the error name (not copied!!!)
 * @param message the error message (not copied!!!)
 */
void
dbus_set_error_const (DBusError  *error,
		      const char *name,
		      const char *message)
{
  DBusRealError *real;

  _dbus_return_if_error_is_set (error);
  _dbus_return_if_fail (name != NULL);
  
  if (error == NULL)
    return;

  _dbus_assert (error->name == NULL);
  _dbus_assert (error->message == NULL);

  if (message == NULL)
    message = message_from_error (name);
  
  real = (DBusRealError *)error;
  
  real->name = (char*) name;
  real->message = (char *)message;
  real->const_message = TRUE;
}

/**
 * Moves an error src into dest, freeing src and
 * overwriting dest. Both src and dest must be initialized.
 * src is reinitialized to an empty error. dest may not
 * contain an existing error. If the destination is
 * #NULL, just frees and reinits the source error.
 * 
 * @param src the source error
 * @param dest the destination error or #NULL
 */
void
dbus_move_error (DBusError *src,
                 DBusError *dest)
{
  _dbus_return_if_error_is_set (dest);

  if (dest)
    {
      dbus_error_free (dest);
      *dest = *src;
      dbus_error_init (src);
    }
  else
    dbus_error_free (src);
}

/**
 * Checks whether the error is set and has the given
 * name.
 * @param error the error
 * @param name the name
 * @returns #TRUE if the given named error occurred
 */
dbus_bool_t
dbus_error_has_name (const DBusError *error,
                     const char      *name)
{
  _dbus_return_val_if_fail (error != NULL, FALSE);
  _dbus_return_val_if_fail (name != NULL, FALSE);

  _dbus_assert ((error->name != NULL && error->message != NULL) ||
                (error->name == NULL && error->message == NULL));
  
  if (error->name != NULL)
    {
      DBusString str1, str2;
      _dbus_string_init_const (&str1, error->name);
      _dbus_string_init_const (&str2, name);
      return _dbus_string_equal (&str1, &str2);
    }
  else
    return FALSE;
}

/**
 * Checks whether an error occurred (the error is set).
 *
 * @param error the error object
 * @returns #TRUE if an error occurred
 */
dbus_bool_t
dbus_error_is_set (const DBusError *error)
{
  _dbus_return_val_if_fail (error != NULL, FALSE);  
  _dbus_assert ((error->name != NULL && error->message != NULL) ||
                (error->name == NULL && error->message == NULL));
  return error->name != NULL;
}

/**
 * Assigns an error name and message to a DBusError.
 * Does nothing if error is #NULL.
 *
 * The format may be #NULL, which means a (pretty much useless)
 * default message will be deduced from the name. This is not a good
 * idea, just go ahead and provide a useful error message. It won't
 * hurt you.
 *
 * If no memory can be allocated for the error message, 
 * an out-of-memory error message will be set instead.
 *
 * @param error the error.or #NULL
 * @param name the error name
 * @param format printf-style format string.
 */
void
dbus_set_error (DBusError  *error,
		const char *name,
		const char *format,
		...)
{
  va_list args;
  
  if (error == NULL)
    return;

  /* it's a bug to pile up errors */
  _dbus_return_if_error_is_set (error);
  _dbus_return_if_fail (name != NULL);

  va_start (args, format);
  _dbus_set_error_valist (error, name, format, args);
  va_end (args);
}

void
_dbus_set_error_valist (DBusError  *error,
                        const char *name,
                        const char *format,
                        va_list     args)
{
  DBusRealError *real;
  DBusString str;

  _dbus_assert (name != NULL);

  if (error == NULL)
    return;

  _dbus_assert (error->name == NULL);
  _dbus_assert (error->message == NULL);

  if (!_dbus_string_init (&str))
    goto nomem;
  
  if (format == NULL)
    {
      if (!_dbus_string_append (&str,
                                message_from_error (name)))
        {
          _dbus_string_free (&str);
          goto nomem;
        }
    }
  else
    {
      if (!_dbus_string_append_printf_valist (&str, format, args))
        {
          _dbus_string_free (&str);
          goto nomem;
        }
    }

  real = (DBusRealError *)error;

  if (!_dbus_string_steal_data (&str, &real->message))
    {
      _dbus_string_free (&str);
      goto nomem;
    }
  _dbus_string_free (&str);
  
  real->name = _dbus_strdup (name);
  if (real->name == NULL)
    {
      dbus_free (real->message);
      real->message = NULL;
      goto nomem;
    }
  real->const_message = FALSE;

  return;
  
 nomem:
  _DBUS_SET_OOM (error);
}

/** @} */ /* End public API */
