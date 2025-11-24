/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-mempool.h Memory pools
 *
 * Copyright (C) 2002, 2003  Red Hat, Inc.
 * Copyright (C) 2003  CodeFactory AB
 * Copyright (C) 2011-2012  Collabora Ltd.
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
#include "dbus-mempool.h"
#include "dbus-internals.h"
#include "dbus-valgrind-internal.h"

/**
 * @defgroup DBusMemPool memory pools
 * @ingroup  DBusInternals
 * @brief DBusMemPool object
 *
 * Types and functions related to DBusMemPool.  A memory pool is used
 * to decrease memory fragmentation/overhead and increase speed for
 * blocks of small uniformly-sized objects. The main point is to avoid
 * the overhead of a malloc block for each small object, speed is
 * secondary.
 */

/**
 * @defgroup DBusMemPoolInternals Memory pool implementation details
 * @ingroup  DBusInternals
 * @brief DBusMemPool implementation details
 *
 * The guts of DBusMemPool.
 *
 * @{
 */

/**
 * typedef so DBusFreedElement struct can refer to itself.
 */
typedef struct DBusFreedElement DBusFreedElement;

/**
 * struct representing an element on the free list.
 * We just cast freed elements to this so we can
 * make a list out of them.
 */
struct DBusFreedElement
{
  DBusFreedElement *next; /**< next element of the free list */
};

/**
 * The dummy size of the variable-length "elements"
 * field in DBusMemBlock
 */
#define ELEMENT_PADDING 4

/**
 * Typedef for DBusMemBlock so the struct can recursively
 * point to itself.
 */
typedef struct DBusMemBlock DBusMemBlock;

/**
 * DBusMemBlock object represents a single malloc()-returned
 * block that gets chunked up into objects in the memory pool.
 */
struct DBusMemBlock
{
  DBusMemBlock *next;  /**< next block in the list, which is already used up;
                        *   only saved so we can free all the blocks
                        *   when we free the mem pool.
                        */

  /* this is a long so that "elements" is aligned */
  long used_so_far;     /**< bytes of this block already allocated as elements. */
  
  unsigned char elements[ELEMENT_PADDING]; /**< the block data, actually allocated to required size */
};

/**
 * Internals fields of DBusMemPool
 */
struct DBusMemPool
{
  int element_size;                /**< size of a single object in the pool */
  int block_size;                  /**< size of most recently allocated block */
  unsigned int zero_elements : 1;  /**< whether to zero-init allocated elements */

  DBusFreedElement *free_elements; /**< a free list of elements to recycle */
  DBusMemBlock *blocks;            /**< blocks of memory from malloc() */
  int allocated_elements;          /**< Count of outstanding allocated elements */
};

/** @} */

/**
 * @addtogroup DBusMemPool
 *
 * @{
 */

/**
 * @typedef DBusMemPool
 *
 * Opaque object representing a memory pool. Memory pools allow
 * avoiding per-malloc-block memory overhead when allocating a lot of
 * small objects that are all the same size. They are slightly
 * faster than calling malloc() also.
 */

/**
 * Creates a new memory pool, or returns #NULL on failure.  Objects in
 * the pool must be at least sizeof(void*) bytes each, due to the way
 * memory pools work. To avoid creating 64 bit problems, this means at
 * least 8 bytes on all platforms, unless you are 4 bytes on 32-bit
 * and 8 bytes on 64-bit.
 *
 * @param element_size size of an element allocated from the pool.
 * @param zero_elements whether to zero-initialize elements
 * @returns the new pool or #NULL
 */
DBusMemPool*
_dbus_mem_pool_new (int element_size,
                    dbus_bool_t zero_elements)
{
  DBusMemPool *pool;

  pool = dbus_new0 (DBusMemPool, 1);
  if (pool == NULL)
    return NULL;

  /* Make the element size at least 8 bytes. */
  if (element_size < 8)
    element_size = 8;
  
  /* these assertions are equivalent but the first is more clear
   * to programmers that see it fail.
   */
  _dbus_assert (element_size >= (int) sizeof (void*));
  _dbus_assert (element_size >= (int) sizeof (DBusFreedElement));

  /* align the element size to a pointer boundary so we won't get bus
   * errors under other architectures.  
   */
  pool->element_size = _DBUS_ALIGN_VALUE (element_size, sizeof (void *));

  pool->zero_elements = zero_elements != FALSE;

  pool->allocated_elements = 0;
  
  /* pick a size for the first block; it increases
   * for each block we need to allocate. This is
   * actually half the initial block size
   * since _dbus_mem_pool_alloc() unconditionally
   * doubles it prior to creating a new block.  */
  pool->block_size = pool->element_size * 8;

  _dbus_assert ((pool->block_size %
                 pool->element_size) == 0);

  VALGRIND_CREATE_MEMPOOL (pool, 0, zero_elements);

  return pool;
}

/**
 * Frees a memory pool (and all elements allocated from it).
 *
 * @param pool the memory pool.
 */
void
_dbus_mem_pool_free (DBusMemPool *pool)
{
  DBusMemBlock *block;

  VALGRIND_DESTROY_MEMPOOL (pool);

  block = pool->blocks;
  while (block != NULL)
    {
      DBusMemBlock *next = block->next;

      dbus_free (block);

      block = next;
    }

  dbus_free (pool);
}

/**
 * Allocates an object from the memory pool.
 * The object must be freed with _dbus_mem_pool_dealloc().
 *
 * @param pool the memory pool
 * @returns the allocated object or #NULL if no memory.
 */
void*
_dbus_mem_pool_alloc (DBusMemPool *pool)
{
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
  if (_dbus_disable_mem_pools ())
    {
      DBusMemBlock *block;
      int alloc_size;
      
      /* This is obviously really silly, but it's
       * debug-mode-only code that is compiled out
       * when tests are disabled (_dbus_disable_mem_pools()
       * is a constant expression FALSE so this block
       * should vanish)
       */
      
      alloc_size = sizeof (DBusMemBlock) - ELEMENT_PADDING +
        pool->element_size;
      
      if (pool->zero_elements)
        block = dbus_malloc0 (alloc_size);
      else
        block = dbus_malloc (alloc_size);

      if (block != NULL)
        {
          block->next = pool->blocks;
          pool->blocks = block;
          pool->allocated_elements += 1;

          VALGRIND_MEMPOOL_ALLOC (pool, (void *) &block->elements[0],
              pool->element_size);
          return (void*) &block->elements[0];
        }
      else
        return NULL;
    }
  else
#endif
    {
      if (_dbus_decrement_fail_alloc_counter ())
        {
          _dbus_verbose (" FAILING mempool alloc\n");
          return NULL;
        }
      else if (pool->free_elements)
        {
          DBusFreedElement *element = pool->free_elements;

          pool->free_elements = pool->free_elements->next;

          VALGRIND_MEMPOOL_ALLOC (pool, element, pool->element_size);

          if (pool->zero_elements)
            memset (element, '\0', pool->element_size);

          pool->allocated_elements += 1;

          return element;
        }
      else
        {
          void *element;
      
          if (pool->blocks == NULL ||
              pool->blocks->used_so_far == pool->block_size)
            {
              /* Need a new block */
              DBusMemBlock *block;
              int alloc_size;
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
              int saved_counter;
#endif
          
              if (pool->block_size <= _DBUS_INT_MAX / 4) /* avoid overflow */
                {
                  /* use a larger block size for our next block */
                  pool->block_size *= 2;
                  _dbus_assert ((pool->block_size %
                                 pool->element_size) == 0);
                }

              alloc_size = sizeof (DBusMemBlock) - ELEMENT_PADDING + pool->block_size;

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
              /* We save/restore the counter, so that memory pools won't
               * cause a given function to have different number of
               * allocations on different invocations. i.e.  when testing
               * we want consistent alloc patterns. So we skip our
               * malloc here for purposes of failed alloc simulation.
               */
              saved_counter = _dbus_get_fail_alloc_counter ();
              _dbus_set_fail_alloc_counter (_DBUS_INT_MAX);
#endif
          
              if (pool->zero_elements)
                block = dbus_malloc0 (alloc_size);
              else
                block = dbus_malloc (alloc_size);

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
              _dbus_set_fail_alloc_counter (saved_counter);
              _dbus_assert (saved_counter == _dbus_get_fail_alloc_counter ());
#endif
          
              if (block == NULL)
                return NULL;

              block->used_so_far = 0;
              block->next = pool->blocks;
              pool->blocks = block;          
            }
      
          element = &pool->blocks->elements[pool->blocks->used_so_far];
          
          pool->blocks->used_so_far += pool->element_size;

          pool->allocated_elements += 1;

          VALGRIND_MEMPOOL_ALLOC (pool, element, pool->element_size);
          return element;
        }
    }
}

/**
 * Deallocates an object previously created with
 * _dbus_mem_pool_alloc(). The previous object
 * must have come from this same pool.
 * @param pool the memory pool
 * @param element the element earlier allocated.
 * @returns #TRUE if there are no remaining allocated elements
 */
dbus_bool_t
_dbus_mem_pool_dealloc (DBusMemPool *pool,
                        void        *element)
{
  VALGRIND_MEMPOOL_FREE (pool, element);

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
  if (_dbus_disable_mem_pools ())
    {
      DBusMemBlock *block;
      DBusMemBlock *prev;

      /* mmm, fast. ;-) debug-only code, so doesn't matter. */
      
      prev = NULL;
      block = pool->blocks;

      while (block != NULL)
        {
          if (block->elements == (unsigned char*) element)
            {
              if (prev)
                prev->next = block->next;
              else
                pool->blocks = block->next;
              
              dbus_free (block);

              _dbus_assert (pool->allocated_elements > 0);
              pool->allocated_elements -= 1;
              
              if (pool->allocated_elements == 0)
                _dbus_assert (pool->blocks == NULL);
              
              return pool->blocks == NULL;
            }
          prev = block;
          block = block->next;
        }
      
      _dbus_assert_not_reached ("freed nonexistent block");
      return FALSE;
    }
  else
#endif
    {
      DBusFreedElement *freed;
      
      freed = element;
      /* used for internal mempool administration */
      VALGRIND_MAKE_MEM_UNDEFINED (freed, sizeof (*freed));

      freed->next = pool->free_elements;
      pool->free_elements = freed;
      
      _dbus_assert (pool->allocated_elements > 0);
      pool->allocated_elements -= 1;
      
      return pool->allocated_elements == 0;
    }
}

#ifdef DBUS_ENABLE_STATS
void
_dbus_mem_pool_get_stats (DBusMemPool   *pool,
                          dbus_uint32_t *in_use_p,
                          dbus_uint32_t *in_free_list_p,
                          dbus_uint32_t *allocated_p)
{
  DBusMemBlock *block;
  DBusFreedElement *freed;
  dbus_uint32_t in_use = 0;
  dbus_uint32_t in_free_list = 0;
  dbus_uint32_t allocated = 0;

  if (pool != NULL)
    {
      in_use = pool->element_size * pool->allocated_elements;

      for (freed = pool->free_elements; freed != NULL; freed = freed->next)
        {
          in_free_list += pool->element_size;
        }

      for (block = pool->blocks; block != NULL; block = block->next)
        {
          if (block == pool->blocks)
            allocated += pool->block_size;
          else
            allocated += block->used_so_far;
        }
    }

  if (in_use_p != NULL)
    *in_use_p = in_use;

  if (in_free_list_p != NULL)
    *in_free_list_p = in_free_list;

  if (allocated_p != NULL)
    *allocated_p = allocated;
}
#endif /* DBUS_ENABLE_STATS */

/** @} */
