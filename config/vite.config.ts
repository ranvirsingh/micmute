import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';

export default defineConfig({
  root: fileURLToPath(new URL('../src/ui', import.meta.url)),
  build: {
    target: 'esnext',
    outDir: '../../dist',
    emptyOutDir: true,
  },
  clearScreen: false,
});
