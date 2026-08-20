export interface PageInfo {
  /** Pagination mode for this collection. */
  mode: 'offset' | 'cursor';
  page: number;
  pageSize: number;
  total: number;
  hasMore: boolean;
}
