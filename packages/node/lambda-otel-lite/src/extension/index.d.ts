import { NodeTracerProvider } from '@opentelemetry/sdk-trace-node';
import { ProcessorMode } from '../types';

export interface ExtensionState {
    provider: NodeTracerProvider | null;
    mode: ProcessorMode | null;
}

/** @internal */
export const state: ExtensionState & { provider: NodeTracerProvider };
export function isDebugEnabled(): boolean;
export function shouldForceFlush(): boolean;
export function withDebugTiming<T>(operation: () => Promise<T>, description: string): Promise<T>;
export function initializeInternalExtension(): Promise<void>; 