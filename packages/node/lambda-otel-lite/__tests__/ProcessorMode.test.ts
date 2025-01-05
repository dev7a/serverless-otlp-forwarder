import { ProcessorMode, processorModeFromEnv } from '../src/types';
import { EnvVarManager } from './utils';
import { describe, it, beforeEach, afterEach, expect } from '@jest/globals';

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

    describe('processorModeFromEnv', () => {
        it('should get mode from environment variable', () => {
            envManager.setup({ TEST_MODE: 'sync' });
            expect(processorModeFromEnv('TEST_MODE')).toBe(ProcessorMode.Sync);

            envManager.setup({ TEST_MODE: 'async' });
            expect(processorModeFromEnv('TEST_MODE')).toBe(ProcessorMode.Async);

            envManager.setup({ TEST_MODE: 'finalize' });
            expect(processorModeFromEnv('TEST_MODE')).toBe(ProcessorMode.Finalize);
        });

        it('should handle case-insensitive values', () => {
            envManager.setup({ TEST_MODE: 'SYNC' });
            expect(processorModeFromEnv('TEST_MODE')).toBe(ProcessorMode.Sync);

            envManager.setup({ TEST_MODE: 'Async' });
            expect(processorModeFromEnv('TEST_MODE')).toBe(ProcessorMode.Async);
        });

        it('should use default mode when env var is not set', () => {
            envManager.setup({ TEST_MODE: undefined });
            expect(processorModeFromEnv('TEST_MODE', ProcessorMode.Sync)).toBe(ProcessorMode.Sync);
        });

        it('should throw error for invalid mode', () => {
            envManager.setup({ TEST_MODE: 'invalid' });
            expect(() => processorModeFromEnv('TEST_MODE')).toThrow(
                'Invalid TEST_MODE: invalid. Must be one of: sync, async, finalize'
            );
        });

        it('should use Async mode by default when env var is not set and no default provided', () => {
            envManager.setup({ TEST_MODE: undefined });
            expect(processorModeFromEnv('TEST_MODE')).toBe(ProcessorMode.Async);
        });

        it('should handle empty string in environment variable', () => {
            envManager.setup({ TEST_MODE: '' });
            expect(processorModeFromEnv('TEST_MODE')).toBe(ProcessorMode.Async);
        });

        it('should handle whitespace in environment variable', () => {
            envManager.setup({ TEST_MODE: '  sync  ' });
            expect(processorModeFromEnv('TEST_MODE')).toBe(ProcessorMode.Sync);
        });

        it('should handle mixed case with whitespace', () => {
            envManager.setup({ TEST_MODE: '  aSyNc  ' });
            expect(processorModeFromEnv('TEST_MODE')).toBe(ProcessorMode.Async);
        });

        it('should use provided default mode over global default', () => {
            envManager.setup({ TEST_MODE: undefined });
            expect(processorModeFromEnv('TEST_MODE', ProcessorMode.Finalize)).toBe(ProcessorMode.Finalize);
        });

        it('should handle non-string environment values', () => {
            // @ts-ignore - Testing runtime behavior with invalid types
            process.env.TEST_MODE = 123;
            expect(processorModeFromEnv('TEST_MODE')).toBe(ProcessorMode.Async);

            // @ts-ignore - Testing runtime behavior with invalid types
            process.env.TEST_MODE = true;
            expect(processorModeFromEnv('TEST_MODE')).toBe(ProcessorMode.Async);
        });
    });
}); 