/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-valgrind-internal.h - valgrind glue
 *
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
 * Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA
 * 02110-1301  USA
 *
 */

#ifndef DBUS_VALGRIND_INTERNAL_H
#define DBUS_VALGRIND_INTERNAL_H

#include "config.h"
#include "dbus-internals.h"

#ifdef WITH_VALGRIND
#   include <memcheck.h>
#   include <valgrind.h>
#else
#   define VALGRIND_CREATE_MEMPOOL(_1, _2, _3) do { } while (0)
#   define VALGRIND_DESTROY_MEMPOOL(_1) do { } while (0)
#   define VALGRIND_MEMPOOL_ALLOC(_1, _2, _3) do { } while (0)
#   define VALGRIND_MEMPOOL_FREE(_1, _2) do { } while (0)

/* Recent gcc will warn if you have a statement that's just a macro
 * expanding to (0), but not if you have an inline stub function that
 * always returns 0, so let's do the latter. */
static inline int
VALGRIND_MAKE_MEM_UNDEFINED (void   *addr,
                             size_t  len)
{
  return 0;
}

static inline int
VALGRIND_PRINTF (const char *format,
                 ...)
{
  return 0;
}

static inline int
VALGRIND_PRINTF_BACKTRACE (const char *format,
                           ...)
{
  return 0;
}

#   define RUNNING_ON_VALGRIND 0
#endif /* WITH_VALGRIND */

#endif /* header guard */
