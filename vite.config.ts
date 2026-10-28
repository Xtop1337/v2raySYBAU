import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Tauri expects a fixed dev port and serves the built files from ../dist.
export default defineConfig({
  root: 'ui',
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { outDir: '../dist', emptyOutDir: true, target: 'es2022' },
});
