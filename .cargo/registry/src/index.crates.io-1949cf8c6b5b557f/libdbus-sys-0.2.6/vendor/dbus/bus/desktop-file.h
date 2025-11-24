/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* desktop-file.h  .desktop file parser
 *
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
#ifndef BUS_DESKTOP_FILE_H
#define BUS_DESKTOP_FILE_H

#include <dbus/dbus.h>
#include <dbus/dbus-string.h>

#define BUS_DESKTOP_PARSE_ERROR_INVALID_SYNTAX  "org.freedesktop.DBus.DesktopParseError.InvalidSyntax"
#define BUS_DESKTOP_PARSE_ERROR_INVALID_ESCAPES "org.freedesktop.DBus.DesktopParseError.InvalidEscapes"
#define BUS_DESKTOP_PARSE_ERROR_INVALID_CHARS   "org.freedesktop.DBus.DesktopParseError.InvalidChars"

#define DBUS_SERVICE_SECTION  "D-BUS Service"
#define DBUS_SERVICE_NAME     "Name"
#define DBUS_SERVICE_EXEC     "Exec"
#define DBUS_SERVICE_USER     "User"
#define DBUS_SERVICE_SYSTEMD_SERVICE "SystemdService"
#define DBUS_SERVICE_ASSUMED_APPARMOR_LABEL "AssumedAppArmorLabel"

typedef struct BusDesktopFile BusDesktopFile;

BusDesktopFile *bus_desktop_file_load (DBusString     *filename,
				       DBusError      *error);
void            bus_desktop_file_free (BusDesktopFile *file);

dbus_bool_t bus_desktop_file_get_raw    (BusDesktopFile  *desktop_file,
					 const char      *section_name,
					 const char      *keyname,
					 const char     **val);
dbus_bool_t bus_desktop_file_get_string (BusDesktopFile  *desktop_file,
					 const char      *section,
					 const char      *keyname,
					 char           **val,
					 DBusError       *error);

static inline void
bus_clear_desktop_file (BusDesktopFile **desktop_p)
{
  _dbus_clear_pointer_impl (BusDesktopFile, desktop_p, bus_desktop_file_free);
}

#endif /* BUS_DESKTOP_FILE_H */
