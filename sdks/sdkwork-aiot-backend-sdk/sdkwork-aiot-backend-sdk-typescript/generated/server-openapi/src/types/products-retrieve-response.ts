import type { AiotProductResponse } from './aiot-product-response';

export interface ProductsRetrieveResponse {
  code: 0;
  data: unknown & Record<string, unknown>;
  /** Server-owned request correlation id. */
  traceId: string;
}
