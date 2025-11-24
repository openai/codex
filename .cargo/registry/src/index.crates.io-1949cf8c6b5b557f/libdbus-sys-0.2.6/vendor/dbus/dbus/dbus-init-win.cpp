/*
 * dbus-init-win.cpp - once-per-process initialization
 *
 * Copyright © 2013 Intel Corporation
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

extern "C"
{
#include "dbus-sysdeps-win.h"
}

class DBusInternalInit
  {
    public:
      DBusInternalInit ()
        {
          _dbus_threads_windows_init_global ();
        }

      void must_not_be_omitted ()
        {
        }
  };

static DBusInternalInit init;

extern "C" void
_dbus_threads_windows_ensure_ctor_linked ()
{
  /* Do nothing significant, just ensure that the global initializer gets
   * linked in. */
  init.must_not_be_omitted ();
}
