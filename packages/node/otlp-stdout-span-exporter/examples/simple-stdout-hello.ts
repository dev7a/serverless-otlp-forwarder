/**
 * Simple example showing basic usage of OTLPStdoutSpanExporter.
 * This example demonstrates:
 * 1. Basic tracer setup
 * 2. Creating a parent span
 * 3. Creating a nested child span
 * 4. Proper cleanup with force flush
 */

import { trace, Span } from '@opentelemetry/api';
import { NodeTracerProvider } from '@opentelemetry/sdk-trace-node';
import { BatchSpanProcessor } from '@opentelemetry/sdk-trace-base';
import { OTLPStdoutSpanExporter } from '../src';

function initTracer(): NodeTracerProvider {
  // Initialize the exporter
  const exporter = new OTLPStdoutSpanExporter();
  // Use batching processor for efficiency
  const processor = new BatchSpanProcessor(exporter);
  // Create a tracer provider
  const provider = new NodeTracerProvider(
    {
      // Register the exporter with the provider
      spanProcessors: [processor]
    }
  );

  // Set as global default tracer provider
  provider.register();

  return provider;
}

async function main() {
  // Initialize tracing
  const provider = initTracer();
  const tracer = trace.getTracer('example/simple');

  // Create a parent span using the correct pattern
  await tracer.startActiveSpan('parent-operation', async (parentSpan: Span) => {
    console.log('Doing work...');

    // Create a nested child span
    await tracer.startActiveSpan('child-operation', async (childSpan: Span) => {
      console.log('Doing more work...');
      childSpan.end();
    });

    parentSpan.end();
  });

  // Ensure all spans are exported
  await provider.forceFlush();

  // Shutdown the provider
  await provider.shutdown();
}

// Run the example
main().catch(console.error);