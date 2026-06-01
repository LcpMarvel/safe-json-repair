import { defineConfig } from 'tsup';

export default defineConfig({
  entry: ['src/index.ts'],
  format: ['esm', 'cjs'],
  dts: true,
  clean: true,
  // No sourcemap: the bundle is a thin wrapper plus a large base64 wasm blob, so
  // a sourcemap just duplicates that blob and doubles the package for no value.
  sourcemap: false,
  treeshake: true,
  target: 'node18',
  // Zero runtime dependencies. The only bundled-in code is the generated
  // wasm-bindgen glue + the inlined wasm bytes (src/generated/).
  minify: false,
});
