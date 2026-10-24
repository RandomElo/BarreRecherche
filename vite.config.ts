import { readFileSync } from 'node:fs';
import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

const { version } = JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf8')) as { version: string };

// Le port est fixe : Tauri l'attend (devUrl de src-tauri/tauri.conf.json).
export default defineConfig({
  plugins: [react(), tailwindcss()],
  define: { __VERSION__: JSON.stringify(version) },
  clearScreen: false,
  server: { port: 1420, strictPort: true, host: '127.0.0.1' },
  build: {
    target: 'safari16',
    outDir: 'dist',
    // Deux pages : la barre (index.html) et les réglages, ouverts à la demande.
    rollupOptions: { input: { barre: 'index.html', reglages: 'reglages.html' } },
  },
});
