import type { JsonValue } from './json-value';
import type { PageInfo } from './page-info';

export interface AiotDeviceListResponse {
  code: string;
  msg?: string;
  data: JsonValue[];
  pageInfo: PageInfo;
}
