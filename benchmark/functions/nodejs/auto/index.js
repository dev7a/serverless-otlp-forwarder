const { trace } = require('@opentelemetry/api');

const tracer = trace.getTracer('benchmark-test');

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
        await tracer.startActiveSpan(`operation_depth_${depth}_iter_${i}`, async (currentSpan) => {
            currentSpan.setAttributes({
                depth,
                iteration: i
            });
            
            await processLevel(depth - 1, iterations);
            currentSpan.end();
        });
    }
}

/**
 * Lambda handler that creates a tree of spans based on input parameters.
 * 
 * @param {Object} event - Lambda event containing depth and iterations parameters
 * @param {Object} context - Lambda context
 * @returns {Object} Response containing the benchmark parameters
 */
exports.handler = async (event, context) => {
    const depth = event.depth ?? 2;
    const iterations = event.iterations ?? 2;
    
    return await tracer.startActiveSpan('benchmark-execution', async (rootSpan) => {
        await processLevel(depth, iterations);
        rootSpan.end();
        
        return {
            statusCode: 200,
            body: JSON.stringify({
                message: 'Benchmark complete',
                depth,
                iterations
            })
        };
    });
}; 