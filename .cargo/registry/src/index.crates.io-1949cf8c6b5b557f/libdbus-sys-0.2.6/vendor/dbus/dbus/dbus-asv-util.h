/* dbus-asv-util.h - utility functions for a{sv}
 *
 * Copyright © 2011-2012 Nokia Corporation
 * Copyright © 2012-2013 Collabora Ltd.
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
 * 02110-1301 USA
 */

#ifndef DBUS_ASV_UTIL_H
#define DBUS_ASV_UTIL_H

#include <dbus/dbus-internals.h>

DBUS_BEGIN_DECLS

DBusMessage *_dbus_asv_new_method_return (DBusMessage      *message,
                                          DBusMessageIter  *iter,
                                          DBusMessageIter  *arr_iter);
dbus_bool_t  _dbus_asv_close             (DBusMessageIter *iter,
                                          DBusMessageIter *arr_iter);
void         _dbus_asv_abandon           (DBusMessageIter *iter,
                                          DBusMessageIter *arr_iter);

dbus_bool_t  _dbus_asv_add_uint32        (DBusMessageIter *arr_iter,
                                          const char      *key,
                                          dbus_uint32_t    value);
dbus_bool_t  _dbus_asv_add_string        (DBusMessageIter *arr_iter,
                                          const char      *key,
                                          const char      *value);
dbus_bool_t  _dbus_asv_add_object_path   (DBusMessageIter *arr_iter,
                                          const char      *key,
                                          const char      *value);
dbus_bool_t  _dbus_asv_add_fixed_array   (DBusMessageIter *arr_iter,
                                          const char      *key,
                                          char             element_type,
                                          const void      *value,
                                          int              n_elements);
dbus_bool_t  _dbus_asv_add_byte_array    (DBusMessageIter *arr_iter,
                                          const char      *key,
                                          const void      *value,
                                          int              n_elements);
dbus_bool_t  _dbus_asv_open_entry        (DBusMessageIter *arr_iter,
                                          DBusMessageIter *entry_iter,
                                          const char      *key,
                                          const char      *type,
                                          DBusMessageIter *var_iter);
dbus_bool_t  _dbus_asv_close_entry       (DBusMessageIter *arr_iter,
                                          DBusMessageIter *entry_iter,
                                          DBusMessageIter *var_iter);
void         _dbus_asv_abandon_entry     (DBusMessageIter *arr_iter,
                                          DBusMessageIter *entry_iter,
                                          DBusMessageIter *var_iter);

#endif
