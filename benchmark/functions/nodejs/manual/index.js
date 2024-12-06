const { NodeTracerProvider } = require('@opentelemetry/sdk-trace-node');
const { BatchSpanProcessor } = require('@opentelemetry/sdk-trace-base');
const { Resource } = require('@opentelemetry/resources');
const { trace } = require('@opentelemetry/api');
const { StdoutOTLPExporterNode } = require('@dev7a/otlp-stdout-exporter');


function createProvider() {
    // Create base resource
    const resource = new Resource({
        'service.name': process.env.OTEL_SERVICE_NAME || 'benchmark-test'
    });

    const provider = new NodeTracerProvider({
        resource
    });

    // Configure stdout exporter
    const exporter = new StdoutOTLPExporterNode();

    // Add the exporter to the provider with batch processing
    provider.addSpanProcessor(
        new BatchSpanProcessor(exporter, {
            scheduledDelayMillis: 1000,
            maxExportBatchSize: 512,
            maxQueueSize: 2048
        })
    );

    return provider;
}

/**
 * Recursively create a tree of spans to measure OpenTelemetry overhead.
 * 
 * @param {number} depth - Current depth level (decrements towards 0)
 * @param {number} iterations - Number of spans to create at each level
 */
async function processLevel(depth, iterations) {
    if (depth <= 0) {
        return;
    }
    
    for (let i = 0; i < iterations; i++) {
        const currentSpan = tracer.startSpan(`operation_depth_${depth}_iter_${i}`);
        
        try {
            currentSpan.setAttributes({
                depth,
                iteration: i,
                timestamp: Date.now()
            });
            
            await processLevel(depth - 1, iterations);
        } finally {
            currentSpan.end();
        }
    }
}

/**
 * Lambda handler that creates a tree of spans based on input parameters.
 * 
 * @param {Object} event - Lambda event containing depth and iterations parameters
 * @param {Object} context - Lambda context
 * @returns {Object} Response containing the benchmark parameters
 */
// Initialize provider
const provider = createProvider();
provider.register();
const tracer = trace.getTracer('benchmark-test');

exports.handler = async (event, context) => {
    
    const depth = event.depth ?? 2;
    const iterations = event.iterations ?? 2;
    
    const rootSpan = tracer.startSpan('benchmark-execution');
    try {
        await processLevel(depth, iterations);
    } finally {
        rootSpan.end();
    }
    
    // Ensure all spans are exported
    await provider.forceFlush();
    
    return {
        statusCode: 200,
        body: JSON.stringify({
            message: 'Benchmark complete',
            depth,
            iterations
        })
    };
}; 