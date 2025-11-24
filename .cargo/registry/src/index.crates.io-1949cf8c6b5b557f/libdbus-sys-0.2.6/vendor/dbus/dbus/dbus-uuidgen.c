/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-uuidgen.c  The guts of the dbus-uuidgen binary live in libdbus, in this file.
 *
 * Copyright (C) 2006  Red Hat, Inc.
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
#include "dbus-uuidgen.h"
#include "dbus-internals.h"
#include "dbus-string.h"
#include "dbus-protocol.h"

#ifdef DBUS_WIN
#error "dbus-uuidgen should not be needed on Windows"
#endif

/**
 * @defgroup DBusInternalsUuidgen dbus-uuidgen implementation
 * @ingroup DBusInternals
 * @brief Functions for dbus-uuidgen binary
 *
 * These are not considered part of the ABI, and if you call them
 * you will get screwed by future changes.
 * 
 * @{
 */

static dbus_bool_t
return_uuid (DBusGUID   *uuid,
             char      **uuid_p,
             DBusError  *error)
{
  if (uuid_p)
    {
      DBusString encoded;

      if (!_dbus_string_init (&encoded))
        {
          _DBUS_SET_OOM (error);
          return FALSE;
        }

      if (!_dbus_uuid_encode (uuid, &encoded) ||
          !_dbus_string_steal_data (&encoded, uuid_p))
        {
          _DBUS_SET_OOM (error);
          _dbus_string_free (&encoded);
          return FALSE;
        }
      _dbus_string_free (&encoded);
    }
  return TRUE;
}

/**
 * For use by the dbus-uuidgen binary ONLY, do not call this.
 * We can and will change this function without modifying
 * the libdbus soname.
 *
 * @param filename the file or #NULL for the machine ID file
 * @param uuid_p out param to return the uuid
 * @param create_if_not_found whether to create it if not already there
 * @param error error return
 * @returns #FALSE if error is set
 */
dbus_bool_t
_dbus_get_uuid (const char   *filename,
                char        **uuid_p,
                dbus_bool_t   create_if_not_found,
                DBusError    *error)
{
  DBusGUID uuid;
  
  if (filename)
    {
      DBusString filename_str;
      _dbus_string_init_const (&filename_str, filename);
      if (!_dbus_read_uuid_file (&filename_str, &uuid, create_if_not_found, error))
        goto error;
    }
  else
    {
      if (!_dbus_read_local_machine_uuid (&uuid, create_if_not_found, error))
        goto error;
    }

  if (!return_uuid(&uuid, uuid_p, error))
    goto error;

  return TRUE;
  
 error:
  _DBUS_ASSERT_ERROR_IS_SET (error);
  return FALSE;
}

/**
 * @param uuid_p out param to return the uuid
 * @param error location to store reason for failure
 * @returns #TRUE on success
 */
dbus_bool_t
_dbus_create_uuid (char      **uuid_p,
                   DBusError  *error)
{
  DBusGUID uuid;

  if (!_dbus_generate_uuid (&uuid, error))
    return FALSE;

  return return_uuid (&uuid, uuid_p, error);
}

/** @} */
