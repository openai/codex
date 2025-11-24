/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus.h  Convenience header including all other headers
 *
 * Copyright (C) 2002, 2003  Red Hat Inc.
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

#ifndef DBUS_H
#define DBUS_H

#define DBUS_INSIDE_DBUS_H 1

#include <dbus/dbus-arch-deps.h>
#include <dbus/dbus-address.h>
#include <dbus/dbus-bus.h>
#include <dbus/dbus-connection.h>
#include <dbus/dbus-errors.h>
#include <dbus/dbus-macros.h>
#include <dbus/dbus-message.h>
#include <dbus/dbus-misc.h>
#include <dbus/dbus-pending-call.h>
#include <dbus/dbus-protocol.h>
#include <dbus/dbus-server.h>
#include <dbus/dbus-shared.h>
#include <dbus/dbus-signature.h>
#include <dbus/dbus-syntax.h>
#include <dbus/dbus-threads.h>
#include <dbus/dbus-types.h>

#undef DBUS_INSIDE_DBUS_H

/**
 * @defgroup DBus D-Bus low-level public API
 * @brief The low-level public API of the D-Bus library
 *
 * libdbus provides a low-level C API intended primarily for use by
 * bindings to specific object systems and languages.  D-Bus is most
 * convenient when used with the GLib bindings, Python bindings, Qt
 * bindings, Mono bindings, and so forth.  This low-level API has a
 * lot of complexity useful only for bindings.
 * 
 * @{
 */

/** @} */

/**
 * @mainpage
 *
 * This manual documents the <em>low-level</em> D-Bus C API. <b>If you use
 * this low-level API directly, you're signing up for some pain.</b>
 *
 * Caveats aside, you might get started learning the low-level API by reading
 * about @ref DBusConnection and @ref DBusMessage.
 * 
 * There are several other places to look for D-Bus information, such
 * as the tutorial and the specification; those can be found at <a
 * href="http://www.freedesktop.org/wiki/Software/dbus">the D-Bus
 * website</a>. If you're interested in a sysadmin or package
 * maintainer's perspective on the dbus-daemon itself and its
 * configuration, be sure to check out the man pages as well.
 *
 * The low-level API documented in this manual deliberately lacks
 * most convenience functions - those are left up to higher-level libraries
 * based on frameworks such as GLib, Qt, Python, Mono, Java,
 * etc. These higher-level libraries (often called "D-Bus bindings")
 * have features such as object systems and main loops that allow a
 * <em>much</em> more convenient API.
 * 
 * The low-level API also contains plenty of clutter to support
 * integration with arbitrary object systems, languages, main loops,
 * and so forth. These features add a lot of noise to the API that you
 * probably don't care about unless you're coding a binding.
 *
 * This manual also contains docs for @ref DBusInternals "D-Bus internals",
 * so you can use it to get oriented to the D-Bus source code if you're
 * interested in patching the code. You should also read the
 * file CONTRIBUTING.md which comes with the source code if you plan to
 * contribute to D-Bus.
 *
 * As you read the code, you can identify internal D-Bus functions
 * because they start with an underscore ('_') character. Also, any
 * identifier or macro that lacks a DBus, dbus_, or DBUS_ namepace
 * prefix is internal, with a couple of exceptions such as #NULL,
 * #TRUE, and #FALSE.
 */

#endif /* DBUS_H */
