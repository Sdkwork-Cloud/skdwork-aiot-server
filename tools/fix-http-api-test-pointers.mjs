import fs from 'node:fs';

const path = 'crates/sdkwork-iot-platform-service/tests/http_api_standard.rs';
let source = fs.readFileSync(path, 'utf8');

source = source.replace(
  /(\w+_json)\s*\n\s*\.pointer\("\/data\/commandId"\)\s*\n\s*\.and_then\(serde_json::Value::as_str\)\s*\n\s*\.expect\("([^"]+)"\)/g,
  'resource_data_str(&$1, "commandId").expect("$2")',
);
source = source.replace(
  /(\w+_json)\s*\n\s*\.pointer\("\/data\/commandName"\)\s*\n\s*\.and_then\(serde_json::Value::as_str\)/g,
  'resource_data_str(&$1, "commandName")',
);
source = source.replace(
  /(\w+_json)\s*\n\s*\.pointer\("\/data\/displayName"\)\s*\n\s*\.and_then\(serde_json::Value::as_str\)/g,
  'resource_data_str(&$1, "displayName")',
);
source = source.replace(
  /(\w+_json)\s*\n\s*\.pointer\("\/data\/tenantId"\)\s*\n\s*\.and_then\(serde_json::Value::as_str\)/g,
  'resource_data_str(&$1, "tenantId")',
);
source = source.replace(
  /(\w+_json)\s*\n\s*\.pointer\("\/data\/organizationId"\)\s*\n\s*\.and_then\(serde_json::Value::as_str\)/g,
  'resource_data_str(&$1, "organizationId")',
);
source = source.replace(
  /(\w+_json)\s*\n\s*\.pointer\("\/data\/desired\/volume"\)\s*\n\s*\.and_then\(serde_json::Value::as_i64\)/g,
  'resource_data_pointer(&$1, "/desired/volume").and_then(serde_json::Value::as_i64)',
);
source = source.replace(
  /(\w+_json)\s*\n\s*\.pointer\("\/data\/reported\/volume"\)\s*\n\s*\.and_then\(serde_json::Value::as_i64\)/g,
  'resource_data_pointer(&$1, "/reported/volume").and_then(serde_json::Value::as_i64)',
);
source = source.replace(
  /(\w+_json)\s*\n\s*\.pointer\("\/data\/desired\/ready"\)\s*\n\s*\.and_then\(serde_json::Value::as_bool\)/g,
  'resource_data_pointer(&$1, "/desired/ready").and_then(serde_json::Value::as_bool)',
);
source = source.replace(
  /(\w+_json)\s*\n\s*\.pointer\("\/data\/credentialId"\)\s*\n\s*\.and_then\(serde_json::Value::as_str\)/g,
  'resource_data_str(&$1, "credentialId")',
);
source = source.replace(
  /(\w+_json)\s*\n\s*\.pointer\("\/data\/status"\)\s*\n\s*\.and_then\(serde_json::Value::as_str\)/g,
  'resource_data_str(&$1, "status")',
);
source = source.replace(
  /(\w+_json)\s*\n\s*\.pointer\("\/data\/deviceId"\)\s*\n\s*\.and_then\(serde_json::Value::as_str\)/g,
  'resource_data_str(&$1, "deviceId")',
);
source = source.replace(
  /let (\w+) = (\w+)\s*\n\s*\.pointer\("\/data"\)\s*\n\s*\.and_then\(serde_json::Value::as_array\)\s*\n\s*\.expect\("([^"]+)"\);/g,
  'let $1 = list_data_items(&$2);',
);

fs.writeFileSync(path, source);
console.log('updated', path);
