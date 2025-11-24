/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-list.c Generic linked list utility (internal to D-Bus implementation)
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

#include <config.h>
#include "dbus-internals.h"
#include "dbus-list.h"
#include "dbus-mempool.h"
#include "dbus-threads-internal.h"
#include <dbus/dbus-test-tap.h>

/**
 * @defgroup DBusList Linked list
 * @ingroup  DBusInternals
 * @brief DBusList data structure
 *
 * Types and functions related to DBusList.
 */

/* Protected by _DBUS_LOCK (list) */
static DBusMemPool *list_pool;

/**
 * @defgroup DBusListInternals Linked list implementation details
 * @ingroup  DBusInternals
 * @brief DBusList implementation details
 *
 * The guts of DBusList.
 *
 * @{
 */

/* the mem pool is probably a speed hit, with the thread
 * lock, though it does still save memory - unknown.
 */
static DBusList*
alloc_link (void *data)
{
  DBusList *link;

  if (!_DBUS_LOCK (list))
    return FALSE;

  if (list_pool == NULL)
    {      
      list_pool = _dbus_mem_pool_new (sizeof (DBusList), TRUE);

      if (list_pool == NULL)
        {
          _DBUS_UNLOCK (list);
          return NULL;
        }

      link = _dbus_mem_pool_alloc (list_pool);
      if (link == NULL)
        {
          _dbus_mem_pool_free (list_pool);
          list_pool = NULL;
          _DBUS_UNLOCK (list);
          return NULL;
        }
    }
  else
    {
      link = _dbus_mem_pool_alloc (list_pool);
    }

  if (link)
    link->data = data;
  
  _DBUS_UNLOCK (list);

  return link;
}

static void
free_link (DBusList *link)
{  
  if (!_DBUS_LOCK (list))
    _dbus_assert_not_reached ("we should have initialized global locks "
        "before we allocated a linked-list link");

  if (_dbus_mem_pool_dealloc (list_pool, link))
    {
      _dbus_mem_pool_free (list_pool);
      list_pool = NULL;
    }
  
  _DBUS_UNLOCK (list);
}

static void
link_before (DBusList **list,
             DBusList  *before_this_link,
             DBusList  *link)
{
  if (*list == NULL)
    {
      link->prev = link;
      link->next = link;
      *list = link;
    }
  else
    {      
      link->next = before_this_link;
      link->prev = before_this_link->prev;
      before_this_link->prev = link;
      link->prev->next = link;
      
      if (before_this_link == *list)
        *list = link;
    }
}

static void
link_after (DBusList **list,
            DBusList  *after_this_link,
            DBusList  *link)
{
  if (*list == NULL)
    {
      link->prev = link;
      link->next = link;
      *list = link;
    }
  else
    {
      link->prev = after_this_link;
      link->next = after_this_link->next;
      after_this_link->next = link;
      link->next->prev = link;
    }
}

#ifdef DBUS_ENABLE_STATS
void
_dbus_list_get_stats     (dbus_uint32_t *in_use_p,
                          dbus_uint32_t *in_free_list_p,
                          dbus_uint32_t *allocated_p)
{
  if (!_DBUS_LOCK (list))
    {
      *in_use_p = 0;
      *in_free_list_p = 0;
      *allocated_p = 0;
      return;
    }

  _dbus_mem_pool_get_stats (list_pool, in_use_p, in_free_list_p, allocated_p);
  _DBUS_UNLOCK (list);
}
#endif

/** @} */

/**
 * @addtogroup DBusList
 * @{
 */

/**
 * @struct DBusList
 *
 * A node in a linked list.
 *
 * DBusList is a circular list; that is, the tail of the list
 * points back to the head of the list. The empty list is
 * represented by a #NULL pointer.
 */

/**
 * @def _dbus_list_get_next_link
 *
 * Gets the next link in the list, or #NULL if
 * there are no more links. Used for iteration.
 *
 * @code
 * DBusList *link;
 * link = _dbus_list_get_first_link (&list);
 * while (link != NULL)
 *   {
 *     printf ("value is %p\n", link->data);
 *     link = _dbus_list_get_next_link (&link);
 *   }
 * @endcode
 *
 * @param list address of the list head.
 * @param link current link.
 * @returns the next link, or %NULL if none.
 * 
 */

/**
 * @def _dbus_list_get_prev_link
 *
 * Gets the previous link in the list, or #NULL if
 * there are no more links. Used for iteration.
 *
 * @code
 * DBusList *link;
 * link = _dbus_list_get_last_link (&list);
 * while (link != NULL)
 *   {
 *     printf ("value is %p\n", link->data);
 *     link = _dbus_list_get_prev_link (&link);
 *   }
 * @endcode
 *
 * @param list address of the list head.
 * @param link current link.
 * @returns the previous link, or %NULL if none.
 * 
 */

/**
 * Allocates a linked list node. Useful for preallocating
 * nodes and using _dbus_list_append_link() to avoid
 * allocations.
 * 
 * @param data the value to store in the link.
 * @returns a newly allocated link.
 */
DBusList*
_dbus_list_alloc_link (void *data)
{
  return alloc_link (data);
}

/**
 * Frees a linked list node allocated with _dbus_list_alloc_link.
 * Does not free the data in the node.
 *
 * @param link the list node
 */
void
_dbus_list_free_link (DBusList *link)
{
  free_link (link);
}


/**
 * Appends a value to the list. May return #FALSE
 * if insufficient memory exists to add a list link.
 * This is a constant-time operation.
 *
 * @param list address of the list head.
 * @param data the value to append.
 * @returns #TRUE on success.
 */
dbus_bool_t
_dbus_list_append (DBusList **list,
                   void      *data)
{
  if (!_dbus_list_prepend (list, data))
    return FALSE;

  /* Now cycle the list forward one so the prepended node is the tail */
  *list = (*list)->next;

  return TRUE;
}

/**
 * Prepends a value to the list. May return #FALSE
 * if insufficient memory exists to add a list link.
 * This is a constant-time operation.
 *
 * @param list address of the list head.
 * @param data the value to prepend.
 * @returns #TRUE on success.
 */
dbus_bool_t
_dbus_list_prepend (DBusList **list,
                    void      *data)
{
  DBusList *link;

  link = alloc_link (data);
  if (link == NULL)
    return FALSE;

  link_before (list, *list, link);

  return TRUE;
}

/**
 * Appends a link to the list.
 * Cannot fail due to out of memory.
 * This is a constant-time operation.
 *
 * @param list address of the list head.
 * @param link the link to append.
 */
void
_dbus_list_append_link (DBusList **list,
			DBusList *link)
{
  _dbus_list_prepend_link (list, link);

  /* Now cycle the list forward one so the prepended node is the tail */
  *list = (*list)->next;
}

/**
 * Prepends a link to the list. 
 * Cannot fail due to out of memory.
 * This is a constant-time operation.
 *
 * @param list address of the list head.
 * @param link the link to prepend.
 */
void
_dbus_list_prepend_link (DBusList **list,
			 DBusList *link)
{
  link_before (list, *list, link);
}

/**
 * Inserts data into the list after the given existing link.
 * 
 * @param list the list to modify
 * @param after_this_link existing link to insert after, or #NULL to prepend
 * @param data the value to insert
 * @returns #TRUE on success, #FALSE if memory allocation fails
 */
dbus_bool_t
_dbus_list_insert_after (DBusList **list,
                         DBusList  *after_this_link,
                         void      *data)
{
  DBusList *link;  

  if (after_this_link == NULL)
    return _dbus_list_prepend (list, data);
  else
    {
      link = alloc_link (data);
      if (link == NULL)
        return FALSE;
  
      link_after (list, after_this_link, link);
    }
  
  return TRUE;
}

/**
 * Inserts a link into the list before the given existing link.
 * 
 * @param list the list to modify
 * @param before_this_link existing link to insert before, or #NULL to append
 * @param link the link to insert
 */
void
_dbus_list_insert_before_link (DBusList **list,
                               DBusList  *before_this_link,
                               DBusList  *link)
{
  if (before_this_link == NULL)
    _dbus_list_append_link (list, link);
  else
    link_before (list, before_this_link, link);
}

/**
 * Inserts a link into the list after the given existing link.
 * 
 * @param list the list to modify
 * @param after_this_link existing link to insert after, or #NULL to prepend
 * @param link the link to insert
 */
void
_dbus_list_insert_after_link (DBusList **list,
                              DBusList  *after_this_link,
                              DBusList  *link)
{
  if (after_this_link == NULL)
    _dbus_list_prepend_link (list, link);
  else  
    link_after (list, after_this_link, link);
}

/**
 * Removes a value from the list. Only removes the
 * first value equal to the given data pointer,
 * even if multiple values exist which match.
 * This is a linear-time operation.
 *
 * @param list address of the list head.
 * @param data the value to remove.
 * @returns #TRUE if a value was found to remove.
 */
dbus_bool_t
_dbus_list_remove (DBusList **list,
                   void      *data)
{
  DBusList *link;

  link = *list;
  while (link != NULL)
    {
      if (link->data == data)
        {
          _dbus_list_remove_link (list, link);
          return TRUE;
        }
      
      link = _dbus_list_get_next_link (list, link);
    }

  return FALSE;
}

/**
 * Removes a value from the list. Only removes the
 * last value equal to the given data pointer,
 * even if multiple values exist which match.
 * This is a linear-time operation.
 *
 * @param list address of the list head.
 * @param data the value to remove.
 * @returns #TRUE if a value was found to remove.
 */
dbus_bool_t
_dbus_list_remove_last (DBusList **list,
                        void      *data)
{
  DBusList *link;

  link = _dbus_list_find_last (list, data);
  if (link)
    {
      _dbus_list_remove_link (list, link);
      return TRUE;
    }
  else
    return FALSE;
}

/**
 * Finds a value in the list. Returns the last link
 * with value equal to the given data pointer.
 * This is a linear-time operation.
 * Returns #NULL if no value found that matches.
 *
 * @param list address of the list head.
 * @param data the value to find.
 * @returns the link if found
 */
DBusList*
_dbus_list_find_last (DBusList **list,
                      void      *data)
{
  DBusList *link;

  link = _dbus_list_get_last_link (list);

  while (link != NULL)
    {
      if (link->data == data)
        return link;
      
      link = _dbus_list_get_prev_link (list, link);
    }

  return NULL;
}

/**
 * Removes the given link from the list, but doesn't
 * free it. _dbus_list_remove_link() both removes the
 * link and also frees it.
 *
 * @param list the list
 * @param link the link in the list
 */
void
_dbus_list_unlink (DBusList **list,
                   DBusList  *link)
{
  if (link->next == link)
    {
      /* one-element list */
      *list = NULL;
    }
  else
    {      
      link->prev->next = link->next;
      link->next->prev = link->prev;
      
      if (*list == link)
        *list = link->next;
    }

  link->next = NULL;
  link->prev = NULL;
}

/**
 * Removes a link from the list. This is a constant-time operation.
 *
 * @param list address of the list head.
 * @param link the list link to remove.
 */
void
_dbus_list_remove_link (DBusList **list,
                        DBusList  *link)
{
  _dbus_list_unlink (list, link);
  free_link (link);
}

/**
 * Frees all links in the list and sets the list head to #NULL. Does
 * not free the data in each link, for obvious reasons. This is a
 * linear-time operation.
 *
 * @param list address of the list head.
 */
void
_dbus_list_clear (DBusList **list)
{
  DBusList *link;

  link = *list;
  while (link != NULL)
    {
      DBusList *next = _dbus_list_get_next_link (list, link);
      
      free_link (link);
      
      link = next;
    }

  *list = NULL;
}

/**
 * Free every link and every element in the list.
 *
 * @param list address of the head of the list.
 * @param function free-function to call for each element.
 *
 */
void
_dbus_list_clear_full (DBusList         **list,
                       DBusFreeFunction   function)
{
  DBusList *link;

  link = *list;
  while (link != NULL)
    {
      DBusList *next = _dbus_list_get_next_link (list, link);

      function (link->data);
      free_link (link);

      link = next;
    }

  *list = NULL;
}

/**
 * Gets the first link in the list.
 * This is a constant-time operation.
 *
 * @param list address of the list head.
 * @returns the first link, or #NULL for an empty list.
 */
DBusList*
_dbus_list_get_first_link (DBusList **list)
{
  return *list;
}

/**
 * Gets the last link in the list.
 * This is a constant-time operation.
 *
 * @param list address of the list head.
 * @returns the last link, or #NULL for an empty list.
 */
DBusList*
_dbus_list_get_last_link (DBusList **list)
{
  if (*list == NULL)
    return NULL;
  else
    return (*list)->prev;
}

/**
 * Gets the last data in the list.
 * This is a constant-time operation.
 *
 * @param list address of the list head.
 * @returns the last data in the list, or #NULL for an empty list.
 */
void*
_dbus_list_get_last (DBusList **list)
{
  if (*list == NULL)
    return NULL;
  else
    return (*list)->prev->data;
}

/**
 * Gets the first data in the list.
 * This is a constant-time operation.
 *
 * @param list address of the list head.
 * @returns the first data in the list, or #NULL for an empty list.
 */
void*
_dbus_list_get_first (DBusList **list)
{
  if (*list == NULL)
    return NULL;
  else
    return (*list)->data;
}

/**
 * Removes the first link in the list and returns it.  This is a
 * constant-time operation.
 *
 * @param list address of the list head.
 * @returns the first link in the list, or #NULL for an empty list.
 */
DBusList*
_dbus_list_pop_first_link (DBusList **list)
{
  DBusList *link;
  
  link = _dbus_list_get_first_link (list);
  if (link == NULL)
    return NULL;

  _dbus_list_unlink (list, link);

  return link;
}

/**
 * Removes the first value in the list and returns it.  This is a
 * constant-time operation.
 *
 * @param list address of the list head.
 * @returns the first data in the list, or #NULL for an empty list.
 */
void*
_dbus_list_pop_first (DBusList **list)
{
  DBusList *link;
  void *data;
  
  link = _dbus_list_get_first_link (list);
  if (link == NULL)
    return NULL;
  
  data = link->data;
  _dbus_list_remove_link (list, link);

  return data;
}

/**
 * Removes the last value in the list and returns it.  This is a
 * constant-time operation.
 *
 * @param list address of the list head.
 * @returns the last data in the list, or #NULL for an empty list.
 */
void*
_dbus_list_pop_last (DBusList **list)
{
  DBusList *link;
  void *data;
  
  link = _dbus_list_get_last_link (list);
  if (link == NULL)
    return NULL;
  
  data = link->data;
  _dbus_list_remove_link (list, link);

  return data;
}

/**
 * Copies a list. This is a linear-time operation.  If there isn't
 * enough memory to copy the entire list, the destination list will be
 * set to #NULL.
 *
 * @param list address of the head of the list to copy.
 * @param dest address where the copied list should be placed.
 * @returns #TRUE on success, #FALSE if not enough memory.
 */
dbus_bool_t
_dbus_list_copy (DBusList **list,
                 DBusList **dest)
{
  DBusList *link;

  _dbus_assert (list != dest);

  *dest = NULL;
  
  link = *list;
  while (link != NULL)
    {
      if (!_dbus_list_append (dest, link->data))
        {
          /* free what we have so far */
          _dbus_list_clear (dest);
          return FALSE;
        }
      
      link = _dbus_list_get_next_link (list, link);
    }

  return TRUE;
}

/**
 * Gets the length of a list. This is a linear-time
 * operation.
 *
 * @param list address of the head of the list
 * @returns number of elements in the list.
 */
int
_dbus_list_get_length (DBusList **list)
{
  DBusList *link;
  int length;

  length = 0;
  
  link = *list;
  while (link != NULL)
    {
      ++length;
      
      link = _dbus_list_get_next_link (list, link);
    }

  return length;
}

/**
 * Calls the given function for each element in the list.  The
 * function is passed the list element as its first argument, and the
 * given data as its second argument.
 *
 * @param list address of the head of the list.
 * @param function function to call for each element.
 * @param data extra data for the function.
 * 
 */
void
_dbus_list_foreach (DBusList          **list,
                    DBusForeachFunction function,
                    void               *data)
{
  DBusList *link;

  link = *list;
  while (link != NULL)
    {
      DBusList *next = _dbus_list_get_next_link (list, link);
      
      (* function) (link->data, data);
      
      link = next;
    }
}

/**
 * Check whether length is exactly one.
 *
 * @param list the list
 * @returns #TRUE if length is exactly one
 */
dbus_bool_t
_dbus_list_length_is_one (DBusList **list)
{
  return (*list != NULL &&
          (*list)->next == *list);
}

/** @} */
