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
      files: ['src/**/*.ts'],
      parser: '@typescript-eslint/parser',
      parserOptions: {
        project: ['./tsconfig.json'],
        tsconfigRootDir: __dirname,
        sourceType: 'module'
      },
      plugins: ['@typescript-eslint'],
      extends: [
        'eslint:recommended',
        'plugin:@typescript-eslint/recommended',
        'prettier'
      ],
      env: {
        'node': true
      },
      rules: {
        '@typescript-eslint/no-explicit-any': 'off',
        '@typescript-eslint/explicit-function-return-type': 'off',
        '@typescript-eslint/no-unused-vars': ['error', { 
          argsIgnorePattern: '^_',
          varsIgnorePattern: '^_'
        }],
        'semi': ['error', 'always'],
        'quotes': ['error', 'single'],
        'indent': ['error', 2, { 'SwitchCase': 1 }],
        'eqeqeq': ['error', 'always', { 'null': 'ignore' }],
        'no-console': ['warn', { allow: ['warn', 'error'] }],
        'curly': ['error', 'all']
      }
    },
    {
      files: ['__tests__/**/*.ts'],
      parser: '@typescript-eslint/parser',
      parserOptions: {
        project: ['./tsconfig.test.json'],
        tsconfigRootDir: __dirname,
        sourceType: 'module'
      },
      plugins: ['@typescript-eslint'],
      extends: [
        'eslint:recommended',
        'plugin:@typescript-eslint/recommended',
        'prettier'
      ],
      env: {
        'node': true,
        'jest': true
      },
      rules: {
        '@typescript-eslint/no-explicit-any': 'off',
        '@typescript-eslint/explicit-function-return-type': 'off',
        '@typescript-eslint/no-unused-vars': ['error', { 
          argsIgnorePattern: '^_',
          varsIgnorePattern: '^_'
        }],
        'semi': ['error', 'always'],
        'quotes': ['error', 'single'],
        'indent': ['error', 2, { 'SwitchCase': 1 }]
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
      extends: ['eslint:recommended', 'prettier'],
      rules: {
        'no-unused-vars': ['error', {
          argsIgnorePattern: '^_',
          varsIgnorePattern: '^_'
        }],
        'semi': ['error', 'always'],
        'quotes': ['error', 'single']
      },
      globals: {
        'Promise': 'readonly'
      }
    }
  ]
}; 