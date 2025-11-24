/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* expirelist.c  List of items that expire
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

#include <config.h>
#include "expirelist.h"
#include "test.h"
#include <dbus/dbus-internals.h>
#include <dbus/dbus-mainloop.h>
#include <dbus/dbus-test-tap.h>
#include <dbus/dbus-timeout.h>

struct BusExpireList
{
  DBusList      *items; /**< List of BusExpireItem */
  DBusTimeout   *timeout;
  DBusLoop      *loop;
  BusExpireFunc  expire_func;
  void          *data;
  int            expire_after; /**< Expire after milliseconds (thousandths) */
};

static dbus_bool_t expire_timeout_handler (void *data);

BusExpireList*
bus_expire_list_new (DBusLoop      *loop,
                     int            expire_after,
                     BusExpireFunc  expire_func,
                     void          *data)
{
  BusExpireList *list;

  list = dbus_new0 (BusExpireList, 1);
  if (list == NULL)
    return NULL;

  list->expire_func = expire_func;
  list->data = data;
  list->loop = loop;
  list->expire_after = expire_after;

  list->timeout = _dbus_timeout_new (100, /* irrelevant */
                                     expire_timeout_handler,
                                     list, NULL);
  if (list->timeout == NULL)
    goto failed;

  _dbus_timeout_disable (list->timeout);

  if (!_dbus_loop_add_timeout (list->loop, list->timeout))
    goto failed;

  return list;

 failed:
  if (list->timeout)
    _dbus_timeout_unref (list->timeout);

  dbus_free (list);

  return NULL;
}

void
bus_expire_list_free (BusExpireList *list)
{
  _dbus_assert (list->items == NULL);

  _dbus_loop_remove_timeout (list->loop, list->timeout);

  _dbus_timeout_unref (list->timeout);

  dbus_free (list);
}

void
bus_expire_timeout_set_interval (DBusTimeout   *timeout,
                                 int            next_interval)
{
  if (next_interval >= 0)
    {
      _dbus_timeout_restart (timeout, next_interval);

      _dbus_verbose ("Enabled an expire timeout with interval %d\n",
                     next_interval);
    }
  else if (dbus_timeout_get_enabled (timeout))
    {
      _dbus_timeout_disable (timeout);

      _dbus_verbose ("Disabled an expire timeout\n");
    }
  else
    _dbus_verbose ("No need to disable this expire timeout\n");
}

void
bus_expire_list_recheck_immediately (BusExpireList *list)
{
  _dbus_verbose ("setting interval on expire list to 0 for immediate recheck\n");

  bus_expire_timeout_set_interval (list->timeout, 0);
}

static int
do_expiration_with_monotonic_time (BusExpireList *list,
                                   long           tv_sec,
                                   long           tv_usec)
{
  DBusList *link;
  int next_interval, min_wait_time, items_to_expire;

  next_interval = -1;
  min_wait_time = 3600 * 1000; /* this is reset anyway if used */
  items_to_expire = 0;
  
  link = _dbus_list_get_first_link (&list->items);
  while (link != NULL)
    {
      DBusList *next = _dbus_list_get_next_link (&list->items, link);
      double elapsed;
      BusExpireItem *item;

      item = link->data;

      elapsed = ELAPSED_MILLISECONDS_SINCE (item->added_tv_sec,
                                            item->added_tv_usec,
                                            tv_sec, tv_usec);

      if (((item->added_tv_sec == 0) && (item->added_tv_usec == 0)) ||
          ((list->expire_after > 0) && (elapsed >= (double) list->expire_after)))
        {
          _dbus_verbose ("Expiring an item %p\n", item);

          /* If the expire function fails, we just end up expiring
           * this item next time we walk through the list. This would
           * be an indeterminate time normally, so we set up the
           * next_interval to be "shortly" (just enough to avoid
           * a busy loop)
           */
          if (!(* list->expire_func) (list, link, list->data))
            {
              next_interval = _dbus_get_oom_wait ();
              break;
            }
        }
      else if (list->expire_after > 0)
        {
          double to_wait;

          items_to_expire = 1;
          to_wait = (double) list->expire_after - elapsed;
          if (min_wait_time > to_wait)
            min_wait_time = to_wait;
        }

      link = next;
    }

  if (next_interval < 0 && items_to_expire)
    next_interval = min_wait_time;

  return next_interval;
}

static void
bus_expirelist_expire (BusExpireList *list)
{
  int next_interval;

  next_interval = -1;

  if (list->items != NULL)
    {
      long tv_sec, tv_usec;

      _dbus_get_monotonic_time (&tv_sec, &tv_usec);

      next_interval = do_expiration_with_monotonic_time (list, tv_sec, tv_usec);
    }

  bus_expire_timeout_set_interval (list->timeout, next_interval);
}

static dbus_bool_t
expire_timeout_handler (void *data)
{
  BusExpireList *list = data;

  _dbus_verbose ("Running\n");

  /* note that this may remove the timeout */
  bus_expirelist_expire (list);

  return TRUE;
}

void
bus_expire_list_remove_link (BusExpireList *list,
                             DBusList      *link)
{
  _dbus_list_remove_link (&list->items, link);
}

dbus_bool_t
bus_expire_list_remove (BusExpireList *list,
                        BusExpireItem *item)
{
  return _dbus_list_remove (&list->items, item);
}

void
bus_expire_list_unlink (BusExpireList *list,
                        DBusList      *link)
{
  _dbus_list_unlink (&list->items, link);
}

dbus_bool_t
bus_expire_list_add (BusExpireList *list,
                     BusExpireItem *item)
{
  dbus_bool_t ret;

  ret = _dbus_list_prepend (&list->items, item);
  if (ret && !dbus_timeout_get_enabled (list->timeout))
    bus_expire_timeout_set_interval (list->timeout, 0);

  return ret;
}

void
bus_expire_list_add_link (BusExpireList *list,
                          DBusList      *link)
{
  _dbus_assert (link->data != NULL);
  
  _dbus_list_prepend_link (&list->items, link);

  if (!dbus_timeout_get_enabled (list->timeout))
    bus_expire_timeout_set_interval (list->timeout, 0);
}

DBusList*
bus_expire_list_get_first_link (BusExpireList *list)
{
  return _dbus_list_get_first_link (&list->items);
}

DBusList*
bus_expire_list_get_next_link (BusExpireList *list,
                               DBusList      *link)
{
  return _dbus_list_get_next_link (&list->items, link);
}

dbus_bool_t
bus_expire_list_contains_item (BusExpireList *list,
                               BusExpireItem *item)
{
  return _dbus_list_find_last (&list->items, item) != NULL;
}

#ifdef DBUS_ENABLE_EMBEDDED_TESTS

typedef struct
{
  BusExpireItem item;
  int expire_count;
} TestExpireItem;

static dbus_bool_t
test_expire_func (BusExpireList *list,
                  DBusList      *link,
                  void          *data)
{
  TestExpireItem *t;

  t = (TestExpireItem*) link->data;

  t->expire_count += 1;

  return TRUE;
}

static void
time_add_milliseconds (long *tv_sec,
                       long *tv_usec,
                       int   milliseconds)
{
  *tv_sec = *tv_sec + milliseconds / 1000;
  *tv_usec = *tv_usec + milliseconds * 1000;
  if (*tv_usec >= 1000000)
    {
      *tv_usec -= 1000000;
      *tv_sec += 1;
    }
}

dbus_bool_t
bus_expire_list_test (const char *test_data_dir _DBUS_GNUC_UNUSED)
{
  DBusLoop *loop;
  BusExpireList *list;
  long tv_sec, tv_usec;
  long tv_sec_not_expired, tv_usec_not_expired;
  long tv_sec_expired, tv_usec_expired;
  long tv_sec_past, tv_usec_past;
  TestExpireItem *item;
  int next_interval;
  dbus_bool_t result = FALSE;


  loop = _dbus_loop_new ();
  _dbus_assert (loop != NULL);

#define EXPIRE_AFTER 100
  
  list = bus_expire_list_new (loop, EXPIRE_AFTER,
                              test_expire_func, NULL);
  _dbus_assert (list != NULL);

  _dbus_get_monotonic_time (&tv_sec, &tv_usec);

  tv_sec_not_expired = tv_sec;
  tv_usec_not_expired = tv_usec;
  time_add_milliseconds (&tv_sec_not_expired,
                         &tv_usec_not_expired, EXPIRE_AFTER - 1);

  tv_sec_expired = tv_sec;
  tv_usec_expired = tv_usec;
  time_add_milliseconds (&tv_sec_expired,
                         &tv_usec_expired, EXPIRE_AFTER);
  

  tv_sec_past = tv_sec - 1;
  tv_usec_past = tv_usec;

  item = dbus_new0 (TestExpireItem, 1);

  if (item == NULL)
    goto oom;

  item->item.added_tv_sec = tv_sec;
  item->item.added_tv_usec = tv_usec;
  if (!bus_expire_list_add (list, &item->item))
    _dbus_test_fatal ("out of memory");

  next_interval =
    do_expiration_with_monotonic_time (list, tv_sec_not_expired,
                                       tv_usec_not_expired);
  _dbus_assert (item->expire_count == 0);
  _dbus_verbose ("next_interval = %d\n", next_interval);
  _dbus_assert (next_interval == 1);
  
  next_interval =
    do_expiration_with_monotonic_time (list, tv_sec_expired,
                                       tv_usec_expired);
  _dbus_assert (item->expire_count == 1);
  _dbus_verbose ("next_interval = %d\n", next_interval);
  _dbus_assert (next_interval == -1);

  next_interval =
    do_expiration_with_monotonic_time (list, tv_sec_past,
                                       tv_usec_past);
  _dbus_assert (item->expire_count == 1);
  _dbus_verbose ("next_interval = %d\n", next_interval);
  _dbus_assert (next_interval == 1000 + EXPIRE_AFTER);

  bus_expire_list_remove (list, &item->item);
  dbus_free (item);
  
  bus_expire_list_free (list);
  _dbus_loop_unref (loop);
  
  result = TRUE;

 oom:
  return result;
}

#endif /* DBUS_ENABLE_EMBEDDED_TESTS */
