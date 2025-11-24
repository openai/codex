Datagram BIO API revisions for sendmmsg/recvmmsg
================================================

We need to evolve the API surface of BIO which is relevant to BIO_dgram (and the
eventual BIO_dgram_mem) to support APIs which allow multiple datagrams to be
sent or received simultaneously, such as sendmmsg(2)/recvmmsg(2).

The adopted design
------------------

### Design decisions

The adopted design makes the following design decisions:

- We use a sendmmsg/recvmmsg-like API. The alternative API was not considered
  for adoption because it is an explicit goal that the adopted API be suitable
  for concurrent use on the same BIO.

- We define our own structures rather than using the OS's `struct mmsghdr`.
  The motivations for this are:

  - It ensures portability between OSes and allows the API to be used
    on OSes which do not support `sendmmsg` or `sendmsg`.

  - It allows us to use structures in keeping with OpenSSL's existing
    abstraction layers (e.g. `BIO_ADDR` rather than `struct sockaddr`).

  - We do not have to expose functionality which we cannot guarantee
    we can support on all platforms (for example, arbitrary control messages).

  - It avoids the need to include OS headers in our own public headers,
    which would pollute the environment of applications which include
    our headers, potentially undesirably.

- For OSes which do not support `sendmmsg`, we emulate it using repeated
  calls to `sendmsg`. For OSes which do not support `sendmsg`, we emulate it
  using `sendto` to the extent feasible. This avoids the need for code consuming
  these new APIs to define a fallback code path.

- We do not define any flags at this time, as the flags previously considered
  for adoption cannot be supported on all platforms (Win32 does not have
  `MSG_DONTWAIT`).

- We ensure the extensibility of our `BIO_MSG` structure in a way that preserves
  ABI compatibility using a `stride` argument which callers must set to
  `sizeof(BIO_MSG)`. Implementations can examine the stride field to determine
  whether a given field is part of a `BIO_MSG`. This allows us to add optional
  fields to `BIO_MSG` at a later time without breaking ABI. All new fields must
  be added to the end of the structure.

- The BIO methods are designed to support stateless operation in which they
  are simply calls to the equivalent system calls, where supported, without
  changing BIO state. In particular, this means that things like retry flags are
  not set or cleared by `BIO_sendmmsg` or `BIO_recvmmsg`.

  The motivation for this is that these functions are intended to support
  concurrent use on the same BIO. If they read or modify BIO state, they would
  need to be synchronised with a lock, undermining performance on what (for
  `BIO_dgram`) would otherwise be a straight system call.

- We do not support iovecs. The motivations for this are:

  - Not all platforms can support iovecs (e.g. Windows).

  - The only way we could emulate iovecs on platforms which don't support
    them is by copying the data to be sent into a staging buffer. This would
    defeat all of the advantages of iovecs and prevent us from meeting our
    zero/single-copy requirements. Moreover, it would lead to extremely
    surprising performance variations for consumers of the API.

  - We do not believe iovecs are needed to meet our performance requirements
    for QUIC. The reason for this is that aside from a minimal packet header,
    all data in QUIC is encrypted, so all data sent via QUIC must pass through
    an encrypt step anyway, meaning that all data sent will already be copied
    and there is not going to be any issue depositing the ciphertext in a
    staging buffer together with the frame header.

  - Even if we did support iovecs, we would have to impose a limit
    on the number of iovecs supported, because we translate from our own
    structures (as discussed above) and also intend these functions to be
    stateless and not requiire locking. Therefore the OS-native iovec structures
    would need to be allocated on the stack.

- Sometimes, an application may wish to learn the local interface address
  associated with a receive operation or specify the local interface address to
  be used for a send operation. We support this, but require this functionality
  to be explicitly enabled before use.

  The reason for this is that enabling this functionality generally requires
  that the socket be reconfigured using `setsockopt` on most platforms. Doing
  this on-demand would require state in the BIO to determine whether this
  functionality is currently switched on, which would require otherwise
  unnecessary locking, undermining performance in concurrent usage of this API
  on a given BIO. By requiring this functionality to be enabled explicitly
  before use, this allows this initialization to be done up front without
  performance cost. It also aids users of the API to understand that this
  functionality is not always available and to detect when this functionality is
  available in advance.

### Design

The currently proposed design is as follows:

```c
typedef struct bio_msg_st {
    void *data;
    size_t data_len;
    BIO_ADDR *peer, *local;
    uint64_t flags;
} BIO_MSG;

#define BIO_UNPACK_ERRNO(e)     /*...*/
#define BIO_IS_ERRNO(e)         /*...*/

ossl_ssize_t BIO_sendmmsg(BIO *b, BIO_MSG *msg, size_t stride,
                          size_t num_msg, uint64_t flags);
ossl_ssize_t BIO_recvmmsg(BIO *b, BIO_MSG *msg, size_t stride,
                          size_t num_msg, uint64_t flags);
```

The API is used as follows:

- `msg` points to an array of `num_msg` `BIO_MSG` structures.

- Both functions have identical prototypes, and return the number of messages
  processed in the array. If no messages were sent due to an error, `-1` is
  returned. If an OS-level socket error occurs, a negative value `v` is
  returned. The caller should determine that `v` is an OS-level socket error by
  calling `BIO_IS_ERRNO(v)` and may obtain the OS-level socket error code by
  calling `BIO_UNPACK_ERRNO(v)`.

- `stride` must be set to `sizeof(BIO_MSG)`.

- `data` points to the buffer of data to be sent or to be filled with received
  data. `data_len` is the size of the buffer in bytes on call. If the
  given message in the array is processed (i.e., if the return value
  exceeds the index of that message in the array), `data_len` is updated
  to the actual amount of data sent or received at return time.

- `flags` in the `BIO_MSG` structure provides per-message flags to
  the `BIO_sendmmsg` or `BIO_recvmmsg` call. If the given message in the array
  is processed, `flags` is written with zero or more result flags at return
  time. The `flags` argument to the call itself provides for global flags
  affecting all messages in the array. Currently, no per-message or global flags
  are defined and all of these fields are set to zero on call and on return.

- `peer` and `local` are optional pointers to `BIO_ADDR` structures into
  which the remote and local addresses are to be filled. If either of these
  are NULL, the given addressing information is not requested. Local address
  support may not be available in all circumstances, in which case processing of
  the message fails. (This means that the function returns the number of
  messages processed, or -1 if the message in question is the first message.)

  Support for `local` must be explicitly enabled before use, otherwise
  attempts to use it fail.

Local address support is enabled as follows:

```c
int BIO_dgram_set_local_addr_enable(BIO *b, int enable);
int BIO_dgram_get_local_addr_enable(BIO *b);
int BIO_dgram_get_local_addr_cap(BIO *b);
```

`BIO_dgram_get_local_addr_cap()` returns 1 if local address support is
available. It is then enabled using `BIO_dgram_set_local_addr_enable()`, which
fails if support is not available.

Options which were considered
-----------------------------

Options for the API surface which were considered included:

### sendmmsg/recvmmsg-like API

This design was chosen to form the basis of the adopted design, which is
described above.

```c
int BIO_readm(BIO *b, BIO_mmsghdr *msgvec,
              unsigned len, int flags, struct timespec *timeout);
int BIO_writem(BIO *b, BIO_mmsghdr *msgvec,
              unsigned len, int flags, struct timespec *timeout);
```

We can either define `BIO_mmsghdr` as a typedef of `struct mmsghdr` or redefine
an equivalent structure. The former has the advantage that we can just pass the
structures through to the syscall without copying them.

Note that in `BIO_mem_dgram` we will have to process and therefore understand
the contents of `struct mmsghdr` ourselves. Therefore, initially we define a
subset of `struct mmsghdr` as being supported, specifically no control messages;
`msg_name` and `msg_iov` only.

The flags argument is defined by us. Initially we can support something like
`MSG_DONTWAIT` (say, `BIO_DONTWAIT`).

#### Implementation Questions

If we go with this, there are some issues that arise:

- Are `BIO_mmsghdr`, `BIO_msghdr` and `BIO_iovec` simple typedefs
  for OS-provided structures, or our own independent structure
  definitions?

  - If we use OS-provided structures:

    - We would need to include the OS headers which provide these
      structures in our public API headers.

    - If we choose to support these functions when OS support is not available
      (see discussion below), We would need to define our own structures in this
      case (a “polyfill” approach).

  - If we use our own structures:

    - We would need to translate these structures during every call.

      But we would need to have storage inside the BIO_dgram for *m* `struct
      msghdr`, *m\*v* iovecs, etc. Since we want to support multithreaded use
      these allocations probably will need to be on the stack, and therefore
      must be limited.

      Limiting *m* isn't a problem, because `sendmmsg` returns the number
      of messages sent, so the existing semantics we are trying to match
      lets us just send or receive fewer messages than we were asked to.

      However, it does seem like we will need to limit *v*, the number of iovecs
      per message. So what limit should we give to *v*, the number of iovecs? We
      will need a fixed stack allocation of OS iovec structures and we can
      allocate from this stack allocation as we iterate through the `BIO_msghdr`
      we have been given. So in practice we could just only send messages
      until we reach our iovec limit, and then return.

      For example, suppose we allocate 64 iovecs internally:

      ```c
      struct iovec vecs[64];
      ```

      If the first message passed to a call to `BIO_writem` has 64 iovecs
      attached to it, no further messages can be sent and `BIO_writem`
      returns 1.

      If three messages are sent, with 32, 32, and 1 iovecs respectively,
      the first two messages are sent and `BIO_writem` returns 2.

      So the only important thing we would need to document in this API
      is the limit of iovecs on a single message; in other words, the
      number of iovecs which must not be exceeded if a forward progress
      guarantee is to be made. e.g. if we allocate 64 iovecs internally,
      `BIO_writem` with a single message with 65 iovecs will never work
      and this becomes part of the API contract.

      Obviously these quantities of iovecs are unrealistically large.
      iovecs are small, so we can afford to set the limit high enough
      that it shouldn't cause any problems in practice. We can increase
      the limit later without a breaking API change, but we cannot decrease
      it later. So we might want to start with something small, like 8.

- We also need to decide what to do for OSes which don't support at least
  `sendmsg`/`recvmsg`.

  - Don't provide these functions and require all users of these functions to
    have an alternate code path which doesn't rely on them?

    - Not providing these functions on OSes that don't support
      at least sendmsg/recvmsg is a simple solution but adds
      complexity to code using BIO_dgram. (Though it does communicate
      to code more realistic performance expectations since it
      knows when these functions are actually available.)

  - Provide these functions and emulate the functionality:

    - However there is a question here as to how we implement
      the iovec arguments on platforms without `sendmsg`/`recvmsg`. (We cannot
      use `writev`/`readv` because we need peer address information.) Logically
      implementing these would then have to be done by copying buffers around
      internally before calling `sendto`/`recvfrom`, defeating the point of
      iovecs and providing a performance profile which is surprising to code
      using BIO_dgram.

    - Another option could be a variable limit on the number of iovecs,
      which can be queried from BIO_dgram. This would be a constant set
      when libcrypto is compiled. It would be 1 for platforms not supporting
      `sendmsg`/`recvmsg`. This again adds burdens on the code using
      BIO_dgram, but it seems the only way to avoid the surprising performance
      pitfall of buffer copying to emulate iovec support. There is a fair risk
      of code being written which accidentally works on one platform but not
      another, because the author didn't realise the iovec limit is 1 on some
      platforms. Possibly we could have an “iovec limit” variable in the
      BIO_dgram which is 1 by default, which can be increased by a call to a
      function BIO_set_iovec_limit, but not beyond the fixed size discussed
      above. It would return failure if not possible and this would give client
      code a clear way to determine if its expectations are met.

### Alternate API

Could we use a simplified API? For example, could we have an API that returns
one datagram where BIO_dgram uses `readmmsg` internally and queues the returned
datagrams, thereby still avoiding extra syscalls but offering a simple API.

The problem here is we want to support “single-copy” (where the data is only
copied as it is decrypted). Thus BIO_dgram needs to know the final resting place
of encrypted data at the time it makes the `readmmsg` call.

One option would be to allow the user to set a callback on BIO_dgram it can use
to request a new buffer, then have an API which returns the buffer:

```c
int BIO_dgram_set_read_callback(BIO *b,
                                void *(*cb)(size_t len, void *arg),
                                void *arg);
int BIO_dgram_set_read_free_callback(BIO *b,
                                     void (*cb)(void *buf,
                                                size_t buf_len,
                                                void *arg),
                                     void *arg);
int BIO_read_dequeue(BIO *b, void **buf, size_t *buf_len);
```

The BIO_dgram calls the specified callback when it needs to generate internal
iovecs for its `readmmsg` call, and the received datagrams can then be popped by
the application and freed as it likes. (The read free callback above is only
used in rare circumstances, such as when calls to `BIO_read` and
`BIO_read_dequeue` are alternated, or when the BIO_dgram is destroyed prior to
all read buffers being dequeued; see below.) For convenience we could have an
extra call to allow a buffer to be pushed back into the BIO_dgram's internal
queue of unused read buffers, which avoids the need for the application to do
its own management of such recycled buffers:

```c
int BIO_dgram_push_read_buffer(BIO *b, void *buf, size_t buf_len);
```

On the write side, the application provides buffers and can get a callback when
they are freed. BIO_write_queue just queues for transmission, and the `sendmmsg`
call is made when calling `BIO_flush`. (TBD: whether it is reasonable to
overload the semantics of BIO_flush in this way.)

```c
int BIO_dgram_set_write_done_callback(BIO *b,
                                      void (*cb)(const void *buf,
                                                 size_t buf_len,
                                                 int status,
                                                 void *arg),
                                      void *arg);
int BIO_write_queue(BIO *b, const void *buf, size_t buf_len);
int BIO_flush(BIO *b);
```

The status argument to the write done callback will be 1 on success, some
negative value on failure, and some special negative value if the BIO_dgram is
being freed before the write could be completed.

For send/receive addresses, we import the `BIO_(set|get)_dgram_(origin|dest)`
APIs proposed in the sendmsg/recvmsg PR (#5257). `BIO_get_dgram_(origin|dest)`
should be called immediately after `BIO_read_dequeue` and
`BIO_set_dgram_(origin|dest)` should be called immediately before
`BIO_write_queue`.

This approach allows `BIO_dgram` to support myriad options via composition of
successive function calls in a “builder” style rather than via a single function
call with an excessive number of arguments or pointers to unwieldy ever-growing
argument structures, requiring constant revision of the central read/write
functions of the BIO API.

Note that since `BIO_set_dgram_(origin|dest)` sets data on outgoing packets and
`BIO_get_dgram_(origin|dest)` gets data on incoming packets, it doesn't follow
that these are accessing the same data (they are not setters and getters of a
variables called "dgram origin" and "dgram destination", even though they look
like setters and getters of the same variables from the name.) We probably want
to separate these as there is no need for a getter for outgoing packet
destination, for example, and by separating these we allow the possibility of
multithreaded use (one thread reads, one thread writes) in the future. Possibly
we should choose less confusing names for these functions. Maybe
`BIO_set_outgoing_dgram_(origin|dest)` and
`BIO_get_incoming_dgram_(origin|dest)`.

Pros of this approach:

  - Application can generate one datagram at a time and still get the advantages
    of sendmmsg/recvmmsg (fewer syscalls, etc.)

    We probably want this for our own QUIC implementation built on top of this
    anyway. Otherwise we will need another piece to do basically the same thing
    and agglomerate multiple datagrams into a single BIO call. Unless we only
    want use `sendmmsg` constructively in trivial cases (e.g. where we send two
    datagrams from the same function immediately after one another... doesn't
    seem like a common use case.)

  - Flexible support for single-copy (zero-copy).

Cons of this approach:

  - Very different way of doing reads/writes might be strange to existing
    applications. *But* the primary consumer of this new API will be our own
    QUIC implementation so probably not a big deal. We can always support
    `BIO_read`/`BIO_write` as a less efficient fallback for existing third party
    users of BIO_dgram.

#### Compatibility interop

Suppose the following sequence happens:

1. BIO_read (legacy call path)
2. BIO_read_dequeue (`recvmmsg` based call path with callback-allocated buffer)
3. BIO_read (legacy call path)

For (1) we have two options

a. Use `recvmmsg` and add the received datagrams to an RX queue just as for the
   `BIO_read_dequeue` path. We use an OpenSSL-provided default allocator
   (`OPENSSL_malloc`) and flag these datagrams as needing to be freed by OpenSSL,
   not the application.

   When the application calls `BIO_read`, a copy is performed and the internal
   buffer is freed.

b. Use `recvfrom` directly. This means we have a `recvmmsg` path and a
   `recvfrom` path depending on what API is being used.

   The disadvantage of (a) is it yields an extra copy relative to what we have now,
   whereas with (b) the buffer passed to `BIO_read` gets passed through to the
   syscall and we do not have to copy anything.

   Since we will probably need to support platforms without
   `sendmmsg`/`recvmmsg` support anyway, (b) seems like the better option.

For (2) the new API is used. Since the previous call to BIO_read is essentially
“stateless” (it's just a simple call to `recvfrom`, and doesn't require mutation
of any internal BIO state other than maybe the last datagram source/destination
address fields), BIO_dgram can go ahead and start using the `recvmmsg` code
path. Since the RX queue will obviously be empty at this point, it is
initialised and filled using `recvmmsg`, then one datagram is popped from it.

For (3) we have a legacy `BIO_read` but we have several datagrams still in the
RX queue. In this case we do have to copy - we have no choice. However this only
happens in circumstances where a user of BIO_dgram alternates between old and
new APIs, which should be very unusual.

Subsequently for (3) we have to free the buffer using the free callback. This is
an unusual case where BIO_dgram is responsible for freeing read buffers and not
the application (the only other case being premature destruction, see below).
But since this seems a very strange API usage pattern, we may just want to fail
in this case.

Probably not worth supporting this. So we can have the following rule:

- After the first call to `BIO_read_dequeue` is made on a BIO_dgram, all
  subsequent calls to ordinary `BIO_read` will fail.

Of course, all of the above applies analogously to the TX side.

#### BIO_dgram_pair

We will also implement from scratch a BIO_dgram_pair. This will be provided as a
BIO pair which provides identical semantics to the BIO_dgram above, both for the
legacy and zero-copy code paths.

#### Thread safety

It is a functional assumption of the above design that we would never want to
have more than one thread doing TX on the same BIO and never have more than one
thread doing RX on the same BIO.

If we did ever want to do this, multiple BIOs on the same FD is one possibility
(for the BIO_dgram case at least). But I don't believe there is any general
intention to support multithreaded use of a single BIO at this time (unless I am
mistaken), so this seems like it isn't an issue.

If we wanted to support multithreaded use of the same FD using the same BIO, we
would need to revisit the set-call-then-execute-call API approach above
(`BIO_(set|get)_dgram_(origin|dest)`) as this would pose a problem. But I mainly
mention this only for completeness. Our recent learnt lessons on cache
contention suggest that this probably wouldn't be a good idea anyway.

#### Other questions

BIO_dgram will call the allocation function to get buffers for `recvmmsg` to
fill. We might want to have a way to specify how many buffers it should offer to
`recvmmsg`, and thus how many buffers it allocates in advance.

#### Premature destruction

If BIO_dgram is freed before all datagrams are read, the read buffer free
callback is used to free any unreturned read buffers.
