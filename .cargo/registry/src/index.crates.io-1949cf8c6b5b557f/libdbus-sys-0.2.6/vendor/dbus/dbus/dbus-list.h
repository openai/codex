/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-list.h Generic linked list utility (internal to D-Bus implementation)
 *
 * Copyright (C) 2002, 2003 Red Hat, Inc.
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

#ifndef DBUS_LIST_H
#define DBUS_LIST_H

#include <dbus/dbus-internals.h>
#include <dbus/dbus-memory.h>
#include <dbus/dbus-types.h>
#include <dbus/dbus-sysdeps.h>

DBUS_BEGIN_DECLS

struct DBusList
{
  DBusList *prev; /**< Previous list node. */
  DBusList *next; /**< Next list node. */
  void     *data; /**< Data stored at this element. */
};
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_list_append             (DBusList **list,
                                           void      *data);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_list_prepend            (DBusList **list,
                                           void      *data);
dbus_bool_t _dbus_list_insert_before      (DBusList **list,
                                           DBusList  *before_this_link,
                                           void      *data);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_list_insert_after       (DBusList **list,
                                           DBusList  *after_this_link,
                                           void      *data);
DBUS_PRIVATE_EXPORT
void        _dbus_list_insert_before_link (DBusList **list,
                                           DBusList  *before_this_link,
                                           DBusList  *link);
DBUS_PRIVATE_EXPORT
void        _dbus_list_insert_after_link  (DBusList **list,
                                           DBusList  *after_this_link,
                                           DBusList  *link);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_list_remove             (DBusList **list,
                                           void      *data);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_list_remove_last        (DBusList **list,
                                           void      *data);
DBUS_PRIVATE_EXPORT
void        _dbus_list_remove_link        (DBusList **list,
                                           DBusList  *link);
DBUS_PRIVATE_EXPORT
DBusList*   _dbus_list_find_last          (DBusList **list,
                                           void      *data);
DBUS_PRIVATE_EXPORT
void        _dbus_list_clear              (DBusList **list);
DBUS_PRIVATE_EXPORT
void        _dbus_list_clear_full         (DBusList         **list,
                                           DBusFreeFunction   function);
DBUS_PRIVATE_EXPORT
DBusList*   _dbus_list_get_first_link     (DBusList **list);
DBUS_PRIVATE_EXPORT
DBusList*   _dbus_list_get_last_link      (DBusList **list);
DBUS_PRIVATE_EXPORT
void*       _dbus_list_get_last           (DBusList **list);
DBUS_PRIVATE_EXPORT
void*       _dbus_list_get_first          (DBusList **list);
DBUS_PRIVATE_EXPORT
void*       _dbus_list_pop_first          (DBusList **list);
DBUS_PRIVATE_EXPORT
void*       _dbus_list_pop_last           (DBusList **list);
DBUS_PRIVATE_EXPORT
DBusList*   _dbus_list_pop_first_link     (DBusList **list);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_list_copy               (DBusList **list,
                                           DBusList **dest);
DBUS_PRIVATE_EXPORT
int         _dbus_list_get_length         (DBusList **list);
DBUS_PRIVATE_EXPORT
DBusList*   _dbus_list_alloc_link         (void      *data);
DBUS_PRIVATE_EXPORT
void        _dbus_list_free_link          (DBusList  *link);
DBUS_PRIVATE_EXPORT
void        _dbus_list_unlink             (DBusList **list,
                                           DBusList  *link);
DBUS_PRIVATE_EXPORT
void        _dbus_list_append_link        (DBusList **list,
                                           DBusList  *link);
DBUS_PRIVATE_EXPORT
void        _dbus_list_prepend_link       (DBusList **list,
                                           DBusList  *link);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_list_length_is_one      (DBusList **list);


DBUS_PRIVATE_EXPORT
void _dbus_list_foreach (DBusList            **list,
                         DBusForeachFunction   function,
                         void                 *data);

#define _dbus_list_get_next_link(list, link) ((link)->next == *(list) ? NULL : (link)->next)
#define _dbus_list_get_prev_link(list, link) ((link) == *(list) ? NULL : (link)->prev)

/* if DBUS_ENABLE_STATS */
DBUS_PRIVATE_EXPORT
void        _dbus_list_get_stats          (dbus_uint32_t *in_use_p,
                                           dbus_uint32_t *in_free_list_p,
                                           dbus_uint32_t *allocated_p);

DBUS_END_DECLS

#endif /* DBUS_LIST_H */
