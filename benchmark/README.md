# Lambda OpenTelemetry Performance Benchmark

This benchmark suite measures and compares the performance overhead of different OpenTelemetry integration approaches in AWS Lambda functions. The goal is to provide quantitative data about the impact of various OpenTelemetry implementations on Lambda performance, particularly focusing on cold start times and execution duration.

## Test Scenarios

We compare three different approaches:
1. Manual instrumentation with OTLP to stdout (this project's approach)
2. AWS Distro for OpenTelemetry Lambda Layer (upstream version)
3. AWS Application Signals (optimized AWS version)

## Test Matrix

### Languages
- Node.js 20.x
- Python 3.12

### Memory Configurations
- 128 MB
- 512 MB
- 1024 MB

## Implementation Structure

The benchmark suite is organized to minimize code duplication while testing different OpenTelemetry implementations:

```
benchmark/
├── functions/          # Lambda function implementations
│   ├── nodejs/
│   │   ├── auto/       # Shared code for ADOT and AppSignals
│   │   │   ├── index.js
│   │   │   └── package.json
│   │   └── manual/     # Manual OTLP implementation
│   │       ├── index.js
│   │       └── package.json
│   └── python/
│       ├── auto/       # Shared code for ADOT and AppSignals
│       │   ├── app.py
│       │   └── requirements.txt
│       └── manual/     # Manual OTLP implementation
│           ├── app.py
│           └── requirements.txt
└── src/                # Rust CLI src tool for running benchmarks
```

### Implementation Details

#### Manual Implementation (`manual/`)
- Uses manual OpenTelemetry SDK initialization
- Configures OTLP exporter for stdout
- Requires full OpenTelemetry SDK dependencies
- Handles context propagation and span lifecycle

#### Auto Implementation (`auto/`)
- Shared code for both ADOT and AppSignals
- Only requires OpenTelemetry API package
- Relies on auto-instrumentation from Lambda layers
- Configuration handled via environment variables and layers

## CLI Tool

The benchmark includes a Rust-based CLI tool for measuring Lambda performance metrics and visualizing results.

### Building the CLI

```bash
cd benchmark
cargo build --release
```

### Usage

The CLI has two main commands:
1. `test`: Run benchmarks and collect metrics
2. `chart`: Generate HTML visualizations from benchmark results

#### Running Benchmarks

```bash
# Basic usage
cargo run --release test <function-name> [options]

# Example: Run benchmark with 128MB memory, 10 concurrent invocations, 3 rounds
cargo run --release test otel-bench-python-stdout -m 128 -c 10 -r 3 --output-dir tmp/python

# Options:
# -m, --memory-size <MB>:          Memory size in MB
# -c, --concurrent-invocations <N>: Number of concurrent invocations per round
# -r, --rounds <N>:                Number of rounds to run
# --output-dir <DIR>:              Directory to save benchmark results
```

The tool provides separate metrics for cold and warm starts:

```
🥶 Cold Start Metrics (10 invocations) | Memory Size: 128 MB

Metric                   Min          P50          P95          Max         Mean
---------------------------------------------------------------------------------------
Init Duration     633.604 ms    682.682 ms    727.279 ms    727.279 ms    683.697 ms
Duration           44.752 ms     64.761 ms     75.354 ms     75.354 ms     63.554 ms
Billed Duration    45.000 ms     65.000 ms     76.000 ms     76.000 ms     64.100 ms
Memory Used          68.0 MB       68.0 MB       68.0 MB       68.0 MB       68.0 MB

🔥️ Warm Start Metrics (30 invocations) | Memory Size: 128 MB

Metric                   Min          P50          P95          Max         Mean
---------------------------------------------------------------------------------------
Duration            3.698 ms     14.752 ms     32.839 ms     39.228 ms     15.124 ms
Billed Duration     4.000 ms     15.000 ms     33.000 ms     40.000 ms     15.533 ms
Memory Used          68.0 MB       69.0 MB       69.0 MB       69.0 MB       69.0 MB
```

#### Generating Visualizations

After running benchmarks, you can generate HTML charts to visualize the results:

```bash
# Basic usage
cargo run --release chart <input-dir> --output-dir <output-dir>

# Example: Generate charts from benchmark results
cargo run --release chart tmp/python --output-dir results/python
```

This will generate three HTML files in the output directory:
- `cold_starts.html`: Cold start performance metrics
- `warm_starts.html`: Warm start performance metrics
- `memory_usage.html`: Memory usage statistics

### Required Permissions

The CLI requires AWS credentials with the following permissions:

```json
{
    "Version": "2012-10-17",
    "Statement": [
        {
            "Effect": "Allow",
            "Action": [
                "lambda:InvokeFunction",
                "lambda:UpdateFunctionConfiguration"
            ],
            "Resource": "arn:aws:lambda:*:*:function:*"
        }
    ]
}
```

### Implementation Details

The CLI tool:
1. Invokes the Lambda function with the specified configuration
2. Captures execution logs directly from the Lambda Invoke API
3. Processes the logs, which are returned as base64-encoded JSON Lines
4. Extracts metrics from the last `platform.report` entry, which has this structure:
   ```json
   {
       "time": "2024-12-04T00:08:48.503Z",
       "type": "platform.report",
       "record": {
           "requestId": "3b07b491-6f5d-44b6-b6a0-56cdde4e4159",
           "metrics": {
               "durationMs": 2.666,
               "billedDurationMs": 3,
               "memorySizeMB": 128,
               "maxMemoryUsedMB": 30,
               "initDurationMs": 89.363
           },
           "status": "success"
       }
   }
   ```
   Note: The logs may contain other JSON lines (OpenTelemetry output, custom logs, etc.), 
   but the platform report is always the last entry, containing the official execution metrics.

5. Calculates statistics for cold and warm starts
6. Can force cold starts by updating the function's memory configuration

## Deployment

### Stack Configuration
The benchmark suite is deployed as a CloudFormation/SAM stack. The stack name is provided as a parameter and is used as a prefix for all function names.

### Function Naming Convention
All functions follow the naming pattern: `${StackName}-[runtime]-[implementation]`

#### Test Functions
Node.js implementations:
- `${StackName}-node-stdout`: Manual OTLP instrumentation with stdout exporter
- `${StackName}-node-adot`: AWS Distro for OpenTelemetry Lambda Layer
- `${StackName}-node-appsignals`: AWS Application Signals

Python implementations:
- `${StackName}-python-stdout`: Manual OTLP instrumentation with stdout exporter
- `${StackName}-python-adot`: AWS Distro for OpenTelemetry Lambda Layer
- `${StackName}-python-appsignals`: AWS Application Signals

### Function Configuration
- Architecture: ARM64
- Runtime: Node.js 20.x and Python 3.12

## Test Methodology

1. **Cold Start Test**
   - Change memory configuration to force cold start
   - Execute N concurrent invocations
   - First invocation in each set will be cold start
   - Subsequent invocations will be warm starts
   - Repeat X times

2. **Warm Start Test**
   - Execute subsequent invocations without memory changes
   - Collect metrics for warm start performance
