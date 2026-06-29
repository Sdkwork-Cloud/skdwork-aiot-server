import type { AiotDevice } from './aiot-device';
import type { PageInfo } from './page-info';

export interface AiotDeviceListResponse {
  code: string;
  msg?: string;
  data: AiotDevice[];
  pageInfo: PageInfo;
}
