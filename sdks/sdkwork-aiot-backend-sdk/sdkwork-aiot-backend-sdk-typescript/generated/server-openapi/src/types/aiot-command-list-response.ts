import type { AiotCommand } from './aiot-command';
import type { PageInfo } from './page-info';

export interface AiotCommandListResponse {
  code: string;
  msg?: string;
  data: AiotCommand[];
  pageInfo: PageInfo;
}
