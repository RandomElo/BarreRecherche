import { defineConfig } from 'vitest/config';

// Logique TypeScript de l'interface. Le Rust se teste par cargo test (npm run test lance les deux).
export default defineConfig({
  define: { __VERSION__: JSON.stringify('0.0.0-test') },
  test: { environment: 'node', include: ['src/**/*.test.ts'] },
});
