export interface PageInfo {
  page: number;
  pageSize: number;
  total: number;
  hasMore: boolean;
  nextCursor?: string;
}
