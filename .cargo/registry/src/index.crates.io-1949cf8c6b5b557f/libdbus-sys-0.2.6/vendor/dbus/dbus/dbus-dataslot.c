/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-dataslot.c  storing data on objects
 *
 * Copyright (C) 2003 Red Hat, Inc.
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
#include "dbus-dataslot.h"
#include "dbus-threads-internal.h"
#include <dbus/dbus-test-tap.h>

/**
 * @defgroup DBusDataSlot Data slots
 * @ingroup  DBusInternals
 * @brief Storing data by ID
 *
 * Types and functions related to storing data by an
 * allocated ID. This is used for dbus_connection_set_data(),
 * dbus_server_set_data(), etc. 
 * @{
 */

/**
 * Initializes a data slot allocator object, used to assign
 * integer IDs for data slots.
 *
 * @param allocator the allocator to initialize
 */
dbus_bool_t
_dbus_data_slot_allocator_init (DBusDataSlotAllocator *allocator,
                                DBusGlobalLock         lock)
{
  allocator->allocated_slots = NULL;
  allocator->n_allocated_slots = 0;
  allocator->n_used_slots = 0;
  allocator->lock = lock;

  return TRUE;
}

/**
 * Allocates an integer ID to be used for storing data
 * in a #DBusDataSlotList. If the value at *slot_id_p is
 * not -1, this function just increments the refcount for
 * the existing slot ID. If the value is -1, a new slot ID
 * is allocated and stored at *slot_id_p.
 * 
 * @param allocator the allocator
 * @param slot_id_p address to fill with the slot ID
 * @returns #TRUE on success
 */
dbus_bool_t
_dbus_data_slot_allocator_alloc (DBusDataSlotAllocator *allocator,
                                 dbus_int32_t          *slot_id_p)
{
  dbus_int32_t slot;

  if (!_dbus_lock (allocator->lock))
    return FALSE;

  if (*slot_id_p >= 0)
    {
      slot = *slot_id_p;
      
      _dbus_assert (slot < allocator->n_allocated_slots);
      _dbus_assert (allocator->allocated_slots[slot].slot_id == slot);
      
      allocator->allocated_slots[slot].refcount += 1;

      goto out;
    }

  _dbus_assert (*slot_id_p < 0);
  
  if (allocator->n_used_slots < allocator->n_allocated_slots)
    {
      slot = 0;
      while (slot < allocator->n_allocated_slots)
        {
          if (allocator->allocated_slots[slot].slot_id < 0)
            {
              allocator->allocated_slots[slot].slot_id = slot;
              allocator->allocated_slots[slot].refcount = 1;
              allocator->n_used_slots += 1;
              break;
            }
          ++slot;
        }

      _dbus_assert (slot < allocator->n_allocated_slots);
    }
  else
    {
      DBusAllocatedSlot *tmp;
      
      slot = -1;
      tmp = dbus_realloc (allocator->allocated_slots,
                          sizeof (DBusAllocatedSlot) * (allocator->n_allocated_slots + 1));
      if (tmp == NULL)
        goto out;

      allocator->allocated_slots = tmp;
      slot = allocator->n_allocated_slots;
      allocator->n_allocated_slots += 1;
      allocator->n_used_slots += 1;
      allocator->allocated_slots[slot].slot_id = slot;
      allocator->allocated_slots[slot].refcount = 1;
    }

  _dbus_assert (slot >= 0);
  _dbus_assert (slot < allocator->n_allocated_slots);
  _dbus_assert (*slot_id_p < 0);
  _dbus_assert (allocator->allocated_slots[slot].slot_id == slot);
  _dbus_assert (allocator->allocated_slots[slot].refcount == 1);
  
  *slot_id_p = slot;
  
  _dbus_verbose ("Allocated slot %d on allocator %p total %d slots allocated %d used\n",
                 slot, allocator, allocator->n_allocated_slots, allocator->n_used_slots);
  
 out:
  _dbus_unlock (allocator->lock);
  return slot >= 0;
}

/**
 * Deallocates an ID previously allocated with
 * _dbus_data_slot_allocator_alloc().  Existing data stored on
 * existing #DBusDataSlotList objects with this ID will be freed when the
 * data list is finalized, but may not be retrieved (and may only be
 * replaced if someone else reallocates the slot).
 * The slot value is reset to -1 if this is the last unref.
 *
 * @param allocator the allocator
 * @param slot_id_p address where we store the slot
 */
void
_dbus_data_slot_allocator_free (DBusDataSlotAllocator *allocator,
                                dbus_int32_t          *slot_id_p)
{
  if (!_dbus_lock (allocator->lock))
    _dbus_assert_not_reached ("we should have initialized global locks "
        "before we allocated this slot");

  _dbus_assert (*slot_id_p < allocator->n_allocated_slots);
  _dbus_assert (allocator->allocated_slots[*slot_id_p].slot_id == *slot_id_p);
  _dbus_assert (allocator->allocated_slots[*slot_id_p].refcount > 0);

  allocator->allocated_slots[*slot_id_p].refcount -= 1;

  if (allocator->allocated_slots[*slot_id_p].refcount > 0)
    {
      _dbus_unlock (allocator->lock);
      return;
    }

  /* refcount is 0, free the slot */
  _dbus_verbose ("Freeing slot %d on allocator %p total %d allocated %d used\n",
                 *slot_id_p, allocator, allocator->n_allocated_slots, allocator->n_used_slots);
  
  allocator->allocated_slots[*slot_id_p].slot_id = -1;
  *slot_id_p = -1;
  
  allocator->n_used_slots -= 1;
  
  if (allocator->n_used_slots == 0)
    {
      dbus_free (allocator->allocated_slots);
      allocator->allocated_slots = NULL;
      allocator->n_allocated_slots = 0;
    }

  _dbus_unlock (allocator->lock);
}

/**
 * Initializes a slot list.
 * @param list the list to initialize.
 */
void
_dbus_data_slot_list_init (DBusDataSlotList *list)
{
  list->slots = NULL;
  list->n_slots = 0;
}

/**
 * Stores a pointer in the data slot list, along with an optional
 * function to be used for freeing the data when the data is set
 * again, or when the slot list is finalized. The slot number must
 * have been allocated with _dbus_data_slot_allocator_alloc() for the
 * same allocator passed in here. The same allocator has to be used
 * with the slot list every time.
 *
 * @param allocator the allocator to use
 * @param list the data slot list
 * @param slot the slot number
 * @param data the data to store
 * @param free_data_func finalizer function for the data
 * @param old_free_func free function for any previously-existing data
 * @param old_data previously-existing data, should be freed with old_free_func
 * @returns #TRUE if there was enough memory to store the data
 */
dbus_bool_t
_dbus_data_slot_list_set  (DBusDataSlotAllocator *allocator,
                           DBusDataSlotList      *list,
                           int                    slot,
                           void                  *data,
                           DBusFreeFunction       free_data_func,
                           DBusFreeFunction      *old_free_func,
                           void                 **old_data)
{
#ifndef DBUS_DISABLE_ASSERT
  /* We need to take the allocator lock here, because the allocator could
   * be e.g. realloc()ing allocated_slots. We avoid doing this if asserts
   * are disabled, since then the asserts are empty.
   */
  if (!_dbus_lock (allocator->lock))
    _dbus_assert_not_reached ("we should have initialized global locks "
        "before we allocated this slot");

  _dbus_assert (slot < allocator->n_allocated_slots);
  _dbus_assert (allocator->allocated_slots[slot].slot_id == slot);
  _dbus_unlock (allocator->lock);
#endif
  
  if (slot >= list->n_slots)
    {
      DBusDataSlot *tmp;
      int i;
      
      tmp = dbus_realloc (list->slots,
                          sizeof (DBusDataSlot) * (slot + 1));
      if (tmp == NULL)
        return FALSE;
      
      list->slots = tmp;
      i = list->n_slots;
      list->n_slots = slot + 1;
      while (i < list->n_slots)
        {
          list->slots[i].data = NULL;
          list->slots[i].free_data_func = NULL;
          ++i;
        }
    }

  _dbus_assert (slot < list->n_slots);

  *old_data = list->slots[slot].data;
  *old_free_func = list->slots[slot].free_data_func;

  list->slots[slot].data = data;
  list->slots[slot].free_data_func = free_data_func;

  return TRUE;
}

/**
 * Retrieves data previously set with _dbus_data_slot_list_set_data().
 * The slot must still be allocated (must not have been freed).
 *
 * @param allocator the allocator slot was allocated from
 * @param list the data slot list
 * @param slot the slot to get data from
 * @returns the data, or #NULL if not found
 */
void*
_dbus_data_slot_list_get  (DBusDataSlotAllocator *allocator,
                           DBusDataSlotList      *list,
                           int                    slot)
{
#ifndef DBUS_DISABLE_ASSERT
  /* We need to take the allocator lock here, because the allocator could
   * be e.g. realloc()ing allocated_slots. We avoid doing this if asserts
   * are disabled, since then the asserts are empty.
   */
  if (!_dbus_lock (allocator->lock))
    _dbus_assert_not_reached ("we should have initialized global locks "
        "before we allocated this slot");

  _dbus_assert (slot >= 0);
  _dbus_assert (slot < allocator->n_allocated_slots);
  _dbus_assert (allocator->allocated_slots[slot].slot_id == slot);
  _dbus_unlock (allocator->lock);
#endif

  if (slot >= list->n_slots)
    return NULL;
  else
    return list->slots[slot].data;
}

/**
 * Frees all data slots contained in the list, calling
 * application-provided free functions if they exist.
 *
 * @param list the list to clear
 */
void
_dbus_data_slot_list_clear (DBusDataSlotList *list)
{
  int i;

  i = 0;
  while (i < list->n_slots)
    {
      if (list->slots[i].free_data_func)
        (* list->slots[i].free_data_func) (list->slots[i].data);
      list->slots[i].data = NULL;
      list->slots[i].free_data_func = NULL;
      ++i;
    }
}

/**
 * Frees the data slot list and all data slots contained
 * in it, calling application-provided free functions
 * if they exist.
 *
 * @param list the list to free
 */
void
_dbus_data_slot_list_free (DBusDataSlotList *list)
{
  _dbus_data_slot_list_clear (list);
  
  dbus_free (list->slots);
  list->slots = NULL;
  list->n_slots = 0;
}

/** @} */

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
#include "dbus-test.h"
#include <stdio.h>

/* Test-only, does not need to be thread-safe */
static int free_counter;

static void
test_free_slot_data_func (void *data)
{
  int i = _DBUS_POINTER_TO_INT (data);

  _dbus_assert (free_counter == i);
  ++free_counter;
}

/**
 * Test function for data slots
 */
dbus_bool_t
_dbus_data_slot_test (const char *test_data_dir _DBUS_GNUC_UNUSED)
{
  DBusDataSlotAllocator allocator;
  DBusDataSlotList list;
  int i;
  DBusFreeFunction old_free_func;
  void *old_data;

  if (!_dbus_data_slot_allocator_init (&allocator, _DBUS_LOCK_server_slots))
    _dbus_test_fatal ("no memory for allocator");

  _dbus_data_slot_list_init (&list);

#define N_SLOTS 100

  i = 0;
  while (i < N_SLOTS)
    {
      /* we don't really want apps to rely on this ordered
       * allocation, but it simplifies things to rely on it
       * here.
       */
      dbus_int32_t tmp = -1;

      _dbus_data_slot_allocator_alloc (&allocator, &tmp);

      if (tmp != i)
        _dbus_test_fatal ("did not allocate slots in numeric order");

      ++i;
    }

  i = 0;
  while (i < N_SLOTS)
    {
      if (!_dbus_data_slot_list_set (&allocator, &list,
                                     i,
                                     _DBUS_INT_TO_POINTER (i), 
                                     test_free_slot_data_func,
                                     &old_free_func, &old_data))
        _dbus_test_fatal ("no memory to set data");

      _dbus_assert (old_free_func == NULL);
      _dbus_assert (old_data == NULL);

      _dbus_assert (_dbus_data_slot_list_get (&allocator, &list, i) ==
                    _DBUS_INT_TO_POINTER (i));
      
      ++i;
    }

  free_counter = 0;
  i = 0;
  while (i < N_SLOTS)
    {
      if (!_dbus_data_slot_list_set (&allocator, &list,
                                     i,
                                     _DBUS_INT_TO_POINTER (i), 
                                     test_free_slot_data_func,
                                     &old_free_func, &old_data))
        _dbus_test_fatal ("no memory to set data");

      _dbus_assert (old_free_func == test_free_slot_data_func);
      _dbus_assert (_DBUS_POINTER_TO_INT (old_data) == i);

      (* old_free_func) (old_data);
      _dbus_assert (i == (free_counter - 1));

      _dbus_assert (_dbus_data_slot_list_get (&allocator, &list, i) ==
                    _DBUS_INT_TO_POINTER (i));
      
      ++i;
    }

  free_counter = 0;
  _dbus_data_slot_list_free (&list);

  _dbus_assert (N_SLOTS == free_counter);

  i = 0;
  while (i < N_SLOTS)
    {
      dbus_int32_t tmp = i;
      
      _dbus_data_slot_allocator_free (&allocator, &tmp);
      _dbus_assert (tmp == -1);
      ++i;
    }

  return TRUE;
}

#endif /* DBUS_ENABLE_EMBEDDED_TESTS */
