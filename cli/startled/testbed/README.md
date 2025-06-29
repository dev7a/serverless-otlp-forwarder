# `startled` Testbed
**Benchmark your Lambda telemetry. Validate your configurations.**

This directory provides a comprehensive testbed for rigorously evaluating OpenTelemetry (Otel) implementations on AWS Lambda, designed to be used with the `startled` CLI tool located in the parent directory. Its **primary objective** is to compare the performance overhead, cold start impact, and overall characteristics of traditional Otel Lambda Extensions (such as stock Otel, AWS Distro for OpenTelemetry (ADOT), and AWS Application Signals) against the lean approach of **serializing OpenTelemetry Protocol (OTLP) data directly to standard output (stdout)**. This direct-to-stdout method, utilizing Protobuf encoding, is a core concept underpinning the `serverless-otlp-forwarder` project and is represented here by the `*-stdout` variants.
It should be noted that the `stdout` implementations are doing manual instrumentation of the application code, while the other implementations are using auto-instrumentation. This is a key difference in the approach and is a factor that should be considered when evaluating the results.


The testbed facilitates this comparison across:
- Various Lambda runtimes (Rust, Node.js, Python).
- Different OpenTelemetry SDKs, distributions, and auto-instrumentation agents.
- Configurations including AWS CloudWatch Application Signals.

## Directory Structure

```
benchmark/testbed/
├── Makefile             # Main Makefile to run benchmarks and generate reports
├── README.md            # This file
├── functions/           # Lambda function source code and configurations
│   ├── confmaps/        # Collector configuration files (otel, adot, rotel)
│   │   ├── Makefile     # Makefile to package collector configs
│   │   ├── adot/
│   │   │   └── collector.yaml
│   │   ├── otel/
│   │   │   └── collector.yaml
│   │   └── rotel/
│   │       └── rotel.env
│   ├── nodejs/
│   │   ├── auto/        # Code for auto-instrumented Node.js functions
│   │   │   └── index.js
│   │   └── manual/      # Code for manually configured/stdout Node.js function
│   │       ├── index.js
│   │       └── init.js  # Helper for lambda-otel-lite
│   ├── python/
│   │   ├── auto/        # Code for auto-instrumented Python functions
│   │   │   └── main.py
│   │   └── manual/      # Code for manually configured/stdout Python function
│   │       └── main.py
│   └── rust/
│       ├── collector/   # Common code for Rust functions using an Otel collector extension
│       │   └── src/main.rs
│       └── stdout/      # Code for Rust function with lambda-otel-lite (baseline)
│           └── src/main.rs
├── samconfig.toml       # AWS SAM CLI configuration for deployment
├── template.yaml        # AWS SAM template defining all Lambda functions and resources
└── test-events/         # Directory for test event JSON files (if any)
```

## Benchmark Scope and Configurations

The `template.yaml` defines a suite of Lambda functions. Each function executes a common workload to ensure fair comparison: recursively creating a tree of spans to simulate application activity and stress the telemetry system. The depth and number of iterations for span creation can be controlled via the event payload.

### Common Workload
All benchmarked functions perform the same core task:
- They receive `depth` and `iterations` parameters in their input event.
- They recursively create a hierarchy of spans. `depth` controls how many levels deep the hierarchy goes, and `iterations` controls how many child spans are created at each level.
- Each span includes attributes like `depth`, `iteration`, and a fixed-size `payload`.

This consistent workload allows for direct comparison of telemetry overhead across different setups.

### Configurations Under Test

The following configurations are benchmarked, with a focus on comparing extension-based solutions against direct OTLP-over-stdout:

#### Rust (`provided.al2023` runtime)
-   **`rust-stdout`**:
    -   Code: `functions/rust/stdout/`
    -   Instrumentation: Uses `lambda-otel-lite`. **Represents the baseline direct OTLP-over-stdout approach**, potentially with traces output to stdout or a very lightweight OTLP export.
-   **`rust-otel`**:
    -   Code: `functions/rust/collector/`
    -   Instrumentation: Generic OpenTelemetry. Application code exports OTLP to `localhost:4318`. Relies on the generic Otel Lambda extension layer for collection and export, using `/opt/otel/collector.yaml`.
-   **`rust-adot`**:
    -   Code: `functions/rust/collector/`
    -   Instrumentation: AWS Distro for OpenTelemetry (ADOT). Application code exports OTLP to `localhost:4318`. Relies on the ADOT Lambda extension layer, using `/opt/adot/collector.yaml`.
-   **`rust-rotel`**:
    -   Code: `functions/rust/collector/`
    -   Instrumentation: Rotel. Application code exports OTLP to `localhost:4318`. Relies on the Rotel Lambda extension layer, using `/opt/rotel/rotel.env`.

#### Node.js (`nodejs22.x` runtime)
-   **`node-stdout`**:
    -   Code: `functions/nodejs/manual/`
    -   Instrumentation: Uses `@dev7a/lambda-otel-lite`. **Serves as the Node.js baseline for direct OTLP-over-stdout.**
-   **`node-otel`**:
    -   Code: `functions/nodejs/auto/`
    -   Instrumentation: Generic OpenTelemetry. Relies on the Otel Node.js Lambda layer and `AWS_LAMBDA_EXEC_WRAPPER=/opt/otel-handler` for auto-instrumentation. Uses `/opt/otel/collector.yaml`.
-   **`node-adot`**:
    -   Code: `functions/nodejs/auto/`
    -   Instrumentation: AWS Distro for OpenTelemetry (ADOT). Relies on the ADOT Node.js Lambda layer and wrapper for auto-instrumentation. Uses `/opt/adot/collector.yaml`.
-   **`node-rotel`**:
    -   Code: `functions/nodejs/auto/`
    -   Instrumentation: Rotel. Relies on specific Rotel Node.js Lambda layers and wrapper for auto-instrumentation. Uses `/opt/rotel/rotel.env`.
-   **`node-signals`**:
    -   Code: `functions/nodejs/auto/`
    -   Instrumentation: AWS CloudWatch Application Signals. Relies on the AppSignals Node.js Lambda layer and `AWS_LAMBDA_EXEC_WRAPPER=/opt/otel-instrument`.

#### Python (`python3.13` runtime)
-   **`python-stdout`**:
    -   Code: `functions/python/manual/`
    -   Instrumentation: Uses a Python `lambda_otel_lite` library and an `OTLPStdoutSpanExporter`. **Acts as the Python baseline for direct OTLP-over-stdout.**
-   **`python-otel`**:
    -   Code: `functions/python/auto/`
    -   Instrumentation: Generic OpenTelemetry. Relies on the Otel Python Lambda layer and `AWS_LAMBDA_EXEC_WRAPPER=/opt/otel-instrument` for auto-instrumentation. Uses `/opt/otel/collector.yaml`.
-   **`python-adot`**:
    -   Code: `functions/python/auto/`
    -   Instrumentation: AWS Distro for OpenTelemetry (ADOT). Relies on the ADOT Python Lambda layer and wrapper for auto-instrumentation. Uses `/opt/adot/collector.yaml`.
-   **`python-rotel`**:
    -   Code: `functions/python/auto/`
    -   Instrumentation: Rotel. Relies on specific Rotel Python Lambda layers and wrapper for auto-instrumentation. Uses `/opt/rotel/rotel.env`.
-   **`python-signals`**:
    -   Code: `functions/python/auto/`
    -   Instrumentation: AWS CloudWatch Application Signals. Relies on the AppSignals Python Lambda layer and `AWS_LAMBDA_EXEC_WRAPPER=/opt/otel-instrument`.

### Supporting Resources
-   **`CollectorConfiglLayer`**: A Lambda layer built from `functions/confmaps/` that packages the `collector.yaml` (for Otel/ADOT) and `rotel.env` (for Rotel) configuration files. These are made available to the respective collector extensions at runtime (e.g., `/opt/otel/collector.yaml`).
-   **`MockOTLPReceiver`**: An API Gateway endpoint defined in `template.yaml` that acts as a mock OTLP receiver. The collector configurations in `functions/confmaps/` are set up to send telemetry to this mock endpoint by default (via the `MOCK_OTLP_ENDPOINT` environment variable). This allows testing telemetry export paths without requiring a full backend observability platform and preventing variability in the results due to external factors.

## Prerequisites

1.  **AWS SAM CLI**: For deploying the CloudFormation stack. ([Installation Guide](https://docs.aws.amazon.com/serverless-application-model/latest/developerguide/install-sam-cli.html))
2.  **Rust & Cargo**: For building Rust-based Lambda functions and the `startled` tool itself. ([Installation Guide](https://www.rust-lang.org/tools/install))
3.  **Node.js & npm**: For Node.js based Lambda functions.
4.  **Python**: For Python-based Lambda functions.
5.  **Docker**: SAM CLI uses Docker to build Lambda deployment packages.
6.  The `startled` CLI tool (from the parent `cli/startled/` directory) must be installed. Navigate to `cli/startled/` and run `cargo install --path .`.

## Deployment

The testbed is deployed as an AWS CloudFormation stack using the AWS SAM CLI.

1.  Navigate to the `cli/startled/testbed/` directory.
2.  Build the SAM application:
    ```bash
    sam build --beta-features
    ```
    *(The `--beta-features` flag is needed for `rust-cargolambda` build support, as indicated in `samconfig.toml`)*.
3.  Deploy the stack:
    ```bash
    sam deploy --guided
    ```
    Follow the prompts. You can accept the defaults specified in `samconfig.toml` (e.g., stack name `benchmark`, region `us-east-1`). Ensure you acknowledge IAM role creation capabilities.

## Running Benchmarks

The `Makefile` in this directory provides convenient targets for running benchmarks using the `startled` CLI.

### Makefile Configuration
Key environment variables to customize benchmark runs:
-   `CONCURRENCY`: Number of concurrent invocations per repetition (default: `10`).
-   `ROUNDS`: Number of warm start repetitions (default: `100`).
-   `STACK_NAME`: Name of the deployed CloudFormation stack (default: `benchmark`).
-   `RESULT_DIR`: Subdirectory name for JSON results (default: `results`). Output path: `/tmp/<STACK_NAME>/<RESULT_DIR>`.
-   `REPORTS_DIR`: Subdirectory name for HTML reports (default: `reports`). Output path: `/tmp/<STACK_NAME>/<REPORTS_DIR>`.

### Available Makefile Targets

#### `make help`
Displays available targets and their descriptions.

#### `make test-runtime RUNTIME=<runtime> MEMORY_CONFIG=<memory>`
Runs benchmarks for a specific runtime and memory configuration.
-   `RUNTIME=<runtime>`: Specifies the runtime. Can be `rust`, `node`, or `python`.
-   `MEMORY_CONFIG=<memory>`: Sets the memory configuration. Can be `128`, `512`, `1024` (or other values defined in the `Makefile`).

**Example:**
```bash
make test-runtime RUNTIME=python MEMORY_CONFIG=512
```
This command will invoke the `startled stack` command, targeting all functions in the deployed stack whose names contain "python" (e.g., `python-stdout`, `python-otel`), with the specified memory, concurrency, and rounds. Results are saved to `/tmp/benchmark/results/`.

#### `make test-all-runtimes`
Runs benchmarks for all defined runtimes (`rust`, `node`, `python`) and memory configurations (`128`, `512`, `1024` MB). This is a comprehensive test suite and may take a significant amount of time.

#### `make report`
Generates an HTML report from the collected JSON results using the `startled report` command. The report will be located in `/tmp/benchmark/reports/index.html`.

#### `make clean`
Removes all generated benchmark result files (JSON) from `/tmp/benchmark/results/` and HTML report files from `/tmp/benchmark/reports/`. Prompts for confirmation.

### Example Workflow
1.  **Deploy the stack:**
    ```bash
    cd cli/startled/testbed/
    sam build --beta-features
    sam deploy --guided
    ```
2.  **Run benchmarks** (e.g., for Python functions with 128MB memory):
    ```bash
    make test-runtime RUNTIME=python MEMORY_CONFIG=128
    ```
    Or, run the full suite (this will take some time):
    ```bash
    make test-all-runtimes
    ```
3.  **Generate the HTML report:**
    ```bash
    make report
    ```
    Open `/tmp/benchmark/reports/index.html` in your browser to view the results.

## Customization
-   **Memory Configurations**: Modify `MEMORY_CONFIGS` in the `Makefile` to test different memory sizes.
-   **Concurrency/Rounds**: Change `CONCURRENCY` and `ROUNDS` in the `Makefile` or override them as environment variables when running `make` commands.
-   **Function Workload**: Modify the `process_level` function (or equivalent) within the language-specific source files in `functions/*/` to change the nature of the work being done by the Lambda functions.
-   **Collector Configuration**: Adjust `collector.yaml` or `rotel.env` files in `functions/confmaps/` to alter how the OTel collector extensions behave (e.g., change exporters, processors, sampling). Remember to rebuild and redeploy the `CollectorConfiglLayer` (which happens automatically with `sam build` if changes are detected in `functions/confmaps/`).
-   **OTel Endpoint**: By default, collectors export to a mock API Gateway. To send data to a real observability backend, update the `OTEL_EXPORTER_OTLP_ENDPOINT` in `template.yaml` (either in `Globals` or function-specific environment variables) or directly in the collector configuration files, then redeploy the stack.

This testbed provides a flexible and robust environment for evaluating and comparing OpenTelemetry performance on AWS Lambda, with a particular focus on the trade-offs between extension-based and direct-to-stdout telemetry solutions.
