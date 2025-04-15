/// <reference types="jest" />

import { jest, describe, it, beforeEach, afterEach, expect } from '@jest/globals';
import { SpyInstance } from 'jest-mock';
import { OTLPStdoutSpanExporter, LogLevel, OutputType } from '../otlp_stdout_span_exporter';
import { ReadableSpan } from '@opentelemetry/sdk-trace-base';
import { ExportResultCode } from '@opentelemetry/core';
import * as zlib from 'zlib';
import * as fs from 'fs';

jest.mock('zlib', () => ({
  gzipSync: jest.fn(() => Buffer.from('mock-compressed-data')),
  constants: {
    Z_BEST_COMPRESSION: 9
  }
}));

// Mock dependencies
jest.mock('@opentelemetry/otlp-transformer', () => ({
  ProtobufTraceSerializer: {
    serializeRequest: jest.fn(() => Buffer.from('mock-serialized-data'))
  }
}));

// Mock fs module
jest.mock('fs', () => ({
  existsSync: jest.fn().mockReturnValue(true),
  writeFile: jest.fn((path, data, callback) => {
    if (typeof callback === 'function') {
      callback();
    }
  })
}));

describe('OTLPStdoutSpanExporter', () => {
  let mockWrite: SpyInstance<any>;
  let originalEnv: NodeJS.ProcessEnv;

  beforeEach(() => {
    originalEnv = { ...process.env };
    mockWrite = jest.spyOn(process.stdout, 'write').mockImplementation(
      (str: string | Uint8Array, 
       encoding?: BufferEncoding | ((err?: Error | undefined) => void), 
       cb?: ((err?: Error | undefined) => void) | undefined): boolean => {
      if (typeof cb === 'function') {
        cb();
      } else if (typeof encoding === 'function') {
        encoding();
      }
      return true;
    });

    // Clear relevant environment variables
    delete process.env.OTEL_SERVICE_NAME;
    delete process.env.AWS_LAMBDA_FUNCTION_NAME;
    delete process.env.OTEL_EXPORTER_OTLP_HEADERS;
    delete process.env.OTEL_EXPORTER_OTLP_TRACES_HEADERS;
    delete process.env.OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL;
    delete process.env.OTLP_STDOUT_SPAN_EXPORTER_LOG_LEVEL;
    delete process.env.OTLP_STDOUT_SPAN_EXPORTER_OUTPUT_TYPE;
  });

  afterEach(() => {
    mockWrite.mockRestore();
    process.env = originalEnv;
    jest.clearAllMocks();
  });

  it('should use default values when no config provided', () => {
    const _exporter = new OTLPStdoutSpanExporter();
    expect(mockWrite).not.toHaveBeenCalled();
  });

  it('should use environment variables for service name', () => {
    process.env.OTEL_SERVICE_NAME = 'test-service';
    const _exporter = new OTLPStdoutSpanExporter();
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, (_result) => {
      const output = JSON.parse(mockWrite.mock.calls[0][0] as string);
      expect(output.source).toBe('test-service');
    });
  });

  it('should fallback to AWS_LAMBDA_FUNCTION_NAME', () => {
    process.env.AWS_LAMBDA_FUNCTION_NAME = 'lambda-function';
    const _exporter = new OTLPStdoutSpanExporter();
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, (_result) => {
      const output = JSON.parse(mockWrite.mock.calls[0][0] as string);
      expect(output.source).toBe('lambda-function');
    });
  });

  it('should use custom gzip level from config', () => {
    const _exporter = new OTLPStdoutSpanExporter({ gzipLevel: zlib.constants.Z_BEST_COMPRESSION });
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, (_result) => {
      expect(zlib.gzipSync).toHaveBeenCalledWith(
        expect.any(Buffer),
        expect.objectContaining({ level: zlib.constants.Z_BEST_COMPRESSION })
      );
    });
  });
  
  it('should use compression level from environment variable', () => {
    process.env.OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL = '3';
    const _exporter = new OTLPStdoutSpanExporter();
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, (_result) => {
      expect(zlib.gzipSync).toHaveBeenCalledWith(
        expect.any(Buffer),
        expect.objectContaining({ level: 3 })
      );
    });
  });

  it('should use environment variable over explicit config for compression level', () => {
    process.env.OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL = '3';
    const _exporter = new OTLPStdoutSpanExporter({ gzipLevel: 8 });
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, (_result) => {
      expect(zlib.gzipSync).toHaveBeenCalledWith(
        expect.any(Buffer),
        expect.objectContaining({ level: 3 })
      );
    });
  });

  it('should handle invalid compression level in environment variable', () => {
    process.env.OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL = 'invalid';
    const _exporter = new OTLPStdoutSpanExporter();
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, (_result) => {
      expect(zlib.gzipSync).toHaveBeenCalledWith(
        expect.any(Buffer),
        expect.objectContaining({ level: 6 }) // Default level
      );
    });
  });

  it('should handle out-of-range compression level in environment variable', () => {
    process.env.OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL = '10'; // Out of range
    const _exporter = new OTLPStdoutSpanExporter();
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, (_result) => {
      expect(zlib.gzipSync).toHaveBeenCalledWith(
        expect.any(Buffer),
        expect.objectContaining({ level: 6 }) // Default level
      );
    });
  });

  it('should handle stdout write errors', () => {
    mockWrite.mockImplementationOnce(
      (str: string | Uint8Array, 
       encoding?: BufferEncoding | ((err?: Error | undefined) => void), 
       cb?: ((err?: Error | undefined) => void) | undefined): boolean => {
      if (typeof cb === 'function') {
        cb(new Error('Write failed'));
      } else if (typeof encoding === 'function') {
        encoding(new Error('Write failed'));
      }
      return true;
    });

    const _exporter = new OTLPStdoutSpanExporter();
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, (_result) => {
      expect(_result.code).toBe(ExportResultCode.FAILED);
    });
  });

  it('should return success on successful export', () => {
    const _exporter = new OTLPStdoutSpanExporter();
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, (_result) => {
      expect(_result.code).toBe(ExportResultCode.SUCCESS);
    });
  });

  it('should include all required fields in output', () => {
    const _exporter = new OTLPStdoutSpanExporter();
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, (_result) => {
      const output = JSON.parse(mockWrite.mock.calls[0][0] as string);
      expect(output).toMatchObject({
        __otel_otlp_stdout: expect.any(String),
        source: expect.any(String),
        endpoint: expect.any(String),
        method: 'POST',
        'content-type': 'application/x-protobuf',
        'content-encoding': 'gzip',
        payload: expect.any(String),
        base64: true
      });
      // Headers should not be present when no custom headers are defined
      expect(output.headers).toBeUndefined();
    });
  });

  // Tests for log level support
  it('should not include level field when no log level is set', () => {
    const _exporter = new OTLPStdoutSpanExporter();
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, (_result) => {
      const output = JSON.parse(mockWrite.mock.calls[0][0] as string);
      expect(output.level).toBeUndefined();
    });
  });

  it('should include log level from config', () => {
    const _exporter = new OTLPStdoutSpanExporter({ logLevel: LogLevel.Debug });
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, (_result) => {
      const output = JSON.parse(mockWrite.mock.calls[0][0] as string);
      expect(output.level).toBe(LogLevel.Debug);
    });
  });

  it('should use log level from environment variable', () => {
    process.env.OTLP_STDOUT_SPAN_EXPORTER_LOG_LEVEL = 'warn';
    const _exporter = new OTLPStdoutSpanExporter();
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, (_result) => {
      const output = JSON.parse(mockWrite.mock.calls[0][0] as string);
      expect(output.level).toBe(LogLevel.Warn);
    });
  });

  it('should use environment variable over explicit config for log level', () => {
    process.env.OTLP_STDOUT_SPAN_EXPORTER_LOG_LEVEL = 'error';
    const _exporter = new OTLPStdoutSpanExporter({ logLevel: LogLevel.Info });
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, (_result) => {
      const output = JSON.parse(mockWrite.mock.calls[0][0] as string);
      expect(output.level).toBe(LogLevel.Error);
    });
  });

  it('should handle invalid log level in environment variable', () => {
    process.env.OTLP_STDOUT_SPAN_EXPORTER_LOG_LEVEL = 'invalid';
    const _exporter = new OTLPStdoutSpanExporter({ logLevel: LogLevel.Info });
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, (_result) => {
      const output = JSON.parse(mockWrite.mock.calls[0][0] as string);
      expect(output.level).toBe(LogLevel.Info);
    });
  });

  // Tests for named pipe output
  it('should use stdout by default', () => {
    const _exporter = new OTLPStdoutSpanExporter();
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, (_result) => {
      expect(mockWrite).toHaveBeenCalled();
      expect(fs.writeFile).not.toHaveBeenCalled();
    });
  });

  it('should use named pipe when configured', () => {
    const _exporter = new OTLPStdoutSpanExporter({ outputType: OutputType.Pipe });
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, (_result) => {
      expect(fs.writeFile).toHaveBeenCalled();
      expect(mockWrite).not.toHaveBeenCalled();
    });
  });

  it('should use output type from environment variable', () => {
    process.env.OTLP_STDOUT_SPAN_EXPORTER_OUTPUT_TYPE = 'pipe';
    const _exporter = new OTLPStdoutSpanExporter();
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, (_result) => {
      expect(fs.writeFile).toHaveBeenCalled();
      expect(mockWrite).not.toHaveBeenCalled();
    });
  });

  it('should use environment variable over explicit config for output type', () => {
    process.env.OTLP_STDOUT_SPAN_EXPORTER_OUTPUT_TYPE = 'pipe';
    const _exporter = new OTLPStdoutSpanExporter({ outputType: OutputType.Stdout });
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, (_result) => {
      expect(fs.writeFile).toHaveBeenCalled();
      expect(mockWrite).not.toHaveBeenCalled();
    });
  });

  it('should fallback to stdout if pipe does not exist', () => {
    jest.spyOn(fs, 'existsSync').mockReturnValueOnce(false);
    const _exporter = new OTLPStdoutSpanExporter({ outputType: OutputType.Pipe });
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, (_result) => {
      expect(mockWrite).toHaveBeenCalled();
    });
  });

  it('should fallback to stdout if pipe write fails', () => {
    jest.spyOn(fs, 'writeFile').mockImplementationOnce((path, data, callback) => {
      if (typeof callback === 'function') {
        callback(new Error('Write to pipe failed'));
      }
    });
    
    const _exporter = new OTLPStdoutSpanExporter({ outputType: OutputType.Pipe });
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, (_result) => {
      expect(mockWrite).toHaveBeenCalled();
    });
  });
});

describe('OTLPStdoutSpanExporter Header Parsing', () => {
  let mockWrite: SpyInstance<any>;
  let originalEnv: NodeJS.ProcessEnv;

  beforeEach(() => {
    originalEnv = { ...process.env };
    mockWrite = jest.spyOn(process.stdout, 'write').mockImplementation(
      (str: string | Uint8Array, 
       encoding?: BufferEncoding | ((err?: Error | undefined) => void), 
       cb?: ((err?: Error | undefined) => void) | undefined): boolean => {
      if (typeof cb === 'function') {
        cb();
      } else if (typeof encoding === 'function') {
        encoding();
      }
      return true;
    });

    // Clear relevant environment variables
    delete process.env.OTEL_EXPORTER_OTLP_HEADERS;
    delete process.env.OTEL_EXPORTER_OTLP_TRACES_HEADERS;
  });

  afterEach(() => {
    mockWrite.mockRestore();
    process.env = originalEnv;
    jest.clearAllMocks();
  });

  it('should not include headers section when no headers defined', () => {
    const _exporter = new OTLPStdoutSpanExporter();
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, () => {});
    const output = JSON.parse(mockWrite.mock.calls[0][0] as string);
    expect(output.headers).toBeUndefined();
  });

  it('should parse headers from OTEL_EXPORTER_OTLP_HEADERS', () => {
    process.env.OTEL_EXPORTER_OTLP_HEADERS = 'api-key=secret123,custom-header=value';
    const _exporter = new OTLPStdoutSpanExporter();
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, () => {});
    const output = JSON.parse(mockWrite.mock.calls[0][0] as string);
    expect(output.headers).toEqual({
      'api-key': 'secret123',
      'custom-header': 'value'
    });
  });

  it('should parse headers from OTEL_EXPORTER_OTLP_TRACES_HEADERS', () => {
    process.env.OTEL_EXPORTER_OTLP_TRACES_HEADERS = 'trace-key=value123,other-header=test';
    const _exporter = new OTLPStdoutSpanExporter();
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, () => {});
    const output = JSON.parse(mockWrite.mock.calls[0][0] as string);
    expect(output.headers).toEqual({
      'trace-key': 'value123',
      'other-header': 'test'
    });
  });

  it('should merge headers with OTLP_TRACES_HEADERS taking precedence', () => {
    process.env.OTEL_EXPORTER_OTLP_HEADERS = 'api-key=secret123,shared-key=general';
    process.env.OTEL_EXPORTER_OTLP_TRACES_HEADERS = 'shared-key=specific,trace-key=value123';
    const _exporter = new OTLPStdoutSpanExporter();
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, () => {});
    const output = JSON.parse(mockWrite.mock.calls[0][0] as string);
    expect(output.headers).toEqual({
      'api-key': 'secret123',
      'shared-key': 'specific',  // TRACES_HEADERS value takes precedence
      'trace-key': 'value123'
    });
  });

  it('should handle headers with whitespace', () => {
    process.env.OTEL_EXPORTER_OTLP_HEADERS = ' api-key = secret123 , custom-header = value ';
    const _exporter = new OTLPStdoutSpanExporter();
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, () => {});
    const output = JSON.parse(mockWrite.mock.calls[0][0] as string);
    expect(output.headers).toEqual({
      'api-key': 'secret123',
      'custom-header': 'value'
    });
  });

  it('should filter out content-type and content-encoding headers', () => {
    process.env.OTEL_EXPORTER_OTLP_HEADERS = 'content-type=text/plain,content-encoding=none,api-key=secret123';
    const _exporter = new OTLPStdoutSpanExporter();
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, () => {});
    const output = JSON.parse(mockWrite.mock.calls[0][0] as string);
    expect(output.headers).toEqual({
      'api-key': 'secret123'
    });
  });

  it('should handle multiple equal signs in header value', () => {
    process.env.OTEL_EXPORTER_OTLP_HEADERS = 'authorization=Basic dXNlcjpwYXNzd29yZA==,api-key=secret=123=456';
    const _exporter = new OTLPStdoutSpanExporter();
    const spans: ReadableSpan[] = [];
    
    _exporter.export(spans, () => {});
    const output = JSON.parse(mockWrite.mock.calls[0][0] as string);
    expect(output.headers).toEqual({
      'authorization': 'Basic dXNlcjpwYXNzd29yZA==',
      'api-key': 'secret=123=456'
    });
  });
});