import { defineConfig, loadEnv } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'node:path';

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, __dirname, '');
  return {
    define: {
      'process.env.SDKWORK_ACCESS_TOKEN': JSON.stringify(env.VITE_ACCESS_TOKEN ?? ''),
    },
          plugins: [react()],
  resolve: {
    alias: {
    },
  },
  server: {
    port: 5175,
  },
  };
});
