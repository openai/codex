console.error('[DEBUG] Top-level: ResourcesList file loaded, MOCK_RESOURCES_LENGTH:', process.env['MOCK_RESOURCES_LENGTH']);

import React, { useState, useEffect } from 'react';
import { Text } from 'ink';
import { fetchPaginated, PaginationState } from '../../../agent/pagination.js';
import { PaginatedList } from '../../../ui/PaginatedList.js';

console.error('[DEBUG] Before ResourcesList component definition, MOCK_RESOURCES_LENGTH:', process.env['MOCK_RESOURCES_LENGTH']);

async function fetchResourcesPage({ page, pageSize }: { page: number; pageSize: number }) {
  // Simulate async fetch
  await new Promise(r => setTimeout(r, 100));
  const mockLength = parseInt(process.env['MOCK_RESOURCES_LENGTH'] || '57', 10);
  const MOCK_RESOURCES = Array.from({ length: mockLength }, (_, i) => ({ name: `Resource #${i + 1}` }));
  const start = page * pageSize;
  const end = start + pageSize;
  const items = MOCK_RESOURCES.slice(start, end);
  // Debug output for E2E
  if (process.env['DEBUG_PAGINATION']) {
    console.error('[DEBUG] fetchResourcesPage', {
      mockLength,
      start,
      end,
      itemsReturned: items.length,
      itemNames: items.map(i => i.name)
    });
  }
  return {
    items,
    total: MOCK_RESOURCES.length,
    nextCursor: end < MOCK_RESOURCES.length ? String(end) : undefined,
  };
}

export default function ResourcesList() {
  console.error('[DEBUG] ResourcesList component rendered, MOCK_RESOURCES_LENGTH:', process.env['MOCK_RESOURCES_LENGTH']);
  // Separate pagination state from data state
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(10);
  const [data, setData] = useState<PaginationState<{ name: string }>>({
    items: [],
    page: 0,
    pageSize: 10,
    hasNext: true,
    hasPrev: false,
  });

  // Fetch on mount and when page/pageSize changes
  useEffect(() => {
    fetchPaginated(fetchResourcesPage, { page, pageSize, items: [], hasNext: true, hasPrev: false }).then(newState => {
      if (process.env['DEBUG_PAGINATION']) {
        console.log('[DEBUG] Pagination fetch', {
          requestedPage: page,
          pageSize,
          itemsReturned: newState.items.length,
          total: newState.total,
          hasNext: newState.hasNext,
          hasPrev: newState.hasPrev
        });
      }
      // Debug: log what is being set
      console.error('[DEBUG] setData called with:', {
        ...newState,
        page,
        pageSize
      });
      setData({ ...newState, page, pageSize });
    });
  }, [page, pageSize]);

  // Debug: log state before rendering PaginatedList
  if (process.env['DEBUG_PAGINATION']) {
    console.error('[DEBUG] Before render PaginatedList', {
      state: data,
      itemsLength: data.items.length,
      items: data.items.map(i => i.name)
    });
  }

  // Prevent quitting until at least one fetch/render with items or empty is complete
  const [canQuit, setCanQuit] = useState(false);
  useEffect(() => {
    setCanQuit(true);
  }, [data.items]);

  return (
    <PaginatedList
      state={data}
      renderItem={(item, idx) => <Text key={idx}>{item.name}</Text>}
      onNext={() => setPage(p => p + 1)}
      onPrev={() => setPage(p => Math.max(0, p - 1))}
      onQuit={() => {
        if (canQuit) process.exit(0);
      }}
    />
  );
}

// To run interactively: render(<ResourcesList />);
