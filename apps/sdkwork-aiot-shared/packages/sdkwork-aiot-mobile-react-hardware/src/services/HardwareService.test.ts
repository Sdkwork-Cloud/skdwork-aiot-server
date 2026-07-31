import assert from "node:assert/strict";
import test from "node:test";

import { HardwareCapabilityUnavailableError, HardwareService } from "./HardwareService";

test("hardware operations fail closed until the owner SDK is composed", async () => {
  for (const operation of [
    () => HardwareService.getHardwareList(),
    () => HardwareService.getHardwareById("hardware-id"),
    () => HardwareService.bindHardware("Device", "camera", "activation-code"),
    () => HardwareService.deleteHardware("hardware-id"),
    () => HardwareService.updateHardwareName("hardware-id", "Device"),
    () => HardwareService.getAllAgents(),
    () => HardwareService.associateAgent("hardware-id", "agent-id"),
  ]) {
    await assert.rejects(operation, HardwareCapabilityUnavailableError);
  }
});
