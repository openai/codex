Thread Pool Support
===================

OpenSSL wishes to support the internal use of threads for purposes of
concurrency and parallelism in some circumstances. There are various reasons why
this is desirable:

  - Some algorithms are designed to be run in parallel (Argon2);
  - Some transports (e.g. QUIC, DTLS) may need to handle timer events
    independently of application calls to OpenSSL.

To support this end, OpenSSL can manage an internal thread pool. Tasks can be
scheduled on the internal thread pool.

There is currently a single model available to an application which wants to use
the thread pool functionality, known as the “default model”. More models
providing more flexible or advanced usage may be added in future releases.

A thread pool is managed on a per-`OSSL_LIB_CTX` basis.

Default Model
-------------

In the default model, OpenSSL creates and manages threads up to a maximum
number of threads authorized by the application.

The application enables thread pooling by calling the following function
during its initialisation:

```c
/*
 * Set the maximum number of threads to be used by the thread pool.
 *
 * If the argument is 0, thread pooling is disabled. OpenSSL will not create any
 * threads and existing threads in the thread pool will be torn down.
 *
 * Returns 1 on success and 0 on failure. Returns failure if OpenSSL-managed
 * thread pooling is not supported (for example, if it is not supported on the
 * current platform, or because OpenSSL is not built with the necessary
 * support).
 */
int OSSL_set_max_threads(OSSL_LIB_CTX *ctx, uint64_t max_threads);

/*
 * Get the maximum number of threads currently allowed to be used by the
 * thread pool. If thread pooling is disabled or not available, returns 0.
 */
uint64_t OSSL_get_max_threads(OSSL_LIB_CTX *ctx);
```

The maximum thread count is a limit, not a target. Threads will not be spawned
unless (and until) there is demand.

As usual, `ctx` can be NULL to use the default library context.

Capability Detection
--------------------

These functions allow the caller to determine if OpenSSL was built with threads
support.

```c
/*
 * Retrieves flags indicating what threading functionality OpenSSL can support
 * based on how it was built and the platform on which it was running.
 */
/* Is thread pool functionality supported at all? */
#define OSSL_THREAD_SUPPORT_FLAG_THREAD_POOL    (1U<<0)

/*
 * Is the default model supported? If THREAD_POOL is supported but DEFAULT_SPAWN
 * is not supported, another model must be used. Note that there is currently
 * only one supported model (the default model), but there may be more in the
 * future.
 */
#define OSSL_THREAD_SUPPORT_FLAG_DEFAULT_SPAWN  (1U<<1)

/* Returns zero or more of OSSL_THREAD_SUPPORT_FLAG_*. */
uint32_t OSSL_get_thread_support_flags(void);
```

Build Options
-------------

A build option `thread-pool`/`no-thread-pool` will be introduced which allows
thread pool functionality to be compiled out. `no-thread-pool` implies
`no-default-thread-pool`.

A build option `default-thread-pool`/`no-default-thread-pool` will be introduced
which allows the default thread pool functionality to be compiled out. If this
functionality is compiled out, another thread pool model must be used. Since the
default model is the only currently supported model, disabling the default model
renders threading functionality unusable. As such, there is little reason to use
this option instead of `thread-pool/no-thread-pool`, however this option is
nonetheless provided for symmetry when additional models are introduced in the
future.

Internals
---------

New internal components will need to be introduced (e.g. condition variables)
to support this functionality, however there is no intention of making
this functionality public at this time.
