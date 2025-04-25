// REMOVAL: This file is for development-time logic validation only. REMOVE BEFORE MERGE TO MAIN.
// (You may keep it for local dev confidence, but it should not ship to main or production.)
// TEMPORARY PURE LOGIC TEST FOR PAGINATION
// TODO: Remove this file before merging to main branch.
// This test bypasses Ink/TTY and tests only the core pagination logic.

import { describe, it, expect } from 'vitest';
import { fetchPaginated, PaginationState } from '../../../agent/pagination.js';

// Mock fetchResourcesPage (sync version for testing)
function makeMockFetchResourcesPage(length: number) {
  const MOCK_RESOURCES = Array.from({ length }, (_, i) => ({ name: `Resource #${i + 1}` }));
  return async ({ page, pageSize }: { page: number; pageSize: number }) => {
    const start = page * pageSize;
    const end = start + pageSize;
    const items = MOCK_RESOURCES.slice(start, end);
    return {
      items,
      total: MOCK_RESOURCES.length,
      nextCursor: end < MOCK_RESOURCES.length ? String(end) : undefined,
    };
  };
}

describe('Pagination Logic (Pure)', () => {
  it('handles single item', async () => {
    const fetchPage = makeMockFetchResourcesPage(1);
    const state: PaginationState<{ name: string }> = { items: [], page: 0, pageSize: 10, hasNext: true, hasPrev: false };
    const newState = await fetchPaginated(fetchPage, state);
    expect(newState.items.length).toBe(1);
    expect(newState.items[0].name).toBe('Resource #1');
    expect(newState.hasNext).toBe(false);
    expect(newState.hasPrev).toBe(false);
  });

  it('handles exact page size', async () => {
    const fetchPage = makeMockFetchResourcesPage(10);
    const state: PaginationState<{ name: string }> = { items: [], page: 0, pageSize: 10, hasNext: true, hasPrev: false };
    const newState = await fetchPaginated(fetchPage, state);
    expect(newState.items.length).toBe(10);
    expect(newState.items[9].name).toBe('Resource #10');
    expect(newState.hasNext).toBe(false);
    expect(newState.hasPrev).toBe(false);
  });

  it('handles page-size-plus-one', async () => {
    const fetchPage = makeMockFetchResourcesPage(11);
    const state: PaginationState<{ name: string }> = { items: [], page: 0, pageSize: 10, hasNext: true, hasPrev: false };
    const firstPage = await fetchPaginated(fetchPage, state);
    expect(firstPage.items.length).toBe(10);
    expect(firstPage.hasNext).toBe(true);
    // Go to next page
    const secondPage = await fetchPaginated(fetchPage, { ...firstPage, page: 1 });
    expect(secondPage.items.length).toBe(1);
    expect(secondPage.items[0].name).toBe('Resource #11');
    expect(secondPage.hasNext).toBe(false);
    expect(secondPage.hasPrev).toBe(true);
  });

  it('handles multi-page navigation', async () => {
    const fetchPage = makeMockFetchResourcesPage(25);
    let state: PaginationState<{ name: string }> = { items: [], page: 0, pageSize: 10, hasNext: true, hasPrev: false };
    // Page 1
    state = await fetchPaginated(fetchPage, state);
    expect(state.items[0].name).toBe('Resource #1');
    expect(state.hasNext).toBe(true);
    // Page 2
    state = await fetchPaginated(fetchPage, { ...state, page: 1 });
    expect(state.items[0].name).toBe('Resource #11');
    expect(state.hasNext).toBe(true);
    expect(state.hasPrev).toBe(true);
    // Page 3
    state = await fetchPaginated(fetchPage, { ...state, page: 2 });
    expect(state.items[0].name).toBe('Resource #21');
    expect(state.hasNext).toBe(false);
    expect(state.hasPrev).toBe(true);
  });

  it('handles no resources', async () => {
    const fetchPage = makeMockFetchResourcesPage(0);
    const state: PaginationState<{ name: string }> = { items: [], page: 0, pageSize: 10, hasNext: true, hasPrev: false };
    const newState = await fetchPaginated(fetchPage, state);
    expect(newState.items.length).toBe(0);
    expect(newState.hasNext).toBe(false);
    expect(newState.hasPrev).toBe(false);
  });

  it('handles less than page size', async () => {
    const fetchPage = makeMockFetchResourcesPage(5);
    const state: PaginationState<{ name: string }> = { items: [], page: 0, pageSize: 10, hasNext: true, hasPrev: false };
    const newState = await fetchPaginated(fetchPage, state);
    expect(newState.items.length).toBe(5);
    expect(newState.hasNext).toBe(false);
    expect(newState.hasPrev).toBe(false);
  });

  it('handles exactly two pages', async () => {
    const fetchPage = makeMockFetchResourcesPage(20);
    let state: PaginationState<{ name: string }> = { items: [], page: 0, pageSize: 10, hasNext: true, hasPrev: false };
    state = await fetchPaginated(fetchPage, state);
    expect(state.items.length).toBe(10);
    expect(state.hasNext).toBe(true);
    // Go to next page
    state = await fetchPaginated(fetchPage, { ...state, page: 1 });
    expect(state.items.length).toBe(10);
    expect(state.items[0].name).toBe('Resource #11');
    expect(state.hasNext).toBe(false);
    expect(state.hasPrev).toBe(true);
    // Go back to prev page
    state = await fetchPaginated(fetchPage, { ...state, page: 0 });
    expect(state.items[0].name).toBe('Resource #1');
    expect(state.hasPrev).toBe(false);
  });

  it('handles going past last page', async () => {
    const fetchPage = makeMockFetchResourcesPage(15);
    let state: PaginationState<{ name: string }> = { items: [], page: 0, pageSize: 10, hasNext: true, hasPrev: false };
    state = await fetchPaginated(fetchPage, state);
    // Go to page 2 (should be last page)
    state = await fetchPaginated(fetchPage, { ...state, page: 1 });
    expect(state.items.length).toBe(5);
    expect(state.hasNext).toBe(false);
    // Try going past last page
    state = await fetchPaginated(fetchPage, { ...state, page: 2 });
    expect(state.items.length).toBe(0);
    expect(state.hasNext).toBe(false);
    expect(state.hasPrev).toBe(true);
  });

  it('handles going before first page', async () => {
    const fetchPage = makeMockFetchResourcesPage(15);
    let state: PaginationState<{ name: string }> = { items: [], page: 0, pageSize: 10, hasNext: true, hasPrev: false };
    // Try negative page
    state = await fetchPaginated(fetchPage, { ...state, page: -1 });
    expect(state.items.length).toBe(0);
    expect(state.hasNext).toBe(true);
    expect(state.hasPrev).toBe(false);
  });

  it('handles rapid page changes', async () => {
    const fetchPage = makeMockFetchResourcesPage(35);
    let state: PaginationState<{ name: string }> = { items: [], page: 0, pageSize: 10, hasNext: true, hasPrev: false };
    // Rapidly go to page 3
    state = await fetchPaginated(fetchPage, { ...state, page: 3 });
    expect(state.items.length).toBe(5);
    expect(state.items[0].name).toBe('Resource #31');
    expect(state.hasNext).toBe(false);
    expect(state.hasPrev).toBe(true);
    // Go back to page 1
    state = await fetchPaginated(fetchPage, { ...state, page: 0 });
    expect(state.items.length).toBe(10);
    expect(state.items[0].name).toBe('Resource #1');
    expect(state.hasPrev).toBe(false);
  });

  it('handles page size of 1', async () => {
    const fetchPage = makeMockFetchResourcesPage(3);
    let state: PaginationState<{ name: string }> = { items: [], page: 0, pageSize: 1, hasNext: true, hasPrev: false };
    state = await fetchPaginated(fetchPage, state);
    expect(state.items.length).toBe(1);
    expect(state.items[0].name).toBe('Resource #1');
    state = await fetchPaginated(fetchPage, { ...state, page: 1 });
    expect(state.items[0].name).toBe('Resource #2');
    state = await fetchPaginated(fetchPage, { ...state, page: 2 });
    expect(state.items[0].name).toBe('Resource #3');
    state = await fetchPaginated(fetchPage, { ...state, page: 3 });
    expect(state.items.length).toBe(0);
  });
});
