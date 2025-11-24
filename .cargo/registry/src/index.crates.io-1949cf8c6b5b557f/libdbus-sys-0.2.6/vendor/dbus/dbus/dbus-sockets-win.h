/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-sockets.h Wrappers around socket features (internal to D-BUS implementation)
 *
 * Copyright (C) 2005 Novell, Inc.
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

#ifndef DBUS_SOCKETS_H
#define DBUS_SOCKETS_H

#if defined(DBUS_WIN) || defined(DBUS_WINCE)



#ifndef STRICT
#define STRICT
#include <winsock2.h>
#undef STRICT
#endif
#include <winsock2.h>

#undef interface

#if HAVE_ERRNO_H
#include <errno.h>
#endif

#define DBUS_SOCKET_API_RETURNS_ERROR(n) ((n) == SOCKET_ERROR)
#define DBUS_SOCKET_SET_ERRNO() (_dbus_win_set_errno (WSAGetLastError()))

#else

#error "dbus-sockets-win.h should not be included on non-Windows"

#endif /* !Win32 */

#endif /* DBUS_SOCKETS_H */
