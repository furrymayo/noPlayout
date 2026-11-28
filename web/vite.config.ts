import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vitest/config';

// The Svelte UI lives in ui/ and builds to dist/ui, which the Tauri app embeds (tauri.conf.json).
export default defineConfig({
  root: 'ui',
  base: './',
  plugins: [svelte()],
  build: {
    outDir: '../dist/ui',
    emptyOutDir: true,
    target: 'es2022',
  },
  test: {
    include: ['**/*.test.ts', '../host/**/*.test.ts'],
  },
});
