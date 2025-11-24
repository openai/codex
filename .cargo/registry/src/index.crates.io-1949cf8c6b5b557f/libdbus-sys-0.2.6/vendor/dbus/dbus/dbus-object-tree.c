/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-object-tree.c  DBusObjectTree (internals of DBusConnection)
 *
 * Copyright (C) 2003, 2005  Red Hat Inc.
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
#include "dbus-object-tree.h"
#include "dbus-connection-internal.h"
#include "dbus-internals.h"
#include "dbus-hash.h"
#include "dbus-protocol.h"
#include "dbus-string.h"
#include <dbus/dbus-test-tap.h>
#include <string.h>
#include <stdlib.h>

/**
 * @defgroup DBusObjectTree A hierarchy of objects with container-contained relationship
 * @ingroup  DBusInternals
 * @brief DBusObjectTree is used by DBusConnection to track the object tree
 *
 * Types and functions related to DBusObjectTree. These
 * are all library-internal.
 *
 * @{
 */

/** Subnode of the object hierarchy */
typedef struct DBusObjectSubtree DBusObjectSubtree;

static DBusObjectSubtree* _dbus_object_subtree_new   (const char                  *name,
                                                      const DBusObjectPathVTable  *vtable,
                                                      void                        *user_data);
static DBusObjectSubtree* _dbus_object_subtree_ref   (DBusObjectSubtree           *subtree);
static void               _dbus_object_subtree_unref (DBusObjectSubtree           *subtree);

/**
 * Internals of DBusObjectTree
 */
struct DBusObjectTree
{
  int                 refcount;   /**< Reference count */
  DBusConnection     *connection; /**< Connection this tree belongs to */

  DBusObjectSubtree  *root;       /**< Root of the tree ("/" node) */
};

/**
 * Struct representing a single registered subtree handler, or node
 * that's a parent of a registered subtree handler. If
 * message_function != NULL there's actually a handler at this node.
 */
struct DBusObjectSubtree
{
  DBusAtomic                         refcount;            /**< Reference count */
  DBusObjectSubtree                 *parent;              /**< Parent node */
  DBusObjectPathUnregisterFunction   unregister_function; /**< Function to call on unregister */
  DBusObjectPathMessageFunction      message_function;    /**< Function to handle messages */
  void                              *user_data;           /**< Data for functions */
  DBusObjectSubtree                **subtrees;            /**< Child nodes */
  int                                n_subtrees;          /**< Number of child nodes */
  int                                max_subtrees;        /**< Number of allocated entries in subtrees */
  unsigned int                       invoke_as_fallback : 1; /**< Whether to invoke message_function when child nodes don't handle the message */
  char                               name[1]; /**< Allocated as large as necessary */
};

/**
 * Creates a new object tree, representing a mapping from paths
 * to handler vtables.
 *
 * @param connection the connection this tree belongs to
 * @returns the new tree or #NULL if no memory
 */
DBusObjectTree*
_dbus_object_tree_new (DBusConnection *connection)
{
  DBusObjectTree *tree;

  /* the connection passed in here isn't fully constructed,
   * so don't do anything more than store a pointer to
   * it
   */

  tree = dbus_new0 (DBusObjectTree, 1);
  if (tree == NULL)
    goto oom;

  tree->refcount = 1;
  tree->connection = connection;
  tree->root = _dbus_object_subtree_new ("/", NULL, NULL);
  if (tree->root == NULL)
    goto oom;
  tree->root->invoke_as_fallback = TRUE;
  
  return tree;

 oom:
  if (tree)
    {
      dbus_free (tree);
    }

  return NULL;
}

/**
 * Increment the reference count
 * @param tree the object tree
 * @returns the object tree
 */
DBusObjectTree *
_dbus_object_tree_ref (DBusObjectTree *tree)
{
  _dbus_assert (tree->refcount > 0);

  tree->refcount += 1;

  return tree;
}

/**
 * Decrement the reference count
 * @param tree the object tree
 */
void
_dbus_object_tree_unref (DBusObjectTree *tree)
{
  _dbus_assert (tree->refcount > 0);

  tree->refcount -= 1;

  if (tree->refcount == 0)
    {
      _dbus_object_tree_free_all_unlocked (tree);

      dbus_free (tree);
    }
}

/** Set to 1 to get a bunch of debug spew about finding the
 * subtree nodes
 */
#define VERBOSE_FIND 0

static DBusObjectSubtree*
find_subtree_recurse (DBusObjectSubtree  *subtree,
                      const char        **path,
                      dbus_bool_t         create_if_not_found,
                      int                *index_in_parent,
                      dbus_bool_t        *exact_match)
{
  int i, j;
  dbus_bool_t return_deepest_match;

  return_deepest_match = exact_match != NULL;

  _dbus_assert (!(return_deepest_match && create_if_not_found));

  if (path[0] == NULL)
    {
#if VERBOSE_FIND
      _dbus_verbose ("  path exhausted, returning %s\n",
                     subtree->name);
#endif
      if (exact_match != NULL)
	*exact_match = TRUE;
      return subtree;
    }

#if VERBOSE_FIND
  _dbus_verbose ("  searching children of %s for %s\n",
                 subtree->name, path[0]);
#endif
  
  i = 0;
  j = subtree->n_subtrees;
  while (i < j)
    {
      int k, v;

      k = (i + j) / 2;
      v = strcmp (path[0], subtree->subtrees[k]->name);

#if VERBOSE_FIND
      _dbus_verbose ("  %s cmp %s = %d\n",
                     path[0], subtree->subtrees[k]->name,
                     v);
#endif
      
      if (v == 0)
        {
          if (index_in_parent)
            {
#if VERBOSE_FIND
              _dbus_verbose ("  storing parent index %d\n", k);
#endif
              *index_in_parent = k;
            }

          if (return_deepest_match)
            {
              DBusObjectSubtree *next;

              next = find_subtree_recurse (subtree->subtrees[k],
                                           &path[1], create_if_not_found, 
                                           index_in_parent, exact_match);
              if (next == NULL &&
                  subtree->invoke_as_fallback)
                {
#if VERBOSE_FIND
                  _dbus_verbose ("  no deeper match found, returning %s\n",
                                 subtree->name);
#endif
		  if (exact_match != NULL)
		    *exact_match = FALSE;
                  return subtree;
                }
              else
                return next;
            }
          else
            return find_subtree_recurse (subtree->subtrees[k],
                                         &path[1], create_if_not_found, 
                                         index_in_parent, exact_match);
        }
      else if (v < 0)
        {
          j = k;
        }
      else
        {
          i = k + 1;
        }
    }

#if VERBOSE_FIND
  _dbus_verbose ("  no match found, current tree %s, create_if_not_found = %d\n",
                 subtree->name, create_if_not_found);
#endif
  
  if (create_if_not_found)
    {
      DBusObjectSubtree* child;
      int child_pos, new_n_subtrees;

#if VERBOSE_FIND
      _dbus_verbose ("  creating subtree %s\n",
                     path[0]);
#endif
      
      child = _dbus_object_subtree_new (path[0],
                                        NULL, NULL);
      if (child == NULL)
        return NULL;

      new_n_subtrees = subtree->n_subtrees + 1;
      if (new_n_subtrees > subtree->max_subtrees)
        {
          int new_max_subtrees;
          DBusObjectSubtree **new_subtrees;

          new_max_subtrees = subtree->max_subtrees == 0 ? 1 : 2 * subtree->max_subtrees;
          new_subtrees = dbus_realloc (subtree->subtrees,
                                       new_max_subtrees * sizeof (DBusObjectSubtree*));
          if (new_subtrees == NULL)
            {
              _dbus_object_subtree_unref (child);
              return NULL;
            }
          subtree->subtrees = new_subtrees;
          subtree->max_subtrees = new_max_subtrees;
        }

      /* The binary search failed, so i == j points to the 
         place the child should be inserted. */
      child_pos = i;
      _dbus_assert (child_pos < new_n_subtrees &&
                    new_n_subtrees <= subtree->max_subtrees);
      if (child_pos + 1 < new_n_subtrees)
	{
	  memmove (&subtree->subtrees[child_pos+1], 
		   &subtree->subtrees[child_pos], 
		   (new_n_subtrees - child_pos - 1) * 
		   sizeof subtree->subtrees[0]);
	}
      subtree->subtrees[child_pos] = child;

      if (index_in_parent)
        *index_in_parent = child_pos;
      subtree->n_subtrees = new_n_subtrees;
      child->parent = subtree;

      return find_subtree_recurse (child,
                                   &path[1], create_if_not_found, 
                                   index_in_parent, exact_match);
    }
  else
    {
      if (exact_match != NULL)
	*exact_match = FALSE;
      return (return_deepest_match && subtree->invoke_as_fallback) ? subtree : NULL;
    }
}

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
static DBusObjectSubtree*
find_subtree (DBusObjectTree *tree,
              const char    **path,
              int            *index_in_parent)
{
  DBusObjectSubtree *subtree;

#if VERBOSE_FIND
  _dbus_verbose ("Looking for exact registered subtree\n");
#endif
  
  subtree = find_subtree_recurse (tree->root, path, FALSE, index_in_parent, NULL);

  if (subtree && subtree->message_function == NULL)
    return NULL;
  else
    return subtree;
}
#endif

static DBusObjectSubtree*
lookup_subtree (DBusObjectTree *tree,
                const char    **path)
{
#if VERBOSE_FIND
  _dbus_verbose ("Looking for subtree\n");
#endif
  return find_subtree_recurse (tree->root, path, FALSE, NULL, NULL);
}

static DBusObjectSubtree*
find_handler (DBusObjectTree *tree,
              const char    **path,
              dbus_bool_t    *exact_match)
{
#if VERBOSE_FIND
  _dbus_verbose ("Looking for deepest handler\n");
#endif
  _dbus_assert (exact_match != NULL);

  *exact_match = FALSE; /* ensure always initialized */
  
  return find_subtree_recurse (tree->root, path, FALSE, NULL, exact_match);
}

static DBusObjectSubtree*
ensure_subtree (DBusObjectTree *tree,
                const char    **path)
{
#if VERBOSE_FIND
  _dbus_verbose ("Ensuring subtree\n");
#endif
  return find_subtree_recurse (tree->root, path, TRUE, NULL, NULL);
}

static char *flatten_path (const char **path);

/**
 * Registers a new subtree in the global object tree.
 *
 * @param tree the global object tree
 * @param fallback #TRUE to handle messages to children of this path
 * @param path NULL-terminated array of path elements giving path to subtree
 * @param vtable the vtable used to traverse this subtree
 * @param user_data user data to pass to methods in the vtable
 * @param error address where an error can be returned
 * @returns #FALSE if an error (#DBUS_ERROR_NO_MEMORY or
 *    #DBUS_ERROR_OBJECT_PATH_IN_USE) is reported
 */
dbus_bool_t
_dbus_object_tree_register (DBusObjectTree              *tree,
                            dbus_bool_t                  fallback,
                            const char                 **path,
                            const DBusObjectPathVTable  *vtable,
                            void                        *user_data,
                            DBusError                   *error)
{
  DBusObjectSubtree  *subtree;

  _dbus_assert (tree != NULL);
  _dbus_assert (vtable->message_function != NULL);
  _dbus_assert (path != NULL);

  subtree = ensure_subtree (tree, path);
  if (subtree == NULL)
    {
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  if (subtree->message_function != NULL)
    {
      if (error != NULL)
        {
          char *complete_path = flatten_path (path);

          dbus_set_error (error, DBUS_ERROR_OBJECT_PATH_IN_USE,
                          "A handler is already registered for %s",
                          complete_path ? complete_path
                                        : "(cannot represent path: out of memory!)");

          dbus_free (complete_path);
        }

      return FALSE;
    }

  subtree->message_function = vtable->message_function;
  subtree->unregister_function = vtable->unregister_function;
  subtree->user_data = user_data;
  subtree->invoke_as_fallback = fallback != FALSE;

  return TRUE;
}

/**
 * Attempts to unregister the given subtree.  If the subtree is registered,
 * stores its unregister function and user data for later use and returns
 * #TRUE.  If subtree is not registered, simply returns #FALSE.  Does not free
 * subtree or remove it from the object tree.
 *
 * @param subtree the subtree to unregister
 * @param unregister_function_out stores subtree's unregister_function
 * @param user_data_out stores subtree's user_data
 * @return #FALSE if the subtree was not registered, #TRUE on success
 */
static dbus_bool_t
unregister_subtree (DBusObjectSubtree                 *subtree,
                    DBusObjectPathUnregisterFunction  *unregister_function_out,
                    void                             **user_data_out)
{
  _dbus_assert (subtree != NULL);
  _dbus_assert (unregister_function_out != NULL);
  _dbus_assert (user_data_out != NULL);

  /* Confirm subtree is registered */
  if (subtree->message_function != NULL)
    {
      subtree->message_function = NULL;

      *unregister_function_out = subtree->unregister_function;
      *user_data_out = subtree->user_data;

      subtree->unregister_function = NULL;
      subtree->user_data = NULL;

      return TRUE;
    }
  else
    {
      /* Assert that this unregistered subtree is either the root node or has
         children, otherwise we have a dangling path which should never
         happen */
      _dbus_assert (subtree->parent == NULL || subtree->n_subtrees > 0);

      /* The subtree is not registered */
      return FALSE;
    }
}

/**
 * Attempts to remove a child subtree from its parent.  If removal is
 * successful, also frees the child.  Returns #TRUE on success, #FALSE
 * otherwise.  A #FALSE return value tells unregister_and_free_path_recurse to
 * stop attempting to remove ancestors, i.e., that no ancestors of the
 * specified child are eligible for removal.
 *
 * @param parent parent from which to remove child
 * @param child_index parent->subtrees index of child to remove
 * @return #TRUE if removal and free succeed, #FALSE otherwise
 */
static dbus_bool_t
attempt_child_removal (DBusObjectSubtree  *parent,
                       int child_index)
{
  /* Candidate for removal */
  DBusObjectSubtree* candidate;

  _dbus_assert (parent != NULL);
  _dbus_assert (child_index >= 0 && child_index < parent->n_subtrees);

  candidate = parent->subtrees[child_index];
  _dbus_assert (candidate != NULL);

  if (candidate->n_subtrees == 0 && candidate->message_function == NULL)
    {
      /* The candidate node is childless and is not a registered
         path, so... */

      /* ... remove it from its parent... */
      /* Assumes a 0-byte memmove is OK */
      memmove (&parent->subtrees[child_index],
               &parent->subtrees[child_index + 1],
               (parent->n_subtrees - child_index - 1)
               * sizeof (parent->subtrees[0]));
      parent->n_subtrees -= 1;

      /* ... and free it */
      candidate->parent = NULL;
      _dbus_object_subtree_unref (candidate);

      return TRUE;
    }
  return FALSE;
}

/**
 * Searches the object tree for a registered subtree node at the given path.
 * If a registered node is found, it is removed from the tree and freed, and
 * TRUE is returned.  If a registered subtree node is not found at the given
 * path, the tree is not modified and FALSE is returned.
 *
 * The found node's unregister_function and user_data are returned in the
 * corresponding _out arguments.  The caller should define these variables and
 * pass their addresses as arguments.
 *
 * Likewise, the caller should define and set to TRUE a boolean variable, then
 * pass its address as the continue_removal_attempts argument.
 *
 * Once a matching registered node is found, removed and freed, the recursive
 * return path is traversed.  Along the way, eligible ancestor nodes are
 * removed and freed.  An ancestor node is eligible for removal if and only if
 * 1) it has no children, i.e., it has become childless and 2) it is not itself
 * a registered handler.
 *
 * For example, suppose /A/B and /A/C are registered paths, and that these are
 * the only paths in the tree.  If B is removed and freed, C is still reachable
 * through A, so A cannot be removed and freed.  If C is subsequently removed
 * and freed, then A becomes a childless node and it becomes eligible for
 * removal, and will be removed and freed.
 *
 * Similarly, suppose /A is a registered path, and /A/B is also a registered
 * path, and that these are the only paths in the tree.  If B is removed and
 * freed, then even though A has become childless, it can't be freed because it
 * refers to a path that is still registered.
 *
 * @param subtree subtree from which to start the search, root for initial call
 * @param path path to subtree (same as _dbus_object_tree_unregister_and_unlock)
 * @param continue_removal_attempts pointer to a bool, #TRUE for initial call
 * @param unregister_function_out returns the found node's unregister_function
 * @param user_data_out returns the found node's user_data
 * @returns #TRUE if a registered node was found at path, #FALSE otherwise
 */
static dbus_bool_t
unregister_and_free_path_recurse
(DBusObjectSubtree                 *subtree,
 const char                       **path,
 dbus_bool_t                       *continue_removal_attempts,
 DBusObjectPathUnregisterFunction  *unregister_function_out,
 void                             **user_data_out)
{
  int i, j;

  _dbus_assert (continue_removal_attempts != NULL);
  _dbus_assert (*continue_removal_attempts);
  _dbus_assert (unregister_function_out != NULL);
  _dbus_assert (user_data_out != NULL);

  if (path[0] == NULL)
    return unregister_subtree (subtree, unregister_function_out, user_data_out);

  i = 0;
  j = subtree->n_subtrees;
  while (i < j)
    {
      int k, v;

      k = (i + j) / 2;
      v = strcmp (path[0], subtree->subtrees[k]->name);

      if (v == 0)
        {
          dbus_bool_t freed;
          freed = unregister_and_free_path_recurse (subtree->subtrees[k],
                                                    &path[1],
                                                    continue_removal_attempts,
                                                    unregister_function_out,
                                                    user_data_out);
          if (freed && *continue_removal_attempts)
            *continue_removal_attempts = attempt_child_removal (subtree, k);
          return freed;
        }
      else if (v < 0)
        {
          j = k;
        }
      else
        {
          i = k + 1;
        }
    }
  return FALSE;
}

/**
 * Unregisters an object subtree that was registered with the
 * same path.
 *
 * @param tree the global object tree
 * @param path path to the subtree (same as the one passed to _dbus_object_tree_register())
 */
void
_dbus_object_tree_unregister_and_unlock (DBusObjectTree          *tree,
                                         const char             **path)
{
  dbus_bool_t found_subtree;
  dbus_bool_t continue_removal_attempts;
  DBusObjectPathUnregisterFunction unregister_function;
  void *user_data;
  DBusConnection *connection;

  _dbus_assert (tree != NULL);
  _dbus_assert (path != NULL);

  continue_removal_attempts = TRUE;
  unregister_function = NULL;
  user_data = NULL;

  found_subtree = unregister_and_free_path_recurse (tree->root,
                                                    path,
                                                    &continue_removal_attempts,
                                                    &unregister_function,
                                                    &user_data);

#ifndef DBUS_DISABLE_CHECKS
  if (found_subtree == FALSE)
    {
      _dbus_warn ("Attempted to unregister path (path[0] = %s path[1] = %s) which isn't registered",
                  path[0] ? path[0] : "null",
                  (path[0] && path[1]) ? path[1] : "null");
      goto unlock;    
    }
#else
  _dbus_assert (found_subtree == TRUE);
#endif

unlock:
  connection = tree->connection;

  /* Unlock and call application code */
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
  if (connection)
#endif
    {
      _dbus_connection_ref_unlocked (connection);
      _dbus_verbose ("unlock\n");
      _dbus_connection_unlock (connection);
    }

  if (unregister_function)
    (* unregister_function) (connection, user_data);

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
  if (connection)
#endif
    dbus_connection_unref (connection);
}

static void
free_subtree_recurse (DBusConnection    *connection,
                      DBusObjectSubtree *subtree)
{
  /* Delete them from the end, for slightly
   * more robustness against odd reentrancy.
   */
  while (subtree->n_subtrees > 0)
    {
      DBusObjectSubtree *child;

      child = subtree->subtrees[subtree->n_subtrees - 1];
      subtree->subtrees[subtree->n_subtrees - 1] = NULL;
      subtree->n_subtrees -= 1;
      child->parent = NULL;

      free_subtree_recurse (connection, child);
    }

  /* Call application code */
  if (subtree->unregister_function)
    (* subtree->unregister_function) (connection,
				      subtree->user_data);

  subtree->message_function = NULL;
  subtree->unregister_function = NULL;
  subtree->user_data = NULL;

  /* Now free ourselves */
  _dbus_object_subtree_unref (subtree);
}

/**
 * Free all the handlers in the tree. Lock on tree's connection
 * must not be held.
 *
 * @param tree the object tree
 */
void
_dbus_object_tree_free_all_unlocked (DBusObjectTree *tree)
{
  if (tree->root)
    free_subtree_recurse (tree->connection,
                          tree->root);
  tree->root = NULL;
}

static dbus_bool_t
_dbus_object_tree_list_registered_unlocked (DBusObjectTree *tree,
                                            const char    **parent_path,
                                            char         ***child_entries)
{
  DBusObjectSubtree *subtree;
  char **retval;
  
  _dbus_assert (parent_path != NULL);
  _dbus_assert (child_entries != NULL);

  *child_entries = NULL;
  
  subtree = lookup_subtree (tree, parent_path);
  if (subtree == NULL)
    {
      retval = dbus_new0 (char *, 1);
    }
  else
    {
      int i;
      retval = dbus_new0 (char*, subtree->n_subtrees + 1);
      if (retval == NULL)
        goto out;
      i = 0;
      while (i < subtree->n_subtrees)
        {
          retval[i] = _dbus_strdup (subtree->subtrees[i]->name);
          if (retval[i] == NULL)
            {
              dbus_free_string_array (retval);
              retval = NULL;
              goto out;
            }
          ++i;
        }
    }

 out:
    
  *child_entries = retval;
  return retval != NULL;
}

static DBusHandlerResult
handle_default_introspect_and_unlock (DBusObjectTree          *tree,
                                      DBusMessage             *message,
                                      const char             **path)
{
  DBusString xml;
  DBusHandlerResult result;
  char **children;
  int i;
  DBusMessage *reply;
  DBusMessageIter iter;
  const char *v_STRING;
  dbus_bool_t already_unlocked;

  /* We have the connection lock here */

  already_unlocked = FALSE;
  
  _dbus_verbose (" considering default Introspect() handler...\n");

  reply = NULL;
  
  if (!dbus_message_is_method_call (message,
                                    DBUS_INTERFACE_INTROSPECTABLE,
                                    "Introspect"))
    {
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
      if (tree->connection)
#endif
        {
          _dbus_verbose ("unlock\n");
          _dbus_connection_unlock (tree->connection);
        }
      
      return DBUS_HANDLER_RESULT_NOT_YET_HANDLED;
    }

  _dbus_verbose (" using default Introspect() handler!\n");
  
  if (!_dbus_string_init (&xml))
    {
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
      if (tree->connection)
#endif
        {
          _dbus_verbose ("unlock\n");
          _dbus_connection_unlock (tree->connection);
        }

      return DBUS_HANDLER_RESULT_NEED_MEMORY;
    }

  result = DBUS_HANDLER_RESULT_NEED_MEMORY;

  children = NULL;
  if (!_dbus_object_tree_list_registered_unlocked (tree, path, &children))
    goto out;

  if (!_dbus_string_append (&xml, DBUS_INTROSPECT_1_0_XML_DOCTYPE_DECL_NODE))
    goto out;
  
  if (!_dbus_string_append (&xml, "<node>\n"))
    goto out;

  i = 0;
  while (children[i] != NULL)
    {
      if (!_dbus_string_append_printf (&xml, "  <node name=\"%s\"/>\n",
                                       children[i]))
        goto out;

      ++i;
    }

  if (!_dbus_string_append (&xml, "</node>\n"))
    goto out;

  reply = dbus_message_new_method_return (message);
  if (reply == NULL)
    goto out;

  dbus_message_iter_init_append (reply, &iter);
  v_STRING = _dbus_string_get_const_data (&xml);
  if (!dbus_message_iter_append_basic (&iter, DBUS_TYPE_STRING, &v_STRING))
    goto out;
  
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
  if (tree->connection)
#endif
    {
      already_unlocked = TRUE;
      
      if (!_dbus_connection_send_and_unlock (tree->connection, reply, NULL))
        goto out;
    }
  
  result = DBUS_HANDLER_RESULT_HANDLED;
  
 out:
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
  if (tree->connection)
#endif
    {
      if (!already_unlocked)
        {
          _dbus_verbose ("unlock\n");
          _dbus_connection_unlock (tree->connection);
        }
    }
  
  _dbus_string_free (&xml);
  dbus_free_string_array (children);
  if (reply)
    dbus_message_unref (reply);
  
  return result;
}

/**
 * Tries to dispatch a message by directing it to handler for the
 * object path listed in the message header, if any. Messages are
 * dispatched first to the registered handler that matches the largest
 * number of path elements; that is, message to /foo/bar/baz would go
 * to the handler for /foo/bar before the one for /foo.
 *
 * @todo thread problems
 *
 * @param tree the global object tree
 * @param message the message to dispatch
 * @param found_object return location for the object
 * @returns whether message was handled successfully
 */
DBusHandlerResult
_dbus_object_tree_dispatch_and_unlock (DBusObjectTree          *tree,
                                       DBusMessage             *message,
                                       dbus_bool_t             *found_object)
{
  char **path;
  dbus_bool_t exact_match;
  DBusList *list;
  DBusList *link;
  DBusHandlerResult result;
  DBusObjectSubtree *subtree;
  
#if 0
  _dbus_verbose ("Dispatch of message by object path\n");
#endif
  
  path = NULL;
  if (!dbus_message_get_path_decomposed (message, &path))
    {
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
      if (tree->connection)
#endif
        {
          _dbus_verbose ("unlock\n");
          _dbus_connection_unlock (tree->connection);
        }
      
      _dbus_verbose ("No memory to get decomposed path\n");

      return DBUS_HANDLER_RESULT_NEED_MEMORY;
    }

  if (path == NULL)
    {
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
      if (tree->connection)
#endif
        {
          _dbus_verbose ("unlock\n");
          _dbus_connection_unlock (tree->connection);
        }
      
      _dbus_verbose ("No path field in message\n");
      return DBUS_HANDLER_RESULT_NOT_YET_HANDLED;
    }
  
  /* Find the deepest path that covers the path in the message */
  subtree = find_handler (tree, (const char**) path, &exact_match);
  
  if (found_object)
    *found_object = !!subtree;

  /* Build a list of all paths that cover the path in the message */

  list = NULL;

  while (subtree != NULL)
    {
      if (subtree->message_function != NULL && (exact_match || subtree->invoke_as_fallback))
        {
          _dbus_object_subtree_ref (subtree);

          /* run deepest paths first */
          if (!_dbus_list_append (&list, subtree))
            {
              result = DBUS_HANDLER_RESULT_NEED_MEMORY;
              _dbus_object_subtree_unref (subtree);
              goto free_and_return;
            }
        }

      exact_match = FALSE;
      subtree = subtree->parent;
    }

  _dbus_verbose ("%d handlers in the path tree for this message\n",
                 _dbus_list_get_length (&list));

  /* Invoke each handler in the list */

  result = DBUS_HANDLER_RESULT_NOT_YET_HANDLED;

  link = _dbus_list_get_first_link (&list);
  while (link != NULL)
    {
      DBusList *next = _dbus_list_get_next_link (&list, link);
      subtree = link->data;

      /* message_function is NULL if we're unregistered
       * due to reentrancy
       */
      if (subtree->message_function)
        {
          DBusObjectPathMessageFunction message_function;
          void *user_data;

          message_function = subtree->message_function;
          user_data = subtree->user_data;

#if 0
          _dbus_verbose ("  (invoking a handler)\n");
#endif
          
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
          if (tree->connection)
#endif
            {
              _dbus_verbose ("unlock\n");
              _dbus_connection_unlock (tree->connection);
            }

          /* FIXME you could unregister the subtree in another thread
           * before we invoke the callback, and I can't figure out a
           * good way to solve this.
           */

          result = (* message_function) (tree->connection,
                                         message,
                                         user_data);

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
          if (tree->connection)
#endif
            _dbus_connection_lock (tree->connection);

          if (result != DBUS_HANDLER_RESULT_NOT_YET_HANDLED)
            goto free_and_return;
        }

      link = next;
    }

 free_and_return:

  if (result == DBUS_HANDLER_RESULT_NOT_YET_HANDLED)
    {
      /* This hardcoded default handler does a minimal Introspect()
       */
      result = handle_default_introspect_and_unlock (tree, message,
                                                     (const char**) path);
    }
  else
    {
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
      if (tree->connection)
#endif
        {
          _dbus_verbose ("unlock\n");
          _dbus_connection_unlock (tree->connection);
        }
    }
  
  while (list != NULL)
    {
      link = _dbus_list_get_first_link (&list);
      _dbus_object_subtree_unref (link->data);
      _dbus_list_remove_link (&list, link);
    }
  
  dbus_free_string_array (path);

  return result;
}

/**
 * Looks up the data passed to _dbus_object_tree_register() for a
 * handler at the given path.
 *
 * @param tree the global object tree
 * @param path NULL-terminated array of path elements giving path to subtree
 * @returns the object's user_data or #NULL if none found
 */
void*
_dbus_object_tree_get_user_data_unlocked (DBusObjectTree *tree,
                                          const char    **path)
{
  dbus_bool_t exact_match;
  DBusObjectSubtree *subtree;

  _dbus_assert (tree != NULL);
  _dbus_assert (path != NULL);
  
  /* Find the deepest path that covers the path in the message */
  subtree = find_handler (tree, (const char**) path, &exact_match);

  if ((subtree == NULL) || !exact_match)
    {
      _dbus_verbose ("No object at specified path found\n");
      return NULL;
    }

  return subtree->user_data;
}

/**
 * Allocates a subtree object.
 *
 * @param name name to duplicate.
 * @returns newly-allocated subtree
 */
static DBusObjectSubtree*
allocate_subtree_object (const char *name)
{
  int len;
  DBusObjectSubtree *subtree;
  const size_t front_padding = _DBUS_STRUCT_OFFSET (DBusObjectSubtree, name);

  _dbus_assert (name != NULL);

  len = strlen (name);

  subtree = dbus_malloc0 (MAX (front_padding + (len + 1), sizeof (DBusObjectSubtree)));

  if (subtree == NULL)
    return NULL;

  memcpy (subtree->name, name, len + 1);

  return subtree;
}

static DBusObjectSubtree*
_dbus_object_subtree_new (const char                  *name,
                          const DBusObjectPathVTable  *vtable,
                          void                        *user_data)
{
  DBusObjectSubtree *subtree;

  subtree = allocate_subtree_object (name);
  if (subtree == NULL)
    goto oom;

  _dbus_assert (name != NULL);

  subtree->parent = NULL;

  if (vtable)
    {
      subtree->message_function = vtable->message_function;
      subtree->unregister_function = vtable->unregister_function;
    }
  else
    {
      subtree->message_function = NULL;
      subtree->unregister_function = NULL;
    }

  subtree->user_data = user_data;
  _dbus_atomic_inc (&subtree->refcount);
  subtree->subtrees = NULL;
  subtree->n_subtrees = 0;
  subtree->max_subtrees = 0;
  subtree->invoke_as_fallback = FALSE;

  return subtree;

 oom:
  return NULL;
}

static DBusObjectSubtree *
_dbus_object_subtree_ref (DBusObjectSubtree *subtree)
{
#ifdef DBUS_DISABLE_ASSERT
  _dbus_atomic_inc (&subtree->refcount);
#else
  dbus_int32_t old_value;

  old_value = _dbus_atomic_inc (&subtree->refcount);
  _dbus_assert (old_value > 0);
#endif

  return subtree;
}

static void
_dbus_object_subtree_unref (DBusObjectSubtree *subtree)
{
  dbus_int32_t old_value;

  old_value = _dbus_atomic_dec (&subtree->refcount);
  _dbus_assert (old_value > 0);

  if (old_value == 1)
    {
      _dbus_assert (subtree->unregister_function == NULL);
      _dbus_assert (subtree->message_function == NULL);

      dbus_free (subtree->subtrees);
      dbus_free (subtree);
    }
}

/**
 * Lists the registered fallback handlers and object path handlers at
 * the given parent_path. The returned array should be freed with
 * dbus_free_string_array().
 *
 * @param tree the object tree
 * @param parent_path the path to list the child handlers of
 * @param child_entries returns #NULL-terminated array of children
 * @returns #FALSE if no memory to allocate the child entries
 */
dbus_bool_t
_dbus_object_tree_list_registered_and_unlock (DBusObjectTree *tree,
                                              const char    **parent_path,
                                              char         ***child_entries)
{
  dbus_bool_t result;

  result = _dbus_object_tree_list_registered_unlocked (tree,
                                                       parent_path,
                                                       child_entries);
  
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
  if (tree->connection)
#endif
    {
      _dbus_verbose ("unlock\n");
      _dbus_connection_unlock (tree->connection);
    }

  return result;
}


/** Set to 1 to get a bunch of spew about disassembling the path string */
#define VERBOSE_DECOMPOSE 0

/**
 * Decompose an object path.  A path of just "/" is
 * represented as an empty vector of strings.
 * The path need not be nul terminated.
 * 
 * @param data the path data
 * @param len  the length of the path string
 * @param path address to store new object path
 * @param path_len length of stored path
 */
dbus_bool_t
_dbus_decompose_path (const char*     data,
                      int             len,
                      char         ***path,
                      int            *path_len)
{
  char **retval;
  int n_components;
  int i, j, comp;

  _dbus_assert (data != NULL);
  _dbus_assert (path != NULL);
  
#if VERBOSE_DECOMPOSE
  _dbus_verbose ("Decomposing path \"%s\"\n",
                 data);
#endif
  
  n_components = 0;
  if (len > 1) /* if path is not just "/" */
    {
      i = 0;
      while (i < len)
        {
          _dbus_assert (data[i] != '\0');
          if (data[i] == '/')
            n_components += 1;
          ++i;
        }
    }
  
  retval = dbus_new0 (char*, n_components + 1);

  if (retval == NULL)
    return FALSE;

  comp = 0;
  if (n_components == 0)
    i = 1;
  else
    i = 0;
  while (comp < n_components)
    {
      _dbus_assert (i < len);
      
      if (data[i] == '/')
        ++i;
      j = i;

      while (j < len && data[j] != '/')
        ++j;

      /* Now [i, j) is the path component */
      _dbus_assert (i < j);
      _dbus_assert (data[i] != '/');
      _dbus_assert (j == len || data[j] == '/');

#if VERBOSE_DECOMPOSE
      _dbus_verbose ("  (component in [%d,%d))\n",
                     i, j);
#endif
      
      retval[comp] = _dbus_memdup (&data[i], j - i + 1);
      if (retval[comp] == NULL)
        {
          dbus_free_string_array (retval);
          return FALSE;
        }
      retval[comp][j-i] = '\0';
#if VERBOSE_DECOMPOSE
      _dbus_verbose ("  (component %d = \"%s\")\n",
                     comp, retval[comp]);
#endif

      ++comp;
      i = j;
    }
  _dbus_assert (i == len);
  
  *path = retval;
  if (path_len)
    *path_len = n_components;
  
  return TRUE;
}

/** @} */

static char*
flatten_path (const char **path)
{
  DBusString str;
  char *s;

  if (!_dbus_string_init (&str))
    return NULL;

  if (path[0] == NULL)
    {
      if (!_dbus_string_append_byte (&str, '/'))
        goto nomem;
    }
  else
    {
      int i;
      
      i = 0;
      while (path[i])
        {
          if (!_dbus_string_append_byte (&str, '/'))
            goto nomem;
          
          if (!_dbus_string_append (&str, path[i]))
            goto nomem;
          
          ++i;
        }
    }

  if (!_dbus_string_steal_data (&str, &s))
    goto nomem;

  _dbus_string_free (&str);

  return s;

 nomem:
  _dbus_string_free (&str);
  return NULL;
}


#ifdef DBUS_ENABLE_EMBEDDED_TESTS

#ifndef DOXYGEN_SHOULD_SKIP_THIS

#include "dbus-test.h"
#include <stdio.h>

typedef enum 
{
  STR_EQUAL,
  STR_PREFIX,
  STR_DIFFERENT
} StrComparison;

/* Returns TRUE if container is a parent of child
 */
static StrComparison
path_contains (const char **container,
               const char **child)
{
  int i;

  i = 0;
  while (child[i] != NULL)
    {
      int v;

      if (container[i] == NULL)
        return STR_PREFIX; /* container ran out, child continues;
                            * thus the container is a parent of the
                            * child.
                            */

      _dbus_assert (container[i] != NULL);
      _dbus_assert (child[i] != NULL);

      v = strcmp (container[i], child[i]);

      if (v != 0)
        return STR_DIFFERENT; /* they overlap until here and then are different,
                               * not overlapping
                               */

      ++i;
    }

  /* Child ran out; if container also did, they are equal;
   * otherwise, the child is a parent of the container.
   */
  if (container[i] == NULL)
    return STR_EQUAL;
  else
    return STR_DIFFERENT;
}

#if 0
static void
spew_subtree_recurse (DBusObjectSubtree *subtree,
                      int                indent)
{
  int i;

  i = 0;
  while (i < indent)
    {
      _dbus_verbose (" ");
      ++i;
    }

  _dbus_verbose ("%s (%d children)\n",
                 subtree->name, subtree->n_subtrees);

  i = 0;
  while (i < subtree->n_subtrees)
    {
      spew_subtree_recurse (subtree->subtrees[i], indent + 2);

      ++i;
    }
}

static void
spew_tree (DBusObjectTree *tree)
{
  spew_subtree_recurse (tree->root, 0);
}
#endif

/**
 * Callback data used in tests
 */
typedef struct
{
  const char **path; /**< Path */
  dbus_bool_t handler_fallback; /**< true if the handler may be called as fallback */
  dbus_bool_t message_handled; /**< Gets set to true if message handler called */
  dbus_bool_t handler_unregistered; /**< gets set to true if handler is unregistered */
} TreeTestData;


static void
test_unregister_function (DBusConnection  *connection,
                          void            *user_data)
{
  TreeTestData *ttd = user_data;

  ttd->handler_unregistered = TRUE;
}

static DBusHandlerResult
test_message_function (DBusConnection  *connection,
                       DBusMessage     *message,
                       void            *user_data)
{
  TreeTestData *ttd = user_data;

  ttd->message_handled = TRUE;

  return DBUS_HANDLER_RESULT_NOT_YET_HANDLED;
}

static dbus_bool_t
do_register (DBusObjectTree *tree,
             const char    **path,
             dbus_bool_t     fallback,
             int             i,
             TreeTestData   *tree_test_data)
{
  DBusObjectPathVTable vtable = { test_unregister_function,
                                  test_message_function, NULL };

  tree_test_data[i].message_handled = FALSE;
  tree_test_data[i].handler_unregistered = FALSE;
  tree_test_data[i].handler_fallback = fallback;
  tree_test_data[i].path = path;

  if (!_dbus_object_tree_register (tree, fallback, path,
                                   &vtable,
                                   &tree_test_data[i],
                                   NULL))
    return FALSE;

  _dbus_assert (_dbus_object_tree_get_user_data_unlocked (tree, path) ==
                &tree_test_data[i]);
  
  return TRUE;
}

static dbus_bool_t
do_test_dispatch (DBusObjectTree *tree,
                  const char    **path,
                  int             i,
                  TreeTestData   *tree_test_data,
                  int             n_test_data)
{
  DBusMessage *message;
  int j;
  DBusHandlerResult result;
  char *flat;

  message = NULL;
  
  flat = flatten_path (path);
  if (flat == NULL)
    goto oom;

  message = dbus_message_new_method_call (NULL,
                                          flat,
                                          "org.freedesktop.TestInterface",
                                          "Foo");
  dbus_free (flat);
  if (message == NULL)
    goto oom;

  j = 0;
  while (j < n_test_data)
    {
      tree_test_data[j].message_handled = FALSE;
      ++j;
    }

  result = _dbus_object_tree_dispatch_and_unlock (tree, message, NULL);
  if (result == DBUS_HANDLER_RESULT_NEED_MEMORY)
    goto oom;

  _dbus_assert (tree_test_data[i].message_handled);

  j = 0;
  while (j < n_test_data)
    {
      if (tree_test_data[j].message_handled)
	{
	  if (tree_test_data[j].handler_fallback)
	    _dbus_assert (path_contains (tree_test_data[j].path,
					 path) != STR_DIFFERENT);
	  else
	    _dbus_assert (path_contains (tree_test_data[j].path, path) == STR_EQUAL);
	}
      else
	{
	  if (tree_test_data[j].handler_fallback)
	    _dbus_assert (path_contains (tree_test_data[j].path,
					 path) == STR_DIFFERENT);
	  else
	    _dbus_assert (path_contains (tree_test_data[j].path, path) != STR_EQUAL);
	}

      ++j;
    }

  dbus_message_unref (message);

  return TRUE;

 oom:
  if (message)
    dbus_message_unref (message);
  return FALSE;
}

typedef struct
{
  const char *path;
  const char *result[20];
} DecomposePathTest;

static DecomposePathTest decompose_tests[] = {
  { "/foo", { "foo", NULL } },
  { "/foo/bar", { "foo", "bar", NULL } },
  { "/", { NULL } },
  { "/a/b", { "a", "b", NULL } },
  { "/a/b/c", { "a", "b", "c", NULL } },
  { "/a/b/c/d", { "a", "b", "c", "d", NULL } },
  { "/foo/bar/q", { "foo", "bar", "q", NULL } },
  { "/foo/bar/this/is/longer", { "foo", "bar", "this", "is", "longer", NULL } }
};

/* Return TRUE on success, FALSE on OOM, die with an assertion failure
 * on failure. */
static dbus_bool_t
run_decompose_tests (void)
{
  int i;

  i = 0;
  while (i < _DBUS_N_ELEMENTS (decompose_tests))
    {
      char **result;
      int    result_len;
      int    expected_len;

      if (!_dbus_decompose_path (decompose_tests[i].path,
                                 strlen (decompose_tests[i].path),
                                 &result, &result_len))
        return FALSE;

      expected_len = _dbus_string_array_length (decompose_tests[i].result);
      
      if (result_len != (int) _dbus_string_array_length ((const char**)result) ||
          expected_len != result_len ||
          path_contains (decompose_tests[i].result,
                         (const char**) result) != STR_EQUAL)
        {
          int real_len = _dbus_string_array_length ((const char**)result);
          _dbus_warn ("Expected decompose of %s to have len %d, returned %d, appears to have %d",
                      decompose_tests[i].path, expected_len, result_len,
                      real_len);
          _dbus_warn ("Decompose resulted in elements: { ");
          i = 0;
          while (i < real_len)
            {
              _dbus_warn ("\"%s\"%s", result[i],
                          (i + 1) == real_len ? "" : ", ");
              ++i;
            }
          _dbus_warn ("}");
          _dbus_test_fatal ("path decompose failed");
        }

      dbus_free_string_array (result);

      ++i;
    }
  
  return TRUE;
}

static DBusObjectSubtree*
find_subtree_registered_or_unregistered (DBusObjectTree *tree,
                                         const char    **path)
{
#if VERBOSE_FIND
  _dbus_verbose ("Looking for exact subtree, registered or unregistered\n");
#endif

  return find_subtree_recurse (tree->root, path, FALSE, NULL, NULL);
}

/* Returns TRUE if the right thing happens, but the right thing might
 * be OOM. */
static dbus_bool_t
object_tree_test_iteration (void        *data,
                            dbus_bool_t  have_memory)
{
  const char *path0[] = { NULL };
  const char *path1[] = { "foo", NULL };
  const char *path2[] = { "foo", "bar", NULL };
  const char *path3[] = { "foo", "bar", "baz", NULL };
  const char *path4[] = { "foo", "bar", "boo", NULL };
  const char *path5[] = { "blah", NULL };
  const char *path6[] = { "blah", "boof", NULL };
  const char *path7[] = { "blah", "boof", "this", "is", "really", "long", NULL };
  const char *path8[] = { "childless", NULL };
  const char *path9[] = { "blah", "a", NULL };
  const char *path10[] = { "blah", "b", NULL };
  const char *path11[] = { "blah", "c", NULL };
  const char *path12[] = { "blah", "a", "d", NULL };
  const char *path13[] = { "blah", "b", "d", NULL };
  const char *path14[] = { "blah", "c", "d", NULL };
  DBusObjectPathVTable test_vtable = { NULL, test_message_function, NULL };
  DBusObjectTree *tree;
  TreeTestData tree_test_data[9];
  int i;
  dbus_bool_t exact_match;

  if (!run_decompose_tests ())
    return TRUE; /* OOM is OK */
  
  tree = NULL;

  tree = _dbus_object_tree_new (NULL);
  if (tree == NULL)
    goto out;

  if (!do_register (tree, path0, TRUE, 0, tree_test_data))
    goto out;

  _dbus_assert (find_subtree (tree, path0, NULL));
  _dbus_assert (!find_subtree (tree, path1, NULL));
  _dbus_assert (!find_subtree (tree, path2, NULL));
  _dbus_assert (!find_subtree (tree, path3, NULL));
  _dbus_assert (!find_subtree (tree, path4, NULL));
  _dbus_assert (!find_subtree (tree, path5, NULL));
  _dbus_assert (!find_subtree (tree, path6, NULL));
  _dbus_assert (!find_subtree (tree, path7, NULL));
  _dbus_assert (!find_subtree (tree, path8, NULL));

  _dbus_assert (find_handler (tree, path0, &exact_match) && exact_match);
  _dbus_assert (find_handler (tree, path1, &exact_match) == tree->root && !exact_match);
  _dbus_assert (find_handler (tree, path2, &exact_match) == tree->root && !exact_match);
  _dbus_assert (find_handler (tree, path3, &exact_match) == tree->root && !exact_match);
  _dbus_assert (find_handler (tree, path4, &exact_match) == tree->root && !exact_match);
  _dbus_assert (find_handler (tree, path5, &exact_match) == tree->root && !exact_match);
  _dbus_assert (find_handler (tree, path6, &exact_match) == tree->root && !exact_match);
  _dbus_assert (find_handler (tree, path7, &exact_match) == tree->root && !exact_match);
  _dbus_assert (find_handler (tree, path8, &exact_match) == tree->root && !exact_match);
  
  if (!do_register (tree, path1, TRUE, 1, tree_test_data))
    goto out;

  _dbus_assert (find_subtree (tree, path0, NULL));
  _dbus_assert (find_subtree (tree, path1, NULL));
  _dbus_assert (!find_subtree (tree, path2, NULL));
  _dbus_assert (!find_subtree (tree, path3, NULL));
  _dbus_assert (!find_subtree (tree, path4, NULL));
  _dbus_assert (!find_subtree (tree, path5, NULL));
  _dbus_assert (!find_subtree (tree, path6, NULL));
  _dbus_assert (!find_subtree (tree, path7, NULL));
  _dbus_assert (!find_subtree (tree, path8, NULL));

  _dbus_assert (find_handler (tree, path0, &exact_match) &&  exact_match);
  _dbus_assert (find_handler (tree, path1, &exact_match) &&  exact_match);
  _dbus_assert (find_handler (tree, path2, &exact_match) && !exact_match);
  _dbus_assert (find_handler (tree, path3, &exact_match) && !exact_match);
  _dbus_assert (find_handler (tree, path4, &exact_match) && !exact_match);
  _dbus_assert (find_handler (tree, path5, &exact_match) == tree->root && !exact_match);
  _dbus_assert (find_handler (tree, path6, &exact_match) == tree->root && !exact_match);
  _dbus_assert (find_handler (tree, path7, &exact_match) == tree->root && !exact_match);
  _dbus_assert (find_handler (tree, path8, &exact_match) == tree->root && !exact_match);

  if (!do_register (tree, path2, TRUE, 2, tree_test_data))
    goto out;

  _dbus_assert (find_subtree (tree, path1, NULL));
  _dbus_assert (find_subtree (tree, path2, NULL));
  _dbus_assert (!find_subtree (tree, path3, NULL));
  _dbus_assert (!find_subtree (tree, path4, NULL));
  _dbus_assert (!find_subtree (tree, path5, NULL));
  _dbus_assert (!find_subtree (tree, path6, NULL));
  _dbus_assert (!find_subtree (tree, path7, NULL));
  _dbus_assert (!find_subtree (tree, path8, NULL));

  if (!do_register (tree, path3, TRUE, 3, tree_test_data))
    goto out;

  _dbus_assert (find_subtree (tree, path0, NULL));
  _dbus_assert (find_subtree (tree, path1, NULL));
  _dbus_assert (find_subtree (tree, path2, NULL));
  _dbus_assert (find_subtree (tree, path3, NULL));
  _dbus_assert (!find_subtree (tree, path4, NULL));
  _dbus_assert (!find_subtree (tree, path5, NULL));
  _dbus_assert (!find_subtree (tree, path6, NULL));
  _dbus_assert (!find_subtree (tree, path7, NULL));
  _dbus_assert (!find_subtree (tree, path8, NULL));
  
  if (!do_register (tree, path4, TRUE, 4, tree_test_data))
    goto out;

  _dbus_assert (find_subtree (tree, path0, NULL));
  _dbus_assert (find_subtree (tree, path1, NULL));
  _dbus_assert (find_subtree (tree, path2, NULL));
  _dbus_assert (find_subtree (tree, path3, NULL));  
  _dbus_assert (find_subtree (tree, path4, NULL));
  _dbus_assert (!find_subtree (tree, path5, NULL));
  _dbus_assert (!find_subtree (tree, path6, NULL));
  _dbus_assert (!find_subtree (tree, path7, NULL));
  _dbus_assert (!find_subtree (tree, path8, NULL));
  
  if (!do_register (tree, path5, TRUE, 5, tree_test_data))
    goto out;

  _dbus_assert (find_subtree (tree, path0, NULL));
  _dbus_assert (find_subtree (tree, path1, NULL));
  _dbus_assert (find_subtree (tree, path2, NULL));
  _dbus_assert (find_subtree (tree, path3, NULL));
  _dbus_assert (find_subtree (tree, path4, NULL));
  _dbus_assert (find_subtree (tree, path5, NULL));
  _dbus_assert (!find_subtree (tree, path6, NULL));
  _dbus_assert (!find_subtree (tree, path7, NULL));
  _dbus_assert (!find_subtree (tree, path8, NULL));

  _dbus_assert (find_handler (tree, path0, &exact_match) == tree->root &&  exact_match);
  _dbus_assert (find_handler (tree, path1, &exact_match) != tree->root &&  exact_match);
  _dbus_assert (find_handler (tree, path2, &exact_match) != tree->root &&  exact_match);
  _dbus_assert (find_handler (tree, path3, &exact_match) != tree->root &&  exact_match);
  _dbus_assert (find_handler (tree, path4, &exact_match) != tree->root &&  exact_match);
  _dbus_assert (find_handler (tree, path5, &exact_match) != tree->root &&  exact_match);
  _dbus_assert (find_handler (tree, path6, &exact_match) != tree->root && !exact_match);
  _dbus_assert (find_handler (tree, path7, &exact_match) != tree->root && !exact_match);
  _dbus_assert (find_handler (tree, path8, &exact_match) == tree->root && !exact_match);

  if (!do_register (tree, path6, TRUE, 6, tree_test_data))
    goto out;

  _dbus_assert (find_subtree (tree, path0, NULL));
  _dbus_assert (find_subtree (tree, path1, NULL));
  _dbus_assert (find_subtree (tree, path2, NULL));
  _dbus_assert (find_subtree (tree, path3, NULL));
  _dbus_assert (find_subtree (tree, path4, NULL));
  _dbus_assert (find_subtree (tree, path5, NULL));
  _dbus_assert (find_subtree (tree, path6, NULL));
  _dbus_assert (!find_subtree (tree, path7, NULL));
  _dbus_assert (!find_subtree (tree, path8, NULL));

  if (!do_register (tree, path7, TRUE, 7, tree_test_data))
    goto out;

  _dbus_assert (find_subtree (tree, path0, NULL));
  _dbus_assert (find_subtree (tree, path1, NULL));
  _dbus_assert (find_subtree (tree, path2, NULL));
  _dbus_assert (find_subtree (tree, path3, NULL));
  _dbus_assert (find_subtree (tree, path4, NULL));
  _dbus_assert (find_subtree (tree, path5, NULL));
  _dbus_assert (find_subtree (tree, path6, NULL));
  _dbus_assert (find_subtree (tree, path7, NULL));
  _dbus_assert (!find_subtree (tree, path8, NULL));

  if (!do_register (tree, path8, TRUE, 8, tree_test_data))
    goto out;

  _dbus_assert (find_subtree (tree, path0, NULL));
  _dbus_assert (find_subtree (tree, path1, NULL));
  _dbus_assert (find_subtree (tree, path2, NULL));
  _dbus_assert (find_subtree (tree, path3, NULL));
  _dbus_assert (find_subtree (tree, path4, NULL));
  _dbus_assert (find_subtree (tree, path5, NULL));
  _dbus_assert (find_subtree (tree, path6, NULL));
  _dbus_assert (find_subtree (tree, path7, NULL));
  _dbus_assert (find_subtree (tree, path8, NULL));

  _dbus_assert (find_handler (tree, path0, &exact_match) == tree->root &&  exact_match);
  _dbus_assert (find_handler (tree, path1, &exact_match) != tree->root && exact_match);
  _dbus_assert (find_handler (tree, path2, &exact_match) != tree->root && exact_match);
  _dbus_assert (find_handler (tree, path3, &exact_match) != tree->root && exact_match);
  _dbus_assert (find_handler (tree, path4, &exact_match) != tree->root && exact_match);
  _dbus_assert (find_handler (tree, path5, &exact_match) != tree->root && exact_match);
  _dbus_assert (find_handler (tree, path6, &exact_match) != tree->root && exact_match);
  _dbus_assert (find_handler (tree, path7, &exact_match) != tree->root && exact_match);
  _dbus_assert (find_handler (tree, path8, &exact_match) != tree->root && exact_match);
  
  /* test the list_registered function */

  {
    const char *root[] = { NULL };
    char **child_entries;
    int nb;

    _dbus_object_tree_list_registered_unlocked (tree, path1, &child_entries);
    if (child_entries != NULL)
      {
	nb = _dbus_string_array_length ((const char**)child_entries);
	_dbus_assert (nb == 1);
	dbus_free_string_array (child_entries);
      }

    _dbus_object_tree_list_registered_unlocked (tree, path2, &child_entries);
    if (child_entries != NULL)
      {
	nb = _dbus_string_array_length ((const char**)child_entries);
	_dbus_assert (nb == 2);
	dbus_free_string_array (child_entries);
      }

    _dbus_object_tree_list_registered_unlocked (tree, path8, &child_entries);
    if (child_entries != NULL)
      {
	nb = _dbus_string_array_length ((const char**)child_entries);
	_dbus_assert (nb == 0);
	dbus_free_string_array (child_entries);
      }

    _dbus_object_tree_list_registered_unlocked (tree, root, &child_entries);
    if (child_entries != NULL)
      {
	nb = _dbus_string_array_length ((const char**)child_entries);
	_dbus_assert (nb == 3);
	dbus_free_string_array (child_entries);
      }
  }

  /* Check that destroying tree calls unregister funcs */
  _dbus_object_tree_unref (tree);

  i = 0;
  while (i < (int) _DBUS_N_ELEMENTS (tree_test_data))
    {
      _dbus_assert (tree_test_data[i].handler_unregistered);
      _dbus_assert (!tree_test_data[i].message_handled);
      ++i;
    }

  /* Now start again and try the individual unregister function */
  tree = _dbus_object_tree_new (NULL);
  if (tree == NULL)
    goto out;

  if (!do_register (tree, path0, TRUE, 0, tree_test_data))
    goto out;
  if (!do_register (tree, path1, TRUE, 1, tree_test_data))
    goto out;
  if (!do_register (tree, path2, TRUE, 2, tree_test_data))
    goto out;
  if (!do_register (tree, path3, TRUE, 3, tree_test_data))
    goto out;
  if (!do_register (tree, path4, TRUE, 4, tree_test_data))
    goto out;
  if (!do_register (tree, path5, TRUE, 5, tree_test_data))
    goto out;
  if (!do_register (tree, path6, TRUE, 6, tree_test_data))
    goto out;
  if (!do_register (tree, path7, TRUE, 7, tree_test_data))
    goto out;
  if (!do_register (tree, path8, TRUE, 8, tree_test_data))
    goto out;

  _dbus_object_tree_unregister_and_unlock (tree, path0);
  _dbus_assert (_dbus_object_tree_get_user_data_unlocked (tree, path0) == NULL);

  _dbus_assert (!find_subtree (tree, path0, NULL));
  _dbus_assert (find_subtree (tree, path1, NULL));
  _dbus_assert (find_subtree (tree, path2, NULL));
  _dbus_assert (find_subtree (tree, path3, NULL));
  _dbus_assert (find_subtree (tree, path4, NULL));
  _dbus_assert (find_subtree (tree, path5, NULL));
  _dbus_assert (find_subtree (tree, path6, NULL));
  _dbus_assert (find_subtree (tree, path7, NULL));
  _dbus_assert (find_subtree (tree, path8, NULL));
  
  _dbus_object_tree_unregister_and_unlock (tree, path1);
  _dbus_assert (_dbus_object_tree_get_user_data_unlocked (tree, path1) == NULL);

  _dbus_assert (!find_subtree (tree, path0, NULL));
  _dbus_assert (!find_subtree (tree, path1, NULL));
  _dbus_assert (find_subtree (tree, path2, NULL));
  _dbus_assert (find_subtree (tree, path3, NULL));
  _dbus_assert (find_subtree (tree, path4, NULL));
  _dbus_assert (find_subtree (tree, path5, NULL));
  _dbus_assert (find_subtree (tree, path6, NULL));
  _dbus_assert (find_subtree (tree, path7, NULL));
  _dbus_assert (find_subtree (tree, path8, NULL));

  _dbus_object_tree_unregister_and_unlock (tree, path2);
  _dbus_assert (_dbus_object_tree_get_user_data_unlocked (tree, path2) == NULL);

  _dbus_assert (!find_subtree (tree, path0, NULL));
  _dbus_assert (!find_subtree (tree, path1, NULL));
  _dbus_assert (!find_subtree (tree, path2, NULL));
  _dbus_assert (find_subtree (tree, path3, NULL));
  _dbus_assert (find_subtree (tree, path4, NULL));
  _dbus_assert (find_subtree (tree, path5, NULL));
  _dbus_assert (find_subtree (tree, path6, NULL));
  _dbus_assert (find_subtree (tree, path7, NULL));
  _dbus_assert (find_subtree (tree, path8, NULL));
  
  _dbus_object_tree_unregister_and_unlock (tree, path3);
  _dbus_assert (_dbus_object_tree_get_user_data_unlocked (tree, path3) == NULL);

  _dbus_assert (!find_subtree (tree, path0, NULL));
  _dbus_assert (!find_subtree (tree, path1, NULL));
  _dbus_assert (!find_subtree (tree, path2, NULL));
  _dbus_assert (!find_subtree (tree, path3, NULL));
  _dbus_assert (find_subtree (tree, path4, NULL));
  _dbus_assert (find_subtree (tree, path5, NULL));
  _dbus_assert (find_subtree (tree, path6, NULL));
  _dbus_assert (find_subtree (tree, path7, NULL));
  _dbus_assert (find_subtree (tree, path8, NULL));
  
  _dbus_object_tree_unregister_and_unlock (tree, path4);
  _dbus_assert (_dbus_object_tree_get_user_data_unlocked (tree, path4) == NULL);

  _dbus_assert (!find_subtree (tree, path0, NULL));
  _dbus_assert (!find_subtree (tree, path1, NULL));
  _dbus_assert (!find_subtree (tree, path2, NULL));
  _dbus_assert (!find_subtree (tree, path3, NULL));
  _dbus_assert (!find_subtree (tree, path4, NULL));
  _dbus_assert (find_subtree (tree, path5, NULL));
  _dbus_assert (find_subtree (tree, path6, NULL));
  _dbus_assert (find_subtree (tree, path7, NULL));
  _dbus_assert (find_subtree (tree, path8, NULL));
  
  _dbus_object_tree_unregister_and_unlock (tree, path5);
  _dbus_assert (_dbus_object_tree_get_user_data_unlocked (tree, path5) == NULL);

  _dbus_assert (!find_subtree (tree, path0, NULL));
  _dbus_assert (!find_subtree (tree, path1, NULL));
  _dbus_assert (!find_subtree (tree, path2, NULL));
  _dbus_assert (!find_subtree (tree, path3, NULL));
  _dbus_assert (!find_subtree (tree, path4, NULL));
  _dbus_assert (!find_subtree (tree, path5, NULL));
  _dbus_assert (find_subtree (tree, path6, NULL));
  _dbus_assert (find_subtree (tree, path7, NULL));
  _dbus_assert (find_subtree (tree, path8, NULL));
  
  _dbus_object_tree_unregister_and_unlock (tree, path6);
  _dbus_assert (_dbus_object_tree_get_user_data_unlocked (tree, path6) == NULL);

  _dbus_assert (!find_subtree (tree, path0, NULL));
  _dbus_assert (!find_subtree (tree, path1, NULL));
  _dbus_assert (!find_subtree (tree, path2, NULL));
  _dbus_assert (!find_subtree (tree, path3, NULL));
  _dbus_assert (!find_subtree (tree, path4, NULL));
  _dbus_assert (!find_subtree (tree, path5, NULL));
  _dbus_assert (!find_subtree (tree, path6, NULL));
  _dbus_assert (find_subtree (tree, path7, NULL));
  _dbus_assert (find_subtree (tree, path8, NULL));

  _dbus_object_tree_unregister_and_unlock (tree, path7);
  _dbus_assert (_dbus_object_tree_get_user_data_unlocked (tree, path7) == NULL);

  _dbus_assert (!find_subtree (tree, path0, NULL));
  _dbus_assert (!find_subtree (tree, path1, NULL));
  _dbus_assert (!find_subtree (tree, path2, NULL));
  _dbus_assert (!find_subtree (tree, path3, NULL));
  _dbus_assert (!find_subtree (tree, path4, NULL));
  _dbus_assert (!find_subtree (tree, path5, NULL));
  _dbus_assert (!find_subtree (tree, path6, NULL));
  _dbus_assert (!find_subtree (tree, path7, NULL));
  _dbus_assert (find_subtree (tree, path8, NULL));

  _dbus_object_tree_unregister_and_unlock (tree, path8);
  _dbus_assert (_dbus_object_tree_get_user_data_unlocked (tree, path8) == NULL);

  _dbus_assert (!find_subtree (tree, path0, NULL));
  _dbus_assert (!find_subtree (tree, path1, NULL));
  _dbus_assert (!find_subtree (tree, path2, NULL));
  _dbus_assert (!find_subtree (tree, path3, NULL));
  _dbus_assert (!find_subtree (tree, path4, NULL));
  _dbus_assert (!find_subtree (tree, path5, NULL));
  _dbus_assert (!find_subtree (tree, path6, NULL));
  _dbus_assert (!find_subtree (tree, path7, NULL));
  _dbus_assert (!find_subtree (tree, path8, NULL));
  
  i = 0;
  while (i < (int) _DBUS_N_ELEMENTS (tree_test_data))
    {
      _dbus_assert (tree_test_data[i].handler_unregistered);
      _dbus_assert (!tree_test_data[i].message_handled);
      ++i;
    }

  /* Test removal of newly-childless unregistered nodes */
  if (!do_register (tree, path2, TRUE, 2, tree_test_data))
    goto out;

  _dbus_object_tree_unregister_and_unlock (tree, path2);
  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path2));
  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path1));
  _dbus_assert (find_subtree_registered_or_unregistered (tree, path0));

  /* Test that unregistered parents cannot be freed out from under their
     children */
  if (!do_register (tree, path2, TRUE, 2, tree_test_data))
    goto out;

  _dbus_assert (!find_subtree (tree, path1, NULL));
  _dbus_assert (find_subtree_registered_or_unregistered (tree, path1));
  _dbus_assert (find_subtree_registered_or_unregistered (tree, path0));

#if 0
  /* This triggers the "Attempted to unregister path ..." warning message */
  _dbus_object_tree_unregister_and_unlock (tree, path1);
#endif
  _dbus_assert (find_subtree (tree, path2, NULL));
  _dbus_assert (!find_subtree (tree, path1, NULL));
  _dbus_assert (find_subtree_registered_or_unregistered (tree, path1));
  _dbus_assert (find_subtree_registered_or_unregistered (tree, path0));

  _dbus_object_tree_unregister_and_unlock (tree, path2);
  _dbus_assert (!find_subtree (tree, path2, NULL));
  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path2));
  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path1));
  _dbus_assert (find_subtree_registered_or_unregistered (tree, path0));

  /* Test that registered parents cannot be freed out from under their
     children, and that if they are unregistered before their children, they
     are still freed when their children are unregistered */
  if (!do_register (tree, path1, TRUE, 1, tree_test_data))
    goto out;
  if (!do_register (tree, path2, TRUE, 2, tree_test_data))
    goto out;

  _dbus_assert (find_subtree (tree, path1, NULL));
  _dbus_assert (find_subtree (tree, path2, NULL));

  _dbus_object_tree_unregister_and_unlock (tree, path1);
  _dbus_assert (!find_subtree (tree, path1, NULL));
  _dbus_assert (find_subtree (tree, path2, NULL));
  _dbus_assert (find_subtree_registered_or_unregistered (tree, path1));
  _dbus_assert (find_subtree_registered_or_unregistered (tree, path0));

  _dbus_object_tree_unregister_and_unlock (tree, path2);
  _dbus_assert (!find_subtree (tree, path1, NULL));
  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path1));
  _dbus_assert (!find_subtree (tree, path2, NULL));
  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path2));
  _dbus_assert (find_subtree_registered_or_unregistered (tree, path0));

  /* Test with NULL unregister_function and user_data */
  if (!_dbus_object_tree_register (tree, TRUE, path2,
                                   &test_vtable,
                                   NULL,
                                   NULL))
    goto out;

  _dbus_assert (_dbus_object_tree_get_user_data_unlocked (tree, path2) == NULL);
  _dbus_object_tree_unregister_and_unlock (tree, path2);
  _dbus_assert (!find_subtree (tree, path2, NULL));
  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path2));
  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path1));
  _dbus_assert (find_subtree_registered_or_unregistered (tree, path0));

  /* Test freeing a long path */
  if (!do_register (tree, path3, TRUE, 3, tree_test_data))
    goto out;

  _dbus_object_tree_unregister_and_unlock (tree, path3);
  _dbus_assert (!find_subtree (tree, path3, NULL));
  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path3));
  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path2));
  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path1));
  _dbus_assert (find_subtree_registered_or_unregistered (tree, path0));

  /* Test freeing multiple children from the same path */
  if (!do_register (tree, path3, TRUE, 3, tree_test_data))
    goto out;
  if (!do_register (tree, path4, TRUE, 4, tree_test_data))
    goto out;

  _dbus_assert (find_subtree (tree, path3, NULL));
  _dbus_assert (find_subtree (tree, path4, NULL));

  _dbus_object_tree_unregister_and_unlock (tree, path3);
  _dbus_assert (!find_subtree (tree, path3, NULL));
  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path3));
  _dbus_assert (find_subtree (tree, path4, NULL));
  _dbus_assert (find_subtree_registered_or_unregistered (tree, path4));
  _dbus_assert (find_subtree_registered_or_unregistered (tree, path2));
  _dbus_assert (find_subtree_registered_or_unregistered (tree, path1));

  _dbus_object_tree_unregister_and_unlock (tree, path4);
  _dbus_assert (!find_subtree (tree, path4, NULL));
  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path4));
  _dbus_assert (!find_subtree (tree, path3, NULL));
  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path3));
  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path2));
  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path1));

  /* Test subtree removal */
  if (!_dbus_object_tree_register (tree, TRUE, path12,
                                   &test_vtable,
                                   NULL,
                                   NULL))
    goto out;

  _dbus_assert (find_subtree (tree, path12, NULL));

  if (!_dbus_object_tree_register (tree, TRUE, path13,
                                   &test_vtable,
                                   NULL,
                                   NULL))
    goto out;

  _dbus_assert (find_subtree (tree, path13, NULL));

  if (!_dbus_object_tree_register (tree, TRUE, path14,
                                   &test_vtable,
                                   NULL,
                                   NULL))
    goto out;

  _dbus_assert (find_subtree (tree, path14, NULL));

  _dbus_object_tree_unregister_and_unlock (tree, path12);

  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path12));
  _dbus_assert (find_subtree (tree, path13, NULL));
  _dbus_assert (find_subtree (tree, path14, NULL));
  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path9));
  _dbus_assert (find_subtree_registered_or_unregistered (tree, path5));

  if (!_dbus_object_tree_register (tree, TRUE, path12,
                                   &test_vtable,
                                   NULL,
                                   NULL))
    goto out;

  _dbus_assert (find_subtree (tree, path12, NULL));

  _dbus_object_tree_unregister_and_unlock (tree, path13);

  _dbus_assert (find_subtree (tree, path12, NULL));
  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path13));
  _dbus_assert (find_subtree (tree, path14, NULL));
  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path10));
  _dbus_assert (find_subtree_registered_or_unregistered (tree, path5));

  if (!_dbus_object_tree_register (tree, TRUE, path13,
                                   &test_vtable,
                                   NULL,
                                   NULL))
    goto out;

  _dbus_assert (find_subtree (tree, path13, NULL));

  _dbus_object_tree_unregister_and_unlock (tree, path14);

  _dbus_assert (find_subtree (tree, path12, NULL));
  _dbus_assert (find_subtree (tree, path13, NULL));
  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path14));
  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path11));
  _dbus_assert (find_subtree_registered_or_unregistered (tree, path5));

  _dbus_object_tree_unregister_and_unlock (tree, path12);

  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path12));
  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path9));
  _dbus_assert (find_subtree_registered_or_unregistered (tree, path5));

  _dbus_object_tree_unregister_and_unlock (tree, path13);

  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path13));
  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path10));
  _dbus_assert (!find_subtree_registered_or_unregistered (tree, path5));

#if 0
  /* Test attempting to unregister non-existent paths.  These trigger
     "Attempted to unregister path ..." warning messages */
  _dbus_object_tree_unregister_and_unlock (tree, path0);
  _dbus_object_tree_unregister_and_unlock (tree, path1);
  _dbus_object_tree_unregister_and_unlock (tree, path2);
  _dbus_object_tree_unregister_and_unlock (tree, path3);
  _dbus_object_tree_unregister_and_unlock (tree, path4);
#endif

  /* Register it all again, and test dispatch */
  
  if (!do_register (tree, path0, TRUE, 0, tree_test_data))
    goto out;
  if (!do_register (tree, path1, FALSE, 1, tree_test_data))
    goto out;
  if (!do_register (tree, path2, TRUE, 2, tree_test_data))
    goto out;
  if (!do_register (tree, path3, TRUE, 3, tree_test_data))
    goto out;
  if (!do_register (tree, path4, TRUE, 4, tree_test_data))
    goto out;
  if (!do_register (tree, path5, TRUE, 5, tree_test_data))
    goto out;
  if (!do_register (tree, path6, FALSE, 6, tree_test_data))
    goto out;
  if (!do_register (tree, path7, TRUE, 7, tree_test_data))
    goto out;
  if (!do_register (tree, path8, TRUE, 8, tree_test_data))
    goto out;

#if 0
  spew_tree (tree);
#endif

  if (!do_test_dispatch (tree, path0, 0, tree_test_data, _DBUS_N_ELEMENTS (tree_test_data)))
    goto out;
  if (!do_test_dispatch (tree, path1, 1, tree_test_data, _DBUS_N_ELEMENTS (tree_test_data)))
    goto out;
  if (!do_test_dispatch (tree, path2, 2, tree_test_data, _DBUS_N_ELEMENTS (tree_test_data)))
    goto out;
  if (!do_test_dispatch (tree, path3, 3, tree_test_data, _DBUS_N_ELEMENTS (tree_test_data)))
    goto out;
  if (!do_test_dispatch (tree, path4, 4, tree_test_data, _DBUS_N_ELEMENTS (tree_test_data)))
    goto out;
  if (!do_test_dispatch (tree, path5, 5, tree_test_data, _DBUS_N_ELEMENTS (tree_test_data)))
    goto out;
  if (!do_test_dispatch (tree, path6, 6, tree_test_data, _DBUS_N_ELEMENTS (tree_test_data)))
    goto out;
  if (!do_test_dispatch (tree, path7, 7, tree_test_data, _DBUS_N_ELEMENTS (tree_test_data)))
    goto out;
  if (!do_test_dispatch (tree, path8, 8, tree_test_data, _DBUS_N_ELEMENTS (tree_test_data)))
    goto out;
  
 out:
  if (tree)
    {
      /* test ref */
      _dbus_object_tree_ref (tree);
      _dbus_object_tree_unref (tree);
      _dbus_object_tree_unref (tree);
    }

  return TRUE;
}

/**
 * @ingroup DBusObjectTree
 * Unit test for DBusObjectTree
 * @returns #TRUE on success.
 */
dbus_bool_t
_dbus_object_tree_test (const char *test_data_dir _DBUS_GNUC_UNUSED)
{
  return _dbus_test_oom_handling ("object tree",
                                  object_tree_test_iteration,
                                  NULL);
}

#endif /* !DOXYGEN_SHOULD_SKIP_THIS */

#endif /* DBUS_ENABLE_EMBEDDED_TESTS */
