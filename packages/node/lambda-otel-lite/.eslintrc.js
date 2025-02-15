module.exports = {
  root: true,
  ignorePatterns: [
    'dist/**/*',
    'coverage/**/*',
    'node_modules/**/*',
    '*.d.ts'
  ],
  overrides: [
    {
      files: ['src/**/*.ts', '__tests__/**/*.ts'],
      parser: '@typescript-eslint/parser',
      parserOptions: {
        project: ['./tsconfig.json', './tsconfig.test.json'],
        tsconfigRootDir: __dirname,
        sourceType: 'module'
      },
      plugins: ['@typescript-eslint'],
      extends: [
        'eslint:recommended',
        'plugin:@typescript-eslint/recommended'
      ],
      env: {
        'node': true,
        'jest': true
      },
      rules: {
        // Essential TypeScript rules
        '@typescript-eslint/no-explicit-any': 'off',
        '@typescript-eslint/explicit-function-return-type': 'off',
        '@typescript-eslint/no-unused-vars': ['error', { 
          argsIgnorePattern: '^_',
          varsIgnorePattern: '^_'
        }],

        // Basic code style
        'semi': ['error', 'always'],
        'quotes': ['error', 'single'],
        'indent': ['error', 2, { 'SwitchCase': 1 }],

        // Best practices
        'eqeqeq': ['error', 'always', { 'null': 'ignore' }],
        'no-console': ['warn', { allow: ['warn', 'error'] }],
        'curly': ['error', 'all']
      }
    },
    {
      files: ['src/**/*.js'],
      env: {
        'node': true
      },
      parserOptions: {
        ecmaVersion: 2020,
        sourceType: 'module'
      },
      rules: {
        'indent': ['error', 2, { 'SwitchCase': 1 }],
        'semi': ['error', 'always'],
        'quotes': ['error', 'single']
      }
    }
  ]
}; 