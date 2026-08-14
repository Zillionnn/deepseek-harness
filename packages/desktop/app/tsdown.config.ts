import { defineConfig } from 'tsdown'

/**
 * The TypeScript face is an empty marker module (see `src/index.ts`); the
 * shell's runtime is the Rust program under `src-tauri/`.
 */
export default defineConfig({
  entry: ['lib/types/index.js'],
  outDir: 'lib',
  format: ['esm'],
  platform: 'node',
  target: 'es2024',
  fixedExtension: false,
  dts: false,
  clean: false,
})