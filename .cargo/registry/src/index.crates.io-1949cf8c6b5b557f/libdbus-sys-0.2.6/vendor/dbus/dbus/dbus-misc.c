/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-misc.c  A few assorted public functions that don't fit elsewhere
 *
 * Copyright (C) 2006 Red Hat, Inc.
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
#include "dbus-misc.h"
#include "dbus-internals.h"
#include "dbus-string.h"
#include <dbus/dbus-test-tap.h>

/**
 * @defgroup DBusMisc Miscellaneous
 * @ingroup  DBus
 * @brief Miscellaneous API that doesn't cleanly fit anywhere else
 *
 * @{
 */

/**
 * Obtains the machine UUID of the machine this process is running on.
 *
 * The returned string must be freed with dbus_free().
 * 
 * This UUID is guaranteed to remain the same until the next reboot
 * (unless the sysadmin foolishly changes it and screws themselves).
 * It will usually remain the same across reboots also, but hardware
 * configuration changes or rebuilding the machine could break that.
 *
 * The idea is that two processes with the same machine ID should be
 * able to use shared memory, UNIX domain sockets, process IDs, and other
 * features of the OS that require both processes to be running
 * on the same OS kernel instance.
 *
 * The machine ID can also be used to create unique per-machine
 * instances. For example, you could use it in bus names or
 * X selection names.
 *
 * The machine ID is preferred over the machine hostname, because
 * the hostname is frequently set to "localhost.localdomain" and
 * may also change at runtime.
 *
 * You can get the machine ID of a remote application by invoking the
 * method GetMachineId from interface org.freedesktop.DBus.Peer.
 *
 * If the remote application has the same machine ID as the one
 * returned by this function, then the remote application is on the
 * same machine as your application.
 *
 * The UUID is not a UUID in the sense of RFC4122; the details
 * are explained in the D-Bus specification.
 *
 * @returns a 32-byte-long hex-encoded UUID string, or #NULL on failure
 */
char *
dbus_try_get_local_machine_id (DBusError *error)
{
  DBusString uuid;
  char *s;

  s = NULL;

  if (!_dbus_string_init (&uuid))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      return NULL;
    }

  if (!_dbus_get_local_machine_uuid_encoded (&uuid, error))
    {
      _dbus_string_free (&uuid);
      return NULL;
    }

  if (!_dbus_string_steal_data (&uuid, &s))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      _dbus_string_free (&uuid);
      return NULL;
    }
  else
    {
      _dbus_string_free (&uuid);
      return s;
    }

}

/**
 * Obtains the machine UUID of the machine this process is running on.
 *
 * The returned string must be freed with dbus_free().
 *
 * This function returns #NULL if there was not enough memory to read
 * the UUID, or if the UUID could not be read because the D-Bus
 * library was installed incorrectly. In the latter case, a warning
 * is logged.
 *
 * Other than its deficient error reporting, this function is the same as
 * dbus_try_get_local_machine_id().
 *
 * @returns a 32-byte-long hex-encoded UUID string, or #NULL on failure
 */
char *
dbus_get_local_machine_id (void)
{
  DBusError error = DBUS_ERROR_INIT;
  char *s;

  s = dbus_try_get_local_machine_id (&error);

  /* The documentation says dbus_get_local_machine_id() only fails on OOM;
   * this can actually also fail if the D-Bus installation is faulty
   * (no UUID), but we have no good way to report that. Historically,
   * _dbus_get_local_machine_uuid_encoded was responsible for issuing the
   * warning; now we do that here. */
  if (s == NULL)
    {
      if (!dbus_error_has_name (&error, DBUS_ERROR_NO_MEMORY))
        _dbus_warn_check_failed ("%s", error.message);

      dbus_error_free (&error);
      return NULL;
    }

  return s;
}

/**
 * @def DBUS_MAJOR_VERSION
 *
 * The COMPILE TIME major version of libdbus, that is, the "X" in "X.Y.Z",
 * as an integer literal. Consider carefully whether to use this or the
 * runtime version from dbus_get_version().
 */

/**
 * @def DBUS_MINOR_VERSION
 *
 * The COMPILE TIME minor version of libdbus, that is, the "Y" in "X.Y.Z",
 * as an integer literal. Consider carefully whether to use this or the
 * runtime version from dbus_get_version().
 */

/**
 * @def DBUS_MICRO_VERSION
 *
 * The COMPILE TIME micro version of libdbus, that is, the "Z" in "X.Y.Z",
 * as an integer literal. Consider carefully whether to use this or the
 * runtime version from dbus_get_version().
 */

/**
 * @def DBUS_VERSION
 *
 * The COMPILE TIME version of libdbus, as a single integer that has 0 in the most
 * significant byte, the major version in the next most significant byte,
 * the minor version in the third most significant, and the micro version in the
 * least significant byte. This means two DBUS_VERSION can be compared to see
 * which is higher.
 *
 * Consider carefully whether to use this or the runtime version from
 * dbus_get_version().
 */

/**
 * @def DBUS_VERSION_STRING
 *
 * The COMPILE TIME version of libdbus, as a string "X.Y.Z".
 *
 * Consider carefully whether to use this or the runtime version from
 * dbus_get_version().
 */

/**
 * Gets the DYNAMICALLY LINKED version of libdbus. Alternatively, there
 * are macros #DBUS_MAJOR_VERSION, #DBUS_MINOR_VERSION, #DBUS_MICRO_VERSION,
 * and #DBUS_VERSION which allow you to test the VERSION YOU ARE COMPILED AGAINST.
 * In other words, you can get either the runtime or the compile-time version.
 * Think carefully about which of these you want in a given case.
 *
 * The libdbus full version number is "MAJOR.MINOR.MICRO" where the
 * MINOR changes if API is added, and the MICRO changes with each
 * release of a MAJOR.MINOR series.  The MINOR is an odd number for
 * development releases and an even number for stable releases.
 * 
 * @param major_version_p pointer to return the major version, or #NULL
 * @param minor_version_p pointer to return the minor version, or #NULL
 * @param micro_version_p pointer to return the micro version, or #NULL 
 * 
 */
void
dbus_get_version (int *major_version_p,
                  int *minor_version_p,
                  int *micro_version_p)
{
  if (major_version_p)
    *major_version_p = DBUS_MAJOR_VERSION;
  if (minor_version_p)
    *minor_version_p = DBUS_MINOR_VERSION;
  if (micro_version_p)
    *micro_version_p = DBUS_MICRO_VERSION;
}


/** @} */ /* End of public API */
