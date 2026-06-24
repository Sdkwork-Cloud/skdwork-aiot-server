-- SDKWork AIoT standard bootstrap seed (tenant 100001).
-- Provides a default Xiaozhi-compatible product template for dev and pilot deployments.

INSERT OR IGNORE INTO iot_product (
    id,
    uuid,
    tenant_id,
    organization_id,
    data_scope,
    owner_type,
    owner_id,
    product_key,
    display_name,
    status,
    created_at,
    updated_at,
    version
) VALUES (
    1,
    'aiot-product-xiaozhi-default',
    100001,
    0,
    0,
    'tenant',
    '100001',
    'xiaozhi.esp32.default',
    'Xiaozhi ESP32 Default',
    1,
    CURRENT_TIMESTAMP,
    CURRENT_TIMESTAMP,
    0
);

INSERT OR IGNORE INTO iot_hardware_profile (
    id,
    uuid,
    tenant_id,
    organization_id,
    data_scope,
    owner_type,
    owner_id,
    profile_key,
    chip_family,
    runtime_profile,
    connectivity_profile,
    security_profile,
    ota_profile,
    status,
    created_at,
    updated_at,
    version
) VALUES (
    1,
    'aiot-hardware-profile-esp32-idf',
    100001,
    0,
    0,
    'tenant',
    '100001',
    'esp32-idf-xiaozhi',
    'esp32',
    'esp_idf',
    'wifi',
    'device_bearer',
    'xiaozhi_ota',
    1,
    CURRENT_TIMESTAMP,
    CURRENT_TIMESTAMP,
    0
);
