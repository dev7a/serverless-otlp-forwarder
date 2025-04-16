import { getBooleanValue, getNumericValue, getStringValue } from '../src/internal/config';
import { EnvVarManager } from './utils';
import { describe, it, beforeEach, afterEach, expect } from '@jest/globals';

describe('Config Helpers', () => {
  let envManager: EnvVarManager;

  beforeEach(() => {
    envManager = new EnvVarManager();
    envManager.setup();
  });

  afterEach(() => {
    envManager.restore();
  });

  describe('getBooleanValue', () => {
    it('should get boolean from environment variable', () => {
      envManager.setup({ TEST_BOOL: 'true' });
      expect(getBooleanValue('TEST_BOOL')).toBe(true);

      envManager.setup({ TEST_BOOL: 'false' });
      expect(getBooleanValue('TEST_BOOL')).toBe(false);
    });

    it('should handle case-insensitive values', () => {
      envManager.setup({ TEST_BOOL: 'TRUE' });
      expect(getBooleanValue('TEST_BOOL')).toBe(true);

      envManager.setup({ TEST_BOOL: 'False' });
      expect(getBooleanValue('TEST_BOOL')).toBe(false);
    });

    it('should use config value when env var is not set', () => {
      envManager.setup({ TEST_BOOL: undefined });
      expect(getBooleanValue('TEST_BOOL', true)).toBe(true);
      expect(getBooleanValue('TEST_BOOL', false)).toBe(false);
    });

    it('should use default value when neither env var nor config is set', () => {
      envManager.setup({ TEST_BOOL: undefined });
      expect(getBooleanValue('TEST_BOOL')).toBe(false); // Default is false
      expect(getBooleanValue('TEST_BOOL', undefined, true)).toBe(true);
    });

    it('should handle invalid environment variable values', () => {
      envManager.setup({ TEST_BOOL: 'invalid' });
      // Should log a warning and use the config value
      expect(getBooleanValue('TEST_BOOL', true)).toBe(true);
      expect(getBooleanValue('TEST_BOOL')).toBe(false);
    });

    it('should handle empty string in environment variable', () => {
      envManager.setup({ TEST_BOOL: '' });
      expect(getBooleanValue('TEST_BOOL', true)).toBe(true);
      expect(getBooleanValue('TEST_BOOL')).toBe(false);
    });

    it('should handle whitespace in environment variable', () => {
      envManager.setup({ TEST_BOOL: '  true  ' });
      expect(getBooleanValue('TEST_BOOL')).toBe(true);
    });
  });

  describe('getNumericValue', () => {
    it('should get number from environment variable', () => {
      envManager.setup({ TEST_NUM: '123' });
      expect(getNumericValue('TEST_NUM')).toBe(123);

      envManager.setup({ TEST_NUM: '0' });
      expect(getNumericValue('TEST_NUM')).toBe(0);

      envManager.setup({ TEST_NUM: '-456' });
      expect(getNumericValue('TEST_NUM')).toBe(-456);
    });

    it('should use config value when env var is not set', () => {
      envManager.setup({ TEST_NUM: undefined });
      expect(getNumericValue('TEST_NUM', 42)).toBe(42);
    });

    it('should use default value when neither env var nor config is set', () => {
      envManager.setup({ TEST_NUM: undefined });
      expect(getNumericValue('TEST_NUM')).toBe(0); // Default is 0
      expect(getNumericValue('TEST_NUM', undefined, 42)).toBe(42);
    });

    it('should handle invalid environment variable values', () => {
      envManager.setup({ TEST_NUM: 'invalid' });
      // Should log a warning and use the config value
      expect(getNumericValue('TEST_NUM', 42)).toBe(42);
      expect(getNumericValue('TEST_NUM')).toBe(0);
    });

    it('should handle empty string in environment variable', () => {
      envManager.setup({ TEST_NUM: '' });
      expect(getNumericValue('TEST_NUM', 42)).toBe(42);
      expect(getNumericValue('TEST_NUM')).toBe(0);
    });

    it('should handle whitespace in environment variable', () => {
      envManager.setup({ TEST_NUM: '  123  ' });
      expect(getNumericValue('TEST_NUM')).toBe(123);
    });

    it('should apply validator function if provided', () => {
      envManager.setup({ TEST_NUM: '123' });
      expect(getNumericValue('TEST_NUM', 42, 0, (value) => value > 100)).toBe(123);
      expect(getNumericValue('TEST_NUM', 42, 0, (value) => value > 200)).toBe(42);
    });
  });

  describe('getStringValue', () => {
    it('should get string from environment variable', () => {
      envManager.setup({ TEST_STR: 'hello' });
      expect(getStringValue('TEST_STR')).toBe('hello');
    });

    it('should use config value when env var is not set', () => {
      envManager.setup({ TEST_STR: undefined });
      expect(getStringValue('TEST_STR', 'world')).toBe('world');
    });

    it('should use default value when neither env var nor config is set', () => {
      envManager.setup({ TEST_STR: undefined });
      expect(getStringValue('TEST_STR')).toBe(''); // Default is empty string
      expect(getStringValue('TEST_STR', undefined, 'default')).toBe('default');
    });

    it('should handle empty string in environment variable', () => {
      envManager.setup({ TEST_STR: '' });
      expect(getStringValue('TEST_STR', 'world')).toBe('world');
    });

    it('should handle whitespace in environment variable', () => {
      envManager.setup({ TEST_STR: '  hello  ' });
      expect(getStringValue('TEST_STR')).toBe('hello');
    });

    it('should apply validator function if provided', () => {
      envManager.setup({ TEST_STR: 'hello' });
      expect(getStringValue('TEST_STR', 'world', '', (value) => value.length > 3)).toBe('hello');
      expect(getStringValue('TEST_STR', 'world', '', (value) => value.length > 5)).toBe('world');
    });
  });
});
