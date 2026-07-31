import type { Agent, Hardware } from "../types";

export class HardwareCapabilityUnavailableError extends Error {
  constructor() {
    super("Hardware is unavailable because the AIoT owner SDK is not composed.");
    this.name = "HardwareCapabilityUnavailableError";
  }
}

export const HardwareService = {
  async getHardwareList(): Promise<Hardware[]> {
    throw new HardwareCapabilityUnavailableError();
  },

  async getHardwareById(_id: string): Promise<Hardware | undefined> {
    throw new HardwareCapabilityUnavailableError();
  },

  async bindHardware(_name: string, _type: string, _activationCode: string): Promise<Hardware> {
    throw new HardwareCapabilityUnavailableError();
  },

  async deleteHardware(_id: string): Promise<void> {
    throw new HardwareCapabilityUnavailableError();
  },

  async updateHardwareName(_id: string, _name: string): Promise<Hardware> {
    throw new HardwareCapabilityUnavailableError();
  },

  async getAllAgents(): Promise<Agent[]> {
    throw new HardwareCapabilityUnavailableError();
  },

  async associateAgent(_hardwareId: string, _agentId: string | undefined): Promise<Hardware> {
    throw new HardwareCapabilityUnavailableError();
  },
};
