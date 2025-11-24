/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* expirelist.h  List of stuff that expires
 *
 * Copyright (C) 2003  Red Hat, Inc.
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

#ifndef BUS_EXPIRE_LIST_H
#define BUS_EXPIRE_LIST_H

#include <dbus/dbus.h>
#include <dbus/dbus-list.h>
#include <dbus/dbus-mainloop.h>

typedef struct BusExpireList BusExpireList;
typedef struct BusExpireItem BusExpireItem;

typedef dbus_bool_t (* BusExpireFunc) (BusExpireList *list,
                                       DBusList      *link,
                                       void          *data);


/* embed this in a child expire item struct */
struct BusExpireItem
{
  long added_tv_sec;  /**< Time we were added (seconds component) */
  long added_tv_usec; /**< Time we were added (microsec component) */
};

BusExpireList* bus_expire_list_new                 (DBusLoop      *loop,
                                                    int            expire_after,
                                                    BusExpireFunc  expire_func,
                                                    void          *data);
void           bus_expire_list_free                (BusExpireList *list);
void           bus_expire_list_recheck_immediately (BusExpireList *list);
void           bus_expire_list_remove_link         (BusExpireList *list,
                                                    DBusList      *link);
dbus_bool_t    bus_expire_list_remove              (BusExpireList *list,
                                                    BusExpireItem *item);
DBusList*      bus_expire_list_get_first_link      (BusExpireList *list);
DBusList*      bus_expire_list_get_next_link       (BusExpireList *list,
                                                    DBusList      *link);
dbus_bool_t    bus_expire_list_add                 (BusExpireList *list,
                                                    BusExpireItem *item);
void           bus_expire_list_add_link            (BusExpireList *list,
                                                    DBusList      *link);
dbus_bool_t    bus_expire_list_contains_item       (BusExpireList *list,
                                                    BusExpireItem *item);
void           bus_expire_list_unlink              (BusExpireList *list,
                                                    DBusList      *link);

/* this macro and function are semi-related utility functions, not really part of the
 * BusExpireList API
 */

#define ELAPSED_MILLISECONDS_SINCE(orig_tv_sec, orig_tv_usec,   \
                                   now_tv_sec, now_tv_usec)     \
 (((double) (now_tv_sec) - (double) (orig_tv_sec)) * 1000.0 +   \
 ((double) (now_tv_usec) - (double) (orig_tv_usec)) / 1000.0)

void bus_expire_timeout_set_interval (DBusTimeout   *timeout,
                                      int            next_interval);

#endif /* BUS_EXPIRE_LIST_H */
