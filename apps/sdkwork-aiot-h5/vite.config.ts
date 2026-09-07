import { resolveViteEnvironment, resolveLucideReactEntry } from '../../../sdkwork-specs/tools/vite-runtime-profile.mjs';
import { resolveBrowserDistOutDir } from '../../../sdkwork-specs/tools/browser-dist-layout.mjs';

import { defineConfig, loadEnv } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import path from 'node:path';

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, '.', '');

  return {
    build: {
      outDir: resolveBrowserDistOutDir(resolveViteEnvironment(mode, process.env)),
      emptyOutDir: true,
    },
  plugins: [react(), tailwindcss()],
  define: {
    'process.env.SDKWORK_ACCESS_TOKEN': JSON.stringify(env.VITE_ACCESS_TOKEN ?? ''),
  },
  resolve: {
    alias: {
    },
  },
  server: { port: 5176 },
  };
});
