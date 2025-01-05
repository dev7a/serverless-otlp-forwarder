import { SpanKind, SpanStatusCode, trace, Context, ROOT_CONTEXT, propagation } from '@opentelemetry/api';
import { NodeTracerProvider } from '@opentelemetry/sdk-trace-node';
import { ProcessorMode } from '../src/types';
import { tracedHandler } from '../src/handler';
import { state, handlerComplete } from '../src/state';
import * as init from '../src/telemetry/init';
import { jest, describe, it, beforeEach, expect } from '@jest/globals';

// Mock the logger
jest.mock('../src/extension/logger', () => ({
    debug: jest.fn(),
    info: jest.fn(),
    warn: jest.fn(),
    error: jest.fn()
}));

// Mock the state module
jest.mock('../src/state', () => {
    const Signal = class {
        private listeners: Array<() => void> = [];
        signal(): void {
            this.listeners.forEach(l => l());
        }
        on(listener: () => void): void {
            this.listeners.push(listener);
        }
    };

    const handlerComplete = new Signal();
    return {
        state: {
            mode: 'sync',
            extensionInitialized: false,
            handlerCompleted: false,
            handlerComplete
        },
        handlerComplete
    };
});

// Mock cold start functions
jest.mock('../src/telemetry/init', () => ({
    isColdStart: jest.fn(),
    setColdStart: jest.fn()
}));

describe('tracedHandler', () => {
    let provider: NodeTracerProvider;
    let tracer: any;
    let mockSpan: any;

    beforeEach(() => {
        // Reset all mocks
        jest.clearAllMocks();
        
        // Create mock span
        mockSpan = {
            setAttribute: jest.fn(),
            setStatus: jest.fn(),
            end: jest.fn(),
            recordException: jest.fn(),
            addEvent: jest.fn()
        };

        // Create mock tracer
        tracer = {
            startActiveSpan: jest.fn((name: string, options: any, context: Context, fn: (span: any) => Promise<any>) => {
                return fn(mockSpan);
            })
        };

        // Create mock provider
        provider = {
            forceFlush: jest.fn(),
            shutdown: jest.fn()
        } as any;

        // Mock cold start as true initially
        (init.isColdStart as jest.Mock).mockReturnValue(true);
    });

    describe('basic functionality', () => {
        it('should work with basic options', async () => {
            const result = await tracedHandler({
                tracer,
                provider,
                name: 'test-handler',
                fn: async (span) => 'success'
            });

            expect(result).toBe('success');
            expect(mockSpan.setAttribute).toHaveBeenCalledWith('faas.cold_start', true);
            expect(mockSpan.setStatus).toHaveBeenCalledWith({ code: SpanStatusCode.OK });
            expect(mockSpan.end).toHaveBeenCalled();
        });

        it('should work with all options', async () => {
            const result = await tracedHandler({
                tracer,
                provider,
                name: 'test-handler',
                kind: SpanKind.SERVER,
                attributes: { custom: 'value' },
                fn: async (span) => 'success'
            });

            expect(result).toBe('success');
            expect(mockSpan.setAttribute).toHaveBeenCalledWith('custom', 'value');
            expect(mockSpan.setStatus).toHaveBeenCalledWith({ code: SpanStatusCode.OK });
        });
    });

    describe('Lambda context handling', () => {
        it('should extract attributes from Lambda context', async () => {
            const lambdaContext = {
                awsRequestId: '123',
                invokedFunctionArn: 'arn:aws:lambda:region:account:function:name'
            };

            await tracedHandler({
                tracer,
                provider,
                name: 'test-handler',
                context: lambdaContext,
                fn: async () => 'success'
            });

            expect(mockSpan.setAttribute).toHaveBeenCalledWith('faas.invocation_id', '123');
            expect(mockSpan.setAttribute).toHaveBeenCalledWith('cloud.account.id', 'account');
            expect(mockSpan.setAttribute).toHaveBeenCalledWith(
                'cloud.resource_id',
                'arn:aws:lambda:region:account:function:name'
            );
        });
    });

    describe('API Gateway event handling', () => {
        it('should handle API Gateway v2 events', async () => {
            const event = {
                version: '2.0',
                routeKey: '/test',
                requestContext: {
                    http: {
                        method: 'GET',
                        path: '/test',
                        protocol: 'HTTP/1.1'
                    }
                }
            };

            await tracedHandler({
                tracer,
                provider,
                name: 'test-handler',
                event,
                fn: async () => 'success'
            });

            expect(mockSpan.setAttribute).toHaveBeenCalledWith('faas.trigger', 'http');
            expect(mockSpan.setAttribute).toHaveBeenCalledWith('http.route', '/test');
            expect(mockSpan.setAttribute).toHaveBeenCalledWith('http.method', 'GET');
            expect(mockSpan.setAttribute).toHaveBeenCalledWith('http.target', '/test');
            expect(mockSpan.setAttribute).toHaveBeenCalledWith('http.scheme', 'http/1.1');
        });

        it('should handle API Gateway v1 events', async () => {
            const event = {
                httpMethod: 'POST',
                resource: '/test',
                path: '/test',
                requestContext: {
                    protocol: 'HTTPS'
                }
            };

            await tracedHandler({
                tracer,
                provider,
                name: 'test-handler',
                event,
                fn: async () => 'success'
            });

            expect(mockSpan.setAttribute).toHaveBeenCalledWith('faas.trigger', 'http');
            expect(mockSpan.setAttribute).toHaveBeenCalledWith('http.route', '/test');
            expect(mockSpan.setAttribute).toHaveBeenCalledWith('http.method', 'POST');
            expect(mockSpan.setAttribute).toHaveBeenCalledWith('http.target', '/test');
            expect(mockSpan.setAttribute).toHaveBeenCalledWith('http.scheme', 'https');
        });
    });

    describe('HTTP response handling', () => {
        it('should handle successful HTTP responses', async () => {
            await tracedHandler({
                tracer,
                provider,
                name: 'test-handler',
                fn: async () => ({
                    statusCode: 200,
                    body: 'success'
                })
            });

            expect(mockSpan.setAttribute).toHaveBeenCalledWith('http.status_code', 200);
            expect(mockSpan.setStatus).toHaveBeenCalledWith({ code: SpanStatusCode.OK });
        });

        it('should handle error HTTP responses', async () => {
            await tracedHandler({
                tracer,
                provider,
                name: 'test-handler',
                fn: async () => ({
                    statusCode: 500,
                    body: 'error'
                })
            });

            expect(mockSpan.setAttribute).toHaveBeenCalledWith('http.status_code', 500);
            expect(mockSpan.setStatus).toHaveBeenCalledWith({
                code: SpanStatusCode.ERROR,
                message: 'HTTP 500 response'
            });
        });
    });

    describe('context propagation', () => {
        it('should extract context from headers', async () => {
            const event = {
                headers: {
                    traceparent: 'test-trace-id'
                }
            };

            // Mock propagation.extract
            const mockContext = {} as Context;
            jest.spyOn(propagation, 'extract').mockReturnValue(mockContext);

            await tracedHandler({
                tracer,
                provider,
                name: 'test-handler',
                event,
                fn: async () => 'success'
            });

            expect(propagation.extract).toHaveBeenCalledWith(ROOT_CONTEXT, event.headers);
            expect(tracer.startActiveSpan).toHaveBeenCalledWith(
                'test-handler',
                expect.any(Object),
                mockContext,
                expect.any(Function)
            );
        });

        it('should use custom carrier extractor', async () => {
            const event = {
                customField: {
                    traceparent: 'test-trace-id'
                }
            };

            const getCarrier = (evt: any) => evt.customField;
            const mockContext = {} as Context;
            jest.spyOn(propagation, 'extract').mockReturnValue(mockContext);

            await tracedHandler({
                tracer,
                provider,
                name: 'test-handler',
                event,
                getCarrier,
                fn: async () => 'success'
            });

            expect(propagation.extract).toHaveBeenCalledWith(ROOT_CONTEXT, event.customField);
        });
    });

    describe('error handling', () => {
        it('should handle and record exceptions', async () => {
            const error = new Error('test error');

            await expect(tracedHandler({
                tracer,
                provider,
                name: 'test-handler',
                fn: async () => {
                    throw error;
                }
            })).rejects.toThrow(error);

            expect(mockSpan.recordException).toHaveBeenCalledWith(error);
            expect(mockSpan.setStatus).toHaveBeenCalledWith({
                code: SpanStatusCode.ERROR,
                message: 'test error'
            });
        });
    });

    describe('processor mode handling', () => {
        it('should force flush in sync mode', async () => {
            state.mode = ProcessorMode.Sync;
            state.extensionInitialized = false;

            await tracedHandler({
                tracer,
                provider,
                name: 'test-handler',
                fn: async (span) => 'success'
            });

            expect(provider.forceFlush).toHaveBeenCalled();
        });

        it('should emit handler complete in async mode', async () => {
            const signalSpy = jest.spyOn(handlerComplete, 'signal');
            state.mode = ProcessorMode.Async;
            state.extensionInitialized = true;

            await tracedHandler({
                tracer,
                provider,
                name: 'test-handler',
                fn: async (span) => 'success'
            });

            expect(signalSpy).toHaveBeenCalled();
        });
    });
}); 