// This file implements a very simple allocator for external scanners running
// in WASM. Allocation is just bumping a static pointer and growing the heap
// as needed, and freeing is just adding the freed region to a free list.
// When additional memory is allocated, the free list is searched first.
// If there is not a suitable region in the free list, the heap is
// grown as necessary, and the allocation is made at the end of the heap.
// When the heap is reset, all allocated memory is considered freed.

#ifdef TREE_SITTER_FEATURE_WASM

#include <stdio.h>
#include <unistd.h>
#include <stdlib.h>
#include <string.h>

extern void tree_sitter_debug_message(const char *, size_t);

#define PAGESIZE 0x10000
#define MAX_HEAP_SIZE (4 * 1024 * 1024)

typedef struct {
  size_t size;
  struct Region *next;
  char data[0];
} Region;

static Region *heap_end = NULL;
static Region *heap_start = NULL;
static Region *next = NULL;
static Region *free_list = NULL;

// Get the region metadata for the given heap pointer.
static inline Region *region_for_ptr(void *ptr) {
  return ((Region *)ptr) - 1;
}

// Get the location of the next region after the given region,
// if the given region had the given size.
static inline Region *region_after(Region *self, size_t len) {
  char *address = self->data + len;
  char *aligned = (char *)((uintptr_t)(address + 3) & ~0x3);
  return (Region *)aligned;
}

static void *get_heap_end() {
  return (void *)(__builtin_wasm_memory_size(0) * PAGESIZE);
}

static int grow_heap(size_t size) {
  size_t new_page_count = ((size - 1) / PAGESIZE) + 1;
  return __builtin_wasm_memory_grow(0, new_page_count) != SIZE_MAX;
}

// Clear out the heap, and move it to the given address.
void reset_heap(void *new_heap_start) {
  heap_start = new_heap_start;
  next = new_heap_start;
  heap_end = get_heap_end();
  free_list = NULL;
}

void *malloc(size_t size) {
  Region *prev = NULL;
  Region *curr = free_list;
  while (curr != NULL) {
    if (curr->size >= size) {
      if (prev == NULL) {
        free_list = curr->next;
      } else {
        prev->next = curr->next;
      }
      return &curr->data;
    }
    prev = curr;
    curr = curr->next;
  }

  Region *region_end = region_after(next, size);

  if (region_end > heap_end) {
    if ((char *)region_end - (char *)heap_start > MAX_HEAP_SIZE) {
      return NULL;
    }
    if (!grow_heap(size)) return NULL;
    heap_end = get_heap_end();
  }

  void *result = &next->data;
  next->size = size;
  next = region_end;

  return result;
}

void free(void *ptr) {
  if (ptr == NULL) return;

  Region *region = region_for_ptr(ptr);
  Region *region_end = region_after(region, region->size);

  // When freeing the last allocated pointer, re-use that
  // pointer for the next allocation.
  if (region_end == next) {
    next = region;
  } else {
    region->next = free_list;
    free_list = region;
  }
}

void *calloc(size_t count, size_t size) {
  void *result = malloc(count * size);
  memset(result, 0, count * size);
  return result;
}

void *realloc(void *ptr, size_t new_size) {
  if (ptr == NULL) {
    return malloc(new_size);
  }

  Region *region = region_for_ptr(ptr);
  Region *region_end = region_after(region, region->size);

  // When reallocating the last allocated region, return
  // the same pointer, and skip copying the data.
  if (region_end == next) {
    next = region;
    return malloc(new_size);
  }

  void *result = malloc(new_size);
  memcpy(result, &region->data, region->size);
  return result;
}

#endif
