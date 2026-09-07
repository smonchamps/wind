import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Relative base: the bundle is embedded by Tauri (frontendDist), not
// served from a domain root.
export default defineConfig({
  base: './',
  plugins: [svelte()],
  // `esnext`: the only runtimes are the embedded engines — WebView2
  // (current Chromium) on Windows, WKWebView (current Safari) on macOS
  // (support matrix, MACOS-BUILD.md) — and both take main.js's
  // top-level module await (language restored before mount) without
  // transformation.
  build: { outDir: 'dist', emptyOutDir: true, target: 'esnext' },
});
