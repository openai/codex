import React, { useState, useEffect } from 'react';
import { render, Text } from 'ink';
import { fetchPaginated, PaginationState } from '../../agent';
import { PaginatedList } from '../../ui/PaginatedList';

// Mock data for demonstration
const MOCK_TEMPLATES = Array.from({ length: 33 }, (_, i) => ({ name: `Template #${i + 1}` }));

async function fetchTemplatesPage({ page, pageSize }: { page: number; pageSize: number }) {
  // Simulate async fetch
  await new Promise(r => setTimeout(r, 100));
  const start = page * pageSize;
  const end = start + pageSize;
  const items = MOCK_TEMPLATES.slice(start, end);
  return {
    items,
    total: MOCK_TEMPLATES.length,
    nextCursor: end < MOCK_TEMPLATES.length ? String(end) : undefined,
  };
}

export default function TemplatesList() {
  const [state, setState] = useState<PaginationState<{ name: string }>>({
    items: [],
    page: 0,
    pageSize: 10,
    hasNext: true,
    hasPrev: false,
  });

  useEffect(() => {
    fetchPaginated(fetchTemplatesPage, state).then(setState);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state.page, state.pageSize]);

  return (
    <PaginatedList
      state={state}
      renderItem={(item, idx) => <Text key={idx}>{item.name}</Text>}
      onNext={() => setState(s => ({ ...s, page: s.page + 1 }))}
      onPrev={() => setState(s => ({ ...s, page: Math.max(0, s.page - 1) }))}
      onQuit={() => process.exit(0)}
    />
  );
}

// To run interactively: render(<TemplatesList />);
