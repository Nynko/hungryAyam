import { defineConfig } from 'vite';
import solidPlugin from 'vite-plugin-solid';
import devtools from 'solid-devtools/vite';
// @ts-expect-error
import path from "node:path";

export default defineConfig({
  plugins: [devtools(), solidPlugin()],
  resolve: {
    alias: {
      // @ts-expect-error process is a nodejs global
      "@": path.resolve(process.cwd(), "src"),
      // @ts-expect-error process is a nodejs global
      "@bindings": path.resolve(process.cwd(), "../backend/bindings"),
    },
  },
  css: {
      preprocessorOptions: {
        scss: {
          silenceDeprecations: ['if-function'],
        },
      },
    },
  server: {
    port: 3000,
    proxy: {
      "/api": {
        target: "http://localhost:3001",
        changeOrigin: true,
      },
      "/setup": {
        target: "http://localhost:3001",
        changeOrigin: true,
      },
    },
  },
  build: {
    target: 'esnext',
  },
});
