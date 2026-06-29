import type { AiotEvent } from './aiot-event';
import type { PageInfo } from './page-info';

export interface AiotEventListResponse {
  code: string;
  msg?: string;
  data: AiotEvent[];
  pageInfo: PageInfo;
}
