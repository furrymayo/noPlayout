import { defineConfig } from 'vite';

export default defineConfig({
  root: 'host',
  base: './',
  build: {
    outDir: '../dist/host', emptyOutDir: true, target: 'es2022',
    rolldownOptions: {
      input: 'host/renderer.html',
      output: { entryFileNames: 'ograf-host.js', chunkFileNames: '[name]-[hash].js' },
    },
  },
});
