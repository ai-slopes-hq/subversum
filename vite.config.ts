import { defineConfig } from 'vitest/config';

export default defineConfig({
  root: 'web',
  base: '/next/',
  build: { outDir: '../dist/next', emptyOutDir: true },
  worker: { format: 'es' },
  test: { root: '.', include: ['web/**/*.test.ts'] },
});
