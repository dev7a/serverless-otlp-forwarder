import { ProcessorMode, resolveProcessorMode } from '../src/mode';
import { EnvVarManager } from './utils';
import { describe, it, beforeEach, afterEach, expect } from '@jest/globals';
import { ENV_VARS } from '../src/constants';

describe('ProcessorMode', () => {
  let envManager: EnvVarManager;

  beforeEach(() => {
    envManager = new EnvVarManager();
    envManager.setup();
  });

  afterEach(() => {
    envManager.restore();
  });

  it('should have correct values', () => {
    expect(ProcessorMode.Sync).toBe('sync');
    expect(ProcessorMode.Async).toBe('async');
    expect(ProcessorMode.Finalize).toBe('finalize');
  });

  describe('resolveProcessorMode', () => {
    it('should get mode from environment variable', () => {
      envManager.setup({ [ENV_VARS.PROCESSOR_MODE]: 'sync' });
      expect(resolveProcessorMode()).toBe(ProcessorMode.Sync);

      envManager.setup({ [ENV_VARS.PROCESSOR_MODE]: 'async' });
      expect(resolveProcessorMode()).toBe(ProcessorMode.Async);

      envManager.setup({ [ENV_VARS.PROCESSOR_MODE]: 'finalize' });
      expect(resolveProcessorMode()).toBe(ProcessorMode.Finalize);
    });

    it('should handle case-insensitive values', () => {
      envManager.setup({ [ENV_VARS.PROCESSOR_MODE]: 'SYNC' });
      expect(resolveProcessorMode()).toBe(ProcessorMode.Sync);

      envManager.setup({ [ENV_VARS.PROCESSOR_MODE]: 'Async' });
      expect(resolveProcessorMode()).toBe(ProcessorMode.Async);
    });

    it('should use config value when env var is not set', () => {
      envManager.setup({ [ENV_VARS.PROCESSOR_MODE]: undefined });
      expect(resolveProcessorMode(ProcessorMode.Async)).toBe(ProcessorMode.Async);
      expect(resolveProcessorMode(ProcessorMode.Finalize)).toBe(ProcessorMode.Finalize);
    });

    it('should use default value when neither env var nor config is set', () => {
      envManager.setup({ [ENV_VARS.PROCESSOR_MODE]: undefined });
      expect(resolveProcessorMode()).toBe(ProcessorMode.Sync);
    });

    it('should handle invalid environment variable values', () => {
      envManager.setup({ [ENV_VARS.PROCESSOR_MODE]: 'invalid' });
      // Should log a warning and use the config value
      expect(resolveProcessorMode(ProcessorMode.Async)).toBe(ProcessorMode.Async);
      expect(resolveProcessorMode()).toBe(ProcessorMode.Sync);
    });

    it('should handle empty string in environment variable', () => {
      envManager.setup({ [ENV_VARS.PROCESSOR_MODE]: '' });
      expect(resolveProcessorMode(ProcessorMode.Async)).toBe(ProcessorMode.Async);
      expect(resolveProcessorMode()).toBe(ProcessorMode.Sync);
    });

    it('should handle whitespace in environment variable', () => {
      envManager.setup({ [ENV_VARS.PROCESSOR_MODE]: '  async  ' });
      expect(resolveProcessorMode(ProcessorMode.Sync)).toBe(ProcessorMode.Async);
    });

    it('should handle mixed case with whitespace', () => {
      envManager.setup({ [ENV_VARS.PROCESSOR_MODE]: '  aSyNc  ' });
      expect(resolveProcessorMode(ProcessorMode.Sync)).toBe(ProcessorMode.Async);
    });

    it('should prioritize environment variable over config value', () => {
      envManager.setup({ [ENV_VARS.PROCESSOR_MODE]: 'async' });
      expect(resolveProcessorMode(ProcessorMode.Sync)).toBe(ProcessorMode.Async);
      expect(resolveProcessorMode(ProcessorMode.Finalize)).toBe(ProcessorMode.Async);
    });

    it('should handle non-string environment values', () => {
      // @ts-expect-error - Testing runtime behavior with invalid types
      process.env[ENV_VARS.PROCESSOR_MODE] = 123;
      expect(resolveProcessorMode()).toBe(ProcessorMode.Sync);

      // @ts-expect-error - Testing runtime behavior with invalid types
      process.env[ENV_VARS.PROCESSOR_MODE] = true;
      expect(resolveProcessorMode()).toBe(ProcessorMode.Sync);
    });

    it('should allow custom environment variable name', () => {
      envManager.setup({ CUSTOM_ENV_VAR: 'async' });
      expect(resolveProcessorMode(ProcessorMode.Sync, 'CUSTOM_ENV_VAR')).toBe(ProcessorMode.Async);
    });
  });
});
