import js from '@eslint/js';
import globals from 'globals';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  { ignores: ['node_modules/', 'dist/', 'src-tauri/target/', 'src-tauri/gen/'] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    languageOptions: { globals: { ...globals.es2023 } },
    rules: {
      '@typescript-eslint/consistent-type-imports': 'error',
      '@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_', varsIgnorePattern: '^_' }],
    },
  },
  {
    files: ['scripts/**', '*.config.{js,ts}'],
    languageOptions: { globals: { ...globals.node } },
  },
  // L'interface ne parle au système que par Tauri (règle 4) : pas de module Node, pas de HTML brut.
  {
    files: ['src/**'],
    languageOptions: { globals: { ...globals.browser } },
    rules: {
      'no-restricted-imports': ['error', { patterns: [{ group: ['node:*'], message: "Pas de module Node dans l'interface." }] }],
      'no-restricted-syntax': [
        'error',
        { selector: "JSXAttribute[name.name='dangerouslySetInnerHTML']", message: 'Jamais de HTML brut injecté (règle 4).' },
        { selector: "AssignmentExpression[left.property.name=/^(innerHTML|outerHTML)$/]", message: 'Jamais de HTML brut injecté (règle 4).' },
      ],
    },
  },
);
