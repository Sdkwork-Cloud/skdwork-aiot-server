import { defineConfig, loadEnv } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import path from 'node:path';

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, '.', '');

  return {
  plugins: [react(), tailwindcss()],
  define: {
    'process.env.SDKWORK_ACCESS_TOKEN': JSON.stringify(env.SDKWORK_ACCESS_TOKEN ?? ''),
  },
  resolve: {
    alias: {
      '@sdkwork/aiot-h5-core': path.resolve(__dirname, 'packages/sdkwork-aiot-h5-core/src/index.ts'),
      '@sdkwork/aiot-h5-console-agent': path.resolve(__dirname, 'packages/sdkwork-aiot-h5-console-agent/src/index.ts'),
      '@sdkwork/aiot-h5-console-device': path.resolve(__dirname, 'packages/sdkwork-aiot-h5-console-device/src/index.ts'),
      '@sdkwork/aiot-h5-console-iot': path.resolve(__dirname, 'packages/sdkwork-aiot-h5-console-iot/src/index.ts'),
      '@sdkwork/aiot-h5-console-voice': path.resolve(__dirname, 'packages/sdkwork-aiot-h5-console-voice/src/index.ts'),
      '@sdkwork/aiot-app-core': path.resolve(__dirname, '../sdkwork-aiot-shared/packages/sdkwork-aiot-app-core/src/index.ts'),
      '@sdkwork/aiot-app-sdk': path.resolve(__dirname, '../../sdks/sdkwork-aiot-app-sdk/sdkwork-aiot-app-sdk-typescript/src/index.ts'),
      '@sdkwork/agents-app-sdk': path.resolve(__dirname, '../../../sdkwork-agents/sdks/sdkwork-agents-app-sdk/sdkwork-agents-app-sdk-typescript/src/index.ts'),
      '@sdkwork/voice-app-sdk': path.resolve(__dirname, '../../../sdkwork-voice/sdks/sdkwork-voice-app-sdk/sdkwork-voice-app-sdk-typescript/src/index.ts'),
    },
  },
  server: { port: 5176 },
  };
});
