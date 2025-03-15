import { describe, it, beforeEach, afterEach, expect } from '@jest/globals';
import { EnvVarManager } from './utils';
import { getLambdaResource } from '../src/internal/telemetry/init';

describe('getLambdaResource', () => {
  const envManager = new EnvVarManager();

  beforeEach(() => {
    // Reset environment before each test
    envManager.restore();
  });

  afterEach(() => {
    envManager.restore();
  });

  it('should set cloud.provider to aws by default', () => {
    const resource = getLambdaResource();
    expect(resource.attributes['cloud.provider']).toBe('aws');
  });

  it('should include all Lambda environment variables when present', () => {
    const lambdaEnv = {
      AWS_REGION: 'us-west-2',
      AWS_LAMBDA_FUNCTION_NAME: 'test-function',
      AWS_LAMBDA_FUNCTION_VERSION: '$LATEST',
      AWS_LAMBDA_LOG_STREAM_NAME: 'test-stream',
      AWS_LAMBDA_FUNCTION_MEMORY_SIZE: '128',
    };
    envManager.setup(lambdaEnv);

    const resource = getLambdaResource();

    expect(resource.attributes['cloud.region']).toBe('us-west-2');
    expect(resource.attributes['faas.name']).toBe('test-function');
    expect(resource.attributes['faas.version']).toBe('$LATEST');
    expect(resource.attributes['faas.instance']).toBe('test-stream');
    expect(resource.attributes['faas.max_memory']).toBe(134217728); // 128MB in bytes
  });

  it('should only include Lambda environment variables that are present', () => {
    const lambdaEnv = {
      AWS_REGION: 'us-west-2',
      AWS_LAMBDA_FUNCTION_NAME: 'test-function',
    };
    envManager.setup(lambdaEnv);

    const resource = getLambdaResource();

    expect(resource.attributes['cloud.region']).toBe('us-west-2');
    expect(resource.attributes['faas.name']).toBe('test-function');
    expect(resource.attributes['faas.version']).toBeUndefined();
    expect(resource.attributes['faas.instance']).toBeUndefined();
    expect(resource.attributes['faas.max_memory']).toBeUndefined();
  });

  it('should set service.name from OTEL_SERVICE_NAME if present', () => {
    envManager.setup({
      OTEL_SERVICE_NAME: 'otel-service',
      AWS_LAMBDA_FUNCTION_NAME: 'lambda-function',
    });

    const resource = getLambdaResource();
    expect(resource.attributes['service.name']).toBe('otel-service');
  });

  it('should fallback to AWS_LAMBDA_FUNCTION_NAME for service.name if OTEL_SERVICE_NAME is not set', () => {
    envManager.setup({
      AWS_LAMBDA_FUNCTION_NAME: 'lambda-function',
    });

    const resource = getLambdaResource();
    expect(resource.attributes['service.name']).toBe('lambda-function');
  });

  it('should use unknown_service if neither OTEL_SERVICE_NAME nor AWS_LAMBDA_FUNCTION_NAME are set', () => {
    const resource = getLambdaResource();
    expect(resource.attributes['service.name']).toBe('unknown_service');
  });

  it('should parse OTEL_RESOURCE_ATTRIBUTES correctly', () => {
    envManager.setup({
      OTEL_RESOURCE_ATTRIBUTES: 'key1=value1,key2=value2,key3=value%20with%20spaces',
    });

    const resource = getLambdaResource();
    expect(resource.attributes['key1']).toBe('value1');
    expect(resource.attributes['key2']).toBe('value2');
    expect(resource.attributes['key3']).toBe('value with spaces');
  });

  it('should handle malformed OTEL_RESOURCE_ATTRIBUTES gracefully', () => {
    envManager.setup({
      OTEL_RESOURCE_ATTRIBUTES: 'key1=value1,malformed,key2=value2,=empty,novalue=',
    });

    const resource = getLambdaResource();
    expect(resource.attributes['key1']).toBe('value1');
    expect(resource.attributes['key2']).toBe('value2');
    // Malformed entries should be skipped without throwing
    expect(resource.attributes['malformed']).toBeUndefined();
    expect(resource.attributes['empty']).toBeUndefined();
    expect(resource.attributes['novalue']).toBeUndefined();
  });

  it('should include Lambda environment variables when present', () => {
    const lambdaEnv = {
      AWS_REGION: 'us-west-2',
      AWS_LAMBDA_FUNCTION_NAME: 'test-function',
      AWS_LAMBDA_FUNCTION_VERSION: '$LATEST',
      AWS_LAMBDA_LOG_STREAM_NAME: 'test-stream',
      AWS_LAMBDA_FUNCTION_MEMORY_SIZE: '128',
      LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE: 'async',
      LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE: '1024',
      LAMBDA_SPAN_PROCESSOR_BATCH_SIZE: '256',
      OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL: '4',
    };
    envManager.setup(lambdaEnv);

    const resource = getLambdaResource();

    // Check standard Lambda attributes
    expect(resource.attributes['cloud.region']).toBe('us-west-2');
    expect(resource.attributes['faas.name']).toBe('test-function');
    expect(resource.attributes['faas.version']).toBe('$LATEST');
    expect(resource.attributes['faas.instance']).toBe('test-stream');
    expect(resource.attributes['faas.max_memory']).toBe(134217728); // 128MB in bytes

    // Check telemetry configuration attributes
    expect(resource.attributes['lambda_otel_lite.extension.span_processor_mode']).toBe('async');
    expect(resource.attributes['lambda_otel_lite.lambda_span_processor.queue_size']).toBe(1024);
    expect(resource.attributes['lambda_otel_lite.lambda_span_processor.batch_size']).toBe(256);
    expect(
      resource.attributes['lambda_otel_lite.otlp_stdout_span_exporter.compression_level']
    ).toBe(4);
  });

  it('should use default values for telemetry configuration when not set', () => {
    const resource = getLambdaResource();

    expect(resource.attributes['lambda_otel_lite.extension.span_processor_mode']).toBe('sync');
    expect(resource.attributes['lambda_otel_lite.lambda_span_processor.queue_size']).toBe(2048);
    expect(resource.attributes['lambda_otel_lite.lambda_span_processor.batch_size']).toBe(512);
    expect(
      resource.attributes['lambda_otel_lite.otlp_stdout_span_exporter.compression_level']
    ).toBe(6);
  });
});
