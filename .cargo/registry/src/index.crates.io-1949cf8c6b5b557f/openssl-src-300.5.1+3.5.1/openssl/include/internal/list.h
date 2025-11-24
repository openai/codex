/*
 * Copyright 2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_INTERNAL_LIST_H
# define OSSL_INTERNAL_LIST_H
# pragma once

# include <string.h>
# include <assert.h>

# ifdef NDEBUG
#  define OSSL_LIST_DBG(x)
# else
#  define OSSL_LIST_DBG(x) x;
# endif

# define OSSL_LIST_FOREACH_FROM(p, name, init)                              \
    for ((p) = (init);                                                      \
         (p) != NULL;                                                       \
         (p) = ossl_list_##name##_next(p))
# define OSSL_LIST_FOREACH(p, name, l)                                      \
    OSSL_LIST_FOREACH_FROM(p, name, ossl_list_##name##_head(l))

# define OSSL_LIST_FOREACH_REV_FROM(p, name, init)                          \
    for ((p) = (init);                                                      \
         (p) != NULL;                                                       \
         (p) = ossl_list_##name##_prev(p))
# define OSSL_LIST_FOREACH_REV(p, name, l)                                  \
    OSSL_LIST_FOREACH_FROM(p, name, ossl_list_##name##_tail(l))

# define OSSL_LIST_FOREACH_DELSAFE_FROM(p, pn, name, init)                  \
    for ((p) = (init);                                                      \
         (p) != NULL && (((pn) = ossl_list_##name##_next(p)), 1);           \
         (p) = (pn))
#define OSSL_LIST_FOREACH_DELSAFE(p, pn, name, l)                           \
    OSSL_LIST_FOREACH_DELSAFE_FROM(p, pn, name, ossl_list_##name##_head(l))

# define OSSL_LIST_FOREACH_REV_DELSAFE_FROM(p, pn, name, init)              \
    for ((p) = (init);                                                      \
         (p) != NULL && (((pn) = ossl_list_##name##_prev(p)), 1);           \
         (p) = (pn))
# define OSSL_LIST_FOREACH_REV_DELSAFE(p, pn, name, l)                      \
    OSSL_LIST_FOREACH_REV_DELSAFE_FROM(p, pn, name, ossl_list_##name##_tail(l))

/* Define a list structure */
# define OSSL_LIST(name) OSSL_LIST_ ## name

/* Define fields to include an element of a list */
# define OSSL_LIST_MEMBER(name, type)                                       \
    struct {                                                                \
        type *next, *prev;                                                  \
        OSSL_LIST_DBG(struct ossl_list_st_ ## name *list)                   \
    } ossl_list_ ## name

# define DECLARE_LIST_OF(name, type)                                        \
    typedef struct ossl_list_st_ ## name OSSL_LIST(name);                   \
    struct ossl_list_st_ ## name {                                          \
        type *alpha, *omega;                                                \
        size_t num_elems;                                                   \
    }                                                                       \

# define DEFINE_LIST_OF_IMPL(name, type)                                    \
    static ossl_unused ossl_inline void                                     \
    ossl_list_##name##_init(OSSL_LIST(name) *list)                          \
    {                                                                       \
        memset(list, 0, sizeof(*list));                                     \
    }                                                                       \
    static ossl_unused ossl_inline void                                     \
    ossl_list_##name##_init_elem(type *elem)                                \
    {                                                                       \
        memset(&elem->ossl_list_ ## name, 0,                                \
               sizeof(elem->ossl_list_ ## name));                           \
    }                                                                       \
    static ossl_unused ossl_inline int                                      \
    ossl_list_##name##_is_empty(const OSSL_LIST(name) *list)                \
    {                                                                       \
        return list->num_elems == 0;                                        \
    }                                                                       \
    static ossl_unused ossl_inline size_t                                   \
    ossl_list_##name##_num(const OSSL_LIST(name) *list)                     \
    {                                                                       \
        return list->num_elems;                                             \
    }                                                                       \
    static ossl_unused ossl_inline type *                                   \
    ossl_list_##name##_head(const OSSL_LIST(name) *list)                    \
    {                                                                       \
        assert(list->alpha == NULL                                          \
               || list->alpha->ossl_list_ ## name.list == list);            \
        return list->alpha;                                                 \
    }                                                                       \
    static ossl_unused ossl_inline type *                                   \
    ossl_list_##name##_tail(const OSSL_LIST(name) *list)                    \
    {                                                                       \
        assert(list->omega == NULL                                          \
               || list->omega->ossl_list_ ## name.list == list);            \
        return list->omega;                                                 \
    }                                                                       \
    static ossl_unused ossl_inline type *                                   \
    ossl_list_##name##_next(const type *elem)                               \
    {                                                                       \
        assert(elem->ossl_list_ ## name.next == NULL                        \
               || elem->ossl_list_ ## name.next                             \
                      ->ossl_list_ ## name.prev == elem);                   \
        return elem->ossl_list_ ## name.next;                               \
    }                                                                       \
    static ossl_unused ossl_inline type *                                   \
    ossl_list_##name##_prev(const type *elem)                               \
    {                                                                       \
        assert(elem->ossl_list_ ## name.prev == NULL                        \
               || elem->ossl_list_ ## name.prev                             \
                      ->ossl_list_ ## name.next == elem);                   \
        return elem->ossl_list_ ## name.prev;                               \
    }                                                                       \
    static ossl_unused ossl_inline void                                     \
    ossl_list_##name##_remove(OSSL_LIST(name) *list, type *elem)            \
    {                                                                       \
        assert(elem->ossl_list_ ## name.list == list);                      \
        OSSL_LIST_DBG(elem->ossl_list_ ## name.list = NULL)                 \
        if (list->alpha == elem)                                            \
            list->alpha = elem->ossl_list_ ## name.next;                    \
        if (list->omega == elem)                                            \
            list->omega = elem->ossl_list_ ## name.prev;                    \
        if (elem->ossl_list_ ## name.prev != NULL)                          \
            elem->ossl_list_ ## name.prev->ossl_list_ ## name.next =        \
                    elem->ossl_list_ ## name.next;                          \
        if (elem->ossl_list_ ## name.next != NULL)                          \
            elem->ossl_list_ ## name.next->ossl_list_ ## name.prev =        \
                    elem->ossl_list_ ## name.prev;                          \
        list->num_elems--;                                                  \
        memset(&elem->ossl_list_ ## name, 0,                                \
               sizeof(elem->ossl_list_ ## name));                           \
    }                                                                       \
    static ossl_unused ossl_inline void                                     \
    ossl_list_##name##_insert_head(OSSL_LIST(name) *list, type *elem)       \
    {                                                                       \
        assert(elem->ossl_list_ ## name.list == NULL);                      \
        OSSL_LIST_DBG(elem->ossl_list_ ## name.list = list)                 \
        if (list->alpha != NULL)                                            \
            list->alpha->ossl_list_ ## name.prev = elem;                    \
        elem->ossl_list_ ## name.next = list->alpha;                        \
        elem->ossl_list_ ## name.prev = NULL;                               \
        list->alpha = elem;                                                 \
        if (list->omega == NULL)                                            \
            list->omega = elem;                                             \
        list->num_elems++;                                                  \
    }                                                                       \
    static ossl_unused ossl_inline void                                     \
    ossl_list_##name##_insert_tail(OSSL_LIST(name) *list, type *elem)       \
    {                                                                       \
        assert(elem->ossl_list_ ## name.list == NULL);                      \
        OSSL_LIST_DBG(elem->ossl_list_ ## name.list = list)                 \
        if (list->omega != NULL)                                            \
            list->omega->ossl_list_ ## name.next = elem;                    \
        elem->ossl_list_ ## name.prev = list->omega;                        \
        elem->ossl_list_ ## name.next = NULL;                               \
        list->omega = elem;                                                 \
        if (list->alpha == NULL)                                            \
            list->alpha = elem;                                             \
        list->num_elems++;                                                  \
    }                                                                       \
    static ossl_unused ossl_inline void                                     \
    ossl_list_##name##_insert_before(OSSL_LIST(name) *list, type *e,        \
                                     type *elem)                            \
    {                                                                       \
        assert(elem->ossl_list_ ## name.list == NULL);                      \
        OSSL_LIST_DBG(elem->ossl_list_ ## name.list = list)                 \
        elem->ossl_list_ ## name.next = e;                                  \
        elem->ossl_list_ ## name.prev = e->ossl_list_ ## name.prev;         \
        if (e->ossl_list_ ## name.prev != NULL)                             \
            e->ossl_list_ ## name.prev->ossl_list_ ## name.next = elem;     \
        e->ossl_list_ ## name.prev = elem;                                  \
        if (list->alpha == e)                                               \
            list->alpha = elem;                                             \
        list->num_elems++;                                                  \
    }                                                                       \
    static ossl_unused ossl_inline void                                     \
    ossl_list_##name##_insert_after(OSSL_LIST(name) *list, type *e,         \
                                    type *elem)                             \
    {                                                                       \
        assert(elem->ossl_list_ ## name.list == NULL);                      \
        OSSL_LIST_DBG(elem->ossl_list_ ## name.list = list)                 \
        elem->ossl_list_ ## name.prev = e;                                  \
        elem->ossl_list_ ## name.next = e->ossl_list_ ## name.next;         \
        if (e->ossl_list_ ## name.next != NULL)                             \
            e->ossl_list_ ## name.next->ossl_list_ ## name.prev = elem;     \
        e->ossl_list_ ## name.next = elem;                                  \
        if (list->omega == e)                                               \
            list->omega = elem;                                             \
        list->num_elems++;                                                  \
    }                                                                       \
    struct ossl_list_st_ ## name

# define DEFINE_LIST_OF(name, type)                                         \
    DECLARE_LIST_OF(name, type);                                            \
    DEFINE_LIST_OF_IMPL(name, type)

#endif
