# Serverless OTLP Forwarder Benchmark Results

This report contains performance benchmarks for different implementations of the serverless-otlp-forwarder across multiple languages and memory configurations.

## Overview

Please review at the template.yaml in the testbed directory for the complete list of functions and configurations used in this test.
Things to note:

- The `rotel` function is used to test the Rotel collector.
- The `adot` function is used to test the Adot collector.
- The `otel` function is used to test the otel collector.
- The `appsignals` function is used to test the appsignals collector.
- The `stdout` function is used to test the stdout exporter using lambda otel lite in sync mode.
- The `async` function is used to test the stdout exporter using lambda otel lite in async mode.

> **Note:** All functions are configured to use a mock otlp endpoint implemented with API Gateway with the mock integration. This is intended to simulate a real otlp collector http v1/traces endpoint, and to guarantee consistently the lowest latency for all tests. In the real world, the latency will likely be higher due to the network round trip and the collector's processing time.


## Testbed Function

Each function is running a simple workload of creating a hierarchy of spans. The depth and number of iterations for span creation can be controlled via the event payload. The default used in this run is 4 iterations at depth 2. The functions are intentionally not doing anything else, to isolate the overhead of the telemetry system, so we are not doing I/O or other heavy computations.
In pseudocode, the workload is as follows:

```
DEFAULT_DEPTH = 2
DEFAULT_ITERATIONS = 4

function process_level(depth, iterations):
    if depth <= 0:
        return
    for i from 0 to iterations-1:
        start span "operation_depth_{depth}_iter_{i}"
        set span attributes: depth, iteration, payload (256 'x')
        process_level(depth - 1, iterations)

function handler(event, lambda_context):
    depth = event.depth or DEFAULT_DEPTH
    iterations = event.iterations or DEFAULT_ITERATIONS
    process_level(depth, iterations)
    return {
        statusCode: 200,
        body: {
            message: "Benchmark complete",
            depth: depth,
            iterations: iterations
        }
    }
```

## Metrics
The benchmarks compare several metrics:

- **Cold Start Performance**: Initialization time, server duration, and total cold start times
- **Warm Start Performance**: Client latency, server processing time, and extension overhead
- **Resource Usage**: Memory consumption across different configurations
- **Bytes Sent**: Bytes sent as response to the client (it's expected to be the same for all tests)

## Test Configuration

All tests were run with the following parameters:
- 100 invocations per function
- 10 concurrent requests
- AWS Lambda arm64 architecture
- Same payload size for all tests

The layers used for this test are:
| Layer Type | Implementation | ARN |
|------------|---------------|-----|
| **Collector Layers** | | |
| | otel | arn:aws:lambda:us-east-1:184161586896:layer:opentelemetry-collector-arm64-0_14_0:1 |
| | adot | arn:aws:lambda:us-east-1:901920570463:layer:aws-otel-collector-arm64-ver-0-115-0:3 |
| | rotel | arn:aws:lambda:us-east-1:418653438961:layer:rotel-extension-arm64-alpha:21 |
| **Python Layers** | | |
| | adot | arn:aws:lambda:us-east-1:901920570463:layer:aws-otel-python-arm64-ver-1-29-0:2 |
| | otel | arn:aws:lambda:us-east-1:184161586896:layer:opentelemetry-python-0_13_0:1 |
| | rotel | arn:aws:lambda:us-east-1:184161586896:layer:opentelemetry-python-0_13_0:1 |
| | appsignals | arn:aws:lambda:us-east-1:615299751070:layer:AWSOpenTelemetryDistroPython:12 |
| **Node.js Layers** | | |
| | adot | arn:aws:lambda:us-east-1:901920570463:layer:aws-otel-nodejs-arm64-ver-1-30-1:2 |
| | otel | arn:aws:lambda:us-east-1:184161586896:layer:opentelemetry-nodejs-0_13_0:1 |
| | rotel | arn:aws:lambda:us-east-1:184161586896:layer:opentelemetry-nodejs-0_13_0:1 |
| | appsignals | arn:aws:lambda:us-east-1:615299751070:layer:AWSOpenTelemetryDistroJs:6 |

These environment variables are set for all functions:
| Environment Variable | Value |
|----------------------|-------|
| OTEL_METRICS_EXPORTER | none |
| OTEL_LOGS_EXPORTER | none |
| OTEL_TRACES_EXPORTER | otlp |
| OTEL_PYTHON_DISABLED_INSTRUMENTATIONS | aiohttp,aiohttp-client,asyncpg,boto,celery,django,elasticsearch,falcon,fastapi,flask,grpc_aio_client,grpc_aio_server,grpc_client,grpc_server,jinja2,mysql,psycopg2,pymemcache,pymongo,pymysql,pyramid,redis,sqlalchemy,starlette,tornado |
| OTEL_NODE_DISABLED_INSTRUMENTATIONS | amqplib,bunyan,cassandra-driver,connect,cucumber,dataloader,dns,express,generic-pool,graphql,grpc,hapi,http,ioredis,kafkajs,knex,koa,lru-memoizer,memcached,mongodb,mongoose,mysql2,mysql,nestjs-core,net,pg,pino,redis,redis-4,restify,router,socket.io,tedious,undici,winston |
| OTEL_TRACES_SAMPLER | always_on |
| OTEL_EXPORTER_OTLP_ENDPOINT | http://localhost:4318 |
| OTEL_EXPORTER_OTLP_PROTOCOL | http/protobuf |
