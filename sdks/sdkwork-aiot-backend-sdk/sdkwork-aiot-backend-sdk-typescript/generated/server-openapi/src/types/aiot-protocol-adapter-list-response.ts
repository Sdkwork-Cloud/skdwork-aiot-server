import type { AiotProtocolAdapter } from './aiot-protocol-adapter';
import type { PageInfo } from './page-info';

export interface AiotProtocolAdapterListResponse {
  code: string;
  msg?: string;
  data: AiotProtocolAdapter[];
  pageInfo: PageInfo;
}
