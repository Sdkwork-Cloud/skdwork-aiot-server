export interface Hardware {
  id: string;
  name: string;
  type: string; // e.g., 'camera', 'speaker', 'robot'
  status: 'online' | 'offline';
  boundAt: string;
  agentId?: string; // Associated agent
  agentName?: string;
}

export interface Agent {
  id: string;
  name: string;
  capabilities: string[];
}
