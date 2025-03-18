const { trace } = require('@opentelemetry/api');

const tracer = trace.getTracer('benchmark-test');

/**
 * Process a single Kinesis record with OpenTelemetry instrumentation.
 * 
 * @param {Object} record - A Kinesis record containing sequence number, partition key, and base64 encoded data
 */
async function processRecord(record) {
    return await tracer.startActiveSpan('process-record', async (span) => {
        try {
            // Extract record metadata
            const sequenceNumber = record?.kinesis?.sequenceNumber ?? '';
            const partitionKey = record?.kinesis?.partitionKey ?? '';

            // Set span attributes for record context
            span.setAttributes({
                'stream.sequence_number': sequenceNumber,
                'stream.partition_key': partitionKey,
            });

            // Decode and process the record data
            const encodedData = record?.kinesis?.data ?? '';
            
            await tracer.startActiveSpan('decode-record', async (decodeSpan) => {
                try {
                    const data = Buffer.from(encodedData, 'base64');
                    decodeSpan.setAttribute('stream.data.size', data.length);

                    // Parse JSON data
                    await tracer.startActiveSpan('parse-record', async (parseSpan) => {
                        try {
                            const jsonData = JSON.parse(data.toString());

                            // Set business-specific attributes
                            parseSpan.setAttributes({
                                'stock.ticker': jsonData.ticker,
                                'stock.price': jsonData.price,
                                'stock.event_time': jsonData.event_time,
                            });
                        } finally {
                            parseSpan.end();
                        }
                    });
                } finally {
                    decodeSpan.end();
                }
            });
        } finally {
            span.end();
        }
    });
}

/**
 * Process a batch of Kinesis records with OpenTelemetry instrumentation.
 * 
 * @param {Array} records - List of Kinesis records to process
 */
async function testStreamOperations(records) {
    return await tracer.startActiveSpan('stream-operations', async (span) => {
        try {
            span.setAttribute('stream.batch.size', records.length);
            
            // Process records sequentially to maintain order
            for (const record of records) {
                await processRecord(record);
            }
        } finally {
            span.end();
        }
    });
}

/**
 * Lambda handler that processes Kinesis records with OpenTelemetry instrumentation.
 * 
 * This handler processes each record in the Kinesis event batch, creating spans
 * for batch processing and individual record processing operations. Each record
 * contains stock ticker data with price information.
 */
exports.handler = async (event, context) => {
    const records = event?.Records ?? [];
    
    await testStreamOperations(records);
        
    return {
        statusCode: 200,
        body: JSON.stringify({
            message: 'Benchmark complete',
            records_processed: records.length
        })
    };
}; 