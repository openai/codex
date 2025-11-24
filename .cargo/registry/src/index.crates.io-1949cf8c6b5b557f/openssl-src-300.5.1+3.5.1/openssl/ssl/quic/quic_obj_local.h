/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_QUIC_OBJ_LOCAL_H
# define OSSL_QUIC_OBJ_LOCAL_H

# include <openssl/ssl.h>
# include "internal/quic_predef.h"
# include "internal/quic_engine.h"
# include "../ssl_local.h"

# ifndef OPENSSL_NO_QUIC

/*
 * QUIC Object Structure.
 *
 * In the libssl APL, we have QLSOs, QCSOs and QSSOs, and in the future might
 * choose to introduce QDSOs. There are also roles such as Port Leader and Event
 * Leader which can be assumed by these different types under different
 * circumstances — in other words, whether an APL object is a Port or Event
 * Leader is not a static function of its type and these roles can 'float'
 * dynamically depending on the circumstances under which an APL object was
 * created.
 *
 * The QUIC_OBJ is a base type for QUIC APL objects which provides functionality
 * common to all QUIC objects and which supports having different APL objects
 * dynamically assume leader roles. It can therefore be seen as an extension of
 * the SSL base class and extends the SSL object for QUIC APL objects. This
 * avoids duplication of functionality for different types of QUIC object and
 * allows access to common responsibilities of different types of APL object
 * without regard to the kind of APL object we are dealing with.
 *
 * The "inheritance" hierarchy is as follows:
 *
 *   SSL
 *      SSL_CONNECTION
 *      QUIC_OBJ
 *          QUIC_DOMAIN         (QDSO) -> QUIC_ENGINE  *E
 *          QUIC_LISTENER       (QLSO) -> QUIC_PORT     eP
 *          QUIC_CONNECTION     (QCSO) -> QUIC_CHANNEL  epCs
 *          QUIC_XSO            (QSSO) -> QUIC_STREAM      S
 *
 * Legend:
 *
 *   *: Not currently modelled in the APL, though QUIC_ENGINE exists internally.
 *
 *   E: Always an event leader if it exists.
 *   e: Potentially an event leader (namely if it is the root APL object in a
 *      hierarchy).
 *
 *   P: Always a port leader if it exists.
 *   p: Potentially a port leader (namely if there is no port leader above it).
 *
 *   C: Always a connection leader.
 *
 *   s: Potentially usable as a stream (if it has a default stream attached).
 *   S: Always has the stream role if it exists.
 *
 * This structure must come at the start of a QUIC object structure definition.
 *
 * ssl->type still determines the actual object type. An SSL object
 * pointer s can be safely cast to (QUIC_OBJ *) iff IS_QUIC(s) is true.
 */
struct quic_obj_st {
    /* SSL object common header. */
    struct ssl_st           ssl;

    /*
     * Pointer to a parent APL object in a QUIC APL object hierarchy, or NULL if
     * this is the root object.
     */
    QUIC_OBJ                *parent_obj;

    /* invariant: != NULL */
    QUIC_OBJ                *cached_event_leader;
    /* invariant: != NULL iff this is a port leader or subsidiary object */
    QUIC_OBJ                *cached_port_leader;

    /*
     * Points to the QUIC_ENGINE instance. Always equals
     * cached_event_leader->engine. The containing_obj APL object owns this
     * instance iff is_event_leader is set, otherwise it is an additional
     * reference cached for convenience. Unlike port this is never NULL because
     * a QUIC domain is always rooted in an event leader.
     */
    QUIC_ENGINE             *engine;

    /*
     * Points to the QUIC_PORT instance applicable to the containing_obj APL
     * object, or NULL if we are not at or below a port leader. Always equals
     * cached_port_leader->port. The containing_obj APL object owns this
     * instance iff is_port_leader is set, otherwise it is an additional
     * reference cached for convenience.
     */
    QUIC_PORT               *port;

    /* SSL_DOMAIN_FLAG values taken from SSL_CTX at construction time. */
    uint64_t                domain_flags;

    unsigned int            init_done       : 1;
    unsigned int            is_event_leader : 1;
    unsigned int            is_port_leader  : 1;

    /*
     * Blocking mode configuration is handled generically through QUIC_OBJ as it
     * by default inherits from the parent SSL object.
     */
    unsigned int            req_blocking_mode       : 2; /* QUIC_BLOCKING_MODE */

    /* Event handling mode. One of SSL_QUIC_VALUE_EVENT_HANDLING. */
    unsigned int            event_handling_mode     : 2;
};

enum {
    QUIC_BLOCKING_MODE_INHERIT,
    QUIC_BLOCKING_MODE_NONBLOCKING,
    QUIC_BLOCKING_MODE_BLOCKING
};

/*
 * Core Functions and Inlines
 * ==========================
 */

/*
 * Initialises a QUIC_OBJ structure with zero or more roles active. Returns 1
 * on success or 0 on failure.
 *
 * ctx: A SSL_CTX used to initialise the SSL base object structure.
 *
 * type: A SSL_TYPE_* value designating the SSL object type.
 *
 * parent_obj: NULL if this is the root APL object in a new hierarchy, or a
 * pointer to the parent APL object otherwise.
 *
 * engine: If non-NULL, this object becomes the Event Leader. parent_obj must be
 * NULL iff this is non-NULL as currently the Event Leader is always the root in
 * an APL object hierarchy. If NULL, the contextually applicable engine is
 * determined by using parent_obj and ancestors to find the Event Leader.
 *
 * port: If non-NULL, this object becomes a Port Leader. If NULL, the
 * contextually applicable port (if any) is determined by using parent_obj and
 * ancestors to find the Port Leader.
 */
int ossl_quic_obj_init(QUIC_OBJ *obj,
                       SSL_CTX *ctx,
                       int type,
                       SSL *parent_obj,
                       QUIC_ENGINE *engine,
                       QUIC_PORT *port);

/*
 * Returns a pointer to the handshake layer object which should be accessible on
 * obj for purposes of handshake API autoforwarding, if any.
 *
 * This returns NULL if a handshake layer SSL object is available but should not
 * be used for autoforwarding purposes, for example on a QSSO.
 */
SSL_CONNECTION *ossl_quic_obj_get0_handshake_layer(QUIC_OBJ *obj);

/*
 * Returns a pointer to the SSL base object structure. Returns NULL if obj is
 * NULL. If obj is non-NULL, it must be initialised.
 */
static ossl_inline ossl_unused SSL *
ossl_quic_obj_get0_ssl(QUIC_OBJ *obj)
{
    /*
     * ->ssl is guaranteed to have an offset of 0 but the NULL check here makes
     *  ubsan happy.
     */
    if (!ossl_assert(obj != NULL))
        return NULL;

    return &obj->ssl;
}

/*
 * Determines the applicable engine and return a pointer to it. Never returns
 * NULL.
 */
static ossl_inline ossl_unused QUIC_ENGINE *
ossl_quic_obj_get0_engine(const QUIC_OBJ *obj)
{
    assert(obj->init_done);
    assert(obj->engine != NULL);
    return obj->engine;
}

/* Determines the applicable port (if any) and returns a pointer to it. */
static ossl_inline ossl_unused QUIC_PORT *
ossl_quic_obj_get0_port(const QUIC_OBJ *obj)
{
    assert(obj->init_done);
    return obj->port;
}

/* Returns 1 iff this leader structure represents an event leader. */
static ossl_inline ossl_unused int
ossl_quic_obj_is_event_leader(const QUIC_OBJ *obj)
{
    return obj->is_event_leader;
}

/*
 * Similar to ossl_quic_obj_get0_engine, but only returns a non-NULL value if
 * the obj object itself is an event leader, rather than one of its ancestors.
 */
static ossl_inline ossl_unused QUIC_ENGINE *
ossl_quic_obj_get0_engine_local(const QUIC_OBJ *obj)
{
    return ossl_quic_obj_is_event_leader(obj)
        ? ossl_quic_obj_get0_engine(obj) : NULL;
}

/* Returns 1 iff this leader structure represents a port leader. */
static ossl_inline ossl_unused int
ossl_quic_obj_is_port_leader(const QUIC_OBJ *obj)
{
    return obj->is_port_leader;
}

/*
 * Similar to ossl_quic_obj_get0_port, but only returns a non-NULL value if
 * the obj object itself is a port leader, rather than one of its ancestors.
 */
static ossl_inline ossl_unused QUIC_PORT *
ossl_quic_obj_get0_port_local(const QUIC_OBJ *obj)
{
    return ossl_quic_obj_is_port_leader(obj)
        ? ossl_quic_obj_get0_port(obj) : NULL;
}

/*
 * Return 1 if we are currently capable of supporting blocking mode (regardless
 * of whether it is actually turned on).
 */
int ossl_quic_obj_can_support_blocking(const QUIC_OBJ *obj);

/*
 * Returns 1 if we *desire* to do blocking I/O, regardless of whether it will
 * actually be used (e.g. because it cannot currently be supported).
 */
int ossl_quic_obj_desires_blocking(const QUIC_OBJ *obj);

/*
 * Return 1 if an API call directly to the given object should use blocking mode
 * and 0 otherwise.
 */
int ossl_quic_obj_blocking(const QUIC_OBJ *obj);

/*
 * Set the (requested) blocking mode, which might or might not be honoured
 * depending on whether the BIO configuration can support it. Argument is a
 * QUIC_BLOCKING_MODE value. If the top-level object in a QSO hierarchy is set
 * to QUIC_BLOCKING_MODE_INHERIT, defaults to blocking mode.
 */
void ossl_quic_obj_set_blocking_mode(QUIC_OBJ *obj, unsigned int mode);

/*
 * Convenience Inlines
 * ===================
 *
 * These inlines are expressed in terms of the core functions and inlines above.
 */

/* Get a pointer to the QUIC domain mutex. Always returns non-NULL. */
static ossl_inline ossl_unused CRYPTO_MUTEX *
ossl_quic_obj_get0_mutex(const QUIC_OBJ *obj)
{
    return ossl_quic_engine_get0_mutex(ossl_quic_obj_get0_engine(obj));
}

/*
 * Get a reference to the reactor applicable to a leader. Always returns
 * non-NULL.
 */
static ossl_inline ossl_unused QUIC_REACTOR *
ossl_quic_obj_get0_reactor(const QUIC_OBJ *obj)
{
    return ossl_quic_engine_get0_reactor(ossl_quic_obj_get0_engine(obj));
}

/* Get a reference to the OSSL_LIB_CTX pointer applicable to a leader. */
static ossl_inline ossl_unused OSSL_LIB_CTX *
ossl_quic_obj_get0_libctx(const QUIC_OBJ *obj)
{
    return ossl_quic_engine_get0_libctx(ossl_quic_obj_get0_engine(obj));
}

/* Get a reference to the propq pointer applicable to a leader. */
static ossl_inline ossl_unused const char *
ossl_quic_obj_get0_propq(const QUIC_OBJ *obj)
{
    return ossl_quic_engine_get0_propq(ossl_quic_obj_get0_engine(obj));
}

/*
 * Returns the APL object pointer to the event leader in a hierarchy. Always
 * returns non-NULL.
 */
static ossl_inline ossl_unused SSL *
ossl_quic_obj_get0_event_leader(const QUIC_OBJ *obj)
{
    assert(obj->init_done);
    return obj->cached_event_leader != NULL
        ? &obj->cached_event_leader->ssl
        : NULL;
}

/*
 * Returns the APL object pointer to the port leader in a hierarchy (if any).
 * Always returns non-NULL.
 */
static ossl_inline ossl_unused SSL *
ossl_quic_obj_get0_port_leader(const QUIC_OBJ *obj)
{
    assert(obj->init_done);
    return obj->cached_port_leader != NULL
        ? &obj->cached_port_leader->ssl
        : NULL;
}

/*
 * Change the domain flags. Should only be called immediately after
 * ossl_quic_obj_init().
 */
static ossl_inline ossl_unused void
ossl_quic_obj_set_domain_flags(QUIC_OBJ *obj, uint64_t domain_flags)
{
    obj->domain_flags = domain_flags;
}

# endif
#endif
