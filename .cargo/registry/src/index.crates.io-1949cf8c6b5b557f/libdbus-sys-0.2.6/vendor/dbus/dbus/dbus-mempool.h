/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-mempool.h Memory pools
 *
 * Copyright (C) 2002  Red Hat, Inc.
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

#ifndef DBUS_MEMPOOL_H
#define DBUS_MEMPOOL_H

#include <dbus/dbus-internals.h>
#include <dbus/dbus-memory.h>
#include <dbus/dbus-types.h>

DBUS_BEGIN_DECLS

typedef struct DBusMemPool DBusMemPool;

DBUS_PRIVATE_EXPORT
DBusMemPool* _dbus_mem_pool_new     (int          element_size,
                                     dbus_bool_t  zero_elements);
DBUS_PRIVATE_EXPORT
void         _dbus_mem_pool_free    (DBusMemPool *pool);
DBUS_PRIVATE_EXPORT
void*        _dbus_mem_pool_alloc   (DBusMemPool *pool);
DBUS_PRIVATE_EXPORT
dbus_bool_t  _dbus_mem_pool_dealloc (DBusMemPool *pool,
                                     void        *element);

/* if DBUS_ENABLE_STATS */
void         _dbus_mem_pool_get_stats (DBusMemPool   *pool,
                                       dbus_uint32_t *in_use_p,
                                       dbus_uint32_t *in_free_list_p,
                                       dbus_uint32_t *allocated_p);

DBUS_END_DECLS

#endif /* DBUS_MEMPOOL_H */
