module.exports = {
  root: true,
  ignorePatterns: [
    'dist/**/*',
    'coverage/**/*',
    'node_modules/**/*',
    '**/*.js',
    '**/*.d.ts'
  ],
  overrides: [
    {
      files: ['src/**/*.ts'],
      parser: '@typescript-eslint/parser',
      parserOptions: {
        project: './tsconfig.eslint.json',
        tsconfigRootDir: __dirname,
        sourceType: 'module'
      },
      plugins: ['@typescript-eslint'],
      extends: [
        'eslint:recommended',
        'plugin:@typescript-eslint/recommended'
      ],
      rules: {
        // Essential TypeScript rules
        '@typescript-eslint/no-explicit-any': 'warn',
        '@typescript-eslint/explicit-function-return-type': 'off',
        '@typescript-eslint/no-unused-vars': ['error', { 
          argsIgnorePattern: '^_',
          varsIgnorePattern: '^_'
        }],

        // Basic code style
        'semi': ['error', 'always'],
        'quotes': ['error', 'single'],
        'indent': ['error', 2],

        // Best practices
        'eqeqeq': ['error', 'always', { 'null': 'ignore' }],
        'no-console': ['warn', { allow: ['warn', 'error'] }],
        'curly': ['error', 'all']
      }
    }
  ]
}; 