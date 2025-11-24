/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-hash.c Generic hash table utility (internal to D-Bus implementation)
 *
 * Copyright 1991-1993 The Regents of the University of California.
 * Copyright 1994 Sun Microsystems, Inc.
 * Copyright 2002-2005 Red Hat, Inc.
 * Copyright 2003 Joe Shaw
 * Copyright 2006 Sjoerd Simons
 * Copyright 2010 Fridrich Štrba
 * Copyright 2016 Ralf Habacker
 * Copyright 2017 Endless Mobile, Inc.
 *
 * Hash table implementation based on generic/tclHash.c from the Tcl
 * source code. The original Tcl license applies to portions of the
 * code from tclHash.c; the Tcl license follows this standad D-Bus
 * license information.
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
/*
 * The following copyright applies to code from the Tcl distribution.
 *
 * Copyright (c) 1991-1993 The Regents of the University of California.
 * Copyright (c) 1994 Sun Microsystems, Inc.
 *
 * This software is copyrighted by the Regents of the University of
 * California, Sun Microsystems, Inc., Scriptics Corporation, and
 * other parties.  The following terms apply to all files associated
 * with the software unless explicitly disclaimed in individual files.
 *
 * The authors hereby grant permission to use, copy, modify,
 * distribute, and license this software and its documentation for any
 * purpose, provided that existing copyright notices are retained in
 * all copies and that this notice is included verbatim in any
 * distributions. No written agreement, license, or royalty fee is
 * required for any of the authorized uses.  Modifications to this
 * software may be copyrighted by their authors and need not follow
 * the licensing terms described here, provided that the new terms are
 * clearly indicated on the first page of each file where they apply.
 *
 * IN NO EVENT SHALL THE AUTHORS OR DISTRIBUTORS BE LIABLE TO ANY
 * PARTY FOR DIRECT, INDIRECT, SPECIAL, INCIDENTAL, OR CONSEQUENTIAL
 * DAMAGES ARISING OUT OF THE USE OF THIS SOFTWARE, ITS DOCUMENTATION,
 * OR ANY DERIVATIVES THEREOF, EVEN IF THE AUTHORS HAVE BEEN ADVISED
 * OF THE POSSIBILITY OF SUCH DAMAGE.
 *
 * THE AUTHORS AND DISTRIBUTORS SPECIFICALLY DISCLAIM ANY WARRANTIES,
 * INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
 * MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE, AND
 * NON-INFRINGEMENT.  THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS,
 * AND THE AUTHORS AND DISTRIBUTORS HAVE NO OBLIGATION TO PROVIDE
 * MAINTENANCE, SUPPORT, UPDATES, ENHANCEMENTS, OR MODIFICATIONS.
 *
 * GOVERNMENT USE: If you are acquiring this software on behalf of the
 * U.S. government, the Government shall have only "Restricted Rights"
 * in the software and related documentation as defined in the Federal
 * Acquisition Regulations (FARs) in Clause 52.227.19 (c) (2).  If you
 * are acquiring the software on behalf of the Department of Defense,
 * the software shall be classified as "Commercial Computer Software"
 * and the Government shall have only "Restricted Rights" as defined
 * in Clause 252.227-7013 (c) (1) of DFARs.  Notwithstanding the
 * foregoing, the authors grant the U.S. Government and others acting
 * in its behalf permission to use and distribute the software in
 * accordance with the terms specified in this license.
 */

#include <config.h>
#include "dbus-hash.h"
#include "dbus-internals.h"
#include "dbus-mempool.h"
#include <dbus/dbus-test-tap.h>

/**
 * @defgroup DBusHashTable Hash table
 * @ingroup  DBusInternals
 * @brief DBusHashTable data structure
 *
 * Types and functions related to DBusHashTable.
 */

/**
 * @defgroup DBusHashTableInternals Hash table implementation details
 * @ingroup  DBusInternals
 * @brief DBusHashTable implementation details
 *
 * The guts of DBusHashTable.
 *
 * @{
 */

/**
 * When there are this many entries per bucket, on average, rebuild
 * the hash table to make it larger.
 */
#define REBUILD_MULTIPLIER  3

/**
 * Takes a preliminary integer hash value and produces an index into a
 * hash tables bucket list.  The idea is to make it so that
 * preliminary values that are arbitrarily similar will end up in
 * different buckets.  The hash function was taken from a
 * random-number generator. (This is used to hash integers.)
 *
 * The down_shift drops off the high bits of the hash index, and
 * decreases as we increase the number of hash buckets (to keep more
 * range in the hash index). The mask also strips high bits and strips
 * fewer high bits as the number of hash buckets increases.
 * I don't understand two things: why is the initial downshift 28
 * to keep 4 bits when the initial mask is 011 to keep 2 bits,
 * and why do we have both a mask and a downshift?
 * 
 */
#define RANDOM_INDEX(table, i) \
    (((((intptr_t) (i))*1103515245) >> (table)->down_shift) & (table)->mask)

/**
 * Initial number of buckets in hash table (hash table statically
 * allocates its buckets for this size and below).
 * The initial mask has to be synced to this.
 */
#define DBUS_SMALL_HASH_TABLE 4

/**
 * Typedef for DBusHashEntry
 */
typedef struct DBusHashEntry DBusHashEntry;

/**
 * @brief Internal representation of a hash entry.
 * 
 * A single entry (key-value pair) in the hash table.
 * Internal to hash table implementation.
 */
struct DBusHashEntry
{
  DBusHashEntry *next;    /**< Pointer to next entry in this
                           * hash bucket, or #NULL for end of
                           * chain.
                           */
  void *key;              /**< Hash key */
  void *value;            /**< Hash value */
};

/**
 * Function used to find and optionally create a hash entry.
 */
typedef DBusHashEntry* (* DBusFindEntryFunction) (DBusHashTable        *table,
                                                  void                 *key,
                                                  dbus_bool_t           create_if_not_found,
                                                  DBusHashEntry      ***bucket,
                                                  DBusPreallocatedHash *preallocated);

/**
 * @brief Internals of DBusHashTable.
 * 
 * Hash table internals. Hash tables are opaque objects, they must be
 * used via accessor functions.
 */
struct DBusHashTable {
  int refcount;                       /**< Reference count */
  
  DBusHashEntry **buckets;            /**< Pointer to bucket array.  Each
                                       * element points to first entry in
                                       * bucket's hash chain, or #NULL.
                                       */
  DBusHashEntry *static_buckets[DBUS_SMALL_HASH_TABLE];
                                       /**< Bucket array used for small tables
                                        * (to avoid mallocs and frees).
                                        */
  int n_buckets;                       /**< Total number of buckets allocated
                                        * at **buckets.
                                        */
  int n_entries;                       /**< Total number of entries present
                                        * in table.
                                        */
  int hi_rebuild_size;                 /**< Enlarge table when n_entries gets
                                        * to be this large.
                                        */
  int lo_rebuild_size;                 /**< Shrink table when n_entries gets
                                        * below this.
                                        */
  int down_shift;                      /**< Shift count used in hashing
                                        * function.  Designed to use high-
                                        * order bits of randomized keys.
                                        */
  int mask;                            /**< Mask value used in hashing
                                        * function.
                                        */
  DBusHashType key_type;               /**< Type of keys used in this table */


  DBusFindEntryFunction find_function; /**< Function for finding entries */

  DBusFreeFunction free_key_function;   /**< Function to free keys */
  DBusFreeFunction free_value_function; /**< Function to free values */

  DBusMemPool *entry_pool;              /**< Memory pool for hash entries */
};

/** 
 * @brief Internals of DBusHashIter.
 */
typedef struct
{
  DBusHashTable *table;     /**< Pointer to table containing entry. */
  DBusHashEntry **bucket;   /**< Pointer to bucket that points to
                             * first entry in this entry's chain:
                             * used for deleting the entry.
                             */
  DBusHashEntry *entry;      /**< Current hash entry */
  DBusHashEntry *next_entry; /**< Next entry to be iterated onto in current bucket */
  int next_bucket;           /**< index of next bucket */
  int n_entries_on_init;     /**< used to detect table resize since initialization */
} DBusRealHashIter;

_DBUS_STATIC_ASSERT (sizeof (DBusRealHashIter) == sizeof (DBusHashIter));

static DBusHashEntry* find_direct_function      (DBusHashTable          *table,
                                                 void                   *key,
                                                 dbus_bool_t             create_if_not_found,
                                                 DBusHashEntry        ***bucket,
                                                 DBusPreallocatedHash   *preallocated);
static DBusHashEntry* find_string_function      (DBusHashTable          *table,
                                                 void                   *key,
                                                 dbus_bool_t             create_if_not_found,
                                                 DBusHashEntry        ***bucket,
                                                 DBusPreallocatedHash   *preallocated);
static unsigned int   string_hash               (const char             *str);
static dbus_bool_t    rebuild_table             (DBusHashTable          *table);
static DBusHashEntry* alloc_entry               (DBusHashTable          *table);
static void           remove_entry              (DBusHashTable          *table,
                                                 DBusHashEntry         **bucket,
                                                 DBusHashEntry          *entry);
static void           free_entry                (DBusHashTable          *table,
                                                 DBusHashEntry          *entry);
static void           free_entry_data           (DBusHashTable          *table,
                                                 DBusHashEntry          *entry);


/** @} */

/**
 * @addtogroup DBusHashTable
 * @{
 */

/**
 * @typedef DBusHashIter
 *
 * Public opaque hash table iterator object.
 */

/**
 * @typedef DBusHashTable
 *
 * Public opaque hash table object.
 */

/**
 * @typedef DBusHashType
 *
 * Indicates the type of a key in the hash table.
 */

/**
 * Constructs a new hash table. Should be freed with
 * _dbus_hash_table_unref(). If memory cannot be
 * allocated for the hash table, returns #NULL.
 *
 * @param type the type of hash key to use.
 * @param key_free_function function to free hash keys.
 * @param value_free_function function to free hash values.
 * @returns a new DBusHashTable or #NULL if no memory.
 */
DBusHashTable*
_dbus_hash_table_new (DBusHashType     type,
                      DBusFreeFunction key_free_function,
                      DBusFreeFunction value_free_function)
{
  DBusHashTable *table;
  DBusMemPool *entry_pool;
  
  table = dbus_new0 (DBusHashTable, 1);
  if (table == NULL)
    return NULL;

  entry_pool = _dbus_mem_pool_new (sizeof (DBusHashEntry), TRUE);
  if (entry_pool == NULL)
    {
      dbus_free (table);
      return NULL;
    }
  
  table->refcount = 1;
  table->entry_pool = entry_pool;
  
  _dbus_assert (DBUS_SMALL_HASH_TABLE == _DBUS_N_ELEMENTS (table->static_buckets));
  
  table->buckets = table->static_buckets;  
  table->n_buckets = DBUS_SMALL_HASH_TABLE;
  table->n_entries = 0;
  table->hi_rebuild_size = DBUS_SMALL_HASH_TABLE * REBUILD_MULTIPLIER;
  table->lo_rebuild_size = 0;
  table->down_shift = 28;
  table->mask = 3;
  table->key_type = type;

  _dbus_assert (table->mask < table->n_buckets);
  
  switch (table->key_type)
    {
    case DBUS_HASH_INT:
    case DBUS_HASH_UINTPTR:
      table->find_function = find_direct_function;
      break;
    case DBUS_HASH_STRING:
      table->find_function = find_string_function;
      break;
    default:
      _dbus_assert_not_reached ("Unknown hash table type");
      break;
    }

  table->free_key_function = key_free_function;
  table->free_value_function = value_free_function;

  return table;
}


/**
 * Increments the reference count for a hash table.
 *
 * @param table the hash table to add a reference to.
 * @returns the hash table.
 */
DBusHashTable *
_dbus_hash_table_ref (DBusHashTable *table)
{
  table->refcount += 1;
  
  return table;
}

/**
 * Decrements the reference count for a hash table,
 * freeing the hash table if the count reaches zero.
 *
 * @param table the hash table to remove a reference from.
 */
void
_dbus_hash_table_unref (DBusHashTable *table)
{
  table->refcount -= 1;

  if (table->refcount == 0)
    {
#if 0
      DBusHashEntry *entry;
      DBusHashEntry *next;
      int i;

      /* Free the entries in the table. */
      for (i = 0; i < table->n_buckets; i++)
        {
          entry = table->buckets[i];
          while (entry != NULL)
            {
              next = entry->next;

              free_entry (table, entry);
              
              entry = next;
            }
        }
#else
      DBusHashEntry *entry;
      int i;

      /* Free the entries in the table. */
      for (i = 0; i < table->n_buckets; i++)
        {
          entry = table->buckets[i];
          while (entry != NULL)
            {
              free_entry_data (table, entry);
              
              entry = entry->next;
            }
        }
      /* We can do this very quickly with memory pools ;-) */
      _dbus_mem_pool_free (table->entry_pool);
#endif
      
      /* Free the bucket array, if it was dynamically allocated. */
      if (table->buckets != table->static_buckets)
        dbus_free (table->buckets);

      dbus_free (table);
    }
}

/**
 * Removed all entries from a hash table.
 *
 * @param table the hash table to remove all entries from.
 */
void
_dbus_hash_table_remove_all (DBusHashTable *table)
{
  DBusHashIter iter;
  _dbus_hash_iter_init (table, &iter);
  while (_dbus_hash_iter_next (&iter))
    {
      _dbus_hash_iter_remove_entry(&iter);
    }
}

static DBusHashEntry*
alloc_entry (DBusHashTable *table)
{
  DBusHashEntry *entry;

  entry = _dbus_mem_pool_alloc (table->entry_pool);
  
  return entry;
}

static void
free_entry_data (DBusHashTable  *table,
		 DBusHashEntry  *entry)
{
  if (table->free_key_function)
    (* table->free_key_function) (entry->key);
  if (table->free_value_function)
    (* table->free_value_function) (entry->value);
}

static void
free_entry (DBusHashTable  *table,
            DBusHashEntry  *entry)
{
  free_entry_data (table, entry);
  _dbus_mem_pool_dealloc (table->entry_pool, entry);
}

static void
remove_entry (DBusHashTable  *table,
              DBusHashEntry **bucket,
              DBusHashEntry  *entry)
{
  _dbus_assert (table != NULL);
  _dbus_assert (bucket != NULL);
  _dbus_assert (*bucket != NULL);  
  _dbus_assert (entry != NULL);
  
  if (*bucket == entry)
    *bucket = entry->next;
  else
    {
      DBusHashEntry *prev;
      prev = *bucket;

      while (prev->next != entry)
        prev = prev->next;      
      
      _dbus_assert (prev != NULL);

      prev->next = entry->next;
    }
  
  table->n_entries -= 1;
  free_entry (table, entry);
}

/**
 * Initializes a hash table iterator. To iterate over all entries in a
 * hash table, use the following code (the printf assumes a hash
 * from strings to strings obviously):
 *
 * @code
 * DBusHashIter iter;
 *
 * _dbus_hash_iter_init (table, &iter);
 * while (_dbus_hash_iter_next (&iter))
 *   {
 *      printf ("The first key is %s and value is %s\n",
 *              _dbus_hash_iter_get_string_key (&iter),
 *              _dbus_hash_iter_get_value (&iter));
 *   }
 * 
 * 
 * @endcode
 *
 * The iterator is initialized pointing "one before" the first hash
 * entry. The first call to _dbus_hash_iter_next() moves it onto
 * the first valid entry or returns #FALSE if the hash table is
 * empty. Subsequent calls move to the next valid entry or return
 * #FALSE if there are no more entries.
 *
 * Note that it is guaranteed to be safe to remove a hash entry during
 * iteration, but it is not safe to add a hash entry.
 * 
 * @param table the hash table to iterate over.
 * @param iter the iterator to initialize.
 */
void
_dbus_hash_iter_init (DBusHashTable *table,
                      DBusHashIter  *iter)
{
  DBusRealHashIter *real;
  
  _DBUS_STATIC_ASSERT (sizeof (DBusHashIter) == sizeof (DBusRealHashIter));
  
  real = (DBusRealHashIter*) iter;

  real->table = table;
  real->bucket = NULL;
  real->entry = NULL;
  real->next_entry = NULL;
  real->next_bucket = 0;
  real->n_entries_on_init = table->n_entries;
}

/**
 * Move the hash iterator forward one step, to the next hash entry.
 * The documentation for _dbus_hash_iter_init() explains in more
 * detail.
 *
 * @param iter the iterator to move forward.
 * @returns #FALSE if there are no more entries to move to.
 */
dbus_bool_t
_dbus_hash_iter_next (DBusHashIter  *iter)
{
  DBusRealHashIter *real;
  
  _DBUS_STATIC_ASSERT (sizeof (DBusHashIter) == sizeof (DBusRealHashIter));
  
  real = (DBusRealHashIter*) iter;

  /* if this assertion failed someone probably added hash entries
   * during iteration, which is bad.
   */
  _dbus_assert (real->n_entries_on_init >= real->table->n_entries);
  
  /* Remember that real->entry may have been deleted */
  
  while (real->next_entry == NULL)
    {
      if (real->next_bucket >= real->table->n_buckets)
        {
          /* invalidate iter and return false */
          real->entry = NULL;
          real->table = NULL;
          real->bucket = NULL;
          return FALSE;
        }

      real->bucket = &(real->table->buckets[real->next_bucket]);
      real->next_entry = *(real->bucket);
      real->next_bucket += 1;
    }

  _dbus_assert (real->next_entry != NULL);
  _dbus_assert (real->bucket != NULL);
  
  real->entry = real->next_entry;
  real->next_entry = real->entry->next;
  
  return TRUE;
}

/**
 * Removes the current entry from the hash table.
 * If a key_free_function or value_free_function
 * was provided to _dbus_hash_table_new(),
 * frees the key and/or value for this entry.
 *
 * @param iter the hash table iterator.
 */
void
_dbus_hash_iter_remove_entry (DBusHashIter *iter)
{
  DBusRealHashIter *real;

  real = (DBusRealHashIter*) iter;

  _dbus_assert (real->table != NULL);
  _dbus_assert (real->entry != NULL);
  _dbus_assert (real->bucket != NULL);
  
  remove_entry (real->table, real->bucket, real->entry);

  real->entry = NULL; /* make it crash if you try to use this entry */
}

/**
 * Gets the value of the current entry.
 *
 * @param iter the hash table iterator.
 */
void*
_dbus_hash_iter_get_value (DBusHashIter *iter)
{
  DBusRealHashIter *real;

  real = (DBusRealHashIter*) iter;

  _dbus_assert (real->table != NULL);
  _dbus_assert (real->entry != NULL);

  return real->entry->value;
}

/**
 * Sets the value of the current entry.
 * If the hash table has a value_free_function
 * it will be used to free the previous value.
 * The hash table will own the passed-in value
 * (it will not be copied).
 *
 * @param iter the hash table iterator.
 * @param value the new value.
 */
void
_dbus_hash_iter_set_value (DBusHashIter *iter,
                           void         *value)
{
  DBusRealHashIter *real;

  real = (DBusRealHashIter*) iter;

  _dbus_assert (real->table != NULL);
  _dbus_assert (real->entry != NULL);

  if (real->table->free_value_function && value != real->entry->value)    
    (* real->table->free_value_function) (real->entry->value);
  
  real->entry->value = value;
}

/**
 * Gets the key for the current entry.
 * Only works for hash tables of type #DBUS_HASH_INT.
 *
 * @param iter the hash table iterator.
 */
int
_dbus_hash_iter_get_int_key (DBusHashIter *iter)
{
  DBusRealHashIter *real;

  real = (DBusRealHashIter*) iter;

  _dbus_assert (real->table != NULL);
  _dbus_assert (real->entry != NULL);

  return _DBUS_POINTER_TO_INT (real->entry->key);
}

/**
 * Gets the key for the current entry.
 * Only works for hash tables of type #DBUS_HASH_UINTPTR.
 *
 * @param iter the hash table iterator.
 */
uintptr_t
_dbus_hash_iter_get_uintptr_key (DBusHashIter *iter)
{
  DBusRealHashIter *real;

  real = (DBusRealHashIter*) iter;

  _dbus_assert (real->table != NULL);
  _dbus_assert (real->entry != NULL);

  return (uintptr_t) real->entry->key;
}

/**
 * Gets the key for the current entry.
 * Only works for hash tables of type #DBUS_HASH_STRING
 * @param iter the hash table iterator.
 */
const char*
_dbus_hash_iter_get_string_key (DBusHashIter *iter)
{
  DBusRealHashIter *real;

  real = (DBusRealHashIter*) iter;

  _dbus_assert (real->table != NULL);
  _dbus_assert (real->entry != NULL);

  return real->entry->key;
}

/**
 * A low-level but efficient interface for manipulating the hash
 * table.  It's efficient because you can get, set, and optionally
 * create the hash entry while only running the hash function one
 * time.
 *
 * Note that while calling _dbus_hash_iter_next() on the iterator
 * filled in by this function may work, it's completely
 * undefined which entries are after this iter and which
 * are before it. So it would be silly to iterate using this
 * iterator.
 *
 * If the hash entry is created, its value will be initialized
 * to all bits zero.
 *
 * #FALSE may be returned due to memory allocation failure, or
 * because create_if_not_found was #FALSE and the entry
 * did not exist.
 *
 * If create_if_not_found is #TRUE, the hash
 * table takes ownership of the key that's passed in (either using it to create
 * the entry, or freeing it immediately).
 *
 * For a hash table of type #DBUS_HASH_INT, cast the int
 * key to the key parameter using #_DBUS_INT_TO_POINTER().
 * 
 * @param table the hash table.
 * @param key the hash key.
 * @param create_if_not_found if #TRUE, create the entry if it didn't exist.
 * @param iter the iterator to initialize.
 * @returns #TRUE if the hash entry now exists (and the iterator is thus valid).
 */
dbus_bool_t
_dbus_hash_iter_lookup (DBusHashTable *table,
                        void          *key,
                        dbus_bool_t    create_if_not_found,
                        DBusHashIter  *iter)
{
  DBusRealHashIter *real;
  DBusHashEntry *entry = NULL;
  DBusHashEntry **bucket = NULL;
  
  _DBUS_STATIC_ASSERT (sizeof (DBusHashIter) == sizeof (DBusRealHashIter));
  
  real = (DBusRealHashIter*) iter;

  entry = (* table->find_function) (table, key, create_if_not_found, &bucket, NULL);

  /* entry == NULL means not found, and either !create_if_not_found or OOM */
  if (entry == NULL)
    return FALSE;

  _dbus_assert (bucket != NULL);
  _dbus_assert (table->n_buckets >= 1);
  _dbus_assert (bucket >= table->buckets);
  _dbus_assert (bucket <= &table->buckets[table->n_buckets - 1]);

  if (create_if_not_found)
    {
      if (table->free_key_function && entry->key != key)
        (* table->free_key_function) (entry->key);

      entry->key = key;
    }

  real->table = table;
  real->bucket = bucket;
  real->entry = entry;
  real->next_entry = entry->next;
  real->next_bucket = (bucket - table->buckets) + 1;
  real->n_entries_on_init = table->n_entries; 

  _dbus_assert (real->next_bucket >= 0);
  _dbus_assert (real->next_bucket <= table->n_buckets);
  _dbus_assert (&(table->buckets[real->next_bucket-1]) == real->bucket);
  
  return TRUE;
}

static void
add_allocated_entry (DBusHashTable   *table,
                     DBusHashEntry   *entry,
                     unsigned int     idx,
                     void            *key,
                     DBusHashEntry ***bucket)
{
  DBusHashEntry **b;  
  
  entry->key = key;
  
  b = &(table->buckets[idx]);
  entry->next = *b;
  *b = entry;

  if (bucket)
    *bucket = b;
  
  table->n_entries += 1;

  /* note we ONLY rebuild when ADDING - because you can iterate over a
   * table and remove entries safely.
   */
  if (table->n_entries >= table->hi_rebuild_size ||
      table->n_entries < table->lo_rebuild_size)
    {
      if (!rebuild_table (table))
        return;

      if (bucket)
        {
          /* Recalculate hash for the new table size */
          switch (table->key_type)
            {
            case DBUS_HASH_STRING:
              idx = string_hash (entry->key) & table->mask;
              break;

            case DBUS_HASH_INT:
            case DBUS_HASH_UINTPTR:
              idx = RANDOM_INDEX (table, entry->key);
              break;

            default:
              idx = 0;
              _dbus_assert_not_reached ("Unknown hash table type");
              break;
            }

          *bucket = &(table->buckets[idx]);
        }
    }
}

static DBusHashEntry*
add_entry (DBusHashTable        *table, 
           unsigned int          idx,
           void                 *key,
           DBusHashEntry      ***bucket,
           DBusPreallocatedHash *preallocated)
{
  DBusHashEntry  *entry;

  if (preallocated == NULL)
    {
      entry = alloc_entry (table);
      if (entry == NULL)
        {
          if (bucket)
            *bucket = NULL;
          return NULL;
        }
    }
  else
    {
      entry = (DBusHashEntry*) preallocated;
    }

  add_allocated_entry (table, entry, idx, key, bucket);
  _dbus_assert (bucket == NULL || *bucket != NULL);

  return entry;
}

/* This is g_str_hash from GLib which was
 * extensively discussed/tested/profiled
 */
static unsigned int
string_hash (const char *str)
{
  const char *p = str;
  unsigned int h = *p;

  if (h)
    for (p += 1; *p != '\0'; p++)
      h = (h << 5) - h + *p;

  return h;
}

/** Key comparison function */
typedef int (* KeyCompareFunc) (const void *key_a, const void *key_b);

static DBusHashEntry*
find_generic_function (DBusHashTable        *table,
                       void                 *key,
                       unsigned int          idx,
                       KeyCompareFunc        compare_func,
                       dbus_bool_t           create_if_not_found,
                       DBusHashEntry      ***bucket,
                       DBusPreallocatedHash *preallocated)
{
  DBusHashEntry *entry;

  if (bucket)
    *bucket = NULL;

  /* Search all of the entries in this bucket. */
  entry = table->buckets[idx];
  while (entry != NULL)
    {
      if ((compare_func == NULL && key == entry->key) ||
          (compare_func != NULL && (* compare_func) (key, entry->key) == 0))
        {
          if (bucket)
            *bucket = &(table->buckets[idx]);

          if (preallocated)
            _dbus_hash_table_free_preallocated_entry (table, preallocated);
          
          return entry;
        }
      
      entry = entry->next;
    }

  if (create_if_not_found)
    {
      entry = add_entry (table, idx, key, bucket, preallocated);

      if (entry == NULL)  /* OOM */
        return NULL;

      _dbus_assert (bucket == NULL || *bucket != NULL);
    }
  else if (preallocated)
    {
      _dbus_hash_table_free_preallocated_entry (table, preallocated);
    }

  return entry;
}

static DBusHashEntry*
find_string_function (DBusHashTable        *table,
                      void                 *key,
                      dbus_bool_t           create_if_not_found,
                      DBusHashEntry      ***bucket,
                      DBusPreallocatedHash *preallocated)
{
  unsigned int idx;
  
  idx = string_hash (key) & table->mask;

  return find_generic_function (table, key, idx,
                                (KeyCompareFunc) strcmp, create_if_not_found, bucket,
                                preallocated);
}

static DBusHashEntry*
find_direct_function (DBusHashTable        *table,
                      void                 *key,
                      dbus_bool_t           create_if_not_found,
                      DBusHashEntry      ***bucket,
                      DBusPreallocatedHash *preallocated)
{
  unsigned int idx;
  
  idx = RANDOM_INDEX (table, key) & table->mask;


  return find_generic_function (table, key, idx,
                                NULL, create_if_not_found, bucket,
                                preallocated);
}

/* Return FALSE if nothing happened. */
static dbus_bool_t
rebuild_table (DBusHashTable *table)
{
  int old_size;
  int new_buckets;
  DBusHashEntry **old_buckets;
  DBusHashEntry **old_chain;
  DBusHashEntry *entry;
  dbus_bool_t growing;
  
  /*
   * Allocate and initialize the new bucket array, and set up
   * hashing constants for new array size.
   */

  growing = table->n_entries >= table->hi_rebuild_size;
  
  old_size = table->n_buckets;
  old_buckets = table->buckets;

  if (growing)
    {
      /* overflow paranoia */
      if (table->n_buckets < _DBUS_INT_MAX / 4 &&
          table->down_shift >= 2)
        new_buckets = table->n_buckets * 4;
      else
        return FALSE;   /* can't grow any more */
    }
  else
    {
      new_buckets = table->n_buckets / 4;
      if (new_buckets < DBUS_SMALL_HASH_TABLE)
        return FALSE;   /* don't bother shrinking this far */
    }

  table->buckets = dbus_new0 (DBusHashEntry*, new_buckets);
  if (table->buckets == NULL)
    {
      /* out of memory, yay - just don't reallocate, the table will
       * still work, albeit more slowly.
       */
      table->buckets = old_buckets;
      return FALSE;
    }

  table->n_buckets = new_buckets;
  
  if (growing)
    {
      table->lo_rebuild_size = table->hi_rebuild_size;
      table->hi_rebuild_size *= 4;
      
      table->down_shift -= 2;               /* keep 2 more high bits */
      table->mask = (table->mask << 2) + 3; /* keep 2 more high bits */
    }
  else
    {
      table->hi_rebuild_size = table->lo_rebuild_size;
      table->lo_rebuild_size /= 4;

      table->down_shift += 2;         /* keep 2 fewer high bits */
      table->mask = table->mask >> 2; /* keep 2 fewer high bits */
    }

#if 0
  printf ("%s table to lo = %d hi = %d downshift = %d mask = 0x%x\n",
          growing ? "GROW" : "SHRINK",
          table->lo_rebuild_size,
          table->hi_rebuild_size,
          table->down_shift,
          table->mask);
#endif
  
  _dbus_assert (table->lo_rebuild_size >= 0);
  _dbus_assert (table->hi_rebuild_size > table->lo_rebuild_size);
  _dbus_assert (table->down_shift >= 0);
  _dbus_assert (table->mask != 0);
  /* the mask is essentially the max index */
  _dbus_assert (table->mask < table->n_buckets);
  
  /*
   * Rehash all of the existing entries into the new bucket array.
   */

  for (old_chain = old_buckets; old_size > 0; old_size--, old_chain++)
    {
      for (entry = *old_chain; entry != NULL; entry = *old_chain)
        {
          unsigned int idx;
          DBusHashEntry **bucket;
          
          *old_chain = entry->next;
          switch (table->key_type)
            {
            case DBUS_HASH_STRING:
              idx = string_hash (entry->key) & table->mask;
              break;
            case DBUS_HASH_INT:
            case DBUS_HASH_UINTPTR:
              idx = RANDOM_INDEX (table, entry->key);
              break;
            default:
              idx = 0;
              _dbus_assert_not_reached ("Unknown hash table type");
              break;
            }
          
          bucket = &(table->buckets[idx]);
          entry->next = *bucket;
          *bucket = entry;
        }
    }
  
  /* Free the old bucket array, if it was dynamically allocated. */

  if (old_buckets != table->static_buckets)
    dbus_free (old_buckets);

  return TRUE;
}

/**
 * Looks up the value for a given string in a hash table
 * of type #DBUS_HASH_STRING. Returns %NULL if the value
 * is not present. (A not-present entry is indistinguishable
 * from an entry with a value of %NULL.)
 * @param table the hash table.
 * @param key the string to look up.
 * @returns the value of the hash entry.
 */
void*
_dbus_hash_table_lookup_string (DBusHashTable *table,
                                const char    *key)
{
  DBusHashEntry *entry;

  _dbus_assert (table->key_type == DBUS_HASH_STRING);
  
  entry = (* table->find_function) (table, (char*) key, FALSE, NULL, NULL);

  if (entry)
    return entry->value;
  else
    return NULL;
}

/**
 * Looks up the value for a given integer in a hash table
 * of type #DBUS_HASH_INT. Returns %NULL if the value
 * is not present. (A not-present entry is indistinguishable
 * from an entry with a value of %NULL.)
 * @param table the hash table.
 * @param key the integer to look up.
 * @returns the value of the hash entry.
 */
void*
_dbus_hash_table_lookup_int (DBusHashTable *table,
                             int            key)
{
  DBusHashEntry *entry;

  _dbus_assert (table->key_type == DBUS_HASH_INT);
  
  entry = (* table->find_function) (table, _DBUS_INT_TO_POINTER (key), FALSE, NULL, NULL);

  if (entry)
    return entry->value;
  else
    return NULL;
}

/**
 * Looks up the value for a given integer in a hash table
 * of type #DBUS_HASH_UINTPTR. Returns %NULL if the value
 * is not present. (A not-present entry is indistinguishable
 * from an entry with a value of %NULL.)
 * @param table the hash table.
 * @param key the integer to look up.
 * @returns the value of the hash entry.
 */
void*
_dbus_hash_table_lookup_uintptr (DBusHashTable *table,
                                 uintptr_t      key)
{
  DBusHashEntry *entry;

  _dbus_assert (table->key_type == DBUS_HASH_UINTPTR);
  
  entry = (* table->find_function) (table, (void*) key, FALSE, NULL, NULL);

  if (entry)
    return entry->value;
  else
    return NULL;
}

/**
 * Removes the hash entry for the given key. If no hash entry
 * for the key exists, does nothing.
 *
 * @param table the hash table.
 * @param key the hash key.
 * @returns #TRUE if the entry existed
 */
dbus_bool_t
_dbus_hash_table_remove_string (DBusHashTable *table,
                                const char    *key)
{
  DBusHashEntry *entry;
  DBusHashEntry **bucket;
  
  _dbus_assert (table->key_type == DBUS_HASH_STRING);
  
  entry = (* table->find_function) (table, (char*) key, FALSE, &bucket, NULL);

  if (entry)
    {
      remove_entry (table, bucket, entry);
      return TRUE;
    }
  else
    return FALSE;
}

/**
 * Removes the hash entry for the given key. If no hash entry
 * for the key exists, does nothing.
 *
 * @param table the hash table.
 * @param key the hash key.
 * @returns #TRUE if the entry existed
 */
dbus_bool_t
_dbus_hash_table_remove_int (DBusHashTable *table,
                             int            key)
{
  DBusHashEntry *entry;
  DBusHashEntry **bucket;
  
  _dbus_assert (table->key_type == DBUS_HASH_INT);
  
  entry = (* table->find_function) (table, _DBUS_INT_TO_POINTER (key), FALSE, &bucket, NULL);
  
  if (entry)
    {
      remove_entry (table, bucket, entry);
      return TRUE;
    }
  else
    return FALSE;
}

/**
 * Removes the hash entry for the given key. If no hash entry
 * for the key exists, does nothing.
 *
 * @param table the hash table.
 * @param key the hash key.
 * @returns #TRUE if the entry existed
 */
dbus_bool_t
_dbus_hash_table_remove_uintptr (DBusHashTable *table,
                                 uintptr_t      key)
{
  DBusHashEntry *entry;
  DBusHashEntry **bucket;
  
  _dbus_assert (table->key_type == DBUS_HASH_UINTPTR);
  
  entry = (* table->find_function) (table, (void*) key, FALSE, &bucket, NULL);
  
  if (entry)
    {
      remove_entry (table, bucket, entry);
      return TRUE;
    }
  else
    return FALSE;
}

/**
 * Creates a hash entry with the given key and value.
 * The key and value are not copied; they are stored
 * in the hash table by reference. If an entry with the
 * given key already exists, the previous key and value
 * are overwritten (and freed if the hash table has
 * a key_free_function and/or value_free_function).
 *
 * Returns #FALSE if memory for the new hash entry
 * can't be allocated.
 * 
 * @param table the hash table.
 * @param key the hash entry key.
 * @param value the hash entry value.
 */
dbus_bool_t
_dbus_hash_table_insert_string (DBusHashTable *table,
                                char          *key,
                                void          *value)
{
  DBusPreallocatedHash *preallocated;

  _dbus_assert (table->key_type == DBUS_HASH_STRING);

  preallocated = _dbus_hash_table_preallocate_entry (table);
  if (preallocated == NULL)
    return FALSE;

  _dbus_hash_table_insert_string_preallocated (table, preallocated,
                                               key, value);
  
  return TRUE;
}

/**
 * Creates a hash entry with the given key and value.
 * The key and value are not copied; they are stored
 * in the hash table by reference. If an entry with the
 * given key already exists, the previous key and value
 * are overwritten (and freed if the hash table has
 * a key_free_function and/or value_free_function).
 *
 * Returns #FALSE if memory for the new hash entry
 * can't be allocated.
 * 
 * @param table the hash table.
 * @param key the hash entry key.
 * @param value the hash entry value.
 */
dbus_bool_t
_dbus_hash_table_insert_int (DBusHashTable *table,
                             int            key,
                             void          *value)
{
  DBusHashEntry *entry;

  _dbus_assert (table->key_type == DBUS_HASH_INT);
  
  entry = (* table->find_function) (table, _DBUS_INT_TO_POINTER (key), TRUE, NULL, NULL);

  if (entry == NULL)
    return FALSE; /* no memory */

  if (table->free_key_function && entry->key != _DBUS_INT_TO_POINTER (key))
    (* table->free_key_function) (entry->key);
  
  if (table->free_value_function && entry->value != value)
    (* table->free_value_function) (entry->value);
  
  entry->key = _DBUS_INT_TO_POINTER (key);
  entry->value = value;

  return TRUE;
}

/**
 * Creates a hash entry with the given key and value.
 * The key and value are not copied; they are stored
 * in the hash table by reference. If an entry with the
 * given key already exists, the previous key and value
 * are overwritten (and freed if the hash table has
 * a key_free_function and/or value_free_function).
 *
 * Returns #FALSE if memory for the new hash entry
 * can't be allocated.
 * 
 * @param table the hash table.
 * @param key the hash entry key.
 * @param value the hash entry value.
 */
dbus_bool_t
_dbus_hash_table_insert_uintptr (DBusHashTable *table,
                                 uintptr_t      key,
                                 void          *value)
{
  DBusHashEntry *entry;

  _dbus_assert (table->key_type == DBUS_HASH_UINTPTR);
  
  entry = (* table->find_function) (table, (void*) key, TRUE, NULL, NULL);

  if (entry == NULL)
    return FALSE; /* no memory */

  if (table->free_key_function && entry->key != (void*) key)
    (* table->free_key_function) (entry->key);
  
  if (table->free_value_function && entry->value != value)
    (* table->free_value_function) (entry->value);
  
  entry->key = (void*) key;
  entry->value = value;

  return TRUE;
}

/**
 * Preallocate an opaque data blob that allows us to insert into the
 * hash table at a later time without allocating any memory.
 *
 * @param table the hash table
 * @returns the preallocated data, or #NULL if no memory
 */
DBusPreallocatedHash*
_dbus_hash_table_preallocate_entry (DBusHashTable *table)
{
  DBusHashEntry *entry;
  
  entry = alloc_entry (table);

  return (DBusPreallocatedHash*) entry;
}

/**
 * Frees an opaque DBusPreallocatedHash that was *not* used
 * in order to insert into the hash table.
 *
 * @param table the hash table
 * @param preallocated the preallocated data
 */
void
_dbus_hash_table_free_preallocated_entry (DBusHashTable        *table,
                                          DBusPreallocatedHash *preallocated)
{
  DBusHashEntry *entry;

  _dbus_assert (preallocated != NULL);
  
  entry = (DBusHashEntry*) preallocated;
  
  /* Don't use free_entry(), since this entry has no key/data */
  _dbus_mem_pool_dealloc (table->entry_pool, entry);
}

/**
 * Inserts a string-keyed entry into the hash table, using a
 * preallocated data block from
 * _dbus_hash_table_preallocate_entry(). This function cannot fail due
 * to lack of memory. The DBusPreallocatedHash object is consumed and
 * should not be reused or freed. Otherwise this function works
 * just like _dbus_hash_table_insert_string().
 *
 * @param table the hash table
 * @param preallocated the preallocated data
 * @param key the hash key
 * @param value the value 
 */
void
_dbus_hash_table_insert_string_preallocated (DBusHashTable        *table,
                                             DBusPreallocatedHash *preallocated,
                                             char                 *key,
                                             void                 *value)
{
  DBusHashEntry *entry;

  _dbus_assert (table->key_type == DBUS_HASH_STRING);
  _dbus_assert (preallocated != NULL);
  
  entry = (* table->find_function) (table, key, TRUE, NULL, preallocated);

  _dbus_assert (entry != NULL);
  
  if (table->free_key_function && entry->key != key)
    (* table->free_key_function) (entry->key);

  if (table->free_value_function && entry->value != value)
    (* table->free_value_function) (entry->value);
      
  entry->key = key;
  entry->value = value;
}

/**
 * Gets the number of hash entries in a hash table.
 *
 * @param table the hash table.
 * @returns the number of entries in the table.
 */
int
_dbus_hash_table_get_n_entries (DBusHashTable *table)
{
  return table->n_entries;
}

/**
 * Imports a string array into a hash table
 * The hash table needs to be initialized with string keys,
 * and dbus_free() as both key and value free-function.
 *
 * @param table the hash table
 * @param array the string array to import
 * @param delimiter the delimiter to separate key and value
 * @return #TRUE on success.
 * @return #FALSE if not enough memory.
 */

dbus_bool_t
_dbus_hash_table_from_array (DBusHashTable *table, char **array, char delimiter)
{
  DBusString   key;
  DBusString   value;
  int          i;
  dbus_bool_t  retval = FALSE;

  _dbus_assert (table != NULL);
  _dbus_assert (array != NULL);

  if (!_dbus_string_init (&key))
    {
        return FALSE;
    }

  if (!_dbus_string_init (&value))
    {
      _dbus_string_free (&key);
      return FALSE;
    }

  for (i = 0; array[i] != NULL; i++)
    {
      if (!_dbus_string_append (&key, array[i]))
        break;

      if (_dbus_string_split_on_byte (&key, delimiter, &value))
        {
          char *hash_key, *hash_value;

          if (!_dbus_string_steal_data (&key, &hash_key))
            break;

          if (!_dbus_string_steal_data (&value, &hash_value))
            break;

          if (!_dbus_hash_table_insert_string (table,
                                               hash_key, hash_value))
            break;
        }
      _dbus_string_set_length (&key, 0);
      _dbus_string_set_length (&value, 0);
    }

  if (array[i] != NULL)
    goto out;

  retval = TRUE;
out:

  _dbus_string_free (&key);
  _dbus_string_free (&value);

  return retval;
}

/**
 * Creates a string array from a hash table
 *
 * @param table the hash table
 * @param delimiter the delimiter to join key and value
 * @return pointer to created string array (free with dbus_free_string_array)
 * @return #FALSE if not enough memory.
 */
char **
_dbus_hash_table_to_array (DBusHashTable *table, char delimiter)
{
  int i, length;
  DBusString entry;
  DBusHashIter iter;
  char **array;

  _dbus_assert (table != NULL);

  length = _dbus_hash_table_get_n_entries (table);

  array = dbus_new0 (char *, length + 1);

  if (array == NULL)
    return NULL;

  i = 0;
  _dbus_hash_iter_init (table, &iter);

  if (!_dbus_string_init (&entry))
    {
      dbus_free_string_array (array);
      return NULL;
    }

  while (_dbus_hash_iter_next (&iter))
    {
      const char *key, *value;

      key = (const char *) _dbus_hash_iter_get_string_key (&iter);
      value = (const char *) _dbus_hash_iter_get_value (&iter);

      if (!_dbus_string_append_printf (&entry, "%s%c%s", key, delimiter, value))
        break;

      if (!_dbus_string_steal_data (&entry, array + i))
        break;
      i++;
    }

  _dbus_string_free (&entry);

  if (i != length)
    {
      dbus_free_string_array (array);
      array = NULL;
    }

  return array;
}

/** @} */
