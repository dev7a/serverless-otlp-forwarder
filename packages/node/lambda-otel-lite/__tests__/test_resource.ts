import { describe, it, beforeEach, afterEach, expect } from '@jest/globals';
import { getLambdaResource } from '../src/internal/telemetry/resource';
import { EnvVarManager } from './utils';
import { ENV_VARS } from '../src/constants';

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
    envManager.setup({});
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
    // Set up Lambda specific environment variables for testing
    process.env[ENV_VARS.PROCESSOR_MODE] = 'async';
    process.env[ENV_VARS.QUEUE_SIZE] = '1024';
    process.env[ENV_VARS.COMPRESSION_LEVEL] = '4';

    const resource = getLambdaResource();

    // Verify that Lambda environment variables were added
    expect(resource.attributes['lambda_otel_lite.extension.span_processor_mode']).toBe('async');
    expect(resource.attributes['lambda_otel_lite.lambda_span_processor.queue_size']).toBe(1024);
    expect(
      resource.attributes['lambda_otel_lite.otlp_stdout_span_exporter.compression_level']
    ).toBe(4);
  });

  it('should not include telemetry configuration attributes when environment variables are not set', () => {
    const resource = getLambdaResource();

    // These attributes should not be present when environment variables are not set
    expect(resource.attributes['lambda_otel_lite.extension.span_processor_mode']).toBeUndefined();
    expect(
      resource.attributes['lambda_otel_lite.lambda_span_processor.queue_size']
    ).toBeUndefined();
    expect(
      resource.attributes['lambda_otel_lite.otlp_stdout_span_exporter.compression_level']
    ).toBeUndefined();
  });
});
