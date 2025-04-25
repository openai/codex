export interface PaginationState<T> {
  items: T[];
  page: number;
  pageSize: number;
  total?: number;
  hasNext: boolean;
  hasPrev: boolean;
  cursor?: string;
}

export type FetchPageFn<T> = (params: { page: number; pageSize: number; cursor?: string }) => Promise<{ items: T[]; total?: number; nextCursor?: string }>;

export async function fetchPaginated<T>(
  fetchFn: FetchPageFn<T>,
  state: PaginationState<T>
): Promise<PaginationState<T>> {
  const { items, total, nextCursor } = await fetchFn({
    page: state.page,
    pageSize: state.pageSize,
    cursor: state.cursor,
  });
  return {
    items,
    page: state.page,
    pageSize: state.pageSize,
    total,
    hasNext: !!nextCursor || (typeof total === 'number' ? (state.page + 1) * state.pageSize < total : false),
    hasPrev: state.page > 0,
    cursor: nextCursor,
  };
}
