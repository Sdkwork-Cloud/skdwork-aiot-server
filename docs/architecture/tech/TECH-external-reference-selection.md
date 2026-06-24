> Migrated from `docs/external-reference-selection.md` on 2026-06-24.
> Owner: SDKWork maintainers

# External Reference Selection

Selection date: 2026-05-31

The `external/` directory is intentionally curated. It is not a broad IoT
source mirror. Submodules should be kept only when they are high-signal
references for SDKWork AIoT server design.

## Rules

- Keep direct product anchors required by the platform, even if they are an
  explicit exception.
- Keep smart-hardware, firmware, protocol bridge, chip SDK, runtime, or IoT
  server references with more than 10k GitHub stars at selection time.
- Do not keep low-signal protocol library mirrors only because the protocol is
  supported by the domain model.
- MQTT broker/server implementation is standardized on RMQTT.

## Current Curated Set

| Submodule | Upstream | Reason |
| --- | --- | --- |
| `external/xiaozhi-esp32` | https://github.com/78/xiaozhi-esp32.git | Primary Xiaozhi intelligent hardware compatibility reference. |
| `external/rmqtt` | https://github.com/rmqtt/rmqtt.git | Explicit MQTT broker/server implementation selection. |
| `external/esphome` | https://github.com/esphome/esphome.git | High-star smart-hardware firmware and component model. |
| `external/tasmota` | https://github.com/arendst/Tasmota.git | High-star device firmware and MQTT command model. |
| `external/zigbee2mqtt` | https://github.com/Koenkk/zigbee2mqtt.git | High-star Zigbee bridge and MQTT topic mapping. |
| `external/wled` | https://github.com/wled/WLED.git | High-star smart lighting firmware and MQTT/JSON control model. |
| `external/esp-idf` | https://github.com/espressif/esp-idf.git | High-star official Espressif chip SDK and runtime baseline. |
| `external/arduino-esp32` | https://github.com/espressif/arduino-esp32.git | High-star ESP32 Arduino hardware runtime baseline. |
| `external/micropython` | https://github.com/micropython/micropython.git | High-star microcontroller runtime and firmware model. |
| `external/zephyr` | https://github.com/zephyrproject-rtos/zephyr.git | High-star RTOS and embedded hardware abstraction baseline. |
| `external/thingsboard` | https://github.com/thingsboard/thingsboard.git | High-star IoT server/product model reference. |

## Non-Vendored Protocol References

The protocol catalog can still model CoAP/LwM2M, Matter, LoRaWAN, Modbus,
OPC UA, and other standards. Those protocol abstractions do not require keeping
their lower-star reference implementations in `external/`.

Raspberry Pi Linux SBC/gateway and Raspberry Pi Pico/RP2040 MCU profiles are
modeled in the core hardware and protocol catalogs:

- `raspberrypi.linux_gateway` covers Linux SBC and edge-gateway deployments
  that bridge MQTT, HTTP, WebSocket, USB radios, camera/audio workloads, and
  downstream protocols.
- `raspberrypi.pico_mqtt` covers Pico/Pico W MCU firmware deployments that use
  MQTT or HTTP-compatible firmware and telemetry patterns.

No Raspberry Pi-specific source tree is kept in `external/` at this stage. This
keeps the reference set focused and avoids adding OS images, sample projects, or
board SDK mirrors unless one becomes a primary implementation anchor later.

