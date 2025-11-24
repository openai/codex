Notes on `sharded-slab`'s implementation and design.

# Design

The sharded slab's design is strongly inspired by the ideas presented by
Leijen, Zorn, and de Moura in [Mimalloc: Free List Sharding in
Action][mimalloc]. In this report, the authors present a novel design for a
memory allocator based on a concept of _free list sharding_.

Memory allocators must keep track of what memory regions are not currently
allocated ("free") in order to provide them to future allocation requests.
The term [_free list_][freelist] refers to a technique for performing this
bookkeeping, where each free block stores a pointer to the next free block,
forming a linked list. The memory allocator keeps a pointer to the most
recently freed block, the _head_ of the free list. To allocate more memory,
the allocator pops from the free list by setting the head pointer to the
next free block of the current head block, and returning the previous head.
To deallocate a block, the block is pushed to the free list by setting its
first word to the current head pointer, and the head pointer is set to point
to the deallocated block. Most implementations of slab allocators backed by
arrays or vectors use a similar technique, where pointers are replaced by
indices into the backing array.

When allocations and deallocations can occur concurrently across threads,
they must synchronize accesses to the free list; either by putting the
entire allocator state inside of a lock, or by using atomic operations to
treat the free list as a lock-free structure (such as a [Treiber stack]). In
both cases, there is a significant performance cost — even when the free
list is lock-free, it is likely that a noticeable amount of time will be
spent in compare-and-swap loops. Ideally, the global synchronzation point
created by the single global free list could be avoided as much as possible.

The approach presented by Leijen, Zorn, and de Moura is to introduce
sharding and thus increase the granularity of synchronization significantly.
In mimalloc, the heap is _sharded_ so that each thread has its own
thread-local heap. Objects are always allocated from the local heap of the
thread where the allocation is performed. Because allocations are always
done from a thread's local heap, they need not be synchronized.

However, since objects can move between threads before being deallocated,
_deallocations_ may still occur concurrently. Therefore, Leijen et al.
introduce a concept of _local_ and _global_ free lists. When an object is
deallocated on the same thread it was originally allocated on, it is placed
on the local free list; if it is deallocated on another thread, it goes on
the global free list for the heap of the thread from which it originated. To
allocate, the local free list is used first; if it is empty, the entire
global free list is popped onto the local free list. Since the local free
list is only ever accessed by the thread it belongs to, it does not require
synchronization at all, and because the global free list is popped from
infrequently, the cost of synchronization has a reduced impact. A majority
of allocations can occur without any synchronization at all; and
deallocations only require synchronization when an object has left its
parent thread (a relatively uncommon case).

[mimalloc]: https://www.microsoft.com/en-us/research/uploads/prod/2019/06/mimalloc-tr-v1.pdf
[freelist]: https://en.wikipedia.org/wiki/Free_list
[Treiber stack]: https://en.wikipedia.org/wiki/Treiber_stack

# Implementation

A slab is represented as an array of [`MAX_THREADS`] _shards_. A shard
consists of a vector of one or more _pages_ plus associated metadata.
Finally, a page consists of an array of _slots_, head indices for the local
and remote free lists.

```text
┌─────────────┐
│ shard 1     │
│             │    ┌─────────────┐        ┌────────┐
│ pages───────┼───▶│ page 1      │        │        │
├─────────────┤    ├─────────────┤  ┌────▶│  next──┼─┐
│ shard 2     │    │ page 2      │  │     ├────────┤ │
├─────────────┤    │             │  │     │XXXXXXXX│ │
│ shard 3     │    │ local_head──┼──┘     ├────────┤ │
└─────────────┘    │ remote_head─┼──┐     │        │◀┘
      ...          ├─────────────┤  │     │  next──┼─┐
┌─────────────┐    │ page 3      │  │     ├────────┤ │
│ shard n     │    └─────────────┘  │     │XXXXXXXX│ │
└─────────────┘          ...        │     ├────────┤ │
                   ┌─────────────┐  │     │XXXXXXXX│ │
                   │ page n      │  │     ├────────┤ │
                   └─────────────┘  │     │        │◀┘
                                    └────▶│  next──┼───▶  ...
                                          ├────────┤
                                          │XXXXXXXX│
                                          └────────┘
```


The size of the first page in a shard is always a power of two, and every
subsequent page added after the first is twice as large as the page that
preceeds it.

```text

pg.
┌───┐   ┌─┬─┐
│ 0 │───▶ │ │
├───┤   ├─┼─┼─┬─┐
│ 1 │───▶ │ │ │ │
├───┤   ├─┼─┼─┼─┼─┬─┬─┬─┐
│ 2 │───▶ │ │ │ │ │ │ │ │
├───┤   ├─┼─┼─┼─┼─┼─┼─┼─┼─┬─┬─┬─┬─┬─┬─┬─┐
│ 3 │───▶ │ │ │ │ │ │ │ │ │ │ │ │ │ │ │ │
└───┘   └─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┘
```

When searching for a free slot, the smallest page is searched first, and if
it is full, the search proceeds to the next page until either a free slot is
found or all available pages have been searched. If all available pages have
been searched and the maximum number of pages has not yet been reached, a
new page is then allocated.

Since every page is twice as large as the previous page, and all page sizes
are powers of two, we can determine the page index that contains a given
address by shifting the address down by the smallest page size and
looking at how many twos places necessary to represent that number,
telling us what power of two page size it fits inside of. We can
determine the number of twos places by counting the number of leading
zeros (unused twos places) in the number's binary representation, and
subtracting that count from the total number of bits in a word.

The formula for determining the page number that contains an offset is thus:

```rust,ignore
WIDTH - ((offset + INITIAL_PAGE_SIZE) >> INDEX_SHIFT).leading_zeros()
```

where `WIDTH` is the number of bits in a `usize`, and `INDEX_SHIFT` is

```rust,ignore
INITIAL_PAGE_SIZE.trailing_zeros() + 1;
```

[`MAX_THREADS`]: https://docs.rs/sharded-slab/latest/sharded_slab/trait.Config.html#associatedconstant.MAX_THREADS
