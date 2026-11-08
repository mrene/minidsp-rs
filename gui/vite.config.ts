import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// During development, API and websocket calls are forwarded to a locally running minidspd
const daemon = process.env.MINIDSPD_URL ?? 'http://127.0.0.1:5380';

export default defineConfig({
  plugins: [svelte()],
  server: {
    proxy: {
      '/devices': { target: daemon, ws: true },
      '/openapi.json': daemon,
      '/api': daemon,
    },
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
});
