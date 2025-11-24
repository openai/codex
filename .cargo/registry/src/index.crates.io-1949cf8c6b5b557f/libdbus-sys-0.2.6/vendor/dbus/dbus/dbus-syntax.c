/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-syntax.c - utility functions for strings with special syntax
 *
 * Author: Simon McVittie <simon.mcvittie@collabora.co.uk>
 * Copyright © 2011 Nokia Corporation
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
#include "dbus-syntax.h"

#include "dbus-internals.h"
#include "dbus-marshal-validate.h"
#include "dbus-shared.h"

/**
 * @defgroup DBusSyntax Utility functions for strings with special syntax
 * @ingroup  DBus
 * @brief Parsing D-Bus type signatures
 * @{
 */

/**
 * Check an object path for validity. Remember that #NULL can always
 * be passed instead of a DBusError *, if you don't care about having
 * an error name and message.
 *
 * This function is suitable for validating C strings, but is not suitable
 * for validating untrusted data from a network unless the string's length
 * is also checked, since it assumes that the string ends at the first zero
 * byte according to normal C conventions.
 *
 * @param path a potentially invalid object path, which must not be #NULL
 * @param error error return
 * @returns #TRUE if path is valid
 */
dbus_bool_t
dbus_validate_path (const char       *path,
                    DBusError        *error)
{
  DBusString str;
  int len;

  _dbus_return_val_if_fail (path != NULL, FALSE);

  _dbus_string_init_const (&str, path);
  len = _dbus_string_get_length (&str);

  /* In general, it ought to be valid... */
  if (_DBUS_LIKELY (_dbus_validate_path (&str, 0, len)))
    return TRUE;

  /* slow path: string is invalid, find out why */

  if (!_dbus_string_validate_utf8 (&str, 0, len))
    {
      /* don't quote the actual string here, since a DBusError also needs to
       * be valid UTF-8 */
      dbus_set_error (error, DBUS_ERROR_INVALID_ARGS,
                      "Object path was not valid UTF-8");
      return FALSE;
    }

  /* FIXME: later, diagnose exactly how it was invalid */
  dbus_set_error (error, DBUS_ERROR_INVALID_ARGS,
                  "Object path was not valid: '%s'", path);
  return FALSE;
}

/**
 * Check an interface name for validity. Remember that #NULL can always
 * be passed instead of a DBusError *, if you don't care about having
 * an error name and message.
 *
 * This function is suitable for validating C strings, but is not suitable
 * for validating untrusted data from a network unless the string's length
 * is also checked, since it assumes that the string ends at the first zero
 * byte according to normal C conventions.
 *
 * @param name a potentially invalid interface name, which must not be #NULL
 * @param error error return
 * @returns #TRUE if name is valid
 */
dbus_bool_t
dbus_validate_interface (const char       *name,
                         DBusError        *error)
{
  DBusString str;
  int len;

  _dbus_return_val_if_fail (name != NULL, FALSE);

  _dbus_string_init_const (&str, name);
  len = _dbus_string_get_length (&str);

  /* In general, it ought to be valid... */
  if (_DBUS_LIKELY (_dbus_validate_interface (&str, 0, len)))
    return TRUE;

  /* slow path: string is invalid, find out why */

  if (!_dbus_string_validate_utf8 (&str, 0, len))
    {
      /* don't quote the actual string here, since a DBusError also needs to
       * be valid UTF-8 */
      dbus_set_error (error, DBUS_ERROR_INVALID_ARGS,
                      "Interface name was not valid UTF-8");
      return FALSE;
    }

  /* FIXME: later, diagnose exactly how it was invalid */
  dbus_set_error (error, DBUS_ERROR_INVALID_ARGS,
                  "Interface name was not valid: '%s'", name);
  return FALSE;
}

/**
 * Check a member (method/signal) name for validity. Remember that #NULL
 * can always be passed instead of a DBusError *, if you don't care about
 * having an error name and message.
 *
 * This function is suitable for validating C strings, but is not suitable
 * for validating untrusted data from a network unless the string's length
 * is also checked, since it assumes that the string ends at the first zero
 * byte according to normal C conventions.
 *
 * @param name a potentially invalid member name, which must not be #NULL
 * @param error error return
 * @returns #TRUE if name is valid
 */
dbus_bool_t
dbus_validate_member (const char       *name,
                      DBusError        *error)
{
  DBusString str;
  int len;

  _dbus_return_val_if_fail (name != NULL, FALSE);

  _dbus_string_init_const (&str, name);
  len = _dbus_string_get_length (&str);

  /* In general, it ought to be valid... */
  if (_DBUS_LIKELY (_dbus_validate_member (&str, 0, len)))
    return TRUE;

  /* slow path: string is invalid, find out why */

  if (!_dbus_string_validate_utf8 (&str, 0, len))
    {
      /* don't quote the actual string here, since a DBusError also needs to
       * be valid UTF-8 */
      dbus_set_error (error, DBUS_ERROR_INVALID_ARGS,
                      "Member name was not valid UTF-8");
      return FALSE;
    }

  /* FIXME: later, diagnose exactly how it was invalid */
  dbus_set_error (error, DBUS_ERROR_INVALID_ARGS,
                  "Member name was not valid: '%s'", name);
  return FALSE;
}

/**
 * Check an error name for validity. Remember that #NULL
 * can always be passed instead of a DBusError *, if you don't care about
 * having an error name and message.
 *
 * This function is suitable for validating C strings, but is not suitable
 * for validating untrusted data from a network unless the string's length
 * is also checked, since it assumes that the string ends at the first zero
 * byte according to normal C conventions.
 *
 * @param name a potentially invalid error name, which must not be #NULL
 * @param error error return
 * @returns #TRUE if name is valid
 */
dbus_bool_t
dbus_validate_error_name (const char       *name,
                          DBusError        *error)
{
  DBusString str;
  int len;

  _dbus_return_val_if_fail (name != NULL, FALSE);

  _dbus_string_init_const (&str, name);
  len = _dbus_string_get_length (&str);

  /* In general, it ought to be valid... */
  if (_DBUS_LIKELY (_dbus_validate_error_name (&str, 0, len)))
    return TRUE;

  /* slow path: string is invalid, find out why */

  if (!_dbus_string_validate_utf8 (&str, 0, len))
    {
      /* don't quote the actual string here, since a DBusError also needs to
       * be valid UTF-8 */
      dbus_set_error (error, DBUS_ERROR_INVALID_ARGS,
                      "Error name was not valid UTF-8");
      return FALSE;
    }

  /* FIXME: later, diagnose exactly how it was invalid */
  dbus_set_error (error, DBUS_ERROR_INVALID_ARGS,
                  "Error name was not valid: '%s'", name);
  return FALSE;
}

/**
 * Check a bus name for validity. Remember that #NULL
 * can always be passed instead of a DBusError *, if you don't care about
 * having an error name and message.
 *
 * This function is suitable for validating C strings, but is not suitable
 * for validating untrusted data from a network unless the string's length
 * is also checked, since it assumes that the string ends at the first zero
 * byte according to normal C conventions.
 *
 * @param name a potentially invalid bus name, which must not be #NULL
 * @param error error return
 * @returns #TRUE if name is valid
 */
dbus_bool_t
dbus_validate_bus_name (const char       *name,
                        DBusError        *error)
{
  DBusString str;
  int len;

  _dbus_return_val_if_fail (name != NULL, FALSE);

  _dbus_string_init_const (&str, name);
  len = _dbus_string_get_length (&str);

  /* In general, it ought to be valid... */
  if (_DBUS_LIKELY (_dbus_validate_bus_name (&str, 0, len)))
    return TRUE;

  /* slow path: string is invalid, find out why */

  if (!_dbus_string_validate_utf8 (&str, 0, len))
    {
      /* don't quote the actual string here, since a DBusError also needs to
       * be valid UTF-8 */
      dbus_set_error (error, DBUS_ERROR_INVALID_ARGS,
                      "Bus name was not valid UTF-8");
      return FALSE;
    }

  /* FIXME: later, diagnose exactly how it was invalid */
  dbus_set_error (error, DBUS_ERROR_INVALID_ARGS,
                  "Bus name was not valid: '%s'", name);
  return FALSE;
}

/**
 * Check a string for validity. Strings on D-Bus must be valid UTF-8.
 * Remember that #NULL can always be passed instead of a DBusError *,
 * if you don't care about having an error name and message.
 *
 * This function is suitable for validating C strings, but is not suitable
 * for validating untrusted data from a network unless the string's length
 * is also checked, since it assumes that the string ends at the first zero
 * byte according to normal C conventions.
 *
 * @param alleged_utf8 a string to be checked, which must not be #NULL
 * @param error error return
 * @returns #TRUE if alleged_utf8 is valid UTF-8
 */
dbus_bool_t
dbus_validate_utf8 (const char       *alleged_utf8,
                    DBusError        *error)
{
  DBusString str;

  _dbus_return_val_if_fail (alleged_utf8 != NULL, FALSE);

  _dbus_string_init_const (&str, alleged_utf8);

  if (_DBUS_LIKELY (_dbus_string_validate_utf8 (&str, 0,
                                                _dbus_string_get_length (&str))))
    return TRUE;

  /* don't quote the actual string here, since a DBusError also needs to
   * be valid UTF-8 */
  dbus_set_error (error, DBUS_ERROR_INVALID_ARGS,
                  "String was not valid UTF-8");
  return FALSE;
}

/** @} */ /* end of group */
